//! Terminal-skin visual theme (ADR 0008): heavy true-dark backgrounds, a per-campaign accent
//! that re-tints the active state/selection/hyperlink/button colors, and a small fixed set of
//! semantic colors (danger/warning/success) that stay put regardless of which campaign is
//! active - see `docs/terminal-ui-blueprint.md` for the full token table. Plain `egui::Visuals`
//! field tweaks (plus two embedded font families), no extra theme crate: the simplest thing
//! that works (`docs/adr/0005-boring-rust.md`).

use std::sync::Arc;

use eframe::egui;

/// Inter Regular and Bold (SIL Open Font License 1.1 - `assets/fonts/Inter-LICENSE.txt`), used
/// for dense body/help text where a monospace face would hurt scanability for no aesthetic
/// gain (`docs/terminal-ui-blueprint.md` \u{a7}1).
const INTER_REGULAR: &[u8] = include_bytes!("../../assets/fonts/Inter-Regular.ttf");
const INTER_BOLD: &[u8] = include_bytes!("../../assets/fonts/Inter-Bold.ttf");

/// IBM Plex Mono Regular and Bold (SIL Open Font License 1.1 -
/// `assets/fonts/IBMPlexMono-LICENSE.txt`), used for structural chrome: nav labels, the status
/// bar, headers, button text, numeric/key readouts.
const PLEX_MONO_REGULAR: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Regular.ttf");
const PLEX_MONO_BOLD: &[u8] = include_bytes!("../../assets/fonts/IBMPlexMono-Bold.ttf");

/// Names of the bold `FontFamily`s registered in `build_fonts` - use `theme::bold()`/
/// `theme::mono_bold()` to get a `RichText` in one of these rather than naming it directly at
/// call sites.
const INTER_BOLD_FAMILY: &str = "Inter-Bold";
const PLEX_MONO_BOLD_FAMILY: &str = "PlexMono-Bold";

/// Fallback accent, and the value `parse_hex` returns if a campaign's accent hex is ever
/// malformed. Not "the" accent any more - see `accent()` - but still a sane default.
pub const ACCENT: egui::Color32 = egui::Color32::from_rgb(0x5d, 0xfc, 0x8a);
pub const AMBER: egui::Color32 = egui::Color32::from_rgb(0xff, 0xd2, 0x3f);
pub const DANGER: egui::Color32 = egui::Color32::from_rgb(0xff, 0x3b, 0x5c);
pub const SUCCESS: egui::Color32 = egui::Color32::from_rgb(0x2d, 0xe8, 0xc0);
pub const MUTED: egui::Color32 = egui::Color32::from_rgb(0x8f, 0xa3, 0x94);
pub const TEXT: egui::Color32 = egui::Color32::from_rgb(0xea, 0xf2, 0xec);
pub const STRONG_TEXT: egui::Color32 = egui::Color32::from_rgb(0xf4, 0xfa, 0xf5);

const BG: egui::Color32 = egui::Color32::from_rgb(0x0a, 0x0b, 0x0a);
const PANEL_BG: egui::Color32 = egui::Color32::from_rgb(0x12, 0x14, 0x12);
const CARD_BG: egui::Color32 = egui::Color32::from_rgb(0x14, 0x17, 0x14);
const EXTREME_BG: egui::Color32 = egui::Color32::from_rgb(0x06, 0x07, 0x06);
const STROKE: egui::Color32 = egui::Color32::from_rgb(0x2a, 0x33, 0x2c);
const INACTIVE_BG: egui::Color32 = egui::Color32::from_rgb(0x18, 0x1e, 0x19);
const HOVER_BG: egui::Color32 = egui::Color32::from_rgb(0x1f, 0x29, 0x21);
const HOVER_STROKE: egui::Color32 = egui::Color32::from_rgb(0x3d, 0x52, 0x40);
const DARK_FG: egui::Color32 = egui::Color32::from_rgb(0x10, 0x12, 0x10);

/// `CARD_BG`/`STROKE`/`PANEL_BG` aren't exported as constants - only a few call sites outside
/// this module need them directly, so they're reached through these functions instead.
pub(super) fn card_bg() -> egui::Color32 {
    CARD_BG
}

pub(super) fn stroke_color() -> egui::Color32 {
    STROKE
}

pub(super) fn bg_panel() -> egui::Color32 {
    PANEL_BG
}

