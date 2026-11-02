use oc_device::install::InstallController;
use oc_device::worker::Command;

use super::os_volume;
use super::volume::{percent_from_db, Pending};
use super::{convert, HwDevice, HOST_ONLY, SETTINGS_SAVE_DELAY_S};

const PROTO_OPTIONS: u8 = 4;
use oc_device::console::PROTO_SUB_DEVICE;
use crate::model::device::{Connection, Device, DeviceMode, FirmwarePart, Meters, Status, Telemetry, UpdateProgress};
use crate::model::eq::ProfileBands;
use crate::model::profile::Profile;
use crate::model::settings::{DeviceSettings, SubSource};

impl HwDevice {
    fn os_result(&mut self, res: Result<(), String>) {
        match res {
            Ok(()) => {}
            Err(e) if e.is_empty() => self.notice(crate::i18n::tr("home.volume.unavailable")),
            Err(e) => self.notice(e),
        }
    }
}

impl HwDevice {
    /// Sends the OS volume the throttle has due (see [`oc_device::throttle`]).
    fn flush_os_volume(&mut self) {
        for ((), v) in self.os_volume_out.take_due(std::time::Instant::now(), false) {
            let now = std::time::Instant::now();
            match self.os_volume.set_volume(v) {
                Ok(()) => self.os_volume_out.sent(&(), now),
                Err(e) => {
                    if self.os_volume_out.failed(&(), v, now) {
                        self.os_result(Err(e));
                    }
                }
            }
        }
    }
}

impl Device for HwDevice {
    fn tick(&mut self, now: f64) {
        self.now = now;
        self.pump();
        self.os_volume.tick(now, self.st.present());
        self.flush_os_volume();
        self.tick_settings_save();
        self.watch_installer();
        self.watch_mode_switch();
        self.watch_restart();
    }

    fn status(&self) -> Status {
        self.st.status()
    }

    fn telemetry(&self) -> Telemetry {
        self.st.telemetry()
    }

    fn reset_clips(&mut self) {
        self.st.reset_clips();
    }

    /// The OS slider position of the speaker's endpoint, else the volume the speaker reports.
    fn volume(&self) -> u8 {
        let device = match self.os_volume.state() {
            Some(s) => os_volume::percent(&s),
            None => self.st.usb.map_or(0, |u| percent_from_db(u.volume_db())),
        };
        Pending::shown(self.volume, device, self.now)
    }

    fn volume_db(&self) -> Option<f32> {
        self.os_volume.state().map(|s| s.db)
    }

    fn set_volume(&mut self, v: u8) {
        let v = v.min(100);
        // Paced like every setting (20 Hz while dragging, the last value always sent): `flush_os_volume`.
        self.os_volume_out.request((), oc_device::throttle::Pace::Live(oc_device::throttle::LIVE), v, std::time::Instant::now());
        self.flush_os_volume();
        self.volume = Some(Pending::new(v, self.now));
    }

    fn muted(&self) -> bool {
        let device = match self.os_volume.state() {
            Some(s) => s.muted,
            None => self.st.usb.is_some_and(|u| u.uac_mute),
        };
        Pending::shown(self.muted, device, self.now)
    }

    fn set_muted(&mut self, m: bool) {
        let res = self.os_volume.set_muted(m);
        self.os_result(res);
        self.muted = Some(Pending::new(m, self.now));
    }

    fn profile_params(&self, i: usize) -> oc_device::params::Profile {
        self.params_of(i).unwrap_or_else(|| crate::model::device::params_of(self, i))
    }

    fn slot_free(&self, i: usize) -> bool {
        super::profiles::slot_empty(&self.st, i)
    }

    fn duplicate(&mut self, from: usize, to: usize, name: &str, hue: usize) -> bool {
        self.duplicate_slot(from, to, name, hue)
    }

    fn import_profile(&mut self, i: usize, p: &oc_device::params::Profile) -> bool {
        self.import(i, p)
    }

    fn profiles(&self) -> &[Profile] {
        &self.profiles
    }

    fn active_profile(&self) -> usize {
        self.ui_active
    }

    fn set_active_profile(&mut self, i: usize) {
        self.activate(i);
    }

    fn store_profile(&mut self, i: usize, bands: &ProfileBands) -> bool {
        self.store(i, bands)
    }

    fn rename_profile(&mut self, i: usize, name: &str) {
        self.rename(i, name);
    }

