//! Renders one schema-driven settings page: a title, an optional intro, and a card per
//! group of settings. This is the dynamic form ADR 0004 picked egui for - the whole
//! function is just "loop over the schema and call the right widget", no per-page code.

use eframe::egui;

use crate::backend::schema::Setting;
use crate::gui::store::Store;
use crate::gui::theme;
use crate::gui::widgets::{card, setting_matches, setting_row};

/// Draws `groups`, filtered by `search` (already lowercased/trimmed by the caller). Returns
/// how many settings matched, for the sidebar's "(N)" badge.
pub fn show(
    ui: &mut egui::Ui,
    store: &mut Store,
    groups: &[(String, Vec<Setting>)],
    intro: &str,
    capturing: &mut Option<String>,
    search: &str,
) -> usize {
    if !intro.is_empty() {
        ui.label(egui::RichText::new(intro).color(theme::MUTED));
        ui.add_space(10.0);
    }

    let mut hits = 0;
    for (gname, settings) in groups {
        let matched: Vec<&Setting> = settings.iter().filter(|s| setting_matches(s, search)).collect();
        if matched.is_empty() {
            continue;
        }
        hits += matched.len();
        card(ui, gname, |ui| {
            for (i, s) in matched.iter().enumerate() {
                setting_row(ui, s, store, capturing, i == 0);
            }
        });
        ui.add_space(12.0);
    }
    hits
}

/// How many of `groups`' settings match `search`, without drawing anything - used to decide
/// whether a nav entry should be enabled/shown a hit count, before its page is even current.
pub fn count_matches(groups: &[(String, Vec<Setting>)], search: &str) -> usize {
    groups.iter().flat_map(|(_, settings)| settings.iter()).filter(|s| setting_matches(s, search)).count()
}
