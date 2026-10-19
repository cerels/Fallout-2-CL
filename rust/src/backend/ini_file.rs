//! Line-preserving INI editing: only the lines that change are rewritten, so comments and
//! layout survive untouched. Values round-trip through latin-1 (a direct byte<->codepoint
//! mapping), so the file's exact byte content is preserved even where we don't interpret it.

use std::fs;
use std::io;
use std::path::PathBuf;

pub struct IniFile {
    path: PathBuf,
    exists: bool,
    newline: &'static str,
    lines: Vec<String>,
    dirty: bool,
}

impl IniFile {
    pub fn open(path: PathBuf) -> io::Result<Self> {
        let exists = path.is_file();
        let raw = if exists { decode_latin1(&fs::read(&path)?) } else { String::new() };
        let newline = if raw.contains("\r\n") { "\r\n" } else { "\n" };
        Ok(IniFile { path, exists, newline, lines: split_lines(&raw), dirty: false })
    }

    pub fn exists(&self) -> bool {
        self.exists
    }

    /// `[section]` name if `line` is a section header, lowercased; `None` otherwise.
    fn header(line: &str) -> Option<String> {
        let t = line.trim();
        let rest = t.strip_prefix('[')?;
        let end = rest.find(']')?;
        Some(rest[..end].trim().to_lowercase())
    }

    /// (section header line, key line, section-end line) as indices into `lines`. The
    /// section end is the index of the next section header, or `lines.len()` at EOF.
    fn find(&self, section: &str, key: &str) -> (Option<usize>, Option<usize>, usize) {
        let (section, key) = (section.to_lowercase(), key.to_lowercase());
        let mut section_start = None;
        let mut key_line = None;
        let mut section_end = self.lines.len();

        for (i, line) in self.lines.iter().enumerate() {
            if let Some(name) = Self::header(line) {
                if section_start.is_some() {
                    section_end = i;
                    break;
                }
                if name == section {
                    section_start = Some(i);
                }
                continue;
            }
            let t = line.trim();
            let is_comment = t.starts_with(';') || t.starts_with('#');
            if section_start.is_some() && key_line.is_none() && !is_comment && t.contains('=') {
                let name = t.split('=').next().unwrap().trim().to_lowercase();
                if name == key {
                    key_line = Some(i);
                }
            }
        }
        (section_start, key_line, section_end)
    }

    pub fn get(&self, section: &str, key: &str) -> Option<String> {
        let (_, key_line, _) = self.find(section, key);
        let (_, value) = self.lines[key_line?].split_once('=')?;
        Some(value.trim().to_string())
    }

    pub fn set(&mut self, section: &str, key: &str, value: &str) {
        let (section_start, key_line, section_end) = self.find(section, key);

        if let Some(i) = key_line {
            let new_line = format!("{}{value}", Self::key_prefix(&self.lines[i]));
            if self.lines[i] != new_line {
                self.lines[i] = new_line;
                self.dirty = true;
            }
            return;
        }

        match section_start {
            None => {
                if self.lines.last().is_some_and(|l| !l.trim().is_empty()) {
                    self.lines.push(String::new());
                }
                self.lines.push(format!("[{section}]"));
                self.lines.push(format!("{key}={value}"));
            }
            Some(start) => {
                let mut insert_at = section_end;
                while insert_at > start + 1 && self.lines[insert_at - 1].trim().is_empty() {
                    insert_at -= 1;
                }
                self.lines.insert(insert_at, format!("{key}={value}"));
            }
        }
        self.dirty = true;
    }

    /// Everything up to and including the `=` plus any whitespace right after it - the part
    /// of a `key = value` line that `set()` keeps untouched.
    fn key_prefix(line: &str) -> &str {
        let eq = line.find('=').expect("only called on lines find() matched, which contain '='");
        let after_eq = eq + 1;
        let value_start = line[after_eq..]
            .find(|c: char| c != ' ' && c != '\t')
            .map_or(line.len(), |offset| after_eq + offset);
        &line[..value_start]
    }

    pub fn save(&mut self) -> io::Result<()> {
        if !self.dirty {
            return Ok(());
        }
        if let Some(parent) = self.path.parent() {
            fs::create_dir_all(parent)?;
        }
        let tmp_name = format!("{}.tmp", self.path.file_name().unwrap().to_string_lossy());
        let tmp = self.path.with_file_name(tmp_name);

        let mut body = self.lines.join(self.newline);
        body.push_str(self.newline);
        fs::write(&tmp, encode_latin1(&body))?;
        fs::rename(&tmp, &self.path)?;

        self.exists = true;
        self.dirty = false;
        Ok(())
    }
}

