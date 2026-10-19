//! Adding a campaign to Steam as a non-Steam game shortcut: for Sonora/Nevada, which have no
//! Steam app of their own to paste a Launch Options line into. Et Tu and CE already have
//! real Steam entries (Fallout 2's, and Fallout 1's forcing Et Tu) - see `launch_option.rs` -
//! so this is only for the other two.
//!
//! Steam stores non-Steam shortcuts in a per-user `shortcuts.vdf`, in an undocumented but
//! well-understood binary format (not the human-readable text VDF): each value is prefixed
//! with a type byte - 0x00 nested map, 0x01 null-terminated string, 0x02 int32 - and every
//! map ends with 0x08. `read_binary_vdf`/`write_binary_vdf` implement just that, generally
//! enough to round-trip a real file byte-for-byte (verified with a synthetic fixture, not a
//! real file - this never reads this launcher's own test data from an actual Steam install).
//!
//! `add_campaign_to_steam` only ever edits the one entry it owns (matched by its `exe` field)
//! inside the existing `shortcuts.vdf` - every other shortcut already in the file is parsed
//! and written back completely unchanged - and backs up the original once before the first
//! write, the same way the GUI's settings save backs up config files under
//! `backup/launcher/`.

use std::io;
use std::path::{Path, PathBuf};

use indexmap::IndexMap;

use super::campaigns::Campaign;
use super::launch_option::launcher_target;

/// A binary-VDF value. Python's plain `dict` preserves insertion order, and
/// `add_campaign_to_steam`'s whole contract depends on that: every shortcut entry other than
/// the one it's adding/editing must round-trip byte-for-byte unchanged, which means key order
/// within each map must survive too. `IndexMap` is the direct equivalent - a `HashMap` that
/// also remembers insertion order - unlike a `BTreeMap`, which would silently alphabetize
/// every map's keys instead of preserving the order they were read in.
#[derive(Debug, Clone, PartialEq)]
pub enum VdfValue {
    Map(VdfMap),
    Str(String),
    Int(i32),
}

pub type VdfMap = IndexMap<String, VdfValue>;

impl VdfValue {
    pub fn as_map(&self) -> Option<&VdfMap> {
        match self {
            VdfValue::Map(m) => Some(m),
            _ => None,
        }
    }

    pub fn as_str(&self) -> Option<&str> {
        match self {
            VdfValue::Str(s) => Some(s),
            _ => None,
        }
    }
}

/// Parse one binary-VDF map from `data[start..end]`. Returns (map, offset after it). Works
/// for both a nested map (ends at its own `0x08` byte) and the implicit top-level map (ends
/// when `end` is reached, since the file has no wrapping `0x08` of its own).
fn read_binary_vdf(data: &[u8], start: usize, end: usize) -> io::Result<(VdfMap, usize)> {
    let mut obj = VdfMap::new();
    let mut i = start;
    while i < end {
        let t = data[i];
        i += 1;
        if t == 0x08 {
            return Ok((obj, i));
        }
        let j = find_nul(data, i)?;
        let key = String::from_utf8_lossy(&data[i..j]).into_owned();
        i = j + 1;

        let value = match t {
            0x00 => {
                let (nested, next) = read_binary_vdf(data, i, end)?;
                i = next;
                VdfValue::Map(nested)
            }
            0x01 => {
                let j = find_nul(data, i)?;
                let s = String::from_utf8_lossy(&data[i..j]).into_owned();
                i = j + 1;
                VdfValue::Str(s)
            }
            0x02 => {
                let bytes: [u8; 4] = data
                    .get(i..i + 4)
                    .and_then(|s| s.try_into().ok())
                    .ok_or_else(|| invalid("truncated int32"))?;
                i += 4;
                VdfValue::Int(i32::from_le_bytes(bytes))
            }
            other => {
                return Err(invalid(&format!("shortcuts.vdf: unsupported type byte {other:#x} at offset {}", i - 1)));
            }
        };
        obj.insert(key, value);
    }
    Ok((obj, i))
}

