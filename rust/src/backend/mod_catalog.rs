//! Mods the launcher can download: `schemas/catalog.toml` (compiled into the binary, same
//! as the settings schemas - see `schema.rs`), GitHub release resolution, installing,
//! update-checking and deleting.

use std::collections::BTreeMap;
use std::io;
use std::path::{Path, PathBuf};

use super::paths::Paths;

/// A mod the launcher can download: see `schemas/catalog.toml`.
#[derive(Debug, Clone, PartialEq)]
pub struct CatalogEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub homepage: String,
    pub dat: Vec<String>,
    pub github: Option<String>,
    pub asset: String,
    pub raw: Option<String>,
    pub ini: Option<String>,
    /// Campaign ids whose Mods page offers this as a download (default: `["ce"]`).
    pub campaigns: Vec<String>,
    /// Where this entry's zip is extracted, relative to the campaign root - `None` means the
    /// usual drop-in-`mods/`-and-toggle-via-`mods_order.txt` behavior (every entry before this
    /// field existed). `Some(dir)` means this entry isn't a "mod" the engine toggles at all -
    /// it's content CE reads directly from `dir` (e.g. HQ music's loose `.acm` files under
    /// `data/sound/music`) - so install/delete skip `mods_order.txt` entirely, and `dat`'s
    /// files may collide by name with ones already there (see `backup_existing_files`).
    pub extract_to: Option<String>,
}

impl CatalogEntry {
    /// A `mods/`-backed entry is installed when its `dat` file/folder exists there, same as
    /// ever. A flat (`extract_to`) entry can't use file presence - its files may share names
    /// with ones the campaign already ships (HQ music overwrites Et Tu's own stock tracks) -
    /// so it goes by this launcher's own install-state bookkeeping instead (see
    /// `install_catalog_entry`'s version save).
    pub fn is_installed(&self, paths: &Paths) -> bool {
        if self.extract_to.is_some() {
            load_catalog_state(paths).contains_key(&self.id)
        } else {
            self.dat.iter().any(|d| paths.mod_installed(d))
        }
    }

    pub fn offered_for(&self, campaign_id: &str) -> bool {
        self.campaigns.iter().any(|c| c == campaign_id)
    }
}

pub fn load_mod_catalog() -> Vec<CatalogEntry> {
    let Ok(doc) = super::schema::CATALOG_TOML.parse::<toml::Value>() else {
        return Vec::new();
    };
    let Some(entries) = doc.get("entry").and_then(|v| v.as_array()) else {
        return Vec::new();
    };
    entries.iter().filter_map(parse_catalog_entry).collect()
}

fn parse_catalog_entry(e: &toml::Value) -> Option<CatalogEntry> {
    let id = e.get("id")?.as_str()?.to_string();
    let name = e.get("name")?.as_str()?.to_string();
    let homepage = e.get("homepage")?.as_str()?.to_string();
    let dat: Vec<String> =
        e.get("dat")?.as_array()?.iter().filter_map(|v| v.as_str().map(str::to_string)).collect();
    Some(CatalogEntry {
        id,
        name,
        description: e.get("description").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        homepage,
        dat,
        github: e.get("github").and_then(|v| v.as_str()).map(str::to_string),
        asset: e.get("asset").and_then(|v| v.as_str()).unwrap_or("*.zip").to_string(),
        raw: e.get("raw").and_then(|v| v.as_str()).map(str::to_string),
        ini: e.get("ini").and_then(|v| v.as_str()).map(str::to_string),
        campaigns: e
            .get("campaigns")
            .and_then(|v| v.as_array())
            .map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect())
            .unwrap_or_else(|| vec!["ce".to_string()]),
        extract_to: e.get("extract_to").and_then(|v| v.as_str()).map(str::to_string),
    })
}

fn catalog_state_path(paths: &Paths) -> PathBuf {
    paths.ci_path("mods/.f2launcher-mods.json", None)
}

