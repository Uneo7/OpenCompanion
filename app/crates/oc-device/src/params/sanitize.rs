//! Host copy of `params_sanitize()` (`firmware/dsp/src/control/params.c`): the DSP clamps every write, so the
//! app applies the same rules to its mirror of the active profile and stays byte-identical with the device.

use super::band::BandKind;
use super::layout::{flags, limits, PROFILE_VERSION};
use super::Profile;

fn clamp_i16(v: i16, (lo, hi): (i16, i16)) -> i16 {
    v.clamp(lo, hi)
}

impl Profile {
    pub fn sanitize(&mut self) {
        self.version = PROFILE_VERSION;
        self.flags &= flags::ALL;
        self.reserved0 = 0;
        self.sub_level_cdb = clamp_i16(self.sub_level_cdb, limits::SUB_LEVEL_CDB);
        self.crossover_hz = 0; // reserved (DSP profile protocol 5): the stock crossover only
        self.aux_gain_cdb = clamp_i16(self.aux_gain_cdb, limits::AUX_GAIN_CDB);
        for t in &mut self.trim_cdb {
            *t = clamp_i16(*t, limits::TRIM_CDB);
        }
        self.bass_milli = 0; // reserved (protocol 5): the bass compensation is gone, see `SubSource`
        self.reserved1 = [0; 2];
        for target in &mut self.bands {
            for bd in target.iter_mut() {
                if bd.band_kind().is_none() && !bd.is_disabled() {
                    bd.kind = BandKind::Off.byte();
                }
                bd.reserved = 0;
                bd.freq_hz = bd.freq_hz.clamp(limits::FREQ_HZ.0, limits::FREQ_HZ.1);
                bd.gain_cdb = clamp_i16(bd.gain_cdb, limits::GAIN_CDB);
                bd.q_milli = bd.q_milli.clamp(limits::Q_MILLI.0, limits::Q_MILLI.1);
            }
        }
    }

    pub fn sanitized(mut self) -> Self {
        self.sanitize();
        self
    }
}
