//! The Launcher page: its own settings (the "Launcher" schema page - show this launcher at
//! startup, what to do when it's skipped) plus file shortcuts, install info, and the Steam
//! launch-options line. Ported from `gui/launcher_page.py`.

use eframe::egui;

use crate::backend::ce_install::{load_ce_state, needs_ce_install};
use crate::backend::ini_file::IniFile;
use crate::backend::launch_option::steam_launch_option;
use crate::gui::app::App;
use crate::gui::open::open_path;
use crate::gui::theme;
use crate::gui::widgets::card;

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Launcher").size(17.0).strong().color(theme::STRONG_TEXT));
    ui.add_space(8.0);

    let launcher_page = app.store.catalog.pages.iter().find(|p| p.name == "Launcher").cloned();
    if let Some(page) = launcher_page {
        let search = app.search.trim().to_lowercase();
        super::settings::show(ui, &mut app.store, &page.groups, &page.intro, &mut app.capturing_key, &search);
    }

    card(ui, "Files & folders", |ui| {
        let links: [(&str, &str); 6] = [
            ("Game folder", ""),
            ("Save games", "data/SAVEGAME"),
            ("Mods folder", "mods"),
            ("Settings backups", "backup/launcher"),
            ("fallout2.cfg", "fallout2.cfg"),
            ("game#patch.cfg", "data/config/game#patch.cfg"),
        ];
        ui.horizontal_wrapped(|ui| {
            for (label, rel) in links {
                let path = if rel.is_empty() { app.paths.game_dir() } else { app.paths.ci_path(rel, None) };
                let exists = path.exists();
                if ui.add_enabled(exists, egui::Button::new(label)).clicked() {
                    open_path(&path);
                }
            }
        });
    });
    ui.add_space(12.0);

    card(ui, "About this install", |ui| {
        let bin_path = app.paths.game_bin();
        let bin_name = bin_path.file_name().map(|n| n.to_string_lossy().to_string()).unwrap_or_default();
        let engine = if bin_path.is_file() {
            let mut s = format!("Fallout 2 Community Edition ({bin_name})");
            if let Some(published) = load_ce_state(&app.paths).get("published_at") {
                let date = &published[..published.len().min(10)];
                s.push_str(&format!(" \u{b7} installed build from {date}"));
            }
            s
        } else if needs_ce_install(&app.paths) {
            "Not installed yet \u{2014} press Play to download and install it".to_string()
        } else {
            format!("Not found ({bin_name} is missing)")
        };

        let mods =
            if app.store.catalog.headlines.is_empty() { "none with a schema".to_string() } else { app.store.catalog.headlines.join(", ") };
        let version = app
            .store
            .catalog
            .files
            .get("content")
            .and_then(|rel| IniFile::open(app.paths.ci_path(rel, None)).ok()?.get("main_menu", "version_string"))
            .unwrap_or_else(|| "unknown".to_string());

        ui.label(egui::RichText::new(format!("Engine: {engine}")).color(theme::MUTED));
        ui.label(egui::RichText::new(format!("Headline mods: {mods}")).color(theme::MUTED));
        ui.label(egui::RichText::new(format!("Version string: {version}")).color(theme::MUTED));
        ui.label(egui::RichText::new(format!("Folder: {}", app.paths.game_dir().display())).color(theme::MUTED));
        ui.label(
            egui::RichText::new("Every time the launcher saves, the previous config files are copied to \
                                  backup/launcher/ first.")
                .color(theme::MUTED)
                .small(),
        );
    });
    ui.add_space(12.0);

    card(ui, "Steam launch options", |ui| {
        ui.label(
            egui::RichText::new(
                "Paste this into the game's Properties \u{2192} General \u{2192} Launch Options in Steam, so \
                 pressing Play opens this launcher instead of the game directly.",
            )
            .color(theme::MUTED),
        );
        let (option_text, verified) = steam_launch_option(None);
        ui.horizontal(|ui| {
            let mut buf = option_text.clone();
            ui.add(egui::TextEdit::singleline(&mut buf).desired_width(420.0).interactive(false));
            if ui.button("Copy").clicked() {
                ui.ctx().copy_text(option_text.clone());
            }
        });
        if !verified {
            ui.label(
                egui::RichText::new(
                    "Unverified on Windows: Steam may run this and still start the game afterward too, \
                     rather than replacing it. Please confirm and adjust if needed.",
                )
                .color(theme::AMBER),
            );
        }
    });
}
