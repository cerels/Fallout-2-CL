# Escalate the terminal skin past "subtle, not kitschy"

Status: accepted

ADR 0004 picked a Pip-Boy-inspired skin "with amber/green CRT accents kept subtle rather than
kitschy." Having shipped that, the layout reads as a generic dark-mode app with a green tint -
flat, linear, nothing distinguishing it as a diegetic terminal - which undersells the actual
goal of a Fallout-themed launcher. This ADR commits to pushing further, on both axes the
original phrase hedged against:

- **Louder, not quieter.** Each campaign already carries its own accent hex
  (`backend::campaigns::Campaign::accent` - green/blue/amber/red-orange for CE/Et
  Tu/Sonora/Nevada) but today it only colors a small badge pill. That accent now drives the
  whole chrome - borders, active nav state, hovers, the idle LAUNCH color - so switching
  campaigns visibly re-tints the terminal, like a different module is inserted. On top of it
  sits a small fixed set of campaign-independent semantic colors (danger/warning/success) so
  status reads consistently no matter which campaign is active.
- **Monospace chrome, not just accent tints.** Structural text - nav, the new top status bar,
  headers, the LAUNCH button, numeric/key readouts - moves to a monospace face; Inter (already
  embedded, see ADR 0004) stays for dense settings body/help text, where monospace would hurt
  scanability for no aesthetic gain.
- **Bracket-text iconography** (`[SYS]`, `[MOD]`, `[!]`) instead of an icon font, consistent
  with ADR 0005's "avoid a dependency unless there's no simpler way" - Inter is still the only
  non-system font asset embedded besides the new monospace face.

We deliberately stop short of simulated CRT artifacts - scanline overlay, phosphor glow/bloom,
vignette, flicker. Those read as a screen-capture gimmick rather than a UI, egui has no
built-in post-processing pass so they'd need a custom shader (against ADR 0005), and they're a
common accessibility/eye-strain complaint. The terminal feel comes entirely from palette, type,
grid rigidity, and iconography - not simulated hardware decay.

The full token set, ASCII wireframe, tab/iconography rules, and egui panel mapping live in
`docs/terminal-ui-blueprint.md`. This supersedes the "kept subtle" framing in ADR 0004's
frontend paragraph; the rest of 0004 (engine choice, backend port, Inter as the base font) is
unaffected.

## Explicitly out of scope

- **Custom window chrome.** The app uses native OS window decorations
  (`eframe::NativeOptions` has no `.with_decorations(false)`); a "window controls" row in the
  status bar would mean a frameless window plus hand-rolled drag/minimize/close regions - a
  separate, much larger decision this ADR doesn't make.
- **A real Save Backups feature.** It appeared in `docs/terminal-ui-blueprint.md`'s wireframe
  as an illustrative disabled placeholder tab, since removed from the nav entirely - no backup
  functionality exists in `backend::saves` today (it only reads saves for the Play page's
  thumbnail picker), and building it is a separate feature decision.
