//! Settings are described by TOML schema files, not by code:
//! - the core schemas (engine settings only) and the schemas for popular mods are compiled
//!   into this binary (see `BUILTIN_SCHEMAS`/`BUILTIN_MOD_SCHEMAS` below) - there's no
//!   PyInstaller-style resource directory to extract at runtime, since the binary already
//!   contains them;
//! - `mods/*.schema.toml` are schemas a mod ships itself, read from the active campaign's
//!   actual `mods/` folder at runtime.
//!
//! Mod schemas only show up when the files they configure are installed, and settings
//! marked `requires_engine` only when the fallout2-ce binary has them. Any `mods/*.ini` that
//! no schema covers gets a tab generated from the file's own comments (see
//! `generate_ini_groups`).
//!
//! `scale_hint`/`HINTS` and `check_schemas` from the Python version aren't ported yet: they
//! need the GUI's settings store, which doesn't exist until the GUI port starts (see
//! `docs/adr/0004-rust-rewrite.md` - backend first, GUI last).

use std::collections::{HashMap, HashSet};
use std::io;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

use regex::Regex;

use super::paths::Paths;

const SETTING_TYPES: &[&str] =
    &["bool", "choice", "int", "float", "volume", "key", "resolution", "text"];

#[derive(Debug, Clone, Default, PartialEq)]
pub struct EnabledIf {
    /// The id (`file:section:key`) of the other setting this one depends on.
    pub setting: Option<String>,
    pub equals: Option<Vec<String>>,
    pub not: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Setting {
    pub file: String,
    pub section: String,
    pub key: String,
    pub label: String,
    pub kind: String,
    pub default: Option<String>,
    pub help: String,
    pub note: String,
    pub options: Vec<(String, String)>,
    pub minimum: f64,
    pub maximum: f64,
    pub step: f64,
    pub suffix: String,
    pub slider: bool,
    pub combo_keys: bool,
    pub enabled_if: Option<EnabledIf>,
    pub hint: Option<String>,
    pub decimals: i32,
    /// Engine setting name that must exist in the fallout2-ce binary (for settings newer
    /// than some CE releases).
    pub requires_engine: Option<String>,
    /// Option value -> engine setting name it needs.
    pub option_requires: HashMap<String, String>,
    /// From a mod schema: hidden when its file isn't installed.
    pub optional: bool,
}

impl Setting {
    fn basic(file: &str, section: &str, key: &str, label: &str, kind: &str) -> Self {
        Setting {
            file: file.to_string(),
            section: section.to_string(),
            key: key.to_string(),
            label: label.to_string(),
            kind: kind.to_string(),
            default: None,
            help: String::new(),
            note: String::new(),
            options: Vec::new(),
            minimum: 0.0,
            maximum: 100.0,
            step: 1.0,
            suffix: String::new(),
            slider: false,
            combo_keys: false,
            enabled_if: None,
            hint: None,
            decimals: 2,
            requires_engine: None,
            option_requires: HashMap::new(),
            optional: false,
        }
    }

    pub fn id(&self) -> String {
        format!("{}:{}:{}", self.file, self.section, self.key)
    }

    pub fn normalize(&self, value: Option<&str>) -> Option<String> {
        let v = value?.trim().to_string();
        match self.kind.as_str() {
            "bool" => {
                let lower = v.to_lowercase();
                let truthy = matches!(lower.as_str(), "1" | "true" | "yes" | "on")
                    || (is_unsigned_digits(&v) && v.parse::<i64>().is_ok_and(|n| n != 0));
                Some(if truthy { "1".to_string() } else { "0".to_string() })
            }
            "int" | "volume" | "choice" if is_signed_int(&v) => {
                Some(v.parse::<i64>().map(|n| n.to_string()).unwrap_or(v))
            }
            "float" => Some(v.parse::<f64>().map(|f| format!("{f:.6}")).unwrap_or(v)),
            _ => Some(v),
        }
    }

    pub fn enabled_by(&self, value: &str) -> bool {
        let Some(cond) = &self.enabled_if else { return true };
        if let Some(equals) = &cond.equals {
            equals.iter().any(|v| v == value)
        } else if let Some(not) = &cond.not {
            !not.iter().any(|v| v == value)
        } else {
            true
        }
    }
}

fn is_unsigned_digits(s: &str) -> bool {
    !s.is_empty() && s.bytes().all(|b| b.is_ascii_digit())
}

fn is_signed_int(s: &str) -> bool {
    let t = s.strip_prefix('-').unwrap_or(s);
    !t.is_empty() && t.bytes().all(|b| b.is_ascii_digit())
}

fn is_float_literal(s: &str) -> bool {
    let t = s.strip_prefix('-').unwrap_or(s);
    match t.split_once('.') {
        Some((int_part, frac_part)) => {
            !int_part.is_empty()
                && !frac_part.is_empty()
                && int_part.bytes().all(|b| b.is_ascii_digit())
                && frac_part.bytes().all(|b| b.is_ascii_digit())
        }
        None => false,
    }
}

#[derive(Debug, Clone, Default, PartialEq)]
pub struct Page {
    pub name: String,
    pub groups: Vec<(String, Vec<Setting>)>,
    pub intro: String,
    pub generated: bool,
}

impl Page {
    fn new(name: &str) -> Self {
        Page { name: name.to_string(), ..Default::default() }
    }

