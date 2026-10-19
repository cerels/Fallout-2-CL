//! Pip-Boy-inspired visual theme: dark background, phosphor-green accents, amber as the
//! "something needs attention" color - see `docs/adr/0004-rust-rewrite.md` ("modern layout
//! and spacing, amber/green CRT accents kept subtle rather than kitschy"). Plain
//! `egui::Visuals` field tweaks, no extra theme crate: the simplest thing that works
//! (`docs/adr/0005-boring-rust.md`).

use eframe::egui;

pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x5d, 0xfc, 0x8a);
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(0xff, 0xb8, 0x4d);
pub const DANGER: egui::Color32 = egui::Color32::from_rgb(0xff, 0x5c, 0x5c);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(0x86, 0xa3, 0x8b);
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(0xd4, 0xec, 0xd6);
pub const STRONG_TEXT: egui::Color32 = egui::Color32::from_rgb(0xe8, 0xf7, 0xea);

const BG: egui::Color32 = egui::Color32::from_rgb(0x0c, 0x11, 0x0d);
const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(0x10, 0x18, 0x12);
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(0x13, 0x1c, 0x15);
const EXTREME_BG: egui::Color32 = egui::Color32::from_rgb(0x0f, 0x17, 0x11);
const STROKE: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x45, 0x30);
const INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(0x18, 0x26, 0x1b);
const HOVER_BG: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x33, 0x24);
const HOVER_STROKE: egui::Color32 = egui::Color32::from_rgb(0x3d, 0x6a, 0x47);
const DARK_FG: egui::Color32 = egui::Color32::from_rgb(0x06, 0x21, 0x0f);

/// `CARD_BG`/`STROKE` aren't exported as constants - only `widgets::card` needs them
/// directly, so they're reached through these two functions instead.
pub(super) fn card_bg() -> egui::Color32 {
    CARD_BG
}

pub(super) fn stroke_color() -> egui::Color32 {
    STROKE
}

/// Apply the theme once, right after the `eframe::App` is created. This app is always dark
/// (a Pip-Boy doesn't have a light mode), so the system theme preference is overridden
/// outright rather than maintaining a separate light palette nobody will ever see.
pub fn install(ctx: &egui::Context) {
    ctx.set_theme(egui::ThemePreference::Dark);
    let visuals = build_visuals();
    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
    });
}

fn build_visuals() -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.window_fill = BG;
    visuals.panel_fill = BG;
    visuals.faint_bg_color = CARD_BG;
    visuals.extreme_bg_color = EXTREME_BG;
    visuals.code_bg_color = EXTREME_BG;
    visuals.hyperlink_color = ACCENT;
    visuals.warn_fg_color = AMBER;
    visuals.error_fg_color = DANGER;
    visuals.window_stroke = egui::Stroke::new(1.0, STROKE);

    let w = &mut visuals.widgets;
    w.noninteractive.bg_fill = PANEL_BG;
    w.noninteractive.weak_bg_fill = PANEL_BG;
    w.noninteractive.bg_stroke = egui::Stroke::new(1.0, STROKE);
    w.noninteractive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    w.inactive.bg_fill = INACTIVE_BG;
    w.inactive.weak_bg_fill = INACTIVE_BG;
    w.inactive.bg_stroke = egui::Stroke::new(1.0, STROKE);
    w.inactive.fg_stroke = egui::Stroke::new(1.0, TEXT);
    w.hovered.bg_fill = HOVER_BG;
    w.hovered.weak_bg_fill = HOVER_BG;
    w.hovered.bg_stroke = egui::Stroke::new(1.0, HOVER_STROKE);
    w.hovered.fg_stroke = egui::Stroke::new(1.0, STRONG_TEXT);
    w.active.bg_fill = HOVER_BG;
    w.active.weak_bg_fill = HOVER_BG;
    w.active.bg_stroke = egui::Stroke::new(1.0, ACCENT);
    w.active.fg_stroke = egui::Stroke::new(1.0, ACCENT);
    w.open.bg_fill = INACTIVE_BG;
    w.open.weak_bg_fill = INACTIVE_BG;
    w.open.bg_stroke = egui::Stroke::new(1.0, HOVER_STROKE);

    visuals.selection.bg_fill = egui::Color32::from_rgb(0x1c, 0x33, 0x22);
    visuals.selection.stroke = egui::Stroke::new(1.0, ACCENT);

    for corner in [
        &mut visuals.window_corner_radius,
        &mut visuals.menu_corner_radius,
    ] {
        *corner = egui::CornerRadius::from(8.0);
    }
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::from(8.0);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::from(6.0);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::from(6.0);
    visuals.widgets.active.corner_radius = egui::CornerRadius::from(6.0);
    visuals.widgets.open.corner_radius = egui::CornerRadius::from(6.0);

    visuals
}

/// Parse a `"#rrggbb"` string (the `Campaign::accent` fields in `backend::campaigns`) into a
/// `Color32`, falling back to the theme's own accent if it's ever malformed - there's no
/// hex-color parser in scope at runtime otherwise (`egui::hex_color!` only works on string
/// literals known at compile time).
pub fn parse_hex(hex: &str) -> egui::Color32 {
    let h = hex.trim_start_matches('#');
    let parsed = (h.len() == 6).then(|| {
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(egui::Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
    });
    parsed.flatten().unwrap_or(ACCENT)
}

/// A button styled as the one unmissable "primary" action (Play / Install), echoing the old
/// Qt build's `#primary` button. `warn` swaps it from green to amber - "needs attention
/// first" (install-needed, pending update, ...).
pub fn primary_button(text: impl Into<egui::WidgetText>, warn: bool) -> egui::Button<'static> {
    let (bg, fg) = if warn {
        (AMBER, egui::Color32::from_rgb(0x2a, 0x18, 0x00))
    } else {
        (ACCENT, DARK_FG)
    };
    egui::Button::new(
        egui::RichText::new(text.into().text().to_string())
            .color(fg)
            .strong(),
    )
    .fill(bg)
    .corner_radius(6.0)
}
