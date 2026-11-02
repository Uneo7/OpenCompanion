#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum UiStep {
    /// Has no step of its own: shown as the preparation (`inst.step.prep`).
    Backup,
    RestoreCheck,
    RestoreDsp,
    DspCritical,
    DspVerify,
    RestoreUsb,
    UsbVerify,
}

impl UiStep {
    pub fn id(self) -> &'static str {
        match self {
            Self::Backup => "prep",
            Self::RestoreCheck => "restore.check",
            Self::RestoreDsp => "restore.dsp",
            Self::DspCritical => "dsp.critical",
            Self::DspVerify => "dsp.verify",
            Self::RestoreUsb => "restore.usb",
            Self::UsbVerify => "usb.verify",
        }
    }

    pub fn key(self) -> String {
        format!("inst.step.{}", self.id())
    }

    /// The "do not unplug" warning applies.
    pub fn writes(self) -> bool {
        !matches!(self, Self::Backup | Self::RestoreCheck)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EN: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../oc-app/i18n/en.json"));
    const FR: &str = include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../oc-app/i18n/fr.json"));

    #[test]
    fn every_step_has_an_i18n_key() {
        use UiStep::*;
        for s in [Backup, RestoreCheck, RestoreDsp, DspCritical, DspVerify, RestoreUsb, UsbVerify] {
            for lang in [EN, FR] {
                assert!(lang.contains(&format!("\"{}\"", s.key())), "missing {}", s.key());
            }
        }
    }
}
