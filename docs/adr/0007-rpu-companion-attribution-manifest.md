# Record RPU's companion files in its own state file, not a hardcoded list

RPU's official release zip is a full install package that, alongside RPU's own content,
happens to bundle NPC Armor, Party Orders, and the Unofficial Patch's files
(`npc_armor.dat`, `party_orders.dat`, `upu.dat`/`upu.ini`) — "the officially-tested
combination" per `rpu_install.rs`'s doc comment. These are extracted wholesale as part of
RPU's `mods/` folder extraction, but unlike RPU's own optional extras (declared in
`restoration-project.toml`'s `[[mod]]` list via `OPTIONAL_DAT_FILES`), none of these three
companions are declared anywhere — they're just whatever the zip happens to contain. To
attribute them correctly to RPU (so the Mods page can hide them, RPU's own page can show
them, and uninstalling RPU can delete them together with it), the launcher needs to know
which files actually came from RPU.

Decision: record exactly which companion files were extracted during a given RPU install
inside the RPU state file that already exists for a different purpose —
`mods/.f2launcher-rpu.json`, written by `install_rpu`/read by `delete_rpu` to track the
installed version — rather than hardcoding a fixed list of known companion filenames (e.g.
a `COMPANION_DAT_FILES` const parallel to `OPTIONAL_DAT_FILES`) directly in code. A
hardcoded list is simpler, but it would go silently stale if a future RPU release changes
which mods it bundles (BGforge swaps in a different companion, or stops shipping one) —
attribution would then miss files it should claim, or claim files it never actually
installed. Recording what was literally extracted this install keeps attribution correct
no matter what future RPU zips contain, and it's not a new persistence mechanism — it
extends a state file the installer already reads and writes.

## Consequences

Installs made before this manifest existed won't have companion attribution recorded until
RPU is next installed or updated — a one-time, self-healing transition, not a migration
that needs separate handling.
