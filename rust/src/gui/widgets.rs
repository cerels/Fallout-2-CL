//! Reusable building blocks with no dependency on any particular page: the card frame every
//! settings group sits in, the toggle switch, and `setting_row` - the schema-driven control
//! renderer that is egui's whole reason for being picked (see
//! `docs/adr/0004-rust-rewrite.md`): one function that loops over a `Setting` and calls the
//! right `ui.*` widget for its `kind`, no per-setting code anywhere.

use eframe::egui;

use crate::backend::schema::Setting;
use crate::gui::keys_egui::{egui_key_to_dik, modifier_dik};
use crate::gui::store::Store;
use crate::gui::theme;

/// A titled card, matching the old Qt build's `card()` helper: a soft panel with an
/// uppercase, accent-colored title, holding whatever `add_contents` draws inside it.
pub fn card(ui: &mut egui::Ui, title: &str, add_contents: impl FnOnce(&mut egui::Ui)) {
    egui::Frame::default()
        .fill(theme::card_bg())
        .stroke(egui::Stroke::new(1.0, theme::stroke_color()))
        .corner_radius(14.0)
        .inner_margin(egui::Margin { left: 16, right: 16, top: 12, bottom: 12 })
        .show(ui, |ui| {
            ui.label(theme::bold(title.to_uppercase()).color(theme::ACCENT).size(12.5));
            ui.add_space(6.0);
            add_contents(ui);
        });
}

/// A small spinner followed by `stage` (or a generic "Working…" when the job hasn't reported
/// a specific stage yet) - the inline "something's happening" indicator for a download/install
/// job, drawn in place of its triggering button (see
/// `docs/adr/0006-indeterminate-download-progress.md`).
pub fn busy_status(ui: &mut egui::Ui, stage: Option<&str>) {
    ui.horizontal(|ui| {
        ui.add(egui::Spinner::new().size(14.0));
        ui.label(egui::RichText::new(stage.unwrap_or("Working\u{2026}")).color(theme::MUTED));
    });
}

/// A small on/off switch, standing in for the old Qt build's animated `Toggle` - the
/// textbook egui "toggle switch" shape (a sliding knob on a pill-shaped track), themed with
/// the Pip-Boy accent color when on.
pub fn toggle_switch(ui: &mut egui::Ui, on: &mut bool) -> egui::Response {
    let size = egui::vec2(40.0, 22.0);
    let (rect, mut response) = ui.allocate_exact_size(size, egui::Sense::click());
    if response.clicked() {
        *on = !*on;
        response.mark_changed();
    }
    if ui.is_rect_visible(rect) {
        let t = ui.ctx().animate_bool(response.id, *on);
        let off_color = egui::Color32::from_rgb(0x2a, 0x3a, 0x2e);
        let track = off_color.lerp_to_gamma(theme::ACCENT, t);
        let radius = rect.height() / 2.0;
        ui.painter().rect_filled(rect, radius, track);
        let knob_x = egui::lerp((rect.left() + radius)..=(rect.right() - radius), t);
        let knob_center = egui::pos2(knob_x, rect.center().y);
        let knob_color = if ui.is_enabled() {
            egui::Color32::from_rgb(0xf2, 0xfb, 0xf3)
        } else {
            egui::Color32::from_rgb(0x6f, 0x7f, 0x72)
        };
        ui.painter().circle_filled(knob_center, radius - 3.0, knob_color);
    }
    response
}

const RESOLUTION_PRESETS: &[&str] = &[
    "1280x720",
    "1366x768",
    "1600x900",
    "1920x1080",
    "1920x1200",
    "2560x1080",
    "2560x1440",
    "3440x1440",
    "3840x2160",
];

/// Whether a setting matches a lowercased search string - against its label, help, note and
/// raw key, same fields the old Qt build's `SettingRow.search_text` indexed. Callers filter
/// a group's settings with this *before* drawing, so an empty group's card can be skipped
/// entirely instead of being drawn with nothing in it.
pub fn setting_matches(s: &Setting, search: &str) -> bool {
    search.is_empty()
        || format!("{} {} {} {}", s.label, s.help, s.note, s.key).to_lowercase().contains(search)
}