    fn set_profile_hue(&mut self, i: usize, hue: usize) {
        self.set_hue(i, hue);
    }

    fn preview(&mut self, bands: Option<&ProfileBands>) {
        HwDevice::preview(self, bands);
    }

    fn set_compare(&mut self, on: bool) {
        self.compare(on);
    }

    fn set_bypass(&mut self, on: bool) {
        self.bypass(on);
    }

    fn settings(&self) -> &DeviceSettings {
        &self.settings
    }

    fn rear_knob_db(&self) -> Option<f32> {
        // DSP protocol 5: the exact knob-mode level (1/100 dB); older firmware: the knob position on ±12 dB.
        let proto = self.st.info.map_or(0, |i| i.proto);
        self.st.bass.map(|b| b.knob_db(proto))
    }

    fn mode(&self) -> Option<DeviceMode> {
        self.st.mode
    }

    fn mode_switching(&self) -> Option<DeviceMode> {
        self.mode_switch.map(|(m, _)| m)
    }

    fn restore_pending(&self) -> bool {
        self.restore_pending.0
    }

    fn can_set_mode(&self) -> bool {
        self.st.usb.is_some_and(|u| oc_device::link::mode::supported(&u))
    }

    fn set_mode(&mut self, mode: DeviceMode) {
        if !self.can_set_mode() {
            self.notice(crate::i18n::tr("settings.device.mode.unsupported"));
            return;
        }
        if self.st.mode != Some(mode) {
            self.mode_switch = Some((mode, self.now));
            self.send(Command::SetMode(mode));
            // The speaker re-enumerates: its audio endpoint changes, look for it again once it is back.
            self.os_volume.reset();
        }
    }

    fn restart(&mut self, part: FirmwarePart) {
        if self.restart.is_some() || self.mode_switch.is_some() || !self.st.ready() {
            return;
        }
        let target = match part {
            FirmwarePart::Dsp => oc_device::worker::RestartTarget::Dsp,
            FirmwarePart::Usb => oc_device::worker::RestartTarget::Usb,
        };
        self.restart = Some((part, self.now, false));
        self.send(Command::Restart(target));
        if part == FirmwarePart::Usb {
            // The speaker re-enumerates: its audio endpoint changes, look for it again once it is back.
            self.os_volume.reset();
        }
    }

    fn restarting(&self) -> Option<FirmwarePart> {
        self.restart.map(|(p, ..)| p)
    }

    fn sub_level_in_profile(&self) -> bool {
        // Unknown protocol (not read yet): the profile path, as before.
        self.st.info.is_none_or(|i| i.proto < PROTO_SUB_DEVICE)
    }

    fn has_device_options(&self) -> bool {
        self.st.info.is_some_and(|i| i.proto >= PROTO_OPTIONS)
    }

    fn set_settings(&mut self, s: DeviceSettings) {
        // The locked profile ("Bose® d'origine") is never written: its fields are refused here, not by the worker.
        let locked = self.profiles.get(self.ui_active).is_some_and(|p| p.locked);
        let s = crate::model::settings::guard_locked(&self.settings, s, locked);
        // "Routage 2.1" follows the device mode on the DSP side: no OPU write any more.
        if s.sub_source != self.settings.sub_source {
            let src = match s.sub_source {
                SubSource::Knob => oc_device::console::BassSource::Knob,
                SubSource::App => oc_device::console::BassSource::Software,
            };
            // The DSP keeps the sub level source in the former bass source field of its config record.
            self.send(Command::SetBassSource(src));
            if let Some(b) = self.st.bass.as_mut() {
                b.source = src; // shown at once; the DSP's reply confirms it
            }
        }
        if s.options != self.settings.options {
            if self.has_device_options() {
                self.send(Command::SetOptions(s.options));
                if let Some(r) = self.st.route21.as_mut() {
                    r.options = Some(s.options); // shown at once; the DSP's reply confirms it
                }
            } else {
                self.notice(HOST_ONLY);
            }
        }
        // DSP protocol 8: the sub level is a device setting (`DEVL`, saved by the DSP), on every profile.
        let sub = self.sub_level_in_profile();
        if !sub && s.sub_level != self.settings.sub_level {
            let db10 = (s.sub_level * 10.0).round().clamp(-120.0, 120.0) as i16;
            self.send(Command::SetSubLevel(db10));
            if let Some(b) = self.st.bass.as_mut() {
                b.soft = db10 * 10; // shown at once; the DSP's reply confirms it
            }
        }
        let write = crate::model::settings::profile_write(&self.settings, &s, locked, sub);
        if write != crate::model::settings::ProfileWrite::None {
            if let (Some(stored), Some(live)) = (self.stored.as_mut(), self.live.as_mut()) {
                convert::apply_settings(live, &s, sub);
                if write == crate::model::settings::ProfileWrite::Save {
                    convert::apply_settings(stored, &s, sub);
                    self.save_settings_at = Some(self.now + SETTINGS_SAVE_DELAY_S);
                }
                if !self.compare {
                    self.play();
                }
            }
        }
        self.settings = s;
        self.rebuild();
    }