    fn group(&mut self, name: &str) -> &mut Vec<Setting> {
        if let Some(pos) = self.groups.iter().position(|(n, _)| n == name) {
            &mut self.groups[pos].1
        } else {
            self.groups.push((name.to_string(), Vec::new()));
            &mut self.groups.last_mut().unwrap().1
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct Catalog {
    pub files: HashMap<String, String>,
    pub defaults_from: HashMap<String, (String, String)>,
    pub pages: Vec<Page>,
    pub errors: Vec<String>,
    /// Files that are mod data rather than player settings; never given a generated tab.
    pub not_settings: Vec<String>,
    /// Mods page descriptions, by lowercase .dat name.
    pub mod_info: HashMap<String, String>,
    /// Installed mods whose title is shown next to the engine name.
    pub headlines: Vec<String>,
    /// Human-readable notes about settings left out (missing files, engine features).
    pub skipped: Vec<String>,
}

impl Catalog {
    fn page(&mut self, name: &str) -> &mut Page {
        if let Some(pos) = self.pages.iter().position(|p| p.name == name) {
            &mut self.pages[pos]
        } else {
            self.pages.push(Page::new(name));
            self.pages.last_mut().unwrap()
        }
    }
}

/// Caches the active campaign's engine binary bytes, mirroring Python's module-level
/// `_engine_binary_cache` - held explicitly rather than as a hidden global; see `paths.rs`'s
/// `Paths` for the same reasoning.
#[derive(Default)]
struct EngineCache {
    binaries: std::cell::RefCell<HashMap<PathBuf, Vec<u8>>>,
}

impl EngineCache {
    fn new() -> Self {
        Self::default()
    }

    /// True if the active campaign's engine binary knows the setting `name`.
    fn supports(&self, paths: &Paths, name: &str) -> bool {
        let path = paths.game_bin();
        if !self.binaries.borrow().contains_key(&path) {
            let bytes = std::fs::read(&path).unwrap_or_default();
            self.binaries.borrow_mut().insert(path.clone(), bytes);
        }
        let binaries = self.binaries.borrow();
        let binary = &binaries[&path];
        let needle = format!("\0{name}\0");
        binary.windows(needle.len()).any(|w| w == needle.as_bytes())
    }
}

/// Core schemas: Fallout 2 CE engine settings only. "Core" because they sit directly under
/// `schemas/`, not `schemas/mods/` - see `is_core` below for what that controls.
const BUILTIN_SCHEMAS: &[(&str, &str)] = &[
    ("00-files.toml", include_str!("../../../schemas/00-files.toml")),
    ("01-display.toml", include_str!("../../../schemas/01-display.toml")),
    ("02-interface.toml", include_str!("../../../schemas/02-interface.toml")),
    ("03-gameplay.toml", include_str!("../../../schemas/03-gameplay.toml")),
    ("04-combat-balance.toml", include_str!("../../../schemas/04-combat-balance.toml")),
    ("05-companions-highlights.toml", include_str!("../../../schemas/05-companions-highlights.toml")),
    ("06-audio.toml", include_str!("../../../schemas/06-audio.toml")),
    ("08-launcher.toml", include_str!("../../../schemas/08-launcher.toml")),
    ("catalog.toml", CATALOG_TOML),
];

/// Also the mod *download* catalog ([[entry]]s with a github/raw source) - see
/// `mod_catalog.rs`, which re-parses this same embedded string for that unrelated purpose.
/// It contributes no settings here: it has no `[files]`/`[[group]]`, so this file's pass
/// through `load_catalog` below is a harmless no-op.
pub(crate) const CATALOG_TOML: &str = include_str!("../../../schemas/catalog.toml");

/// Reference schemas for popular mods, shipped inside this binary so they work even before
/// the mod itself (and any `mods/<name>.schema.toml` it carries) is installed.
const BUILTIN_MOD_SCHEMAS: &[(&str, &str)] = &[
    ("mods/fo2tweaks.toml", include_str!("../../../schemas/mods/fo2tweaks.toml")),
    ("mods/npc-armor.toml", include_str!("../../../schemas/mods/npc-armor.toml")),
    ("mods/party-orders.toml", include_str!("../../../schemas/mods/party-orders.toml")),
    ("mods/restoration-project.toml", include_str!("../../../schemas/mods/restoration-project.toml")),
    ("mods/sfall-mods.toml", include_str!("../../../schemas/mods/sfall-mods.toml")),
    ("mods/unofficial-patch.toml", include_str!("../../../schemas/mods/unofficial-patch.toml")),
];

/// (display name, is_core, file content or the error reading it) for every schema to load:
/// the builtins above, plus whatever `*.schema.toml` files are actually sitting in the
/// active campaign's `mods/` folder right now.
fn schema_sources(paths: &Paths) -> Vec<(String, bool, io::Result<String>)> {
    let mut sources = Vec::new();
    for &(name, content) in BUILTIN_SCHEMAS {
        sources.push((name.to_string(), true, Ok(content.to_string())));
    }
    for &(name, content) in BUILTIN_MOD_SCHEMAS {
        sources.push((name.to_string(), false, Ok(content.to_string())));
    }

    let mods_dir = paths.ci_path("mods", None);
    if mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            let mut extra: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name()
                        .map(|n| n.to_string_lossy().to_lowercase().ends_with(".schema.toml"))
                        .unwrap_or(false)
                })
                .collect();
            extra.sort();
            for path in extra {
                let name = path.file_name().unwrap().to_string_lossy().to_string();
                sources.push((name, false, std::fs::read_to_string(&path)));
            }
        }
    }
    sources
}

