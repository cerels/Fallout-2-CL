# Launcher self-update: notify-and-link, not a self-replacing binary

The launcher now checks `cerels/Fallout-2-CL`'s latest GitHub release against its own
`CARGO_PKG_VERSION` (see `backend/self_update.rs`) and, when they differ, swaps the status bar's
`BUILD {version}` readout for a clickable `UPDATE AVAILABLE (vX)` that opens the releases page in
the browser (`gui::open::open`, already used for "Open source" buttons elsewhere) - the same shape
as every other update check in this codebase (RPU, regular mods, now the total-conversion
campaigns too), minus the download/overwrite step.

We decided against actually downloading the new build and replacing the running executable.
This launcher ships as a single bare binary dropped straight into the game folder (README's
"Install" section - no installer, no elevated permissions, no update service), and replacing a
file while its own process is still executing it is a genuinely hard, OS-specific problem:
Windows won't let you overwrite (though it will let you rename) a running `.exe`, so a real
self-updater needs a rename-old/write-new/relaunch dance, plus failure handling for every step of
that dance going wrong on someone's particular antivirus or permissions setup - none of which is
remotely debuggable without their machine in front of us. Per `docs/adr/0005-boring-rust.md` and
`0006-indeterminate-download-progress.md`, we've consistently picked the simplest thing that
actually answers what was asked ("let me know an update exists") over the more impressive version
of the feature.

## Considered Options

- **Self-replacing binary**: rejected for the reasons above - real complexity for a single-file,
  no-install-step app, with failure modes we can't reproduce or fix remotely.
- **Notify only, no link**: rejected as strictly worse than notify-and-link for the same effort -
  `gui::open::open` already exists and the releases page is one constant away.

## Consequences

Updating the launcher is still a manual "download the new file, drop it in again" step for the
user, same as today - this ADR only changes whether they find out an update exists.
