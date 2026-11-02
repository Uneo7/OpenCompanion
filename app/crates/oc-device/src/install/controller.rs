use std::sync::{Arc, Mutex};
use std::thread::JoinHandle;
use std::time::Duration;

use super::auto_backup::{self, BackupState};
use super::backup;
use super::check::{self, UpdateCheck};
use super::flow::Flow;
use super::host::{Notice, Shared, UiHost};
use super::progress::{Phase, Progress};
use super::runner::{self, RunError};
use super::sources::FirmwareSources;
use crate::worker::LinkLease;

/// How long the worker gets to close the speaker before an install starts.
const LEASE_TIMEOUT: Duration = Duration::from_secs(5);

pub struct InstallController {
    shared: Arc<Shared>,
    lease: LinkLease,
    thread: Option<JoinHandle<()>>,
    backup: Arc<Mutex<BackupState>>,
    backup_thread: Option<JoinHandle<()>>,
    updates: Arc<Mutex<UpdateCheck>>,
    check_thread: Option<JoinHandle<()>>,
}

impl InstallController {
    /// `lease` = the device worker's ([`crate::worker::Worker::lease`]), or [`LinkLease::standalone`].
    pub fn new(sources: FirmwareSources, lease: LinkLease) -> Self {
        let backup = Arc::new(Mutex::new(BackupState::from_disk(&sources)));
        let shared = Shared::with_sources(Progress::new(Flow::Update), sources);
        Self {
            shared,
            lease,
            thread: None,
            backup,
            backup_thread: None,
            updates: Arc::default(),
            check_thread: None,
        }
    }

    /// Called from the install thread on every change, and with the plan when a confirmation is needed
    /// (answer with [`InstallController::answer`]). Typically requests a repaint.
    pub fn set_notifier(&self, f: impl Fn(Notice) + Send + Sync + 'static) {
        *self.shared.notifier.lock().unwrap_or_else(|e| e.into_inner()) = Some(Arc::new(f));
    }

    pub fn sources(&self) -> FirmwareSources {
        self.shared.sources()
    }

    /// Feature `github`; without it, nothing happens.
    pub fn enable_releases(&self, api_base: &str, repo: &str) {
        self.shared.release().remote = Some((api_base.into(), repo.into()));
        super::flow::set_download_step(cfg!(feature = "github"));
    }

    pub fn app_update(&self) -> Option<super::AppUpdate> {
        self.shared.release().app_update.clone()
    }

    pub fn release_status(&self) -> Option<super::host::ReleaseStatus> {
        self.shared.release().status.clone()
    }

