//! Installing Fallout: Sonora: a full standalone game (its own master.dat/critter.dat),
//! into its own `Sonora/` campaign root - see `GLOSSARY.md` / `docs/adr/0001` and
//! `docs/adr/0011-sonora-onto-fallout2-ce.md`.
//!
//! Sonora's own official release (cambragol/Fallout-Sonora-English) bundles a standalone
//! engine of its own (FISSION, a hard fork of the long-unmaintained original
//! alexbatalov/fallout2-ce, not the actively-maintained fallout2-ce/fallout2-ce this
//! launcher otherwise standardizes on for every other campaign). `install_sonora` instead
//! uses NovaRain/Fallout-Sonora-English's `eng_repack` branch - a repack of the same
//! Sonora release, kept in sync with cambragol's own version numbering, built for classic
//! `Fallout2.exe`+sfall instead - and swaps its classic Windows-only engine for a native
//! `fallout2-ce` build the same way as every other campaign, via `ce_install::install_ce`.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::campaigns::{by_id, Campaign};
use super::paths::Paths;

const SONORA_REPO: &str = "NovaRain/Fallout-Sonora-English";
const CAMPAIGN_ID: &str = "sonora";

/// Inside the release zip: dev-only tooling, never part of the playable game.
const SKIP_TOP_DIRS: &[&str] = &["__MACOSX", "BIS_mapper"];
/// The classic Windows-only engine, its sfall injection, and the classic-engine-only
/// high-res patch bundled alongside it - all superseded by fetching a native fallout2-ce
/// build instead, on every platform.
const SKIP_NAMES: &[&str] = &[
    "fallout2.exe",
    "ddraw.dll",
    "ddraw.ini",
    "f2_res.dat",
    "f2_res.dll",
    "f2_res.ini",
    "f2_res_config.exe",
    "f2_res_change.log",
    "f2_res_readme.rtf",
    "log_develop.txt",
];

fn campaign() -> &'static Campaign {
    by_id(CAMPAIGN_ID).expect("\"sonora\" is a registered campaign id")
}

pub fn sonora_root(paths: &Paths) -> PathBuf {
    campaign().root(paths)
}

fn state_path(paths: &Paths) -> PathBuf {
    sonora_root(paths).join(".f2launcher-sonora.json")
}

pub fn load_sonora_state(paths: &Paths) -> BTreeMap<String, String> {
    std::fs::read_to_string(state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_sonora_state(paths: &Paths, state: &BTreeMap<String, String>) -> io::Result<()> {
    let json = serde_json::to_string_pretty(state).expect("BTreeMap<String, String> always serializes");
    std::fs::write(state_path(paths), json)
}

pub fn is_installed(paths: &Paths) -> bool {
    campaign().is_installed(paths)
}

/// Pick the main English repack zip out of a "latest release" API response - skipping the
/// optional `fs_lang_*.zip` language packs published alongside it (Russian, Spanish: not
/// part of this install, see `docs/adr/0011-sonora-onto-fallout2-ce.md`'s scope note).
fn pick_sonora_asset(body: &serde_json::Value) -> Result<(String, String, String), String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let lower = name.to_lowercase();
        if lower.starts_with("fallout_sonora") && lower.ends_with(".zip") {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, name.to_string(), version));
        }
    }
    Err("No main Fallout_Sonora*.zip asset found in the latest release.".to_string())
}

/// Return (url, filename, version) for Sonora's latest English repack release zip.
fn resolve_sonora_download() -> Result<(String, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{SONORA_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_sonora_asset(&body)
}

fn should_skip(parts: &[String]) -> bool {
    if parts.is_empty() || SKIP_TOP_DIRS.contains(&parts[0].as_str()) {
        return true;
    }
    let lower = parts.last().unwrap().to_lowercase();
    SKIP_NAMES.contains(&lower.as_str()) || lower.ends_with(".ds_store")
}

/// Validate the release's single top-level folder, then copy everything under it (minus
/// `should_skip` matches) into `root`. Split out from `install_sonora` so it's testable
/// against a synthetic archive with no network access needed.
fn extract_sonora_release(archive_bytes: &[u8], paths: &Paths, root: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;

    let mut top_level = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| e.to_string())?.name().to_string();
        if let Some(first) = name.split('/').next() {
            if !first.is_empty() && !SKIP_TOP_DIRS.contains(&first) {
                top_level.insert(first.to_string());
            }
        }
    }
    if top_level.len() != 1 {
        let mut found: Vec<&String> = top_level.iter().collect();
        found.sort();
        return Err(format!(
            "Sonora's release layout has changed (expected one top-level folder besides \
             {SKIP_TOP_DIRS:?}, found {found:?}) - update sonora_install.rs's assumptions."
        ));
    }
    let prefix = format!("{}/", top_level.iter().next().unwrap());

    let mut copied = 0;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() || !file.name().starts_with(&prefix) {
            continue;
        }
        let rel = file.name()[prefix.len()..].to_string();
        let parts: Vec<String> = rel.split('/').map(str::to_string).collect();
        if should_skip(&parts) {
            continue;
        }
        let dest = paths.ci_path(&parts.join("/"), Some(root));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    if copied == 0 {
        return Err("Sonora archive layout has changed - no recognized files found".to_string());
    }
    Ok(())
}

