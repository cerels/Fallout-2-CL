//! Installing Fallout et Tu: a full, self-contained engine+content package that drops into
//! its own subfolder (`Fallout1in2/`) underneath the Vanilla/CE campaign's folder - see
//! `GLOSSARY.md` and `docs/adr/0001-campaign-root-is-a-subfolder.md`. Everything in
//! rotators/Fo1in2's release belongs together under one top-level `Fallout1in2/` folder, so
//! the whole thing is extracted as-is - no allowlist needed (unlike RPU's zip).
//!
//! Two things make Et Tu different from a plain CE install, though:
//!
//! - The release only ships Windows binaries (`fallout2-ce.exe`, `Fallout2.exe`) - there's
//!   no native Linux/macOS engine in it. On Linux, `install_ettu` fetches one the same way
//!   `ce_install` does for Vanilla/CE, just pointed at Et Tu's own root instead.
//!
//! - Et Tu needs Fallout 1's own data, which this can never download on its own (that would
//!   mean distributing someone else's purchased game). The release bundles the official
//!   conversion tools (`undat.sh`/`undat.bat`, plus the GUI `UNDAT.EXE`) for pulling the
//!   handful of files it actually needs out of a Fallout 1 install's MASTER.DAT;
//!   `extract_fallout1_data` runs the command-line one, but only once the user points it at
//!   a Fallout 1 copy they already own.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::Duration;

use super::campaigns::{by_id, Campaign};
use super::paths::Paths;

const ETTU_REPO: &str = "rotators/Fo1in2";
const CAMPAIGN_ID: &str = "ettu";

fn campaign() -> &'static Campaign {
    by_id(CAMPAIGN_ID).expect("\"ettu\" is a registered campaign id")
}

pub fn ettu_root(paths: &Paths) -> PathBuf {
    campaign().root(paths)
}

fn state_path(paths: &Paths) -> PathBuf {
    ettu_root(paths).join(".f2launcher-ettu.json")
}

/// Loose, JSON-shaped state (unlike `ce_install`'s, this one mixes a string `version` and a
/// bool `fallout1_data_extracted`) - mirrors Python's plain `dict` directly rather than
/// introducing a typed struct for two ad hoc fields.
pub fn load_ettu_state(paths: &Paths) -> BTreeMap<String, serde_json::Value> {
    std::fs::read_to_string(state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_ettu_state(paths: &Paths, state: &BTreeMap<String, serde_json::Value>) -> io::Result<()> {
    let json = serde_json::to_string_pretty(state).expect("a BTreeMap<String, Value> always serializes");
    std::fs::write(state_path(paths), json)
}

pub fn is_installed(paths: &Paths) -> bool {
    campaign().is_installed(paths)
}

/// True once `extract_fallout1_data` has run successfully. There's no fully reliable way to
/// detect this by file presence alone (undat's file list could change release to release),
/// so this is our own bookkeeping, same as CE's/RPU's install state files.
pub fn has_fallout1_data(paths: &Paths) -> bool {
    load_ettu_state(paths).get("fallout1_data_extracted").and_then(|v| v.as_bool()).unwrap_or(false)
}

/// Pick the `Fallout1in2.zip` asset and the release tag out of a "latest release" API
/// response. Split out from `resolve_ettu_download` so it's testable against a canned JSON
/// response, with no network access needed.
fn pick_ettu_asset(body: &serde_json::Value) -> Result<(String, String, String), String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if name.to_lowercase() == "fallout1in2.zip" {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, name.to_string(), version));
        }
    }
    Err("No Fallout1in2.zip asset found in the latest Et Tu release.".to_string())
}

/// Return (url, filename, version) for Et Tu's latest release zip.
fn resolve_ettu_download() -> Result<(String, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{ETTU_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_ettu_asset(&body)
}

/// The zip's top-level folder names (every entry's first path component). Split out from
/// `install_ettu` so the "has the release layout changed?" sanity check is testable against
/// a synthetic archive.
fn top_level_folders(bytes: &[u8]) -> Result<HashSet<String>, String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut top_level = HashSet::new();
    for i in 0..zip.len() {
        let name = zip.by_index(i).map_err(|e| e.to_string())?.name().to_string();
        if let Some(first) = name.split('/').next() {
            if !first.is_empty() {
                top_level.insert(first.to_string());
            }
        }
    }
    Ok(top_level)
}

