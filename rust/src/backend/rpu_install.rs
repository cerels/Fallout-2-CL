//! Installing RPU (Restoration Project Updated): a content overlay for the Vanilla/CE
//! campaign, not a separate campaign - see `GLOSSARY.md` and
//! `schemas/mods/restoration-project.toml`, which already describes its settings once
//! installed.
//!
//! The official release zip is a full classic-sfall install package meant to be unzipped by
//! hand onto a vanilla Fallout 2 copy: alongside RPU's own content it bundles fallout2.exe,
//! ddraw.dll/.ini, sfall.dat, f2_res's own files, its own mods_order.txt, and - deliberately,
//! this is the officially-tested combination - NPC Armor, Party Orders and the Unofficial
//! Patch's settings. None of the classic-engine files belong on a CE install, and blindly
//! extracting mods_order.txt would overwrite whatever load order the user already has, but
//! everything under mods/ itself is exactly what's needed for those mods' own bundled
//! schemas to find their files and show up - so it's all extracted, merged into the existing
//! mods/ setup via the same `enable_mod_file` helper the regular mod catalog uses.
//!
//! RPU also needs several ddraw.ini (sfall) settings its shipped defaults don't have - CE
//! enforces some of them as hard limits sized to vanilla content and throws a friendly
//! in-game error naming the exact key when they're wrong. Rather than guessing at the
//! complete set, `install_rpu` reads RPU's *own* bundled ddraw.ini (from the same zip) and
//! merges every key it sets into the existing ddraw.ini via `IniFile`, so only the lines
//! that actually change are touched.

use std::collections::{BTreeMap, HashSet};
use std::io::{self, Read};
use std::path::PathBuf;
use std::time::Duration;

use super::dat::parse_ini_text;
use super::ini_file::IniFile;
use super::mod_catalog::{delete_mod_files, enable_mod_file};
use super::paths::Paths;

const RPU_REPO: &str = "BGforgeNet/Fallout2_Restoration_Project";

/// RPU's own optional content dat files, beyond the core rpu.dat - each is an individually
/// togglable extra on the regular Mods page once installed (see `restoration-project.toml`).
pub const OPTIONAL_DAT_FILES: &[&str] = &[
    "rpu_enhanced_worldmap.dat",
    "rpu_extended_flamer.dat",
    "rpu_rifle_animations.dat",
    "rpu_wakizashi_animations.dat",
    "cassidy_head.dat",
    "cassidy_voice_joey_bracken_hq.dat",
    "rpu_improved_mysterious_stranger.dat",
    "rpu_alternative_explosions.dat",
];

fn rpu_state_path(paths: &Paths) -> PathBuf {
    paths.ci_path("mods/.f2launcher-rpu.json", None)
}

/// The version last installed, for update checks - not needed for detection, which just
/// checks `mod_installed("rpu.dat")` directly.
pub fn load_rpu_state(paths: &Paths) -> BTreeMap<String, String> {
    std::fs::read_to_string(rpu_state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_rpu_state(paths: &Paths, state: &BTreeMap<String, String>) -> io::Result<()> {
    let path = rpu_state_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state).expect("BTreeMap<String, String> always serializes");
    std::fs::write(path, json)
}

pub fn is_installed(paths: &Paths) -> bool {
    paths.mod_installed("rpu.dat")
}

/// The "officially-tested combination" companion files (NPC Armor, Party Orders, the
/// Unofficial Patch) that `install_rpu` recorded as having actually extracted this install -
/// see `docs/adr/0007-rpu-companion-attribution-manifest.md`. Empty before RPU is installed,
/// or for an install made before this manifest existed (self-healing: populated again the
/// next time RPU is installed or updated).
pub fn load_rpu_companions(paths: &Paths) -> Vec<String> {
    load_rpu_state(paths)
        .get("companions")
        .map(|s| s.split(',').filter(|f| !f.is_empty()).map(str::to_string).collect())
        .unwrap_or_default()
}

/// Every filename RPU owns and that must never show up as its own row on the Mods page: its
/// core files, its optional extras, and its recorded companions (see `GLOSSARY.md`'s RPU
/// entry). Lowercased for case-insensitive matching against files found on disk.
pub fn owned_file_names(paths: &Paths) -> HashSet<String> {
    let mut set: HashSet<String> = OPTIONAL_DAT_FILES.iter().map(|s| s.to_lowercase()).collect();
    set.insert("rpu.dat".to_string());
    set.insert("rpu.ini".to_string());
    set.extend(load_rpu_companions(paths).into_iter().map(|s| s.to_lowercase()));
    set
}

/// Pick the main release zip (not the separate `*_translations.zip` - this launcher is
/// English-only for now) and the release tag out of a "latest release" API response.
fn pick_rpu_asset(body: &serde_json::Value) -> Result<(String, String, String), String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).unwrap_or("").to_string();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for a in &assets {
        let name = a.get("name").and_then(|v| v.as_str()).unwrap_or("");
        let lower = name.to_lowercase();
        if lower.ends_with(".zip") && !lower.contains("translations") {
            let url = a.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok((url, name.to_string(), version));
        }
    }
    Err("No RPU release zip found.".to_string())
}

