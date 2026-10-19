# Terminal UI blueprint

Full layout and token blueprint for the ADR 0008 terminal-skin escalation. Grounded in the
actual code: `gui::app::App` (nav + panels), `gui::theme` (visuals/fonts), `gui::widgets`
(reusable controls), `gui::pages::*` (page bodies), egui/eframe 0.37.

Settled going in (see ADR 0008 for the "why"):

- Per-campaign accent drives most chrome, not just the badge; plus a small fixed set of
  campaign-independent semantic colors. More colorful than today, not monochrome.
- No simulated CRT artifacts (scanlines/glow/vignette/flicker).
- Hybrid type: monospace for structural chrome, Inter for dense body/help text.
- Campaign switcher stays the dropdown it is today - not promoted to stacked dock tabs.
- Bracket-text sigils for iconography (`[SYS]`, `[!]`), no icon font.
- Out of scope: custom window chrome ("window controls"), a real Save Backups feature - see
  ADR 0008's "Explicitly out of scope."

---

## 1. Intentional aesthetic & tokens

### Palette

Two layers: a **dynamic accent** that changes with the active campaign, and a small **fixed
set** that never does - so status reads the same regardless of which campaign is open.

**Dynamic (already exist in `backend::campaigns::Campaign::accent` - just widen their reach)**

| Campaign | Hex | Reads as |
|---|---|---|
| Fallout 2 / CE | `#5dfc8a` | Pip-Boy green |
| Fallout et Tu | `#5d9dfc` | Terminal blue |
| Fallout: Sonora | `#ffb84d` | Desert amber |
| Fallout 2: Nevada | `#ff7a5d` | Warning red-orange |

**Fixed (campaign-independent)**