/// Download Et Tu's release and extract it whole into its own subfolder, then (Linux only
/// for now, same scope as CE itself) fetch a native engine binary into that same folder,
/// since the release only ships Windows ones.
pub fn install_ettu(paths: &Paths, on_stage: &dyn Fn(&str)) -> Result<(), String> {
    let campaign = campaign();
    let (url, _filename, version) = resolve_ettu_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(600))?;
    on_stage("Installing\u{2026}");

    let top_level = top_level_folders(&bytes)?;
    let expected: HashSet<String> = [campaign.subdir.to_string()].into_iter().collect();
    if top_level != expected {
        let mut found: Vec<&String> = top_level.iter().collect();
        found.sort();
        return Err(format!(
            "Et Tu's release layout has changed (expected a single {:?} folder, found {found:?}) - \
             update ettu_install.rs's assumptions.",
            campaign.subdir
        ));
    }

    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(&bytes)).map_err(|e| e.to_string())?;
    zip.extract(paths.launcher_dir()).map_err(|e| e.to_string())?;

    if cfg!(target_os = "linux") {
        let _guard = paths.override_campaign_root(ettu_root(paths));
        super::ce_install::install_ce(paths, on_stage)?;
    }
    // Windows: the release's own fallout2-ce.exe/Fallout2.exe already run natively.
    // macOS: not automated yet, same as CE's own install (no native build fetched here).

    let mut state = load_ettu_state(paths);
    state.insert("version".to_string(), serde_json::Value::String(version));
    save_ettu_state(paths, &state).map_err(|e| e.to_string())
}

#[derive(Debug)]
struct CommandOutput {
    success: bool,
    stdout: String,
    stderr: String,
}

