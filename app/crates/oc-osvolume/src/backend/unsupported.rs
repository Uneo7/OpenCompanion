use crate::{Callback, DeviceInfo, Error, Result, VolumeState};

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    Ok(Vec::new())
}

pub struct Device;
pub struct Subscription;

impl Device {
    pub fn open(_info: &DeviceInfo) -> Result<Self> {
        Err(Error::Unsupported)
    }
    pub fn get(&self) -> Result<VolumeState> {
        Err(Error::Unsupported)
    }
    pub fn set_scalar(&self, _scalar: f32) -> Result<()> {
        Err(Error::Unsupported)
    }
    pub fn set_mute(&self, _muted: bool) -> Result<()> {
        Err(Error::Unsupported)
    }
    pub fn subscribe(&self, _cb: Callback) -> Result<Subscription> {
        Err(Error::Unsupported)
    }
}
