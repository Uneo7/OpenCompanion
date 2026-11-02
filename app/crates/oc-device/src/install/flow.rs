//! Every label is an i18n key of `oc-app/i18n/{fr,en}.json` (`inst.*`, `fw.*`).

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Flow {
    First,
    Update,
    Restore,
    File(Chip, PathBuf),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Chip {
    /// DSP (U7001 boot EEPROM).
    Dsp,
    /// TAS1020B USB controller (U6002 EEPROM).
    Usb,
}

impl Chip {
    pub fn key(self) -> &'static str {
        match self {
            Chip::Dsp => "inst.chip.dsp",
            Chip::Usb => "inst.chip.usb",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum StepId {
    /// Production builds, first install and update only.
    Download,
    Backup,
    Prep,
    PrepUpdate,
    FileCheck,
    RestoreCheck,
    DspBoot,
    DspCritical,
    DspWrite,
    DspVerify,
    UsbWrite,
    UsbVerify,
    RestoreDsp,
    RestoreUsb,
}

impl StepId {
    pub fn id(self) -> &'static str {
        match self {
            StepId::Download => "download",
            StepId::Backup => "backup",
            StepId::Prep => "prep",
            StepId::PrepUpdate => "prep.update",
            StepId::FileCheck => "file.check",
            StepId::RestoreCheck => "restore.check",
            StepId::DspBoot => "dsp.boot",
            StepId::DspCritical => "dsp.critical",
            StepId::DspWrite => "dsp.write",
            StepId::DspVerify => "dsp.verify",
            StepId::UsbWrite => "usb.write",
            StepId::UsbVerify => "usb.verify",
            StepId::RestoreDsp => "restore.dsp",
            StepId::RestoreUsb => "restore.usb",
        }
    }

    pub fn from_ui(s: oc_install::UiStep) -> Self {
        use oc_install::UiStep as U;
        match s {
            // The automatic backup belongs to the preparation group.
            U::Backup => StepId::Backup,
            U::RestoreCheck => StepId::RestoreCheck,
            U::RestoreDsp => StepId::RestoreDsp,
            U::DspCritical => StepId::DspCritical,
            U::DspVerify => StepId::DspVerify,
            U::RestoreUsb => StepId::RestoreUsb,
            U::UsbVerify => StepId::UsbVerify,
        }
    }

    pub fn key(self) -> &'static str {
        match self {
            StepId::Download => "inst.step.download",
            StepId::Backup => "inst.step.backup",
            StepId::Prep => "inst.step.prep",
            StepId::PrepUpdate => "inst.step.prep.update",
            StepId::FileCheck => "inst.step.file.check",
            StepId::RestoreCheck => "inst.step.restore.check",
            StepId::DspBoot => "inst.step.dsp.boot",
            StepId::DspCritical => "inst.step.dsp.critical",
            StepId::DspWrite => "inst.step.dsp.write",
            StepId::DspVerify => "inst.step.dsp.verify",
            StepId::UsbWrite => "inst.step.usb.write",
            StepId::UsbVerify => "inst.step.usb.verify",
            StepId::RestoreDsp => "inst.step.restore.dsp",
            StepId::RestoreUsb => "inst.step.restore.usb",
        }
    }

    /// `None` = the preparation group.
    pub fn chip(self) -> Option<Chip> {
        match self {
            StepId::DspBoot | StepId::DspCritical | StepId::DspWrite | StepId::DspVerify | StepId::RestoreDsp => Some(Chip::Dsp),
            StepId::UsbWrite | StepId::UsbVerify | StepId::RestoreUsb => Some(Chip::Usb),
            _ => None,
        }
    }

    /// Shows the "do not unplug" warning.
    pub fn writes(self) -> bool {
        self.chip().is_some()
    }

    /// The irrecoverable window (boot0, ~17 ms).
    pub fn critical(self) -> bool {
        self == StepId::DspCritical
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StepDef {
    pub id: StepId,
    pub weight: u32,
}

const fn s(id: StepId, weight: u32) -> StepDef {
    StepDef { id, weight }
}

/// The first install and the update start with [`StepId::Download`] (set when GitHub releases are used).
static DOWNLOAD_STEP: AtomicBool = AtomicBool::new(false);

pub(crate) fn set_download_step(on: bool) {
    DOWNLOAD_STEP.store(on, Ordering::Relaxed);
}

impl Flow {
    pub fn steps(&self) -> Vec<StepDef> {
        let mut steps = self.base_steps();
        if matches!(self, Flow::First | Flow::Update) && DOWNLOAD_STEP.load(Ordering::Relaxed) {
            steps.insert(0, s(StepId::Download, 10));
        }
        steps
    }

    fn base_steps(&self) -> Vec<StepDef> {
        use StepId::*;
        match self {
            Flow::First => vec![s(Backup, 12), s(Prep, 8), s(DspBoot, 14), s(DspCritical, 4), s(DspWrite, 30), s(DspVerify, 8), s(UsbWrite, 18), s(UsbVerify, 8)],
            Flow::Update => vec![s(PrepUpdate, 12), s(DspWrite, 50), s(DspVerify, 14), s(UsbWrite, 18), s(UsbVerify, 6)],
            // DSP order of `oc_install::restore::dsp`: stock image under our boot chain, then boot0, then boot1.
            Flow::Restore => vec![s(RestoreCheck, 8), s(RestoreDsp, 40), s(DspCritical, 4), s(DspVerify, 12), s(RestoreUsb, 24)],
            Flow::File(Chip::Dsp, _) => vec![s(FileCheck, 8), s(DspCritical, 4), s(DspWrite, 40), s(DspVerify, 8)],
            Flow::File(Chip::Usb, _) => vec![s(FileCheck, 8), s(UsbWrite, 24), s(UsbVerify, 8)],
        }
    }

    pub fn chips(&self) -> Vec<Chip> {
        match self {
            Flow::File(c, _) => vec![*c],
            _ => vec![Chip::Dsp, Chip::Usb],
        }
    }

    pub fn title_key(&self) -> &'static str {
        match self {
            Flow::First => "inst.title.first",
            Flow::Update => "inst.title.update",
            Flow::Restore => "inst.title.restore",
            Flow::File(..) => "inst.title.file",
        }
    }

    pub fn start_key(&self) -> &'static str {
        match self {
            Flow::First => "inst.start.first",
            Flow::Update => "inst.start.update",
            Flow::Restore => "fw.restore.ok",
            Flow::File(..) => "fw.install.ok",
        }
    }

    pub fn done_key(&self) -> &'static str {
        match self {
            Flow::First | Flow::Update => "inst.done.title",
            Flow::Restore => "inst.done.restore",
            Flow::File(..) => "fw.done",
        }
    }

    pub fn prep_key(&self) -> &'static str {
        // The backup (first install) has its own group: the preparation is the first step after it.
        self.base_steps().iter().map(|s| s.id).find(|id| *id != StepId::Backup).map_or("", StepId::key)
    }

    pub fn minutes(&self) -> u32 {
        match self {
            Flow::File(Chip::Dsp, _) => 2,
            Flow::File(Chip::Usb, _) => 1,
            _ => 3,
        }
    }

    /// Index of the step that stands for `wanted` in this flow: the step itself, or the closest one (e.g. a
    /// boot1 write during an update is shown as the firmware write).
    pub fn resolve(&self, wanted: StepId) -> Option<usize> {
        let steps = self.steps();
        let find = |id: StepId| steps.iter().position(|s| s.id == id);
        find(wanted).or_else(|| match wanted {
            StepId::DspBoot | StepId::DspCritical => find(StepId::DspWrite).or_else(|| find(StepId::RestoreDsp)),
            StepId::DspWrite => find(StepId::RestoreDsp),
            StepId::UsbWrite | StepId::UsbVerify => find(StepId::RestoreUsb),
            StepId::Backup | StepId::Prep | StepId::PrepUpdate | StepId::FileCheck | StepId::RestoreCheck => {
                Some(steps.iter().position(|s| s.id != StepId::Download).unwrap_or(0))
            }
            _ => None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../oc-app/i18n/en.json"));
    const FR: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../oc-app/i18n/fr.json"));

    fn flows() -> Vec<Flow> {
        vec![Flow::First, Flow::Update, Flow::Restore, Flow::File(Chip::Dsp, "x.out".into()), Flow::File(Chip::Usb, "x.dfu".into())]
    }

    #[test]
    fn every_key_exists_in_both_languages() {
        for lang in [EN, FR] {
            let has = |k: &str| assert!(lang.contains(&format!("\"{k}\"")), "missing i18n key {k}");
            for f in flows() {
                [f.title_key(), f.start_key(), f.done_key(), f.prep_key()].into_iter().for_each(has);
                for st in f.steps() {
                    has(st.id.key());
                    assert_eq!(st.id.key(), format!("inst.step.{}", st.id.id()));
                }
                for c in f.chips() {
                    has(c.key());
                }
                has(StepId::Download.key());
            }
        }
    }

    #[test]
    fn resolution_falls_back_to_the_closest_step() {
        let u = Flow::Update;
        assert_eq!(u.resolve(StepId::DspBoot), Some(1));
        assert_eq!(u.resolve(StepId::Prep), Some(0));
        assert_eq!(Flow::First.resolve(StepId::DspCritical), Some(3));
        assert_eq!(Flow::First.resolve(StepId::Backup), Some(0));
        assert_eq!(Flow::First.prep_key(), "inst.step.prep");
        assert_eq!(Flow::File(Chip::Usb, "x".into()).resolve(StepId::DspWrite), None);
        assert!(Flow::First.steps().iter().filter(|s| s.id.critical()).count() == 1);
    }
}
