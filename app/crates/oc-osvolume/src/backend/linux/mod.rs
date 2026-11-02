//! `snd-usb-audio` exposes the UAC2 Feature Unit as a simple element (usually "PCM"). PulseAudio / PipeWire
//! bind their sink volume to this same element, so writing it moves the desktop slider without fighting; a
//! sink volume above 100 % (software gain) or a software-only path is invisible here.

mod cards;
mod mixer;
mod watch;

use std::sync::Mutex;

use crate::{Callback, DeviceInfo, Error, Result, VolumeState};

pub use cards::list_devices;
pub use watch::Subscription;

use mixer::Control;

pub struct Device {
    control: Mutex<Control>,
}

impl Device {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        Ok(Self {
            control: Mutex::new(Control::open(&info.id)?),
        })
    }

    fn with<T>(&self, f: impl FnOnce(&Control) -> Result<T>) -> Result<T> {
        let control = self
            .control
            .lock()
            .map_err(|_| Error::os("alsa", "mixer lock poisoned"))?;
        f(&control)
    }

    pub fn get(&self) -> Result<VolumeState> {
        self.with(Control::read)
    }

    pub fn set_scalar(&self, scalar: f32) -> Result<()> {
        self.with(|c| c.set_scalar(scalar))
    }

    pub fn set_mute(&self, muted: bool) -> Result<()> {
        self.with(|c| c.set_mute(muted))
    }

    pub fn subscribe(&self, cb: Callback) -> Result<Subscription> {
        let ctl = self.with(|c| Ok(c.ctl_name().to_owned()))?;
        Subscription::spawn(ctl, cb)
    }
}