    pub fn is_running(&self) -> bool {
        self.thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    pub fn progress(&self) -> Progress {
        self.shared.lock().clone()
    }

    pub fn with_progress<R>(&self, f: impl FnOnce(&Progress) -> R) -> R {
        f(&self.shared.lock())
    }

    pub fn phase(&self) -> Phase {
        self.shared.lock().phase
    }

    /// Refused while a flow runs.
    pub fn open(&mut self, flow: Flow) -> bool {
        if self.is_running() {
            return false;
        }
        let mut p = Progress::new(flow);
        let running = self.shared.running();
        let sources = self.shared.sources();
        for v in &mut p.versions {
            let r = match v.chip {
                super::flow::Chip::Dsp => &running.dsp,
                super::flow::Chip::Usb => &running.usb,
            };
            v.from = r.version();
            v.from_stock = r.is_stock();
            v.from_probable = r.is_probably_stock();
            v.to_stock = p.flow == Flow::Restore;
            // The target comes from the firmware files: known before the start, without the speaker.
            if !v.to_stock {
                v.to = sources.target_version(&p.flow, v.chip);
            }
        }
        // Chips already in place are complete from the start: the DSP of an update when the check found it up to
        // date, the DSP of a restore already verified.
        let dsp_current = matches!(&*self.updates.lock().unwrap_or_else(|e| e.into_inner()),
            UpdateCheck::Done(c) if c.iter().any(|u| u.chip == super::flow::Chip::Dsp && !u.available));
        if (p.flow == Flow::Update && dsp_current) || (p.flow == Flow::Restore && super::runner::restore_in_progress()) {
            p.skip_chip(super::flow::Chip::Dsp);
        }
        *self.shared.lock() = p;
        self.shared.notify(Notice::Changed);
        self.check_updates();
        true
    }

    /// Also resumes after a failure: every flow re-reads the device and skips what is already in place.
    pub fn start(&mut self) -> bool {
        if self.is_running() || self.backup_running() {
            return false;
        }
        let flow = {
            let mut p = self.shared.lock();
            let ok = match p.phase {
                Phase::Confirm => true,
                Phase::Failed => p.can_resume,
                _ => false,
            };
            if !ok {
                return false;
            }
            p.restart();
            p.flow.clone()
        };
        self.shared.notify(Notice::Changed);
        let (shared, sources, lease) = (self.shared.clone(), self.shared.sources(), self.lease.clone());
        // An update check started when the view opened ends first (one user of the speaker at a time).
        let check = self.check_thread.take();
        self.thread = Some(
            std::thread::Builder::new()
                .name("oc-install".into())
                .spawn(move || {
                    if let Some(c) = check {
                        let _ = c.join();
                    }
                    body(&flow, &sources, &shared, &lease)
                })
                .expect("spawning the install thread"),
        );
        true
    }

    pub fn resume(&mut self) -> bool {
        self.start()
    }

    pub fn answer(&self, accept: bool) {
        self.shared.answer(accept);
    }

    pub fn last_backup(&self) -> Option<String> {
        backup::latest(&self.shared.sources().backups).map(|(d, _)| d)
    }

    /// To call when the app connects to a speaker: makes sure a verified backup of its Bose firmware exists,
    /// reading both EEPROMs if needed (nothing is written; ~2 min the first time, a few seconds after). Runs in
    /// the background; [`InstallController::stock_backup`] tells the result. `false` if busy.
    pub fn ensure_stock_backup(&mut self) -> bool {
        if self.is_running() || self.backup_running() || self.check_running() {
            return false;
        }
        *self.backup.lock().unwrap_or_else(|e| e.into_inner()) = BackupState::Running;
        self.shared.notify(Notice::Changed);
        let (sources, lease, state, shared) = (self.shared.sources(), self.lease.clone(), self.backup.clone(), self.shared.clone());
        self.backup_thread = Some(
            std::thread::Builder::new()
                .name("oc-backup".into())
                .spawn(move || auto_backup::run(&sources, &lease, &state, &shared))
                .expect("spawning the backup thread"),
        );
        true
    }

    pub fn stock_backup(&self) -> BackupState {
        self.backup.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    fn backup_running(&self) -> bool {
        self.backup_thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// Runs in the background, writes nothing; `false` if busy.
    pub fn check_updates(&mut self) -> bool {
        if self.check_running() || self.is_running() || self.backup_running() {
            return false;
        }
        *self.updates.lock().unwrap_or_else(|e| e.into_inner()) = UpdateCheck::Running;
        self.shared.notify(Notice::Changed);
        let (lease, state, shared) = (self.lease.clone(), self.updates.clone(), self.shared.clone());
        self.check_thread = Some(
            std::thread::Builder::new()
                .name("oc-update-check".into())
                .spawn(move || check::run(&lease, &state, &shared))
                .expect("spawning the update check thread"),
        );
        true
    }

    fn check_running(&self) -> bool {
        self.check_thread.as_ref().is_some_and(|t| !t.is_finished())
    }

    /// What the device worker sees running (versions from `SVM` / `INFO`): the fallback of the installer and
    /// the update check when a read of their own fails, and the versions shown before a flow starts.
    pub fn set_running(&self, r: super::versions::RunningVersions) {
        let mut cur = self.shared.running.lock().unwrap_or_else(|e| e.into_inner());
        if *cur != r {
            *cur = r;
        }
    }

    pub fn updates(&self) -> UpdateCheck {
        self.updates.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }
}

impl Drop for InstallController {
    fn drop(&mut self) {
        // Unblock a pending confirmation (declined: nothing is written after a refusal). A running write keeps
        // going on its thread until the process exits; the app must not close during an install.
        self.shared.answer(false);
    }
}

fn body(flow: &Flow, sources: &FirmwareSources, shared: &Arc<Shared>, lease: &LinkLease) {
    let Some(guard) = lease.acquire(LEASE_TIMEOUT) else {
        shared.update(|p| p.fail("the speaker is busy: the device thread did not release it".into(), true));
        return;
    };
    let mut host = UiHost::new(shared.clone());
    let r = runner::run(flow, sources, shared, &mut host);
    drop(host); // releases the keep-awake guard
    drop(guard); // the worker reconnects
    shared.update(|p| match r {
        Ok(o) => {
            p.power_cycle = o.power_cycle;
            p.finish();
            if o.power_cycle {
                p.log.push("power-cycle the speaker to start the new USB controller firmware".into());
            }
        }
        Err(RunError::Cancelled) => {
            *p = Progress { log: std::mem::take(&mut p.log), versions: std::mem::take(&mut p.versions), ..Progress::new(p.flow.clone()) };
            p.log.push("cancelled: nothing was written by the declined step".into());
            p.cancelled = true;
        }
        Err(RunError::Failed { msg, resumable }) => p.fail(msg, resumable),
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::install::Chip;

    #[test]
    fn a_flow_without_inputs_fails_before_touching_the_device() {
        let dir = std::env::temp_dir().join("oc-install-no-such-dir");
        let mut c = InstallController::new(FirmwareSources::from_dir(&dir), LinkLease::standalone());
        assert!(c.open(Flow::File(Chip::Dsp, dir.join("x.out"))));
        assert_eq!(c.progress().phase, Phase::Confirm);
        assert!(c.start());
        while c.is_running() {
            std::thread::sleep(Duration::from_millis(5));
        }
        let p = c.progress();
        assert_eq!(p.phase, Phase::Failed);
        assert!(!p.can_resume, "a missing file is not resumable");
        assert_eq!(p.current_key(), Some("inst.step.file.check"));
        assert!(!c.resume());
    }
}
