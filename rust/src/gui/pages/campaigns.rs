//! The Campaigns page: install, switch between, and manage every campaign this launcher
//! knows about (see `GLOSSARY.md`). The sidebar's campaign switcher covers picking an
//! already-installed one; this page is where you install a new one or fix up something
//! like Et Tu's Fallout 1 data. Ported from `gui/campaigns_page.py`.

use std::path::{Path, PathBuf};

use eframe::egui;

use crate::backend::campaigns::{Campaign, CAMPAIGNS};
use crate::backend::launch_option::steam_launch_option;
use crate::backend::paths::Paths;
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

fn install_fn(id: &'static str) -> fn(&crate::backend::paths::Paths, &dyn Fn(&str)) -> Result<(), String> {
    match id {
        "ce" => ce_install::install_ce,
        "ettu" => ettu_install::install_ettu,
        "sonora" => sonora_install::install_sonora,
        "nevada" => nevada_install::install_nevada,
        _ => unreachable!("every registered campaign id has an installer"),
    }
}

pub fn show(app: &mut App, ui: &mut egui::Ui) {
    ui.label(theme::bold("Campaigns").size(17.0).color(theme::STRONG_TEXT));
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
                if busy.is_some() {
                    crate::gui::widgets::busy_status(ui, app.job_stages.get(campaign.id).map(String::as_str));
                } else if ui.button("Install").clicked() {
                    start_install(app, campaign);
                }
            }
            if is_installed && !is_active && ui.button("Switch to this campaign").clicked() {
                app.switch_campaign(ui.ctx(), campaign);
            }
        });

        if campaign.id == "ettu" && is_installed {
            maybe_autodetect_fallout1(app);
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
                        .desired_width(300.0)
                        .hint_text("/path/to/Fallout 1/MASTER.DAT"),
                );
                if ui.button("Browse\u{2026}").clicked() {
                    browse_for_fallout1_master_dat(app);
                }
                let extracting = app.campaigns_busy.get("ettu-f1").is_some();
                if extracting {
                    crate::gui::widgets::busy_status(ui, app.job_stages.get("ettu-f1").map(String::as_str));
                } else if ui.button("Extract").clicked() {
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
    app.job_stages.insert(campaign.id.to_string(), "Downloading\u{2026}".to_string());
    let target = *campaign;
    let installer = install_fn(campaign.id);
    app.jobs.spawn(campaign.id, move |report| {
        let paths = crate::gui::jobs::fresh_paths(target);
        let result = installer(&paths, report).inspect(|()| {
            crate::backend::shared_settings::seed_new_campaign(&paths, &target);
        });
        JobOutcome::CampaignInstall(target.id, result)
    });
}

/// Fallout 1 and Fallout 2 are usually sibling folders under the same Steam `common/`
/// directory (e.g. `.../common/Fallout 1` next to `.../common/Fallout 2`). Tried once per
/// session, the first time this card is shown with an empty path field - doesn't re-run on
/// every frame, and never overwrites something the player already typed or picked.
fn maybe_autodetect_fallout1(app: &mut App) {
    if app.fallout1_autodetect_tried {
        return;
    }
    app.fallout1_autodetect_tried = true;
    if app.fallout1_path.trim().is_empty() {
        if let Some(master_dat) = detect_fallout1_master_dat(&app.paths) {
            app.fallout1_path = master_dat.to_string_lossy().into_owned();
        }
    }
}

/// Best-effort guess at a sibling Fallout 1 install's MASTER.DAT - `None` when nothing in
/// the game's parent directory matches. The "Browse..." file dialog is always available
/// regardless, so a missed or wrong guess here is never a dead end.
fn detect_fallout1_master_dat(paths: &Paths) -> Option<PathBuf> {
    let game_dir = paths.launcher_dir();
    let parent = game_dir.parent()?;
    let mut siblings: Vec<PathBuf> = std::fs::read_dir(parent)
        .ok()?
        .flatten()
        .map(|entry| entry.path())
        .filter(|path| path.is_dir() && path != game_dir)
        .collect();
    siblings.sort();
    siblings.into_iter().find_map(|dir| {
        let candidate = paths.ci_path("MASTER.DAT", Some(&dir));
        candidate.is_file().then_some(candidate)
    })
}

/// Open a native "pick a file" dialog for Fallout 1's MASTER.DAT, starting from the
/// autodetected folder (or the directory of whatever's already typed) when there is one.
fn browse_for_fallout1_master_dat(app: &mut App) {
    let mut dialog = rfd::FileDialog::new()
        .set_title("Locate Fallout 1's MASTER.DAT")
        .add_filter("MASTER.DAT", &["dat", "DAT"]);
    let start_dir = Path::new(app.fallout1_path.trim())
        .parent()
        .filter(|dir| dir.is_dir())
        .map(Path::to_path_buf)
        .or_else(|| app.paths.launcher_dir().parent().map(Path::to_path_buf));
    if let Some(dir) = start_dir {
        dialog = dialog.set_directory(dir);
    }
    if let Some(path) = dialog.pick_file() {
        app.fallout1_path = path.to_string_lossy().into_owned();
    }
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
    app.job_stages.insert("ettu-f1".to_string(), "Extracting\u{2026}".to_string());
    app.jobs.spawn("ettu-f1", move |_report| {
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
