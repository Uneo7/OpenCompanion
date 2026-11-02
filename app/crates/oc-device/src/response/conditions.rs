/// Reference master volume (dB) of the curves: the level-dependent stock loudness is evaluated here.
pub const REF_VOLUME_DB: f64 = -20.0;

/// Spatial steady state of the stock control 0x10027978 for the reference programme (decorrelated stereo
/// at equal level on L and R; `re/dsp/spatial_ref.py`, `notes/spatial.md`): direct gain d and array gain
/// sqrt(1 - d^2), the same on both sides.
pub const STEREO_DIRECT: f64 = 0.696_743_130_683_898_9;
pub const STEREO_ARRAY: f64 = 0.717_320_740_222_930_9;

/// `PF_STOCK_GRAPH`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SoundBase {
    Bose,
    /// Stock crossover only.
    Neutral,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Conditions {
    pub base: SoundBase,
    /// Off when the EQ is bypassed.
    pub loudness: bool,
    pub volume_db: f64,
}

impl Conditions {
    pub fn reference(base: SoundBase, loudness: bool) -> Self {
        Self { base, loudness, volume_db: REF_VOLUME_DB }
    }

    /// Band gain of the loudness relative to the direct gain, max(10^(boost / 20) - 1, 0).
    pub fn loud_band(&self) -> f64 {
        if self.loudness {
            (10f64.powf(loudness_boost_db(self.volume_db) / 20.0) - 1.0).max(0.0)
        } else {
            0.0
        }
    }
}

/// Small-signal boost (dB) of the stock loudness 0x1001AF08 (`firmware/dsp/src/audio/loudness.c`) at master
/// volume `volume_db`: the level detector reads its floor (-20 dB), so L = min(volume + 91, 105) and
/// boost = 0.4 (74 - L) + 4 up to 74, 0.2 (94 - L) above, at most 19 dB. This is the largest boost at that
/// volume; loud programme material lowers it.
pub fn loudness_boost_db(volume_db: f64) -> f64 {
    let lvl = (volume_db - 20.0 + 111.0).min(105.0);
    let b = if lvl > 74.0 { 0.2 * (94.0 - lvl) } else { 0.4 * (74.0 - lvl) + 0.2 * (94.0 - 74.0) };
    b.min(19.0)
}
