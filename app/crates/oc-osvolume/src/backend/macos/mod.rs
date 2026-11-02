//! The USB audio driver publishes the UAC2 Feature Unit as `kAudioDevicePropertyVolumeScalar` / `Mute`, on the
//! main element or per channel (1, 2); knob changes (UAC2 interrupt endpoint) fire the property listeners.

mod controls;
mod ffi;
mod listener;
mod props;

use ffi::*;
use props::addr;

use crate::matcher::{parse_colon_pair, DeviceInfo};
use crate::{Callback, Error, Result, VolumeState};

pub use listener::Subscription;

use controls::Controls;

fn device_ids() -> Result<Vec<AudioObjectID>> {
    props::get_vec(
        kAudioObjectSystemObject,
        &addr(
            kAudioHardwarePropertyDevices,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
        ),
    )
}

fn global_string(id: AudioObjectID, selector: AudioObjectPropertySelector) -> Option<String> {
    props::get_string(
        id,
        &addr(
            selector,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
        ),
    )
    .ok()
    .filter(|s| !s.is_empty())
}

fn has_output(id: AudioObjectID) -> bool {
    let a = addr(
        kAudioDevicePropertyStreams,
        kAudioDevicePropertyScopeOutput,
        kAudioObjectPropertyElementMain,
    );
    props::get_vec::<AudioObjectID>(id, &a).is_ok_and(|s| !s.is_empty())
}

fn describe(id: AudioObjectID) -> Option<DeviceInfo> {
    if !has_output(id) {
        return None;
    }
    let uid = global_string(id, kAudioDevicePropertyDeviceUID)?;
    let transport: u32 = props::get(
        id,
        &addr(
            kAudioDevicePropertyTransportType,
            kAudioObjectPropertyScopeGlobal,
            kAudioObjectPropertyElementMain,
        ),
    )
    .unwrap_or(0);
    // The USB audio driver's model UID ends in "<vid>:<pid>" (hex). Best
    // effort: name matching remains the fallback.
    let usb = (transport == kAudioDeviceTransportTypeUSB)
        .then(|| global_string(id, kAudioDevicePropertyModelUID))
        .flatten()
        .and_then(|m| parse_colon_pair(&m));
    Some(DeviceInfo {
        id: uid,
        names: global_string(id, kAudioObjectPropertyName)
            .into_iter()
            .collect(),
        usb,
    })
}

pub fn list_devices() -> Result<Vec<DeviceInfo>> {
    Ok(device_ids()?.into_iter().filter_map(describe).collect())
}

pub struct Device {
    id: AudioObjectID,
    controls: Controls,
}

impl Device {
    pub fn open(info: &DeviceInfo) -> Result<Self> {
        let id = device_ids()?
            .into_iter()
            .find(|&id| {
                global_string(id, kAudioDevicePropertyDeviceUID).as_deref() == Some(&info.id)
            })
            .ok_or_else(|| Error::os("CoreAudio", "device disappeared"))?;
        Ok(Self {
            id,
            controls: Controls::probe(id),
        })
    }

    pub fn get(&self) -> Result<VolumeState> {
        self.controls.read(self.id)
    }

    pub fn set_scalar(&self, scalar: f32) -> Result<()> {
        self.controls.set_scalar(self.id, scalar)
    }

    pub fn set_mute(&self, muted: bool) -> Result<()> {
        self.controls.set_mute(self.id, muted)
    }

    pub fn subscribe(&self, cb: Callback) -> Result<Subscription> {
        Subscription::register(self.id, self.controls.clone(), cb)
    }
}
