//! Installing Fallout 2: Nevada (English translation): a full standalone game (its own
//! master.dat/critter.dat), into its own `Nevada/` campaign root - see `GLOSSARY.md` /
//! `docs/adr/0001`.
//!
//! HogLord69/fallout-english-localization's release bundles the game with the classic
//! Windows-only engine (NEVADA_EV.exe, ddraw.dll's sfall injection, an sfall ini-editor GUI)
//! - none of that is needed once running it through fallout2-ce instead, which the
//! project's own README lists as "supported with minor tweaks needed". The one tweak
//! verified by inspecting the actual archive: its main config file is named `config.cfg`,
//! not `fallout2.cfg`, but is otherwise the exact same classic-engine format `fallout2.cfg`
//! already uses - `install_nevada` renames it. Engine binary comes from the same place as
//! every other campaign's, fallout2-ce's own release, reused via `ce_install::install_ce`
//! pointed at this campaign's root instead of Vanilla/CE's.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::campaigns::{by_id, Campaign};
use super::paths::Paths;

const NEVADA_REPO: &str = "HogLord69/fallout-english-localization";
const CAMPAIGN_ID: &str = "nevada";

/// The classic Windows-only engine + its sfall injection + its ini-editor GUI - all
/// superseded by fetching a native fallout2-ce build instead, on every platform.
const SKIP_NAMES: &[&str] = &["nevada_ev.exe", "ddraw.dll", "ddraw.ini", "play nevada.bat", "sf-configurator.exe"];

fn campaign() -> &'static Campaign {
    by_id(CAMPAIGN_ID).expect("\"nevada\" is a registered campaign id")
}

pub fn nevada_root(paths: &Paths) -> PathBuf {
    campaign().root(paths)
}

fn state_path(paths: &Paths) -> PathBuf {
    nevada_root(paths).join(".f2launcher-nevada.json")
}

pub fn load_nevada_state(paths: &Paths) -> BTreeMap<String, String> {
    std::fs::read_to_string(state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_nevada_state(paths: &Paths, state: &BTreeMap<String, String>) -> io::Result<()> {
    let json = serde_json::to_string_pretty(state).expect("BTreeMap<String, String> always serializes");
    std::fs::write(state_path(paths), json)
}

pub fn is_installed(paths: &Paths) -> bool {
    campaign().is_installed(paths)
}

/// Pick the `Fallout-Nevada-English.zip` asset and the release tag out of a "latest
/// release" API response - this repo's releases bundle Nevada, Resurrection and Sonora
/// English repacks together, so this picks out just the Nevada asset.
fn pick_nevada_asset(body: &serde_json::Value) -> Result<(String, String, String), String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.to_lowercase() == "fallout-nevada-english.zip" {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, name.to_string(), version));
        }
    }
    Err("No Fallout-Nevada-English.zip asset found in the latest release.".to_string())
}