/// Versions the launcher last installed/saw, by catalog id. Its own bookkeeping file. A
/// `BTreeMap` rather than a `HashMap` so saving it back out is deterministically sorted by
/// key, matching Python's `json.dumps(..., sort_keys=True)`, with no extra sorting step.
pub fn load_catalog_state(paths: &Paths) -> BTreeMap<String, String> {
    std::fs::read_to_string(catalog_state_path(paths))
        .ok()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

pub fn save_catalog_state(paths: &Paths, state: &BTreeMap<String, String>) -> io::Result<()> {
    let path = catalog_state_path(paths);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let json = serde_json::to_string_pretty(state).expect("BTreeMap<String, String> always serializes");
    std::fs::write(path, json)
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedDownload {
    pub url: String,
    pub filename: String,
    /// The GitHub release tag, or `None` for a raw (non-versioned) download.
    pub version: Option<String>,
}

/// Return (url, filename, version) for the file this entry's catalog data points at.
pub fn resolve_download(entry: &CatalogEntry) -> Result<ResolvedDownload, String> {
    if let Some(raw) = &entry.raw {
        return Ok(ResolvedDownload { url: raw.clone(), filename: entry.dat[0].clone(), version: None });
    }
    let github = entry
        .github
        .as_deref()
        .ok_or_else(|| format!("{}: catalog entry has neither github nor raw", entry.name))?;
    let api_url = format!("https://api.github.com/repos/{github}/releases/latest");
    let body = super::http::get_json(&api_url, std::time::Duration::from_secs(20))?;
    pick_release_asset(&body, &entry.asset, &entry.name)
}

/// Pick the first release asset matching `asset_pattern` (an fnmatch-style pattern, usually
/// `"*.zip"`) out of a GitHub "latest release" API response. Split out from
/// `resolve_download` so it can be tested against a canned JSON response, with no network
/// access needed.
fn pick_release_asset(
    body: &serde_json::Value,
    asset_pattern: &str,
    entry_name: &str,
) -> Result<ResolvedDownload, String> {
    let version = body.get("tag_name").and_then(|v| v.as_str()).map(str::to_string);
    let pattern = asset_pattern.to_lowercase();
    let assets = body.get("assets").and_then(|v| v.as_array()).cloned().unwrap_or_default();
    for asset in &assets {
        let name = asset.get("name").and_then(|v| v.as_str()).unwrap_or("");
        if fnmatch_simple(&pattern, &name.to_lowercase()) {
            let url =
                asset.get("browser_download_url").and_then(|v| v.as_str()).unwrap_or("").to_string();
            return Ok(ResolvedDownload { url, filename: name.to_string(), version });
        }
    }
    Err(format!("{entry_name}: no release asset matching {asset_pattern:?}"))
}

/// A `*`-only glob matcher - the only wildcard `schemas/catalog.toml` ever actually uses for
/// its `asset` patterns (default `"*.zip"`), so that's all this supports.
fn fnmatch_simple(pattern: &str, name: &str) -> bool {
    let parts: Vec<&str> = pattern.split('*').collect();
    if parts.len() == 1 {
        return name == pattern;
    }
    let mut rest = name;
    let last = parts.len() - 1;
    for (i, part) in parts.iter().enumerate() {
        if part.is_empty() {
            continue;
        }
        if i == 0 {
            if !rest.starts_with(part) {
                return false;
            }
            rest = &rest[part.len()..];
        } else if i == last {
            return rest.ends_with(part);
        } else {
            match rest.find(part) {
                Some(pos) => rest = &rest[pos + part.len()..],
                None => return false,
            }
        }
    }
    true
}

/// Latest version if it's newer than what we last installed, else `None`. The first time an
/// entry is checked, its current version becomes the baseline (nothing to compare against
/// yet), so no update is reported that round.
pub fn check_for_update(paths: &Paths, entry: &CatalogEntry) -> Option<String> {
    entry.github.as_ref()?;
    let version = resolve_download(entry).ok()?.version?;
    let mut state = load_catalog_state(paths);
    match state.get(&entry.id) {
        None => {
            state.insert(entry.id.clone(), version);
            let _ = save_catalog_state(paths, &state); // best-effort, like Python's version
            None
        }
        Some(baseline) if *baseline != version => Some(version),
        Some(_) => None,
    }
}

/// Download `entry`'s file and place it under `mods/` (or, for an `extract_to` entry, under
/// that campaign-relative folder instead - see `CatalogEntry::extract_to`). `on_stage` is
/// called once the download finishes and installing begins - see
/// `docs/adr/0006-indeterminate-download-progress.md`.
pub fn install_catalog_entry(paths: &Paths, entry: &CatalogEntry, on_stage: &dyn Fn(&str)) -> Result<(), String> {
    let resolved = resolve_download(entry)?;
    let bytes = super::http::get_bytes(&resolved.url, std::time::Duration::from_secs(60))?;
    on_stage("Installing\u{2026}");

    if let Some(target) = &entry.extract_to {
        let dest = paths.ci_path(target, None);
        std::fs::create_dir_all(&dest).map_err(|e| e.to_string())?;
        backup_existing_files(paths, &entry.id, &dest, &entry.dat)?;
        extract_zip_flat(&bytes, &dest, &entry.name)?;
    } else {
        let mods_dir = paths.ci_path("mods", None);
        std::fs::create_dir_all(&mods_dir).map_err(|e| e.to_string())?;

        if entry.raw.is_some() {
            std::fs::write(mods_dir.join(&entry.dat[0]), &bytes).map_err(|e| e.to_string())?;
        } else {
            extract_mods_from_zip(&bytes, &mods_dir, &entry.name)?;
        }

        enable_mod_file(paths, &entry.dat[0]).map_err(|e| e.to_string())?;
    }

    if let Some(version) = resolved.version {
        let mut state = load_catalog_state(paths);
        state.insert(entry.id.clone(), version);
        save_catalog_state(paths, &state).map_err(|e| e.to_string())?;
    }
    Ok(())
}

/// Where a flat (`extract_to`) entry's pre-install files are preserved - see
/// `backup_existing_files`/`delete_flat_entry_files`.
fn content_backup_dir(paths: &Paths, entry_id: &str) -> PathBuf {
    paths.backup_dir().join("content-backups").join(entry_id)
}

/// Before a flat entry overwrites `dest` with downloaded content, preserve whatever was
/// already there under any of `dat`'s names - HQ music's files share names with Et Tu's own
/// stock tracks, so without this, installing is a silent one-way loss of the originals. Only
/// the *first* install snapshots anything: a later Update must never re-backup, or it would
/// capture the installed content itself as the "original" and the real pre-install files would
/// be gone for good.
fn backup_existing_files(paths: &Paths, entry_id: &str, dest: &Path, dat: &[String]) -> Result<(), String> {
    let backup_dir = content_backup_dir(paths, entry_id);
    if backup_dir.is_dir() {
        return Ok(());
    }
    for name in dat {
        let src = dest.join(name);
        if src.is_file() {
            std::fs::create_dir_all(&backup_dir).map_err(|e| e.to_string())?;
            std::fs::copy(&src, backup_dir.join(name)).map_err(|e| e.to_string())?;
        }
    }
    Ok(())
}

/// Remove a flat entry's files from `target` (a campaign-relative folder, e.g.
/// `data/sound/music`): restores whatever `backup_existing_files` preserved, if anything did
/// collide, or just deletes `dat`'s files if nothing did. Returns the paths touched (restored
/// or removed), same shape as `delete_mod_files`.
pub fn delete_flat_entry_files(paths: &Paths, entry_id: &str, target: &str, dat: &[String]) -> Vec<PathBuf> {
    let dest = paths.ci_path(target, None);
    let backup_dir = content_backup_dir(paths, entry_id);
    let mut touched = Vec::new();
    if backup_dir.is_dir() {
        for name in dat {
            let live = dest.join(name);
            let backed_up = backup_dir.join(name);
            let restored = if backed_up.is_file() { std::fs::copy(&backed_up, &live).is_ok() } else { false };
            if !restored && live.is_file() {
                let _ = std::fs::remove_file(&live);
            }
            touched.push(live);
        }
        let _ = std::fs::remove_dir_all(&backup_dir);
    } else {
        for name in dat {
            let p = dest.join(name);
            if p.is_file() && std::fs::remove_file(&p).is_ok() {
                touched.push(p);
            }
        }
    }
    touched
}

/// Copies every file in a downloaded zip directly under `dest`, preserving each entry's own
/// relative path - unlike `extract_mods_from_zip`, there's no `mods/`-prefix to strip, because
/// a flat (`extract_to`) entry's archive is just its content as-is (e.g. HQ music's `.acm`
/// files sit at the zip's root, meant to land straight in `data/sound/music`).
fn extract_zip_flat(bytes: &[u8], dest: &Path, entry_name: &str) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut copied = 0;
    for i in 0..zip.len() {
        let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
        if file.is_dir() {
            continue;
        }
        let rel: Vec<String> =
            file.name().replace('\\', "/").split('/').filter(|p| !p.is_empty()).map(str::to_string).collect();
        if rel.is_empty() {
            continue;
        }
        let out_path = dest.join(rel.join("/"));
        if let Some(parent) = out_path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&out_path).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    if copied == 0 {
        return Err(format!("{entry_name}: archive was empty"));
    }
    Ok(())
}

/// Copies a downloaded zip's `mods/` contents into `mods_dir` as-is; everything else in the
/// archive (e.g. a Windows `ddraw.dll`) is ignored. Split out from `install_catalog_entry` so
/// it's testable against an in-memory zip, with no network access needed.
fn extract_mods_from_zip(bytes: &[u8], mods_dir: &Path, entry_name: &str) -> Result<(), String> {
    let mut zip = zip::ZipArchive::new(std::io::Cursor::new(bytes)).map_err(|e| e.to_string())?;
    let mut copied = 0;
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
        if parts.is_empty() || parts[0].to_lowercase() != "mods" || parts.len() < 2 {
            continue;
        }
        let dest = mods_dir.join(parts[1..].join("/"));
        if let Some(parent) = dest.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let mut out = std::fs::File::create(&dest).map_err(|e| e.to_string())?;
        io::copy(&mut file, &mut out).map_err(|e| e.to_string())?;
        copied += 1;
    }
    if copied == 0 {
        return Err(format!("{entry_name}: archive had no mods/ contents"));
    }
    Ok(())
}

