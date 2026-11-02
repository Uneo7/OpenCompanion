//! Reads both EEPROMs; nothing is written.

use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use oc_install::{Host, Plan};

use super::backup;
use super::host::{Notice, Shared};
use super::runner;
use super::sources::FirmwareSources;
use crate::worker::LinkLease;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BackupState {
    Missing,
    Running,
    /// `date` = `YYYY-MM-DD`, UTC; `created` = made by this session.
    Saved { date: String, dir: PathBuf, created: bool },
    /// Nothing written to the speaker; retried at the next connection.
    Failed(String),
}

impl BackupState {
    /// No device access.
    pub fn from_disk(sources: &FirmwareSources) -> Self {
        match backup::stock(&sources.backups) {
            Some(e) => Self::Saved { date: e.date().to_string(), dir: e.dir, created: false },
            None => Self::Missing,
        }
    }
}

/// Reads only: no confirmation is ever needed; the log lines go to the install log once done.
#[derive(Default)]
struct Quiet {
    lines: Vec<String>,
}

impl Host for Quiet {
    fn log(&mut self, line: &str) {
        self.lines.push(line.to_owned());
    }

    fn confirm(&mut self, _plan: &Plan) -> bool {
        false
    }

    fn keep_awake(&mut self, _on: bool) {}
}

pub(super) fn run(sources: &FirmwareSources, lease: &LinkLease, state: &Arc<Mutex<BackupState>>, shared: &Arc<Shared>) {
    let set = |s: BackupState| {
        *state.lock().unwrap_or_else(|e| e.into_inner()) = s;
        shared.notify(Notice::Changed);
    };
    super::log_file::append(&[format!("stock backup: starting (into {})", sources.backups.display())]);
    // The RAM loaders that read U7001 ship with the firmware: a release build without one downloads it first.
    let sources = match with_loaders(sources, shared) {
        Ok(s) => s,
        Err(e) => {
            super::log_file::append(&[format!("stock backup FAILED: {e}")]);
            set(BackupState::Failed(e));
            return;
        }
    };
    if let Err(e) = std::fs::create_dir_all(&sources.backups) {
        let e = format!("{}{}: {e}", NOT_WRITABLE, sources.backups.display());
        super::log_file::append(&[format!("stock backup FAILED: {e}")]);
        set(BackupState::Failed(e));
        return;
    }
    let Some(guard) = lease.acquire(std::time::Duration::from_secs(5)) else {
        super::log_file::append(&["stock backup FAILED: the speaker is busy"]);
        set(BackupState::Failed("the speaker is busy".into()));
        return;
    };
    let mut host = Quiet::default();
    let r = runner::open()
        .map_err(|e| match e {
            runner::RunError::Failed { msg, .. } => format!("{NO_USB}{msg}"),
            runner::RunError::Cancelled => format!("{NO_USB}cancelled"),
        })
        .and_then(|conn| backup::ensure(&sources, &conn, &mut host));
    drop(guard);
    super::log_file::append(&host.lines);
    match &r {
        Ok(b) => super::log_file::append(&[format!("stock backup: OK ({}, created: {})", b.entry.dir.display(), b.created)]),
        Err(e) => super::log_file::append(&[format!("stock backup FAILED: {e}")]),
    }
    set(match r {
        Ok(b) => BackupState::Saved { date: b.entry.date().to_string(), dir: b.entry.dir.clone(), created: b.created },
        Err(e) => BackupState::Failed(e),
    });
}

/// Start of a backup error: the backups folder cannot be created (the path and the OS error follow).
pub const NOT_WRITABLE: &str = "backup folder not writable: ";
/// Start of a backup error: the speaker's USB HID interface could not be opened (the OS error follows).
pub const NO_USB: &str = "cannot open the speaker over USB: ";

#[cfg(feature = "github")]
fn with_loaders(sources: &FirmwareSources, shared: &Shared) -> Result<FirmwareSources, String> {
    if sources.loaders().is_ok() {
        return Ok(sources.clone());
    }
    if shared.release().pending.is_none() {
        super::check::refresh_release(shared);
    }
    let pending = shared.release().pending.clone();
    let s = match pending {
        Some(av) => runner::download_release(shared, &av, |_| {})?,
        None => shared.sources(),
    };
    s.loaders().map(|_| s.clone()).map_err(|e| match shared.release().status.clone() {
        Some(st) => format!("{}: {st}", super::NO_FIRMWARE),
        None => format!("{}: {e}", super::NO_FIRMWARE),
    })
}

#[cfg(not(feature = "github"))]
fn with_loaders(sources: &FirmwareSources, _shared: &Shared) -> Result<FirmwareSources, String> {
    sources.loaders().map(|_| sources.clone()).map_err(|e| format!("{}: {e}", super::NO_FIRMWARE))
}
