use oc_device::install::{Chip, Flow, Phase, Progress, StepId, StepState};

use crate::model::device::Status;

const TICK_S: f64 = 0.12;

#[derive(Clone)]
pub struct Demo {
    pub progress: Progress,
    i: usize,
    p: f32,
    last: f64,
    /// Step that loses the connection once, past 42 %.
    pub fail_at: Option<StepId>,
    /// Stops on the sensitive step past 40 % (screenshots).
    pub hold_critical: bool,
    pub frozen: bool,
}

fn versions(flow: &Flow, status: &Status) -> Vec<(Chip, Option<String>, Option<String>)> {
    let (dsp, usb) = (Some(status.firmware_dsp.clone()), Some(status.firmware_usb.clone()));
    match flow {
        Flow::First => vec![(Chip::Dsp, None, dsp), (Chip::Usb, None, usb)],
        Flow::Update => vec![(Chip::Dsp, dsp, Some("0.9.4".into())), (Chip::Usb, usb.clone(), usb)],
        Flow::Restore => vec![(Chip::Dsp, dsp, None), (Chip::Usb, usb, None)],
        Flow::File(Chip::Dsp, _) => vec![(Chip::Dsp, dsp, Some("0.9.4".into()))],
        Flow::File(Chip::Usb, _) => vec![(Chip::Usb, usb, Some("1.2.1".into()))],
    }
}

impl Demo {
    pub fn new(flow: Flow, status: &Status) -> Self {
        let mut progress = Progress::new(flow.clone());
        for (k, (chip, from, to)) in versions(&flow, status).into_iter().enumerate() {
            if let Some(v) = progress.versions.get_mut(k) {
                v.chip = chip;
                v.from = from;
                v.to = to;
            }
        }
        // A chip already up to date has nothing to write: no segment in the bar.
        let same: Vec<Chip> = progress.versions.iter().filter(|v| v.unchanged()).map(|v| v.chip).collect();
        for s in &mut progress.steps {
            if s.id.chip().is_some_and(|c| same.contains(&c)) {
                s.weight = 0;
                s.state = StepState::Skipped;
            }
        }
        Self { progress, i: 0, p: 0.0, last: 0.0, fail_at: None, hold_critical: false, frozen: false }
    }

    pub fn start(&mut self, now: f64) {
        self.progress.phase = Phase::Running;
        self.progress.error = None;
        self.last = now;
        self.sync();
    }

    pub fn resume(&mut self, now: f64) {
        self.progress.log.push("resume: pages already written are read back and skipped".into());
        self.start(now);
    }

    pub fn jump(&mut self, i: usize, percent: f32) {
        self.i = i;
        self.p = percent;
        self.progress.phase = Phase::Running;
        self.sync();
    }

    pub fn fail_now(&mut self) {
        self.progress.phase = Phase::Failed;
        self.progress.can_resume = true;
        self.progress.error = Some("HID timeout (link lost)".into());
        self.sync();
    }

    pub fn finish(&mut self) {
        self.i = self.progress.steps.len();
        self.progress.phase = Phase::Done;
        self.sync();
    }

    pub fn tick(&mut self, now: f64) -> bool {
        if self.progress.phase != Phase::Running || self.frozen {
            return false;
        }
        while now - self.last >= TICK_S {
            self.last += TICK_S;
            let Some(st) = self.progress.steps.get(self.i).copied() else { break };
            if st.state == StepState::Skipped || st.weight == 0 {
                self.p = 100.0;
            } else if st.id.critical() {
                self.p = (self.p + 9.0).min(100.0);
            } else {
                self.p = (self.p + 100.0 / (st.weight as f32 * 1.6 + 6.0)).min(100.0);
            }
            if st.id.critical() && self.p > 40.0 && self.hold_critical {
                self.p = self.p.min(45.0);
                self.sync();
                return true;
            }
            if self.fail_at == Some(st.id) && self.p > 42.0 {
                self.fail_at = None;
                self.fail_now();
                return false;
            }
            if self.p >= 100.0 {
                self.i += 1;
                self.p = 0.0;
                if self.i >= self.progress.steps.len() {
                    self.finish();
                    return false;
                }
            }
        }
        self.sync();
        true
    }

    fn sync(&mut self) {
        let failed = self.progress.phase == Phase::Failed;
        let done = self.progress.phase == Phase::Done;
        for (k, s) in self.progress.steps.iter_mut().enumerate() {
            if s.weight == 0 {
                s.state = StepState::Skipped;
                continue;
            }
            (s.state, s.percent) = if done || k < self.i {
                (StepState::Done, 100)
            } else if k == self.i && self.progress.phase != Phase::Confirm {
                (if failed { StepState::Error } else { StepState::Running }, self.p.round() as u8)
            } else {
                (StepState::Pending, 0)
            };
        }
        self.progress.current = (!done && self.progress.phase != Phase::Confirm).then_some(self.i);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{Device, SimDevice};

    #[test]
    fn walks_fails_resumes_and_finishes() {
        let status = SimDevice::new().status();
        let mut d = Demo::new(Flow::First, &status);
        d.fail_at = Some(StepId::DspWrite);
        d.start(0.0);
        let mut now = 0.0;
        while d.tick(now) {
            now += 0.12;
        }
        assert_eq!(d.progress.phase, Phase::Failed);
        assert_eq!(d.progress.current_step().map(|s| s.id), Some(StepId::DspWrite));
        d.resume(now);
        while d.tick(now) {
            now += 0.12;
        }
        assert_eq!((d.progress.phase, d.progress.percent()), (Phase::Done, 100));
    }

    #[test]
    fn an_unchanged_chip_has_no_segment() {
        let status = SimDevice::new().status();
        let d = Demo::new(Flow::Update, &status);
        assert!(d.progress.steps.iter().filter(|s| s.id.chip() == Some(Chip::Usb)).all(|s| s.weight == 0));
    }
}
