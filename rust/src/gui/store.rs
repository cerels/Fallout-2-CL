//! Store: the live settings values every settings page reads from and writes through.
//! Ported from the Python `gui/store.py` - see that file's history for the design this
//! mirrors. Unlike the Qt build, nothing here needs to *emit* change notifications: egui is
//! immediate-mode, so every page just re-reads `Store::value` fresh each frame.

use std::collections::HashMap;

use crate::backend::dat::{parse_ini_text, DatCache};
use crate::backend::ini_file::IniFile;
use crate::backend::paths::SharedPaths;
use crate::backend::schema::{Catalog, Setting};

pub struct Store {
    pub catalog: Catalog,
    files: HashMap<String, IniFile>,
    settings: HashMap<String, Setting>,
    initial: HashMap<String, String>,
    current: HashMap<String, String>,
    dat_defaults: HashMap<String, HashMap<(String, String), String>>,
    backup_stamp: Option<String>,
}

impl Store {
    pub fn new(paths: &SharedPaths, catalog: Catalog) -> Self {
        let mut files = HashMap::new();
        for (fid, rel) in &catalog.files {
            if let Ok(f) = IniFile::open(paths.ci_path(rel, None)) {
                files.insert(fid.clone(), f);
            }
        }

        let dat_cache = DatCache::new();
        let mut dat_defaults = HashMap::new();
        for (fid, (dat, inner)) in &catalog.defaults_from {
            let dat_path = paths.ci_path(dat, None);
            if let Some(bytes) = dat_cache.read(&dat_path, inner) {
                dat_defaults.insert(fid.clone(), parse_ini_text(&decode_latin1(&bytes)));
            }
        }

        let mut store = Store {
            catalog,
            files,
            settings: HashMap::new(),
            initial: HashMap::new(),
            current: HashMap::new(),
            dat_defaults,
            backup_stamp: None,
        };
        store.register_all();
        store
    }

    /// Every setting in every page is registered up front (unlike Python, which registered
    /// lazily as each `SettingRow` widget was built) - there's no widget tree here to drive
    /// that, and registering everything once is simpler to reason about.
    fn register_all(&mut self) {
        let settings: Vec<Setting> = self
            .catalog
            .pages
            .iter()
            .flat_map(|p| p.groups.iter())
            .flat_map(|(_, settings)| settings.iter().cloned())
            .collect();
        for s in settings {
            self.register(s);
        }
    }

    fn register(&mut self, s: Setting) {
        let value = self.read(&s).unwrap_or_else(|| self.default_of(&s).unwrap_or_default());
        let id = s.id();
        self.initial.insert(id.clone(), value.clone());
        self.current.insert(id.clone(), value);
        self.settings.insert(id, s);
    }

    fn read(&self, s: &Setting) -> Option<String> {
        let f = self.files.get(&s.file)?;
        if s.kind == "resolution" {
            let (kx, ky) = s.key.split_once('|')?;
            let (x, y) = (f.get(&s.section, kx), f.get(&s.section, ky));
            return match (x, y) {
                (Some(x), Some(y)) => Some(format!("{x}x{y}")),
                _ => None,
            };
        }
        s.normalize(f.get(&s.section, &s.key).as_deref())
    }

    pub fn default_of(&self, s: &Setting) -> Option<String> {
        if let Some(d) =
            self.dat_defaults.get(&s.file).and_then(|m| m.get(&(s.section.to_lowercase(), s.key.to_lowercase())))
        {
            return s.normalize(Some(d.as_str()));
        }
        s.normalize(s.default.as_deref())
    }

    pub fn value(&self, sid: &str) -> &str {
        self.current.get(sid).map(String::as_str).unwrap_or("")
    }

    pub fn set(&mut self, sid: &str, value: &str) {
        let Some(s) = self.settings.get(sid) else { return };
        let value = s.normalize(Some(value)).unwrap_or_default();
        if self.current.get(sid).map(String::as_str) == Some(value.as_str()) {
            return;
        }
        self.current.insert(sid.to_string(), value);
    }

    pub fn file_ok(&self, s: &Setting) -> bool {
        match self.files.get(&s.file) {
            Some(f) => f.exists() || s.file == "launcher",
            None => false,
        }
    }

