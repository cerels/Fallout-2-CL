//! Reading Fallout 2 DAT2 archives, and a minimal read-only INI parser for their contents.

use std::cell::RefCell;
use std::collections::HashMap;
use std::fs::File;
use std::io::{self, Read, Seek, SeekFrom};
use std::path::{Path, PathBuf};

use flate2::read::ZlibDecoder;

#[derive(Clone, Copy)]
struct DatEntry {
    compressed: bool,
    packed_size: u32,
    offset: u32,
}

/// Caches each DAT's directory listing after the first read, mirroring Python's
/// module-level `_dat_dirs` cache - held explicitly here rather than as a hidden global; see
/// `paths.rs`'s `Paths` for the same reasoning.
#[derive(Default)]
pub struct DatCache {
    dirs: RefCell<HashMap<PathBuf, HashMap<String, DatEntry>>>,
}

impl DatCache {
    pub fn new() -> Self {
        Self::default()
    }

    /// Read one file from a Fallout 2 DAT2 archive. `None` on any I/O/format error or if
    /// `name` isn't in the archive - callers treat "couldn't read it" and "not present" the
    /// same way, as Python does.
    pub fn read(&self, dat: &Path, name: &str) -> Option<Vec<u8>> {
        self.try_read(dat, name).ok().flatten()
    }

    fn try_read(&self, dat: &Path, name: &str) -> io::Result<Option<Vec<u8>>> {
        if !self.dirs.borrow().contains_key(dat) {
            let entries = read_directory(dat)?;
            self.dirs.borrow_mut().insert(dat.to_path_buf(), entries);
        }

        let entry = match self.dirs.borrow()[dat].get(&name.to_lowercase()) {
            Some(&entry) => entry,
            None => return Ok(None),
        };

        let mut file = File::open(dat)?;
        file.seek(SeekFrom::Start(entry.offset as u64))?;
        let mut raw = vec![0u8; entry.packed_size as usize];
        file.read_exact(&mut raw)?;

        if entry.compressed {
            let mut out = Vec::new();
            ZlibDecoder::new(&raw[..]).read_to_end(&mut out)?;
            Ok(Some(out))
        } else {
            Ok(Some(raw))
        }
    }
}

fn read_directory(dat: &Path) -> io::Result<HashMap<String, DatEntry>> {
    let invalid = |msg: &str| io::Error::new(io::ErrorKind::InvalidData, msg.to_string());

    let mut file = File::open(dat)?;
    file.seek(SeekFrom::End(-8))?;
    let mut footer = [0u8; 8];
    file.read_exact(&mut footer)?;
    let tree_size = u32::from_le_bytes(footer[0..4].try_into().unwrap());
    let dat_size = u32::from_le_bytes(footer[4..8].try_into().unwrap());

    let tree_offset = (dat_size as u64)
        .checked_sub(8 + tree_size as u64)
        .ok_or_else(|| invalid("DAT tree size exceeds the file size"))?;
    file.seek(SeekFrom::Start(tree_offset))?;
    let mut tree = vec![0u8; tree_size as usize];
    file.read_exact(&mut tree)?;

    let mut entries = HashMap::new();
    let count = read_u32(&tree, 0)?;
    let mut o = 4usize;
    for _ in 0..count {
        let name_len = read_u32(&tree, o)? as usize;
        o += 4;
        let name_bytes =
            tree.get(o..o + name_len).ok_or_else(|| invalid("DAT entry name runs past the tree"))?;
        let name: String = name_bytes.iter().map(|&b| b as char).collect::<String>().to_lowercase();
        o += name_len;

        let compressed = *tree.get(o).ok_or_else(|| invalid("truncated DAT entry"))? != 0;
        let packed_size = read_u32(&tree, o + 5)?;
        let offset = read_u32(&tree, o + 9)?;
        o += 13; // flags(1) + real_size(4, unused) + packed_size(4) + offset(4)

        entries.insert(name, DatEntry { compressed, packed_size, offset });
    }
    Ok(entries)
}

fn read_u32(buf: &[u8], offset: usize) -> io::Result<u32> {
    buf.get(offset..offset + 4)
        .and_then(|s| s.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "truncated DAT directory"))
}

/// Minimal read-only INI parser for DAT-embedded config text: `(section, key)` -> value,
/// both lowercased.
pub fn parse_ini_text(text: &str) -> HashMap<(String, String), String> {
    let mut values = HashMap::new();
    let mut section = String::new();
    for line in text.lines() {
        let t = line.trim();
        if let Some(rest) = t.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                section = rest[..end].trim().to_lowercase();
                continue;
            }
        }
        if t.contains('=') && !t.starts_with(';') && !t.starts_with('#') {
            if let Some((k, v)) = t.split_once('=') {
                values.insert((section.clone(), k.trim().to_lowercase()), v.trim().to_string());
            }
        }
    }
    values
}