fn value_to_string(v: &toml::Value) -> String {
    match v {
        toml::Value::String(s) => s.clone(),
        toml::Value::Integer(i) => i.to_string(),
        toml::Value::Float(f) => f.to_string(),
        toml::Value::Boolean(b) => b.to_string(),
        other => other.to_string(),
    }
}

fn as_number(v: &toml::Value) -> Option<f64> {
    match v {
        toml::Value::Integer(i) => Some(*i as f64),
        toml::Value::Float(f) => Some(*f),
        _ => None,
    }
}

fn truthy(v: &toml::Value) -> bool {
    match v {
        toml::Value::Boolean(b) => *b,
        toml::Value::String(s) => !s.is_empty(),
        toml::Value::Integer(i) => *i != 0,
        toml::Value::Float(f) => *f != 0.0,
        toml::Value::Array(a) => !a.is_empty(),
        toml::Value::Table(t) => !t.is_empty(),
        toml::Value::Datetime(_) => true,
    }
}

fn parse_enabled_if(v: &toml::Value) -> EnabledIf {
    let string_list = |key: &str| -> Option<Vec<String>> {
        v.get(key).and_then(|a| a.as_array()).map(|arr| arr.iter().map(value_to_string).collect())
    };
    EnabledIf {
        setting: v.get("setting").and_then(|s| s.as_str()).map(str::to_string),
        equals: string_list("equals"),
        not: string_list("not"),
    }
}

fn setting_from_toml(
    data: &toml::Value,
    group: &toml::Value,
    where_: &str,
    optional: bool,
) -> Result<Setting, String> {
    let kind = data.get("type").and_then(|v| v.as_str()).unwrap_or("text").to_string();
    if !SETTING_TYPES.contains(&kind.as_str()) {
        return Err(format!("{where_}: unknown type {kind:?}"));
    }
    for required in ["key", "label"] {
        if data.get(required).is_none() {
            return Err(format!("{where_}: missing {required:?}"));
        }
    }
    let key = data.get("key").and_then(|v| v.as_str()).unwrap_or_default().to_string();
    let label = data.get("label").and_then(|v| v.as_str()).unwrap_or_default().to_string();

    let file_id = data
        .get("file")
        .and_then(|v| v.as_str())
        .or_else(|| group.get("file").and_then(|v| v.as_str()));
    let section = data
        .get("section")
        .and_then(|v| v.as_str())
        .or_else(|| group.get("section").and_then(|v| v.as_str()));
    let (Some(file_id), Some(section)) = (file_id.filter(|f| !f.is_empty()), section) else {
        return Err(format!("{where_}: {key} needs a file and section"));
    };

    let mut options = Vec::new();
    let mut option_requires = HashMap::new();
    if let Some(opts) = data.get("options").and_then(|v| v.as_array()) {
        for opt in opts {
            let entries = opt.as_array().cloned().unwrap_or_default();
            if entries.len() != 2 && entries.len() != 3 {
                return Err(format!(
                    "{where_}: options entries are [value, label] or [value, label, requires_engine]"
                ));
            }
            let value = value_to_string(&entries[0]);
            let opt_label = value_to_string(&entries[1]);
            if entries.len() == 3 {
                option_requires.insert(value.clone(), value_to_string(&entries[2]));
            }
            options.push((value, opt_label));
        }
    }

    Ok(Setting {
        file: file_id.to_string(),
        section: section.to_string(),
        key,
        label,
        kind,
        default: data.get("default").map(value_to_string),
        help: data.get("help").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        note: data.get("note").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        options,
        option_requires,
        requires_engine: data.get("requires_engine").and_then(|v| v.as_str()).map(str::to_string),
        optional,
        minimum: data.get("min").and_then(as_number).unwrap_or(0.0),
        maximum: data.get("max").and_then(as_number).unwrap_or(100.0),
        step: data.get("step").and_then(as_number).unwrap_or(1.0),
        suffix: data.get("suffix").and_then(|v| v.as_str()).unwrap_or("").to_string(),
        slider: data.get("slider").and_then(|v| v.as_bool()).unwrap_or(false),
        combo_keys: data.get("combo_keys").and_then(|v| v.as_bool()).unwrap_or(false),
        enabled_if: data.get("enabled_if").map(parse_enabled_if),
        hint: data.get("hint").and_then(|v| v.as_str()).map(str::to_string),
        decimals: data.get("decimals").and_then(as_number).map(|n| n as i32).unwrap_or(2),
    })
}

