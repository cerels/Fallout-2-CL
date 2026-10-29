# Sonora moves onto fallout2-ce, off its own FISSION fork

ADR 0001 treated Sonora like Et Tu and Nevada — "own engine binary" — and took that to
mean installing cambragol/Fallout-Sonora-English's own standalone package, engine
included: FISSION (cambragol/fission-ce), a hard fork of the long-unmaintained
alexbatalov/fallout2-ce, not the actively-maintained fallout2-ce/fallout2-ce this launcher
uses for every other campaign. In practice Et Tu, Nevada and RPU all install their data
onto a *shared* native fallout2-ce binary (`ce_install::install_ce`, reused via
`nevada_install.rs`/`ettu_install.rs`); Sonora alone carried its own separate, actively-
diverging fork of a dead codebase. "Own engine binary" undersold the real difference: a
private *copy* of the same engine (Et Tu, Nevada) versus a genuinely different one
(Sonora).

That bespoke engine was also the direct cause of a real crash: talking to a specific NPC
(Granny Adonisia) reliably segfaulted inside FISSION's dialogue-window code, right after
failed message-string lookups it didn't handle gracefully. Confirmed via a crash dump
(`systemd-coredump`) and, separately, confirmed *not* to crash when the same content was
run on fallout2-ce instead.

Decided: `sonora_install.rs` installs NovaRain/Fallout-Sonora-English's `eng_repack`
branch — a repack of the same cambragol release (same version numbering, actively kept in
sync, no added content) built for classic `Fallout2.exe`+sfall instead of FISSION — and
swaps its classic Windows-only engine for a native fallout2-ce build via
`ce_install::install_ce`, identically to Nevada. Sonora's `Campaign.shareable` flips to
`true` now that it genuinely uses `fallout2.cfg`.

## Considered and rejected

HogLord69/fallout-english-localization (Nevada's own source repo) also bundles a Sonora
asset in the same classic-engine+sfall shape — structurally a cleaner fit, reusing an
already-trusted repo. Rejected: inspecting it (`Sonora_Info.txt`) showed it's a fan
"Extended Release" rebuild with its own bundled mods (Glasses Mod, MapBlurFX, edited maps
and scripts) layered on top of the base game — out of scope for a change meant to swap
only the engine, not the content.

## Consequences

Existing Fission-based Sonora installs aren't migrated — no save-compatible path exists
between the two engines, so switching an existing install means deleting `Sonora/` and
reinstalling fresh. No migration tooling was built for this one-time, already-incompatible
change (see ADR template's "hard to reverse" framing: reversing *this* decision would mean
the same thing in the other direction).
