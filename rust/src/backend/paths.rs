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

/// `game_bin()`'s default search names - Vanilla/CE's own engine binary. Every other
/// campaign whose own `bin_names` includes these (Et Tu, Nevada - both built on fallout2-ce)
/// still resolves correctly; only a campaign with its own distinct engine (Sonora's
/// `fallout-fission`) needs `set_campaign_bin_names()` called with its own list.
pub const CE_BIN_NAMES: &[&str] = &["fallout2-ce", "fallout2-ce.exe"];

pub struct Paths {
    launcher_dir: PathBuf,
    active_root: RefCell<PathBuf>,
    active_bin_names: RefCell<&'static [&'static str]>,
}

/// Shared handle to a `Paths`: cheap to clone, cheap to pass around - everything that needs
/// to know "where is the game" holds one of these rather than reaching for a global.
pub type SharedPaths = Rc<Paths>;

impl Paths {
    /// Vanilla/CE is active by default - `active_root` starts out equal to `launcher_dir`.
    pub fn new(launcher_dir: PathBuf) -> SharedPaths {
        let active_root = RefCell::new(launcher_dir.clone());
        Rc::new(Paths { launcher_dir, active_root, active_bin_names: RefCell::new(CE_BIN_NAMES) })
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

    /// Which names `game_bin()` searches `game_dir()` for - defaults to `CE_BIN_NAMES`.
    /// `campaigns::activate()` calls this together with `set_campaign_root()` whenever the
    /// active campaign has its own distinct engine binary (Sonora's `fallout-fission`);
    /// campaigns built on fallout2-ce (Et Tu, Nevada) never need to, since `CE_BIN_NAMES` is
    /// already a subset of their own `bin_names`.
    pub fn set_campaign_bin_names(&self, names: &'static [&'static str]) {
        *self.active_bin_names.borrow_mut() = names;
    }

    /// Temporarily point `game_dir()` (and `game_bin()`'s search names, always back to CE's
    /// own) elsewhere - e.g. so an installer that's generic over "the active campaign's root"
    /// can be pointed at a campaign other than the one currently active, such as fetching
    /// CE's native engine binary into a freshly-installed total conversion's own root.
    /// Restores the previous root and bin names when the returned guard is dropped (including
    /// on an early return or panic) - this is the same shape as Python's
    /// `@contextmanager campaign_root_override`.
    pub fn override_campaign_root(&self, root: PathBuf) -> CampaignRootOverride<'_> {
        let previous = self.game_dir();
        let previous_bin_names = *self.active_bin_names.borrow();
        self.set_campaign_root(root);
        self.set_campaign_bin_names(CE_BIN_NAMES);
        CampaignRootOverride { paths: self, previous, previous_bin_names }
    }

    /// The active campaign's engine binary, if any of `set_campaign_bin_names()`'s names
    /// exist; otherwise the first such name, so callers have something to report as missing.
    pub fn game_bin(&self) -> PathBuf {
        let root = self.game_dir();
        let names = *self.active_bin_names.borrow();
        for name in names {
            let candidate = root.join(name);
            if candidate.is_file() {
                return candidate;
            }
        }
        root.join(names.first().copied().unwrap_or("fallout2-ce"))
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

/// RAII guard returned by `Paths::override_campaign_root` - put the previous root and bin
/// names back on drop.
pub struct CampaignRootOverride<'a> {
    paths: &'a Paths,
    previous: PathBuf,
    previous_bin_names: &'static [&'static str],
}

