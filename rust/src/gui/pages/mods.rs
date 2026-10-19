//! The Mods page: enable/disable/reorder installed mods, and download/update/delete catalog
//! mods, all in one list. See `backend::mod_catalog` for the install/update/delete logic;
//! this just drives it. Ported from `gui/mods_page.py`, minus drag-and-drop reordering -
//! "Move up"/"Move down" buttons do the same job without egui-specific drag plumbing.

use std::collections::{HashMap, HashSet};

use eframe::egui;

use crate::backend::mod_catalog::{
    check_for_update, delete_mod_files, install_catalog_entry, load_catalog_state, save_catalog_state,
    CatalogEntry,
};
use crate::backend::paths::SharedPaths;
use crate::gui::app::{App, ConfirmAction, PendingConfirm};
use crate::gui::jobs::JobOutcome;
use crate::gui::open::open_path;
use crate::gui::theme;
use crate::gui::widgets::card;

#[derive(Clone)]
pub(crate) struct ModRow {
    pub(crate) dat: String,
    pub(crate) enabled: bool,
    pub(crate) installed: bool,
    entry: Option<CatalogEntry>,
    subtitle: String,
}

pub struct ModsPage {
    // `pub(crate)` rather than private + accessor methods: `pages::rpu::show` renders a
    // subset of these same rows (RPU's own optional extras and bundled companions) - see its
    // module doc comment - so it needs direct read/write access from another page module in
    // this crate, same as `app.rs` already has to `App`'s own fields.
    pub(crate) rows: Vec<ModRow>,
    initial: Vec<(String, bool)>,
    hide_missing: bool,
    pending_updates: HashMap<String, String>,
    busy: HashSet<String>,
    checked_for_updates: bool,
}

impl ModsPage {
    pub fn new(
        paths: &SharedPaths,
        mod_catalog: &[CatalogEntry],
        mod_info: &HashMap<String, String>,
        is_ce: bool,
    ) -> Self {
        let mut page = ModsPage {
            rows: Vec::new(),
            initial: Vec::new(),
            hide_missing: true,
            pending_updates: HashMap::new(),
            busy: HashSet::new(),
            checked_for_updates: false,
        };
        page.rebuild(paths, mod_catalog, mod_info, is_ce);
        page
    }

    /// `is_ce` hides the catalog's "not installed yet, download it" suggestions outside the
    /// Vanilla/CE campaign - `schemas/catalog.toml`'s own header scopes every entry to "regular
    /// mods fallout2-ce's own README lists", so they're not guaranteed to even apply to Et
    /// Tu/Sonora/Nevada. Anything already installed still shows regardless of campaign - this
    /// only hides unsolicited download offers, never an existing row.
    pub fn rebuild(
        &mut self,
        paths: &SharedPaths,
        mod_catalog: &[CatalogEntry],
        mod_info: &HashMap<String, String>,
        is_ce: bool,
    ) {
        self.rows.clear();
        let mut seen = HashSet::new();

        let order_path = paths.ci_path("mods/mods_order.txt", None);
        if let Ok(bytes) = std::fs::read(&order_path) {
            for line in decode_latin1(&bytes).lines() {
                let t = line.trim();
                if t.is_empty() {
                    continue;
                }
                let enabled = !t.contains(';') && !t.contains('#');
                let name: String = t.chars().filter(|&c| c != ';' && c != '#').collect::<String>().trim().to_string();
                let lower = name.to_lowercase();
                if name.is_empty() || seen.contains(&lower) {
                    continue;
                }
                if paths.mod_installed(&name) {
                    let entry = find_catalog_entry(mod_catalog, &name);
                    let subtitle = entry
                        .as_ref()
                        .map(|e| e.description.clone())
                        .unwrap_or_else(|| mod_info.get(&lower).cloned().unwrap_or_default());
                    self.rows.push(ModRow { dat: name, enabled, installed: true, entry, subtitle });
                    seen.insert(lower);
                }
            }
        }

        let mods_dir = paths.ci_path("mods", None);
        if mods_dir.is_dir() {
            if let Ok(entries) = std::fs::read_dir(&mods_dir) {
                let mut extra: Vec<_> = entries
                    .flatten()
                    .map(|e| e.path())
                    .filter(|p| p.extension().map(|e| e.to_string_lossy().to_lowercase()) == Some("dat".to_string()))
                    .collect();
                extra.sort();
                for p in extra {
                    let name = p.file_name().unwrap().to_string_lossy().to_string();
                    let lower = name.to_lowercase();
                    if seen.contains(&lower) || lower == "f2_res.dat" {
                        continue;
                    }
                    self.rows.push(ModRow { dat: name, enabled: false, installed: true, entry: None, subtitle: String::new() });
                    seen.insert(lower);
                }
            }
        }

        for entry in mod_catalog {
            if !is_ce || entry.is_installed(paths) {
                continue;
            }
            self.rows.push(ModRow {
                dat: entry.dat[0].clone(),
                enabled: false,
                installed: false,
                subtitle: entry.description.clone(),
                entry: Some(entry.clone()),
            });
        }

        self.initial = self.serialize();
    }

