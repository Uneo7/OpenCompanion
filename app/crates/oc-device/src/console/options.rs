//! Device options byte (`OPT_*`, `firmware/dsp/include/profiles.h`): byte 11 of the DSP's `OCC1` config record,
//! read with `OPU` / `OPC` (4th reply byte, DSP profile protocol 4) and set with `OPC <byte>`.
//!
//! | Bits | Meaning |
//! |---|---|
//! | 0..2 | standby delay before the silence standby request ([`StandbyDelay`]) |
//! | 3 | the pod double press (mute then unmute within 600 ms) does NOT cycle the profiles |
//! | 4 | headphone EQ: the profile's global EQ also plays on the headphone feed (clear = flat, stock) |
//! | 5..7 | signed step −4..+3 of the silence threshold, 6.02 dB per step around the stock −71.1 dBFS |
//!
//! 0 = every stock default (also what records written by older firmware hold).

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
pub enum StandbyDelay {
    /// 2 min.
    #[default]
    Stock,
    Off,
    Min5,
    Min10,
    Min20,
    Min30,
    Min60,
}

impl StandbyDelay {
    /// In code order: the index is the 3-bit field (7 is read as [`StandbyDelay::Stock`]).
    pub const ALL: [StandbyDelay; 7] =
        [Self::Stock, Self::Off, Self::Min5, Self::Min10, Self::Min20, Self::Min30, Self::Min60];

    /// `None` = never.
    pub fn minutes(self) -> Option<u8> {
        match self {
            Self::Stock => Some(2),
            Self::Off => None,
            Self::Min5 => Some(5),
            Self::Min10 => Some(10),
            Self::Min20 => Some(20),
            Self::Min30 => Some(30),
            Self::Min60 => Some(60),
        }
    }

    pub fn from_minutes(minutes: Option<u8>) -> Option<Self> {
        Self::ALL.into_iter().find(|d| d.minutes() == minutes)
    }

    fn code(self) -> u8 {
        self as u8
    }
}

/// Stock silence threshold (mean L/R block power after the master volume), dBFS: 10·log10(7.801579e-8).
pub const THRESHOLD_STOCK_DBFS: f32 = -71.078;
/// dB per threshold step (power × 4).
pub const THRESHOLD_STEP_DB: f32 = 6.0206;
pub const THRESHOLD_STEPS: (i8, i8) = (-4, 3);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct DeviceOptions {
    pub standby: StandbyDelay,
    pub double_tap: bool,
    pub headphone_eq: bool,
    pub threshold_step: i8,
}

impl Default for DeviceOptions {
    fn default() -> Self {
        Self::from_byte(0)
    }
}

impl DeviceOptions {
    pub fn from_byte(b: u8) -> Self {
        Self {
            standby: StandbyDelay::ALL.get(usize::from(b & 7)).copied().unwrap_or_default(),
            double_tap: b & 0x08 == 0,
            headphone_eq: b & 0x10 != 0,
            threshold_step: (b as i8) >> 5,
        }
    }

    pub fn to_byte(self) -> u8 {
        let step = self.threshold_step.clamp(THRESHOLD_STEPS.0, THRESHOLD_STEPS.1);
        self.standby.code()
            | if self.double_tap { 0 } else { 0x08 }
            | if self.headphone_eq { 0x10 } else { 0 }
            | ((step as u8) << 5)
    }

    pub fn threshold_dbfs(self) -> f32 {
        THRESHOLD_STOCK_DBFS + THRESHOLD_STEP_DB * f32::from(self.threshold_step)
    }

    pub fn set_threshold_dbfs(&mut self, dbfs: f32) {
        let step = ((dbfs - THRESHOLD_STOCK_DBFS) / THRESHOLD_STEP_DB).round();
        self.threshold_step = step.clamp(f32::from(THRESHOLD_STEPS.0), f32::from(THRESHOLD_STEPS.1)) as i8;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PROFILES_H: &str =
        include_str!(concat!(env!("CARGO_MANIFEST_DIR"), "/../../../firmware/dsp/include/profiles.h"));

    #[test]
    fn zero_is_stock() {
        let o = DeviceOptions::default();
        assert_eq!(o, DeviceOptions { standby: StandbyDelay::Stock, double_tap: true, headphone_eq: false, threshold_step: 0 });
        assert_eq!(o.to_byte(), 0);
        assert!((o.threshold_dbfs() + 71.08).abs() < 0.01);
    }

    #[test]
    fn every_byte_round_trips() {
        for b in 0..=255u8 {
            let o = DeviceOptions::from_byte(b);
            let want = if b & 7 == 7 { b & !7 } else { b }; // 7 = reserved, read as stock
            assert_eq!(o.to_byte(), want, "{b:#04x}");
        }
    }

    #[test]
    fn fields() {
        let o = DeviceOptions { standby: StandbyDelay::Min30, double_tap: false, headphone_eq: true, threshold_step: -4 };
        assert_eq!(o.to_byte(), 5 | 0x08 | 0x10 | 0x80);
        assert!((o.threshold_dbfs() + 95.16).abs() < 0.01);
        let mut p = o;
        p.set_threshold_dbfs(-54.0);
        assert_eq!(p.threshold_step, 3);
        p.set_threshold_dbfs(-200.0);
        assert_eq!(p.threshold_step, -4);
        assert_eq!(StandbyDelay::from_minutes(Some(10)), Some(StandbyDelay::Min10));
        assert_eq!(StandbyDelay::from_minutes(None), Some(StandbyDelay::Off));
        assert_eq!(StandbyDelay::from_minutes(Some(1)), None);
    }

    #[test]
    fn matches_profiles_h() {
        let def = |name: &str| -> u8 {
            let l = PROFILES_H.lines().find(|l| l.split_whitespace().take(2).eq(["#define", name])).unwrap();
            u8::from_str_radix(l.split_whitespace().nth(2).unwrap().trim_start_matches("0x"), 16).unwrap()
        };
        assert_eq!(def("OPT_DELAY"), 0x07);
        assert_eq!(DeviceOptions { double_tap: false, ..Default::default() }.to_byte(), def("OPT_NO_DTAP"));
        assert_eq!(DeviceOptions { headphone_eq: true, ..Default::default() }.to_byte(), def("OPT_HP_EQ"));
    }
}
