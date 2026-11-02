pub use oc_device::console::Meters;
use oc_device::install::InstallController;
use oc_device::params::Profile as Params;
pub use oc_device::link::{DeviceMode, StreamFormat};

use super::eq::ProfileBands;
use super::profile::Profile;
use super::settings::DeviceSettings;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkState {
    Connected,
    /// The speaker re-enumerates (e.g. after a USB format change), or is not usable right now (see
    /// [`Device::connection`]).
    Reconnecting,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Connection {
    Simulated,
    Ready,
    Absent,
    /// The stock Bose firmware runs on the USB controller: offer the install ([`Device::installer`]).
    Stock,
    /// Our USB controller firmware, but the DSP does not run OpenCompanion (rescue, updater, interrupted
    /// install): offer the update. The text says what the DSP runs.
    DspNotReady(String),
    /// The USB controller is in ROM DFU mode: recovery needed (`opencompanion/docs/recovery.md`).
    Recovery,
    /// Found but the HID interface cannot be opened (Linux: udev rule, see `oc-protocol`).
    Inaccessible(String),
    Installing,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Status {
    pub link: LinkState,
    /// The audio stream the computer sends (USB alternate setting chosen by the OS; the app cannot set it).
    pub stream: StreamFormat,
    pub firmware_usb: String,
    pub firmware_dsp: String,
    /// The pod is plugged in (TAS `INFO` flags0 bit 6).
    pub pod_present: bool,
    /// The pod's bass knob reading is valid (TAS `INFO` flags1 bit 1).
    pub pod_knob_ok: bool,
    pub pod_bass: i8,
    pub headphones: bool,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Telemetry {
    /// DSP load (%).
    pub dsp_load: f32,
    /// Supply voltage (V).
    pub supply: f32,
    pub thermal_trip: bool,
    /// Clipping events of amplifiers A and B since the last reset.
    pub clips: (u32, u32),
    pub faults: u32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FirmwarePart {
    Usb,
    Dsp,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct UpdateProgress {
    pub step: &'static str,
    /// 0–100.
    pub percent: u8,
}

pub trait Device {
    /// Advances time-based state (telemetry, reconnection, updates). `now` is in seconds.
    fn tick(&mut self, now: f64);

    fn status(&self) -> Status;
    fn telemetry(&self) -> Telemetry;
    fn reset_clips(&mut self);

    /// Master volume (0–100) and mute, mirrored with the computer's volume.
    fn volume(&self) -> u8;
    /// The bass module volume set by its rear knob (dB), as the DSP reads it ([`super::SubSource::Knob`]).
    fn rear_knob_db(&self) -> Option<f32> {
        None
    }

    fn mode(&self) -> Option<DeviceMode> {
        None
    }

    /// A restore to the Bose firmware is unfinished (its DSP part is verified, the USB controller step remains).
    fn restore_pending(&self) -> bool {
        false
    }

    fn can_set_mode(&self) -> bool {
        false
    }

    /// Switches the mode: the speaker leaves the USB bus for a few seconds and comes back.
    fn set_mode(&mut self, _mode: DeviceMode) {}

    /// Target of a mode switch in progress (until the speaker is back in that mode, or a timeout).
    fn mode_switching(&self) -> Option<DeviceMode> {
        None
    }

    fn restart(&mut self, _part: FirmwarePart) {}

    fn restarting(&self) -> Option<FirmwarePart> {
        None
    }
    /// Level of the volume in dB, when the computer reports it (the OS mixer of the speaker).
    fn volume_db(&self) -> Option<f32> {
        None
    }
    fn set_volume(&mut self, v: u8);
    fn muted(&self) -> bool;
    fn set_muted(&mut self, m: bool);

    fn profiles(&self) -> &[Profile];
    fn active_profile(&self) -> usize;
    fn set_active_profile(&mut self, i: usize);
    fn store_profile(&mut self, i: usize, bands: &ProfileBands) -> bool;
    fn rename_profile(&mut self, i: usize, name: &str);
    fn set_profile_hue(&mut self, i: usize, hue: usize);
    /// Plays `bands` right away without storing them (`None` returns to the stored profile).
    fn preview(&mut self, bands: Option<&ProfileBands>);
    fn set_compare(&mut self, on: bool);
    fn set_bypass(&mut self, on: bool);

    fn profile_params(&self, i: usize) -> Params {
        params_of(self, i)
    }

    /// Slot `i` holds no saved profile (a copy can go there without replacing anything).
    fn slot_free(&self, _i: usize) -> bool {
        false
    }

    /// Copies profile `from` into slot `to` (never the locked one) as `name` / `hue`, saves it, and makes the copy
    /// the active profile (to edit it right away). Returns false when refused.
    fn duplicate(&mut self, from: usize, to: usize, name: &str, hue: usize) -> bool;

    /// Replaces the sound of profile `i` (the active one) with `p`, keeping its name and colour, and stores it.
    fn import_profile(&mut self, i: usize, p: &Params) -> bool {
        if i >= self.profiles().len() || i != self.active_profile() {
            return false;
        }
        let bands = super::hw::convert::bands_from(p, None);
        if !self.store_profile(i, &bands) {
            return false;
        }
        let s = super::hw::convert::settings_from(p, self.settings(), self.sub_level_in_profile());
        self.set_settings(s);
        true
    }

    fn settings(&self) -> &DeviceSettings;
    /// The "app" sub level is saved with each profile (DSP before protocol 8). From protocol 8 it is a device
    /// setting, the same on every profile: switching, importing or exporting a profile never changes it.
    fn sub_level_in_profile(&self) -> bool {
        false
    }
    /// The speaker stores the device options ([`DeviceSettings::options`]; DSP profile protocol 4). Without it
    /// the views hide those controls.
    fn has_device_options(&self) -> bool {
        true
    }
    fn set_settings(&mut self, s: DeviceSettings);

    fn console(&mut self, command: &str) -> String;

    fn begin_update(&mut self, part: FirmwarePart);
    fn update_progress(&self, part: FirmwarePart) -> Option<UpdateProgress>;
    fn last_backup(&self) -> String;
    fn backup(&mut self);
    fn restore_backup(&mut self);
    /// Reinstalls the Bose firmware on both chips (erases the four profiles).
    fn restore_factory(&mut self);

    fn connection(&self) -> Connection {
        Connection::Simulated
    }

    /// Level meters (refreshed at ~15 Hz on the speaker).
    fn meters(&self) -> Meters {
        Meters::default()
    }

    fn boot_status(&self) -> Option<String> {
        None
    }

    fn installer(&mut self) -> Option<&mut InstallController> {
        None
    }

    /// A newer app on GitHub (release builds; found by the update check).
    fn app_update(&mut self) -> Option<oc_device::install::AppUpdate> {
        self.installer().and_then(|i| i.app_update())
    }

    /// What the update check found (the installer's; nothing without one).
    fn updates(&mut self) -> oc_device::install::UpdateCheck {
        self.installer().map(|i| i.updates()).unwrap_or_default()
    }

    /// A message for the user (a command failed, a feature is not available yet). One per call.
    fn take_notice(&mut self) -> Option<String> {
        None
    }

    /// Console output that arrived after [`Device::console`] returned (the hardware answers asynchronously).
    fn take_console_output(&mut self) -> Option<String> {
        None
    }

    /// The active profile was (re)loaded from the speaker (load finished, pod double press): the editor's draft
    /// should be refreshed if it has no unsaved changes. Reading clears the flag.
    fn take_profile_reloaded(&mut self) -> bool {
        false
    }
}

pub fn params_of<D: Device + ?Sized>(d: &D, i: usize) -> Params {
    let prof = &d.profiles()[i];
    let mut p = Params::defaults();
    p.set_name(&prof.name);
    p.color = (prof.hue % super::HUE_CHOICES.len()) as u8;
    if i == d.active_profile() {
        super::hw::convert::apply_settings(&mut p, d.settings(), d.sub_level_in_profile());
    }
    super::hw::convert::apply_bands(&mut p, &prof.bands);
    p.sanitized()
}