/// Matches Python's `str.splitlines()` for the line endings that actually occur in these
/// files (`\n` and `\r\n`): no trailing empty element when `raw` ends with a newline, and an
/// empty string splits to no lines at all.
pub(crate) fn split_lines(raw: &str) -> Vec<String> {
    if raw.is_empty() {
        return Vec::new();
    }
    let mut lines: Vec<String> =
        raw.split('\n').map(|l| l.strip_suffix('\r').unwrap_or(l).to_string()).collect();
    if raw.ends_with('\n') {
        lines.pop();
    }
    lines
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// Inverse of `decode_latin1`. Only ever called on text this module itself produced (section
/// names, keys and values set via `IniFile::set`, or bytes that round-tripped through
/// `decode_latin1`), so every char is guaranteed to be `<= 0xFF`.
fn encode_latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ini_with(contents: &str) -> (tempfile::TempDir, IniFile) {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("test.ini");
        fs::write(&path, contents).unwrap();
        let ini = IniFile::open(path).unwrap();
        (tmp, ini)
    }

    #[test]
    fn get_is_case_insensitive_on_section_and_key() {
        let (_tmp, ini) = ini_with("[Display]\nWindowed=1\n");
        assert_eq!(ini.get("display", "windowed"), Some("1".to_string()));
        assert_eq!(ini.get("DISPLAY", "WINDOWED"), Some("1".to_string()));
    }

    #[test]
    fn get_missing_key_is_none() {
        let (_tmp, ini) = ini_with("[Display]\nWindowed=1\n");
        assert_eq!(ini.get("Display", "Fullscreen"), None);
        assert_eq!(ini.get("Nope", "Windowed"), None);
    }

    #[test]
    fn set_existing_key_preserves_everything_else() {
        let (_tmp, mut ini) = ini_with("[Display]\nWindowed = 1   ; keep me on\nScale=2\n");
        ini.set("Display", "Windowed", "0");
        assert_eq!(ini.get("Display", "Windowed"), Some("0".to_string()));
        // The comment and spacing around `=` on unrelated/changed lines are untouched.
        assert_eq!(ini.lines[1], "Windowed = 0");
        assert_eq!(ini.lines[2], "Scale=2");
    }

    #[test]
    fn set_unchanged_value_does_not_mark_dirty() {
        let (_tmp, mut ini) = ini_with("[Display]\nWindowed=1\n");
        ini.set("Display", "Windowed", "1");
        assert!(!ini.dirty);
    }

    #[test]
    fn set_new_key_in_existing_section_inserts_before_trailing_blank_lines() {
        let (_tmp, mut ini) = ini_with("[Display]\nWindowed=1\n\n[Audio]\nVolume=5\n");
        ini.set("Display", "Scale", "2");
        assert_eq!(
            ini.lines,
            vec!["[Display]", "Windowed=1", "Scale=2", "", "[Audio]", "Volume=5"]
        );
    }

    #[test]
    fn set_new_section_appends_with_a_blank_separator() {
        let (_tmp, mut ini) = ini_with("[Display]\nWindowed=1\n");
        ini.set("Audio", "Volume", "5");
        assert_eq!(ini.lines, vec!["[Display]", "Windowed=1", "", "[Audio]", "Volume=5"]);
    }

    #[test]
    fn set_on_missing_file_creates_it_from_scratch() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("new.ini");
        let mut ini = IniFile::open(path.clone()).unwrap();
        assert!(!ini.exists());

        ini.set("Display", "Windowed", "1");
        ini.save().unwrap();

        assert!(ini.exists());
        let saved = fs::read_to_string(&path).unwrap();
        // A brand-new file has no newlines to sniff yet, so this defaults to "\n", matching
        // Python (`"\r\n" in ""` is also false there).
        assert_eq!(saved, "[Display]\nWindowed=1\n");
    }

    #[test]
    fn save_only_rewrites_changed_lines_and_keeps_newline_style() {
        let (tmp, mut ini) = ini_with("[Display]\nWindowed=1\nScale=2\n");
        ini.set("Display", "Windowed", "0");
        ini.save().unwrap();

        let saved = fs::read_to_string(tmp.path().join("test.ini")).unwrap();
        assert_eq!(saved, "[Display]\nWindowed=0\nScale=2\n");
    }

    #[test]
    fn save_is_a_noop_when_not_dirty() {
        let (tmp, mut ini) = ini_with("[Display]\nWindowed=1\n");
        let path = tmp.path().join("test.ini");
        let before = fs::metadata(&path).unwrap().modified().unwrap();
        ini.save().unwrap();
        let after = fs::metadata(&path).unwrap().modified().unwrap();
        assert_eq!(before, after);
    }

    #[test]
    fn preserves_crlf_line_endings() {
        let (tmp, mut ini) = ini_with("[Display]\r\nWindowed=1\r\n");
        ini.set("Display", "Windowed", "0");
        ini.save().unwrap();
        let saved = fs::read_to_string(tmp.path().join("test.ini")).unwrap();
        assert_eq!(saved, "[Display]\r\nWindowed=0\r\n");
    }
}
