//! The UI thread never touches USB: commands go to the worker through a channel, and [`super::Device::tick`]
//! drains the worker's events into a local copy of the speaker state that the getters read.
//!
//! The DSP keeps one active profile in RAM (what plays) and 4 EEPROM slots. The device side keeps two copies of
//! the active profile: `stored` (as loaded / last saved) and `live` (what plays: `stored` + the unsaved EQ draft
//! being previewed + bypass). Saving writes `stored` to its slot, then plays `live` again.

pub(crate) mod convert;
mod device_impl;
mod install;
mod profiles;
mod state;
mod os_volume;
mod post_install;
pub mod volume;

use std::collections::VecDeque;

use oc_device::install::{FirmwareSources, InstallController, Phase};
use oc_device::link::DeviceMode;
use oc_device::params::Profile as Params;
use oc_device::worker::{Command, Event, Rates, Worker};

use super::device::FirmwarePart;
use super::eq::ProfileBands;
use super::profile::{Profile, PROFILE_COUNT};
use super::settings::DeviceSettings;
use state::{Changed, HwState};
use os_volume::OsLink;
use volume::Pending;

// Device options (standby, wake threshold, double press, headphone EQ) need DSP profile protocol 4 (`OPC`);
// with an older DSP the views hide them.
const HOST_ONLY: &str = "not stored in the speaker yet";

/// Delay between the last settings change and its save to the profile slot (EEPROM wear: sliders send one
/// change per frame).
const SETTINGS_SAVE_DELAY_S: f64 = 1.5;

pub struct HwDevice {
    worker: Worker,
    installer: InstallController,
    st: HwState,
    stored: Option<Params>,
    live: Option<Params>,
    compare: bool,
    /// Slot the UI treats as active (the speaker may have none: flat defaults at boot).
    ui_active: usize,
    /// Slot of a load the app asked for (an empty slot loads the defaults and keeps the DSP's active slot).
    requested: Option<usize>,
    profiles: Vec<Profile>,
    /// Bands last known per slot (only the active slot can be read from the speaker).
    bands: [Option<ProfileBands>; PROFILE_COUNT],
    settings: DeviceSettings,
    save_settings_at: Option<f64>,
    os_volume: OsLink,
    /// OS volume changes, paced ([`oc_device::throttle`]).
    os_volume_out: oc_device::throttle::Throttle<(), u8>,
    volume: Option<Pending<u8>>,
    muted: Option<Pending<bool>>,
    notices: VecDeque<String>,
    /// Last notice and when, so that a slider moving every frame does not repeat it.
    last_notice: Option<(String, f64)>,
    console: VecDeque<String>,
    console_id: u64,
    reloaded: bool,
    /// The empty slots were filled with the default profiles on this connection ([`Command::Provision`]).
    provisioned: bool,
    check_after_install: bool,
    /// When the last periodic update check started (s).
    last_check: f64,
    /// [`oc_device::install::restore_in_progress`], refreshed every few seconds: `(pending, checked at)`.
    restore_pending: (bool, f64),
    post_install: post_install::PostInstall,
    inst_phase: Phase,
    mode_switch: Option<(DeviceMode, f64)>,
    restart: Option<(FirmwarePart, f64, bool)>,
    now: f64,
}

impl HwDevice {
    /// Starts the device thread. `repaint` is called from the installer thread on progress and from the OS
    /// volume notifications (request a frame).
    pub fn new(sources: FirmwareSources, repaint: impl Fn() + Send + Sync + 'static) -> Self {
        let repaint: std::sync::Arc<dyn Fn() + Send + Sync> = std::sync::Arc::new(repaint);
        let worker = Worker::spawn(Rates::default());
        let installer = InstallController::new(sources, worker.lease());
        #[cfg(feature = "release-updates")]
        {
            use oc_device::install::release::{API_BASE, REPO};
            installer.enable_releases(API_BASE, REPO);
        }
        let notify = repaint.clone();
        installer.set_notifier(move |_| notify());
        let mut d = Self {
            worker,
            installer,
            st: HwState::default(),
            stored: None,
            live: None,
            compare: false,
            ui_active: 0,
            requested: None,
            profiles: Vec::new(),
            bands: Default::default(),
            settings: DeviceSettings::default(),
            save_settings_at: None,
            os_volume: OsLink::new(repaint),
            os_volume_out: oc_device::throttle::Throttle::default(),
            volume: None,
            muted: None,
            notices: VecDeque::new(),
            last_notice: None,
            console: VecDeque::new(),
            console_id: 0,
            reloaded: false,
            provisioned: false,
            check_after_install: false,
            last_check: 0.0,
            restore_pending: (false, f64::NEG_INFINITY),
            post_install: post_install::PostInstall::default(),
            inst_phase: Phase::Confirm,
            mode_switch: None,
            restart: None,
            now: 0.0,
        };
        d.rebuild();
        d
    }

    fn send(&self, c: Command) {
        self.worker.send(c);
    }

    fn notice(&mut self, msg: impl Into<String>) {
        const REPEAT_S: f64 = 5.0;
        let msg = msg.into();
        if self.last_notice.as_ref().is_some_and(|(m, t)| *m == msg && self.now - t < REPEAT_S) {
            return;
        }
        self.last_notice = Some((msg.clone(), self.now));
        self.notices.push_back(msg);
    }