    fn dirty_ids(&self) -> Vec<String> {
        self.current
            .iter()
            .filter(|(sid, v)| self.initial.get(*sid).map(String::as_str) != Some(v.as_str()))
            .map(|(sid, _)| sid.clone())
            .collect()
    }

    pub fn dirty_count(&self) -> usize {
        self.dirty_ids().len()
    }

    pub fn revert(&mut self) {
        for sid in self.dirty_ids() {
            let initial = self.initial[&sid].clone();
            self.current.insert(sid, initial);
        }
    }

    /// Write every pending change to disk, backing up each touched file's previous content
    /// first (once per save, under `backup_dir()/<timestamp>/...`). Returns an error message
    /// on failure, matching Python's `Store.save`.
    pub fn save(&mut self, paths: &SharedPaths) -> Result<(), String> {
        let dirty = self.dirty_ids();
        if dirty.is_empty() {
            return Ok(());
        }
        let mut touched = std::collections::HashSet::new();
        for sid in &dirty {
            let s = self.settings[sid].clone();
            let value = self.current[sid].clone();
            let Some(f) = self.files.get_mut(&s.file) else { continue };
            if s.kind == "resolution" {
                if let Some((kx, ky)) = s.key.split_once('|') {
                    if let Some((x, y)) = value.to_lowercase().split_once('x') {
                        f.set(&s.section, kx, x);
                        f.set(&s.section, ky, y);
                    }
                }
            } else {
                f.set(&s.section, &s.key, &value);
            }
            touched.insert(s.file.clone());
        }

        let stamp = self.backup_stamp.clone().unwrap_or_else(timestamp_now);
        let mut stamp_used = false;
        for fid in &touched {
            let Some(f) = self.files.get(fid) else { continue };
            if f.exists() && fid.as_str() != "launcher" {
                if let Some(rel) = self.catalog.files.get(fid) {
                    let dest = paths.backup_dir().join(&stamp).join(rel);
                    if !dest.exists() {
                        if let Some(parent) = dest.parent() {
                            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                        }
                        // Re-resolved rather than read off `f` (`IniFile` doesn't expose its
                        // own path) - same case-insensitive lookup `Store::new` used to open
                        // it in the first place, so it names the same file.
                        std::fs::copy(paths.ci_path(rel, None), &dest).map_err(|e| e.to_string())?;
                        stamp_used = true;
                    }
                }
            }
        }
        if stamp_used {
            self.backup_stamp = Some(stamp);
        }

        for fid in &touched {
            if let Some(f) = self.files.get_mut(fid) {
                f.save().map_err(|e| e.to_string())?;
            }
        }
        for sid in dirty {
            let value = self.current[&sid].clone();
            self.initial.insert(sid, value);
        }
        Ok(())
    }
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

/// `"%Y%m%d-%H%M%S"` in the local sense Python's `datetime.now()` gives, but computed by
/// hand from `SystemTime` - pulling in a date/time crate just to label a backup folder isn't
/// worth a new dependency (see `docs/adr/0005-boring-rust.md`). Civil-date math is the
/// well-known "days since epoch" algorithm (Howard Hinnant's `civil_from_days`, public
/// domain); UTC rather than local time, which only matters for the folder's label, not for
/// anything this launcher reads back.
pub(crate) fn timestamp_now() -> String {
    let secs = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    let days = (secs / 86400) as i64;
    let time_of_day = secs % 86400;
    let (hh, mm, ss) = (time_of_day / 3600, (time_of_day % 3600) / 60, time_of_day % 60);
    let (y, m, d) = civil_from_days(days);
    format!("{y:04}{m:02}{d:02}-{hh:02}{mm:02}{ss:02}")
}

/// Days-since-1970-01-01 -> (year, month, day). See
/// <https://howardhinnant.github.io/date_algorithms.html#civil_from_days>.
fn civil_from_days(z: i64) -> (i64, u32, u32) {
    let z = z + 719468;
    let era = if z >= 0 { z } else { z - 146096 } / 146097;
    let doe = (z - era * 146097) as u64;
    let yoe = (doe - doe / 1460 + doe / 36524 - doe / 146096) / 365;
    let y = yoe as i64 + era * 400;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (if m <= 2 { y + 1 } else { y }, m, d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn civil_from_days_matches_known_dates() {
        assert_eq!(civil_from_days(0), (1970, 1, 1));
        assert_eq!(civil_from_days(19889), (2024, 6, 15));
    }
}