/// Download Sonora's release and extract it into its own campaign root (skipping the
/// classic Windows-only engine bits), then fetch a native fallout2-ce engine binary the
/// same way as every other campaign.
pub fn install_sonora(paths: &Paths, on_stage: &dyn Fn(&str)) -> Result<(), String> {
    let root = sonora_root(paths);
    let (url, _filename, version) = resolve_sonora_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(600))?;
    on_stage("Installing\u{2026}");

    extract_sonora_release(&bytes, paths, &root)?;

    if cfg!(any(target_os = "linux", target_os = "windows")) {
        let _guard = paths.override_campaign_root(root.clone());
        super::ce_install::install_ce(paths, on_stage)?;
    }
    // macOS: not automated yet, same as CE's own install.

    let mut state = BTreeMap::new();
    state.insert("version".to_string(), version);
    save_sonora_state(paths, &state).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn sonora_root_is_a_subfolder_of_the_launcher_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(sonora_root(&paths), tmp.path().join("Sonora"));
    }

    #[test]
    fn pick_sonora_asset_skips_the_language_packs() {
        let body = serde_json::json!({
            "tag_name": "1.16.4-sfall",
            "assets": [
                {"name": "fs_lang_russian.zip", "browser_download_url": "https://x/ru.zip"},
                {"name": "fs_lang_spanish.zip", "browser_download_url": "https://x/es.zip"},
                {"name": "Fallout_Sonora_1.16.4E_sfall.zip", "browser_download_url": "https://x/sonora.zip"},
            ]
        });
        let (url, name, version) = pick_sonora_asset(&body).unwrap();
        assert_eq!(url, "https://x/sonora.zip");
        assert_eq!(name, "Fallout_Sonora_1.16.4E_sfall.zip");
        assert_eq!(version, "1.16.4-sfall");
    }

    #[test]
    fn pick_sonora_asset_errors_when_missing() {
        let body = serde_json::json!({
            "tag_name": "1.16.4-sfall",
            "assets": [{"name": "fs_lang_russian.zip", "browser_download_url": "https://x/ru.zip"}]
        });
        assert!(pick_sonora_asset(&body).is_err());
    }

    #[test]
    fn should_skip_top_level_dev_folders() {
        assert!(should_skip(&["__MACOSX".to_string(), "x".to_string()]));
        assert!(should_skip(&["BIS_mapper".to_string()]));
    }

    #[test]
    fn should_skip_the_classic_engine_and_classic_hires_patch() {
        assert!(should_skip(&["Fallout2.exe".to_string()]));
        assert!(should_skip(&["ddraw.dll".to_string()]));
        assert!(should_skip(&["DDRAW.INI".to_string()]));
        assert!(should_skip(&["f2_res.dat".to_string()]));
        assert!(should_skip(&["f2_res_Config.exe".to_string()]));
        assert!(should_skip(&["x".to_string(), "thumbs.DS_Store".to_string()]));
    }

    #[test]
    fn should_skip_keeps_ordinary_game_files() {
        assert!(!should_skip(&["data".to_string(), "master.dat".to_string()]));
        assert!(!should_skip(&["fallout2.cfg".to_string()]));
        assert!(!should_skip(&["sfall.dat".to_string()]));
    }

    fn build_zip(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        for &(name, content) in entries {
            writer.start_file(name, opts).unwrap();
            writer.write_all(content).unwrap();
        }
        writer.finish().unwrap();
        buf
    }

    #[test]
    fn extract_sonora_release_copies_the_game_and_skips_the_classic_engine() {
        let zip = build_zip(&[
            ("Fallout_Sonora_1_16_E/data/master.dat", b"master"),
            ("Fallout_Sonora_1_16_E/fallout2.cfg", b"[system]\n"),
            ("Fallout_Sonora_1_16_E/sfall.dat", b"sfall"),
            ("Fallout_Sonora_1_16_E/Fallout2.exe", b"classic engine, superseded"),
            ("Fallout_Sonora_1_16_E/ddraw.dll", b"sfall injection, superseded"),
            ("Fallout_Sonora_1_16_E/f2_res.dat", b"classic hires patch, superseded"),
            ("__MACOSX/._data", b"mac junk"),
            ("Fallout_Sonora_1_16_E/BIS_mapper/readme.txt", b"dev tool"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();

        extract_sonora_release(&zip, &paths, &root).unwrap();

        assert_eq!(std::fs::read(root.join("data").join("master.dat")).unwrap(), b"master");
        assert!(root.join("fallout2.cfg").is_file());
        assert!(root.join("sfall.dat").is_file());
        assert!(!root.join("Fallout2.exe").exists());
        assert!(!root.join("ddraw.dll").exists());
        assert!(!root.join("f2_res.dat").exists());
        assert!(!root.join("BIS_mapper").exists());
    }

    #[test]
    fn extract_sonora_release_rejects_an_unexpected_layout() {
        let zip = build_zip(&[("One/a.txt", b"x"), ("Two/b.txt", b"x")]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        assert!(extract_sonora_release(&zip, &paths, &root).is_err());
    }

    #[test]
    fn extract_sonora_release_rejects_an_archive_with_no_folders_besides_the_skipped_ones() {
        let zip = build_zip(&[("__MACOSX/._x", b"x"), ("BIS_mapper/readme.txt", b"x")]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        let err = extract_sonora_release(&zip, &paths, &root).unwrap_err();
        assert!(err.contains("layout has changed"));
    }

    #[test]
    fn extract_sonora_release_errors_when_every_file_is_skipped() {
        let zip = build_zip(&[
            ("Fallout_Sonora_1_16_E/Fallout2.exe", b"classic engine, superseded"),
            ("Fallout_Sonora_1_16_E/log_develop.txt", b"dev log"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        let err = extract_sonora_release(&zip, &paths, &root).unwrap_err();
        assert!(err.contains("no recognized files found"));
    }

    #[test]
    fn state_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(sonora_root(&paths)).unwrap();

        let mut state = BTreeMap::new();
        state.insert("version".to_string(), "1.16.4-sfall".to_string());
        save_sonora_state(&paths, &state).unwrap();
        assert_eq!(load_sonora_state(&paths), state);
    }
}
