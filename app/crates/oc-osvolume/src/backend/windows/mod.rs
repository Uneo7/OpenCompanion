//! For a UAC2 device with a Feature Unit the endpoint volume *is* the hardware volume: setting it sends
//! SET_CUR, and knob changes come back as `IAudioEndpointVolumeCallback` notifications.

mod callback;
mod com;
mod enumerate;

use windows::Win32::Media::Audio::Endpoints::IAudioEndpointVolume;
use windows::Win32::System::Com::CLSCTX_ALL;

use crate::{Callback, DeviceInfo, Error, Result, VolumeState};

pub use callback::Subscription;
pub use enumerate::list_devices;

/// Lets notifications be traced back to our own changes.
const EVENT_CONTEXT: windows_core::GUID =
    windows_core::GUID::from_u128(0x6f63_6f73_766f_6c75_6d65_0000_0000_0001);

pub struct Device {
    endpoint: IAudioEndpointVolume,
}

// SAFETY: MMDevice API objects are free-threaded (they aggregate the
// free-threaded marshaler), and every entry point below first makes sure COM
// is initialized on the calling thread.
unsafe impl Send for Device {}
unsafe impl Sync for Device {}

impl Device {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        com::ensure()?;
        let device = enumerate::device_by_id(&info.id)?;
        let endpoint: IAudioEndpointVolume = unsafe { device.Activate(CLSCTX_ALL, None) }
            .map_err(|e| Error::os("IMMDevice::Activate", e))?;
        Ok(Self { endpoint })
    }

    pub fn get(&self) -> Result<VolumeState> {
        com::ensure()?;
        read_state(&self.endpoint)
    }

    pub fn set_scalar(&self, scalar: f32) -> Result<()> {
        com::ensure()?;
        unsafe {
            self.endpoint
                .SetMasterVolumeLevelScalar(scalar, &EVENT_CONTEXT)
        }
        .map_err(|e| Error::os("SetMasterVolumeLevelScalar", e))
    }

    pub fn set_mute(&self, muted: bool) -> Result<()> {
        com::ensure()?;
        unsafe { self.endpoint.SetMute(muted, &EVENT_CONTEXT) }
            .map_err(|e| Error::os("IAudioEndpointVolume::SetMute", e))
    }

    pub fn subscribe(&self, cb: Callback) -> Result<Subscription> {
        com::ensure()?;
        Subscription::register(&self.endpoint, cb)
    }
}

fn read_state(endpoint: &IAudioEndpointVolume) -> Result<VolumeState> {
    unsafe {
        let scalar = endpoint
            .GetMasterVolumeLevelScalar()
            .map_err(|e| Error::os("GetMasterVolumeLevelScalar", e))?;
        let db = endpoint
            .GetMasterVolumeLevel()
            .map_err(|e| Error::os("GetMasterVolumeLevel", e))?;
        let muted = endpoint
            .GetMute()
            .map_err(|e| Error::os("IAudioEndpointVolume::GetMute", e))?;
        Ok(VolumeState {
            scalar,
            db,
            muted: muted.as_bool(),
        })
    }
}