fn find_nul(data: &[u8], from: usize) -> io::Result<usize> {
    data[from..].iter().position(|&b| b == 0).map(|p| from + p).ok_or_else(|| invalid("unterminated string"))
}

fn invalid(msg: &str) -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, msg.to_string())
}

fn write_binary_vdf(obj: &VdfMap, top_level: bool, out: &mut Vec<u8>) {
    for (key, val) in obj {
        let key_bytes = key.as_bytes();
        match val {
            VdfValue::Map(m) => {
                out.push(0x00);
                out.extend_from_slice(key_bytes);
                out.push(0);
                write_binary_vdf(m, false, out);
            }
            VdfValue::Str(s) => {
                out.push(0x01);
                out.extend_from_slice(key_bytes);
                out.push(0);
                out.extend_from_slice(s.as_bytes());
                out.push(0);
            }
            VdfValue::Int(n) => {
                out.push(0x02);
                out.extend_from_slice(key_bytes);
                out.push(0);
                out.extend_from_slice(&n.to_le_bytes());
            }
        }
    }
    if !top_level {
        out.push(0x08);
    }
}

pub fn read_shortcuts(path: &Path) -> io::Result<VdfMap> {
    let data = std::fs::read(path)?;
    Ok(read_binary_vdf(&data, 0, data.len())?.0)
}

pub fn write_shortcuts(path: &Path, root: &VdfMap) -> io::Result<()> {
    let mut data = Vec::new();
    write_binary_vdf(root, true, &mut data);

    let backup = path.with_file_name(format!("{}.bak-f2launcher", path.file_name().unwrap().to_string_lossy()));
    if path.is_file() && !backup.exists() {
        std::fs::copy(path, &backup)?;
    }
    let tmp = path.with_file_name(format!("{}.tmp", path.file_name().unwrap().to_string_lossy()));
    std::fs::write(&tmp, &data)?;
    std::fs::rename(&tmp, path)
}

/// Every `userdata/<id>/config` folder this install of Steam might have, across the handful
/// of locations each OS commonly uses. Best-effort: Steam's own install location is
/// user-configurable, so this can't be exhaustive.
fn steam_userdata_configs() -> Vec<PathBuf> {
    match home_dir() {
        Some(home) => steam_userdata_configs_under(&home),
        None => Vec::new(),
    }
}

/// The lookup above, with `$HOME` passed in explicitly instead of read from the real
/// environment - so tests can exercise it without mutating process-global state (which
/// `std::env::set_var` both requires `unsafe` for and isn't safely shareable across the
/// parallel threads `cargo test` runs on).
fn steam_userdata_configs_under(home: &Path) -> Vec<PathBuf> {
    let mut bases = Vec::new();
    if cfg!(target_os = "linux") || cfg!(target_os = "macos") {
        bases.push(home.join(".steam/steam/userdata"));
        bases.push(home.join(".local/share/Steam/userdata"));
        if cfg!(target_os = "macos") {
            bases.push(home.join("Library/Application Support/Steam/userdata"));
        }
    } else {
        for var in ["ProgramFiles(x86)", "ProgramFiles"] {
            if let Ok(dir) = std::env::var(var) {
                bases.push(PathBuf::from(dir).join("Steam").join("userdata"));
            }
        }
    }

    let mut seen = std::collections::HashSet::new();
    let mut unique = Vec::new();
    for base in bases {
        let Ok(entries) = std::fs::read_dir(&base) else { continue };
        for entry in entries.flatten() {
            let config = entry.path().join("config");
            if !config.is_dir() {
                continue;
            }
            let resolved = config.canonicalize().unwrap_or_else(|_| config.clone());
            if seen.insert(resolved) {
                unique.push(config);
            }
        }
    }
    unique
}

