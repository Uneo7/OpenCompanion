mod detect;
pub mod mode;

use std::time::Duration;

use oc_protocol::oc_hid::{self, OcDevice, OcInfo, OcLink, OcRelay};
use oc_protocol::transport::Companion;
use oc_protocol::Link;

pub use detect::{probe, Presence};
pub use mode::DeviceMode;

pub enum Conn {
    Stock(Companion),
    Oc(OcDevice),
}

impl Conn {
    pub fn with_dsp<R>(&self, f: impl FnOnce(&dyn Link) -> R) -> R {
        match self {
            Self::Stock(c) => f(c),
            Self::Oc(d) => f(&OcRelay::new(d)),
        }
    }

    pub fn oc(&self) -> Option<&dyn OcLink> {
        match self {
            Self::Stock(_) => None,
            Self::Oc(d) => Some(d),
        }
    }

    pub fn tas_info(&self) -> Option<oc_protocol::Result<TasInfo>> {
        let link = self.oc()?;
        Some(link.call(oc_hid::cmd::INFO, &[], Duration::from_millis(300)).and_then(|p| TasInfo::parse(&p)))
    }
}

/// USB alternate setting of the streaming interface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    /// alt 0.
    Idle,
    Surround51,
    /// 24-bit, never selected by Windows.
    Hq21,
    Stereo,
    Unknown(u8),
}

impl StreamFormat {
    pub fn bits(self) -> Option<u8> {
        match self {
            StreamFormat::Surround51 => Some(16),
            StreamFormat::Hq21 | StreamFormat::Stereo => Some(24),
            StreamFormat::Idle | StreamFormat::Unknown(_) => None,
        }
    }
}

/// `INFO` reply (`firmware/usb/app/proto/protocol.c`, `cmd_info`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TasInfo {
    pub base: OcInfo,
    pub stream_alt: u8,
    /// UAC Feature Unit volume, 1/256 dB (−127..0 dB), synced with the OS slider.
    pub uac_volume: i16,
    pub uac_mute: bool,
    pub pod_present: bool,
    pub pod_mute: bool,
    /// Pod knob (%), 0..99, when [`TasInfo::knob_valid`].
    pub pod_knob: u8,
    pub knob_valid: bool,
    pub usb_active: bool,
}

impl TasInfo {
    pub fn parse(p: &[u8]) -> oc_protocol::Result<Self> {
        let base = OcInfo::parse(p)?;
        if p.len() < 11 {
            return Err(oc_protocol::Error::Unexpected(format!("INFO reply too short ({} bytes)", p.len())));
        }
        Ok(Self {
            base,
            stream_alt: p[7],
            // Byte 8 (the former rate code) is ignored: OpenCompanion is 48 kHz only (TAS 0.5.1, DSP 0.2.4).
            uac_volume: i16::from_le_bytes([p[9], p[10]]),
            uac_mute: base.flags1 & 0x01 != 0,
            pod_present: base.flags0 & 0x40 != 0,
            pod_mute: base.flags0 & 0x80 != 0,
            pod_knob: p.get(21).copied().unwrap_or(0),
            knob_valid: base.flags1 & 0x02 != 0,
            usb_active: base.flags0 & 0x02 != 0,
        })
    }

    /// What the computer sends, from the alternate setting: TAS 0.3+ has 1 = 5.1 16-bit, 2 = 2.1 24-bit,
    /// 3 = stereo 24-bit; TAS 0.2.x had only 1 = stereo 24-bit (`tools/oc_tas.py`).
    pub fn stream_format(&self) -> StreamFormat {
        let (major, minor, _) = self.base.version;
        match (self.stream_alt, (major, minor) >= (0, 3)) {
            (0, _) => StreamFormat::Idle,
            (1, true) => StreamFormat::Surround51,
            (2, true) => StreamFormat::Hq21,
            (3, true) | (1, false) => StreamFormat::Stereo,
            (alt, _) => StreamFormat::Unknown(alt),
        }
    }

    pub fn volume_db(&self) -> f32 {
        f32::from(self.uac_volume) / 256.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn info_fields() {
        let mut p = vec![3, 0, 2, 1, 0x00, 0x5A, 0x03];
        p.extend_from_slice(&[1, 0]); // alt 1, byte 8 ignored (48 kHz only)
        p.extend_from_slice(&(-20i16 * 256).to_le_bytes());
        p.resize(23, 0);
        p[21] = 42;
        let i = TasInfo::parse(&p).unwrap();
        assert_eq!((i.stream_alt, i.volume_db()), (1, -20.0));
        assert!(i.uac_mute && i.knob_valid && i.pod_present && !i.pod_mute && i.usb_active);
        assert_eq!(i.pod_knob, 42);
        assert_eq!(i.base.version_string(), "0.2.1");
        assert!(TasInfo::parse(&p[..9]).is_err());
        // TAS 0.2.x: alt 1 is the stereo 24-bit stream.
        assert_eq!(i.stream_format(), StreamFormat::Stereo);
        p[1..4].copy_from_slice(&[0, 3, 0]);
        assert_eq!(TasInfo::parse(&p).unwrap().stream_format(), StreamFormat::Surround51);
        p[7] = 0;
        assert_eq!(TasInfo::parse(&p).unwrap().stream_format(), StreamFormat::Idle);
        p[1..4].copy_from_slice(&[0, 3, 0]);
        p[7] = 2;
        assert_eq!(TasInfo::parse(&p).unwrap().stream_format(), StreamFormat::Hq21);
    }
}
