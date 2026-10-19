//! Reading SAVE.DAT headers and the game's palette for save-slot thumbnails.

use std::fs::File;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::time::SystemTime;

use super::dat::DatCache;
use super::paths::{Paths, PREVIEW_H, PREVIEW_W};

const LOCATIONS: &[(&str, &str)] = &[
    ("ARTEMPLE", "Temple of Trials"),
    ("AR", "Arroyo"),
    ("KLA", "Klamath"),
    ("DEN", "The Den"),
    ("MOD", "Modoc"),
    ("GHOST", "Ghost Farm"),
    ("VCT", "Vault City"),
    ("VAULTC", "Vault City"),
    ("GECK", "Gecko"),
    ("BH", "Broken Hills"),
    ("BROKEN", "Broken Hills"),
    ("NEWR", "New Reno"),
    ("RED", "Redding"),
    ("NCR", "NCR"),
    ("V13", "Vault 13"),
    ("V15", "Vault 15"),
    ("SF", "San Francisco"),
    ("NAV", "Navarro"),
    ("RAIDERS", "Raiders' camp"),
    ("ABBEY", "Abbey"),
    ("EPA", "EPA"),
];

const MONTHS: [&str; 12] =
    ["Jan", "Feb", "Mar", "Apr", "May", "Jun", "Jul", "Aug", "Sep", "Oct", "Nov", "Dec"];

#[derive(Debug, Clone, PartialEq)]
pub struct SaveGame {
    pub slot: u32,
    pub path: PathBuf,
    pub name: String,
    pub description: String,
    pub location: String,
    pub map_name: String,
    pub game_date: String,
    pub mtime: SystemTime,
    pub thumbnail: Option<Vec<u8>>,
}

pub fn friendly_location(map_name: &str) -> String {
    let up = map_name.to_uppercase();
    for &(prefix, label) in LOCATIONS {
        if up.starts_with(prefix) {
            return label.to_string();
        }
    }
    if map_name.is_empty() {
        "Unknown".to_string()
    } else {
        title_case(map_name)
    }
}

pub fn read_saves(paths: &Paths) -> Vec<SaveGame> {
    let mut saves = Vec::new();
    let root = paths.ci_path("data/SAVEGAME", None);
    let Ok(entries) = std::fs::read_dir(&root) else {
        return saves;
    };

    for entry in entries.flatten() {
        let Some(slot) = slot_number(&entry.file_name().to_string_lossy()) else { continue };
        if !entry.path().is_dir() {
            continue;
        }
        let Some(dat_path) = find_save_dat(&entry.path()) else { continue };
        if let Some(save) = read_save_header(slot, &dat_path) {
            saves.push(save);
        }
    }
    saves.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    saves
}

fn slot_number(dir_name: &str) -> Option<u32> {
    let digits = dir_name.to_lowercase().strip_prefix("slot")?.to_string();
    if !digits.is_empty() && digits.bytes().all(|b| b.is_ascii_digit()) {
        digits.parse().ok()
    } else {
        None
    }
}

fn find_save_dat(dir: &Path) -> Option<PathBuf> {
    std::fs::read_dir(dir)
        .ok()?
        .flatten()
        .find(|e| e.file_name().to_string_lossy().eq_ignore_ascii_case("save.dat"))
        .map(|e| e.path())
}

fn read_save_header(slot: u32, dat_path: &Path) -> Option<SaveGame> {
    let mut file = File::open(dat_path).ok()?;
    let needed = 131 + (PREVIEW_W * PREVIEW_H) as usize;
    let mut head = Vec::new();
    file.by_ref().take(needed as u64).read_to_end(&mut head).ok()?;
    if head.len() < 131 || !head.starts_with(b"FALLOUT SAVE FILE") {
        return None;
    }

    let month = i16::from_be_bytes([head[101], head[102]]);
    let day = i16::from_be_bytes([head[103], head[104]]);
    let year = i16::from_be_bytes([head[105], head[106]]);
    let map_name = strip_extension(&cstr(&head[115..131]));
    let game_date = if (1..=12).contains(&month) {
        format!("{day} {} {year}", MONTHS[(month - 1) as usize])
    } else {
        String::new()
    };
    let thumbnail = if head.len() == needed { Some(head[131..].to_vec()) } else { None };
    let mtime = dat_path.metadata().and_then(|m| m.modified()).unwrap_or(SystemTime::UNIX_EPOCH);

    let name = cstr(&head[29..61]);
    let map_name_for_location = map_name.clone();
    Some(SaveGame {
        slot,
        path: dat_path.to_path_buf(),
        name: if name.is_empty() { "Unnamed".to_string() } else { name },
        description: cstr(&head[61..91]),
        location: friendly_location(&map_name_for_location),
        map_name,
        game_date,
        mtime,
        thumbnail,
    })
}