/// Runs `program` and waits up to `timeout`, killing it if it runs longer. Reads stdout/
/// stderr only after the process exits - fine for a script that prints a few status lines,
/// which is all `undat.sh`/`undat.bat` ever do; a chattier child could in principle fill its pipe
/// buffer and stall before then, but that's not a shape this script takes.
fn run_with_timeout(
    program: &Path,
    args: &[String],
    cwd: &Path,
    timeout: Duration,
) -> Result<CommandOutput, String> {
    let mut command = Command::new(program);
    command.args(args).current_dir(cwd).stdout(Stdio::piped()).stderr(Stdio::piped());
    // The launcher is a GUI-subsystem app, so running undat.bat would otherwise flash up a
    // console window of its own.
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        command.creation_flags(CREATE_NO_WINDOW);
    }
    let mut child = command.spawn().map_err(|e| e.to_string())?;

    let deadline = std::time::Instant::now() + timeout;
    loop {
        if let Some(status) = child.try_wait().map_err(|e| e.to_string())? {
            let mut stdout = String::new();
            let mut stderr = String::new();
            if let Some(mut out) = child.stdout.take() {
                let _ = out.read_to_string(&mut stdout);
            }
            if let Some(mut err) = child.stderr.take() {
                let _ = err.read_to_string(&mut stderr);
            }
            return Ok(CommandOutput { success: status.success(), stdout, stderr });
        }
        if std::time::Instant::now() >= deadline {
            let _ = child.kill();
            let _ = child.wait();
            return Err(format!("{} timed out after {}s", program.display(), timeout.as_secs()));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// Et Tu's command-line extractor for this OS: `undat.sh` (runs `utils/dat3`) everywhere
/// but Windows, where it's `undat.bat` (runs `utils/dat2.exe`) instead.
const UNDAT_SCRIPT: &str = if cfg!(windows) { "undat.bat" } else { "undat.sh" };

/// Run Et Tu's bundled `undat.sh`/`undat.bat` against a Fallout 1 install's MASTER.DAT.
pub fn extract_fallout1_data(paths: &Paths, master_dat: &Path) -> Result<(), String> {
    if !is_installed(paths) {
        return Err("Et Tu isn't installed yet - install it first.".to_string());
    }
    let root = ettu_root(paths);
    let script = paths.ci_path(UNDAT_SCRIPT, Some(&root));
    if !script.is_file() {
        // Older Et Tu releases only shipped the GUI UNDAT.EXE on Windows - no command-line
        // form to automate - so point at that instead of a dead end.
        if cfg!(windows) {
            return Err("undat.bat not found in the Et Tu install - run UNDAT.EXE from the \
                         Fallout1in2 folder yourself and point it at Fallout 1's MASTER.DAT."
                .to_string());
        }
        return Err("undat.sh not found in the Et Tu install - it may be a newer/older release \
                     with a different layout than this launcher expects."
            .to_string());
    }
    // The release zip doesn't reliably preserve the Unix exec bit, same as the engine
    // binaries the other installers drop in - see `ce_install::make_executable`.
    super::ce_install::make_executable(&script).map_err(|e| e.to_string())?;

    let output = run_with_timeout(
        &script,
        &[master_dat.to_string_lossy().to_string()],
        &root,
        Duration::from_secs(300),
    )?;
    if !output.success {
        let message = [&output.stdout, &output.stderr]
            .into_iter()
            .map(|s| s.trim())
            .find(|s| !s.is_empty())
            .unwrap_or("Extracting Fallout 1's data failed");
        return Err(message.to_string());
    }

    let mut state = load_ettu_state(paths);
    state.insert("fallout1_data_extracted".to_string(), serde_json::Value::Bool(true));
    save_ettu_state(paths, &state).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn ettu_root_is_a_subfolder_of_the_launcher_dir() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(ettu_root(&paths), tmp.path().join("Fallout1in2"));
    }

    #[test]
    fn state_round_trips_mixed_value_types() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        // Real callers only ever save state after install_ettu() has created this folder
        // (or after confirming is_installed(), which implies it exists).
        std::fs::create_dir_all(ettu_root(&paths)).unwrap();
        assert!(!has_fallout1_data(&paths));

        let mut state = load_ettu_state(&paths);
        state.insert("version".to_string(), serde_json::Value::String("v1.0".to_string()));
        state.insert("fallout1_data_extracted".to_string(), serde_json::Value::Bool(true));
        save_ettu_state(&paths, &state).unwrap();

        assert!(has_fallout1_data(&paths));
        let reloaded = load_ettu_state(&paths);
        assert_eq!(reloaded.get("version").and_then(|v| v.as_str()), Some("v1.0"));
    }

    #[test]
    fn pick_ettu_asset_is_case_insensitive_on_the_asset_name() {
        let body = serde_json::json!({
            "tag_name": "v2.0",
            "assets": [{"name": "FALLOUT1IN2.ZIP", "browser_download_url": "https://x/f.zip"}]
        });
        let (url, name, version) = pick_ettu_asset(&body).unwrap();
        assert_eq!(url, "https://x/f.zip");
        assert_eq!(name, "FALLOUT1IN2.ZIP");
        assert_eq!(version, "v2.0");
    }

    #[test]
    fn pick_ettu_asset_errors_when_the_zip_is_missing() {
        let body = serde_json::json!({ "tag_name": "v2.0", "assets": [] });
        assert!(pick_ettu_asset(&body).is_err());
    }

    fn build_zip(entries: &[&str]) -> Vec<u8> {
        let mut buf = Vec::new();
        let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
        let opts = zip::write::SimpleFileOptions::default();
        for &name in entries {
            if name.ends_with('/') {
                writer.add_directory(name, opts).unwrap();
            } else {
                writer.start_file(name, opts).unwrap();
                writer.write_all(b"x").unwrap();
            }
        }
        writer.finish().unwrap();
        buf
    }

    #[test]
    fn top_level_folders_sees_the_single_expected_folder() {
        let zip = build_zip(&["Fallout1in2/", "Fallout1in2/fallout2-ce.exe", "Fallout1in2/data/master.dat"]);
        assert_eq!(top_level_folders(&zip).unwrap(), HashSet::from(["Fallout1in2".to_string()]));
    }

    #[test]
    fn top_level_folders_flags_an_unexpected_layout() {
        let zip = build_zip(&["Fallout1in2/x", "Extra/y"]);
        let found = top_level_folders(&zip).unwrap();
        assert_eq!(found, HashSet::from(["Fallout1in2".to_string(), "Extra".to_string()]));
    }

    #[test]
    fn extract_fallout1_data_fails_cleanly_when_not_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let err = extract_fallout1_data(&paths, Path::new("/fake/MASTER.DAT")).unwrap_err();
        assert!(err.contains("isn't installed"));
    }

    #[cfg(unix)]
    fn mark_ettu_installed(paths: &Paths) {
        std::fs::create_dir_all(ettu_root(paths)).unwrap();
        std::fs::write(ettu_root(paths).join("fallout2-ce"), b"x").unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn extract_fallout1_data_fails_cleanly_when_undat_script_is_missing() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        mark_ettu_installed(&paths);

        let err = extract_fallout1_data(&paths, Path::new("/fake/MASTER.DAT")).unwrap_err();
        assert!(err.contains("undat.sh not found"));
    }

    #[cfg(unix)]
    fn write_undat_script(paths: &Paths, body: &str) {
        let script = ettu_root(paths).join("undat.sh");
        std::fs::write(&script, format!("#!/bin/sh\n{body}\n")).unwrap();
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&script, std::fs::Permissions::from_mode(0o755)).unwrap();
    }

    #[test]
    #[cfg(unix)]
    fn extract_fallout1_data_succeeds_runs_the_script_and_preserves_other_state() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        mark_ettu_installed(&paths);
        write_undat_script(&paths, "exit 0");

        // A pre-existing "version" key must survive the merge-on-save in
        // extract_fallout1_data, exactly like Python's `{**load_ettu_state(), ...}`.
        let mut state = load_ettu_state(&paths);
        state.insert("version".to_string(), serde_json::Value::String("v9.9".to_string()));
        save_ettu_state(&paths, &state).unwrap();

        assert!(!has_fallout1_data(&paths));
        extract_fallout1_data(&paths, Path::new("/fake/MASTER.DAT")).unwrap();

        assert!(has_fallout1_data(&paths));
        let reloaded = load_ettu_state(&paths);
        assert_eq!(reloaded.get("version").and_then(|v| v.as_str()), Some("v9.9"));
    }

    #[test]
    #[cfg(unix)]
    fn extract_fallout1_data_surfaces_the_scripts_own_error_output() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        mark_ettu_installed(&paths);
        write_undat_script(&paths, "echo 'MASTER.DAT not found' >&2; exit 1");

        let err = extract_fallout1_data(&paths, Path::new("/fake/MASTER.DAT")).unwrap_err();
        assert_eq!(err, "MASTER.DAT not found");
        assert!(!has_fallout1_data(&paths));
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_captures_output_and_exit_status() {
        let script = PathBuf::from("/bin/sh");
        let out = run_with_timeout(
            &script,
            &["-c".to_string(), "echo hello; exit 3".to_string()],
            Path::new("/tmp"),
            Duration::from_secs(5),
        )
        .unwrap();
        assert!(!out.success);
        assert_eq!(out.stdout.trim(), "hello");
    }

    #[test]
    #[cfg(unix)]
    fn run_with_timeout_kills_a_hanging_process() {
        let script = PathBuf::from("/bin/sh");
        let err = run_with_timeout(
            &script,
            &["-c".to_string(), "sleep 5".to_string()],
            Path::new("/tmp"),
            Duration::from_millis(100),
        )
        .unwrap_err();
        assert!(err.contains("timed out"));
    }
}
