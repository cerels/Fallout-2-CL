//! The Play page: continue/menu choice and the save-game picker with thumbnails. Ported
//! from the Python `gui/play_page.py`; the manual grid-column math it needed for Qt's
//! retained layout is gone - `egui::Ui::horizontal_wrapped` reflows on its own every frame.

use std::collections::HashMap;
use std::path::PathBuf;

use eframe::egui;

use crate::backend::dat::DatCache;
use crate::backend::paths::{SharedPaths, PREVIEW_H, PREVIEW_W};
use crate::backend::saves::{load_palette, read_saves, SaveGame};
use crate::gui::theme;
use crate::gui::widgets::card;

pub struct PlayPage {
    pub saves: Vec<SaveGame>,
    pub mode: String,
    pub slot: Option<u32>,
    palette: Option<Vec<(u8, u8, u8)>>,
    textures: HashMap<PathBuf, egui::TextureHandle>,
}

impl PlayPage {
    pub fn new(paths: &SharedPaths) -> Self {
        let saves = read_saves(paths);
        let dat_cache = DatCache::new();
        let palette = load_palette(paths, &dat_cache);
        let (mode, slot) = match saves.first() {
            Some(s) => ("continue".to_string(), Some(s.slot)),
            None => ("menu".to_string(), None),
        };
        PlayPage { saves, mode, slot, palette, textures: HashMap::new() }
    }

    pub fn play_label(&self) -> String {
        match self.mode.as_str() {
            "menu" => "PLAY  \u{25b6}".to_string(),
            "continue" => "CONTINUE  \u{25b6}".to_string(),
            _ => format!("LOAD SLOT {}  \u{25b6}", self.slot.unwrap_or(0)),
        }
    }

    fn selected(&self) -> Option<&SaveGame> {
        self.slot.and_then(|slot| self.saves.iter().find(|s| s.slot == slot))
    }

    fn choose(&mut self, mode: &str, slot: Option<u32>) {
        self.mode = mode.to_string();
        self.slot = if mode == "continue" { self.saves.first().map(|s| s.slot) } else { slot };
    }

    fn texture_for(&mut self, ctx: &egui::Context, save: &SaveGame) -> Option<egui::TextureHandle> {
        if let Some(t) = self.textures.get(&save.path) {
            return Some(t.clone());
        }
        let thumb = save.thumbnail.as_ref()?;
        let palette = self.palette.as_ref()?;
        let (w, h) = (PREVIEW_W as usize, PREVIEW_H as usize);
        if thumb.len() < w * h {
            return None;
        }
        let pixels: Vec<egui::Color32> = thumb[..w * h]
            .iter()
            .map(|&idx| {
                let (r, g, b) = palette.get(idx as usize).copied().unwrap_or((0, 0, 0));
                egui::Color32::from_rgb(r, g, b)
            })
            .collect();
        let image = egui::ColorImage::new([w, h], pixels);
        let handle = ctx.load_texture(format!("save-{}", save.slot), image, egui::TextureOptions::NEAREST);
        self.textures.insert(save.path.clone(), handle.clone());
        Some(handle)
    }

