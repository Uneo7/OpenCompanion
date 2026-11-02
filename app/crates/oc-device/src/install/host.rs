use std::sync::{Arc, Condvar, Mutex, MutexGuard};

use oc_install::awake::KeepAwake;
use oc_install::{Host, Plan, UiStep};

use super::flow::StepId;
use super::progress::{Phase, Progress};

#[derive(Debug, Clone, PartialEq)]
pub enum Notice {
    Changed,
    Confirm(Plan),
}

pub type Notifier = Arc<dyn Fn(Notice) + Send + Sync>;

pub struct Shared {
    pub progress: Mutex<Progress>,
    answer: Mutex<Option<bool>>,
    answered: Condvar,
    pub notifier: Mutex<Option<Notifier>>,
    /// Fallback when a read during a flow fails.
    pub running: Mutex<super::versions::RunningVersions>,
    pub sources: Mutex<super::FirmwareSources>,
    pub release: Mutex<ReleaseState>,
}

/// `Display` is the log / CLI form.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReleaseStatus {
    InUse { tag: String },
    Available { tag: String },
    NeedsApp { tag: String, min: String },
    /// No release could be used (`error` says why): the newest downloaded one, if any, is used.
    Unavailable { cached: Option<String>, error: super::ReleaseError },
}

impl std::fmt::Display for ReleaseStatus {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::InUse { tag } => write!(f, "firmware {tag} (GitHub)"),
            Self::Available { tag } => write!(f, "firmware {tag} available on GitHub"),
            Self::NeedsApp { tag, min } => write!(f, "release {tag} needs app {min} or newer"),
            Self::Unavailable { cached: Some(tag), error } => write!(f, "{error}: firmware {tag} (downloaded) in use"),
            Self::Unavailable { cached: None, error } => write!(f, "{error}: no firmware downloaded yet"),
        }
    }
}

/// A newer app on GitHub (release builds): its version, the release page, and the firmware tag that needs it
/// (that firmware is not used before the app is updated).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AppUpdate {
    pub version: String,
    pub url: String,
    pub needed_by_firmware: Option<String>,
}

#[derive(Debug, Clone, Default)]
pub struct ReleaseState {
    pub remote: Option<(String, String)>,
    pub status: Option<ReleaseStatus>,
    pub app_update: Option<AppUpdate>,
    #[cfg(feature = "github")]
    pub pending: Option<super::release::github::Available>,
}

impl Shared {
    #[cfg(test)]
    pub fn new(progress: Progress) -> Arc<Self> {
        Self::with_sources(progress, super::FirmwareSources::from_dir(std::path::Path::new("")))
    }

    pub fn with_sources(progress: Progress, sources: super::FirmwareSources) -> Arc<Self> {
        Arc::new(Self {
            progress: Mutex::new(progress),
            answer: Mutex::new(None),
            answered: Condvar::new(),
            notifier: Mutex::new(None),
            running: Mutex::default(),
            sources: Mutex::new(sources),
            release: Mutex::default(),
        })
    }