impl Drop for CampaignRootOverride<'_> {
    fn drop(&mut self) {
        self.paths.set_campaign_root(self.previous.clone());
        self.paths.set_campaign_bin_names(self.previous_bin_names);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths_in(dir: &Path) -> SharedPaths {
        Paths::new(dir.to_path_buf())
    }

    /// On a case-sensitive filesystem (Linux), ci_path must return the on-disk spelling.
    /// On a case-insensitive one (macOS, Windows) the as-typed path already opens the file,
    /// so ci_path's fast path keeps it as-is - only require that it points at the file.
    fn assert_resolves_to(resolved: &Path, on_disk: &Path) {
        let upper = on_disk.with_file_name(on_disk.file_name().unwrap().to_string_lossy().to_uppercase());
        let case_insensitive_fs = upper != on_disk && upper.exists();
        if case_insensitive_fs {
            assert!(resolved.exists(), "{resolved:?} should open {on_disk:?}");
            assert_eq!(resolved.to_string_lossy().to_lowercase(), on_disk.to_string_lossy().to_lowercase());
        } else {
            assert_eq!(resolved, on_disk);
        }
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
        assert_resolves_to(&resolved, &tmp.path().join("Mods").join("NPCArmor.dat"));
    }

    #[test]
    fn ci_path_resolves_case_mismatch_at_every_level() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Mods").join("NpcArmor")).unwrap();
        std::fs::write(tmp.path().join("Mods").join("NpcArmor").join("Data.DAT"), b"").unwrap();
        let paths = paths_in(tmp.path());

        let resolved = paths.ci_path("mods/npcarmor/data.dat", None);
        assert_resolves_to(&resolved, &tmp.path().join("Mods").join("NpcArmor").join("Data.DAT"));
    }

    #[test]
    fn ci_path_handles_unicode_filenames_case_insensitively() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("CAFÉ.cfg"), b"").unwrap();
        let paths = paths_in(tmp.path());

        assert_resolves_to(&paths.ci_path("café.cfg", None), &tmp.path().join("CAFÉ.cfg"));
    }

    #[test]
    fn ci_path_nonexistent_path_returns_best_effort_join() {
        // Neither Python nor Rust special-case a totally missing path: ci_path just
        // returns the naive join when nothing on disk matches at any level.
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());

        let resolved = paths.ci_path("mods/missing/thing.dat", None);
        assert_eq!(resolved, tmp.path().join("mods").join("missing").join("thing.dat"));
    }

    #[test]
    fn ci_path_accepts_backslash_separators() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path().join("Mods")).unwrap();
        std::fs::write(tmp.path().join("Mods").join("Thing.dat"), b"").unwrap();
        let paths = paths_in(tmp.path());

        assert_resolves_to(&paths.ci_path("mods\\thing.dat", None), &tmp.path().join("Mods").join("Thing.dat"));
    }

    #[test]
    fn ci_path_honors_explicit_root_override() {
        let tmp = tempfile::tempdir().unwrap();
        let other_root = tmp.path().join("Other");
        std::fs::create_dir_all(&other_root).unwrap();
        std::fs::write(other_root.join("File.txt"), b"").unwrap();
        let paths = paths_in(tmp.path());

        // game_dir() is still tmp.path() here, so passing `other_root` as an explicit
        // root must take priority over the active campaign root.
        let resolved = paths.ci_path("file.txt", Some(&other_root));
        assert_resolves_to(&resolved, &other_root.join("File.txt"));
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

    #[test]
    fn game_bin_searches_the_active_campaigns_own_bin_names() {
        // A campaign with its own distinct engine (Sonora's fallout-fission) is never found
        // under CE's own binary names - game_bin() must search whatever
        // set_campaign_bin_names() was last told, not a hardcoded fallout2-ce/.exe pair.
        let tmp = tempfile::tempdir().unwrap();
        std::fs::write(tmp.path().join("fallout-fission"), b"").unwrap();
        let paths = paths_in(tmp.path());
        assert_eq!(paths.game_bin(), tmp.path().join("fallout2-ce"), "still CE's names by default");

        paths.set_campaign_bin_names(&["fallout-fission", "fallout-fission.exe"]);
        assert_eq!(paths.game_bin(), tmp.path().join("fallout-fission"));
    }

    #[test]
    fn override_campaign_root_restores_bin_names_on_drop() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = paths_in(tmp.path());
        paths.set_campaign_bin_names(&["fallout-fission", "fallout-fission.exe"]);

        {
            let _guard = paths.override_campaign_root(tmp.path().join("elsewhere"));
            // Overridden root is always treated as CE while the guard is held.
            assert_eq!(paths.game_bin().file_name().unwrap(), "fallout2-ce");
        }

        std::fs::write(tmp.path().join("fallout-fission"), b"").unwrap();
        assert_eq!(paths.game_bin(), tmp.path().join("fallout-fission"));
    }
}
