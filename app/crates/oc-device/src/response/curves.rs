//! From the complex transfer functions to the curves the app draws.
//!
//! - Per output (inner L, inner R, outer L, outer R, bass): the level for the reference programme,
//!   decorrelated stereo at equal level on USB L and R, i.e. sqrt(|H_L|^2 + |H_R|^2) (in dB).
//! - "Main satellite path" (EQ target "Simple"): the left satellite as a whole, the power average of its two
//!   drivers, sqrt((P_inner_L + P_outer_L) / 2) (the right one is its mirror). With the direct path alone it is
//!   exactly that path's response.
//!
//! 0 dB = the signal leaves the program at the level it came in on USB, the master volume itself excluded,
//! the USB input margin of the Bose program included. Not included: master volume and its -12 dB ceiling,
//! output trims, sub level, the final gain (-1 dB VRAW ceiling, thermal) common to every output and both
//! bases, and anything acoustic (drivers, enclosure, room).

use super::complex::{scaled, Spectrum};
use super::conditions::{Conditions, SoundBase, STEREO_ARRAY, STEREO_DIRECT};
use super::stock::{StockGraph, Steady};
use super::ResponseError;

pub const FS: f64 = 48_000.0;
/// Outputs, in the DSP target order of the output EQ (inner L, inner R, outer L, outer R, bass).
pub const OUTPUTS: usize = 5;

#[derive(Debug, Clone, PartialEq)]
pub struct BaseResponse {
    pub freqs: Vec<f64>,
    pub outputs: [Vec<f64>; OUTPUTS],
    pub main: Vec<f64>,
}

fn power(h: &Spectrum, i: usize) -> f64 {
    h[i].norm_sqr()
}

fn db_of_power(p: f64) -> f64 {
    10.0 * p.max(1e-24).log10()
}

impl StockGraph {
    /// `[from USB L, from USB R][output]`.
    pub fn transfer(&self, c: &Conditions, freqs: &[f64]) -> Result<[[Spectrum; OUTPUTS]; 2], ResponseError> {
        let w: Vec<f64> = freqs.iter().map(|f| 2.0 * std::f64::consts::PI * f / FS).collect();
        let lb = c.loud_band();
        match c.base {
            SoundBase::Bose => {
                let st = Steady { loud_band: lb, direct: STEREO_DIRECT, array: STEREO_ARRAY };
                let k = self.usb_level()?;
                let side = |src| -> Result<[Spectrum; OUTPUTS], ResponseError> { Ok(self.responses(src, &st, &w)?.map(|h| scaled(&h, k))) };
                Ok([side(0)?, side(1)?])
            }
            SoundBase::Neutral => Ok([self.neutral_responses(0, lb, &w)?, self.neutral_responses(1, lb, &w)?]),
        }
    }

    pub fn base_response(&self, c: &Conditions, freqs: &[f64]) -> Result<BaseResponse, ResponseError> {
        let [hl, hr] = self.transfer(c, freqs)?;
        let pw: [Vec<f64>; OUTPUTS] = std::array::from_fn(|o| (0..freqs.len()).map(|i| power(&hl[o], i) + power(&hr[o], i)).collect());
        let outputs = pw.clone().map(|p| p.into_iter().map(db_of_power).collect());
        let main = (0..freqs.len()).map(|i| db_of_power((pw[0][i] + pw[2][i]) / 2.0)).collect();
        Ok(BaseResponse { freqs: freqs.to_vec(), outputs, main })
    }
}
