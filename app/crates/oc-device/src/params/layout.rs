//! Wire format of `OPR` / `OPW` and EEPROM storage format: little-endian, no padding.
//! `tests.rs` checks every value here against `firmware/dsp/include/params.h`.

pub const PROFILE_VERSION: u8 = 1;
pub const PROFILE_SIZE: usize = 616;
pub const EQ_MAX_BANDS: usize = 12;
pub const EQ_TARGETS: usize = 6;
pub const EQ_OUTPUTS: usize = 5;
/// `PROFILE_NAME_LEN`: UTF-8, NUL-padded, not necessarily NUL-terminated.
pub const NAME_LEN: usize = 16;
pub const BAND_SIZE: usize = 8;

pub mod off {
    pub const VERSION: usize = 0;
    pub const FLAGS: usize = 1;
    pub const COLOR: usize = 2;
    pub const RESERVED0: usize = 3;
    pub const NAME: usize = 4;
    pub const SUB_LEVEL: usize = 20;
    pub const CROSSOVER: usize = 22;
    pub const AUX_GAIN: usize = 24;
    pub const TRIMS: usize = 26;
    pub const BASS: usize = 36;
    pub const RESERVED1: usize = 38;
    pub const BANDS: usize = 40;
}

pub mod band_off {
    pub const TYPE: usize = 0;
    pub const RESERVED: usize = 1;
    pub const FREQ: usize = 2;
    pub const GAIN: usize = 4;
    pub const Q: usize = 6;
}

pub mod flags {
    /// Bypass the user EQ (global + per output + loudness).
    pub const BYPASS: u8 = 0x01;
    /// Right outputs use the left outputs' bands.
    pub const LINK_LR: u8 = 0x02;
    pub const LOUDNESS: u8 = 0x04;
    /// Former stock (Bose percentage) volume law: kept in the format, ignored by the firmware since layout v3
    /// (the master volume is always the UAC dB value, in every profile).
    pub const VOL_STOCK: u8 = 0x08;
    /// Base of the profile: the stock Bose signal graph ("Bose base"); clear = neutral base (EQ path). The
    /// user EQ, trims and bass value apply on top of either base.
    pub const STOCK_GRAPH: u8 = 0x10;
    /// App only: the EQ is edited per output ("Advanced") rather than global ("Simple"). The DSP keeps the bit and
    /// ignores it (`PF_ADVANCED`).
    pub const ADVANCED: u8 = 0x20;
    /// Bits `params_sanitize()` keeps.
    pub const ALL: u8 = BYPASS | LINK_LR | LOUDNESS | VOL_STOCK | STOCK_GRAPH | ADVANCED;
}

/// Ranges enforced by `params_sanitize()`.
pub mod limits {
    pub const SUB_LEVEL_CDB: (i16, i16) = (-1200, 1200);
    pub const AUX_GAIN_CDB: (i16, i16) = (-2400, 1200);
    pub const TRIM_CDB: (i16, i16) = (-1200, 600);
    pub const FREQ_HZ: (u16, u16) = (20, 20000);
    pub const GAIN_CDB: (i16, i16) = (-1500, 1500);
    pub const Q_MILLI: (u16, u16) = (100, 20000);
    /// `RAW_POLE_MAX`: largest accepted pole radius (stability limit of the parametric base format).
    pub const RAW_POLE_MAX: f32 = 0.99999;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum EqTarget {
    Global = 0,
    InnerL = 1,
    InnerR = 2,
    OuterL = 3,
    OuterR = 4,
    Sub = 5,
}

impl EqTarget {
    pub const ALL: [EqTarget; EQ_TARGETS] =
        [EqTarget::Global, EqTarget::InnerL, EqTarget::InnerR, EqTarget::OuterL, EqTarget::OuterR, EqTarget::Sub];

    pub const fn index(self) -> usize {
        self as usize
    }
}

pub const fn band_offset(target: EqTarget, index: usize) -> usize {
    off::BANDS + (target.index() * EQ_MAX_BANDS + index) * BAND_SIZE
}

pub const fn target_range(target: EqTarget) -> std::ops::Range<usize> {
    let start = band_offset(target, 0);
    start..start + EQ_MAX_BANDS * BAND_SIZE
}
