//! The Restoration Project (RPU) page: a dedicated tab inside the CE campaign, always
//! present - RPU is a content overlay, not a separate campaign and not a regular mod (see
//! `GLOSSARY.md`), so it gets its own install/update/uninstall card instead of living on the
//! Mods page, and shows up even before it's installed. Ported from `gui/rpu_page.py`.
//!
//! Everything RPU owns - its optional extras and its bundled companions (NPC Armor, Party
//! Orders, the Unofficial Patch) - is toggled right here too, never on the Mods page (see
//! `GLOSSARY.md`'s RPU entry). The toggle state itself still lives in `app.mods.rows` - same
//! data, same save/discard handling, just a different page renders this subset of it (see
//! `pages::mods::show`'s matching exclusion).

use eframe::egui;

use crate::backend::campaigns::by_id;
use crate::backend::rpu_install::{is_installed, load_rpu_state, owned_file_names};
use crate::gui::app::{App, ConfirmAction, PendingConfirm};
use crate::gui::pages::campaigns::cover_thumbnail;
use crate::gui::theme;
use crate::gui::widgets::card;

const RPU_HOMEPAGE: &str = "https://github.com/BGforgeNet/Fallout2_Restoration_Project";

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(theme::bold("Restoration Project").size(17.0).color(theme::STRONG_TEXT));
    ui.add_space(8.0);

    let installed = is_installed(&app.paths);
    // RPU isn't a separate campaign (see GLOSSARY.md) so it gets no cover art of its own -
    // this reuses CE's, since RPU is just more of that same game.
    let ce = by_id("ce").expect("\"ce\" is a registered campaign");
    card(ui, "Restoration Project Updated", |ui| {
        ui.horizontal(|ui| {
            cover_thumbnail(ui, ce);
            ui.vertical(|ui| {
                let status = if installed {
                    let version = load_rpu_state(&app.paths)
                        .get("version")
                        .cloned()
                        .unwrap_or_else(|| "an unknown version".to_string());
                    format!(
                        "Installed ({version}). Restores cut content and fixes to Fallout 2, and brings its \
                         officially-bundled NPC Armor, Party Orders and Unofficial Patch along with it, plus \
                         its own optional extras (enhanced world map, new animations, Cassidy's voice/head, \
                         ...) - all switched on individually below, never on the Mods page."
                    )
                } else {
                    "Restores cut content and fixes to Fallout 2 - not a separate campaign, just more of the \
                     same game. Not installed yet."
                        .to_string()
                };
                ui.label(egui::RichText::new(status).color(theme::MUTED));
                ui.add_space(6.0);

                ui.horizontal(|ui| {
                    if ui.button("Open source").clicked() {
                        crate::gui::open::open(RPU_HOMEPAGE);
                    }
                    let busy = app.rpu_busy;
                    if busy {
                        crate::gui::widgets::busy_status(ui, app.job_stages.get("rpu").map(String::as_str));
                    } else {
                        let action_text = if !installed {
                            "Install".to_string()
                        } else if let Some(v) = &app.rpu_pending_update {
                            format!("Update to {v}")
                        } else {
                            "Check for updates".to_string()
                        };
                        if ui.button(action_text).clicked() {
                            on_action(app, installed);
                        }
                    }
                    if installed && ui.add_enabled(!busy, egui::Button::new("Uninstall")).clicked() {
                        app.pending_confirm = Some(PendingConfirm {
                            title: "Uninstall Restoration Project".to_string(),
                            message: "Remove RPU's files from mods/? This can't be undone.".to_string(),
                            action: ConfirmAction::UninstallRpu,
                        });
                    }
                });
            });
        });
    });
    ui.add_space(12.0);

    if installed {
        draw_bundled_content(app, ui);
    }

    // RPU's own togglable settings (once installed) live on a schema page literally named
    // "Restoration Project" - same page the old Qt build's `RpuPage` extended `SettingsPage`
    // with, drawn right here rather than as a separate nav entry.
    let rpu_page = app.store.catalog.pages.iter().find(|p| p.name == "Restoration Project").cloned();
    if let Some(page) = rpu_page {
        let search = app.search.trim().to_lowercase();
        super::settings::show(ui, &mut app.store, &page.groups, &page.intro, &mut app.capturing_key, &search);
    }

    // The Unofficial Patch's own togglable settings (same mods/upu.ini RPU ships) nest here
    // too, rather than getting their own nav tab - see `build_nav`'s exclusion in `app.rs`.
    let upu_page = app.store.catalog.pages.iter().find(|p| p.name == "Unofficial Patch").cloned();
    if let Some(page) = upu_page {
        ui.add_space(12.0);
        let search = app.search.trim().to_lowercase();
        super::settings::show(ui, &mut app.store, &page.groups, &page.intro, &mut app.capturing_key, &search);
    }
}

/// The toggle rows for everything RPU brought in beyond its own core (rpu.dat/rpu.ini, which
/// have no toggle - they're just "on" as part of RPU itself): its optional extras and its
/// bundled companions. Reuses `app.mods.rows`' existing enable/disable + save/discard
/// machinery - this only changes which page renders this subset of rows, not how they're
/// persisted.
fn draw_bundled_content(app: &mut App, ui: &mut egui::Ui) {
    let owned = owned_file_names(&app.paths);
    let rows: Vec<usize> = app
        .mods
        .rows
        .iter()
        .enumerate()
        .filter(|(_, r)| r.installed && owned.contains(&r.dat.to_lowercase()) && r.dat.to_lowercase() != "rpu.dat")
        .map(|(i, _)| i)
        .collect();
    if rows.is_empty() {
        return;
    }

    card(ui, "Bundled content", |ui| {
        ui.label(
            egui::RichText::new("Optional extras and companion mods that came in with RPU - toggle them here.")
                .color(theme::MUTED),
        );
        ui.add_space(6.0);
        for (n, i) in rows.iter().enumerate() {
            if n > 0 {
                ui.separator();
            }
            let dat = app.mods.rows[*i].dat.clone();
            let label = app.store.catalog.mod_info.get(&dat.to_lowercase()).cloned().unwrap_or_else(|| dat.clone());
            let mut enabled = app.mods.rows[*i].enabled;
            if ui.checkbox(&mut enabled, label).changed() {
                app.mods.rows[*i].enabled = enabled;
            }
        }
    });
    ui.add_space(12.0);
}

fn on_action(app: &mut App, installed: bool) {
    use crate::backend::rpu_install::{check_for_rpu_update, install_rpu};
    use crate::gui::jobs::fresh_paths;

    let ce = *crate::backend::campaigns::by_id("ce").expect("\"ce\" is a registered campaign");
    app.rpu_busy = true;
    if !installed || app.rpu_pending_update.take().is_some() {
        app.job_stages.insert("rpu".to_string(), "Downloading\u{2026}".to_string());
        app.jobs.spawn("rpu", move |report| crate::gui::jobs::JobOutcome::RpuInstall(install_rpu(&fresh_paths(ce), report)));
    } else {
        app.job_stages.insert("rpu".to_string(), "Working\u{2026}".to_string());
        app.jobs.spawn("rpu", move |_report| {
            crate::gui::jobs::JobOutcome::RpuUpdateCheck(check_for_rpu_update(&fresh_paths(ce)))
        });
    }
}
