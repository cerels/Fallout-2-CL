# Rewrite from Python/PySide6 to Rust/egui

Status: accepted

The launcher was vibecoded in Python/PySide6 over one afternoon (19 commits) and already tagged
`v0.1.0` for distribution to other players. Rather than let more surface area accumulate on a
stack that was never meant to be load-bearing, we're rewriting it in Rust now, while the codebase
is still small: it ships as a genuinely dependency-free static binary (no Python/Qt runtime to
bundle, no PyInstaller AV false-positive risk), which matters once this is in other people's
hands.

The backend (campaign/installer logic, the TOML schema engine, save-thumbnail parsing, Steam
shortcut writing) is a faithful 1:1 port: same file formats, same behavior, `GLOSSARY.md` and
ADRs 0001-0003 unchanged — the Python code is the spec. Only OS-detection/packaging plumbing may
be unified; actual OS-specific behavior (e.g. macOS still needing a manual CE install) stays as
today's documented limitation rather than being fixed as part of this rewrite. The backend is
ported and tested module-by-module before any GUI code is written.

The frontend is redesigned freely on top of the ported backend, using **egui**, with a
Pip-Boy-inspired skin (modern layout and spacing, amber/green CRT accents kept subtle rather than
kitschy).

Work happens on a new branch; `main` (Python) stays the shippable default until the Rust version
reaches feature parity, then we cut over.

## Considered options (GUI layer)

- **Slint** — nicest default look of the four, and its royalty-free license is actually fine for
  a free OSS binary (just an attribution requirement). Rejected because its compile-time `.slint`
  DSL fights the runtime-dynamic forms we need to generate from TOML schemas.
- **iced** — System76's COSMIC desktop proves it can look genuinely modern. Rejected for now:
  still explicitly "experimental," with a slow, breaking release cadence (0.13→0.14 took ~15
  months) — too much upgrade risk for a solo maintainer.
- **floem** — no release in ~2 years, single maintainer, broken IME/accessibility. Not viable.
- **Tauri** — would mean a system webview dependency and moving the UI out of Rust, which defeats
  the point of this rewrite.
- **egui** (picked) — immediate-mode model makes schema-driven dynamic forms trivial (loop over
  the schema, call `ui.checkbox()`/`ui.slider()`); its plain default look is the cheapest of the
  four to fix with a theme crate.

## Update

ADR 0008 escalates "amber/green CRT accents kept subtle rather than kitschy" above into a
louder, more colorful, monospace-chrome terminal skin. The rest of this ADR stands.
