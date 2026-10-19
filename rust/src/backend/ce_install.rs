//! Installing Fallout 2 CE itself: detecting a vanilla game directory with no CE engine yet,
//! downloading the fallout2-ce/fallout2-ce fork's current build, and placing the engine
//! binary + ce.dat. Linux and Windows for now; macOS ships as a .dmg disk image, which needs
//! mounting rather than a simple archive extract, so it stays a manual install for now - see
//! `README.md`.
//!
//! The fork ships a single continuously-overwritten prerelease (tag "continious", sic)
//! rather than versioned releases, so there's nothing to pin a version against - we just
//! record when the build we fetched was published, for display elsewhere in the launcher.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::Duration;

use super::paths::Paths;

const CE_REPO: &str = "fallout2-ce/fallout2-ce";
const CE_RELEASE_TAG: &str = "continious";

#[derive(Debug, Clone, Copy, PartialEq)]
enum ArchiveKind {
    Tar,
    Zip,
}

struct CeAsset {
    file_name: &'static str,
    kind: ArchiveKind,
    bin_name: &'static str,
}

/// True for an OS this module has a build for at all (Linux, Windows) - regardless of
/// whether this specific CPU architecture turns out to have one too. Mirrors Python's
/// two-layer check: `needs_ce_install` only looks at the OS, and a missing-architecture
/// build only surfaces later, as an error from `install_ce` itself.
fn os_has_any_build() -> bool {
    matches!(std::env::consts::OS, "linux" | "windows")
}

/// (asset filename, archive kind, engine binary name inside it) for this OS/CPU, picked at
/// compile time from `std::env::consts` rather than Python's runtime `sys.platform`/
/// `platform.machine()` - CI already builds one binary per OS/arch natively (see
/// `.github/workflows/build.yml`), so there's no cross-platform detection left to do, only a
/// lookup.
fn asset_for_platform() -> Result<CeAsset, String> {
    use std::env::consts::{ARCH, OS};
    let asset = match (OS, ARCH) {
        ("linux", "x86_64") => {
            CeAsset { file_name: "fallout2-ce-linux-x64.tar.gz", kind: ArchiveKind::Tar, bin_name: "fallout2-ce" }
        }
        ("linux", "aarch64") => {
            CeAsset { file_name: "fallout2-ce-linux-arm64.tar.gz", kind: ArchiveKind::Tar, bin_name: "fallout2-ce" }
        }
        ("linux", "arm") => {
            CeAsset { file_name: "fallout2-ce-linux-armhf.tar.gz", kind: ArchiveKind::Tar, bin_name: "fallout2-ce" }
        }
        ("windows", "x86_64") => {
            CeAsset { file_name: "fallout2-ce-windows-x64.zip", kind: ArchiveKind::Zip, bin_name: "fallout2-ce.exe" }
        }
        ("windows", "x86") => {
            CeAsset { file_name: "fallout2-ce-windows-x86.zip", kind: ArchiveKind::Zip, bin_name: "fallout2-ce.exe" }
        }
        (os, arch) => {
            return Err(format!("No Fallout 2 CE build available for this platform/CPU ({os}/{arch})."))
        }
    };
    Ok(asset)
}

fn ce_state_path(paths: &Paths) -> PathBuf {
    paths.game_dir().join(".f2launcher-ce.json")
}

/// What the launcher last installed: the release's publish time and asset name, for
/// display. Not needed for detection, which just checks `game_bin()` directly.
pub fn load_ce_state(paths: &Paths) -> BTreeMap<String, String> {
    std::fs::read_to_string(ce_state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_ce_state(paths: &Paths, state: &BTreeMap<String, String>) -> io::Result<()> {
    let json = serde_json::to_string_pretty(state).expect("BTreeMap<String, String> always serializes");
    std::fs::write(ce_state_path(paths), json)
}

/// True if this is a real Fallout 2 game directory (has the original data files),
/// regardless of whether CE is installed yet.
pub fn looks_like_vanilla_install(paths: &Paths) -> bool {
    paths.ci_path("master.dat", None).is_file() && paths.ci_path("critter.dat", None).is_file()
}

/// True when this is a real game directory that's just missing the CE engine - the case
/// this module can offer to fix. Other platforms never need it here (no installer for them
/// yet), and an unrelated/empty folder is left to the normal "not found" error.
pub fn needs_ce_install(paths: &Paths) -> bool {
    os_has_any_build() && looks_like_vanilla_install(paths) && !paths.game_bin().is_file()
}

/// Pick the named asset and the release's `published_at` out of a GitHub "release by tag"
/// API response. Split out from `resolve_ce_download` so it's testable against a canned
/// JSON response, with no network access needed.
fn pick_ce_asset(body: &serde_json::Value, asset_name: &str) -> Result<(String, String, String), String> {
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        if a.get("name").and_then(|v| v.as_str()) == Some(asset_name) {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            let published_at = body.get("published_at").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, asset_name.to_string(), published_at));
        }
    }
    Err(format!("No {asset_name} asset found in the latest fallout2-ce release."))
}

