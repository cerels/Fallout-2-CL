//! The Campaigns page: install, switch between, and manage every campaign this launcher
//! knows about (see `GLOSSARY.md`). The sidebar's campaign switcher covers picking an
//! already-installed one; this page is where you install a new one or fix up something
//! like Et Tu's Fallout 1 data. Ported from `gui/campaigns_page.py`.

use eframe::egui;

use crate::backend::campaigns::{Campaign, CAMPAIGNS};
use crate::backend::launch_option::steam_launch_option;
use crate::backend::steam_shortcuts::add_campaign_to_steam;
use crate::backend::{ce_install, ettu_install, nevada_install, sonora_install};
use crate::gui::app::{App, ConfirmAction, PendingConfirm};
use crate::gui::jobs::JobOutcome;
use crate::gui::theme;
use crate::gui::widgets::card;

fn description(id: &str) -> &'static str {
    match id {
        "ce" => {
            "Vanilla Fallout 2, on the Community Edition engine. RPU (Restoration Project) lives \
             on its own tab inside this campaign's menu, not here."
        }
        "ettu" => {
            "Fallout 1, ported into the Fallout 2 engine. Installing fetches Fallout 2's own engine \
             files automatically; it still needs a one-time extraction from a Fallout 1 copy you \
             already own before it can start a game."
        }
        "sonora" => "A standalone total conversion set in a post-war Mexico/Sonora - no other game required.",
        "nevada" => "A standalone total conversion (English translation) - no other game required.",
        _ => "",
    }
}

fn install_fn(id: &'static str) -> fn(&crate::backend::paths::Paths) -> Result<(), String> {
    match id {
        "ce" => ce_install::install_ce,
        "ettu" => ettu_install::install_ettu,
        "sonora" => sonora_install::install_sonora,
        "nevada" => nevada_install::install_nevada,
        _ => unreachable!("every registered campaign id has an installer"),
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(egui::RichText::new("Campaigns").size(17.0).strong().color(theme::STRONG_TEXT));
    ui.label(
        egui::RichText::new(
            "Pick what you're playing. Switching reloads this launcher's settings, mods and saves \
             for that campaign - each keeps its own.",
        )
        .color(theme::MUTED),
    );
    ui.add_space(10.0);

    for campaign in CAMPAIGNS {
        campaign_card(app, ui, campaign);
        ui.add_space(12.0);
    }
}

fn campaign_card(app: &mut App, ui: &mut egui::Ui, campaign: &'static Campaign) {
    let is_active = campaign.id == app.campaign.id;
    let is_installed = campaign.is_installed(&app.paths);
    let busy = app.campaigns_busy.get(campaign.id).cloned();

    card(ui, &format!("{}  {}", campaign.badge, campaign.name), |ui| {
        let mut status = description(campaign.id).to_string();
        if is_active {
            status.push_str(" Currently active.");
        } else if is_installed {
            status.push_str(" Installed.");
        } else {
            status.push_str(" Not installed yet.");
        }
        if campaign.id == "ettu" && is_installed && !ettu_install::has_fallout1_data(&app.paths) {
            status.push_str(" Needs Fallout 1's data extracted before it can start a game.");
        }
        ui.label(egui::RichText::new(status).color(theme::MUTED));
        ui.add_space(6.0);

        ui.horizontal(|ui| {
            if !is_installed {
                let label = busy.clone().unwrap_or_else(|| "Install".to_string());
                if ui.add_enabled(busy.is_none(), egui::Button::new(label)).clicked() {
                    start_install(app, campaign);
                }
            }
            if is_installed && !is_active && ui.button("Switch to this campaign").clicked() {
                app.switch_campaign(campaign);
            }
        });

        if campaign.id == "ettu" && is_installed {
            ui.add_space(8.0);
            ui.label(
                egui::RichText::new(
                    "Path to a Fallout 1 copy you already own's MASTER.DAT (needed once, to pull its \
                     data into Et Tu):",
                )
                .color(theme::MUTED)
                .small(),
            );
            ui.horizontal(|ui| {
                ui.add(
                    egui::TextEdit::singleline(&mut app.fallout1_path)
                        .desired_width(380.0)
                        .hint_text("/path/to/Fallout 1/MASTER.DAT"),
                );
                let extracting = app.campaigns_busy.get("ettu-f1").is_some();
                if ui.add_enabled(!extracting, egui::Button::new(if extracting { "Extracting\u{2026}" } else { "Extract" })).clicked() {
                    start_fallout1_extract(app);
                }
            });
            ui.add_space(4.0);
            ui.label(
                egui::RichText::new(
                    "Paste into Fallout 1's own Steam entry (Properties \u{2192} General \u{2192} Launch \
                     Options) so pressing Play there opens straight into Et Tu instead:",
                )
                .color(theme::MUTED)
                .small(),
            );
            ui.horizontal(|ui| {
                let (option_text, _verified) = steam_launch_option(Some("ettu"));
                let mut buf = option_text.clone();
                ui.add(egui::TextEdit::singleline(&mut buf).desired_width(420.0).interactive(false));
                if ui.button("Copy").clicked() {
                    ui.ctx().copy_text(option_text);
                }
            });
        }

        if matches!(campaign.id, "sonora" | "nevada") && is_installed {
            ui.add_space(8.0);
            if ui
                .button("Add to Steam\u{2026}")
                .on_hover_text(format!(
                    "Adds this launcher as a non-Steam game, forced into this campaign - {} has no \
                     Steam app of its own to paste launch options into.",
                    campaign.name
                ))
                .clicked()
            {
                app.pending_confirm = Some(PendingConfirm {
                    title: "Add to Steam".to_string(),
                    message: format!(
                        "Add this launcher to your Steam library as a non-Steam game, forced into {}? \
                         This edits Steam's own shortcuts.vdf (backed up first).",
                        campaign.name
                    ),
                    action: ConfirmAction::AddCampaignToSteam(campaign),
                });
            }
        }
    });
}

fn start_install(app: &mut App, campaign: &'static Campaign) {
    app.campaigns_busy.insert(campaign.id, "Installing\u{2026}".to_string());
    let target = *campaign;
    let installer = install_fn(campaign.id);
    app.jobs.spawn(move || {
        let paths = crate::gui::jobs::fresh_paths(target);
        JobOutcome::CampaignInstall(target.id, installer(&paths))
    });
}

fn start_fallout1_extract(app: &mut App) {
    if app.fallout1_path.trim().is_empty() {
        app.error_dialog = Some((
            "Fallout 1 data".to_string(),
            "Type the path to Fallout 1's MASTER.DAT below, then try again.".to_string(),
        ));
        return;
    }
    let master_dat = std::path::PathBuf::from(app.fallout1_path.trim());
    let target = *crate::backend::campaigns::by_id("ettu").expect("\"ettu\" is a registered campaign");
    app.campaigns_busy.insert("ettu-f1", "Extracting\u{2026}".to_string());
    app.jobs.spawn(move || {
        let paths = crate::gui::jobs::fresh_paths(target);
        JobOutcome::Fallout1Extract(ettu_install::extract_fallout1_data(&paths, &master_dat))
    });
}

pub(crate) fn add_to_steam_now(app: &mut App, campaign: &'static Campaign) {
    match add_campaign_to_steam(campaign) {
        Ok(_) => {
            app.info_dialog = Some((
                "Added to Steam".to_string(),
                "Restart Steam to see it in your library (under the non-Steam games section).".to_string(),
            ));
        }
        Err(e) => app.error_dialog = Some(("Couldn't add to Steam".to_string(), e)),
    }
}
