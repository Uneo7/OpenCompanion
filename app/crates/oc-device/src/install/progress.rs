use oc_install::Plan;

use super::flow::{Chip, Flow, StepId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StepState {
    Pending,
    Running,
    Done,
    Error,
    Skipped,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepProgress {
    pub id: StepId,
    pub weight: u32,
    pub state: StepState,
    pub percent: u8,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Phase {
    Confirm,
    Running,
    AwaitingConfirmation,
    Done,
    Failed,
}

/// Current → target version of one chip. `None` is the stock Bose firmware when `*_stock` is set, else a
/// version that could not be read ([`super::versions::label`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipVersion {
    pub chip: Chip,
    pub from: Option<String>,
    pub to: Option<String>,
    pub from_stock: bool,
    pub to_stock: bool,
    /// `from` unknown, most likely the Bose firmware (garbled reply).
    pub from_probable: bool,
}

impl ChipVersion {
    pub fn unchanged(&self) -> bool {
        // An unknown version is never "already up to date".
        if self.from.is_none() && !(self.from_stock && self.to_stock) {
            return false;
        }
        let norm = |v: &Option<String>| v.as_deref().map(|s| super::check::normalize(s).to_owned());
        self.from_stock == self.to_stock && norm(&self.from) == norm(&self.to)
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Progress {
    pub flow: Flow,
    pub phase: Phase,
    pub steps: Vec<StepProgress>,
    pub current: Option<usize>,
    pub versions: Vec<ChipVersion>,
    /// Log language: English.
    pub error: Option<String>,
    pub can_resume: bool,
    pub plan: Option<Plan>,
    pub log: Vec<String>,
    pub power_cycle: bool,
    /// The user declined a write plan: nothing was written by the step that asked; back to Confirm.
    pub cancelled: bool,
    /// Chips already in place ([`Progress::skip_chip`]): complete from the start.
    pub skipped_chips: Vec<Chip>,
}

impl Progress {
    pub fn new(flow: Flow) -> Self {
        let steps = flow
            .steps()
            .into_iter()
            .map(|s| StepProgress { id: s.id, weight: s.weight, state: StepState::Pending, percent: 0 })
            .collect();
        let versions = flow.chips().into_iter().map(|chip| ChipVersion { chip, from: None, to: None, from_stock: false, to_stock: false, from_probable: false }).collect();
        Self {
            flow,
            phase: Phase::Confirm,
            steps,
            current: None,
            versions,
            error: None,
            can_resume: false,
            plan: None,
            log: Vec::new(),
            power_cycle: false,
            cancelled: false,
            skipped_chips: Vec::new(),
        }
    }

    pub fn percent(&self) -> u8 {
        if self.phase == Phase::Done {
            return 100;
        }
        let total: u32 = self.steps.iter().map(|s| s.weight).sum();
        let done: u32 = self
            .steps
            .iter()
            .map(|s| match s.state {
                StepState::Done | StepState::Skipped => s.weight * 100,
                StepState::Running | StepState::Error => s.weight * u32::from(s.percent),
                StepState::Pending => 0,
            })
            .sum();
        done.checked_div(total).map_or(0, |v| v.min(100) as u8)
    }

    pub fn current_step(&self) -> Option<&StepProgress> {
        self.current.and_then(|i| self.steps.get(i))
    }

    pub fn current_key(&self) -> Option<&'static str> {
        self.current_step().map(|s| s.id.key())
    }

    pub fn writing(&self) -> bool {
        self.phase == Phase::Running && self.current_step().is_some_and(|s| s.id.writes())
    }

    pub fn critical(&self) -> bool {
        self.phase == Phase::Running && self.current_step().is_some_and(|s| s.id.critical())
    }

    /// The first group is the preparation.
    pub fn groups(&self) -> Vec<(&'static str, u32)> {
        let mut out: Vec<(&'static str, u32)> = Vec::new();
        for s in &self.steps {
            // The backup (first install) shares the preparation's group.
            let key = s.id.chip().map_or_else(|| self.flow.prep_key(), Chip::key);
            match out.iter_mut().find(|(k, _)| *k == key) {
                Some(g) => g.1 += s.weight,
                None => out.push((key, s.weight)),
            }
        }
        out.retain(|g| g.1 > 0);
        out
    }

    pub fn error_chip_key(&self) -> &'static str {
        self.current_step().and_then(|s| s.id.chip()).map_or_else(|| self.flow.prep_key(), Chip::key)
    }

    /// Enters step `i`: earlier steps that ran are done, earlier steps that never ran are skipped.
    pub(crate) fn enter(&mut self, i: usize) {
        if self.current == Some(i) {
            return;
        }
        for (j, s) in self.steps.iter_mut().enumerate().take(i) {
            if matches!(s.state, StepState::Running | StepState::Error) || (Some(j) == self.current) {
                s.state = StepState::Done;
                s.percent = 100;
            } else if s.state == StepState::Pending {
                s.state = StepState::Skipped;
            }
        }
        if let Some(s) = self.steps.get_mut(i) {
            s.state = StepState::Running;
            s.percent = 0;
        }
        self.current = Some(i);
    }

    /// The steps of `chip` that have not run are already in place (up to date, or verified earlier): shown
    /// complete from the start, so that the bar shows only what remains.
    pub(crate) fn skip_chip(&mut self, chip: Chip) {
        if !self.skipped_chips.contains(&chip) {
            self.skipped_chips.push(chip);
        }
        for s in self.steps.iter_mut().filter(|s| s.id.chip() == Some(chip) && s.state == StepState::Pending) {
            s.state = StepState::Skipped;
        }
    }

    pub(crate) fn step_percent(&mut self, p: u8) {
        if let Some(s) = self.current.and_then(|i| self.steps.get_mut(i)) {
            s.percent = s.percent.max(p.min(99));
        }
    }

    /// Ends the flow: the running step is done, untouched steps are skipped.
    pub(crate) fn finish(&mut self) {
        let n = self.steps.len();
        self.enter(n);
        self.current = None;
        self.phase = Phase::Done;
        self.plan = None;
    }

    pub(crate) fn fail(&mut self, error: String, can_resume: bool) {
        if let Some(s) = self.current.and_then(|i| self.steps.get_mut(i)) {
            s.state = StepState::Error;
        }
        self.phase = Phase::Failed;
        self.error = Some(error);
        self.can_resume = can_resume;
        self.plan = None;
    }

    /// Back to a fresh run of the same flow (resume: the flows are idempotent and skip what is in place).
    pub(crate) fn restart(&mut self) {
        let log = std::mem::take(&mut self.log);
        let versions = std::mem::take(&mut self.versions);
        // Chips found already in place stay complete.
        let chips = std::mem::take(&mut self.skipped_chips);
        *self = Self::new(self.flow.clone());
        for c in chips {
            self.skip_chip(c);
        }
        self.log = log;
        self.versions = versions;
        self.phase = Phase::Running;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn weighted_percentage_and_groups() {
        let mut p = Progress::new(Flow::First);
        assert_eq!(p.percent(), 0);
        assert_eq!(
            p.groups(),
            vec![("inst.step.prep", 20), ("inst.chip.dsp", 56), ("inst.chip.usb", 26)]
        );
        p.phase = Phase::Running;
        p.enter(0);
        p.step_percent(50);
        assert_eq!(p.percent(), 5);
        p.enter(1);
        p.step_percent(50);
        assert_eq!(p.percent(), 15);
        p.enter(3); // straight to the critical window: prep done, dsp.boot skipped
        assert_eq!(p.steps[2].state, StepState::Skipped);
        assert_eq!(p.steps[1].state, StepState::Done);
        assert!(p.critical() && p.writing());
        assert_eq!(p.percent(), 33);
        p.fail("link lost".into(), true);
        assert_eq!(p.error_chip_key(), "inst.chip.dsp");
        assert_eq!(p.steps[3].state, StepState::Error);
        p.restart();
        assert_eq!((p.phase, p.current, p.percent()), (Phase::Running, None, 0));
        p.finish();
        assert_eq!(p.percent(), 100);
        assert!(p.steps.iter().all(|s| s.state == StepState::Skipped));
    }

    #[test]
    fn chips_in_place_are_complete_from_the_start_and_stay_so() {
        let mut p = Progress::new(Flow::Update);
        p.skip_chip(Chip::Dsp);
        let dsp: u32 = p.steps.iter().filter(|s| s.id.chip() == Some(Chip::Dsp)).map(|s| s.weight).sum();
        let total: u32 = p.steps.iter().map(|s| s.weight).sum();
        assert_eq!(u32::from(p.percent()), dsp * 100 / total);
        p.restart();
        assert!(p.steps.iter().filter(|s| s.id.chip() == Some(Chip::Dsp)).all(|s| s.state == StepState::Skipped));
        assert!(p.steps.iter().filter(|s| s.id.chip() != Some(Chip::Dsp)).all(|s| s.state == StepState::Pending));
    }

    #[test]
    fn an_unknown_version_is_never_already_up_to_date() {
        let v = |from: Option<&str>, to: Option<&str>| ChipVersion {
            chip: Chip::Dsp,
            from: from.map(Into::into),
            to: to.map(Into::into),
            from_stock: false,
            to_stock: false,
            from_probable: from.is_none(),
        };
        assert!(!v(None, None).unchanged(), "unknown and no target");
        assert!(!v(None, Some("0.2.1")).unchanged());
        assert!(v(Some("OC0.2.1+1"), Some("0.2.1+1")).unchanged());
        assert!(!v(Some("0.4.9"), Some("0.4.10")).unchanged());
    }
}
