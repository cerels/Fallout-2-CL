# Fallout 2 Launcher

A settings/mod-manager launcher for Fallout 2 Community Edition and the total-conversion mods built on the same engine family.

## Language

**Campaign**:
One of the mutually-exclusive top-level games the launcher can run and manage: Vanilla/CE, Fallout et Tu, Fallout: Sonora, Fallout 2: Nevada. Picked via a switcher at the top-left badge; the launcher remembers the last one played.
_Avoid_: Mod (for Et Tu/Sonora/Nevada — they're whole separate games, not drop-in mods), Conversion (ambiguous with "total conversion" as a genre label), Game.

**Campaign root**:
The self-contained directory holding one campaign's engine binary, data archives, `mods/` folder and config files. For Vanilla/CE it's the launcher's own game folder (`GAME_DIR`); for Et Tu/Sonora/Nevada it's a dedicated subfolder underneath that same folder (e.g. `Fallout1in2/`, `Sonora/`, `Nevada/`), self-contained enough to coexist without colliding with the vanilla files sitting beside it.

**RPU** (Restoration Project Updated):
A content overlay for the Vanilla/CE campaign — restored cut content and fixes for the same underlying game, not a different game. Lives inside the CE campaign's own menu on its own always-visible tab (install prompt shown even before it's installed), hidden entirely outside the CE campaign. Nothing belonging to RPU — its own core files, its optional extras, or its bundled companions — ever appears on the regular Mods list.
_Avoid_: Total conversion, Campaign (RPU is neither — it's a variant of the CE campaign).

**RPU optional extra**:
RPU's own bonus content (enhanced world map, rifle/flamer/wakizashi animations, Cassidy's voice and head, Mysterious Stranger, alternative explosions) — individually toggleable, but only from RPU's own page, never the Mods page.
_Avoid_: Regular mod, Bundled companion.

**Bundled companion**:
A regular mod (NPC Armor, Party Orders, Unofficial Patch) that happens to ship inside RPU's own release zip alongside RPU's content, rather than being part of RPU itself. The launcher records which files a given RPU install actually extracted, so these can be shown on RPU's page and removed together with RPU — while remaining separately available as an ordinary regular mod for someone who doesn't install RPU at all.
_Avoid_: RPU optional extra (that's RPU's own content, not a separately-distributable mod), Regular mod (on its own, without the "arrived via RPU" distinction).

**Regular mod**:
A drop-in download managed through the existing Mods page/catalog (NPC Armor, Party Orders, FO2Tweaks, etc.) — optional, flat files under a campaign's own `mods/` folder. Installed independently per campaign; never literally shared across campaign roots. The Mods page itself is shown for every campaign, but catalog entries scoped to Vanilla/CE only are hidden when another campaign is active.
_Avoid_: Total conversion, RPU, RPU optional extra, Bundled companion (while it arrived via RPU).

**Forced campaign**:
A launch-time override, passed as an argument on a Steam "Launch Options" line, that boots the launcher directly into one specific campaign — bypassing "remember last campaign" for that launch. Lets one Steam library entry (e.g. Fallout 1's) point at the shared launcher living under the Fallout 2 campaign root and force it open on a different campaign (e.g. Et Tu).

**Shared setting**:
A core engine setting (resolution, display mode, pixel scale, mouse lock, and the Audio page's volume sliders and sound toggles) treated as one player preference across every installed campaign that shares the engine's own config format (Vanilla/CE, Et Tu, Nevada) rather than siloed per campaign. Saving it writes the same value into every such installed campaign's own config file unconditionally, overwriting whatever was there — including a value changed by hand outside the launcher since the last sync. Installing a new campaign seeds it from an already-installed one's current values the same way.
_Avoid_: Regular mod (mod-provided settings, including any key bindings a mod defines, are never shared, even when the same mod happens to be installed in more than one campaign — this term is for core engine settings only).

**Shareable campaign**:
A campaign whose config uses the same file format as Vanilla/CE, so shared settings can sync into it. Now true of every campaign — Vanilla/CE, Et Tu, Sonora, Nevada — since all four run on fallout2-ce and its `fallout2.cfg` format (see `docs/adr/0011-sonora-onto-fallout2-ce.md`). Kept as its own concept rather than inlined as "every campaign," since a future campaign that genuinely uses a different engine/config format could reintroduce an exception.

**Badge**:
The short bracket-text code (`F2`, `ET`, `SO`, `NV`) identifying a campaign everywhere in the terminal chrome — the sidebar switcher, the nav menu entries. Stays bracket-text by deliberate choice (see `docs/adr/0008-terminal-skin-escalation.md`).
_Avoid_: Cover art, icon, logo.

**Cover art**:
A campaign's own official promotional art (not drawn by this project), shown on its Campaigns-page card and, as a faded background wash, behind the sidebar nav. RPU has none of its own — its tab reuses CE's, since RPU isn't a separate campaign. Distinct from the badge, which it doesn't replace (see `docs/adr/0010-campaign-cover-art.md`).
_Avoid_: Badge, Logo, Grid (SteamGridDB's own asset-type names, not this project's vocabulary).
