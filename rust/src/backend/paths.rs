//! Game-tree locations and case-insensitive path resolution.
//!
//! Two directories matter, and they're not always the same one:
//! - `launcher_dir`: where the launcher executable itself lives - fixed for the life of the
//!   process. Holds `launcher.ini`. (The Python build's PyInstaller temp-extraction
//!   directory for bundled schemas has no Rust equivalent: schemas are compiled directly
//!   into this binary - see `schema.rs` once it's ported - so there's nothing to extract.)
//! - `game_dir()`: the active campaign's root - its engine binary, data, `mods/`, config
//!   files. Defaults to `launcher_dir` (the Vanilla/CE campaign IS that folder) but a total
//!   conversion campaign roots at a subfolder underneath it instead; see `campaigns.rs` once
//!   it's ported. Switching the active campaign calls `set_campaign_root()`.
//!
//! Python kept these as module-level globals. Here they live on a `Paths` value instead
//! (shared via `Rc` where needed) so tests don't fight over shared global state, and so it's
//! obvious from a function's signature whether it touches the active campaign root at all.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;

pub const PREVIEW_W: u32 = 224;
pub const PREVIEW_H: u32 = 133;

pub struct Paths {
    launcher_dir: PathBuf,
    active_root: RefCell<PathBuf>,
}

/// Shared handle to a `Paths`: cheap to clone, cheap to pass around - everything that needs
/// to know "where is the game" holds one of these rather than reaching for a global.
pub type SharedPaths = Rc<Paths>;

impl Paths {
    /// Vanilla/CE is active by default - `active_root` starts out equal to `launcher_dir`.
    pub fn new(launcher_dir: PathBuf) -> SharedPaths {
        let active_root = RefCell::new(launcher_dir.clone());
        Rc::new(Paths { launcher_dir, active_root })
    }

    /// Locate the real launcher directory from the running executable, for production use.
    /// Tests build a `Paths` directly from a temp directory instead (`Paths::new(..)`).
    pub fn from_current_exe() -> SharedPaths {
        let dir = std::env::current_exe()
            .expect("could not locate the running executable")
            .parent()
            .expect("executable has no parent directory")
            .to_path_buf();
        Self::new(dir)
    }

    pub fn launcher_dir(&self) -> &Path {
        &self.launcher_dir
    }

    pub fn launcher_ini(&self) -> PathBuf {
        self.launcher_dir.join("launcher.ini")
    }

    /// The active campaign's root. Vanilla/CE (`launcher_dir`) until `set_campaign_root()`
    /// points elsewhere.
    pub fn game_dir(&self) -> PathBuf {
        self.active_root.borrow().clone()
    }

    pub fn set_campaign_root(&self, root: PathBuf) {
        *self.active_root.borrow_mut() = root;
    }

    /// Temporarily point `game_dir()` elsewhere - e.g. so an installer that's generic over
    /// "the active campaign's root" can be pointed at a campaign other than the one
    /// currently active, such as fetching a native engine binary into a freshly-installed
    /// total conversion's own root. Restores the previous root when the returned guard is
    /// dropped (including on an early return or panic) - this is the same shape as Python's
    /// `@contextmanager campaign_root_override`.
    pub fn override_campaign_root(&self, root: PathBuf) -> CampaignRootOverride<'_> {
        let previous = self.game_dir();
        self.set_campaign_root(root);
        CampaignRootOverride { paths: self, previous }
    }

    /// The active campaign's engine binary, if either known name exists; otherwise the
    /// Linux name, so callers have something to report as missing.
    pub fn game_bin(&self) -> PathBuf {
        let root = self.game_dir();
        for name in ["fallout2-ce", "fallout2-ce.exe"] {
            let candidate = root.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
        root.join("fallout2-ce")
    }

    pub fn backup_dir(&self) -> PathBuf {
        self.game_dir().join("backup").join("launcher")
    }

    /// Resolve a path case-insensitively the way CE does, under `root` (default: the active
    /// campaign's root).
    pub fn ci_path(&self, rel: &str, root: Option<&Path>) -> PathBuf {
        let mut cur = root.map(Path::to_path_buf).unwrap_or_else(|| self.game_dir());
        for part in rel.split(['\\', '/']) {
            if part.is_empty() {
                continue;
            }
            let mut candidate = cur.join(part);
            if !candidate.exists() && cur.is_dir() {
                let lower = part.to_lowercase();
                if let Ok(entries) = std::fs::read_dir(&cur) {
                    for entry in entries.flatten() {
                        if entry.file_name().to_string_lossy().to_lowercase() == lower {
                            candidate = entry.path();
                            break;
                        }
                    }
                }
            }
            cur = candidate;
        }
        cur
    }

    pub fn mod_installed(&self, dat: &str) -> bool {
        self.ci_path(&format!("mods/{dat}"), None).is_file()
    }
}

/// RAII guard returned by `Paths::override_campaign_root` - put the previous root back on
/// drop.
pub struct CampaignRootOverride<'a> {
    paths: &'a Paths,
    previous: PathBuf,
}

impl Drop for CampaignRootOverride<'_> {
    fn drop(&mut self) {
        self.paths.set_campaign_root(self.previous.clone());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &Path) -> SharedPaths {
        Paths::new(dir.to_path_buf())
    }

    #[test]
    fn game_dir_defaults_to_launcher_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());
        assert_eq!(paths.game_dir(), tmp.path());
        assert_eq!(paths.launcher_ini(), tmp.path().join("launcher.ini"));
    }

    #[test]
    fn set_campaign_root_changes_game_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());
        let other = tmp.path().join("Sonora");
        paths.set_campaign_root(other.clone());
        assert_eq!(paths.game_dir(), other);
    }

    #[test]
    fn override_campaign_root_restores_on_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());
        let original = paths.game_dir();
        let other = tmp.path().join("Nevada");
        {
            let _guard = paths.override_campaign_root(other.clone());
            assert_eq!(paths.game_dir(), other);
        }
        assert_eq!(paths.game_dir(), original);
    }

    #[test]
    fn ci_path_finds_mismatched_case() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Mods")).unwrap();
        std::fs::write(tmp.path().join("Mods").join("NPCArmor.dat"), b"").unwrap();
        let paths = paths_in(tmp.path());

        let resolved = paths.ci_path("mods/npcarmor.dat", None);
        assert_eq!(resolved, tmp.path().join("Mods").join("NPCArmor.dat"));
    }

    #[test]
    fn ci_path_is_exact_when_case_already_matches() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout2.cfg"), b"").unwrap();
        let paths = paths_in(tmp.path());

        assert_eq!(paths.ci_path("fallout2.cfg", None), tmp.path().join("fallout2.cfg"));
    }

    #[test]
    fn mod_installed_is_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        std::fs::write(tmp.path().join("mods").join("FO2Tweaks.dat"), b"").unwrap();
        let paths = paths_in(tmp.path());

        assert!(paths.mod_installed("fo2tweaks.dat"));
        assert!(!paths.mod_installed("missing.dat"));
    }

    #[test]
    fn game_bin_falls_back_to_linux_name_when_absent() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());
        assert_eq!(paths.game_bin(), tmp.path().join("fallout2-ce"));
    }

    #[test]
    fn game_bin_prefers_whichever_binary_exists() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout2-ce.exe"), b"").unwrap();
        let paths = paths_in(tmp.path());
        assert_eq!(paths.game_bin(), tmp.path().join("fallout2-ce.exe"));
    }
}
