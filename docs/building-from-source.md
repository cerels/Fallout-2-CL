# Building from source

Most players just want [the prebuilt binary](../README.md#install). This is for contributors,
packagers, or anyone who'd rather build it themselves.

## Requirements

- [Rust](https://www.rust-lang.org/tools/install) (stable toolchain, edition 2024).
- Linux also needs the X11/Wayland dev headers egui builds against:
  `libxcb-render0-dev libxcb-shape0-dev libxcb-xfixes0-dev libxkbcommon-dev libwayland-dev`
  (Debian/Ubuntu package names; see [.github/workflows/build.yml](../.github/workflows/build.yml)
  for the exact list CI installs).

## Build and run

Check this repo out as `launcher/` inside an actual Fallout 2 install (it needs `fallout2.cfg`,
`mods/`, etc. as siblings to run against):

```
Fallout 2/
├── fallout2-ce
├── fallout2.cfg
├── mods/
└── launcher/        ← this repo
    └── rust/
```

```sh
cargo build --manifest-path launcher/rust/Cargo.toml
```

The binary itself needs to run from the game folder, so copy or symlink the result into place to
try it:

```sh
cp launcher/rust/target/debug/f2launcher ./f2launcher && ./f2launcher
```

Everything the binary needs — schemas, fonts, the window icon — is baked in at compile time with
`include_str!`/`include_bytes!`, so the result is a single self-contained file, same as the
release binaries.

## Tests

```sh
cargo test --manifest-path rust/Cargo.toml
```

The backend (campaign/installer logic, the TOML schema engine, save-thumbnail parsing, Steam
shortcut writing) is covered module-by-module and has no GUI dependency, so it's fast and runs
headless. [.github/workflows/ci.yml](../.github/workflows/ci.yml) runs this on every push/PR.

## Project layout

```
rust/src/
├── backend/   campaign + installer logic, schema engine, saves, Steam shortcuts — no egui here
├── gui/       the egui window: one module per page (campaigns, mods, play, settings, ...)
└── main.rs
```

The backend/GUI split is deliberate: anything that reads or writes a file lives in `backend/`
and is unit-testable without a display; `gui/` only renders state and calls into it. See
[docs/adr/](adr/) for the design decisions behind specific choices (why campaigns live in
subfolders, why RPU isn't a regular mod, why settings sync across campaigns, etc.) and
[GLOSSARY.md](../GLOSSARY.md) for the terms those ADRs use.

## Releasing

Pushing a tag matching `vX.Y.Z` (after bumping the version in `rust/Cargo.toml` to match) runs
[build.yml](../.github/workflows/build.yml), which builds native binaries for Linux, Windows and
macOS and attaches them to a GitHub Release.
