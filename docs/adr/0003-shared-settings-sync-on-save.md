# Shared settings sync on save, not a single shared file

A handful of settings (resolution, volume, key bindings) are really one player
preference, not five independent ones — but each campaign root keeps its own config file
in its own format (`fallout2.cfg`, `nevada.cfg`, `fission.cfg`, ...), so there's no single
physical file to point them all at.

Decided: a schema setting can be tagged as shared. Editing it writes the same value into
every *installed* campaign's own config, under that campaign's own key — not a symlink or
a shared file. A campaign can still edit its own copy afterward, which then wins for that
campaign going forward (no further sync until the shared value is changed again).
Installing a new campaign seeds its config from the current shared values.

We considered one physical shared config all campaigns read from. Rejected: the engines
disagree on key names and file format (Sonora's FISSION fork in particular uses an
unrelated `[enhancements]`-style file), so a single shared file would either be lossy or
force a translation layer per campaign anyway — sync-on-save gets the same practical
result without inventing a new file format.

## Addendum: concrete scope and mechanics

Settings opt in one at a time via a `shared = true` flag on the schema entry, not by file
or page. The initial set: Display's resolution, display mode, pixel scale and mouse lock,
plus Audio's volume sliders and sound toggles. Gameplay/Combat/Companions/Interface stay
per-campaign — those are balance and per-engine UI behavior, not player identity. Mod
settings (including any keybindings a mod defines) are never shared, even when the same
mod is installed in more than one campaign — only core engine settings participate.

Sync only reaches campaigns whose config uses the same file format as Vanilla/CE (Et Tu,
Nevada today). Sonora's FISSION fork has no equivalent keys and is skipped entirely, not
treated as an error.

Sync is a plain mirror, not a merge: saving a shared setting from any campaign's settings
page overwrites that key in every other installed, same-format campaign's config
unconditionally — including a value someone changed by hand outside the launcher since
the last sync. If a sibling's config file doesn't exist yet, that sibling is skipped
silently rather than having a new, incomplete file fabricated for it. A sibling write that
fails (e.g. disk full) is logged and skipped rather than failing the whole save — the
active campaign's own settings already saved successfully by that point, and the user's
save shouldn't appear to fail because of it. Each touched sibling's config is backed up
into that sibling's own `backup_dir()` first, same timestamp convention as the active
campaign's own backup.

Installing a new same-format campaign seeds its shared settings from whichever
same-format campaign is already installed, preferring Vanilla/CE, else the first
installed in campaign-list order — there's no persistent "current shared value" to read
back otherwise, since the value only ever lives inside each campaign's own config.