    fn console(&mut self, command: &str) -> String {
        self.console_id += 1;
        self.send(Command::Console { id: self.console_id, line: command.to_owned() });
        String::new()
    }

    fn begin_update(&mut self, part: FirmwarePart) {
        self.begin(part);
    }

    fn update_progress(&self, part: FirmwarePart) -> Option<UpdateProgress> {
        self.progress_of(part)
    }

    fn last_backup(&self) -> String {
        self.backup_date()
    }

    fn backup(&mut self) {
        // TODO(oc-install): a standalone backup needs the DSP updater to read U7001; today the backup is taken by
        // the first install (`oc_device::install`, automatically, during its preparation step).
        self.notice(crate::i18n::tr("fw.backup.host_only"));
    }

    fn restore_backup(&mut self) {
        // TODO(oc-install): no flow writes a backup back yet.
        self.notice(crate::i18n::tr("fw.restore.unavailable"));
    }

    fn restore_factory(&mut self) {
        self.run_flow(oc_device::install::Flow::Restore);
    }

    fn connection(&self) -> Connection {
        // The speaker leaves the USB bus during a mode switch or a restart: that absence is part of it.
        if self.mode_switch.is_some() || self.restart.is_some() {
            return Connection::Ready;
        }
        self.st.connection(self.installer_phase())
    }

    fn meters(&self) -> Meters {
        self.st.meters
    }

    fn boot_status(&self) -> Option<String> {
        self.st.boot_line()
    }

    fn installer(&mut self) -> Option<&mut InstallController> {
        Some(&mut self.installer)
    }

    fn take_notice(&mut self) -> Option<String> {
        self.notices.pop_front()
    }

    fn take_console_output(&mut self) -> Option<String> {
        self.console.pop_front()
    }

    fn take_profile_reloaded(&mut self) -> bool {
        std::mem::take(&mut self.reloaded)
    }
}

impl HwDevice {
    /// Ends a mode switch once the speaker is back and reports the target mode; reports a speaker that does not
    /// come back within [`MODE_SWITCH_TIMEOUT_S`].
    fn watch_mode_switch(&mut self) {
        let Some((target, since)) = self.mode_switch else { return };
        if self.st.present() && self.st.mode == Some(target) {
            self.mode_switch = None;
        } else if self.now - since > MODE_SWITCH_TIMEOUT_S {
            self.mode_switch = None;
            self.notice(crate::i18n::tr("settings.device.mode.timeout"));
        }
    }
}

impl HwDevice {
    /// Ends a USB controller restart once the speaker is back and ready; reports a restart that does not end
    /// within [`RESTART_TIMEOUT_S`] (the DSP's end comes from the worker, [`Self::end_restart`]).
    fn watch_restart(&mut self) {
        let Some((part, since, gone)) = self.restart else { return };
        if part == FirmwarePart::Usb && gone && self.st.ready() {
            self.end_restart(true);
        } else if self.now - since > RESTART_TIMEOUT_S {
            self.end_restart(false);
        }
    }

    pub(super) fn end_restart(&mut self, ok: bool) {
        let Some((part, ..)) = self.restart.take() else { return };
        let key = match (ok, part) {
            (true, FirmwarePart::Dsp) => "settings.advanced.restart.done_dsp",
            (true, FirmwarePart::Usb) => "settings.advanced.restart.done_usb",
            (false, _) => "settings.advanced.restart.timeout",
        };
        self.notice(crate::i18n::tr(key));
    }
}

/// How long a restart may take before it is reported as failed (the DSP's own wait is 15 s).
const RESTART_TIMEOUT_S: f64 = 25.0;

/// How long a mode switch may take before it is reported as failed (the TAS re-enumerates in a few seconds).
const MODE_SWITCH_TIMEOUT_S: f64 = 15.0;
