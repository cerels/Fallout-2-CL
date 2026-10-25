//! The `eframe::App` itself: the window shell (campaign switcher, search, sidebar nav,
//! bottom save/discard/play bar) and the state every page needs. Pages live in
//! `gui::pages::*` as free functions taking `&mut App` - see each module's own doc comment.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use eframe::egui;

use crate::backend::campaigns::{activate, active_campaign, by_id, save_last_campaign_id, Campaign, CAMPAIGNS};
use crate::backend::ce_install::{install_ce, needs_ce_install};
use crate::backend::launch::launcher_prefs;
use crate::backend::mod_catalog::{load_mod_catalog, CatalogEntry};
use crate::backend::paths::SharedPaths;
use crate::backend::rpu_install::delete_rpu;
use crate::backend::schema::{load_catalog, Catalog};

use super::jobs::{fresh_paths, JobOutcome, Jobs};
use super::pages;
use super::pages::mods::ModsPage;
use super::pages::play::PlayPage;
use super::store::Store;
use super::theme;

/// What happens when a "are you sure?" modal is confirmed. One flat enum rather than a
/// callback-per-dialog per `docs/adr/0005-boring-rust.md` - there are only a few kinds of
/// confirmation in this whole app.
pub(crate) enum ConfirmAction {
    InstallCeThenPlay,
    AddCampaignToSteam(&'static Campaign),
    UninstallRpu,
    DeleteMod { dat: String, extra: Vec<String>, catalog_id: Option<String>, extract_to: Option<String> },
    Discard,
    SwitchCampaign(&'static Campaign),
}

pub(crate) struct PendingConfirm {
    pub title: String,
    pub message: String,
    pub action: ConfirmAction,
}

/// One sidebar entry. `Settings` pages are looked up by name each frame rather than by index
/// into `store.catalog.pages`, so a reload that changes the page set can't leave a stale
/// index pointing at the wrong page.
enum NavEntry {
    Play,
    Campaigns,
    Settings(String),
    Rpu,
    Mods,
    Launcher,
}

/// Which group of the Left Nav Dock an entry belongs to - see `docs/terminal-ui-blueprint.md`
/// \u{a7}3 ("Vertical distribution"). `Primary` entries get no divider/group label; `Config`
/// and `Util` each get one.
#[derive(PartialEq, Eq, Clone, Copy)]
enum NavTier {
    Primary,
    Config,
    Util,
}

impl NavEntry {
    fn key(&self) -> String {
        match self {
            NavEntry::Play => "play".to_string(),
            NavEntry::Campaigns => "campaigns".to_string(),
            NavEntry::Settings(name) => format!("settings:{name}"),
            NavEntry::Rpu => "rpu".to_string(),
            NavEntry::Mods => "mods".to_string(),
            NavEntry::Launcher => "launcher".to_string(),
        }
    }

    fn label(&self) -> &str {
        match self {
            NavEntry::Play => "Play",
            NavEntry::Campaigns => "Campaigns",
            NavEntry::Settings(name) => name,
            NavEntry::Rpu => "Restoration Project",
            NavEntry::Mods => "Mods",
            NavEntry::Launcher => "Launcher",
        }
    }

    fn tier(&self) -> NavTier {
        match self {
            NavEntry::Play | NavEntry::Campaigns => NavTier::Primary,
            NavEntry::Settings(_) => NavTier::Config,
            NavEntry::Rpu | NavEntry::Mods | NavEntry::Launcher => NavTier::Util,
        }
    }
}

/// `campaign` gates `NavEntry::Rpu` - RPU is a CE-only content overlay (see `GLOSSARY.md`),
/// so its tab must never show up after switching to Et Tu/Sonora/Nevada.
fn build_nav(catalog: &Catalog, campaign: &Campaign) -> Vec<NavEntry> {
    let mut nav = vec![NavEntry::Play, NavEntry::Campaigns];
    for page in &catalog.pages {
        if page.name == "Launcher" || page.name == "Restoration Project" || page.name == "Unofficial Patch" {
            continue;
        }
        nav.push(NavEntry::Settings(page.name.clone()));
    }
    if campaign.id == "ce" {
        nav.push(NavEntry::Rpu);
    }
    nav.push(NavEntry::Mods);
    nav.push(NavEntry::Launcher);
    nav
}

pub struct App {
    pub(crate) paths: SharedPaths,
    pub(crate) campaign: &'static Campaign,
    pub(crate) store: Store,
    pub(crate) mod_catalog: Vec<CatalogEntry>,
    nav: Vec<NavEntry>,
    current: usize,
    pub(crate) search: String,
    pub(crate) capturing_key: Option<String>,
    pub(crate) jobs: Jobs,
    /// Live status text for in-flight jobs, keyed the same way as `campaigns_busy`/`mods.busy`
    /// (a campaign id, a mod id, "rpu") - see `Jobs::spawn` and
    /// `docs/adr/0006-indeterminate-download-progress.md`.
    pub(crate) job_stages: HashMap<String, String>,
    pub(crate) play: PlayPage,
    pub(crate) mods: ModsPage,
    pub(crate) campaigns_busy: HashMap<&'static str, String>,
    pub(crate) fallout1_path: String,
    pub(crate) fallout1_path_error: Option<String>,
    pub(crate) fallout1_autodetect_tried: bool,
    pub(crate) rpu_pending_update: Option<String>,
    pub(crate) rpu_busy: bool,
    ce_installing: bool,
    pub(crate) needs_reload: bool,
    pub(crate) pending_confirm: Option<PendingConfirm>,
    pub(crate) error_dialog: Option<(String, String)>,
    pub(crate) info_dialog: Option<(String, String)>,
    want_close: bool,
    launch_result: Rc<RefCell<Option<(String, Option<u32>)>>>,
}

impl App {
    pub fn new(paths: SharedPaths, launch_result: Rc<RefCell<Option<(String, Option<u32>)>>>) -> Self {
        let campaign = active_campaign(&paths);
        let catalog = load_catalog(&paths);
        for err in &catalog.errors {
            eprintln!("schema error: {err}");
        }
        let mod_catalog = load_mod_catalog();
        let store = Store::new(&paths, catalog);
        let nav = build_nav(&store.catalog, campaign);
        let play = PlayPage::new(&paths);
        let mods = ModsPage::new(&paths, &mod_catalog, &store.catalog.mod_info, campaign.id);

        App {
            paths,
            campaign,
            store,
            mod_catalog,
            nav,
            current: 0,
            search: String::new(),
            capturing_key: None,
            jobs: Jobs::new(),
            job_stages: HashMap::new(),
            play,
            mods,
            campaigns_busy: HashMap::new(),
            fallout1_path: String::new(),
            fallout1_path_error: None,
            fallout1_autodetect_tried: false,
            rpu_pending_update: None,
            rpu_busy: false,
            ce_installing: false,
            needs_reload: false,
            pending_confirm: None,
            error_dialog: None,
            info_dialog: None,
            want_close: false,
            launch_result,
        }
    }

    /// Reloads the settings catalog, mod catalog, store, saves and mods list for whichever
    /// campaign is currently active - installing/deleting a mod or RPU can change which
    /// schema-backed settings pages exist, so the whole page set is rebuilt from scratch
    /// rather than patched in place. Any pending settings/mods edits are saved first so a
    /// reload triggered by an unrelated action (e.g. deleting a mod while Display settings
    /// are mid-edit) never silently discards them.
    fn rebuild_everything(&mut self) {
        self.save_all_best_effort();
        let current_key = self.nav.get(self.current).map(NavEntry::key);

        let catalog = load_catalog(&self.paths);
        for err in &catalog.errors {
            eprintln!("schema error: {err}");
        }
        self.mod_catalog = load_mod_catalog();
        self.store = Store::new(&self.paths, catalog);
        self.play = PlayPage::new(&self.paths);
        self.mods = ModsPage::new(&self.paths, &self.mod_catalog, &self.store.catalog.mod_info, self.campaign.id);
        self.nav = build_nav(&self.store.catalog, self.campaign);
        self.current = current_key.and_then(|k| self.nav.iter().position(|n| n.key() == k)).unwrap_or(0);
    }

    fn save_all_best_effort(&mut self) {
        if self.store.dirty_count() > 0 {
            let _ = self.store.save(&self.paths);
        }
        if self.mods.dirty_count() > 0 {
            let _ = self.mods.save(&self.paths);
        }
    }

    fn save_all(&mut self) -> Result<(), String> {
        self.store.save(&self.paths)?;
        self.mods.save(&self.paths)?;
        Ok(())
    }

    /// Entry point every "switch campaign" button should call instead of `switch_campaign`
    /// directly - asks first when there's something unsaved to lose, since switching reloads
    /// the whole settings/mods page set for the new campaign (see `switch_campaign`'s doc
    /// comment on `save_all_best_effort`, which is silent about a failed save).
    pub(crate) fn request_switch_campaign(&mut self, ctx: &egui::Context, campaign: &'static Campaign) {
        if self.dirty_count() > 0 {
            self.pending_confirm = Some(PendingConfirm {
                title: "Switch campaign".to_string(),
                message: format!(
                    "Switch to {}? Your unsaved changes will be saved first, but if that save fails \
                     for any reason they'll be lost silently.",
                    campaign.name
                ),
                action: ConfirmAction::SwitchCampaign(campaign),
            });
        } else {
            self.switch_campaign(ctx, campaign);
        }
    }

    pub(crate) fn switch_campaign(&mut self, ctx: &egui::Context, campaign: &'static Campaign) {
        self.save_all_best_effort();
        activate(&self.paths, campaign);
        if let Ok(mut prefs) = launcher_prefs(&self.paths) {
            let _ = save_last_campaign_id(&mut prefs, campaign.id);
        }
        self.campaign = campaign;
        // Re-tint the whole chrome to this campaign's accent - see
        // `docs/terminal-ui-blueprint.md` \u{a7}1 and `theme::install`'s doc comment.
        theme::install(ctx, theme::parse_hex(campaign.accent));
        self.rebuild_everything();
    }

    fn go_to_campaigns(&mut self) {
        if let Some(i) = self.nav.iter().position(|n| matches!(n, NavEntry::Campaigns)) {
            self.current = i;
        }
    }

    fn dirty_count(&self) -> usize {
        self.store.dirty_count() + self.mods.dirty_count()
    }

    /// (button label, install-needed, subtitle line) - mirrors the old Qt build's
    /// `update_play`.
    fn play_status(&self) -> (String, bool, String) {
        let headline = |base: &str| {
            if self.store.catalog.headlines.is_empty() {
                base.to_string()
            } else {
                format!("{base} \u{b7} {}", self.store.catalog.headlines.join(" \u{b7} "))
            }
        };
        if self.campaign.id == "ce" {
            let install_needed = needs_ce_install(&self.paths);
            let subtitle = if install_needed {
                "Community Edition \u{2014} not installed".to_string()
            } else {
                headline("Community Edition")
            };
            let label = if self.ce_installing {
                "Installing CE\u{2026}".to_string()
            } else if install_needed {
                "Install CE".to_string()
            } else {
                self.play.play_label()
            };
            (label, install_needed, subtitle)
        } else {
            let install_needed = !self.campaign.is_installed(&self.paths);
            let subtitle =
                if install_needed { "Not installed \u{2014} use the Campaigns page".to_string() } else { headline(self.campaign.name) };
            let label = if install_needed { "Go to Campaigns".to_string() } else { self.play.play_label() };
            (label, install_needed, subtitle)
        }
    }

    fn on_play_or_install_clicked(&mut self, install_needed: bool) {
        if install_needed && self.campaign.id != "ce" {
            self.go_to_campaigns();
            return;
        }
        if self.dirty_count() > 0 {
            if let Err(e) = self.save_all() {
                self.error_dialog = Some(("Couldn't save settings".to_string(), super::errors::friendly(&e)));
                return;
            }
        }
        if self.campaign.id == "ce" && needs_ce_install(&self.paths) {
            self.pending_confirm = Some(PendingConfirm {
                title: "Install Fallout 2 CE".to_string(),
                message: "This looks like a vanilla Fallout 2 install, but this launcher needs the \
                           Community Edition engine to run the game. Download and install it now?"
                    .to_string(),
                action: ConfirmAction::InstallCeThenPlay,
            });
            return;
        }
        if !self.paths.game_bin().is_file() {
            if !self.campaign.is_installed(&self.paths) {
                self.info_dialog = Some((
                    "Not installed".to_string(),
                    format!("{} isn't installed yet - use the Campaigns page.", self.campaign.name),
                ));
                self.go_to_campaigns();
            } else {
                self.error_dialog =
                    Some(("Game not found".to_string(), format!("{} is missing.", self.paths.game_bin().display())));
            }
            return;
        }
        *self.launch_result.borrow_mut() = Some((self.play.mode.clone(), self.play.slot));
        self.want_close = true;
    }

    fn apply_confirm(&mut self, action: ConfirmAction, ctx: &egui::Context) {
        match action {
            ConfirmAction::Discard => {
                self.store.revert();
                self.mods.revert(&self.paths, &self.mod_catalog, &self.store.catalog.mod_info, self.campaign.id);
            }
            ConfirmAction::SwitchCampaign(campaign) => self.switch_campaign(ctx, campaign),
            ConfirmAction::InstallCeThenPlay => {
                self.ce_installing = true;
                self.job_stages.insert("ce".to_string(), "Downloading\u{2026}".to_string());
                let ce = *by_id("ce").expect("\"ce\" is a registered campaign");
                self.jobs.spawn("ce", move |report| {
                    let paths = fresh_paths(ce);
                    let result = install_ce(&paths, report).inspect(|()| {
                        crate::backend::shared_settings::seed_new_campaign(&paths, &ce);
                    });
                    JobOutcome::CeInstall(result)
                });
            }
            ConfirmAction::AddCampaignToSteam(campaign) => pages::campaigns::add_to_steam_now(self, campaign),
            ConfirmAction::UninstallRpu => {
                delete_rpu(&self.paths);
                self.needs_reload = true;
            }
            ConfirmAction::DeleteMod { dat, extra, catalog_id, extract_to } => {
                pages::mods::delete_mod_now(self, &dat, &extra, catalog_id, extract_to)
            }
        }
    }

    fn handle_job_outcome(&mut self, outcome: JobOutcome) {
        match outcome {
            JobOutcome::CeInstall(result) => {
                self.ce_installing = false;
                self.job_stages.remove("ce");
                match result {
                    Ok(()) => {
                        self.info_dialog = Some((
                            "Fallout 2 CE installed".to_string(),
                            "Fallout 2 CE is installed. Press Play when you're ready.".to_string(),
                        ))
                    }
                    Err(e) => self.error_dialog = Some(("Install failed".to_string(), super::errors::friendly(&e))),
                }
            }
            JobOutcome::CampaignInstall(id, result) => {
                self.campaigns_busy.remove(id);
                self.job_stages.remove(id);
                if let Err(e) = result {
                    let name = by_id(id).map(|c| c.name).unwrap_or(id);
                    self.error_dialog = Some(("Install failed".to_string(), format!("{name}: {}", super::errors::friendly(&e))));
                }
            }
            JobOutcome::Fallout1Extract(result) => {
                self.campaigns_busy.remove("ettu-f1");
                self.job_stages.remove("ettu-f1");
                match result {
                    Ok(()) => {
                        self.info_dialog = Some((
                            "Fallout 1 data extracted".to_string(),
                            "Done - Et Tu is ready to play.".to_string(),
                        ))
                    }
                    Err(e) => self.error_dialog = Some(("Extraction failed".to_string(), super::errors::friendly(&e))),
                }
            }
            JobOutcome::RpuInstall(result) => {
                self.rpu_busy = false;
                self.job_stages.remove("rpu");
                match result {
                    Ok(()) => self.needs_reload = true,
                    Err(e) => {
                        self.error_dialog =
                            Some(("Install failed".to_string(), format!("Restoration Project: {}", super::errors::friendly(&e))))
                    }
                }
            }
            JobOutcome::RpuUpdateCheck(version) => {
                self.rpu_busy = false;
                self.job_stages.remove("rpu");
                match version {
                    Some(v) => self.rpu_pending_update = Some(v),
                    None => self.info_dialog = Some(("Restoration Project".to_string(), "Already up to date.".to_string())),
                }
            }
            JobOutcome::ModInstall(id, result) => self.handle_mod_install(id, result),
            JobOutcome::ModUpdateCheck(id, version) => self.handle_mod_update_check(id, version),
        }
    }
}

impl eframe::App for App {
    /// eframe 0.37 hands the root call a plain `egui::Ui` (no margin/background) rather than
    /// a `&Context` - panels are still attached the usual way via `ctx`, just fetched off
    /// this `ui` first (`Context` is cheap to clone, it's `Rc`-based internally).
    fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        let ctx = root_ui.ctx().clone();

        for (key, stage) in self.jobs.poll_stages() {
            self.job_stages.insert(key, stage);
        }
        for outcome in self.jobs.poll() {
            self.handle_job_outcome(outcome);
        }
        if self.needs_reload {
            self.needs_reload = false;
            self.rebuild_everything();
        }
        if self.jobs.in_flight() {
            ctx.request_repaint_after(std::time::Duration::from_millis(150));
        }

        self.draw_status_bar(root_ui);
        self.draw_nav(root_ui);
        self.draw_bottom_bar(root_ui);
        egui::CentralPanel::default().show(root_ui, |ui| {
            egui::ScrollArea::vertical().show(ui, |ui| {
                ui.set_width(ui.available_width());
                self.draw_current_page(ui, &ctx);
            });
        });
        self.draw_modals(&ctx);

        if self.want_close {
            ctx.send_viewport_cmd(egui::ViewportCommand::Close);
        }
    }
}

impl App {
    /// Session metadata: campaign-independent, so it lives in its own top panel rather than
    /// sharing the bottom bar with LAUNCH (`docs/terminal-ui-blueprint.md` \u{a7}2/\u{a7}4).
    fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::top("status_bar")
            .exact_size(40.0)
            .frame(egui::Frame::default().fill(theme::bg_panel()).inner_margin(egui::Margin::symmetric(14, 0)))
            .show(ui, |ui| {
                ui.horizontal_centered(|ui| {
                    let dirty = self.dirty_count();
                    if dirty > 0 {
                        ui.label(
                            theme::mono_bold(format!("\u{25cf} {dirty} UNSAVED"))
                                .color(theme::AMBER),
                        );
                        if ui.button(theme::mono("[ SAVE ]")).clicked() {
                            if let Err(e) = self.save_all() {
                                self.error_dialog = Some(("Couldn't save settings".to_string(), super::errors::friendly(&e)));
                            }
                        }
                        if ui.button(theme::mono("[ DISCARD ]")).clicked() {
                            self.pending_confirm = Some(PendingConfirm {
                                title: "Discard changes".to_string(),
                                message: format!("Throw away {dirty} unsaved change{}? This can't be undone.", if dirty == 1 { "" } else { "s" }),
                                action: ConfirmAction::Discard,
                            });
                        }
                    } else {
                        ui.label(theme::mono("ALL SAVED").color(theme::MUTED));
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(theme::mono(format!("BUILD {}", env!("CARGO_PKG_VERSION"))).color(theme::MUTED));
                    });
                });
            });
    }

    fn draw_nav(&mut self, ui: &mut egui::Ui) {
        let accent = theme::parse_hex(self.campaign.accent);
        egui::Panel::left("nav")
            .resizable(false)
            .exact_size(250.0)
            .frame(egui::Frame::default().fill(theme::bg_panel()).inner_margin(egui::Margin::same(8)))
            .show(ui, |ui| {
                self.draw_nav_cover_wash(ui);
                ui.add_space(8.0);
                self.draw_campaign_switcher(ui);
                ui.add_space(8.0);
                ui.add(
                    egui::TextEdit::singleline(&mut self.search).hint_text("> search\u{2026}").font(egui::TextStyle::Monospace).desired_width(f32::INFINITY),
                );
                ui.add_space(6.0);
                egui::ScrollArea::vertical().show(ui, |ui| {
                    let search = self.search.trim().to_lowercase();
                    let mut shown_tier: Option<NavTier> = None;
                    for i in 0..self.nav.len() {
                        let tier = self.nav[i].tier();
                        if shown_tier.as_ref() != Some(&tier) && tier != NavTier::Primary {
                            ui.add_space(10.0);
                            ui.label(theme::mono(match tier {
                                NavTier::Config => "CONFIG",
                                NavTier::Util => "UTIL",
                                NavTier::Primary => unreachable!(),
                            }).color(theme::MUTED).small());
                            ui.separator();
                        }
                        shown_tier = Some(tier);

                        let needs_attention = self.nav_needs_attention(i);
                        let base_label = match &self.nav[i] {
                            NavEntry::Settings(name) => {
                                let hits = self
                                    .store
                                    .catalog
                                    .pages
                                    .iter()
                                    .find(|p| &p.name == name)
                                    .map(|p| pages::settings::count_matches(&p.groups, &search))
                                    .unwrap_or(0);
                                if search.is_empty() {
                                    name.clone()
                                } else {
                                    format!("{name}  ({hits})")
                                }
                            }
                            other => other.label().to_string(),
                        };
                        let label = base_label.to_uppercase();
                        let text = if needs_attention { theme::mono(label).color(theme::AMBER) } else { theme::mono(label) };
                        let hits_zero = matches!(&self.nav[i], NavEntry::Settings(name) if {
                            let hits = self.store.catalog.pages.iter().find(|p| &p.name == name)
                                .map(|p| pages::settings::count_matches(&p.groups, &search)).unwrap_or(0);
                            hits == 0
                        });
                        let enabled = search.is_empty() || !hits_zero;
                        ui.add_enabled_ui(enabled, |ui| {
                            if ui.selectable_label(self.current == i, text).clicked() {
                                self.current = i;
                            }
                        });
                    }
                });
            });
        egui::Panel::left("nav_accent_rule").resizable(false).exact_size(2.0).frame(egui::Frame::default().fill(accent)).show(ui, |_ui| {});
    }

    /// Experimental: a faint wash of the active campaign's cover art behind the whole nav
    /// panel, re-tinting the sidebar by campaign beyond just `accent` (ADR 0008 already
    /// drives chrome color from it; this layers the cover's own shape/color underneath at
    /// low opacity, painted first so every widget added after still draws on top of it).
    /// Keep only if it reads as atmosphere, not noise - see docs/adr/0010-campaign-cover-art.md.
    fn draw_nav_cover_wash(&self, ui: &egui::Ui) {
        let rect = ui.max_rect();
        egui::Image::from_bytes(self.campaign.cover_uri, self.campaign.cover)
            .tint(egui::Color32::from_white_alpha(45))
            .paint_at(ui, rect);
    }

    /// Whether nav row `i` needs a player's attention right now - drawn as its label turning
    /// `AMBER` instead of a `[!]` prefix (brackets stay reserved for the campaign badge; see
    /// `docs/terminal-ui-blueprint.md` \u{a7}3's bracket-sigil rules, revised to color-only for
    /// the nav list after it proved busier than the "modern and sleek" half of the brief
    /// wanted).
    fn nav_needs_attention(&self, i: usize) -> bool {
        match &self.nav[i] {
            NavEntry::Play if self.campaign.id == "ce" => needs_ce_install(&self.paths),
            NavEntry::Play => !self.campaign.is_installed(&self.paths),
            NavEntry::Rpu => self.rpu_pending_update.is_some(),
            _ => false,
        }
    }

    fn draw_campaign_switcher(&mut self, ui: &mut egui::Ui) {
        let campaign = self.campaign;
        ui.horizontal(|ui| {
            let accent = theme::parse_hex(campaign.accent);
            ui.label(theme::mono_bold(format!("[{}]", campaign.badge)).color(accent));
            ui.menu_button(theme::mono(format!("{} \u{25be}", campaign.name.to_uppercase())), |ui| {
                for c in CAMPAIGNS {
                    let installed = c.is_installed(&self.paths);
                    let text = if installed { format!("[{}] {}", c.badge, c.name) } else { format!("[{}] {} (not installed)", c.badge, c.name) };
                    let enabled = installed && c.id != self.campaign.id;
                    ui.add_enabled_ui(enabled, |ui| {
                        if ui.button(theme::mono(text)).clicked() {
                            let ctx = ui.ctx().clone();
                            self.request_switch_campaign(&ctx, c);
                            ui.close();
                        }
                    });
                }
                ui.separator();
                if ui.button(theme::mono("Manage campaigns\u{2026}")).clicked() {
                    self.go_to_campaigns();
                    ui.close();
                }
            });
        });
        let (_, install_needed, subtitle) = self.play_status();
        ui.label(
            egui::RichText::new(subtitle).small().color(if install_needed { theme::DANGER } else { theme::MUTED }),
        );
    }

    /// Isolated, single-purpose: nothing competes with LAUNCH for attention here - see
    /// `docs/terminal-ui-blueprint.md` \u{a7}2/\u{a7}4.
    fn draw_bottom_bar(&mut self, ui: &mut egui::Ui) {
        egui::Panel::bottom("action_bar")
            .exact_size(96.0)
            .frame(egui::Frame::default().fill(theme::bg_panel()))
            .show(ui, |ui| {
                let (label, install_needed, _) = self.play_status();
                let accent = theme::accent(ui);
                let busy = self.ce_installing;
                let size = egui::vec2(360.0, 64.0);

                ui.add_space(16.0);
                let clicked = ui
                    .horizontal(|ui| {
                        let pad = ((ui.available_width() - size.x) / 2.0).max(0.0);
                        ui.add_space(pad);
                        let button = theme::primary_button(egui::RichText::new(label).size(18.0), install_needed, accent);
                        ui.add_enabled_ui(!busy, |ui| ui.add_sized(size, button)).inner.clicked()
                    })
                    .inner;
                ui.add_space(16.0);

                if clicked {
                    self.on_play_or_install_clicked(install_needed);
                }
            });
    }

    fn draw_current_page(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) {
        let Some(entry_key) = self.nav.get(self.current).map(NavEntry::key) else { return };
        match entry_key.as_str() {
            "play" => {
                if self.play.show(ui, ctx) {
                    self.on_play_or_install_clicked(false);
                }
            }
            "campaigns" => pages::campaigns::show(self, ui),
            "rpu" => pages::rpu::show(self, ui),
            "mods" => pages::mods::show(self, ui),
            "launcher" => pages::launcher_info::show(self, ui),
            _ => {
                let Some(NavEntry::Settings(name)) = self.nav.get(self.current) else { return };
                let name = name.clone();
                let search = self.search.trim().to_lowercase();
                if let Some(page) = self.store.catalog.pages.iter().find(|p| p.name == name).cloned() {
                    ui.label(theme::bold(&page.name).size(17.0).color(theme::STRONG_TEXT));
                    ui.add_space(8.0);
                    pages::settings::show(ui, &mut self.store, &page.groups, &page.intro, &mut self.capturing_key, &search);
                }
            }
        }
    }

    fn draw_modals(&mut self, ctx: &egui::Context) {
        if let Some((title, message)) = self.error_dialog.clone() {
            let mut open = true;
            egui::Modal::new(egui::Id::unique("error_dialog")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(theme::bold(&title).color(theme::DANGER).size(15.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(message).color(theme::TEXT));
                ui.add_space(12.0);
                if ui.button("OK").clicked() {
                    open = false;
                }
            });
            if !open {
                self.error_dialog = None;
            }
            return;
        }
        if let Some((title, message)) = self.info_dialog.clone() {
            let mut open = true;
            egui::Modal::new(egui::Id::unique("info_dialog")).show(ctx, |ui| {
                ui.set_max_width(420.0);
                ui.label(theme::bold(&title).color(theme::SUCCESS).size(15.0));
                ui.add_space(8.0);
                ui.label(egui::RichText::new(message).color(theme::TEXT));
                ui.add_space(12.0);
                if ui.button("OK").clicked() {
                    open = false;
                }
            });
            if !open {
                self.info_dialog = None;
            }
            return;
        }
        if self.pending_confirm.is_some() {
            let mut choice = None;
            {
                let confirm = self.pending_confirm.as_ref().unwrap();
                egui::Modal::new(egui::Id::unique("confirm_dialog")).show(ctx, |ui| {
                    ui.set_max_width(420.0);
                    ui.label(theme::bold(&confirm.title).color(theme::STRONG_TEXT).size(15.0));
                    ui.add_space(8.0);
                    ui.label(egui::RichText::new(&confirm.message).color(theme::TEXT));
                    ui.add_space(12.0);
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Yes").clicked() {
                            choice = Some(true);
                        }
                        if ui.button("No").clicked() {
                            choice = Some(false);
                        }
                    });
                });
            }
            if let Some(yes) = choice {
                let confirm = self.pending_confirm.take().unwrap();
                if yes {
                    self.apply_confirm(confirm.action, ctx);
                }
            }
        }
    }
}