fn stem(name: &str) -> String {
    Path::new(name).file_stem().map(|s| s.to_string_lossy().to_string()).unwrap_or_else(|| name.to_string())
}

pub fn load_catalog(paths: &Paths) -> Catalog {
    let mut cat = Catalog::default();
    let engine_cache = EngineCache::new();

    for (name, is_core, content) in schema_sources(paths) {
        let content = match content {
            Ok(c) => c,
            Err(e) => {
                cat.errors.push(format!("{name}: {e}"));
                continue;
            }
        };
        let doc: toml::Value = match content.parse() {
            Ok(v) => v,
            Err(e) => {
                cat.errors.push(format!("{name}: {e}"));
                continue;
            }
        };

        if let Some(files) = doc.get("files").and_then(|v| v.as_table()) {
            for (k, v) in files {
                cat.files.insert(k.clone(), value_to_string(v));
            }
        }
        if let Some(list) = doc.get("not_settings").and_then(|v| v.as_array()) {
            cat.not_settings.extend(list.iter().map(value_to_string));
        }

        // A mod counts as installed when its archive is present; schemas without [[mod]]
        // entries fall back to their config files.
        let mods = doc.get("mod").and_then(|v| v.as_array()).cloned().unwrap_or_default();
        for m in &mods {
            if let Some(desc) = m.get("description") {
                let dat = m.get("dat").map(value_to_string).unwrap_or_default().to_lowercase();
                cat.mod_info.insert(dat, value_to_string(desc));
            }
        }
        let installed = if !mods.is_empty() {
            mods.iter().any(|m| {
                let dat = m.get("dat").map(value_to_string).unwrap_or_default();
                paths.mod_installed(&dat)
            })
        } else {
            doc.get("files")
                .and_then(|v| v.as_table())
                .map(|files| {
                    files.values().any(|rel| paths.ci_path(&value_to_string(rel), None).is_file())
                })
                .unwrap_or(false)
        };
        let headline = doc.get("headline").map(truthy).unwrap_or(false);
        if headline && installed {
            if let Some(title) = doc.get("title").and_then(|v| v.as_str()) {
                cat.headlines.push(title.to_string());
            }
        }

        if let Some(defaults_from) = doc.get("defaults_from").and_then(|v| v.as_table()) {
            for (fid, src) in defaults_from {
                let dat = src.get("dat").map(value_to_string).unwrap_or_default();
                let path = src.get("path").map(value_to_string).unwrap_or_default();
                cat.defaults_from.insert(fid.clone(), (dat, path));
            }
        }

        let doc_page_name = doc.get("page").and_then(|v| v.as_str()).map(str::to_string);
        let doc_intro = doc.get("intro").and_then(|v| v.as_str());
        if let Some(groups) = doc.get("group").and_then(|v| v.as_array()) {
            for (gi, group) in groups.iter().enumerate() {
                let page_name = group
                    .get("page")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .or_else(|| doc_page_name.clone())
                    .filter(|s| !s.is_empty())
                    .unwrap_or_else(|| stem(&name));
                let page = cat.page(&page_name);
                if let Some(intro) = doc_intro {
                    if page.intro.is_empty() {
                        page.intro = intro.to_string();
                    }
                }
                let group_name = group.get("name").and_then(|v| v.as_str()).unwrap_or("Settings");
                let settings = page.group(group_name);
                // Collected separately, like the unknown-file errors below: `settings`
                // still borrows `cat` (via `page`) at this point.
                let mut group_errors = Vec::new();
                if let Some(setting_list) = group.get("setting").and_then(|v| v.as_array()) {
                    for (si, data) in setting_list.iter().enumerate() {
                        let where_ = format!("{name} group {} setting {}", gi + 1, si + 1);
                        match setting_from_toml(data, group, &where_, !is_core) {
                            Ok(s) => settings.push(s),
                            Err(e) => group_errors.push(e),
                        }
                    }
                }
                cat.errors.extend(group_errors);
            }
        }
    }

    // Unknown-file references, collected separately so we're not borrowing `cat.pages` and
    // `cat.errors` mutably at the same time.
    let mut unknown_file_errors = Vec::new();
    for page in &cat.pages {
        for (_, settings) in &page.groups {
            for s in settings {
                if !cat.files.contains_key(&s.file) && s.file != "launcher" {
                    unknown_file_errors
                        .push(format!("{}: {} refers to unknown file {:?}", page.name, s.key, s.file));
                }
            }
        }
    }
    cat.errors.extend(unknown_file_errors);

    // Leave out what this install can't use: mod settings whose file isn't installed, and
    // engine settings the installed fallout2-ce binary doesn't have. Pages are taken out of
    // `cat` and rebuilt locally for the same borrowing reason as above.
    let old_pages = std::mem::take(&mut cat.pages);
    let mut new_pages = Vec::with_capacity(old_pages.len());
    let mut skipped = Vec::new();
    for mut page in old_pages {
        let mut kept_groups = Vec::with_capacity(page.groups.len());
        for (gname, settings) in page.groups {
            let mut kept = Vec::with_capacity(settings.len());
            for mut s in settings {
                if let Some(req) = s.requires_engine.clone() {
                    if !engine_cache.supports(paths, &req) {
                        skipped.push(format!("{}: needs engine setting {req:?}", s.key));
                        continue;
                    }
                }
                if s.optional {
                    let rel = cat.files.get(&s.file).cloned().unwrap_or_default();
                    if !paths.ci_path(&rel, None).is_file() {
                        skipped.push(format!("{rel} not installed"));
                        continue;
                    }
                }
                for (value, feature) in s.option_requires.clone() {
                    if !engine_cache.supports(paths, &feature) {
                        s.options.retain(|(v, _)| v != &value);
                        skipped.push(format!("{} option {value}: needs engine setting {feature:?}", s.key));
                    }
                }
                kept.push(s);
            }
            if !kept.is_empty() {
                kept_groups.push((gname, kept));
            }
        }
        page.groups = kept_groups;
        new_pages.push(page);
    }
    cat.pages = new_pages;
    cat.skipped.extend(skipped);

    // "Restoration Project" stays even with nothing installed yet - it's not a regular mod,
    // it gets its own always-visible tab (see the GUI port's rpu_page, once it exists).
    cat.pages.retain(|p| !p.groups.is_empty() || p.name == "Launcher" || p.name == "Restoration Project");

    // Mod config files without a schema get a generated tab.
    let covered: HashSet<PathBuf> = cat
        .files
        .values()
        .cloned()
        .chain(cat.not_settings.iter().cloned())
        .map(|rel| {
            let p = paths.ci_path(&rel, None);
            p.canonicalize().unwrap_or(p)
        })
        .collect();
    let mods_dir = paths.ci_path("mods", None);
    if mods_dir.is_dir() {
        if let Ok(entries) = std::fs::read_dir(&mods_dir) {
            let mut ini_paths: Vec<PathBuf> = entries
                .flatten()
                .map(|e| e.path())
                .filter(|p| {
                    p.extension().map(|e| e.to_string_lossy().to_lowercase()) == Some("ini".to_string())
                })
                .collect();
            ini_paths.sort();
            for ini in ini_paths {
                let resolved = ini.canonicalize().unwrap_or_else(|_| ini.clone());
                if covered.contains(&resolved) {
                    continue;
                }
                let ini_name = ini.file_name().unwrap().to_string_lossy().to_string();
                let stem_name = stem(&ini_name);
                let file_id = format!("auto_{}", sanitize_id(&stem_name.to_lowercase()));
                cat.files.insert(file_id.clone(), format!("mods/{ini_name}"));

                match generate_ini_groups(&ini, &file_id) {
                    Ok((mut groups, intro)) => {
                        if groups.is_empty() {
                            continue;
                        }
                        for (_, settings) in &mut groups {
                            for s in settings {
                                s.optional = true;
                            }
                        }
                        let mut page = Page::new(&stem_name);
                        page.groups = groups;
                        page.generated = true;
                        page.intro = format!(
                            "Generated automatically from mods/{ini_name}. Labels and controls are \
                             guessed from the file's comments; a schema file describes it properly. {intro}"
                        )
                        .trim()
                        .to_string();
                        cat.pages.push(page);
                    }
                    Err(e) => cat.errors.push(format!("{ini_name}: {e}")),
                }
            }
        }
    }

    cat
}

