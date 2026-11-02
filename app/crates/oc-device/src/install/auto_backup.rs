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

/// Reads only: no confirmation is ever needed, log lines are dropped.
struct Quiet;

impl Host for Quiet {
    fn log(&mut self, _line: &str) {}

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
    let Some(guard) = lease.acquire(std::time::Duration::from_secs(5)) else {
        set(BackupState::Failed("the speaker is busy".into()));
        return;
    };
    let r = runner::open().map_err(|_| "no speaker".to_string()).and_then(|conn| backup::ensure(sources, &conn, &mut Quiet));
    drop(guard);
    set(match r {
        Ok(b) => BackupState::Saved { date: b.entry.date().to_string(), dir: b.entry.dir.clone(), created: b.created },
        Err(e) => BackupState::Failed(e),
    });
}
