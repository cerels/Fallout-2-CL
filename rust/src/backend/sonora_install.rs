//! Installing Fallout: Sonora: a full standalone game (its own master.dat/critter.dat),
//! into its own `Sonora/` campaign root - see `GLOSSARY.md` / `docs/adr/0001`.
//!
//! The official English release (cambragol/Fallout-Sonora-English) ships the complete game
//! plus a default Windows engine binary and Sonora's own fission.dat, and bundles every
//! platform's engine build again as nested zips inside itself. Extracting those nested zips
//! was more trouble than it's worth (a zip inside a zip means parsing a second central
//! directory out of a byte range of the first), and there's a cleaner source anyway: the
//! engine's own project, cambragol/fission-ce, publishes the same per-platform builds as
//! plain top-level release assets. `install_sonora` uses that instead for the binary - but
//! never for fission.dat: fission-ce's own fission.dat is a different size than the one
//! bundled in the Sonora release, so it's Sonora-specific and must come from the Sonora zip,
//! not the generic engine release.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::time::Duration;

use super::campaigns::{by_id, Campaign};
use super::paths::Paths;

const SONORA_REPO: &str = "cambragol/Fallout-Sonora-English";
const FISSION_REPO: &str = "cambragol/fission-ce";
const CAMPAIGN_ID: &str = "sonora";

/// Inside the main release zip: things that aren't part of the playable game, or that we're
/// replacing with a native binary fetched from `FISSION_REPO` instead.
const SKIP_TOP_DIRS: &[&str] = &["__MACOSX", "BIS_mapper"];
/// `fallout-sonora-x64.exe` is the bundled default Windows binary - always superseded by
/// `fetch_fission_binary`, on every platform, so there's no point keeping it around.
const SKIP_NAMES: &[&str] = &["fsonoradlc.exe", "log_develop.txt", "fallout-sonora-x64.exe"];

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

/// Pick the first `*.zip` asset (skipping the macOS `.dmg` also published there) and the
/// release tag out of a "latest release" API response.
fn pick_sonora_asset(body: &serde_json::Value) -> Result<(String, String, String), String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.to_lowercase().ends_with(".zip") {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, name.to_string(), version));
        }
    }
    Err("No Sonora release zip found.".to_string())
}

