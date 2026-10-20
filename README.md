# Fallout 2 Launcher

A single launcher for [fallout2-ce](https://github.com/fallout2-ce/fallout2-ce) and the total
conversions built on the same engine: [Fallout et Tu](https://github.com/rotators/Fo1in2),
[Fallout: Sonora](https://github.com/cambragol/Fallout-Sonora-English), and
[Fallout 2: Nevada](https://github.com/HogLord69/fallout-english-localization), plus the
[Restoration Project](https://github.com/BGforgeNet/Fallout2_Restoration_Project) and the regular
mods people actually run alongside them.

## Why this exists

Playing one of the classic Fallout games properly today means hand-editing `fallout2.cfg`,
hunting down a `game#patch.cfg` key by trial and error, or digging through a mod's `.ini` for the
one setting you actually wanted to change, and that's before you've figured out which total
conversion needs which installer, or untangled your save folder from your settings folder.

This launcher is meant to be the one place that handles all of it: pick a game, configure it with
real labeled controls instead of raw config text, manage your mods and saves, and press Play.

## What it does

- One launcher, every campaign. CE, Et Tu, Sonora and Nevada each get their own settings, mods and
  saves, kept fully independent. Switch between them with one click on the sidebar logo, and the
  launcher remembers the last one you played. (Olympus 2207 isn't supported yet; see
  [docs/adr/0002-olympus-not-supported.md](docs/adr/0002-olympus-not-supported.md).)
- Every setting, as an actual control. Switches, dropdowns, sliders, key capture, resolution
  pickers: anything that used to mean editing a config file by hand. Changed-from-default values
  are marked, search finds a setting by name, and one click resets it. The Restoration Project
  lives on its own tab inside the CE campaign, since it's an overlay for CE rather than a separate
  game.
- A real save picker. Thumbnails, character name, location, in-game date. Continue your most
  recent save, jump to a specific one, or start at the main menu. No more guessing which `SLOT03`
  is the right one.
- Mods, managed. Enable, disable and reorder your load order. Mods the launcher knows about (the
  ones `fallout2-ce` itself explicitly supports) show up with one-click Download, and later Update
  or Delete, even before you have them installed. Disabling a mod keeps it on disk (it's commented
  out, not deleted), so turning it back on doesn't mean reinstalling.
- Installs the engine for you, not run on it. The launcher doesn't do anything with a vanilla
  Fallout 2 install except offer to fix it: point it at one and pressing Play offers to download
  and install `fallout2-ce` first, no separate step. Once that's done, you're on CE like everyone
  else.
- Steam, done right. The Launcher page shows the exact Launch Options line for your OS with a
  Copy button, a second line for routing *Fallout 1's* own Steam entry straight into Et Tu, and an
  "Add to Steam" button for Sonora and Nevada, which don't have Steam entries of their own.
- Won't wreck your configs. Only the lines that actually change get rewritten, so your comments
  and formatting survive, and the first save of every session backs up the originals to
  `backup/launcher/`.

## Install

Grab `f2launcher` (Linux), `f2launcher-macos` (macOS) or `f2launcher.exe` (Windows) from
[the Releases page](../../releases/latest) and drop it straight into your game folder:

```
Fallout 2/
├── fallout2-ce
├── fallout2.cfg
├── mods/
└── f2launcher          ← just this one file (chmod +x on Linux/macOS)
```

That's it. No install step, nothing to unzip.

**Steam:** open the game's Properties → General → Launch Options and set it to

```
"/path/to/Fallout 2/f2launcher" # %command%
```

The `#` stops Steam from also starting the original game, and pressing **Play** replaces the
launcher process with the real game so Steam keeps tracking play time normally. The Launcher page
inside the app shows this exact line with your real path already filled in, plus a Copy button, so
you shouldn't need to type it by hand.

> On Windows this is **unverified**: `cmd.exe` has no equivalent to the `#` trick above, so Steam
> may end up running both the launcher and the game in sequence instead of replacing one with the
> other. If you can confirm how this behaves (or how to fix it) on a real Windows+Steam setup,
> please open an issue.

On macOS, install `fallout2-ce` itself first from
[its own releases](https://github.com/fallout2-ce/fallout2-ce/releases) (it ships as a `.dmg`
there, which this launcher doesn't automate), but everything else works the same afterward.

## A quick tour

- Campaigns: the page to install Et Tu, Sonora, Nevada, or RPU for the first time. The sidebar
  logo above it is the quick-switcher between whichever campaigns you already have installed.
- Settings pages: one tab per settings area (Display, Interface, Gameplay, Combat Balance, Audio,
  and one per installed mod that ships its own settings). Use the search box at the top if you
  know roughly what you're looking for but not which tab it's on.
- Mods: your load order, drag-to-reorder, plus the catalog of mods the launcher can fetch and
  install directly.
- Play: your save picker, and the big Launch button.

## Command-line options

Mostly useful for setting up more than one Steam entry pointed at the same install (e.g. forcing
a specific campaign from a different game's Steam entry):

| Option | Effect |
|---|---|
| `--ui` | Always show the launcher window |
| `--no-ui` | Skip the window and start the game directly |
| `--campaign=<id>` | Force a specific campaign for this launch (`ce`, `ettu`, `sonora`, `nevada`) |

Whether the window appears by default, and what happens when it's skipped, can also be set from
the launcher's own Launcher page. The flags above just override that for one launch.

## Supported campaigns and mods

| Campaign | Source | Notes |
|---|---|---|
| CE | installed from your existing game copy | runs [fallout2-ce](https://github.com/fallout2-ce/fallout2-ce); a vanilla copy gets offered the CE install automatically |
| Fallout et Tu | [rotators/Fo1in2](https://github.com/rotators/Fo1in2) | installs into `Fallout1in2/` alongside your Fallout 2 copy |
| Fallout: Sonora | [cambragol/Fallout-Sonora-English](https://github.com/cambragol/Fallout-Sonora-English) | installs into `Sonora/` |
| Fallout 2: Nevada | [HogLord69/fallout-english-localization](https://github.com/HogLord69/fallout-english-localization) | installs into `Nevada/` |
| Restoration Project | [BGforgeNet/Fallout2_Restoration_Project](https://github.com/BGforgeNet/Fallout2_Restoration_Project) | a content overlay for CE, not a separate campaign, with its own tab inside CE's menu |

Regular mods with one-click install from the Mods page today: NPC Armor, Party Orders, FO2Tweaks,
and the High Resolution Patch. Several more (Unofficial Patch, sfall's mod settings) get a full
settings page the moment you install them yourself, even without one-click install yet. Any other
mod still gets a usable settings tab generated from its `.ini` file automatically.

## For mod authors and contributors

- [docs/writing-mod-schemas.md](docs/writing-mod-schemas.md): give your mod a proper settings
  page instead of (or in addition to) the auto-generated one.
- [docs/building-from-source.md](docs/building-from-source.md): build it yourself, run the tests,
  project layout, how releases are cut.
- [docs/adr/](docs/adr/) and [GLOSSARY.md](GLOSSARY.md): the design decisions behind specific
  choices (why campaigns live in subfolders, why RPU isn't a regular mod, why some settings sync
  across campaigns, ...) and the terms those decisions use.

## License

[MIT](LICENSE)