fn sanitize_id(s: &str) -> String {
    let mut out = String::new();
    let mut last_was_sep = false;
    for c in s.chars() {
        if c.is_ascii_alphanumeric() {
            out.push(c);
            last_was_sep = false;
        } else if !last_was_sep {
            out.push('_');
            last_was_sep = true;
        }
    }
    out
}

/// Comment lines like "1 - glowing red" or "2: Fill screen".
fn option_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"^(-?\d+)\s*(?:-|–|:|=)\s*(.+)$").unwrap())
}

/// A comment that talks about values other than 0/1 ("set to 2", "-1 is vanilla", "is 49").
fn non_bool_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)(?:^|\s)-1\b|\b(?:set (?:it )?to|to|is|=)\s*(?:[2-9]|\d\d+)\b").unwrap()
    })
}

fn bool_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| {
        Regex::new(r"(?i)enable|disable|set (?:it )?to [01]\b|\b1/0\b|\b0/1\b|\bon\b|\boff\b").unwrap()
    })
}

fn scan_code_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"(?i)scan ?code|\bdik\b|directinput|dik\.h").unwrap())
}

/// Finds the first "whitespace then `;`" so a value with a trailing inline comment (e.g.
/// `"5   ; notes"`) can be trimmed down to just `"5"`.
fn inline_comment_re() -> &'static Regex {
    static RE: OnceLock<Regex> = OnceLock::new();
    RE.get_or_init(|| Regex::new(r"\s+;").unwrap())
}