    fn serialize(&self) -> Vec<(String, bool)> {
        self.rows.iter().filter(|r| r.installed).map(|r| (r.dat.clone(), r.enabled)).collect()
    }

    pub fn dirty_count(&self) -> usize {
        if self.serialize() == self.initial {
            0
        } else {
            1
        }
    }

    /// Discard in-memory reordering/toggling by re-reading from disk - `mods_order.txt`
    /// itself is the only source of truth, so "revert" and "rebuild" are the same thing.
    pub fn revert(
        &mut self,
        paths: &SharedPaths,
        mod_catalog: &[CatalogEntry],
        mod_info: &HashMap<String, String>,
        is_ce: bool,
    ) {
        self.rebuild(paths, mod_catalog, mod_info, is_ce);
    }

    pub fn save(&mut self, paths: &SharedPaths) -> Result<(), String> {
        if self.dirty_count() == 0 {
            return Ok(());
        }
        let path = paths.ci_path("mods/mods_order.txt", None);
        if path.is_file() {
            let stamp = crate::gui::store::timestamp_now();
            let dest = paths.backup_dir().join(stamp).join("mods").join("mods_order.txt");
            if let Some(parent) = dest.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::copy(&path, &dest).map_err(|e| e.to_string())?;
        }
        let lines: Vec<String> =
            self.serialize().into_iter().map(|(name, enabled)| if enabled { name } else { format!(";{name}") }).collect();
        std::fs::write(&path, encode_latin1(&format!("{}\n", lines.join("\n")))).map_err(|e| e.to_string())?;
        self.initial = self.serialize();
        Ok(())
    }
}

fn find_catalog_entry(mod_catalog: &[CatalogEntry], filename: &str) -> Option<CatalogEntry> {
    mod_catalog.iter().find(|e| e.dat.iter().any(|d| d.eq_ignore_ascii_case(filename))).cloned()
}

fn decode_latin1(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn encode_latin1(s: &str) -> Vec<u8> {
    s.chars().map(|c| c as u8).collect()
}

/// Draws the Mods page and handles its buttons. Takes the whole `App` (rather than just the
/// `ModsPage`) because downloading/deleting a mod spawns a background job and can trigger a
/// full page reload - both live on `App`.
pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(theme::bold("Mods").size(17.0).color(theme::STRONG_TEXT));
    ui.label(
        egui::RichText::new(
            "Tick to enable, use Move up/down to reorder. Mods lower in the list override the ones \
             above. Mods fallout2-ce explicitly supports that you don't have yet are listed too, with \
             a Download button.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(10.0);

    if !app.mods.checked_for_updates {
        app.mods.checked_for_updates = true;
        let campaign = *app.campaign;
        for entry in app.mod_catalog.iter().filter(|e| e.is_installed(&app.paths) && e.github.is_some()) {
            let entry = entry.clone();
            let id = entry.id.clone();
            app.jobs.spawn(move || {
                let paths = crate::gui::jobs::fresh_paths(campaign);
                JobOutcome::ModUpdateCheck(id, check_for_update(&paths, &entry))
            });
        }
    }

    ui.horizontal(|ui| {
        let can_move = app.mods.rows.iter().position(|r| r.installed).is_some();
        ui.add_enabled_ui(can_move, |ui| {
            if ui.button("Move up").clicked() {
                move_row(app, -1);
            }
            if ui.button("Move down").clicked() {
                move_row(app, 1);
            }
        });
        let mut show_missing = !app.mods.hide_missing;
        if ui.checkbox(&mut show_missing, "Show not installed").changed() {
            app.mods.hide_missing = !show_missing;
        }
        if ui.button("Open mods folder").clicked() {
            open_path(&app.paths.ci_path("mods", None));
        }
    });
    ui.add_space(8.0);

    // RPU's own core, optional extras and bundled companions never show here - they're
    // managed entirely from RPU's own page (see `GLOSSARY.md`'s RPU entry and
    // `pages::rpu::show`), even though their toggle state lives in these same `rows`.
    let rpu_owned = if app.campaign.id == "ce" {
        crate::backend::rpu_install::owned_file_names(&app.paths)
    } else {
        HashSet::new()
    };

    card(ui, "Load order", |ui| {
        let rows = app.mods.rows.clone();
        let mut move_requested: Option<(usize, i32)> = None;
        for (i, row) in rows.iter().enumerate() {
            if rpu_owned.contains(&row.dat.to_lowercase()) {
                continue;
            }
            if !row.installed && app.mods.hide_missing {
                continue;
            }
            if i > 0 {
                ui.separator();
            }
            ui.horizontal(|ui| {
                if row.installed {
                    let mut enabled = row.enabled;
                    if ui.checkbox(&mut enabled, "").changed() {
                        app.mods.rows[i].enabled = enabled;
                    }
                } else {
                    ui.add_space(22.0);
                }
                ui.vertical(|ui| {
                    let title = row.entry.as_ref().map(|e| e.name.clone()).unwrap_or_else(|| row.dat.clone());
                    ui.label(theme::bold(&title).color(theme::STRONG_TEXT));
                    if !row.subtitle.is_empty() {
                        ui.label(egui::RichText::new(&row.subtitle).color(theme::MUTED).small());
                    }
                });
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    let busy = app.mods.busy.contains(&row.dat.to_lowercase());
                    if row.installed {
                        if ui.add_enabled(!busy, egui::Button::new("Delete")).clicked() {
                            app.confirm_delete_mod(row.clone());
                        }
                    }
                    if let Some(entry) = &row.entry {
                        if row.installed {
                            if let Some(version) = app.mods.pending_updates.get(&entry.id).cloned() {
                                if ui.add_enabled(!busy, egui::Button::new(format!("Update ({version})"))).clicked()
                                {
                                    start_download(app, entry.clone());
                                }
                            }
                        } else if ui
                            .add_enabled(!busy, egui::Button::new(if busy { "Downloading\u{2026}" } else { "Download" }))
                            .clicked()
                        {
                            start_download(app, entry.clone());
                        }
                        if ui.button("Open source").clicked() {
                            crate::gui::open::open(&entry.homepage);
                        }
                    }
                    if i > 0 && row.installed {
                        if ui.small_button("\u{25b2}").clicked() {
                            move_requested = Some((i, -1));
                        }
                    }
                    if row.installed && i + 1 < rows.len() {
                        if ui.small_button("\u{25bc}").clicked() {
                            move_requested = Some((i, 1));
                        }
                    }
                });
            });
        }
        if let Some((i, delta)) = move_requested {
            let j = (i as i32 + delta) as usize;
            app.mods.rows.swap(i, j);
        }
    });
}