fn home_dir() -> Option<PathBuf> {
    std::env::var_os("HOME").map(PathBuf::from)
}

/// The Steam profile most likely to be the one in use: whichever has the most recently
/// modified `localconfig.vdf`. Falls back to the only candidate, or `None`.
fn pick_userdata_config() -> Option<PathBuf> {
    let configs = steam_userdata_configs();
    if configs.is_empty() {
        return None;
    }
    let with_mtime: Vec<(PathBuf, std::time::SystemTime)> = configs
        .iter()
        .filter_map(|c| {
            let m = c.join("localconfig.vdf").metadata().ok()?.modified().ok()?;
            Some((c.clone(), m))
        })
        .collect();
    if with_mtime.is_empty() {
        return Some(configs[0].clone());
    }
    with_mtime.into_iter().max_by_key(|(_, mtime)| *mtime).map(|(c, _)| c)
}

/// Add or update (matched by `exe`) one non-Steam game shortcut. Returns the `shortcuts.vdf`
/// path written.
pub fn add_non_steam_game(
    name: &str,
    exe: &Path,
    start_dir: &Path,
    launch_options: &str,
) -> Result<PathBuf, String> {
    let config_dir = pick_userdata_config()
        .ok_or_else(|| "Couldn't find a Steam userdata folder - is Steam installed?".to_string())?;
    add_non_steam_game_in(&config_dir, name, exe, start_dir, launch_options)
}

/// The actual shortcuts.vdf read/edit/write, with the Steam profile directory passed in
/// explicitly - split out from `add_non_steam_game` so tests can exercise it against a
/// temp directory instead of a real (or faked-via-env-var) Steam install.
fn add_non_steam_game_in(
    config_dir: &Path,
    name: &str,
    exe: &Path,
    start_dir: &Path,
    launch_options: &str,
) -> Result<PathBuf, String> {
    let path = config_dir.join("shortcuts.vdf");

    let mut root = if path.is_file() {
        read_shortcuts(&path).map_err(|e| e.to_string())?
    } else {
        let mut r = VdfMap::new();
        r.insert("shortcuts".to_string(), VdfValue::Map(VdfMap::new()));
        r
    };
    let shortcuts = match root.entry("shortcuts".to_string()).or_insert_with(|| VdfValue::Map(VdfMap::new())) {
        VdfValue::Map(m) => m,
        _ => return Err("shortcuts.vdf: top-level \"shortcuts\" is not a map".to_string()),
    };

    let exe_quoted = format!("\"{}\"", exe.display());
    let existing_key = shortcuts
        .iter()
        .find(|(_, v)| v.as_map().and_then(|m| m.get("exe")).and_then(|e| e.as_str()) == Some(exe_quoted.as_str()))
        .map(|(k, _)| k.clone());
    let key = existing_key.unwrap_or_else(|| shortcuts.len().to_string());

    let existing = shortcuts.get(&key).and_then(VdfValue::as_map).cloned().unwrap_or_default();
    let get_str = |k: &str, default: &str| -> String {
        existing.get(k).and_then(VdfValue::as_str).unwrap_or(default).to_string()
    };
    let get_int = |k: &str, default: i32| -> i32 {
        match existing.get(k) {
            Some(VdfValue::Int(n)) => *n,
            _ => default,
        }
    };

    let mut entry = VdfMap::new();
    entry.insert("appname".to_string(), VdfValue::Str(name.to_string()));
    entry.insert("exe".to_string(), VdfValue::Str(exe_quoted));
    entry.insert("StartDir".to_string(), VdfValue::Str(format!("\"{}\"", start_dir.display())));
    entry.insert("icon".to_string(), VdfValue::Str(get_str("icon", "")));
    entry.insert("ShortcutPath".to_string(), VdfValue::Str(String::new()));
    entry.insert("LaunchOptions".to_string(), VdfValue::Str(launch_options.to_string()));
    entry.insert("IsHidden".to_string(), VdfValue::Int(0));
    entry.insert("AllowDesktopConfig".to_string(), VdfValue::Int(1));
    entry.insert("AllowOverlay".to_string(), VdfValue::Int(1));
    entry.insert("OpenVR".to_string(), VdfValue::Int(0));
    entry.insert("Devkit".to_string(), VdfValue::Int(0));
    entry.insert("DevkitGameID".to_string(), VdfValue::Str(String::new()));
    entry.insert("DevkitOverrideAppID".to_string(), VdfValue::Int(0));
    entry.insert("LastPlayTime".to_string(), VdfValue::Int(get_int("LastPlayTime", 0)));
    entry.insert("FlatpakAppID".to_string(), VdfValue::Str(get_str("FlatpakAppID", "")));
    entry.insert(
        "tags".to_string(),
        existing.get("tags").cloned().unwrap_or_else(|| VdfValue::Map(VdfMap::new())),
    );

    shortcuts.insert(key, VdfValue::Map(entry));
    write_shortcuts(&path, &root).map_err(|e| e.to_string())?;
    Ok(path)
}

