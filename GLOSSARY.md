# Fallout 2 Launcher

A settings/mod-manager launcher for Fallout 2 Community Edition and the total-conversion mods built on the same engine family.

## Language

**Campaign**:
One of the mutually-exclusive top-level games the launcher can run and manage: Vanilla/CE, Fallout et Tu, Fallout: Sonora, Fallout 2: Nevada. Picked via a switcher at the top-left logo; the launcher remembers the last one played.
_Avoid_: Mod (for Et Tu/Sonora/Nevada — they're whole separate games, not drop-in mods), Conversion (ambiguous with "total conversion" as a genre label), Game.

**Campaign root**:
The self-contained directory holding one campaign's engine binary, data archives, `mods/` folder and config files. For Vanilla/CE it's the launcher's own game folder (`GAME_DIR`); for Et Tu/Sonora/Nevada it's a dedicated subfolder underneath that same folder (e.g. `Fallout1in2/`, `Sonora/`, `Nevada/`), self-contained enough to coexist without colliding with the vanilla files sitting beside it.

**RPU** (Restoration Project Updated):
A content overlay for the Vanilla/CE campaign — restored cut content and fixes for the same underlying game, not a different game. Lives inside the CE campaign's own menu on its own always-visible tab (install prompt shown even before it's installed), never in the regular Mods list.
_Avoid_: Total conversion, Campaign (RPU is neither — it's a variant of the CE campaign).

**Regular mod**:
A drop-in download managed through the existing Mods page/catalog (NPC Armor, Party Orders, FO2Tweaks, etc.) — optional, flat files under a campaign's own `mods/` folder. Installed independently per campaign; never literally shared across campaign roots.
_Avoid_: Total conversion, RPU.

**Forced campaign**:
A launch-time override, passed as an argument on a Steam "Launch Options" line, that boots the launcher directly into one specific campaign — bypassing "remember last campaign" for that launch. Lets one Steam library entry (e.g. Fallout 1's) point at the shared launcher living under the Fallout 2 campaign root and force it open on a different campaign (e.g. Et Tu).

**Shared setting**:
A setting treated as one player preference across every installed campaign (e.g. resolution, volume, key bindings) rather than siloed per campaign. Changing it writes the same value into every installed campaign's own config file under that campaign's own key names; a campaign can still diverge afterward by editing its own copy directly, which then wins for that campaign going forward.