/// Return (url, filename, version) for Sonora's latest English release zip.
fn resolve_sonora_download() -> Result<(String, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{SONORA_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_sonora_asset(&body)
}

struct FissionAsset {
    archive_name: &'static str,
    bin_name: &'static str,
}

/// (archive filename, binary filename inside it) from `FISSION_REPO`'s latest release, for
/// this OS/CPU - same idea as `ce_install::asset_for_platform`.
fn fission_asset_for_platform() -> Result<FissionAsset, String> {
    use std::env::consts::{ARCH, OS};
    let asset = match (OS, ARCH) {
        ("linux", "x86_64") => FissionAsset {
            archive_name: "fallout-fission-linux-x64.zip",
            bin_name: "fallout-fission-linux-x64",
        },
        ("linux", "aarch64") => FissionAsset {
            archive_name: "fallout-fission-linux-arm64.zip",
            bin_name: "fallout-fission-linux-arm64",
        },
        ("linux", "arm") => FissionAsset {
            archive_name: "fallout-fission-linux-armhf.zip",
            bin_name: "fallout-fission-linux-armhf",
        },
        ("windows", "x86_64") => FissionAsset {
            archive_name: "fallout-fission-windows-x64.zip",
            bin_name: "fallout-fission-x64.exe",
        },
        ("windows", "x86") => FissionAsset {
            archive_name: "fallout-fission-windows-x86.zip",
            bin_name: "fallout-fission-x86.exe",
        },
        (os, arch) => {
            return Err(format!(
                "No native Sonora/FISSION engine build for this platform/CPU ({os}/{arch}) - install \
                 fails, since the release's own bundled engine binary is Windows-only."
            ))
        }
    };
    Ok(asset)
}

fn should_skip(parts: &[String]) -> bool {
    if parts.is_empty() || SKIP_TOP_DIRS.contains(&parts[0].as_str()) {
        return true;
    }
    let lower = parts.last().unwrap().to_lowercase();
    if SKIP_NAMES.contains(&lower.as_str()) || lower.ends_with(".ds_store") {
        return true;
    }
    // The nested per-platform engine zips (fallout-sonora-linux-x64.zip, ...android.zip,
    // ...ipa.zip) - superseded by fetching a native build from FISSION_REPO instead.
    if parts.len() == 1 && lower.starts_with("fallout-sonora") && lower.ends_with(".zip") {
        return true;
    }
    false
}

fn find_fission_asset_url(body: &serde_json::Value, asset_name: &str) -> Result<String, String> {
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    assets
        .iter()
        .find(|a| a.get("name").and_then(|v| v.as_str()) == Some(asset_name))
        .and_then(|a| a.get("browser_download_url").and_then(|v| v.as_str()).map(str::to_string))
        .ok_or_else(|| format!("No {asset_name} asset found in the latest FISSION release."))
}

/// Fetch a native engine binary for this platform from `FISSION_REPO`'s latest release and
/// place it in `root`, under the name `campaigns.rs` expects to find.
fn fetch_fission_binary(root: &Path) -> Result<(), String> {
    let asset = fission_asset_for_platform()?;
    let api_url = format!("https://api.github.com/repos/{FISSION_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    let asset_url = find_fission_asset_url(&body, asset.archive_name)?;
    let bytes = super::http::get_bytes(&asset_url, Duration::from_secs(120))?;
    write_fission_binary(&bytes, asset.bin_name, root)
}

/// The zip-reading and chmod part of `fetch_fission_binary`, split out so it's testable
/// against a synthetic archive with no network access needed.
fn write_fission_binary(archive_bytes: &[u8], bin_name: &str, root: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;
    let mut binary = Vec::new();
    zip.by_name(bin_name).map_err(|e| e.to_string())?.read_to_end(&mut binary).map_err(|e| e.to_string())?;

    let out_name = if cfg!(windows) { "fallout-fission.exe" } else { "fallout-fission" };
    let out_path = root.join(out_name);
    std::fs::write(&out_path, &binary).map_err(|e| e.to_string())?;
    super::ce_install::make_executable(&out_path).map_err(|e| e.to_string())
}

/// The zip-download part of `install_sonora`: validate the release's single top-level
/// folder, then copy everything under it (minus `should_skip` matches) into `root`. Split
/// out so it's testable against a synthetic archive with no network access needed.
fn extract_sonora_release(archive_bytes: &[u8], paths: &Paths, root: &Path) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;

    let mut top_level = HashSet::new();
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

/// Download Sonora's release, extract the game (skipping dev/platform-bundle extras), then
/// fetch a native engine binary from `FISSION_REPO`.
pub fn install_sonora(paths: &Paths) -> Result<(), String> {
    let root = sonora_root(paths);
    let (url, _filename, version) = resolve_sonora_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(600))?;

    extract_sonora_release(&bytes, paths, &root)?;
    fetch_fission_binary(&root)?;

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
    fn pick_sonora_asset_skips_the_macos_dmg() {
        let body = serde_json::json!({
            "tag_name": "v1.5",
            "assets": [
                {"name": "Fallout-Sonora.dmg", "browser_download_url": "https://x/sonora.dmg"},
                {"name": "Fallout-Sonora-English.zip", "browser_download_url": "https://x/sonora.zip"},
            ]
        });
        let (url, name, version) = pick_sonora_asset(&body).unwrap();
        assert_eq!(url, "https://x/sonora.zip");
        assert_eq!(name, "Fallout-Sonora-English.zip");
        assert_eq!(version, "v1.5");
    }

    #[test]
    fn should_skip_top_level_dev_folders() {
        assert!(should_skip(&["__MACOSX".to_string(), "x".to_string()]));
        assert!(should_skip(&["BIS_mapper".to_string()]));
    }

    #[test]
    fn should_skip_known_junk_filenames_case_insensitively() {
        assert!(should_skip(&["data".to_string(), "FSonoraDLC.exe".to_string()]));
        assert!(should_skip(&["x".to_string(), "thumbs.DS_Store".to_string()]));
    }

    #[test]
    fn should_skip_nested_platform_engine_zips() {
        assert!(should_skip(&["fallout-sonora-android.zip".to_string()]));
        assert!(should_skip(&["FALLOUT-SONORA-LINUX-X64.ZIP".to_string()]));
    }

    #[test]
    fn should_skip_keeps_ordinary_game_files() {
        assert!(!should_skip(&["data".to_string(), "master.dat".to_string()]));
        assert!(!should_skip(&["fission.dat".to_string()]));
    }

    #[test]
    fn should_skip_nested_platform_zip_lookalike_is_not_skipped() {
        // The "nested platform engine zip" rule only fires at the top level (`parts.len()
        // == 1`); a same-named file sitting in a subfolder is an ordinary game file as far
        // as this check is concerned.
        assert!(!should_skip(&["mods".to_string(), "fallout-sonora-extra.zip".to_string()]));
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
    fn extract_sonora_release_copies_the_game_and_skips_extras() {
        let zip = build_zip(&[
            ("Fallout-Sonora/data/master.dat", b"master"),
            ("Fallout-Sonora/fission.dat", b"fission"),
            ("Fallout-Sonora/fallout-sonora-x64.exe", b"windows binary, superseded"),
            ("__MACOSX/._data", b"mac junk"),
            ("Fallout-Sonora/fallout-sonora-android.zip", b"nested bundle, superseded"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();

        extract_sonora_release(&zip, &paths, &root).unwrap();

        assert_eq!(std::fs::read(root.join("data").join("master.dat")).unwrap(), b"master");
        assert_eq!(std::fs::read(root.join("fission.dat")).unwrap(), b"fission");
        assert!(!root.join("fallout-sonora-x64.exe").exists());
        assert!(!root.join("fallout-sonora-android.zip").exists());
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
        // One valid top-level folder, but every file inside it matches a skip rule - there's
        // nothing left to copy, which should be its own clear error rather than silently
        // "succeeding" with an empty install.
        let zip = build_zip(&[
            ("Fallout-Sonora/fallout-sonora-x64.exe", b"windows binary, superseded"),
            ("Fallout-Sonora/log_develop.txt", b"dev log"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let root = sonora_root(&paths);
        std::fs::create_dir_all(&root).unwrap();
        let err = extract_sonora_release(&zip, &paths, &root).unwrap_err();
        assert!(err.contains("no recognized files found"));
    }

    #[test]
    fn find_fission_asset_url_matches_by_exact_name() {
        let body = serde_json::json!({
            "assets": [{"name": "fallout-fission-linux-x64.zip", "browser_download_url": "https://x/f.zip"}]
        });
        assert_eq!(find_fission_asset_url(&body, "fallout-fission-linux-x64.zip").unwrap(), "https://x/f.zip");
        assert!(find_fission_asset_url(&body, "fallout-fission-linux-arm64.zip").is_err());
    }

    #[cfg(target_os = "linux")]
    #[test]
    fn write_fission_binary_extracts_and_marks_executable() {
        let zip = build_zip(&[("fallout-fission-linux-x64", b"elf bytes")]);
        let tmp = tempfile::tempdir().unwrap();
        write_fission_binary(&zip, "fallout-fission-linux-x64", tmp.path()).unwrap();

        let out = tmp.path().join("fallout-fission");
        assert_eq!(std::fs::read(&out).unwrap(), b"elf bytes");
        use std::os::unix::fs::PermissionsExt;
        assert_ne!(std::fs::metadata(&out).unwrap().permissions().mode() & 0o111, 0);
    }

    #[test]
    fn write_fission_binary_errors_cleanly_when_the_named_binary_is_absent() {
        let zip = build_zip(&[("some-other-file", b"x")]);
        let tmp = tempfile::tempdir().unwrap();
        assert!(write_fission_binary(&zip, "fallout-fission-linux-x64", tmp.path()).is_err());
    }

    #[test]
    fn fission_asset_for_platform_errors_clearly_on_an_unsupported_cpu() {
        // FissionAsset isn't Debug/PartialEq (no production changes just for a test), so
        // match explicitly instead of unwrap_err().
        if let Err(e) = fission_asset_for_platform() {
            assert!(e.contains("No native Sonora/FISSION engine build"));
        }
    }

    #[test]
    fn state_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(sonora_root(&paths)).unwrap();

        let mut state = BTreeMap::new();
        state.insert("version".to_string(), "v1.5".to_string());
        save_sonora_state(&paths, &state).unwrap();
        assert_eq!(load_sonora_state(&paths), state);
    }
}
