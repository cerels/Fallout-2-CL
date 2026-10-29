//! Background jobs: installers and mod downloads make network calls that would otherwise
//! freeze the UI thread, so each one runs on its own `std::thread` and reports back through
//! a channel polled once per frame - the plain, dependency-free way to do this off the main
//! thread (`docs/adr/0005-boring-rust.md`); no async runtime needed for a handful of
//! one-shot background calls.

use std::sync::mpsc::{channel, Receiver, Sender};

use crate::backend::campaigns::{activate, Campaign};
use crate::backend::paths::{Paths, SharedPaths};

/// Build a fresh, campaign-activated `Paths` for a background job. A job closure must be
/// `Send + 'static`, but `SharedPaths` is `Rc<Paths>` - not `Send` - so it can never be
/// captured from the UI thread and carried into a spawned one. Every job that needs
/// `Paths` instead captures which `Campaign` it targets (a plain `Copy` value, trivially
/// `Send`) and calls this to build its own `Paths` on the worker thread, activated to that
/// same campaign so campaign-relative lookups (`ci_path`, `game_bin`, ...) resolve exactly
/// like they would in the foreground for that campaign - regardless of whichever campaign
/// happens to be active in the UI by the time the job finishes.
pub(crate) fn fresh_paths(campaign: Campaign) -> SharedPaths {
    let paths = Paths::from_current_exe();
    activate(&paths, &campaign);
    paths
}

/// What a finished background job reports back. One flat enum rather than a job-specific
/// trait object per `docs/adr/0005-boring-rust.md` ("avoid ... trait-object machinery unless
/// there's no simpler way") - there are only a handful of job kinds, so a match arm per kind
/// in the UI code is simpler to follow than a generic callback.
pub enum JobOutcome {
    CeInstall(Result<(), String>),
    CampaignInstall(&'static str, Result<(), String>),
    CampaignUpdateCheck(&'static str, Option<String>),
    Fallout1Extract(Result<(), String>),
    RpuInstall(Result<(), String>),
    RpuUpdateCheck(Option<String>),
    ModInstall(String, Result<(), String>),
    ModUpdateCheck(String, Option<String>),
    LauncherUpdateCheck(Option<String>),
}

pub struct Jobs {
    tx: Sender<JobOutcome>,
    rx: Receiver<JobOutcome>,
    stage_tx: Sender<(String, String)>,
    stage_rx: Receiver<(String, String)>,
    in_flight: u32,
}

impl Jobs {
    pub fn new() -> Self {
        let (tx, rx) = channel();
        let (stage_tx, stage_rx) = channel();
        Jobs { tx, rx, stage_tx, stage_rx, in_flight: 0 }
    }

    /// Spawn a background job identified by `key` (a mod id, "rpu", a campaign id, ...) -
    /// `work` gets a `report` callback it can call mid-job to post a short status
    /// ("Downloading…", "Installing…") that the UI shows inline on whatever row/button
    /// started it (see `App::job_stages`, `docs/adr/0006-indeterminate-download-progress.md`).
    /// A job that has only one stage can just ignore the callback - the UI already shows
    /// *something's* happening from the moment the caller marks the key busy.
    pub fn spawn(&mut self, key: impl Into<String>, work: impl FnOnce(&dyn Fn(&str)) -> JobOutcome + Send + 'static) {
        self.in_flight += 1;
        let tx = self.tx.clone();
        let stage_tx = self.stage_tx.clone();
        let key = key.into();
        std::thread::spawn(move || {
            let report = move |stage: &str| {
                let _ = stage_tx.send((key.clone(), stage.to_string()));
            };
            let _ = tx.send(work(&report));
        });
    }

    pub fn in_flight(&self) -> bool {
        self.in_flight > 0
    }

    /// Drain whatever jobs have finished since the last poll.
    pub fn poll(&mut self) -> Vec<JobOutcome> {
        let mut out = Vec::new();
        while let Ok(outcome) = self.rx.try_recv() {
            self.in_flight = self.in_flight.saturating_sub(1);
            out.push(outcome);
        }
        out
    }

    /// Drain whatever (job key, stage text) reports have arrived since the last poll.
    pub fn poll_stages(&mut self) -> Vec<(String, String)> {
        let mut out = Vec::new();
        while let Ok(msg) = self.stage_rx.try_recv() {
            out.push(msg);
        }
        out
    }
}