    pub fn lock(&self) -> MutexGuard<'_, Progress> {
        self.progress.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn sources(&self) -> super::FirmwareSources {
        self.sources.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    #[cfg_attr(not(feature = "github"), allow(dead_code))]
    pub fn set_sources(&self, s: super::FirmwareSources) {
        *self.sources.lock().unwrap_or_else(|e| e.into_inner()) = s;
    }

    pub fn release(&self) -> std::sync::MutexGuard<'_, ReleaseState> {
        self.release.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn running(&self) -> super::versions::RunningVersions {
        self.running.lock().unwrap_or_else(|e| e.into_inner()).clone()
    }

    pub fn notify(&self, n: Notice) {
        let f = self.notifier.lock().unwrap_or_else(|e| e.into_inner()).clone();
        if let Some(f) = f {
            f(n);
        }
    }

    /// Also appended to the persistent log ([`super::log_file`]).
    pub fn update(&self, f: impl FnOnce(&mut Progress)) {
        let mut out = Vec::new();
        {
            let mut p = self.lock();
            let (n, phase, step, error) = (p.log.len(), p.phase, p.current_key(), p.error.clone());
            f(&mut p);
            if p.log.len() > n {
                out.extend(p.log[n..].iter().cloned());
            }
            if p.phase != phase || p.current_key() != step {
                out.push(format!("[{:?}] {:?} step {}", p.flow, p.phase, p.current_key().unwrap_or("-")));
            }
            if p.error != error {
                if let Some(e) = &p.error {
                    out.push(format!("ERROR: {e} (resumable: {})", p.can_resume));
                }
            }
        }
        super::log_file::append(&out);
        self.notify(Notice::Changed);
    }

    pub fn answer(&self, accept: bool) {
        *self.answer.lock().unwrap_or_else(|e| e.into_inner()) = Some(accept);
        self.answered.notify_all();
    }

    fn clear_answer(&self) {
        *self.answer.lock().unwrap_or_else(|e| e.into_inner()) = None;
    }

    fn wait_answer(&self) -> bool {
        let a = self.answer.lock().unwrap_or_else(|e| e.into_inner());
        let a = self.answered.wait_while(a, |a| a.is_none()).unwrap_or_else(|e| e.into_inner());
        a.unwrap_or(false)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Signal {
    Enter(StepId),
    Pages(usize, usize),
}

/// Maps the log lines of `oc_install::dsp::install` and `oc_install::tas::install` to steps.
pub fn classify(line: &str) -> Option<Signal> {
    let l = line.trim_start();
    if let Some((done, total)) = pages(l) {
        return Some(Signal::Pages(done, total));
    }
    let enter = |id| Some(Signal::Enter(id));
    match () {
        _ if l.starts_with("DSP: ") || l.starts_with("M1:") || l.starts_with("loading the updater") => enter(StepId::Prep),
        _ if l.starts_with("reading U7001") || l.starts_with("survey:") => enter(StepId::Prep),
        _ if l.starts_with("M2:") => enter(StepId::DspBoot),
        _ if l.starts_with("M3:") => enter(StepId::DspCritical),
        _ if l.starts_with("M4:") => enter(StepId::DspWrite),
        _ if l.starts_with("M5:") || l.starts_with("U7001 already holds") => enter(StepId::DspVerify),
        _ if l.starts_with("reading the TAS EEPROM") || l.starts_with("marking the TAS EEPROM") => enter(StepId::UsbWrite),
        _ if l.starts_with("writing the final header") => enter(StepId::UsbWrite),
        _ if l.starts_with("verifying the whole TAS EEPROM") || l.starts_with("the target image is already installed") => {
            enter(StepId::UsbVerify)
        }
        _ => None,
    }
}

/// `"code 12/191 pages"`, `"body: 32/384 pages"` → `(12, 191)`.
fn pages(l: &str) -> Option<(usize, usize)> {
    let l = l.strip_suffix(" pages")?;
    let frac = l.rsplit(' ').next()?;
    let (a, b) = frac.split_once('/')?;
    Some((a.parse().ok()?, b.parse().ok()?))
}

pub struct UiHost {
    shared: Arc<Shared>,
    awake: Option<KeepAwake>,
    /// The user confirmed a plan of this run: it covers the whole flow (no second confirmation).
    confirmed: bool,
}

impl UiHost {
    pub fn new(shared: Arc<Shared>) -> Self {
        Self { shared, awake: None, confirmed: false }
    }

    /// The steps of `chip` are already in place: complete at once.
    pub fn skip_chip(&mut self, chip: super::flow::Chip) {
        self.shared.update(|p| p.skip_chip(chip));
    }

    pub fn enter(&mut self, id: StepId) {
        self.shared.update(|p| {
            if let Some(i) = p.flow.resolve(id) {
                // Never go back: a late "reading…" line during the TAS install must not reopen the preparation.
                if p.current.is_none_or(|c| i >= c) {
                    p.enter(i);
                }
            }
        });
    }
}

/// In a file flow the TAS EEPROM is read to build the write plan: that read belongs to the file check, it
/// must not start the write step (the user has not confirmed anything yet).
/// While the backup reads the speaker, its "reading U7001" / loader lines must not start the preparation step
/// (the runner enters it once the backup is verified).
fn reading_during_backup(p: &Progress, line: &str) -> bool {
    p.current_step().is_some_and(|s| s.id == StepId::Backup) && matches!(classify(line), Some(Signal::Enter(StepId::Prep)))
}

fn reading_during_check(p: &Progress, line: &str) -> bool {
    matches!(p.flow, super::flow::Flow::File(..))
        && line.trim_start().starts_with("reading the TAS EEPROM")
        && p.current_step().is_some_and(|s| s.id == StepId::FileCheck)
}

impl Host for UiHost {
    fn log(&mut self, line: &str) {
        let in_check = {
            let p = self.shared.lock();
            reading_during_check(&p, line) || reading_during_backup(&p, line)
        };
        match classify(line) {
            Some(Signal::Enter(_)) if in_check => {}
            Some(Signal::Enter(id)) => self.enter(id),
            Some(Signal::Pages(done, total)) if total > 0 => {
                let pct = (done * 100 / total).min(99) as u8;
                self.shared.update(|p| p.step_percent(pct));
            }
            _ => {}
        }
        self.shared.update(|p| p.log.push(line.to_owned()));
    }

    fn confirm(&mut self, plan: &Plan) -> bool {
        if self.confirmed {
            self.shared.update(|p| p.log.push(format!("{}: covered by the confirmation at the start", plan.title)));
            return true;
        }
        self.shared.clear_answer();
        self.shared.update(|p| {
            p.phase = Phase::AwaitingConfirmation;
            p.plan = Some(plan.clone());
        });
        self.shared.notify(Notice::Confirm(plan.clone()));
        let ok = self.shared.wait_answer();
        self.shared.update(|p| {
            p.plan = None;
            p.phase = Phase::Running;
            p.log.push(if ok { "confirmed by the user".into() } else { "cancelled by the user".into() });
        });
        self.confirmed = ok;
        ok
    }

    fn step(&mut self, step: UiStep) {
        self.enter(StepId::from_ui(step));
    }

    fn keep_awake(&mut self, on: bool) {
        if !on {
            self.awake = None;
            return;
        }
        match KeepAwake::acquire("OpenCompanion is writing the speaker's EEPROM") {
            Ok(g) => self.awake = Some(g),
            Err(e) => self.log(&format!(
                "WARNING: cannot keep the computer awake ({e}); disable sleep manually until the end"
            )),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn install_log_lines_map_to_steps() {
        assert_eq!(classify("M1: installing the RAM loader stub (BWW, verified with BRR)…"), Some(Signal::Enter(StepId::Prep)));
        assert_eq!(classify("M2: writing boot1 (stock firmware still boots during this step)…"), Some(Signal::Enter(StepId::DspBoot)));
        assert_eq!(classify("M3: entering the critical window (boot0, both pages in one updater command)…"), Some(Signal::Enter(StepId::DspCritical)));
        assert_eq!(classify("M4: writing the firmware body…"), Some(Signal::Enter(StepId::DspWrite)));
        assert_eq!(classify("M5: rebooting the DSP from U7001…"), Some(Signal::Enter(StepId::DspVerify)));
        assert_eq!(classify("  body: 32/384 pages"), Some(Signal::Pages(32, 384)));
        assert_eq!(classify("  code 191/191 pages"), Some(Signal::Pages(191, 191)));
        assert_eq!(classify("marking the TAS EEPROM header \"download in progress\" (DataType 3)…"), Some(Signal::Enter(StepId::UsbWrite)));
        assert_eq!(classify("verifying the whole TAS EEPROM…"), Some(Signal::Enter(StepId::UsbVerify)));
        assert_eq!(classify("anything else"), None);
    }

    #[test]
    fn one_confirmation_covers_the_whole_flow() {
        let shared = Shared::new(Progress::new(super::super::Flow::First));
        let mut host = UiHost::new(shared.clone());
        host.confirmed = true;
        assert!(host.confirm(&Plan { title: "TAS EEPROM install".into(), ..Default::default() }));
        let p = shared.lock();
        assert_eq!(p.phase, Phase::Confirm, "no second dialog");
        assert!(p.log.last().is_some_and(|l| l.contains("covered by the confirmation")));
    }

    #[test]
    fn confirmation_waits_for_the_ui() {
        let shared = Shared::new(Progress::new(super::super::Flow::Update));
        let seen = Arc::new(Mutex::new(Vec::new()));
        let s2 = seen.clone();
        let sh = shared.clone();
        *shared.notifier.lock().unwrap() = Some(Arc::new(move |n| {
            if let Notice::Confirm(_) = n {
                s2.lock().unwrap().push("confirm");
                sh.answer(true);
            }
        }));
        let mut host = UiHost::new(shared.clone());
        assert!(host.confirm(&Plan { title: "t".into(), ..Default::default() }));
        assert_eq!(*seen.lock().unwrap(), ["confirm"]);
        assert_eq!(shared.lock().phase, Phase::Running);
    }

    #[test]
    fn the_backup_is_a_step_and_its_reads_do_not_start_the_preparation() {
        let shared = Shared::new(Progress::new(super::super::Flow::First));
        let mut host = UiHost::new(shared.clone());
        host.enter(StepId::Backup);
        host.log("reading U7001 through the stock loader…");
        assert_eq!(shared.lock().current_step().map(|s| s.id), Some(StepId::Backup));
        host.enter(StepId::Prep);
        assert_eq!(shared.lock().current_step().map(|s| s.id), Some(StepId::Prep));
    }

    #[test]
    fn the_eeprom_read_of_a_file_flow_stays_in_the_file_check() {
        let flow = super::super::Flow::File(super::super::Chip::Usb, "x.bin".into());
        let shared = Shared::new(Progress::new(flow));
        let mut host = UiHost::new(shared.clone());
        host.enter(StepId::FileCheck);
        host.log("reading the TAS EEPROM (8192 bytes)…");
        assert_eq!(shared.lock().current_key(), Some("inst.step.file.check"));
        host.log("marking the TAS EEPROM header \"download in progress\" (DataType 3)…");
        assert_eq!(shared.lock().current_step().map(|s| s.id), Some(StepId::UsbWrite));
    }
}
