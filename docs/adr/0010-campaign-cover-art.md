# Cover art on Campaigns-page cards, bracket badges stay

Status: accepted

Each campaign (CE, Et Tu, Sonora, Nevada) now shows its own 600x900 cover on its Campaigns-page
card and on RPU's tab (which reuses CE's cover rather than getting its own - RPU isn't a separate
campaign, see GLOSSARY.md). The covers are each campaign's own official promotional art, sourced
from the dev's own SteamGridDB uploads, not something this project drew. This is additive to
ADR 0008's bracket-text-iconography decision, not a reversal of it: the sidebar switcher's
`[F2]`/`[ET]`/`[SO]`/`[NV]` badges are untouched, since a store-style card is a different context
than the terminal chrome ADR 0008 was specifically about.

Decoding the cover PNGs needed an image-decoding dependency the project didn't have (added
`image` via `egui_extras`'s image loader) - the one existing raster asset, the window icon, was
deliberately pre-converted to raw `.rgba` specifically to avoid this dependency. Added anyway per
ADR 0005's "avoid a dependency unless there's no simpler way" exception: there's no boring way to
decode arbitrary PNGs without one, and unlike the icon, these covers are expected to change if a
better source image turns up later, so hand-reconverting to raw bytes on every change would be
more fragile than one general decode path.

As an experiment, the sidebar nav panel also now shows a faint (alpha ~45/255), heavily-faded
wash of the active campaign's cover behind its content (`App::draw_nav_cover_wash`) - kept because
it reads as atmosphere, not noise, and reinforces the per-campaign re-tinting ADR 0008 already
does via `accent`. If a future redesign finds it distracting, drop it; it's a few lines, not load
bearing.

Considered: replacing the switcher's badge with cover art too - rejected, since that would
directly reverse ADR 0008 rather than add to it.