    /// Draws the page. Returns `true` if a save was double-clicked - the caller (the bottom
    /// bar's Play button lives in `App`, not here) treats that the same as pressing Play.
    pub fn show(&mut self, ui: &mut egui::Ui, ctx: &egui::Context) -> bool {
        let mut play_now = false;

        card(ui, "Continue", |ui| {
            ui.horizontal(|ui| {
                let selected = self.selected().cloned();
                if let Some(save) = &selected {
                    if let Some(tex) = self.texture_for(ctx, save) {
                        let size = egui::vec2(PREVIEW_W as f32 * 1.4, PREVIEW_H as f32 * 1.4);
                        ui.add(egui::Image::new(&tex).fit_to_exact_size(size));
                    }
                }
                ui.vertical(|ui| {
                    match &selected {
                        Some(save) => {
                            ui.label(theme::bold(&save.name).size(16.0).color(theme::STRONG_TEXT));
                            ui.label(egui::RichText::new(&save.location).color(theme::ACCENT));
                            if !save.description.is_empty() {
                                ui.label(
                                    egui::RichText::new(format!("\u{201c}{}\u{201d}", save.description))
                                        .color(theme::MUTED),
                                );
                            }
                            ui.label(
                                egui::RichText::new(format!(
                                    "{} \u{b7} Slot {} \u{b7} saved {}",
                                    save.game_date,
                                    save.slot,
                                    time_ago(save.mtime)
                                ))
                                .color(theme::MUTED),
                            );
                        }
                        None => {
                            ui.label(theme::bold("Main menu").size(16.0).color(theme::STRONG_TEXT));
                            ui.label(
                                egui::RichText::new("Start a new game, load a save, or change options in-game.")
                                    .color(theme::MUTED),
                            );
                        }
                    }
                    ui.add_space(8.0);
                    ui.horizontal(|ui| {
                        let can_continue = !self.saves.is_empty();
                        ui.add_enabled_ui(can_continue, |ui| {
                            if ui.selectable_label(self.mode == "continue", "Continue latest save").clicked() {
                                self.choose("continue", None);
                            }
                        });
                        if ui.selectable_label(self.mode == "menu", "Start at main menu").clicked() {
                            self.choose("menu", None);
                        }
                    });
                });
            });
        });

        ui.add_space(12.0);
        card(ui, &format!("Saved games ({})", self.saves.len()), |ui| {
            if self.saves.is_empty() {
                ui.label(
                    egui::RichText::new("No saves yet. Start at the main menu and create a new character.")
                        .color(theme::MUTED),
                );
                return;
            }
            ui.label(
                egui::RichText::new("Click a save to load it instead, or double-click to start right away.")
                    .color(theme::MUTED),
            );
            ui.add_space(6.0);
            let slots: Vec<u32> = self.saves.iter().map(|s| s.slot).collect();
            ui.horizontal_wrapped(|ui| {
                for slot in slots {
                    let save = self.saves.iter().find(|s| s.slot == slot).unwrap().clone();
                    let selected = self.slot == Some(save.slot);
                    let tex = self.texture_for(ctx, &save);
                    let response = save_card(ui, &save, tex.as_ref(), selected);
                    if response.clicked() {
                        self.choose("slot", Some(save.slot));
                    }
                    if response.double_clicked() {
                        self.choose("slot", Some(save.slot));
                        play_now = true;
                    }
                }
            });
        });

        play_now
    }
}

fn save_card(
    ui: &mut egui::Ui,
    save: &SaveGame,
    texture: Option<&egui::TextureHandle>,
    selected: bool,
) -> egui::Response {
    let width = PREVIEW_W as f32 + 24.0;
    let fill = if selected { egui::Color32::from_rgb(0x17, 0x26, 0x1a) } else { egui::Color32::from_rgb(0x0f, 0x17, 0x11) };
    let stroke_color = if selected { theme::ACCENT } else { egui::Color32::from_rgb(0x1f, 0x33, 0x24) };

    let frame = egui::Frame::default()
        .fill(fill)
        .stroke(egui::Stroke::new(if selected { 2.0 } else { 1.0 }, stroke_color))
        .corner_radius(14.0)
        .inner_margin(egui::Margin::same(12));

    let response = frame
        .show(ui, |ui| {
            ui.set_width(width - 24.0);
            if let Some(tex) = texture {
                ui.add(egui::Image::new(tex).fit_to_exact_size(egui::vec2(PREVIEW_W as f32, PREVIEW_H as f32)));
            } else {
                let (rect, _) =
                    ui.allocate_exact_size(egui::vec2(PREVIEW_W as f32, PREVIEW_H as f32), egui::Sense::hover());
                ui.painter().rect_filled(rect, 4.0, egui::Color32::from_rgb(0x0a, 0x0f, 0x0b));
            }
            ui.label(theme::bold(format!("{}  \u{b7} slot {}", save.name, save.slot)).color(theme::STRONG_TEXT));
            let loc = if save.description.is_empty() {
                save.location.clone()
            } else {
                format!("{} \u{2014} {}", save.location, save.description)
            };
            ui.label(egui::RichText::new(loc).color(theme::ACCENT).small());
            ui.label(
                egui::RichText::new(format!("{} \u{b7} saved {}", save.game_date, time_ago(save.mtime)))
                    .color(theme::MUTED)
                    .small(),
            );
        })
        .response;

    response.interact(egui::Sense::click()).on_hover_cursor(egui::CursorIcon::PointingHand)
}

fn time_ago(mtime: std::time::SystemTime) -> String {
    let elapsed = mtime.elapsed().unwrap_or_default();
    let mins = elapsed.as_secs() / 60;
    if mins < 1 {
        "just now".to_string()
    } else if mins < 60 {
        format!("{mins} min ago")
    } else if mins < 60 * 24 {
        format!("{} h ago", mins / 60)
    } else {
        format!("{} days ago", mins / (60 * 24))
    }
}