fn humanize(key: &str) -> String {
    let mut spaced = String::with_capacity(key.len() + 4);
    let mut prev_lower = false;
    for c in key.chars() {
        if prev_lower && c.is_ascii_uppercase() {
            spaced.push(' ');
        }
        spaced.push(c);
        prev_lower = c.is_ascii_lowercase();
    }
    let text = spaced.replace('_', " ");
    let text = text.trim();
    if text.is_empty() {
        return key.to_string();
    }
    let mut chars = text.chars();
    let first = chars.next().unwrap().to_uppercase().to_string();
    format!("{first}{}", chars.as_str())
}

fn infer_setting(
    file_id: &str,
    section: &str,
    key: &str,
    value: &str,
    comments: &[String],
    bool_hint: bool,
) -> Setting {
    let text = comments.join(" ");
    let mut options = Vec::new();
    let mut help_lines = Vec::new();
    for c in comments {
        match option_re().captures(c) {
            Some(m) => options.push((m[1].to_string(), m[2].trim().to_string())),
            None => help_lines.push(c.clone()),
        }
    }

    let label = humanize(key);
    let mut s = Setting::basic(file_id, section, key, &label, "text");
    s.help = help_lines.join(" ");

    let is_int = is_signed_int(value);
    if options.len() >= 2 && is_int {
        s.kind = "choice".to_string();
        s.options = options;
    } else if is_int && scan_code_re().is_match(&text) {
        s.kind = "key".to_string();
    } else if (value == "0" || value == "1")
        && (bool_hint || bool_re().is_match(&text))
        && !non_bool_re().is_match(&text)
    {
        s.kind = "bool".to_string();
    } else if is_int {
        s.kind = "int".to_string();
        s.minimum = -1_000_000.0;
        s.maximum = 1_000_000.0;
    } else if is_float_literal(value) {
        s.kind = "float".to_string();
        s.minimum = -1_000_000.0;
        s.maximum = 1_000_000.0;
        s.decimals = 3;
    }
    s
}