/// Remove `dat_name` (and any `extra` files, e.g. its `.ini`) from `mods/`, and drop
/// `dat_name`'s line from `mods_order.txt`, along with any of `extra`'s own lines (an `.ini`
/// sidecar never has one, but a caller like `rpu_install::delete_rpu` passes other `.dat`
/// files that can). Returns the paths removed.
pub fn delete_mod_files(paths: &Paths, dat_name: &str, extra: &[String]) -> Vec<PathBuf> {
    let mut removed = Vec::new();
    for rel in std::iter::once(dat_name).chain(extra.iter().map(String::as_str)) {
        let p = paths.ci_path(&format!("mods/{rel}"), None);
        if p.is_file() && std::fs::remove_file(&p).is_ok() {
            removed.push(p);
        }
    }

    let path = paths.ci_path("mods/mods_order.txt", None);
    if let Ok(raw) = std::fs::read(&path) {
        let lines = super::ini_file::split_lines(&decode_latin1(&raw));
        let kept: Vec<String> = lines
            .iter()
            .filter(|ln| !matches_mod_line(ln, dat_name) && !extra.iter().any(|e| matches_mod_line(ln, e)))
            .cloned()
            .collect();
        if kept != lines {
            let _ = std::fs::write(&path, encode_latin1(&format!("{}\n", kept.join("\n"))));
        }
    }
    removed
}