fn move_row(app: &mut App, delta: i32) {
    // "Move up/down" toolbar buttons act on the first installed row for simplicity; the
    // per-row ▲/▼ buttons (drawn in `show`) are the precise way to reorder a specific mod.
    if let Some(i) = app.mods.rows.iter().position(|r| r.installed) {
        let j = i as i32 + delta;
        if j >= 0 && (j as usize) < app.mods.rows.len() {
            app.mods.rows.swap(i, j as usize);
        }
    }
}

fn start_download(app: &mut App, entry: CatalogEntry) {
    app.mods.busy.insert(entry.dat[0].to_lowercase());
    let campaign = *app.campaign;
    let entry_for_job = entry.clone();
    app.jobs.spawn(move || {
        let paths = crate::gui::jobs::fresh_paths(campaign);
        let result = install_catalog_entry(&paths, &entry_for_job);
        JobOutcome::ModInstall(entry_for_job.id.clone(), result)
    });
}

impl App {
    pub(crate) fn handle_mod_update_check(&mut self, entry_id: String, version: Option<String>) {
        if let Some(v) = version {
            self.mods.pending_updates.insert(entry_id, v);
        }
    }

    pub(crate) fn handle_mod_install(&mut self, entry_id: String, result: Result<(), String>) {
        if let Some(entry) = self.mod_catalog.iter().find(|e| e.id == entry_id) {
            self.mods.busy.remove(&entry.dat[0].to_lowercase());
        }
        match result {
            Ok(()) => {
                self.mods.pending_updates.remove(&entry_id);
                self.needs_reload = true;
            }
            Err(e) => self.error_dialog = Some(("Download failed".to_string(), e)),
        }
    }

    fn confirm_delete_mod(&mut self, row: ModRow) {
        let extra: Vec<String> = row.entry.as_ref().and_then(|e| e.ini.clone()).into_iter().collect();
        let listed = std::iter::once(row.dat.clone()).chain(extra.iter().cloned()).collect::<Vec<_>>().join(", ");
        self.pending_confirm = Some(PendingConfirm {
            title: "Delete mod".to_string(),
            message: format!("Remove {listed} from mods/? This can't be undone."),
            action: ConfirmAction::DeleteMod { dat: row.dat, extra, catalog_id: row.entry.map(|e| e.id) },
        });
    }
}

/// Carries out a mod deletion the player already confirmed via the `ConfirmAction::DeleteMod`
/// modal (see `app.rs`'s confirm-dialog handling).
pub(crate) fn delete_mod_now(app: &mut App, dat: &str, extra: &[String], catalog_id: Option<String>) {
    delete_mod_files(&app.paths, dat, extra);
    if let Some(id) = catalog_id {
        let mut state = load_catalog_state(&app.paths);
        if state.remove(&id).is_some() {
            let _ = save_catalog_state(&app.paths, &state);
        }
        app.mods.pending_updates.remove(&id);
    }
    app.needs_reload = true;
}
