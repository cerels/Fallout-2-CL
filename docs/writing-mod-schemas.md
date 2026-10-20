# Writing a schema for a mod

If you maintain a mod, you can ship it a settings page in the launcher without the launcher
knowing about your mod ahead of time. Settings are data, not code: the launcher reads schemas
from, in this order,

| Location | Contents |
|---|---|
| `rust/schemas/*.toml` | Core: baked into the binary, Fallout 2 CE engine settings only |
| `rust/schemas/mods/*.toml` | Baked-in schemas for popular mods, shown once that mod's files are installed |
| `mods/*.schema.toml` | Schemas shipped by mods themselves, read from disk at runtime |

## Without a schema

A `mods/*.ini` with no schema at all still gets a tab, generated from the file itself:
section headers become groups, the comment lines above each key become its description, and the
control type is guessed from context — `0`/`1` described as enable/disable becomes a switch,
lists like `1 - red` become a dropdown, "scancode" keys get key capture, everything else is a
number or text field. This works, but a real schema gets you better labels and controls — and a
schema always wins over the generated version when both exist.

## With a schema

Ship `mods/<name>.schema.toml` next to your mod's `.ini`:

```toml
page = "My Mod"                # sidebar entry; use an existing page name to add groups there
title = "My Mod"               # optional, shown next to the engine name when headline = true
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

To check your schema, drop it and the mod's `.ini` into a test install's `mods/` folder and run
the launcher from a terminal. A malformed schema (bad TOML, an unknown setting `type`, a setting
missing a required field, ...) is reported as an error rather than crashing the launcher — look
for a `schema error: ...` line printed to stderr if your page isn't showing up the way you expect.

Bundled mod schemas today: Restoration Project, Unofficial Patch, FO2Tweaks, Party Orders,
sfall's `sfall-mods.ini`, and NPC Armor. If yours is popular enough to be worth bundling rather
than shipped with your own mod, open a PR adding it under `rust/schemas/mods/`.