fn resolve_rpu_download() -> Result<(String, String, String), String> {
    let api_url = format!("https://api.github.com/repos/{RPU_REPO}/releases/latest");
    let body = super::http::get_json(&api_url, Duration::from_secs(20))?;
    pick_rpu_asset(&body)
}

/// Latest version if newer than what's installed, else `None`. Mirrors
/// `mod_catalog::check_for_update`: the first check just records a baseline.
pub fn check_for_rpu_update(paths: &Paths) -> Option<String> {
    let version = resolve_rpu_download().ok()?.2;
    if version.is_empty() {
        return None;
    }
    let mut state = load_rpu_state(paths);
    match state.get("version") {
        None => {
            state.insert("version".to_string(), version);
            let _ = save_rpu_state(paths, &state);
            None
        }
        Some(baseline) if *baseline != version => Some(version),
        Some(_) => None,
    }
}

/// Where to extract a zip entry's parts, or `None` to skip it - see the module doc comment
/// for why most of the archive is skipped. Everything directly under `mods/` is wanted
/// except `mods_order.txt` itself, which must never overwrite the user's own load order.
/// `parts` is assumed non-empty (callers only reach this after checking that).
fn wanted_dest(paths: &Paths, parts: &[String]) -> Option<PathBuf> {
    if parts[0] == "appearance" || (parts.len() >= 2 && parts[0] == "data" && parts[1] == "sound") {
        return Some(paths.ci_path(&parts.join("/"), None));
    }
    if parts[0] == "mods" && parts.len() == 2 && parts[1].to_lowercase() != "mods_order.txt" {
        return Some(paths.ci_path(&format!("mods/{}", parts[1]), None));
    }
    None
}

/// Merge every (section, key) RPU's own bundled ddraw.ini sets into the existing one - see
/// the module doc comment. Only changes values that actually differ; creates a missing
/// section the same way `IniFile::set` already does for any other missing key.
fn apply_rpu_ddraw_settings(paths: &Paths, rpu_ddraw_text: &str) -> Result<(), String> {
    let path = paths.ci_path("ddraw.ini", None);
    if !path.is_file() {
        return Ok(());
    }
    let wanted = parse_ini_text(rpu_ddraw_text);
    let mut ini = IniFile::open(path).map_err(|e| e.to_string())?;
    for ((section, key), value) in &wanted {
        if section.is_empty() || key.is_empty() {
            continue;
        }
        let differs = match ini.get(section, key) {
            Some(current) => current.to_lowercase() != value.to_lowercase(),
            None => true,
        };
        if differs {
            ini.set(section, key, value);
        }
    }
    ini.save().map_err(|e| e.to_string())
}

