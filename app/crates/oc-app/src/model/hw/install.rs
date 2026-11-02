use oc_device::install::{Chip, Flow, Phase};

use super::HwDevice;
use crate::i18n;
use crate::model::device::{FirmwarePart, UpdateProgress};

fn chip(part: FirmwarePart) -> Chip {
    match part {
        FirmwarePart::Usb => Chip::Usb,
        FirmwarePart::Dsp => Chip::Dsp,
    }
}

impl HwDevice {
    pub(super) fn run_flow(&mut self, flow: Flow) {
        if self.installer.is_running() {
            self.notice(i18n::tr("inst.busy"));
            return;
        }
        if !(self.installer.open(flow) && self.installer.start()) {
            self.notice(i18n::tr("inst.start.failed"));
        }
    }

    pub(super) fn begin(&mut self, part: FirmwarePart) {
        let s = self.installer.sources();
        let path = match part {
            FirmwarePart::Usb => s.tas_image.clone(),
            FirmwarePart::Dsp => s.dsp_app.clone(),
        };
        self.run_flow(Flow::File(chip(part), path));
    }

    pub(super) fn progress_of(&self, part: FirmwarePart) -> Option<UpdateProgress> {
        self.installer.with_progress(|p| {
            if !matches!(p.phase, Phase::Running | Phase::AwaitingConfirmation) || !p.flow.chips().contains(&chip(part)) {
                return None;
            }
            let key = p.current_key().unwrap_or_else(|| p.flow.prep_key());
            Some(UpdateProgress { step: i18n::tr(key), percent: p.percent() })
        })
    }

    pub(super) fn watch_installer(&mut self) {
        if self.now - self.restore_pending.1 >= 3.0 && !self.installer.is_running() {
            self.restore_pending = (oc_device::install::restore_in_progress(), self.now);
        }
        self.installer.set_running(self.st.running_versions());
        // After an install, the versions are checked again once the speaker is back.
        if self.check_after_install && self.st.ready() && self.installer.check_updates() {
            self.check_after_install = false;
            self.last_check = self.now;
        }
        // Periodic check: a firmware staged (dev) or published (GitHub) while the app runs shows up by itself.
        let period = if cfg!(feature = "release-updates") { 6.0 * 3600.0 } else { 180.0 };
        if self.st.ready() && self.now - self.last_check >= period && !self.installer.is_running() && self.installer.check_updates() {
            self.last_check = self.now;
        }
        // After an install, once the speaker is back: the OS volume it had, then the mute.
        let ready = self.st.present();
        let lines = self.post_install.tick(self.now, ready, &mut self.os_volume);
        oc_device::install::log_file::append(&lines);
        let phase = self.installer.phase();
        if phase == self.inst_phase {
            return;
        }
        let was_running = matches!(self.inst_phase, Phase::Running | Phase::AwaitingConfirmation);
        let ended_from_run = was_running && !matches!(phase, Phase::Running | Phase::AwaitingConfirmation);
        if !was_running && phase == Phase::Running {
            // Before the first write (the flow starts with its preparation).
            let line = self.post_install.started(&self.os_volume);
            oc_device::install::log_file::append(&[line]);
            // A restore ends with the stock DSP at full volume (no stock handshake until a mains power cycle):
            // the speaker is muted before anything is written, and stays muted afterwards (post_install).
            if self.installer.with_progress(|p| p.flow == oc_device::install::Flow::Restore) {
                let line = match self.os_volume.set_muted(true) {
                    Ok(()) => "before the restore: OS mute on".to_owned(),
                    Err(e) => format!("before the restore: setting the OS mute failed: {e}"),
                };
                oc_device::install::log_file::append(&[line]);
            }
        }
        self.inst_phase = phase;
        if !ended_from_run {
            return;
        }
        // A flow that completed: the endpoint is looked for again (the speaker may have re-enumerated).
        if phase == Phase::Done {
            self.post_install.ended(self.now);
            self.os_volume.reset();
        } else {
            self.post_install = super::post_install::PostInstall::Idle;
        }
        self.check_after_install = true;
        // A power cycle is asked by its own page (views::install), not by a notice.
        if phase == Phase::Failed {
            let error = self.installer.with_progress(|p| p.error.clone());
            self.notice(format!("{} · {}", i18n::tr("inst.err.title"), error.unwrap_or_default()));
        }
    }

    pub(super) fn backup_date(&self) -> String {
        self.installer.last_backup().unwrap_or_else(|| "-".into())
    }
}
