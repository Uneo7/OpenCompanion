//! The OS volume of the speaker's own UAC2 device is the single master: the pod knob moves it through UAC2
//! interrupts and the app slider must move it too (never the default output device, which may be another one).

mod backend;
pub mod convert;
mod error;
pub mod matcher;

pub use error::{Error, Result};
pub use matcher::{DeviceInfo, DeviceMatch, UsbId};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct VolumeState {
    /// 0.0..=1.0, as shown by the OS volume slider.
    pub scalar: f32,
    /// Hardware Feature Unit level, in dB.
    pub db: f32,
    pub muted: bool,
}

/// Called on every change, whoever made it (app, OS mixer, pod knob). Runs on an OS-owned thread: keep it short.
pub type Callback = Box<dyn Fn(VolumeState) + Send + Sync + 'static>;

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    backend::list_devices()
}

/// USB ID first, then name.
pub fn find_device(m: &DeviceMatch) -> Result<Option<OsVolume>> {
    let devices = backend::list_devices()?;
    match matcher::best_match(&devices, m) {
        Some(i) => Ok(Some(OsVolume {
            inner: backend::Device::open(&devices[i])?,
            info: devices[i].clone(),
        })),
        None => Ok(None),
    }
}

pub struct OsVolume {
    inner: backend::Device,
    info: DeviceInfo,
}

impl OsVolume {
    pub fn info(&self) -> &DeviceInfo {
        &self.info
    }

    pub fn get(&self) -> Result<VolumeState> {
        self.inner.get()
    }

    /// Clamped to 0.0..=1.0.
    pub fn set_scalar(&self, scalar: f32) -> Result<()> {
        self.inner.set_scalar(convert::clamp_scalar(scalar))
    }

    pub fn set_mute(&self, muted: bool) -> Result<()> {
        self.inner.set_mute(muted)
    }

    /// Notifications stop when the returned [`Subscription`] is dropped.
    pub fn subscribe<F>(&self, callback: F) -> Result<Subscription>
    where
        F: Fn(VolumeState) + Send + Sync + 'static,
    {
        Ok(Subscription {
            _inner: self.inner.subscribe(Box::new(callback))?,
        })
    }
}

/// Dropping it unregisters the callback.
pub struct Subscription {
    _inner: backend::Subscription,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_send_sync<T: Send + Sync>() {}

    #[test]
    fn handles_are_thread_safe() {
        assert_send_sync::<OsVolume>();
        assert_send_sync::<Subscription>();
    }
}