/// Extracts the wanted parts of the archive (see `wanted_dest`) and returns the bundled
/// top-level ddraw.ini's text (if present, for `apply_rpu_ddraw_settings` to merge) along with
/// every filename written directly under `mods/` - `install_rpu` uses the latter to work out
/// which of them are companion files it doesn't already know about (see
/// `docs/adr/0007-rpu-companion-attribution-manifest.md`). Split out from `install_rpu` so
/// it's testable against a synthetic archive with no network access needed.
fn extract_rpu_release(archive_bytes: &[u8], paths: &Paths) -> Result<(Option<String>, Vec<String>), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(archive_bytes)).map_err(|e| e.to_string())?;
    let mut copied = 0;
    let mut rpu_ddraw_text = None;
    let mut mods_files = Vec::new();

    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let parts: Vec<String> =
            file.name().replace('\\', "/").split('/').filter(|p| !p.is_empty()).map(str::to_string).collect();
        if parts.is_empty() {
            continue;
        }
        if parts.len() == 1 && parts[0] == "ddraw.ini" {
            let mut buf = Vec::new();
            file.read_to_end(&mut buf).map_err(|e| e.to_string())?;
            rpu_ddraw_text = Some(decode_latin1(&buf));
            continue;
        }
        let Some(dest) = wanted_dest(paths, &parts) else { continue };
        if parts.len() == 2 && parts[0] == "mods" {
            mods_files.push(parts[1].clone());
        }
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    if copied == 0 {
        return Err("RPU archive layout has changed - no recognized files found".to_string());
    }
    Ok((rpu_ddraw_text, mods_files))
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Download RPU's current release and extract its content (including the mods it officially
/// bundles), then merge its recommended ddraw.ini settings into the existing one. Never
/// touches mods_order.txt beyond enabling rpu.dat itself.
pub fn install_rpu(paths: &Paths, on_stage: &dyn Fn(&str)) -> Result<(), String> {
    let (url, _filename, version) = resolve_rpu_download()?;
    let bytes = super::http::get_bytes(&url, Duration::from_secs(600))?;
    on_stage("Installing\u{2026}");

    let (rpu_ddraw_text, mods_files) = extract_rpu_release(&bytes, paths)?;
    enable_mod_file(paths, "rpu.dat").map_err(|e| e.to_string())?;
    if let Some(text) = rpu_ddraw_text {
        apply_rpu_ddraw_settings(paths, &text)?;
    }

    // Everything the zip wrote under mods/ that isn't RPU's own core or optional-extra files
    // is, by elimination, a companion mod (NPC Armor, Party Orders, Unofficial Patch) - see
    // `docs/adr/0007-rpu-companion-attribution-manifest.md` for why this is recorded rather
    // than hardcoded.
    let known: HashSet<String> =
        OPTIONAL_DAT_FILES.iter().map(|s| s.to_lowercase()).chain(["rpu.dat".to_string(), "rpu.ini".to_string()]).collect();
    let companions: Vec<String> = mods_files.into_iter().filter(|f| !known.contains(&f.to_lowercase())).collect();

    let mut state = BTreeMap::new();
    state.insert("version".to_string(), version);
    state.insert("companions".to_string(), companions.join(","));
    save_rpu_state(paths, &state).map_err(|e| e.to_string())
}