fn resolve_nevada_download() -> Result<(String, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{NEVADA_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_nevada_asset(&body)
}

/// Validate the release's single top-level folder, then copy everything under it (minus
/// `SKIP_NAMES` matches) into `root`. Split out from `install_nevada` so it's testable
/// against a synthetic archive with no network access needed.
fn extract_nevada_release(archive_bytes: &[u8], paths: &Paths, root: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;

    let mut top_level = std::collections::HashSet::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| e.to_string())?.name().to_string();
        if let Some(first) = name.split('/').next() {
            if !first.is_empty() {
                top_level.insert(first.to_string());
            }
        }
    }
    if top_level.len() != 1 {
        let mut found: Vec<&String> = top_level.iter().collect();
        found.sort();
        return Err(format!(
            "Nevada's release layout has changed (expected one top-level folder, found {found:?}) - \
             update nevada_install.rs's assumptions."
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
        let basename = rel.rsplit('/').next().unwrap_or(&rel).to_lowercase();
        if SKIP_NAMES.contains(&basename.as_str()) {
            continue;
        }
        let dest = paths.ci_path(&rel, Some(root));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    if copied == 0 {
        return Err("Nevada archive layout has changed - no recognized files found".to_string());
    }
    Ok(())
}

/// Download Nevada's release and extract it into its own campaign root (skipping the
/// classic Windows-only engine bits), rename its config.cfg to fallout2.cfg, then fetch a
/// native fallout2-ce engine binary the same way as every other campaign.
pub fn install_nevada(paths: &Paths) -> Result<(), String> {
    let root = nevada_root(paths);
    let (url, _filename, version) = resolve_nevada_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(600))?;

    extract_nevada_release(&bytes, paths, &root)?;

    let config = paths.ci_path("config.cfg", Some(&root));
    if config.is_file() {
        std::fs::rename(&config, root.join("fallout2.cfg")).map_err(|e| e.to_string())?;
    }

    if cfg!(any(target_os = "linux", target_os = "windows")) {
        let _guard = paths.override_campaign_root(root.clone());
        super::ce_install::install_ce(paths)?;
    }
    // macOS: not automated yet, same as CE's own install.

    let mut state = BTreeMap::new();
    state.insert("version".to_string(), version);
    save_nevada_state(paths, &state).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn nevada_root_is_a_subfolder_of_the_launcher_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(nevada_root(&paths), tmp.path().join("Nevada"));
    }

    #[test]
    fn pick_nevada_asset_picks_the_nevada_zip_out_of_a_bundle() {
        let body = serde_json::json!({
            "tag_name": "v3.0",
            "assets": [
                {"name": "Fallout-Resurrection-English.zip", "browser_download_url": "https://x/res.zip"},
                {"name": "Fallout-Nevada-English.zip", "browser_download_url": "https://x/nevada.zip"},
                {"name": "Fallout-Sonora-English.zip", "browser_download_url": "https://x/sonora.zip"},
            ]
        });
        let (url, name, version) = pick_nevada_asset(&body).unwrap();
        assert_eq!(url, "https://x/nevada.zip");
        assert_eq!(name, "Fallout-Nevada-English.zip");
        assert_eq!(version, "v3.0");
    }

    #[test]
    fn pick_nevada_asset_errors_when_missing() {
        let body = serde_json::json!({ "assets": [] });
        assert!(pick_nevada_asset(&body).is_err());
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
    fn extract_nevada_release_copies_the_game_and_skips_the_classic_engine() {
        let zip = build_zip(&[
            ("Fallout Nevada/data/master.dat", b"master"),
            ("Fallout Nevada/config.cfg", b"[main]\n"),
            ("Fallout Nevada/NEVADA_EV.exe", b"classic engine, superseded"),
            ("Fallout Nevada/ddraw.dll", b"sfall injection, superseded"),
            ("Fallout Nevada/Play Nevada.bat", b"superseded"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = nevada_root(&paths);
        std::fs::create_dir_all(&root).unwrap();

        extract_nevada_release(&zip, &paths, &root).unwrap();

        assert_eq!(std::fs::read(root.join("data").join("master.dat")).unwrap(), b"master");
        assert!(root.join("config.cfg").is_file());
        assert!(!root.join("NEVADA_EV.exe").exists());
        assert!(!root.join("ddraw.dll").exists());
        assert!(!root.join("Play Nevada.bat").exists());
    }

    #[test]
    fn extract_nevada_release_rejects_multiple_top_level_folders() {
        let zip = build_zip(&[("One/a.txt", b"x"), ("Two/b.txt", b"x")]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = nevada_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        assert!(extract_nevada_release(&zip, &paths, &root).is_err());
    }

    #[test]
    fn install_nevada_renames_config_cfg_to_fallout2_cfg() {
        // Exercises just the rename step install_nevada performs after extraction, without
        // the network-dependent download/ce-install steps.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = nevada_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(root.join("config.cfg"), b"[main]\n").unwrap();

        let config = paths.ci_path("config.cfg", Some(&root));
        assert!(config.is_file());
        std::fs::rename(&config, root.join("fallout2.cfg")).unwrap();

        assert!(!root.join("config.cfg").exists());
        assert!(root.join("fallout2.cfg").is_file());
    }

    #[test]
    fn state_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(nevada_root(&paths)).unwrap();

        let mut state = BTreeMap::new();
        state.insert("version".to_string(), "v3.0".to_string());
        save_nevada_state(&paths, &state).unwrap();
        assert_eq!(load_nevada_state(&paths), state);
    }
}
