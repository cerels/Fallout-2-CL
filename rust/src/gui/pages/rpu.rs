//! The Restoration Project (RPU) page: a dedicated tab inside the CE campaign, always
//! present - RPU is a content overlay, not a separate campaign and not a regular mod (see
//! `GLOSSARY.md`), so it gets its own install/update/uninstall card instead of living on the
//! Mods page, and shows up even before it's installed. Ported from `gui/rpu_page.py`.

use eframe::egui;

use crate::backend::rpu_install::{is_installed, load_rpu_state};
use crate::gui::app::{App, ConfirmAction, PendingConfirm};
use crate::gui::theme;
use crate::gui::widgets::card;

const RPU_HOMEPAGE: &str = "https://github.com/BGforgeNet/Fallout2_Restoration_Project";

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(theme::bold("Restoration Project").size(17.0).color(theme::STRONG_TEXT));
    ui.add_space(8.0);

    let installed = is_installed(&app.paths);
    card(ui, "Restoration Project Updated", |ui| {
        let status = if installed {
            let version = load_rpu_state(&app.paths)
                .get("version")
                .cloned()
                .unwrap_or_else(|| "an unknown version".to_string());
            format!(
                "Installed ({version}). Restores cut content and fixes to Fallout 2, and brings its \
                 officially-bundled NPC Armor, Party Orders and Unofficial Patch along with it (their \
                 own settings pages appear once installed) - the optional extras (enhanced world map, \
                 new animations, Cassidy's voice/head, ...) can be switched on individually from the \
                 Mods page."
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
            let action_text = if busy {
                "Working\u{2026}".to_string()
            } else if !installed {
                "Install".to_string()
            } else if let Some(v) = &app.rpu_pending_update {
                format!("Update to {v}")
            } else {
                "Check for updates".to_string()
            };
            if ui.add_enabled(!busy, egui::Button::new(action_text)).clicked() {
                on_action(app, installed);
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
    ui.add_space(12.0);

    // RPU's own togglable settings (once installed) live on a schema page literally named
    // "Restoration Project" - same page the old Qt build's `RpuPage` extended `SettingsPage`
    // with, drawn right here rather than as a separate nav entry.
    let rpu_page = app.store.catalog.pages.iter().find(|p| p.name == "Restoration Project").cloned();
    if let Some(page) = rpu_page {
        let search = app.search.trim().to_lowercase();
        super::settings::show(ui, &mut app.store, &page.groups, &page.intro, &mut app.capturing_key, &search);
    }
}

fn on_action(app: &mut App, installed: bool) {
    use crate::backend::rpu_install::{check_for_rpu_update, install_rpu};
    use crate::gui::jobs::fresh_paths;

    let ce = *crate::backend::campaigns::by_id("ce").expect("\"ce\" is a registered campaign");
    app.rpu_busy = true;
    if !installed || app.rpu_pending_update.take().is_some() {
        app.jobs.spawn(move || crate::gui::jobs::JobOutcome::RpuInstall(install_rpu(&fresh_paths(ce))));
    } else {
        app.jobs.spawn(move || crate::gui::jobs::JobOutcome::RpuUpdateCheck(check_for_rpu_update(&fresh_paths(ce))));
    }
}
