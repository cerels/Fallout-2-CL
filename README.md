# Fallout 2 CE Launcher

A settings launcher for [Fallout 2 Community Edition](https://github.com/fallout2-ce/fallout2-ce)
and the total conversions built on the same engine family. Every option that normally means
editing `fallout2.cfg`, `data/config/game#patch.cfg` or a mod's `.ini` gets a proper UI, plus a
save picker and a mod load-order editor.

- **Campaigns**: Vanilla/CE, [Fallout et Tu](https://github.com/rotators/Fo1in2),
  [Fallout: Sonora](https://github.com/cambragol/Fallout-Sonora-English) and
  [Fallout 2: Nevada](https://github.com/HogLord69/fallout-english-localization) (English) are
  one-click installs each from the **Campaigns** page; [RPU](https://github.com/BGforgeNet/Fallout2_Restoration_Project)
  (Restoration Project) installs the same way from its own tab inside the CE campaign's menu,
  since it's a content overlay for CE rather than a separate game - see `GLOSSARY.md`. The sidebar
  logo is a quick-switcher between whichever campaigns are installed; switching reloads this
  launcher's settings, mods and saves for that campaign, each kept independent
  (`docs/adr/0001-campaign-root-is-a-subfolder.md`). The launcher remembers the last campaign you
  played. Olympus 2207 isn't supported - see `docs/adr/0002-olympus-not-supported.md`.
- **Steam integration**: besides the default Launch Options line (below), the Campaigns page
  shows a second one for Et Tu, meant for pasting into *Fallout 1's* own Steam entry so pressing
  Play there opens straight into Et Tu. Sonora and Nevada have no Steam app of their own, so they
  get an "Add to Steam" button instead (a non-Steam game shortcut, forced into that campaign).
- **Settings pages** with descriptions, the right control for each value (switches, dropdowns,
  sliders, key capture), a marker on anything changed from its default, one-click reset and search.
- **Save picker** with thumbnails, character name, location and in-game date. Continue the latest
  save, load a specific one, or start at the main menu.
- **Mods page**: enable, disable and reorder `mods/mods_order.txt`. Disabled mods stay in the
  file, commented out; a line whose file no longer exists is dropped automatically. Mods
  fallout2-ce's README explicitly supports (see `schemas/catalog.toml`) show up in the same list
  even before you have them, each with Open source / Download — and, once installed, Update (when
  a newer release exists) and Delete.
- **Safe edits**: only the lines that change are rewritten, so comments and layout survive.
  The first save of each session copies the originals to `backup/launcher/<timestamp>/`.
- **Modular**: settings are described by TOML schemas. Mods can ship their own, and any mod `.ini`
  without a schema still gets a tab generated from its comments.
- **Installs CE itself**: pointed at a vanilla Fallout 2 copy (game data present, no `fallout2-ce`
  binary), pressing Play offers to download and install the engine from
  [fallout2-ce/fallout2-ce](https://github.com/fallout2-ce/fallout2-ce)'s current build. The
  Launcher page also shows the exact Steam Launch Options line for your OS, with a Copy button.
- **Portable**: ships as a single-file executable per OS (PyInstaller) with no Python or Qt
  install needed - download one file, drop it in the game folder, done.

## Requirements

A Fallout 2 CE install, or a vanilla Fallout 2 copy - the launcher will offer to install CE
itself the first time you press Play against a vanilla copy (Linux and Windows for now; on
macOS, install CE yourself from [the project's releases](https://github.com/fallout2-ce/fallout2-ce/releases) -
it ships as a `.dmg` there, which the launcher doesn't automate).
The launcher otherwise only edits config files and starts `fallout2-ce` with
`--dev-load-game=<slot>` to load a save.

## Install

### Prebuilt binary (recommended)

Download `f2launcher` (Linux), `f2launcher-macos` (macOS) or `f2launcher.exe` (Windows) from
[the Releases page](../../releases/latest) and drop it directly into the game folder - no
Python or PySide6 install needed, it's bundled in:

```
Fallout 2/
├── fallout2-ce
├── fallout2.cfg
├── mods/
└── f2launcher          ← just this one file (chmod +x on Linux/macOS)
```

**Steam:** set the game's launch options to

```
"/path/to/Fallout 2/f2launcher" # %command%
```

The `#` stops Steam from also starting the original game. Pressing **Play** replaces the
launcher process with the game, so Steam keeps tracking it. The Launcher page in the app shows
this line with your actual path already filled in, and a Copy button.

On Windows, use `"C:\path\to\Fallout 2\f2launcher.exe" %command%` instead - but note this is
**unverified**: `cmd.exe` has no equivalent to the `#` trick above, so Steam may end up running
both the launcher and the game in sequence rather than replacing one with the other. Please
confirm on a real Windows+Steam setup and adjust if needed.

Binaries are built per-OS by [.github/workflows/build.yml](.github/workflows/build.yml)
(PyInstaller, see `packaging/f2launcher.spec`) whenever a `vX.Y.Z` tag is pushed.

### From source

For development, or if you'd rather use your distro's own PySide6 package:

- Python 3.11+ and PySide6 (Qt 6): `python3-pyside6` from your distro, or `pip install PySide6`.
- Tested on Linux. Windows should work for editing settings; launching uses `os.execv`.

Copy this folder into the game folder as `launcher/`:

```
Fallout 2/
├── fallout2-ce
├── fallout2.cfg
├── mods/
└── launcher/        ← this project
    ├── f2launcher.py   entry point: CLI args, and wires backend/ + gui/ together
    ├── backend/        config/save/schema reading and mod downloads, no Qt dependency
    ├── gui/            the PySide6 window: one page per module (store, mods_page, ...)
    └── schemas/
```

Run it with `launcher/launch.sh` (or `python3 launcher/f2launcher.py`). On Windows, use
`launcher/launch.bat` instead - same options, same fallback to starting the game directly if
Python isn't found. The Steam launch-options line is the same idea as above, just pointing at
`launcher/launch.sh` / `launcher/launch.bat` instead of the prebuilt binary - the Launcher page
shows whichever one applies to how you're actually running it.

### Command-line options

| Option | Effect |
|---|---|
| `--ui` | Always show the launcher window |
| `--no-ui` | Skip the window and start the game (continues the latest save by default) |
| `--check-schemas` | Validate the schemas and list the pages this install would show |

Whether the window appears by default, and what `--no-ui` does, can be set on the launcher's
own Launcher page (`launcher/launcher.ini`).

## Schemas

Settings are data, not code. The launcher reads, in this order:

| Location | Contents |
|---|---|
| `launcher/schemas/*.toml` | Core: Fallout 2 CE engine settings only |
| `launcher/schemas/mods/*.toml` | Schemas for popular mods, bundled with the launcher |
| `mods/*.schema.toml` | Schemas shipped by mods themselves |

A schema from the last two locations only appears when its files are installed. Bundled mod
schemas: Restoration Project, Unofficial Patch, FO2Tweaks, Party Orders, sfall's
`sfall-mods.ini`, and NPC Armor.

### Writing a schema for a mod

Mod authors can ship `mods/<name>.schema.toml` next to their `.ini`:

```toml
page = "My Mod"                # sidebar entry; use an existing page name to add groups there
title = "My Mod"               # optional, with headline = true shown next to the engine name
intro = "One-line description shown at the top of the page."

[files]
mymod = "mods/mymod.ini"       # file id → path (case-insensitive, relative to the game folder)

[[mod]]                        # description for the Mods page; also used to detect the install
dat = "mymod.dat"
description = "My Mod — what it does"

[[group]]
name = "Combat"
file = "mymod"                 # default file and section for the settings below
section = "main"

[[group.setting]]
key = "faster_reload"
label = "Faster reloading"
type = "bool"
default = "1"
help = "Shown under the label."
note = "Shown highlighted, for warnings."

[[group.setting]]
key = "reload_speed"
label = "Reload speed"
type = "int"
default = "2"
min = 1
max = 5
slider = true
suffix = "×"
enabled_if = { setting = "mymod:main:faster_reload", equals = ["1"] }
```

**Setting types:** `bool`, `choice` (`options = [["value", "Label"], …]`), `int`, `float`,
`volume` (0–32767 shown as %), `key` (DirectInput scan codes, with `combo_keys = true` for
`48+29`-style combinations), `resolution`, `text`.

**Other fields:**
- `enabled_if` takes `equals` or `not`.
- `hint = "scale"` adds a live hint.
- `requires_engine = "<setting name>"` hides a setting unless the installed `fallout2-ce` binary
  knows that setting. This is for options added in newer CE releases. An option entry can take
  the same as a third element: `["2", "Label", "<setting name>"]`.
- `not_settings = ["mods/data.ini"]` marks `.ini` files that hold data rather than player settings,
  so they don't get a generated tab.

### Generated tabs

A `mods/*.ini` with no schema gets a tab built from the file itself:
- section headers become groups, and the comment lines above each key become its description
- the control type is guessed: `0`/`1` described as enable/disable becomes a switch, lists like
  `1 - red` become a dropdown, "scancode" keys get key capture, and everything else is a number or
  text field

These tabs work, but a schema gives better labels and controls, and the schema always wins.

Run `python3 launcher/f2launcher.py --check-schemas` after editing schemas.