/// Draw one settings row: label/help/note on the left, the right control for `s.kind` on the
/// right, and a reset-to-default button when the value has been changed. Callers are
/// expected to have already filtered with `setting_matches`.
pub fn setting_row(ui: &mut egui::Ui, s: &Setting, store: &mut Store, capturing: &mut Option<String>, first: bool) {
    let sid = s.id();
    let file_ok = store.file_ok(s);
    let enabled_by_dep = match &s.enabled_if {
        Some(cond) => {
            let dep_value = cond.setting.as_deref().map(|id| store.value(id).to_string()).unwrap_or_default();
            s.enabled_by(&dep_value)
        }
        None => true,
    };
    let enabled = file_ok && enabled_by_dep;

    if !first {
        ui.add_space(2.0);
        ui.separator();
        ui.add_space(2.0);
    }

    ui.add_enabled_ui(enabled, |ui| {
        egui::Sides::new().shrink_left().show(
            ui,
            |ui| {
                ui.vertical(|ui| {
                    ui.label(theme::bold(&s.label).color(theme::STRONG_TEXT));
                    if !s.help.is_empty() {
                        ui.label(egui::RichText::new(&s.help).color(theme::MUTED).small());
                    }
                    if !s.note.is_empty() {
                        ui.label(egui::RichText::new(&s.note).color(theme::AMBER).small());
                    }
                });
            },
            |ui| {
                ui.horizontal(|ui| {
                    let default = store.default_of(s);
                    let differs = file_ok && default.as_deref().is_some_and(|d| d != store.value(&sid));
                    if differs
                        && ui
                            .add(egui::Button::new("\u{21ba}").frame(false))
                            .on_hover_text("Reset to default")
                            .clicked()
                    {
                        if let Some(d) = default {
                            store.set(&sid, &d);
                        }
                    }
                    draw_control(ui, s, store, capturing);
                });
            },
        );
    });
}

fn draw_control(ui: &mut egui::Ui, s: &Setting, store: &mut Store, capturing: &mut Option<String>) {
    let sid = s.id();
    match s.kind.as_str() {
        "bool" => {
            let mut on = store.value(&sid) == "1";
            if toggle_switch(ui, &mut on).clicked() {
                store.set(&sid, if on { "1" } else { "0" });
            }
        }
        "choice" => {
            let current = store.value(&sid).to_string();
            let label = s
                .options
                .iter()
                .find(|(v, _)| v == &current)
                .map(|(_, l)| l.clone())
                .unwrap_or_else(|| format!("Custom ({current})"));
            egui::ComboBox::from_id_salt(&sid).selected_text(label).width(220.0).show_ui(ui, |ui| {
                for (value, opt_label) in &s.options {
                    if ui.selectable_label(value == &current, opt_label).clicked() {
                        store.set(&sid, value);
                    }
                }
            });
        }
        "resolution" => draw_resolution(ui, &sid, store),
        "key" => draw_key_capture(ui, s, &sid, store, capturing),
        "volume" => draw_volume(ui, &sid, store),
        "int" => draw_number(ui, s, &sid, store, false),
        "float" => draw_number(ui, s, &sid, store, true),
        _ => {
            let mut buf = store.value(&sid).to_string();
            if ui.add(egui::TextEdit::singleline(&mut buf).desired_width(220.0)).changed() {
                store.set(&sid, &buf);
            }
        }
    }
}

fn draw_resolution(ui: &mut egui::Ui, sid: &str, store: &mut Store) {
    let current = store.value(sid).to_string();
    let is_preset = RESOLUTION_PRESETS.contains(&current.as_str());
    let selected_text = if is_preset { current.replace('x', " \u{d7} ") } else { "Custom\u{2026}".to_string() };
    egui::ComboBox::from_id_salt(sid).selected_text(selected_text).width(160.0).show_ui(ui, |ui| {
        for preset in RESOLUTION_PRESETS {
            if ui.selectable_label(*preset == current, preset.replace('x', " \u{d7} ")).clicked() {
                store.set(sid, preset);
            }
        }
    });
    // Always editable, not just once "Custom" is picked - lets a player fine-tune away from
    // a preset without a separate mode switch; the combo box above will naturally relabel
    // itself "Custom..." as soon as the value no longer matches one of the presets.
    let (mut w, mut h) = current
        .to_lowercase()
        .split_once('x')
        .and_then(|(w, h)| Some((w.parse::<i32>().ok()?, h.parse::<i32>().ok()?)))
        .unwrap_or((640, 480));
    ui.add(egui::DragValue::new(&mut w).range(640..=7680));
    ui.label("\u{d7}");
    ui.add(egui::DragValue::new(&mut h).range(480..=4320));
    let updated = format!("{w}x{h}");
    if updated != current {
        store.set(sid, &updated);
    }
}