/// Add `dat_name` to `mods_order.txt`, enabled, unless it's already listed.
pub fn enable_mod_file(paths: &Paths, dat_name: &str) -> io::Result<()> {
    let path = paths.ci_path("mods/mods_order.txt", None);
    let mut lines = if path.is_file() {
        super::ini_file::split_lines(&decode_latin1(&std::fs::read(&path)?))
    } else {
        Vec::new()
    };
    if lines.iter().any(|ln| matches_mod_line(ln, dat_name)) {
        return Ok(());
    }
    lines.push(dat_name.to_string());
    std::fs::write(&path, encode_latin1(&format!("{}\n", lines.join("\n"))))
}

/// True if `line` (possibly commented-out with `;`/`#`) names `dat_name`, ignoring case.
fn matches_mod_line(line: &str, dat_name: &str) -> bool {
    line.chars().filter(|c| *c != ';' && *c != '#').collect::<String>().trim().to_lowercase()
        == dat_name.to_lowercase()
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn encode_latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    #[test]
    fn load_mod_catalog_reads_the_real_embedded_catalog() {
        let entries = load_mod_catalog();
        assert!(!entries.is_empty());
        let npc_armor = entries.iter().find(|e| e.id == "npc-armor").expect("npc-armor entry");
        assert_eq!(npc_armor.github, Some("BGforgeNet/Fallout2_NPC_Armor".to_string()));
        assert_eq!(npc_armor.asset, "*.zip");
        let f2_res = entries.iter().find(|e| e.id == "f2-res").expect("f2-res entry");
        assert!(f2_res.raw.is_some());
        assert_eq!(f2_res.github, None);
        // Default-scoped entries stay CE-only...
        assert_eq!(npc_armor.campaigns, vec!["ce".to_string()]);
        assert!(!npc_armor.offered_for("ettu"));
        // ...but fallout2-ce's own README names these two as working out of the box on Et
        // Tu too ("Full Support", Party Orders/FO2Tweaks called out by name), so they opt in.
        let party_orders = entries.iter().find(|e| e.id == "party-orders").expect("party-orders entry");
        assert!(party_orders.offered_for("ce"));
        assert!(party_orders.offered_for("ettu"));
        let fo2tweaks = entries.iter().find(|e| e.id == "fo2tweaks").expect("fo2tweaks entry");
        assert!(fo2tweaks.offered_for("ettu"));
        // HQ music is a flat (extract_to) entry, scoped to Et Tu only - see
        // schemas/catalog.toml's comment on why not Vanilla/CE too.
        let hq_music = entries.iter().find(|e| e.id == "hq-music").expect("hq-music entry");
        assert_eq!(hq_music.extract_to.as_deref(), Some("data/sound/music"));
        assert!(hq_music.offered_for("ettu"));
        assert!(!hq_music.offered_for("ce"));
        assert_eq!(hq_music.dat.len(), 31);
    }

    #[test]
    fn fnmatch_simple_matches_the_default_zip_pattern() {
        assert!(fnmatch_simple("*.zip", "npc-armor-v1.2.0.zip"));
        assert!(!fnmatch_simple("*.zip", "npc-armor-v1.2.0.tar.gz"));
        assert!(fnmatch_simple("exact.name", "exact.name"));
        assert!(!fnmatch_simple("exact.name", "other.name"));
    }

    #[test]
    fn fnmatch_simple_hq_music_pattern_picks_only_the_hq_asset() {
        // The real BGforgeNet/Fallout2-HQ-music release ships these 8 assets (4 zips + their
        // .torrent files) under one release - "*hq*.zip" must land on exactly one.
        let pattern = "*hq*.zip".to_lowercase();
        let assets = [
            "fallout_music_hq_v8.1.zip",
            "fallout_music_hq_v8.1.zip.torrent",
            "fallout_music_lossless_flac_v8.1.zip",
            "fallout_music_lossless_flac_v8.1.zip.torrent",
            "fallout_music_lossless_wavpack_v8.1.zip",
            "fallout_music_lossless_wavpack_v8.1.zip.torrent",
            "fallout_music_lossless_wav_v8.1.zip",
            "fallout_music_lossless_wav_v8.1.zip.torrent",
        ];
        let matches: Vec<&str> =
            assets.iter().filter(|a| fnmatch_simple(&pattern, &a.to_lowercase())).copied().collect();
        assert_eq!(matches, vec!["fallout_music_hq_v8.1.zip"]);
    }

    #[test]
    fn parse_catalog_entry_skips_entries_missing_a_required_field() {
        let missing_homepage: toml::Value =
            r#"id = "x"
               name = "X"
               dat = ["x.dat"]"#
                .parse()
                .unwrap();
        assert!(parse_catalog_entry(&missing_homepage).is_none());

        let complete: toml::Value =
            r#"id = "x"
               name = "X"
               homepage = "https://example.com"
               dat = ["x.dat"]"#
                .parse()
                .unwrap();
        assert!(parse_catalog_entry(&complete).is_some());
    }

    #[test]
    fn resolve_download_without_github_or_raw_errors_without_any_network_call() {
        let entry = CatalogEntry {
            id: "x".into(),
            name: "X".into(),
            description: String::new(),
            homepage: String::new(),
            dat: vec!["x.dat".into()],
            github: None,
            asset: "*.zip".into(),
            raw: None,
            ini: None,
            campaigns: vec!["ce".into()],
            extract_to: None,
        };
        let err = resolve_download(&entry).unwrap_err();
        assert!(err.contains("neither github nor raw"));
    }

    #[test]
    fn resolve_download_for_a_raw_entry_needs_no_network() {
        let entry = CatalogEntry {
            id: "f2-res".into(),
            name: "High Resolution Patch".into(),
            description: String::new(),
            homepage: String::new(),
            dat: vec!["f2_res.dat".into()],
            github: None,
            asset: "*.zip".into(),
            raw: Some("https://example.com/f2_res.dat".into()),
            ini: None,
            campaigns: vec!["ce".into()],
            extract_to: None,
        };
        let resolved = resolve_download(&entry).unwrap();
        assert_eq!(resolved.url, "https://example.com/f2_res.dat");
        assert_eq!(resolved.filename, "f2_res.dat");
        assert_eq!(resolved.version, None);
    }

    #[test]
    fn pick_release_asset_finds_the_matching_zip_and_skips_others() {
        let body = serde_json::json!({
            "tag_name": "v1.2.0",
            "assets": [
                {"name": "source.tar.gz", "browser_download_url": "https://x/source.tar.gz"},
                {"name": "NPC-Armor-v1.2.0.zip", "browser_download_url": "https://x/npc-armor.zip"},
            ]
        });
        let resolved = pick_release_asset(&body, "*.zip", "NPC Armor").unwrap();
        assert_eq!(resolved.version, Some("v1.2.0".to_string()));
        assert_eq!(resolved.url, "https://x/npc-armor.zip");
    }

    #[test]
    fn pick_release_asset_errors_when_nothing_matches() {
        let body = serde_json::json!({ "tag_name": "v1.0.0", "assets": [] });
        assert!(pick_release_asset(&body, "*.zip", "NPC Armor").is_err());
    }

    #[test]
    fn extract_mods_from_zip_copies_only_the_mods_subtree() {
        let tmp = tempfile::tempdir().unwrap();
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            writer.start_file("mods/npc_armor.dat", opts).unwrap();
            writer.write_all(b"dat contents").unwrap();
            writer.start_file("mods/npcarmor.ini", opts).unwrap();
            writer.write_all(b"[main]\n").unwrap();
            writer.start_file("ddraw.dll", opts).unwrap();
            writer.write_all(b"ignored").unwrap();
            writer.finish().unwrap();
        }

        extract_mods_from_zip(&buf, tmp.path(), "NPC Armor").unwrap();
        assert_eq!(std::fs::read(tmp.path().join("npc_armor.dat")).unwrap(), b"dat contents");
        assert!(tmp.path().join("npcarmor.ini").is_file());
        assert!(!tmp.path().join("ddraw.dll").exists());
    }

    #[test]
    fn extract_mods_from_zip_accepts_backslash_separators_and_mixed_case_mods_folder() {
        let tmp = tempfile::tempdir().unwrap();
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            // A Windows-built zip can use backslashes, and "Mods" may not be lowercase.
            writer.start_file("Mods\\sub\\thing.dat", opts).unwrap();
            writer.write_all(b"nested contents").unwrap();
            writer.finish().unwrap();
        }

        extract_mods_from_zip(&buf, tmp.path(), "Something").unwrap();
        assert_eq!(
            std::fs::read(tmp.path().join("sub").join("thing.dat")).unwrap(),
            b"nested contents"
        );
    }

    #[test]
    fn extract_mods_from_zip_errors_when_archive_has_no_mods_folder() {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            writer.start_file("ddraw.dll", zip::write::SimpleFileOptions::default()).unwrap();
            writer.write_all(b"x").unwrap();
            writer.finish().unwrap();
        }
        let tmp = tempfile::tempdir().unwrap();
        assert!(extract_mods_from_zip(&buf, tmp.path(), "Something").is_err());
    }

    #[test]
    fn extract_zip_flat_copies_entries_at_the_zips_own_root_with_no_prefix_stripping() {
        let tmp = tempfile::tempdir().unwrap();
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            let opts = zip::write::SimpleFileOptions::default();
            writer.start_file("01hub.acm", opts).unwrap();
            writer.write_all(b"hq audio").unwrap();
            writer.finish().unwrap();
        }
        extract_zip_flat(&buf, tmp.path(), "HQ Music").unwrap();
        assert_eq!(std::fs::read(tmp.path().join("01hub.acm")).unwrap(), b"hq audio");
    }

    #[test]
    fn extract_zip_flat_errors_on_an_empty_archive() {
        let mut buf = Vec::new();
        {
            let mut writer = zip::ZipWriter::new(std::io::Cursor::new(&mut buf));
            writer.add_directory("empty_dir/", zip::write::SimpleFileOptions::default()).unwrap();
            writer.finish().unwrap();
        }
        let tmp = tempfile::tempdir().unwrap();
        assert!(extract_zip_flat(&buf, tmp.path(), "Something").is_err());
    }

    #[test]
    fn flat_entry_install_then_delete_restores_the_file_it_overwrote() {
        // HQ music overwrites Et Tu's own stock .acm files by name - deleting it later must
        // give the original back, not just leave the music folder empty.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let music_dir = tmp.path().join("data/sound/music");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"stock 22khz").unwrap();

        backup_existing_files(&paths, "hq-music", &music_dir, &["01hub.acm".to_string()]).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"hq 44khz").unwrap(); // simulates extract_zip_flat

        delete_flat_entry_files(&paths, "hq-music", "data/sound/music", &["01hub.acm".to_string()]);

        assert_eq!(std::fs::read(music_dir.join("01hub.acm")).unwrap(), b"stock 22khz");
        assert!(!content_backup_dir(&paths, "hq-music").exists());
    }

    #[test]
    fn flat_entry_backup_is_not_overwritten_by_a_later_update() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let music_dir = tmp.path().join("data/sound/music");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"stock 22khz").unwrap();

        backup_existing_files(&paths, "hq-music", &music_dir, &["01hub.acm".to_string()]).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"hq v8.0").unwrap();
        // A later Update re-runs backup_existing_files - it must be a no-op now, or the
        // real original would be lost, replaced by this already-installed HQ content.
        backup_existing_files(&paths, "hq-music", &music_dir, &["01hub.acm".to_string()]).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"hq v8.1").unwrap();

        delete_flat_entry_files(&paths, "hq-music", "data/sound/music", &["01hub.acm".to_string()]);
        assert_eq!(std::fs::read(music_dir.join("01hub.acm")).unwrap(), b"stock 22khz");
    }

    #[test]
    fn flat_entry_delete_with_no_prior_collision_just_removes_the_files() {
        // Nothing was there before install (no backup slot ever created), so delete has
        // nothing to restore - it should simply remove what it added.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let music_dir = tmp.path().join("data/sound/music");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("wind1.acm"), b"hq only track").unwrap();

        delete_flat_entry_files(&paths, "hq-music", "data/sound/music", &["wind1.acm".to_string()]);

        assert!(!music_dir.join("wind1.acm").exists());
    }

    #[test]
    fn flat_entry_is_installed_goes_by_launcher_state_not_file_presence() {
        // File presence can't tell HQ music apart from the stock tracks it overwrites by
        // name - CatalogEntry::is_installed must use the install-state file instead.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let entry = CatalogEntry {
            id: "hq-music".into(),
            name: "HQ Music".into(),
            description: String::new(),
            homepage: String::new(),
            dat: vec!["01hub.acm".into()],
            github: Some("BGforgeNet/Fallout2-HQ-music".into()),
            asset: "*hq*.zip".into(),
            raw: None,
            ini: None,
            campaigns: vec!["ettu".into()],
            extract_to: Some("data/sound/music".into()),
        };
        // The same filename already exists (Et Tu's own stock track) - must not read as
        // "HQ music is installed" on its own.
        let music_dir = tmp.path().join("data/sound/music");
        std::fs::create_dir_all(&music_dir).unwrap();
        std::fs::write(music_dir.join("01hub.acm"), b"stock").unwrap();
        assert!(!entry.is_installed(&paths));

        let mut state = BTreeMap::new();
        state.insert("hq-music".to_string(), "v8.1".to_string());
        save_catalog_state(&paths, &state).unwrap();
        assert!(entry.is_installed(&paths));
    }

    #[test]
    fn catalog_state_round_trips_and_sorts_keys() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mut state = BTreeMap::new();
        state.insert("z-mod".to_string(), "v2".to_string());
        state.insert("a-mod".to_string(), "v1".to_string());
        save_catalog_state(&paths, &state).unwrap();

        let reloaded = load_catalog_state(&paths);
        assert_eq!(reloaded, state);
        let raw = std::fs::read_to_string(catalog_state_path(&paths)).unwrap();
        assert!(raw.find("a-mod").unwrap() < raw.find("z-mod").unwrap());
    }

    #[test]
    fn load_catalog_state_missing_file_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert!(load_catalog_state(&paths).is_empty());
    }

    #[test]
    fn load_catalog_state_corrupt_json_is_empty_not_an_error() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        std::fs::write(catalog_state_path(&paths), b"{ this is not json").unwrap();
        assert!(load_catalog_state(&paths).is_empty());
    }

    #[test]
    fn enable_mod_file_adds_once_and_is_idempotent() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();

        enable_mod_file(&paths, "npc_armor.dat").unwrap();
        enable_mod_file(&paths, "npc_armor.dat").unwrap();

        let contents = std::fs::read_to_string(tmp.path().join("mods").join("mods_order.txt")).unwrap();
        assert_eq!(contents.lines().count(), 1);
        assert_eq!(contents.trim(), "npc_armor.dat");
    }

    #[test]
    fn enable_mod_file_recognizes_an_already_disabled_entry() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        std::fs::write(tmp.path().join("mods").join("mods_order.txt"), ";npc_armor.dat\n").unwrap();

        enable_mod_file(&paths, "npc_armor.dat").unwrap();

        let contents = std::fs::read_to_string(tmp.path().join("mods").join("mods_order.txt")).unwrap();
        assert_eq!(contents.trim(), ";npc_armor.dat");
    }

    #[test]
    fn delete_mod_files_removes_dat_and_extras_and_drops_the_order_line() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("npc_armor.dat"), b"x").unwrap();
        std::fs::write(mods.join("npcarmor.ini"), b"x").unwrap();
        std::fs::write(mods.join("other.dat"), b"x").unwrap();
        std::fs::write(mods.join("mods_order.txt"), "npc_armor.dat\nother.dat\n").unwrap();

        let removed = delete_mod_files(&paths, "npc_armor.dat", &["npcarmor.ini".to_string()]);
        assert_eq!(removed.len(), 2);
        assert!(!mods.join("npc_armor.dat").exists());
        assert!(!mods.join("npcarmor.ini").exists());
        assert!(mods.join("other.dat").exists());

        let order = std::fs::read_to_string(mods.join("mods_order.txt")).unwrap();
        assert_eq!(order.trim(), "other.dat");
    }

    #[test]
    fn delete_mod_files_matches_order_lines_commented_with_either_char() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("mods_order.txt"), "#npc_armor.dat\nother.dat\n").unwrap();

        delete_mod_files(&paths, "npc_armor.dat", &[]);

        let order = std::fs::read_to_string(mods.join("mods_order.txt")).unwrap();
        assert_eq!(order.trim(), "other.dat");
    }

    #[test]
    fn delete_mod_files_also_drops_order_lines_for_extra_dat_files() {
        // A regular mod's `extra` is just its `.ini` sidecar, which never has its own order
        // line - but rpu_install::delete_rpu passes other `.dat` files as `extra` too, and
        // those can, if the player had enabled them independently.
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("mods_order.txt"), "rpu.dat\nnpc_armor.dat\nother.dat\n").unwrap();

        delete_mod_files(&paths, "rpu.dat", &["npc_armor.dat".to_string()]);

        let order = std::fs::read_to_string(mods.join("mods_order.txt")).unwrap();
        assert_eq!(order.trim(), "other.dat");
    }

    #[test]
    fn delete_mod_files_is_case_insensitive_on_the_order_line() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let mods = tmp.path().join("mods");
        std::fs::create_dir_all(&mods).unwrap();
        std::fs::write(mods.join("mods_order.txt"), "NPC_Armor.DAT\nother.dat\n").unwrap();

        delete_mod_files(&paths, "npc_armor.dat", &[]);

        let order = std::fs::read_to_string(mods.join("mods_order.txt")).unwrap();
        assert_eq!(order.trim(), "other.dat");
    }
}
