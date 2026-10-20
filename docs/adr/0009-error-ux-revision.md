# Revise error presentation and confirm destructive-adjacent actions

A UX pass driven by live testing (headless, screenshot-and-click) turned up a few inconsistencies
in how the launcher handles unsaved edits and errors, on top of ADR 0006's existing "a blocking
modal for every outcome" model. Deciding them together since they're all the same underlying
question - when does this app ask before acting, and when does it just tell you what happened:

- **Field-level validation vs. job outcomes.** Every error/success in the app used the same
  blocking `egui::Modal`, including a simple "you left this field empty" check (Et Tu's MASTER.DAT
  path). We moved that one case inline (red text under the field, cleared on edit), but left modals
  in place for actual background-job outcomes (install/extract/download success or failure) and for
  confirmations - those still benefit from demanding attention the way a field-level typo doesn't.
- **Raw backend text reaching the player.** A bad Et Tu extraction path surfaced
  `undat.sh`'s own `"Please pass path to Fallout 1's master.dat to the script."` verbatim. Added
  `gui::errors::friendly()`, a small pattern-match that rewrites a handful of recognized raw
  messages into plain language and passes anything else through unchanged - never hiding an
  unanticipated cause behind a guess, just translating the ones we already know about. Applied at
  every `error_dialog` site that surfaces a job's `Err(String)`, not only the one that prompted it.
- **Confirming unsaved loss.** Delete Mod and Uninstall RPU both ask "are you sure?" first; Discard
  and switching campaigns did not, even though both can throw away edits (switching does it via a
  *silent* best-effort save - see `App::save_all_best_effort` - so a failed save there was
  previously invisible). Both now go through the same `pending_confirm` Yes/No gate as the other
  two, but only when `dirty_count() > 0` - nothing to lose means nothing to ask about.

## Consequences

`ConfirmAction` gained two variants (`Discard`, `SwitchCampaign`) rather than a generic
"confirm-then-closure" callback - consistent with ADR 0005's preference for a flat enum over
callback plumbing, and cheap since there are now six kinds of confirmation total, not an open-ended
set. `App::switch_campaign` itself is unchanged; the new `App::request_switch_campaign` wraps it
and is what every call site (sidebar switcher, Campaigns page) now calls instead. We did not extend
this to auto-saving before Play/Continue - that path already calls `save_all()` (not the silent
best-effort one) and blocks the launch on an error dialog if it fails, so there was nothing to fix
there; it's the model the silent paths arguably should have followed from the start, but changing
`save_all_best_effort`'s callers to also surface failures was out of scope for this pass.

`gui::errors::friendly()` is deliberately a flat substring match, not a structured error-code
taxonomy - the number of actually-recognized failure shapes is small (a handful of HTTP/script
conditions), and a taxonomy would need every backend error to carry a code instead of a `String`,
which is a much bigger change for no current benefit.