/// Remove RPU's own mods/ files (not appearance/ or the restored music - just extra data,
/// harmless to leave, and re-installing overwrites them anyway), its recorded companion mods
/// (NPC Armor, Party Orders, the Unofficial Patch - you only have them because of RPU, see
/// `docs/adr/0007-rpu-companion-attribution-manifest.md`), and drop all of their mods_order.txt
/// lines.
pub fn delete_rpu(paths: &Paths) -> Vec<PathBuf> {
    let mut extra: Vec<String> = OPTIONAL_DAT_FILES.iter().map(|s| s.to_string()).collect();
    extra.push("rpu.ini".to_string());
    extra.extend(load_rpu_companions(paths));
    let removed = delete_mod_files(paths, "rpu.dat", &extra);

    let state_path = rpu_state_path(paths);
    if state_path.is_file() {
        let _ = std::fs::remove_file(&state_path);
    }
    removed
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn parts(s: &str) -> Vec<String> {
        s.split('/').map(str::to_string).collect()
    }

    #[test]
    fn wanted_dest_keeps_appearance_and_data_sound() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(wanted_dest(&paths, &parts("appearance/hero.frm")), Some(tmp.path().join("appearance/hero.frm")));
        assert_eq!(
            wanted_dest(&paths, &parts("data/sound/music/map.acm")),
            Some(tmp.path().join("data/sound/music/map.acm"))
        );
    }

    #[test]
    fn wanted_dest_skips_other_data_subfolders() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(wanted_dest(&paths, &parts("data/art/hero.frm")), None);
        assert_eq!(wanted_dest(&paths, &parts("data")), None);
    }

    #[test]
    fn wanted_dest_keeps_mods_files_except_mods_order_txt() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(wanted_dest(&paths, &parts("mods/rpu.dat")), Some(tmp.path().join("mods/rpu.dat")));
        assert_eq!(wanted_dest(&paths, &parts("mods/MODS_ORDER.TXT")), None);
        // Nested under mods/ (len > 2) isn't a flat drop-in file - skipped.
        assert_eq!(wanted_dest(&paths, &parts("mods/sub/x.dat")), None);
    }

    #[test]
    fn wanted_dest_skips_classic_engine_files() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(wanted_dest(&paths, &parts("fallout2.exe")), None);
        assert_eq!(wanted_dest(&paths, &parts("ddraw.dll")), None);
    }

    #[test]
    fn pick_rpu_asset_skips_the_translations_zip() {
        let body = serde_json::json!({
            "tag_name": "v3.1",
            "assets": [
                {"name": "RPU_translations.zip", "browser_download_url": "https://x/t.zip"},
                {"name": "RPU.zip", "browser_download_url": "https://x/rpu.zip"},
            ]
        });
        let (url, name, version) = pick_rpu_asset(&body).unwrap();
        assert_eq!(url, "https://x/rpu.zip");
        assert_eq!(name, "RPU.zip");
        assert_eq!(version, "v3.1");
    }

    #[test]
    fn apply_rpu_ddraw_settings_only_touches_differing_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(
            tmp.path().join("ddraw.ini"),
            "[Debugging]\nBoostScriptDialogLimit=0\n[Scripts]\nWorldMapSlots=20\nUnrelated=keep\n",
        )
        .unwrap();

        // RPU wants BoostScriptDialogLimit bumped, but WorldMapSlots already matches
        // exactly - that line, and the unrelated one, must survive completely untouched.
        let rpu_ddraw = "[Debugging]\nBoostScriptDialogLimit=10000\n[Scripts]\nWorldMapSlots=20\n";
        apply_rpu_ddraw_settings(&paths, rpu_ddraw).unwrap();

        let saved = std::fs::read_to_string(tmp.path().join("ddraw.ini")).unwrap();
        assert_eq!(saved, "[Debugging]\nBoostScriptDialogLimit=10000\n[Scripts]\nWorldMapSlots=20\nUnrelated=keep\n");
    }

    #[test]
    fn apply_rpu_ddraw_settings_creates_a_missing_section() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("ddraw.ini"), "[Debugging]\nSomeKey=1\n").unwrap();

        apply_rpu_ddraw_settings(&paths, "[Graphics]\nWindowed=1\n").unwrap();

        let saved = std::fs::read_to_string(tmp.path().join("ddraw.ini")).unwrap();
        // parse_ini_text lowercases section/key names (see dat.rs), so the newly-created
        // section header comes out as "[graphics]" - matching Python's own behavior here,
        // surprising as it looks.
        assert!(saved.contains("[graphics]"));
        assert!(saved.contains("windowed=1"));
    }

    #[test]
    fn apply_rpu_ddraw_settings_treats_a_case_only_difference_as_unchanged() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::write(tmp.path().join("ddraw.ini"), "[Debugging]\nSomeFlag=On\n").unwrap();

        // RPU's own ddraw.ini spells the value "on" (lowercase) - same effective value, so
        // the existing line's exact casing must be left alone, not rewritten to match.
        apply_rpu_ddraw_settings(&paths, "[Debugging]\nSomeFlag=on\n").unwrap();

        let saved = std::fs::read_to_string(tmp.path().join("ddraw.ini")).unwrap();
        assert_eq!(saved, "[Debugging]\nSomeFlag=On\n");
    }

    #[test]
    fn apply_rpu_ddraw_settings_is_a_noop_without_an_existing_ddraw_ini() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        apply_rpu_ddraw_settings(&paths, "[Graphics]\nWindowed=1\n").unwrap();
        assert!(!tmp.path().join("ddraw.ini").exists());
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
    fn extract_rpu_release_copies_wanted_files_and_captures_ddraw_text() {
        let zip = build_zip(&[
            ("fallout2.exe", b"classic engine, skipped"),
            ("ddraw.dll", b"sfall injection, skipped"),
            ("ddraw.ini", b"[Debugging]\nBoostScriptDialogLimit=10000\n"),
            ("appearance/hero.frm", b"sprite"),
            ("data/sound/music/map.acm", b"music"),
            ("data/art/hero.frm", b"skipped, not sound"),
            ("mods/rpu.dat", b"rpu content"),
            ("mods/npc_armor.dat", b"companion content"),
            ("mods/mods_order.txt", b"never overwrite this"),
            ("mods/sub/nested.dat", b"skipped, nested"),
        ]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());

        let (ddraw_text, mods_files) = extract_rpu_release(&zip, &paths).unwrap();
        assert_eq!(ddraw_text.unwrap(), "[Debugging]\nBoostScriptDialogLimit=10000\n");
        assert_eq!(std::fs::read(tmp.path().join("appearance").join("hero.frm")).unwrap(), b"sprite");
        assert_eq!(
            std::fs::read(tmp.path().join("data").join("sound").join("music").join("map.acm")).unwrap(),
            b"music"
        );
        assert_eq!(std::fs::read(tmp.path().join("mods").join("rpu.dat")).unwrap(), b"rpu content");
        assert!(!tmp.path().join("fallout2.exe").exists());
        assert!(!tmp.path().join("ddraw.dll").exists());
        assert!(!tmp.path().join("data").join("art").join("hero.frm").exists());
        assert!(!tmp.path().join("mods").join("mods_order.txt").exists());
        assert!(!tmp.path().join("mods").join("sub").exists());
        // mods_order.txt and the nested file are correctly absent - only flat mods/ files
        // make it into the "what did we actually write" list `install_rpu` uses for
        // companion attribution.
        let mut mods_files = mods_files;
        mods_files.sort();
        assert_eq!(mods_files, vec!["npc_armor.dat".to_string(), "rpu.dat".to_string()]);
    }

    #[test]
    fn extract_rpu_release_errors_when_nothing_recognized_is_found() {
        let zip = build_zip(&[("fallout2.exe", b"x"), ("ddraw.dll", b"x")]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert!(extract_rpu_release(&zip, &paths).is_err());
    }

    #[test]
    fn extract_rpu_release_errors_even_if_only_ddraw_ini_is_present() {
        // Matches Python precisely: capturing the top-level ddraw.ini text does its own
        // `continue` without incrementing `copied`, so an archive containing *only*
        // ddraw.ini (no mods/appearance/sound content at all) still counts as "nothing
        // recognized" and errors - the captured text is simply discarded with it.
        let zip = build_zip(&[("ddraw.ini", b"[Debugging]\nBoostScriptDialogLimit=10000\n")]);
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let err = extract_rpu_release(&zip, &paths).unwrap_err();
        assert!(err.contains("no recognized files found"));
    }

    #[test]
    fn load_rpu_companions_is_empty_before_rpu_is_installed() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert!(load_rpu_companions(&paths).is_empty());
    }

    #[test]
    fn owned_file_names_always_includes_core_and_optional_extras() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let owned = owned_file_names(&paths);
        assert!(owned.contains("rpu.dat"));
        assert!(owned.contains("rpu.ini"));
        assert!(owned.contains("rpu_extended_flamer.dat"));
        assert!(!owned.contains("npc_armor.dat"), "companions are only owned once recorded");
    }

    #[test]
    fn owned_file_names_includes_recorded_companions() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mut state = BTreeMap::new();
        state.insert("companions".to_string(), "npc_armor.dat,npcarmor.ini".to_string());
        save_rpu_state(&paths, &state).unwrap();

        let owned = owned_file_names(&paths);
        assert!(owned.contains("npc_armor.dat"));
        assert!(owned.contains("npcarmor.ini"));
    }

    #[test]
    fn delete_rpu_removes_dat_files_ini_and_state() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("rpu.dat"), b"x").unwrap();
        std::fs::write(mods.join("rpu.ini"), b"x").unwrap();
        std::fs::write(mods.join("rpu_extended_flamer.dat"), b"x").unwrap();
        std::fs::write(mods.join("mods_order.txt"), "rpu.dat\n").unwrap();
        let mut state = BTreeMap::new();
        state.insert("version".to_string(), "v1".to_string());
        save_rpu_state(&paths, &state).unwrap();

        let removed = delete_rpu(&paths);
        assert_eq!(removed.len(), 3);
        assert!(!mods.join("rpu.dat").exists());
        assert!(!mods.join("rpu.ini").exists());
        assert!(!mods.join("rpu_extended_flamer.dat").exists());
        assert!(!rpu_state_path(&paths).exists());
        assert_eq!(std::fs::read_to_string(mods.join("mods_order.txt")).unwrap().trim(), "");
    }

    #[test]
    fn delete_rpu_also_removes_its_recorded_companions_and_their_order_lines() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("rpu.dat"), b"x").unwrap();
        std::fs::write(mods.join("npc_armor.dat"), b"x").unwrap();
        std::fs::write(mods.join("npcarmor.ini"), b"x").unwrap();
        // The player ticked the "enable" checkbox for the companion at some point, so it
        // picked up its own mods_order.txt line, same as any regular mod would.
        std::fs::write(mods.join("mods_order.txt"), "rpu.dat\nnpc_armor.dat\n").unwrap();
        let mut state = BTreeMap::new();
        state.insert("version".to_string(), "v1".to_string());
        state.insert("companions".to_string(), "npc_armor.dat,npcarmor.ini".to_string());
        save_rpu_state(&paths, &state).unwrap();

        delete_rpu(&paths);
        assert!(!mods.join("npc_armor.dat").exists());
        assert!(!mods.join("npcarmor.ini").exists());
        assert_eq!(std::fs::read_to_string(mods.join("mods_order.txt")).unwrap().trim(), "");
    }
}
