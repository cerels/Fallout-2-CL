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