fn draw_key_capture(ui: &mut egui::Ui, s: &Setting, sid: &str, store: &mut Store, capturing: &mut Option<String>) {
    use crate::backend::keys::dik_label;
    let is_capturing = capturing.as_deref() == Some(sid);
    let text = if is_capturing {
        "Press a key\u{2026}  (Esc cancels, Del clears)".to_string()
    } else {
        dik_label(store.value(sid))
    };
    let button = egui::Button::new(egui::RichText::new(text).monospace())
        .fill(if is_capturing { egui::Color32::from_rgb(0x2a, 0x1e, 0x08) } else { ui.visuals().widgets.inactive.weak_bg_fill })
        .stroke(egui::Stroke::new(1.0, if is_capturing { theme::AMBER } else { theme::stroke_color() }));
    if ui.add_sized([180.0, 0.0], button).clicked() {
        *capturing = Some(sid.to_string());
    }

    if is_capturing {
        let (events, modifiers) = ui.ctx().input(|i| (i.events.clone(), i.modifiers));
        for event in events {
            let egui::Event::Key { key, pressed: true, .. } = event else { continue };
            if key == egui::Key::Escape {
                *capturing = None;
                break;
            }
            if key == egui::Key::Delete {
                store.set(sid, "0");
                *capturing = None;
                break;
            }
            let Some(dik) = egui_key_to_dik(key) else { continue };
            let value = if s.combo_keys {
                match modifier_dik(modifiers) {
                    Some(m) => format!("{dik}+{m}"),
                    None => dik.to_string(),
                }
            } else {
                dik.to_string()
            };
            store.set(sid, &value);
            *capturing = None;
            break;
        }
    }
}

fn draw_volume(ui: &mut egui::Ui, sid: &str, store: &mut Store) {
    let raw: i32 = store.value(sid).trim().parse().unwrap_or(0);
    let mut pct = (raw as f64 * 100.0 / 32767.0).round() as i32;
    if ui.add(egui::Slider::new(&mut pct, 0..=100).suffix("%").show_value(true)).changed() {
        store.set(sid, &((pct as f64) * 32767.0 / 100.0).round().to_string());
    }
}

fn draw_number(ui: &mut egui::Ui, s: &Setting, sid: &str, store: &mut Store, float: bool) {
    let raw = store.value(sid).to_string();
    if float {
        let mut v: f64 = raw.parse().unwrap_or(s.minimum);
        let changed = if s.slider {
            ui.add(egui::Slider::new(&mut v, s.minimum..=s.maximum).step_by(s.step).suffix(&s.suffix)).changed()
        } else {
            ui.add(
                egui::DragValue::new(&mut v)
                    .range(s.minimum..=s.maximum)
                    .speed(s.step)
                    .suffix(&s.suffix)
                    .max_decimals(s.decimals.max(0) as usize),
            )
            .changed()
        };
        if changed {
            store.set(sid, &format!("{v:.6}"));
        }
    } else {
        let mut v: i32 = raw.parse().unwrap_or(s.minimum as i32);
        let changed = if s.slider {
            ui.add(egui::Slider::new(&mut v, (s.minimum as i32)..=(s.maximum as i32)).suffix(&s.suffix)).changed()
        } else {
            ui.add(
                egui::DragValue::new(&mut v)
                    .range((s.minimum as i32)..=(s.maximum as i32))
                    .speed(s.step.max(1.0))
                    .suffix(&s.suffix),
            )
            .changed()
        };
        if changed {
            store.set(sid, &v.to_string());
        }
    }
}
