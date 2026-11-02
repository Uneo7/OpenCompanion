use windows::Win32::Media::Audio::Endpoints::{
    IAudioEndpointVolume, IAudioEndpointVolumeCallback, IAudioEndpointVolumeCallback_Impl,
};
use windows::Win32::Media::Audio::AUDIO_VOLUME_NOTIFICATION_DATA;
use windows_core::implement;

use super::com;
use crate::{Callback, Error, Result, VolumeState};

#[implement(IAudioEndpointVolumeCallback)]
struct Notifier {
    /// Read back the dB level (not part of the notification data). This
    /// reference cycle (endpoint -> callback -> endpoint) is broken by
    /// `Subscription::drop`, which unregisters the callback.
    endpoint: IAudioEndpointVolume,
    cb: Callback,
}

impl IAudioEndpointVolumeCallback_Impl for Notifier_Impl {
    fn OnNotify(&self, data: *mut AUDIO_VOLUME_NOTIFICATION_DATA) -> windows_core::Result<()> {
        // SAFETY: Core Audio passes a valid pointer for the call's duration.
        let Some(data) = (unsafe { data.as_ref() }) else {
            return Ok(());
        };
        let db = unsafe { self.endpoint.GetMasterVolumeLevel() }.unwrap_or(f32::NAN);
        (self.cb)(VolumeState {
            scalar: data.fMasterVolume,
            db,
            muted: data.bMuted.as_bool(),
        });
        Ok(())
    }
}

pub struct Subscription {
    endpoint: IAudioEndpointVolume,
    callback: IAudioEndpointVolumeCallback,
}

// SAFETY: see `Device`; both objects are free-threaded.
unsafe impl Send for Subscription {}
unsafe impl Sync for Subscription {}

impl Subscription {
    pub(super) fn register(endpoint: &IAudioEndpointVolume, cb: Callback) -> Result<Self> {
        let callback: IAudioEndpointVolumeCallback = Notifier {
            endpoint: endpoint.clone(),
            cb,
        }
        .into();
        unsafe { endpoint.RegisterControlChangeNotify(&callback) }
            .map_err(|e| Error::os("RegisterControlChangeNotify", e))?;
        Ok(Self {
            endpoint: endpoint.clone(),
            callback,
        })
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        if com::ensure().is_ok() {
            // Blocks until any in-flight OnNotify has returned.
            let _ = unsafe { self.endpoint.UnregisterControlChangeNotify(&self.callback) };
        }
    }
}