/// Return (url, asset_filename, published_at) for the current release build.
fn resolve_ce_download() -> Result<(String, String, String), String> {
    let asset = asset_for_platform()?;
    let api_url = format!("https://api.github.com/repos/{CE_REPO}/releases/tags/{CE_RELEASE_TAG}");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_ce_asset(&body, asset.file_name)
}

/// Pull `wanted` top-level files (e.g. `{"ce.dat", "fallout2-ce.exe"}`) out of a release
/// archive that wraps everything in one directory, regardless of tar vs. zip. Split out from
/// `install_ce` so it's testable against synthetic in-memory archives.
fn extract_members(
    archive_bytes: &[u8],
    kind: ArchiveKind,
    wanted: &HashSet<&str>,
) -> Result<std::collections::HashMap<String, Vec<u8>>, String> {
    let mut found = std::collections::HashMap::new();
    match kind {
        ArchiveKind::Tar => {
            let gz = flate2::read::GzDecoder::new(archive_bytes);
            let mut archive = tar::Archive::new(gz);
            for entry in archive.entries().map_err(|e| e.to_string())? {
                let mut entry = entry.map_err(|e| e.to_string())?;
                if !entry.header().entry_type().is_file() {
                    continue;
                }
                let path = entry.path().map_err(|e| e.to_string())?.into_owned();
                let parts: Vec<String> =
                    path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect();
                if parts.len() == 2 && wanted.contains(parts[1].as_str()) {
                    let mut buf = Vec::new();
                    entry.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                    found.insert(parts[1].clone(), buf);
                }
            }
        }
        ArchiveKind::Zip => {
            let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;
            for i in 0..zip.len() {
                let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
                if file.is_dir() {
                    continue;
                }
                let parts: Vec<String> = file
                    .name()
                    .replace('\\', "/")
                    .split('/')
                    .filter(|p| !p.is_empty())
                    .map(str::to_string)
                    .collect();
                if parts.len() == 2 && wanted.contains(parts[1].as_str()) {
                    let mut buf = Vec::new();
                    file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
                    found.insert(parts[1].clone(), buf);
                }
            }
        }
    }
    Ok(found)
}

/// Download and place the fallout2-ce binary + ce.dat in the active campaign's root. Never
/// touches any existing vanilla files - only adds the CE engine files.
pub fn install_ce(paths: &Paths) -> Result<(), String> {
    let asset = asset_for_platform()?;
    let (url, filename, published_at) = resolve_ce_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(120))?;

    let wanted: HashSet<&str> = [asset.bin_name, "ce.dat"].into_iter().collect();
    let members = extract_members(&bytes, asset.kind, &wanted)?;
    let mut missing: Vec<&&str> = wanted.iter().filter(|w| !members.contains_key(**w)).collect();
    if !missing.is_empty() {
        missing.sort();
        let names: Vec<&str> = missing.into_iter().copied().collect();
        return Err(format!("Release archive is missing: {}", names.join(", ")));
    }
    for (name, data) in &members {
        std::fs::write(paths.game_dir().join(name), data).map_err(|e| e.to_string())?;
    }

    let bin_path = paths.game_dir().join(asset.bin_name);
    make_executable(&bin_path).map_err(|e| e.to_string())?;

    let mut state = BTreeMap::new();
    state.insert("published_at".to_string(), published_at);
    state.insert("asset".to_string(), filename);
    save_ce_state(paths, &state).map_err(|e| e.to_string())
}

/// Shared with the other installers that drop in a downloaded binary (sonora_install,
/// nevada_install, rpu_install).
#[cfg(unix)]
pub(crate) fn make_executable(path: &std::path::Path) -> io::Result<()> {
    use std::os::unix::fs::PermissionsExt;
    let mut perms = std::fs::metadata(path)?.permissions();
    perms.set_mode(perms.mode() | 0o111);
    std::fs::set_permissions(path, perms)
}

