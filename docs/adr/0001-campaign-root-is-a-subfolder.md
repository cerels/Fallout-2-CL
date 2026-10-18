# Campaign root is a subfolder under the launcher's own game folder

Et Tu, Sonora and Nevada each install as a self-contained directory (own engine binary,
data archives, `mods/`, config) — confirmed from their actual release assets, not just
install guides. Rather than running a separate launcher install per campaign (one copy
living inside each campaign's own directory), one launcher install — the one already
living in this Fallout 2 folder — treats each total conversion as a subfolder underneath
itself (`Fallout1in2/`, `Sonora/`, `Nevada/`) and switches its active root between them.

We considered a separate launcher copy per campaign folder (mirroring how CE's launcher
works today, one install per game folder). Rejected: it would mean duplicating settings
state, the mod catalog, and "remember last campaign" across N independent installs with
no shared memory of each other — defeating the point of a single campaign switcher.

A consequence: `GAME_DIR` (`backend/paths.py`) can no longer mean "the one game folder
this launcher cares about" — it becomes the Vanilla/CE root specifically, with sibling
campaign roots resolved relative to it.
