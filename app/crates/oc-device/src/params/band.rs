//! Raw biquad pairs (types 7 / 8) were removed in DSP profile protocol 6: the firmware and sanitize turn them off.

use super::layout::{band_off, BAND_SIZE};

/// `BAND_DISABLED` (`params.h`, DSP profile protocol 4): enabled bit of the type byte. `kind | BAND_DISABLED`
/// (0x81..0x86) is a parametric band switched off that keeps its type, frequency, gain and Q; the DSP does not
/// compile it and does not count it in the Bose-base CPU cap. Only valid on [`BandKind::is_parametric`] kinds
/// (the DSP turns any other value ≥ `BAND_TYPE_COUNT` into `BAND_OFF`). Profiles saved before have the bit
/// clear; an older DSP firmware reads a disabled band as `BAND_OFF`.
pub const BAND_DISABLED: u8 = 0x80;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum BandKind {
    Off,
    Peak,
    LowShelf,
    HighShelf,
    HighPass,
    LowPass,
    Notch,
}

impl BandKind {
    /// In `BAND_*` order: the index is the type byte.
    pub const ALL: [BandKind; 7] = [
        BandKind::Off,
        BandKind::Peak,
        BandKind::LowShelf,
        BandKind::HighShelf,
        BandKind::HighPass,
        BandKind::LowPass,
        BandKind::Notch,
    ];

    /// `None` for a byte ≥ `BAND_TYPE_COUNT` (the firmware turns such a slot off).
    pub fn from_byte(b: u8) -> Option<Self> {
        Self::ALL.get(usize::from(b)).copied()
    }

    pub fn byte(self) -> u8 {
        self as u8
    }

    /// Peak .. notch: the kinds that may carry [`BAND_DISABLED`].
    pub fn is_parametric(self) -> bool {
        !matches!(self, BandKind::Off)
    }
}

/// `kind` is kept as the raw byte so that unknown values survive a round trip.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct BandSlot {
    pub kind: u8,
    pub reserved: u8,
    pub freq_hz: u16,
    pub gain_cdb: i16,
    pub q_milli: u16,
}

impl BandSlot {
    /// As written by `params_defaults()` (off, 0 dB, Q 0.707).
    pub const fn off_at(freq_hz: u16) -> Self {
        Self { kind: 0, reserved: 0, freq_hz, gain_cdb: 0, q_milli: 707 }
    }

    pub fn new(kind: BandKind, freq_hz: u16, gain_cdb: i16, q_milli: u16) -> Self {
        Self { kind: kind.byte(), reserved: 0, freq_hz, gain_cdb, q_milli }
    }

    /// Kind of the type byte; `None` for a disabled band ([`BandSlot::kept_kind`]) or an unknown byte.
    pub fn band_kind(&self) -> Option<BandKind> {
        BandKind::from_byte(self.kind)
    }

    pub fn is_disabled(&self) -> bool {
        self.kind & BAND_DISABLED != 0 && BandKind::from_byte(self.kind & !BAND_DISABLED).is_some_and(BandKind::is_parametric)
    }

    /// The band's kind, enabled or not: [`BandSlot::band_kind`], or the kind a disabled band keeps.
    pub fn kept_kind(&self) -> Option<BandKind> {
        if self.is_disabled() { BandKind::from_byte(self.kind & !BAND_DISABLED) } else { self.band_kind() }
    }

    pub fn is_enabled(&self) -> bool {
        self.band_kind().is_some_and(BandKind::is_parametric)
    }

    /// No effect on off, raw or unknown slots.
    pub fn set_enabled(&mut self, on: bool) {
        if self.kept_kind().is_some_and(BandKind::is_parametric) {
            self.kind = if on { self.kind & !BAND_DISABLED } else { self.kind | BAND_DISABLED };
        }
    }

    pub fn is_off(&self) -> bool {
        self.band_kind() == Some(BandKind::Off)
    }

    pub fn to_bytes(&self) -> [u8; BAND_SIZE] {
        let mut b = [0u8; BAND_SIZE];
        b[band_off::TYPE] = self.kind;
        b[band_off::RESERVED] = self.reserved;
        b[band_off::FREQ..band_off::FREQ + 2].copy_from_slice(&self.freq_hz.to_le_bytes());
        b[band_off::GAIN..band_off::GAIN + 2].copy_from_slice(&self.gain_cdb.to_le_bytes());
        b[band_off::Q..band_off::Q + 2].copy_from_slice(&self.q_milli.to_le_bytes());
        b
    }

    pub fn from_bytes(b: &[u8; BAND_SIZE]) -> Self {
        let u16_at = |o: usize| u16::from_le_bytes([b[o], b[o + 1]]);
        Self {
            kind: b[band_off::TYPE],
            reserved: b[band_off::RESERVED],
            freq_hz: u16_at(band_off::FREQ),
            gain_cdb: u16_at(band_off::GAIN) as i16,
            q_milli: u16_at(band_off::Q),
        }
    }
}