/// Reads the active campaign's master palette (VGA 6-bit-per-channel), scaled up to 8-bit
/// (`value * 4`, clamped), for rendering a save's raw thumbnail bytes as an actual image.
pub fn load_palette(paths: &Paths, dat_cache: &DatCache) -> Option<Vec<(u8, u8, u8)>> {
    let dat = paths.ci_path("master.dat", None);
    let pal = dat_cache.read(&dat, "color.pal")?;
    if pal.len() < 768 {
        return None;
    }
    Some(
        (0..768)
            .step_by(3)
            .map(|i| {
                let scale = |v: u8| (v as u16 * 4).min(255) as u8;
                (scale(pal[i]), scale(pal[i + 1]), scale(pal[i + 2]))
            })
            .collect(),
    )
}

fn cstr(bytes: &[u8]) -> String {
    let end = bytes.iter().position(|&b| b == 0).unwrap_or(bytes.len());
    decode_latin1(&bytes[..end]).trim().to_string()
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn strip_extension(name: &str) -> String {
    match name.rfind('.') {
        Some(idx) => name[..idx].to_string(),
        None => name.to_string(),
    }
}

/// Approximates Python's `str.title()`: capitalize the first letter of each maximal run of
/// alphabetic characters, lowercase the rest. Only reached for unrecognized map names, as a
/// last-resort display fallback.
fn title_case(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut at_word_start = true;
    for c in s.chars() {
        if c.is_alphabetic() {
            if at_word_start {
                out.extend(c.to_uppercase());
            } else {
                out.extend(c.to_lowercase());
            }
            at_word_start = false;
        } else {
            out.push(c);
            at_word_start = true;
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_field(head: &mut [u8], range: std::ops::Range<usize>, value: &str) {
        let bytes = value.as_bytes();
        head[range.start..range.start + bytes.len()].copy_from_slice(bytes);
    }

    fn build_save_bytes(
        name: &str,
        description: &str,
        (month, day, year): (i16, i16, i16),
        map_name: &str,
        with_thumbnail: bool,
    ) -> Vec<u8> {
        let mut head = vec![0u8; 131];
        write_field(&mut head, 0..18, "FALLOUT SAVE FILE");
        write_field(&mut head, 29..61, name);
        write_field(&mut head, 61..91, description);
        head[101..103].copy_from_slice(&month.to_be_bytes());
        head[103..105].copy_from_slice(&day.to_be_bytes());
        head[105..107].copy_from_slice(&year.to_be_bytes());
        write_field(&mut head, 115..131, map_name);
        if with_thumbnail {
            head.extend(vec![0x7Fu8; (PREVIEW_W * PREVIEW_H) as usize]);
        }
        head
    }

    fn write_save(dir: &Path, slot_name: &str, bytes: &[u8]) {
        let slot_dir = dir.join("data").join("SAVEGAME").join(slot_name);
        std::fs::create_dir_all(&slot_dir).unwrap();
        std::fs::write(slot_dir.join("SAVE.DAT"), bytes).unwrap();
    }

    #[test]
    fn friendly_location_matches_known_prefixes() {
        assert_eq!(friendly_location("ARVILLAGE"), "Arroyo");
        assert_eq!(friendly_location("artemple"), "Temple of Trials");
    }

    #[test]
    fn friendly_location_falls_back_to_title_case_or_unknown() {
        assert_eq!(friendly_location(""), "Unknown");
        assert_eq!(friendly_location("zzztown"), "Zzztown");
    }

    #[test]
    fn reads_a_well_formed_save_with_thumbnail() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let bytes = build_save_bytes("Hero", "Before the crash", (6, 15, 2241), "ARVILLAGE.SAV", true);
        write_save(tmp.path(), "slot1", &bytes);

        let saves = read_saves(&paths);
        assert_eq!(saves.len(), 1);
        let s = &saves[0];
        assert_eq!(s.slot, 1);
        assert_eq!(s.name, "Hero");
        assert_eq!(s.description, "Before the crash");
        assert_eq!(s.map_name, "ARVILLAGE");
        assert_eq!(s.location, "Arroyo");
        assert_eq!(s.game_date, "15 Jun 2241");
        assert_eq!(s.thumbnail.as_ref().unwrap().len(), (PREVIEW_W * PREVIEW_H) as usize);
    }

    #[test]
    fn slot_directory_name_is_case_insensitive() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let bytes = build_save_bytes("Hero", "", (1, 1, 2241), "DEN", true);
        write_save(tmp.path(), "SLOT7", &bytes);

        let saves = read_saves(&paths);
        assert_eq!(saves.len(), 1);
        assert_eq!(saves[0].slot, 7);
    }

    #[test]
    fn unnamed_save_gets_a_default_name() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let bytes = build_save_bytes("", "", (1, 1, 2241), "DEN", true);
        write_save(tmp.path(), "slot0", &bytes);

        assert_eq!(read_saves(&paths)[0].name, "Unnamed");
    }

    #[test]
    fn truncated_save_without_full_thumbnail_has_no_thumbnail() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let bytes = build_save_bytes("Hero", "", (1, 1, 2241), "DEN", false);
        write_save(tmp.path(), "slot0", &bytes);

        assert_eq!(read_saves(&paths)[0].thumbnail, None);
    }

    #[test]
    fn invalid_month_leaves_game_date_empty() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let bytes = build_save_bytes("Hero", "", (0, 1, 2241), "DEN", true);
        write_save(tmp.path(), "slot0", &bytes);

        assert_eq!(read_saves(&paths)[0].game_date, "");
    }

    #[test]
    fn non_slot_directories_and_bad_magic_are_skipped() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("data").join("SAVEGAME").join("notaslot")).unwrap();
        write_save(tmp.path(), "slot2", b"not a real save file");

        assert_eq!(read_saves(&paths), vec![]);
    }

    #[test]
    fn missing_savegame_directory_returns_no_saves() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        assert_eq!(read_saves(&paths), vec![]);
    }

    #[test]
    fn load_palette_scales_vga_values_to_8_bit() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let dat_path = tmp.path().join("master.dat");

        let mut pal = vec![0u8; 768];
        pal[0] = 63; // the usual max 6-bit VGA channel value -> 63*4 = 252, no clamping yet
        pal[1] = 70; // out of the normal 0-63 range, but still a legal byte -> clamps to 255
        pal[2] = 0;
        std::fs::write(&dat_path, build_test_dat_with_color_pal(&pal)).unwrap();

        let cache = DatCache::new();
        let palette = load_palette(&paths, &cache).unwrap();
        assert_eq!(palette[0], (252, 255, 0));
    }

    /// Minimal DAT2 archive containing a single uncompressed `color.pal` entry - mirrors the
    /// fixture builder in `dat.rs`'s own tests, kept local since it's only needed here.
    fn build_test_dat_with_color_pal(pal: &[u8]) -> Vec<u8> {
        let name = "color.pal";
        let mut archive = pal.to_vec();
        let mut tree = Vec::new();
        tree.extend_from_slice(&1u32.to_le_bytes());
        tree.extend_from_slice(&(name.len() as u32).to_le_bytes());
        tree.extend_from_slice(name.as_bytes());
        tree.push(0); // not compressed
        tree.extend_from_slice(&(pal.len() as u32).to_le_bytes()); // real size
        tree.extend_from_slice(&(pal.len() as u32).to_le_bytes()); // packed size
        tree.extend_from_slice(&0u32.to_le_bytes()); // offset
        archive.extend_from_slice(&tree);
        let dat_size = (archive.len() + 8) as u32;
        archive.extend_from_slice(&(tree.len() as u32).to_le_bytes());
        archive.extend_from_slice(&dat_size.to_le_bytes());
        archive
    }
}
