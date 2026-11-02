mod band;
mod codec;
pub mod file;
pub mod layout;
mod sanitize;
#[cfg(test)]
mod tests;

pub use band::{BandKind, BandSlot, BAND_DISABLED};
pub use codec::{write_ranges, LayoutError};
pub use layout::{flags, limits, EqTarget, EQ_MAX_BANDS, EQ_OUTPUTS, EQ_TARGETS, NAME_LEN, PROFILE_SIZE, PROFILE_VERSION};

/// Default band frequencies of `params_defaults()`: log-spaced 32 Hz .. 12 kHz.
pub const DEFAULT_FREQS: [u16; EQ_MAX_BANDS] = [32, 55, 95, 160, 280, 480, 820, 1400, 2400, 4100, 7000, 12000];

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Profile {
    pub version: u8,
    pub flags: u8,
    /// UI colour index (opaque to the DSP).
    pub color: u8,
    pub reserved0: u8,
    pub name: [u8; NAME_LEN],
    /// Subwoofer level, 1/100 dB.
    pub sub_level_cdb: i16,
    /// Reserved since DSP profile protocol 5, written as 0 (= the stock crossover, always on; the former
    /// 60..250 Hz LR4 choice is ignored by the firmware).
    pub crossover_hz: u16,
    pub aux_gain_cdb: i16,
    /// Inner L, inner R, outer L, outer R, sub.
    pub trim_cdb: [i16; EQ_OUTPUTS],
    /// Reserved since DSP profile protocol 5 (was the software value of the removed bass compensation),
    /// written as 0. The sub level is `sub_level_cdb` (source "app") or the rear knob (`SubSource`).
    pub bass_milli: i16,
    pub reserved1: [u8; 2],
    /// `band[target][slot]`, targets in [`EqTarget`] order.
    pub bands: [[BandSlot; EQ_MAX_BANDS]; EQ_TARGETS],
}

impl Default for Profile {
    fn default() -> Self {
        Self::defaults()
    }
}

impl Profile {
    /// Same content as `params_defaults()`: "Flat", every band off.
    pub fn defaults() -> Self {
        let mut p = Self {
            version: PROFILE_VERSION,
            flags: 0,
            color: 0,
            reserved0: 0,
            name: [0; NAME_LEN],
            sub_level_cdb: 0,
            crossover_hz: 0,
            aux_gain_cdb: 0,
            trim_cdb: [0; EQ_OUTPUTS],
            bass_milli: 0,
            reserved1: [0; 2],
            bands: [DEFAULT_FREQS.map(BandSlot::off_at); EQ_TARGETS],
        };
        p.set_name("Flat");
        p
    }

    pub fn name(&self) -> String {
        name_from_bytes(&self.name)
    }

    pub fn set_name(&mut self, name: &str) {
        self.name = name_to_bytes(name);
    }

    pub fn has_flag(&self, flag: u8) -> bool {
        self.flags & flag != 0
    }

    pub fn set_flag(&mut self, flag: u8, on: bool) {
        if on {
            self.flags |= flag;
        } else {
            self.flags &= !flag;
        }
    }

    pub fn target(&self, t: EqTarget) -> &[BandSlot; EQ_MAX_BANDS] {
        &self.bands[t.index()]
    }

    pub fn target_mut(&mut self, t: EqTarget) -> &mut [BandSlot; EQ_MAX_BANDS] {
        &mut self.bands[t.index()]
    }

}

pub fn name_from_bytes(b: &[u8]) -> String {
    let end = b.iter().position(|&c| c == 0).unwrap_or(b.len());
    String::from_utf8_lossy(&b[..end]).into_owned()
}

pub fn name_to_bytes(name: &str) -> [u8; NAME_LEN] {
    let mut out = [0u8; NAME_LEN];
    let mut len = 0;
    for c in name.chars() {
        let n = c.len_utf8();
        if len + n > NAME_LEN {
            break;
        }
        c.encode_utf8(&mut out[len..len + n]);
        len += n;
    }
    out
}