| Token | Hex | Use |
|---|---|---|
| `bg-void` | `#0a0b0a` | App/window background. Neutral near-black (no green cast) so *any* campaign accent reads clean against it - today's `BG` (`#0c110d`) is slightly green-tinted, which muddies Et Tu's blue and Nevada's red. |
| `bg-panel` | `#121412` | Nav dock, status bar, bottom bar surfaces. |
| `bg-inset` | `#060706` | Recessed fields: search box, key-capture button, text inputs (today's `EXTREME_BG`). |
| `text-primary` | `#eaf2ec` | Body/label text. Stays neutral off-white (not accent-tinted) so it stays legible against all four accents - recoloring body text per campaign would hurt contrast on the blue/red ones. |
| `text-muted` | `#8fa394` | Help text, subtitles, disabled labels. |
| `status-danger` | `#ff3b5c` | Destructive actions, errors. Deliberately more pink-red than Nevada's `#ff7a5d` (more orange) so the two stay visually distinct when Nevada is active. |
| `status-warning` | `#ffd23f` | Needs-attention states (today's `[!]` cases). Shifted more yellow than Sonora's `#ffb84d` (more orange) for the same reason - the two sit close but are distinguishable at a glance. |
| `status-success` | `#2de8c0` | Confirmation/"done" states. A teal, not another green - CE's own accent is already green, so a second green for "success" would blend into it instead of standing apart. |

Keep `AMBER` in `theme.rs` as the name for `status-warning`'s *value*, not Sonora's accent -
today both are literally the same hex (`#ffb84d`); this blueprint splits them.

### Typography

| Role | Face | Notes |
|---|---|---|
| Chrome (nav labels, status bar, headers, button text, numeric/key readouts) | **IBM Plex Mono** (Regular + Bold, SIL OFL - same license shape as the existing `Inter-LICENSE.txt`) | Boxy, slightly institutional forms read as "terminal" without tipping into a pixel-font gimmick; stays crisp at UI sizes (12-15px), unlike bitmap faces like VT323. |
| Body (settings labels, help/note text, descriptions) | **Inter** (already embedded) | Unchanged - a paragraph of settings help in monospace is measurably harder to scan for no aesthetic payoff. |

Chrome text in Plex Mono renders `UPPERCASE` with normal tracking - egui's `RichText` has no
direct letter-spacing knob, and reaching for a hand-built `LayoutJob` with
`TextFormat::extra_letter_spacing` just to widen a handful of nav labels isn't worth the
complexity (ADR 0005). Uppercase + monospace + the bracket sigils below already carry the
terminal read.

---

## 2. ASCII wireframe & structural zoning

```
┌──────────────────────────────────────────────────────────────────────────────────────────┐
│ [F2] FALLOUT 2 ▾         ●3 UNSAVED   [ SAVE ]  [ DISCARD ]            BUILD v0.3.0        │ ← Top Status Bar
╞════════════════════╦═══════════════════════════════════════════════════════════════════╡
│  ┌────────────────┐│                                                                     │
│  │ > search...    ││   PLAY                                                             │
│  └────────────────┘│   ──────────────────────────────────────────────────────────────   │
│                    │                                                                     │
│  [SYS] PLAY        │   Continue — Slot 3, Vault 13 · 04:12:08                            │
│  [SYS] CAMPAIGNS   │                                                                     │
│  ╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌  │   ┌────────┐ ┌────────┐ ┌────────┐ ┌────────┐                       │
│  CONFIG            │   │ slot 1 │ │ slot 2 │ │ slot 3 │ │ slot 4 │   ← Central ScrollArea │
│    [GFX] DISPLAY   │   └────────┘ └────────┘ └────────┘ └────────┘     (active tab body,  │
│    [AUD] AUDIO     │                                                    mod toggles, etc)│
│    [KEY] CONTROLS  │                                                                     │
│  ╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌╌  │                                                                     │
│  UTIL              │                                                                     │
│    [MOD] MODS      │                                                                     │
│    [RPU] RESTORE   │                                                                     │
│    [SAV] SAVES 🔒  │   ← disabled placeholder, not a real feature (ADR 0008)             │
│    [SYS] LAUNCHER  │                                                                     │
│                    │                                                                     │
│   ← rigid, fixed-   │                                                                     │
│     width dock      │                                                                     │
╞════════════════════╩═══════════════════════════════════════════════════════════════════╡
│                            ┌───────────────────────────────────────┐                     │
│                            │                                       │                     │
│                            │          ▶  C O N T I N U E            │  ← Bottom Action    │
│                            │                                       │     Bar, isolated,   │
│                            └───────────────────────────────────────┘     LAUNCH only      │
└──────────────────────────────────────────────────────────────────────────────────────────┘
```

Four zones, each a distinct egui panel (mapped exactly in §4):

1. **Top Status Bar** — campaign readout + dirty-state ("●3 UNSAVED") + Save/Discard + a
   quiet build-version readout. This is where today's bottom bar's status/Save/Discard moves
   to (ADR 0008 Q6) — it's session metadata, which is what a terminal status line shows.
2. **Left Nav Dock** — rigid, fixed-width (unchanged from today's 250px), heavier border than
   the rest of the UI (2px `bg-void`-on-`<accent>` stroke, vs 1px elsewhere) so it reads as a
   distinct hardware module bolted to the side, not just another panel.
3. **Central Main Panel** — unchanged structurally: a `ScrollArea` holding whatever the active
   nav entry renders (`App::draw_current_page`).
4. **Bottom Action Bar** — isolated, nothing but the LAUNCH button, centered, oversized
   (See §4 for sizing/styling specifics).

---

## 3. Tab distribution & iconography rules

### Vertical distribution in the Left Nav Dock

Three tiers, top to bottom, matching `build_nav`'s existing entry order (no reshuffling,
just grouping it visually):

1. **Primary** (no divider, no group label, always first): `Play`, `Campaigns`. These are the
   two entries about *playing the game right now*, not configuring it - they stay un-grouped
   so they're never visually subordinate to a category label.
2. **CONFIG** (under a dim `╌╌╌` divider + small `CONFIG` label): the dynamically-loaded
   settings pages from the schema catalog (`Display`, `Audio`, `Controls`, ...).
3. **UTIL** (under its own divider + `UTIL` label): `Mods`, `Restoration Project` (CE-only,
   same gating as today), `Launcher` info, and the disabled `Saves` placeholder.

The divider + label is a cheap `ui.add_space` + small `text-muted` label in Plex Mono,
uppercase - no new widget needed.

### Bracket-sigil rules

One mechanism, applied consistently - never mix in a second icon system on the same row.

| Rule | Example |
|---|---|
| Every nav row gets a 3-letter bracket tag before its label. | `[SYS] LAUNCHER`, `[MOD] MODS` |
| Settings pages get a tag from a small lookup table keyed on page name, falling back to a generic `[CFG]` for any page name the table doesn't recognize (schema-driven page names are arbitrary). | `Display → [GFX]`, `Audio → [AUD]`, `Controls → [KEY]`, anything else → `[CFG]` |
| A row needing attention gets its tag replaced by `[!]` in `status-warning` (or `status-danger` for something broken, e.g. a failed install), overriding the normal tag - never shown alongside it. | CE not installed → `[!] PLAY` instead of `[SYS] PLAY`; RPU update available → `[!] RESTORE` |
| A disabled/placeholder row gets `[LOCKED]` in `text-muted`, row itself drawn with `ui.add_enabled_ui(false)`. | `[LOCKED] SAVES` |
| Campaign badges (`F2`/`ET`/`SO`/`NV`) already *are* bracket-style 2-letter sigils - keep them, just render them inside literal `[ ]` in the switcher button for visual consistency with the rest of the sigil system, instead of today's colored pill. | `[F2] FALLOUT 2 ▾` |
| Bracket tags always render in Plex Mono, even on rows whose label uses Inter (there's no such row today, but if one ever needs Inter for a long dynamic name, the tag stays mono). | — |

---

## 4. egui architecture guidelines

Current code already uses the unified `egui::Panel` builder (egui 0.37), not the legacy
`SidePanel`/`TopBottomPanel` names - same thing, newer API. Panel registration order matters:
top/bottom/side panels first, `CentralPanel` last (it claims whatever space is left).

### Top-level `App::ui` (today: `gui/app.rs:377`)

```rust
fn ui(&mut self, root_ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
    // ...job polling / reload unchanged...

    self.draw_status_bar(root_ui);   // NEW - egui::Panel::top("status_bar")
    self.draw_nav(root_ui);          // unchanged call site, restyled inside
    self.draw_bottom_bar(root_ui);   // same call site, now LAUNCH-only
    egui::CentralPanel::default().show(root_ui, |ui| {
        egui::ScrollArea::vertical().show(ui, |ui| {
            ui.set_width(ui.available_width());
            self.draw_current_page(ui, &ctx);
        });
    });
    self.draw_modals(&ctx);
}
```

### Top Status Bar (new)

```rust
fn draw_status_bar(&mut self, ui: &mut egui::Ui) {
    egui::Panel::top("status_bar")
        .exact_height(40.0)
        .frame(egui::Frame::default().fill(theme::bg_panel())
            .inner_margin(egui::Margin::symmetric(14, 0)))
        .show(ui, |ui| {
            ui.horizontal_centered(|ui| {
                self.draw_campaign_switcher_compact(ui);   // "[F2] FALLOUT 2 ▾", moved here
                ui.separator();
                let dirty = self.dirty_count();
                if dirty > 0 {
                    ui.colored_label(theme::status_warning(),
                        format!("\u{25cf}{dirty} UNSAVED"));
                    if ui.button("[ SAVE ]").clicked() { /* self.save_all() */ }
                    if ui.button("[ DISCARD ]").clicked() { /* revert */ }
                } else {
                    ui.colored_label(theme::text_muted(), "ALL SAVED");
                }
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.colored_label(theme::text_muted(), format!("BUILD {}", env!("CARGO_PKG_VERSION")));
                });
            });
        });
}
```

Window controls are intentionally absent here - the app uses native OS decorations
(`eframe::NativeOptions` has no `.with_decorations(false)`), so there's no custom
minimize/close chrome to draw (ADR 0008's "out of scope").

### Left Nav Dock (restyle `draw_nav`, same call site)

```rust
fn draw_nav(&mut self, ui: &mut egui::Ui) {
    egui::Panel::left("nav")
        .resizable(false)
        .exact_size(250.0)
        .frame(egui::Frame::default()
            .fill(theme::bg_panel())
            .stroke(egui::Stroke::new(2.0, self.campaign_accent()))) // heavier, accent-colored
        .show(ui, |ui| {
            self.draw_campaign_switcher(ui);   // unchanged: stays the dropdown
            ui.add(search_box);
            egui::ScrollArea::vertical().show(ui, |ui| {
                draw_nav_group(ui, None, &primary_entries);           // Play, Campaigns
                draw_nav_group(ui, Some("CONFIG"), &settings_entries);
                draw_nav_group(ui, Some("UTIL"), &util_entries);       // incl. locked "Saves"
            });
        });
}

fn draw_nav_group(ui: &mut egui::Ui, label: Option<&str>, entries: &[NavEntry]) {
    if let Some(l) = label {
        ui.add_space(10.0);
        ui.label(theme::mono_muted(l));   // "CONFIG" / "UTIL", small, uppercase, Plex Mono
        ui.separator();
    }
    for entry in entries {
        draw_nav_row(ui, entry);   // bracket tag + label, per §3's rules
    }
}
```

### Bottom Action Bar (restyle `draw_bottom_bar`, same call site)

```rust
fn draw_bottom_bar(&mut self, ui: &mut egui::Ui) {
    egui::Panel::bottom("action_bar")
        .exact_height(96.0)
        .frame(egui::Frame::default().fill(theme::bg_panel()))
        .show(ui, |ui| {
            ui.centered_and_justified(|ui| {
                let (label, warn, _) = self.play_status();
                let button = theme::launch_button(label, warn, self.campaign_accent());
                if ui.add_sized([360.0, 64.0], button).clicked() {
                    self.on_play_or_install_clicked(warn);
                }
            });
        });
}
```

`theme::launch_button` is a new sibling to today's `primary_button` in `theme.rs`: same shape
(filled, bold, rounded), just sized up and keyed off the active campaign's accent instead of
the fixed green/amber choice `primary_button` makes today.

### `show_inside` - when a page needs its own split, not the window's

No page needs this today (Mods is a flat list, Settings is a single scroll of cards). The
pattern is here for when one does - e.g. if Mods ever grows a list-on-the-left,
details-on-the-right layout, it's a panel *inside* that page's own `ui`, not a new top-level
panel on `root_ui`:

```rust
// inside pages::mods::show(app, ui) - a panel scoped to this page's own Ui, not the ctx
egui::Panel::left("mods_list").exact_size(220.0).show_inside(ui, |ui| {
    for row in &app.mods.rows { /* ... */ }
});
egui::CentralPanel::default().show_inside(ui, |ui| {
    /* selected mod's details */
});
```

`show_inside` takes the already-constrained `ui` the host page was given (itself already
inside the window's own `CentralPanel` + `ScrollArea`) rather than the root `ctx` - it carves
up space within that page's content area only, with zero effect on the window-level panels in
§4.