#[cfg(not(unix))]
pub(crate) fn make_executable(_path: &std::path::Path) -> io::Result<()> {
    Ok(()) // Windows has no executable bit to set.
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn needs_ce_install_is_false_for_an_unrelated_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert!(!needs_ce_install(&paths));
    }

    #[test]
    fn needs_ce_install_is_true_for_vanilla_without_ce_binary() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("master.dat"), b"x").unwrap();
        std::fs::write(tmp.path().join("critter.dat"), b"x").unwrap();
        assert_eq!(needs_ce_install(&paths), os_has_any_build());
    }

    #[test]
    fn needs_ce_install_is_false_once_the_binary_exists() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("master.dat"), b"x").unwrap();
        std::fs::write(tmp.path().join("critter.dat"), b"x").unwrap();
        std::fs::write(tmp.path().join("fallout2-ce"), b"x").unwrap();
        assert!(!needs_ce_install(&paths));
    }

    #[test]
    fn pick_ce_asset_finds_the_named_asset() {
        let body = serde_json::json!({
            "published_at": "2024-01-01T00:00:00Z",
            "assets": [
                {"name": "fallout2-ce-windows-x64.zip", "browser_download_url": "https://x/win.zip"},
                {"name": "fallout2-ce-linux-x64.tar.gz", "browser_download_url": "https://x/linux.tar.gz"},
            ]
        });
        let (url, name, published) = pick_ce_asset(&body, "fallout2-ce-linux-x64.tar.gz").unwrap();
        assert_eq!(url, "https://x/linux.tar.gz");
        assert_eq!(name, "fallout2-ce-linux-x64.tar.gz");
        assert_eq!(published, "2024-01-01T00:00:00Z");
    }

    #[test]
    fn pick_ce_asset_errors_when_the_asset_is_missing() {
        let body = serde_json::json!({ "assets": [] });
        assert!(pick_ce_asset(&body, "fallout2-ce-linux-x64.tar.gz").is_err());
    }

    fn build_tar_gz(entries: &[(&str, &[u8])]) -> Vec<u8> {
        let mut tar_bytes = Vec::new();
        {
            let mut builder = tar::Builder::new(&mut tar_bytes);
            for &(name, content) in entries {
                let mut header = tar::Header::new_gnu();
                header.set_path(name).unwrap();
                header.set_size(content.len() as u64);
                header.set_mode(0o644);
                header.set_cksum();
                builder.append(&header, content).unwrap();
            }
            builder.finish().unwrap();
        }
        let mut gz_bytes = Vec::new();
        {
            let mut encoder = flate2::write::GzEncoder::new(&mut gz_bytes, flate2::Compression::default());
            encoder.write_all(&tar_bytes).unwrap();
            encoder.finish().unwrap();
        }
        gz_bytes
    }

    #[test]
    fn extract_members_pulls_wanted_files_out_of_a_tar_gz() {
        let archive = build_tar_gz(&[
            ("fallout2-ce-linux-x64/fallout2-ce", b"binary bytes"),
            ("fallout2-ce-linux-x64/ce.dat", b"dat bytes"),
            ("fallout2-ce-linux-x64/README.md", b"ignored"),
        ]);
        let wanted: HashSet<&str> = ["fallout2-ce", "ce.dat"].into_iter().collect();
        let members = extract_members(&archive, ArchiveKind::Tar, &wanted).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members["fallout2-ce"], b"binary bytes");
        assert_eq!(members["ce.dat"], b"dat bytes");
    }

    #[test]
    fn extract_members_pulls_wanted_files_out_of_a_zip() {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            writer.start_file("fallout2-ce-windows-x64/fallout2-ce.exe", opts).unwrap();
            writer.write_all(b"exe bytes").unwrap();
            writer.start_file("fallout2-ce-windows-x64/ce.dat", opts).unwrap();
            writer.write_all(b"dat bytes").unwrap();
            writer.finish().unwrap();
        }
        let wanted: HashSet<&str> = ["fallout2-ce.exe", "ce.dat"].into_iter().collect();
        let members = extract_members(&buf, ArchiveKind::Zip, &wanted).unwrap();
        assert_eq!(members.len(), 2);
        assert_eq!(members["fallout2-ce.exe"], b"exe bytes");
    }

    #[test]
    fn ce_state_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert!(load_ce_state(&paths).is_empty());

        let mut state = BTreeMap::new();
        state.insert("published_at".to_string(), "2024-01-01T00:00:00Z".to_string());
        state.insert("asset".to_string(), "fallout2-ce-linux-x64.tar.gz".to_string());
        save_ce_state(&paths, &state).unwrap();
        assert_eq!(load_ce_state(&paths), state);
    }
}