    fn installer_phase(&self) -> Phase {
        self.installer.phase()
    }

    fn pump(&mut self) {
        let events: Vec<Event> = self.worker.events().collect();
        let mut dirty = false;
        for e in events {
            match self.st.apply(e) {
                Ok(Changed::Nothing) => {}
                Ok(Changed::Link) => {
                    // The restarted USB controller left the bus (seen here: it may be back within one batch).
                    if let Some((FirmwarePart::Usb, _, gone)) = self.restart.as_mut() {
                        *gone |= !self.st.present();
                    }
                    if !self.st.ready() {
                        self.stored = None;
                        self.live = None;
                        self.compare = false;
                        self.provisioned = false;
                    }
                    dirty = true;
                }
                Ok(_) => dirty = true,
                Err(Event::Profile(p)) => {
                    self.on_profile(*p);
                    dirty = true;
                }
                Err(Event::Console { reply, .. }) => self.console.push_back(reply),
                Err(Event::Provisioned(slots)) if !slots.is_empty() => {
                    let list = slots.iter().map(|s| (s + 1).to_string()).collect::<Vec<_>>().join(", ");
                    self.console.push_back(format!("default profiles written to the empty slots {list}"));
                    self.notice(crate::i18n::t_with("profile.provisioned", &[("slots", &list)]));
                    dirty = true;
                }
                Err(Event::Restarted { ok, .. }) => self.end_restart(ok),
                Err(Event::Failed { what, error }) => self.notice(format!("{what}: {error}")),
                Err(_) => {}
            }
        }
        if dirty {
            self.rebuild();
        }
        self.provision();
    }

    /// A fresh install leaves the four slots empty (nothing for the pod's double press to cycle through): once
    /// per connection, the empty ones get the default profiles (Bose base, flat EQ, default names and colours).
    /// Saved slots are never touched (the worker checks `OPI` again before each write).
    fn provision(&mut self) {
        let Some(info) = self.st.info else { return };
        if self.provisioned || !self.st.ready() || self.st.slots.len() < PROFILE_COUNT {
            return;
        }
        self.provisioned = true;
        let empty: Vec<_> = (0..PROFILE_COUNT)
            .filter(|i| !info.slot_valid(*i))
            .map(|i| (i as u8, crate::model::profile::DEFAULT_NAMES[i].to_owned(), i as u8))
            .collect();
        if !empty.is_empty() {
            self.send(Command::Provision(empty));
        }
    }

    fn on_profile(&mut self, p: Params) {
        if self.compare {
            return; // the built-in Bose profile plays for the comparison: not the user's profile
        }
        if let Some(r) = self.requested.take() {
            self.ui_active = r;
        } else if let Some(a) = self.st.info.and_then(|i| i.active) {
            self.ui_active = usize::from(a).min(PROFILE_COUNT - 1);
        }
        let mut p = p;
        if profiles::slot_empty(&self.st, self.ui_active) {
            // An empty slot loads as the Bose base (`OPK`): it keeps its default name and colour until saved.
            p.set_name(crate::model::profile::DEFAULT_NAMES[self.ui_active]);
            p.color = self.ui_active as u8;
        }
        let hint = self.bands[self.ui_active].clone();
        self.bands[self.ui_active] = Some(convert::bands_from(&p, hint.as_ref()));
        self.stored = Some(p.clone());
        self.live = Some(p);
        self.save_settings_at = None;
        self.reloaded = true;
    }

    fn rebuild(&mut self) {
        self.profiles = profiles::list(&self.st, &self.bands);
        let sub = crate::model::Device::sub_level_in_profile(self);
        if let Some(live) = &self.live {
            self.settings = convert::settings_from(live, &self.settings, sub);
        }
        if let Some(o) = self.st.route21.and_then(|r| r.options) {
            self.settings.options = o;
        }
        if let Some(b) = self.st.bass {
            // DSP protocol 8: the device's own level (`DEV` `SL:`, OPQ), whatever the profile.
            if !sub {
                self.settings.sub_level = f32::from(b.soft) / 100.0;
            }
            self.settings.sub_source = match b.source {
                oc_device::console::BassSource::Knob => super::settings::SubSource::Knob,
                oc_device::console::BassSource::Software => super::settings::SubSource::App,
            };
        }
    }

    fn play(&mut self) {
        if let Some(l) = &self.live {
            self.send(Command::Write(l.clone()));
        }
    }

    fn save_stored(&mut self) {
        let (Some(stored), Some(live)) = (self.stored.clone(), self.live.clone()) else { return };
        self.send(Command::Save { slot: self.ui_active as u8, profile: stored.clone() });
        if live != stored {
            self.send(Command::Write(live));
        }
    }

    fn tick_settings_save(&mut self) {
        if self.save_settings_at.is_some_and(|t| self.now >= t) {
            self.save_settings_at = None;
            self.save_stored();
        }
    }
}

impl Drop for HwDevice {
    fn drop(&mut self) {
        // A settings change still waiting for its save is written now (the worker drains its queue first).
        if self.save_settings_at.take().is_some() {
            self.save_stored();
        }
    }
}