/// Add this launcher as a non-Steam game, forced into `campaign` - for a campaign with no
/// Steam app of its own (Sonora, Nevada). Pressing Play on the resulting Steam entry opens
/// straight into that campaign.
pub fn add_campaign_to_steam(campaign: &Campaign) -> Result<PathBuf, String> {
    let target = launcher_target();
    let start_dir = target.parent().map(Path::to_path_buf).unwrap_or_default();
    add_non_steam_game(
        &format!("Fallout 2 Launcher — {}", campaign.name),
        &target,
        &start_dir,
        &format!("--campaign={}", campaign.id),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_vdf_bytes() -> Vec<u8> {
        // {"shortcuts": {"0": {"appname": "Game", "exe": "\"/path/game\"", "IsHidden": 0}}}
        let mut out = Vec::new();
        out.push(0x00);
        out.extend_from_slice(b"shortcuts\0");
        out.push(0x00);
        out.extend_from_slice(b"0\0");
        out.push(0x01);
        out.extend_from_slice(b"appname\0");
        out.extend_from_slice(b"Game\0");
        out.push(0x01);
        out.extend_from_slice(b"exe\0");
        out.extend_from_slice(b"\"/path/game\"\0");
        out.push(0x02);
        out.extend_from_slice(b"IsHidden\0");
        out.extend_from_slice(&0i32.to_le_bytes());
        out.push(0x08); // end of "0"
        out.push(0x08); // end of "shortcuts"
        out
    }

    #[test]
    fn reads_a_synthetic_shortcuts_file() {
        let data = sample_vdf_bytes();
        let (root, consumed) = read_binary_vdf(&data, 0, data.len()).unwrap();
        assert_eq!(consumed, data.len());
        let shortcuts = root.get("shortcuts").unwrap().as_map().unwrap();
        let entry = shortcuts.get("0").unwrap().as_map().unwrap();
        assert_eq!(entry.get("appname").unwrap().as_str(), Some("Game"));
        assert_eq!(entry.get("exe").unwrap().as_str(), Some("\"/path/game\""));
        assert_eq!(entry.get("IsHidden"), Some(&VdfValue::Int(0)));
    }

    #[test]
    fn round_trips_byte_for_byte() {
        let data = sample_vdf_bytes();
        let (root, _) = read_binary_vdf(&data, 0, data.len()).unwrap();
        let mut written = Vec::new();
        write_binary_vdf(&root, true, &mut written);
        assert_eq!(written, data);
    }

    #[test]
    fn rejects_an_unknown_type_byte() {
        let mut data = sample_vdf_bytes();
        data[0] = 0x03; // not a valid top-level-first-byte type
        assert!(read_binary_vdf(&data, 0, data.len()).is_err());
    }

    #[test]
    fn round_trips_a_doubly_nested_map() {
        // {"shortcuts": {"0": {"tags": {"favorite": "1"}}}}
        let mut out = Vec::new();
        out.push(0x00);
        out.extend_from_slice(b"shortcuts\0");
        out.push(0x00);
        out.extend_from_slice(b"0\0");
        out.push(0x00);
        out.extend_from_slice(b"tags\0");
        out.push(0x01);
        out.extend_from_slice(b"favorite\0");
        out.extend_from_slice(b"1\0");
        out.push(0x08); // end of "tags"
        out.push(0x08); // end of "0"
        out.push(0x08); // end of "shortcuts"

        let (root, consumed) = read_binary_vdf(&out, 0, out.len()).unwrap();
        assert_eq!(consumed, out.len());
        let tags = root
            .get("shortcuts")
            .unwrap()
            .as_map()
            .unwrap()
            .get("0")
            .unwrap()
            .as_map()
            .unwrap()
            .get("tags")
            .unwrap()
            .as_map()
            .unwrap();
        assert_eq!(tags.get("favorite").unwrap().as_str(), Some("1"));

        let mut written = Vec::new();
        write_binary_vdf(&root, true, &mut written);
        assert_eq!(written, out);
    }

    #[test]
    fn write_shortcuts_backs_up_once_and_round_trips() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("shortcuts.vdf");
        let original = sample_vdf_bytes();
        std::fs::write(&path, &original).unwrap();

        let root = read_shortcuts(&path).unwrap();
        write_shortcuts(&path, &root).unwrap();

        let backup = tmp.path().join("shortcuts.vdf.bak-f2launcher");
        assert_eq!(std::fs::read(&backup).unwrap(), original);
        assert_eq!(std::fs::read(&path).unwrap(), original);

        // A second write must not clobber the backup with the (already-identical) new file.
        std::fs::write(&path, b"different bytes now").unwrap();
        let root2 = VdfMap::new();
        write_shortcuts(&path, &root2).unwrap();
        assert_eq!(std::fs::read(&backup).unwrap(), original);
    }

    #[test]
    fn add_non_steam_game_creates_and_then_updates_the_same_entry() {
        let tmp = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(tmp.path()).unwrap();

        let exe = PathBuf::from("/game/f2launcher");
        let start = PathBuf::from("/game");
        let path1 =
            add_non_steam_game_in(tmp.path(), "Fallout 2 Launcher — Nevada", &exe, &start, "--campaign=nevada")
                .unwrap();
        let root = read_shortcuts(&path1).unwrap();
        let shortcuts = root.get("shortcuts").unwrap().as_map().unwrap();
        assert_eq!(shortcuts.len(), 1);
        let first_key = shortcuts.keys().next().unwrap().clone();

        // Adding the same exe again should update the existing entry, not create a second one.
        let path2 = add_non_steam_game_in(
            tmp.path(),
            "Fallout 2 Launcher — Nevada (renamed)",
            &exe,
            &start,
            "--campaign=nevada",
        )
        .unwrap();
        let root2 = read_shortcuts(&path2).unwrap();
        let shortcuts2 = root2.get("shortcuts").unwrap().as_map().unwrap();
        assert_eq!(shortcuts2.len(), 1);
        assert_eq!(
            shortcuts2.get(&first_key).unwrap().as_map().unwrap().get("appname").unwrap().as_str(),
            Some("Fallout 2 Launcher — Nevada (renamed)")
        );
    }

    #[test]
    fn add_non_steam_game_preserves_icon_last_play_time_and_tags_on_update() {
        let tmp = tempfile::tempdir().unwrap();
        let exe = PathBuf::from("/game/f2launcher");
        let start = PathBuf::from("/game");

        // Seed an existing entry with fields a real Steam client would have set, that this
        // launcher's own write must not clobber when it later updates the same shortcut.
        let mut tags = VdfMap::new();
        tags.insert("favorite".to_string(), VdfValue::Str("1".to_string()));
        let mut existing_entry = VdfMap::new();
        existing_entry.insert("appname".to_string(), VdfValue::Str("Old Name".to_string()));
        existing_entry.insert(
            "exe".to_string(),
            VdfValue::Str(format!("\"{}\"", exe.display())),
        );
        existing_entry.insert("icon".to_string(), VdfValue::Str("/game/icon.ico".to_string()));
        existing_entry.insert("LastPlayTime".to_string(), VdfValue::Int(1_700_000_000));
        existing_entry.insert("FlatpakAppID".to_string(), VdfValue::Str("com.example.App".to_string()));
        existing_entry.insert("tags".to_string(), VdfValue::Map(tags.clone()));
        let mut shortcuts = VdfMap::new();
        shortcuts.insert("0".to_string(), VdfValue::Map(existing_entry));
        let mut root = VdfMap::new();
        root.insert("shortcuts".to_string(), VdfValue::Map(shortcuts));
        let path = tmp.path().join("shortcuts.vdf");
        let mut data = Vec::new();
        write_binary_vdf(&root, true, &mut data);
        std::fs::write(&path, data).unwrap();

        add_non_steam_game_in(tmp.path(), "New Name", &exe, &start, "--campaign=nevada").unwrap();

        let updated = read_shortcuts(&path).unwrap();
        let entry = updated.get("shortcuts").unwrap().as_map().unwrap().get("0").unwrap().as_map().unwrap();
        assert_eq!(entry.get("appname").unwrap().as_str(), Some("New Name"));
        assert_eq!(entry.get("icon").unwrap().as_str(), Some("/game/icon.ico"));
        assert_eq!(entry.get("LastPlayTime"), Some(&VdfValue::Int(1_700_000_000)));
        assert_eq!(entry.get("FlatpakAppID").unwrap().as_str(), Some("com.example.App"));
        assert_eq!(entry.get("tags"), Some(&VdfValue::Map(tags)));
    }

    #[test]
    fn add_non_steam_game_errors_when_shortcuts_key_is_not_a_map() {
        let tmp = tempfile::tempdir().unwrap();
        let mut root = VdfMap::new();
        root.insert("shortcuts".to_string(), VdfValue::Str("not a map".to_string()));
        let path = tmp.path().join("shortcuts.vdf");
        let mut data = Vec::new();
        write_binary_vdf(&root, true, &mut data);
        std::fs::write(&path, data).unwrap();

        let result = add_non_steam_game_in(
            tmp.path(),
            "Name",
            Path::new("/game/f2launcher"),
            Path::new("/game"),
            "",
        );
        assert!(result.is_err());
    }

    #[test]
    fn add_non_steam_game_errors_cleanly_on_a_corrupt_existing_file() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("shortcuts.vdf");
        std::fs::write(&path, [0xFFu8, 0x00, 0x00]).unwrap(); // 0xFF is not a valid type byte

        let result = add_non_steam_game_in(
            tmp.path(),
            "Name",
            Path::new("/game/f2launcher"),
            Path::new("/game"),
            "",
        );
        assert!(result.is_err());
    }

    #[test]
    fn steam_userdata_configs_under_finds_and_dedupes_profiles() {
        let tmp = tempfile::tempdir().unwrap();
        let userdata = tmp.path().join(".steam/steam/userdata");
        std::fs::create_dir_all(userdata.join("111").join("config")).unwrap();
        std::fs::create_dir_all(userdata.join("222")).unwrap(); // no config dir - skipped

        let configs = steam_userdata_configs_under(tmp.path());
        assert_eq!(configs, vec![userdata.join("111").join("config")]);
    }

    #[test]
    fn steam_userdata_configs_under_with_no_steam_install_is_empty() {
        let tmp = tempfile::tempdir().unwrap();
        assert!(steam_userdata_configs_under(tmp.path()).is_empty());
    }
}
