//! Setting the mode makes the TAS re-enumerate: the speaker leaves the bus and comes back.

use std::time::Duration;

use oc_protocol::oc_hid::{key, OcLink};

use super::{StreamFormat, TasInfo};

pub const MODE_PROTOCOL: u8 = 4;
/// HID command (oc_tas 0.4.0): no payload = read, reply `[mode]`; `key + [mode]` = set it.
pub const CMD_MODE: u8 = 0x30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum DeviceMode {
    Surround51,
    /// The DSP routes 2.1 by itself in this mode.
    Hq21,
}

impl DeviceMode {
    pub const ALL: [DeviceMode; 2] = [DeviceMode::Surround51, DeviceMode::Hq21];

    fn byte(self) -> u8 {
        match self {
            Self::Surround51 => 1,
            Self::Hq21 => 2,
        }
    }

    fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.into_iter().find(|m| m.byte() == b)
    }

    /// `None`: no stream, or a stereo alternate of an older firmware.
    pub fn of_stream(s: StreamFormat) -> Option<Self> {
        match s {
            StreamFormat::Surround51 => Some(Self::Surround51),
            StreamFormat::Hq21 => Some(Self::Hq21),
            _ => None,
        }
    }
}

pub fn supported(info: &TasInfo) -> bool {
    info.base.protocol >= MODE_PROTOCOL
}

pub fn get_mode(link: &dyn OcLink, info: &TasInfo) -> Option<DeviceMode> {
    if !supported(info) {
        return DeviceMode::of_stream(info.stream_format());
    }
    let reply = link.call(CMD_MODE, &[], Duration::from_millis(300)).ok()?;
    reply.first().copied().and_then(DeviceMode::from_byte)
}

/// The speaker re-enumerates when the mode changes.
pub fn set_mode(link: &dyn OcLink, info: &TasInfo, mode: DeviceMode) -> Result<(), String> {
    if !supported(info) {
        return Err(format!("this USB controller firmware has no mode command (protocol {}, needs {MODE_PROTOCOL})", info.base.protocol));
    }
    let mut payload = key(CMD_MODE).to_vec();
    payload.push(mode.byte());
    link.call(CMD_MODE, &payload, Duration::from_millis(500)).map(drop).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bytes_and_streams() {
        for m in DeviceMode::ALL {
            assert_eq!(DeviceMode::from_byte(m.byte()), Some(m));
        }
        assert_eq!(DeviceMode::of_stream(StreamFormat::Hq21), Some(DeviceMode::Hq21));
        assert_eq!(DeviceMode::of_stream(StreamFormat::Idle), None);
    }
}