/// Apply the theme, keyed to `accent` (the active campaign's color). Called once at startup
/// and again on every campaign switch (`App::switch_campaign`) - re-running the whole thing
/// rather than keeping a separate "just update the accent" path is simpler to reason about
/// (`docs/adr/0005-boring-rust.md`), and a campaign switch is rare enough that redoing the
/// font/spacing setup costs nothing noticeable. This app is always dark (a terminal doesn't
/// have a light mode), so the system theme preference is overridden outright.
pub fn install(ctx: &egui::Context, accent: egui::Color32) {
    ctx.set_theme(egui::ThemePreference::Dark);
    ctx.set_fonts(build_fonts());
    let visuals = build_visuals(accent);
    ctx.all_styles_mut(|style| {
        style.visuals = visuals.clone();
        style.spacing.item_spacing = egui::vec2(10.0, 10.0);
        style.spacing.button_padding = egui::vec2(12.0, 6.0);
        // A settings page with more rows below the fold previously gave no hint of that until
        // the player was already mid-scroll (egui's "floating" scrollbar default is invisible
        // at rest). `thin()` keeps a bar always present but faint when idle, still expanding on
        // hover/interaction.
        style.spacing.scroll = egui::style::ScrollStyle::thin();
        style.spacing.scroll.dormant_background_opacity = 0.3;
        style.spacing.scroll.dormant_handle_opacity = 0.4;
    });
}

/// The active accent anywhere a `Ui` is in scope - reads back the same color `install` just
/// set on `visuals.selection.stroke`, rather than threading a campaign/accent parameter through
/// every card/button/toggle that wants to re-tint with it.
pub fn accent(ui: &egui::Ui) -> egui::Color32 {
    ui.visuals().selection.stroke.color
}

/// Put Inter ahead of egui's bundled default proportional font, and Plex Mono ahead of egui's
/// bundled default monospace font, so every `.monospace()` call (key-capture buttons, etc.)
/// picks up Plex Mono automatically. The originals stay right behind as fallbacks for any glyph
/// the new fonts don't cover. Both bold weights are registered as their own named families
/// (`bold_family_id()`/`plex_mono_bold_family_id()`) rather than replacing the family outright,
/// so regular text is unaffected and only call sites that ask for it (`theme::bold()`/
/// `theme::mono_bold()`) render bold.
fn build_fonts() -> egui::FontDefinitions {
    let mut fonts = egui::FontDefinitions::default();
    fonts.font_data.insert("Inter".to_owned(), Arc::new(egui::FontData::from_static(INTER_REGULAR)));
    fonts.font_data.insert("Inter-Bold".to_owned(), Arc::new(egui::FontData::from_static(INTER_BOLD)));
    fonts.font_data.insert("PlexMono".to_owned(), Arc::new(egui::FontData::from_static(PLEX_MONO_REGULAR)));
    fonts.font_data.insert("PlexMono-Bold".to_owned(), Arc::new(egui::FontData::from_static(PLEX_MONO_BOLD)));

    fonts.families.entry(egui::FontFamily::Proportional).or_default().insert(0, "Inter".to_owned());
    fonts.families.entry(egui::FontFamily::Monospace).or_default().insert(0, "PlexMono".to_owned());

    let proportional_fallbacks = fonts.families.get(&egui::FontFamily::Proportional).cloned().unwrap_or_default();
    let mut bold_family = vec!["Inter-Bold".to_owned()];
    bold_family.extend(proportional_fallbacks);
    fonts.families.insert(bold_family_id(), bold_family);

    let monospace_fallbacks = fonts.families.get(&egui::FontFamily::Monospace).cloned().unwrap_or_default();
    let mut mono_bold_family = vec!["PlexMono-Bold".to_owned()];
    mono_bold_family.extend(monospace_fallbacks);
    fonts.families.insert(plex_mono_bold_family_id(), mono_bold_family);

    fonts
}

fn bold_family_id() -> egui::FontFamily {
    egui::FontFamily::Name(INTER_BOLD_FAMILY.into())
}

fn plex_mono_bold_family_id() -> egui::FontFamily {
    egui::FontFamily::Name(PLEX_MONO_BOLD_FAMILY.into())
}

/// A `RichText` in Inter Bold with the usual "strong" color baked in - the bold counterpart
/// to calling `.strong()` directly, which (per egui) only changes color, not weight. Chain
/// `.color()`/`.size()` afterward same as any other `RichText`.
pub fn bold(text: impl AsRef<str>) -> egui::RichText {
    egui::RichText::new(text.as_ref().to_string()).strong().family(bold_family_id())
}