#[cfg(test)]
mod tests {
    use super::*;
    use flate2::write::ZlibEncoder;
    use flate2::Compression;
    use std::io::Write;

    /// Builds a minimal, valid DAT2 archive in memory: one uncompressed and one
    /// zlib-compressed entry, laid out the way Fallout 2's DAT2 format expects
    /// (`data..., tree, tree_size:u32, dat_size:u32`, all little-endian).
    fn build_dat(entries: &[(&str, &[u8], bool)]) -> Vec<u8> {
        let mut file_data = Vec::new();
        let mut tree = Vec::new();
        let mut descriptors = Vec::new();

        for &(name, content, compress) in entries {
            let offset = file_data.len() as u32;
            let stored = if compress {
                let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
                encoder.write_all(content).unwrap();
                encoder.finish().unwrap()
            } else {
                content.to_vec()
            };
            let packed_size = stored.len() as u32;
            file_data.extend_from_slice(&stored);
            descriptors.push((name, content.len() as u32, packed_size, offset, compress));
        }

        tree.extend_from_slice(&(descriptors.len() as u32).to_le_bytes());
        for (name, real_size, packed_size, offset, compress) in descriptors {
            tree.extend_from_slice(&(name.len() as u32).to_le_bytes());
            tree.extend_from_slice(name.as_bytes());
            tree.push(if compress { 1 } else { 0 });
            tree.extend_from_slice(&real_size.to_le_bytes());
            tree.extend_from_slice(&packed_size.to_le_bytes());
            tree.extend_from_slice(&offset.to_le_bytes());
        }

        let mut archive = file_data;
        archive.extend_from_slice(&tree);
        let dat_size = (archive.len() + 8) as u32;
        archive.extend_from_slice(&(tree.len() as u32).to_le_bytes());
        archive.extend_from_slice(&dat_size.to_le_bytes());
        archive
    }

    fn write_dat(entries: &[(&str, &[u8], bool)]) -> (tempfile::TempDir, PathBuf) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("master.dat");
        std::fs::write(&path, build_dat(entries)).unwrap();
        (tmp, path)
    }

    #[test]
    fn reads_an_uncompressed_entry() {
        let (_tmp, dat) = write_dat(&[("readme.txt", b"hello world", false)]);
        let cache = DatCache::new();
        assert_eq!(cache.read(&dat, "readme.txt"), Some(b"hello world".to_vec()));
    }

    #[test]
    fn reads_a_compressed_entry() {
        let (_tmp, dat) = write_dat(&[("config.cfg", b"[display]\nscale=2\n", true)]);
        let cache = DatCache::new();
        assert_eq!(cache.read(&dat, "config.cfg"), Some(b"[display]\nscale=2\n".to_vec()));
    }

    #[test]
    fn entry_lookup_is_case_insensitive() {
        let (_tmp, dat) = write_dat(&[("Data\\Config.cfg", b"x", false)]);
        let cache = DatCache::new();
        assert_eq!(cache.read(&dat, "data\\config.cfg"), Some(b"x".to_vec()));
    }

    #[test]
    fn missing_entry_is_none() {
        let (_tmp, dat) = write_dat(&[("readme.txt", b"hello", false)]);
        let cache = DatCache::new();
        assert_eq!(cache.read(&dat, "nope.txt"), None);
    }

    #[test]
    fn directory_is_cached_across_reads() {
        let (_tmp, dat) = write_dat(&[("a.txt", b"first", false), ("b.txt", b"second", false)]);
        let cache = DatCache::new();
        assert_eq!(cache.read(&dat, "a.txt"), Some(b"first".to_vec()));
        assert_eq!(cache.read(&dat, "b.txt"), Some(b"second".to_vec()));
        assert_eq!(cache.dirs.borrow().len(), 1);
    }

    #[test]
    fn missing_file_does_not_panic() {
        let cache = DatCache::new();
        assert_eq!(cache.read(Path::new("/does/not/exist.dat"), "x"), None);
    }

    #[test]
    fn parse_ini_text_reads_sections_case_insensitively() {
        let values = parse_ini_text("[Display]\nScale = 2\n; comment\n[Audio]\nVolume=5\n");
        assert_eq!(values.get(&("display".into(), "scale".into())), Some(&"2".to_string()));
        assert_eq!(values.get(&("audio".into(), "volume".into())), Some(&"5".to_string()));
    }

    #[test]
    fn parse_ini_text_ignores_comments_and_keys_before_any_section() {
        let values = parse_ini_text("loose=1\n# c\n[Section]\nkey=value\n");
        assert_eq!(values.len(), 2);
        assert_eq!(values.get(&("".into(), "loose".into())), Some(&"1".to_string()));
        assert_eq!(values.get(&("section".into(), "key".into())), Some(&"value".to_string()));
    }
}