/// Build settings groups from an INI file's sections, keys and comments.
fn generate_ini_groups(path: &Path, file_id: &str) -> io::Result<(Vec<(String, Vec<Setting>)>, String)> {
    let raw = std::fs::read(path)?;
    let text = decode_latin1(&raw);

    let mut groups: Vec<(String, Vec<Setting>)> = Vec::new();
    let mut preamble: Vec<String> = Vec::new();
    let mut comments: Vec<String> = Vec::new();
    let mut section: Option<String> = None;

    for raw_line in super::ini_file::split_lines(&text) {
        let t = raw_line.trim();
        if t.is_empty() {
            comments.clear();
        } else if let Some(rest) = t.strip_prefix('[') {
            if let Some(end) = rest.find(']') {
                let name = rest[..end].trim().to_string();
                section = Some(name.clone());
                groups.push((name, Vec::new()));
                comments.clear();
            }
        } else if t.starts_with(';') || t.starts_with('#') {
            let c = t.trim_start_matches([';', '#']).trim().to_string();
            if !c.is_empty() {
                if section.is_some() {
                    comments.push(c);
                } else {
                    preamble.push(c);
                }
            }
        } else if t.contains('=') && section.is_some() {
            let (key, raw_value) = t.split_once('=').unwrap();
            let key = key.trim();
            let value = match inline_comment_re().find(raw_value) {
                Some(m) => raw_value[..m.start()].trim().to_string(),
                None => raw_value.trim().to_string(),
            };
            let bool_hint = bool_re().is_match(&preamble.join(" "));
            let setting =
                infer_setting(file_id, section.as_deref().unwrap(), key, &value, &comments, bool_hint);
            if let Some((_, settings)) = groups.last_mut() {
                settings.push(setting);
            }
            comments.clear();
        }
    }

    let groups = groups.into_iter().filter(|(_, ss)| !ss.is_empty()).collect();
    Ok((groups, preamble.join(" ")))
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn setting(kind: &str) -> Setting {
        Setting::basic("cfg", "screen", "scale", "Scale", kind)
    }

    #[test]
    fn normalize_bool_accepts_common_truthy_spellings() {
        let s = setting("bool");
        assert_eq!(s.normalize(Some("true")), Some("1".to_string()));
        assert_eq!(s.normalize(Some("YES")), Some("1".to_string()));
        assert_eq!(s.normalize(Some("5")), Some("1".to_string()));
        assert_eq!(s.normalize(Some("0")), Some("0".to_string()));
        assert_eq!(s.normalize(Some("off")), Some("0".to_string()));
    }

    #[test]
    fn normalize_int_strips_leading_zeros() {
        let s = setting("int");
        assert_eq!(s.normalize(Some("007")), Some("7".to_string()));
        assert_eq!(s.normalize(Some("-3")), Some("-3".to_string()));
        assert_eq!(s.normalize(Some("not a number")), Some("not a number".to_string()));
    }

    #[test]
    fn normalize_float_formats_to_six_decimals() {
        let s = setting("float");
        assert_eq!(s.normalize(Some("1.5")), Some("1.500000".to_string()));
    }

    #[test]
    fn normalize_none_stays_none() {
        assert_eq!(setting("text").normalize(None), None);
    }

    #[test]
    fn enabled_by_equals_and_not() {
        let mut s = setting("text");
        s.enabled_if = Some(EnabledIf {
            setting: Some("cfg:screen:windowed".to_string()),
            equals: Some(vec!["1".to_string()]),
            not: None,
        });
        assert!(s.enabled_by("1"));
        assert!(!s.enabled_by("0"));

        s.enabled_if = Some(EnabledIf { setting: None, equals: None, not: Some(vec!["0".to_string()]) });
        assert!(s.enabled_by("1"));
        assert!(!s.enabled_by("0"));
    }

    #[test]
    fn enabled_by_with_no_condition_is_always_enabled() {
        assert!(setting("text").enabled_by("anything"));
    }

    #[test]
    fn humanize_splits_camel_case_and_underscores() {
        // Only the very first character is uppercased; everything else - including the
        // original "L" from the camelCase boundary - is left exactly as it was, matching
        // Python's `text[:1].upper() + text[1:]`.
        assert_eq!(humanize("mouseLock"), "Mouse Lock");
        assert_eq!(humanize("main_menu_scale_mode"), "Main menu scale mode");
        assert_eq!(humanize("scale"), "Scale");
    }

    #[test]
    fn page_group_finds_or_creates() {
        let mut page = Page::new("Display");
        page.group("Screen").push(setting("bool"));
        page.group("Screen").push(setting("int"));
        assert_eq!(page.groups.len(), 1);
        assert_eq!(page.groups[0].1.len(), 2);
    }

    #[test]
    fn infer_setting_picks_choice_from_numbered_comments() {
        let comments = vec!["0 - Fullscreen".to_string(), "1 - Windowed".to_string()];
        let s = infer_setting("cfg", "screen", "windowed", "0", &comments, false);
        assert_eq!(s.kind, "choice");
        assert_eq!(s.options, vec![("0".to_string(), "Fullscreen".to_string()), ("1".to_string(), "Windowed".to_string())]);
    }

    #[test]
    fn infer_setting_picks_bool_from_enable_disable_comment() {
        let comments = vec!["Enable or disable the thing.".to_string()];
        let s = infer_setting("cfg", "main", "thing", "1", &comments, false);
        assert_eq!(s.kind, "bool");
    }

    #[test]
    fn infer_setting_does_not_call_a_scale_factor_a_bool() {
        // "set to 2" talks about a value other than 0/1, so this must not be inferred as bool
        // even though the stored value happens to be "1" right now.
        let comments = vec!["Set to 2 for double size.".to_string()];
        let s = infer_setting("cfg", "main", "scale", "1", &comments, false);
        assert_ne!(s.kind, "bool");
    }

    #[test]
    fn infer_setting_picks_key_from_scancode_mention() {
        let comments = vec!["DIK scan code for the hotkey.".to_string()];
        let s = infer_setting("cfg", "main", "hotkey", "42", &comments, false);
        assert_eq!(s.kind, "key");
    }

    #[test]
    fn infer_setting_falls_back_to_int_then_float_then_text() {
        assert_eq!(infer_setting("cfg", "main", "count", "42", &[], false).kind, "int");
        assert_eq!(infer_setting("cfg", "main", "ratio", "1.5", &[], false).kind, "float");
        assert_eq!(infer_setting("cfg", "main", "name", "hello", &[], false).kind, "text");
    }

    #[test]
    fn generate_ini_groups_builds_settings_from_comments() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.ini");
        std::fs::write(
            &path,
            "; preamble\n\n[main]\n; Enable the thing\nenabled=1\n; 0 - Red\n; 1 - Blue\ncolor=0   ; inline comment\n",
        )
        .unwrap();

        let (groups, intro) = generate_ini_groups(&path, "mymod").unwrap();
        assert_eq!(intro, "preamble");
        assert_eq!(groups.len(), 1);
        let (section, settings) = &groups[0];
        assert_eq!(section, "main");
        assert_eq!(settings.len(), 2);
        assert_eq!(settings[0].key, "enabled");
        assert_eq!(settings[0].kind, "bool");
        assert_eq!(settings[1].key, "color");
        assert_eq!(settings[1].kind, "choice");
    }

    #[test]
    fn generate_ini_groups_drops_empty_sections() {
        let tmp = tempfile::tempdir().unwrap();
        let path = tmp.path().join("mod.ini");
        std::fs::write(&path, "[empty]\n[main]\nkey=1\n").unwrap();

        let (groups, _) = generate_ini_groups(&path, "mymod").unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].0, "main");
    }

    fn write_schema(dir: &Path, name: &str, content: &str) {
        std::fs::write(dir.join(name), content).unwrap();
    }

    #[test]
    fn load_catalog_reads_a_mod_schema_from_disk() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        // The setting is optional (schemas found on disk aren't "core"), so it's filtered
        // out as "not installed" unless the file it configures actually exists too.
        std::fs::write(tmp.path().join("mods").join("mymod.ini"), "[main]\nenabled=1\n").unwrap();
        write_schema(
            &tmp.path().join("mods"),
            "mymod.schema.toml",
            r#"
                page = "My Mod"

                [files]
                mymod = "mods/mymod.ini"

                [[group]]
                name = "Main"
                file = "mymod"
                section = "main"

                [[group.setting]]
                key = "enabled"
                label = "Enabled"
                type = "bool"
                default = "1"
            "#,
        );

        let cat = load_catalog(&paths);
        assert!(cat.errors.is_empty(), "unexpected errors: {:?}", cat.errors);
        let page = cat.pages.iter().find(|p| p.name == "My Mod").expect("page should exist");
        assert_eq!(page.groups[0].1[0].key, "enabled");
        assert!(page.groups[0].1[0].optional);
    }

    #[test]
    fn load_catalog_reports_malformed_schema_as_an_error_not_a_panic() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        write_schema(&tmp.path().join("mods"), "broken.schema.toml", "this is not [ valid toml");

        let cat = load_catalog(&paths);
        assert!(cat.errors.iter().any(|e| e.contains("broken.schema.toml")));
    }

    #[test]
    fn load_catalog_rejects_setting_with_unknown_type() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        write_schema(
            &tmp.path().join("mods"),
            "bad.schema.toml",
            r#"
                [files]
                mymod = "mods/mymod.ini"

                [[group]]
                file = "mymod"
                section = "main"

                [[group.setting]]
                key = "x"
                label = "X"
                type = "not_a_real_type"
            "#,
        );

        let cat = load_catalog(&paths);
        assert!(cat.errors.iter().any(|e| e.contains("unknown type")));
    }

    #[test]
    fn load_catalog_generates_a_tab_for_an_uncovered_mod_ini() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        std::fs::create_dir_all(tmp.path().join("mods")).unwrap();
        std::fs::write(
            tmp.path().join("mods").join("UncoveredMod.ini"),
            "[main]\n; Enable the feature\nfeature=1\n",
        )
        .unwrap();

        let cat = load_catalog(&paths);
        let page = cat.pages.iter().find(|p| p.name == "UncoveredMod").expect("generated page");
        assert!(page.generated);
        assert!(page.groups[0].1[0].optional);
    }

    #[test]
    fn load_catalog_includes_the_core_display_schema() {
        let tmp = tempfile::tempdir().unwrap();
        let paths = Paths::new(tmp.path().to_path_buf());
        let cat = load_catalog(&paths);
        assert!(cat.errors.is_empty(), "unexpected errors in builtin schemas: {:?}", cat.errors);
        let display = cat.pages.iter().find(|p| p.name == "Display").expect("Display page");
        let resolution = display
            .groups
            .iter()
            .flat_map(|(_, ss)| ss)
            .find(|s| s.key == "resolution_x|resolution_y")
            .expect("resolution setting");
        assert_eq!(resolution.kind, "resolution");
        assert_eq!(resolution.file, "cfg");
    }
}