/// A `RichText` in Plex Mono Regular - structural/chrome text (nav labels, status bar,
/// headers, bracket sigils). Plain `.monospace()` already resolves to Plex Mono since
/// `build_fonts` puts it first in the `Monospace` family, but this name is the chrome-text
/// counterpart to `bold()`/`mono_bold()` for call sites that want to say what they mean.
pub fn mono(text: impl AsRef<str>) -> egui::RichText {
    egui::RichText::new(text.as_ref().to_string()).monospace()
}

/// A `RichText` in Plex Mono Bold - the mono counterpart to `bold()`, for chrome text that
/// needs both (nav section headers, the LAUNCH button, status bar readouts).
pub fn mono_bold(text: impl AsRef<str>) -> egui::RichText {
    egui::RichText::new(text.as_ref().to_string()).strong().family(plex_mono_bold_family_id())
}

fn build_visuals(accent: egui::Color32) -> egui::Visuals {
    let mut visuals = egui::Visuals::dark();
    visuals.override_text_color = Some(TEXT);
    visuals.window_fill = BG;
    visuals.panel_fill = BG;
    visuals.faint_bg_color = CARD_BG;
    visuals.extreme_bg_color = EXTREME_BG;
    visuals.code_bg_color = EXTREME_BG;
    visuals.hyperlink_color = accent;
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
    w.active.bg_stroke = egui::Stroke::new(1.0, accent);
    w.active.fg_stroke = egui::Stroke::new(1.0, accent);
    w.open.bg_fill = INACTIVE_BG;
    w.open.weak_bg_fill = INACTIVE_BG;
    w.open.bg_stroke = egui::Stroke::new(1.0, HOVER_STROKE);

    // A translucent wash of the accent itself, so text selection highlighting re-tints along
    // with everything else rather than staying a fixed, campaign-independent green.
    visuals.selection.bg_fill = egui::Color32::from_rgba_unmultiplied(accent.r(), accent.g(), accent.b(), 40);
    visuals.selection.stroke = egui::Stroke::new(1.0, accent);

    // Softer/more modern than the sharp-ish corners this started with - still short of
    // "bubbly". Windows/menus round the most since they're the biggest shapes on screen.
    for corner in [&mut visuals.window_corner_radius, &mut visuals.menu_corner_radius] {
        *corner = egui::CornerRadius::from(14.0);
    }
    visuals.widgets.noninteractive.corner_radius = egui::CornerRadius::from(12.0);
    visuals.widgets.inactive.corner_radius = egui::CornerRadius::from(10.0);
    visuals.widgets.hovered.corner_radius = egui::CornerRadius::from(10.0);
    visuals.widgets.active.corner_radius = egui::CornerRadius::from(10.0);
    visuals.widgets.open.corner_radius = egui::CornerRadius::from(10.0);

    visuals
}

/// Parse a `"#rrggbb"` string (the `Campaign::accent` fields in `backend::campaigns`) into a
/// `Color32`, falling back to `ACCENT` if it's ever malformed - there's no hex-color parser in
/// scope at runtime otherwise (`egui::hex_color!` only works on string literals known at
/// compile time).
pub fn parse_hex(hex: &str) -> egui::Color32 {
    let h = hex.trim_start_matches('#');
    let parsed = (h.len() == 6).then(|| {
        let byte = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok();
        Some(egui::Color32::from_rgb(byte(0)?, byte(2)?, byte(4)?))
    });
    parsed.flatten().unwrap_or(ACCENT)
}

/// The one unmissable "primary" action (Play / Install / Launch) - styled as a backlit switch
/// (dark bezel, glowing outline and label) rather than a solid-filled pill, so it reads as
/// mechanical rather than as just another big SaaS button; see
/// `docs/terminal-ui-blueprint.md`'s LAUNCH-button note. `warn` swaps the glow from the
/// campaign accent to amber - "needs attention first" (install-needed, pending update, ...).
pub fn primary_button(text: impl Into<egui::WidgetText>, warn: bool, accent: egui::Color32) -> egui::Button<'static> {
    let glow = if warn { AMBER } else { accent };
    egui::Button::new(mono_bold(text.into().text()).color(glow))
        .fill(DARK_FG)
        .stroke(egui::Stroke::new(2.0, glow))
        .corner_radius(4.0)
}
