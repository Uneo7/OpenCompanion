//! Small-signal response of the built-in neutral program (`firmware/dsp/src/bose/bose_progs.c`, `eq.c`):
//! USB L / R -> BADDATA selector (direct branch) -> master volume + stock loudness (x + k BP(x), band-pass
//! 0x100103C0) -> routing L, R, L, R, LFE + (L + R) / 2 -> the stock crossover always on in the output lanes:
//! satellites the high-pass of biquad 0x10010A1C (set 0, filter 0x10009D6C) twice, bass the low-pass of
//! biquad 0x100108FC (filter 0x10009C58). The coefficients are read from the user's backup, like the
//! firmware's own copies (`eq.c` `BOSE_HPF_SAT`, `BOSE_LPF_SUB`).

use super::complex::{product, scaled, Spectrum, C64};
use super::graph::biquad_sets;
use super::stock::{StockGraph, LOUD_BP};
use super::ResponseError;

const SAT_HPF: u32 = 0x1001_0A1C;
const BASS_LPF: u32 = 0x1001_08FC;
/// Bass = LFE + 0.5 L + 0.5 R (`engine.c` `mtx[12..13]`).
const BASS_SHARE: f64 = 0.5;

impl StockGraph {
    pub(super) fn set0(&self, addr: u32, w: &[f64]) -> Result<Spectrum, ResponseError> {
        let m = self.module(addr)?;
        let (sets, _) = biquad_sets(&self.ram, m)?;
        Ok(sets[0].response(w))
    }

    pub(super) fn loudness_stage(&self, loud_band: f64, w: &[f64]) -> Result<Spectrum, ResponseError> {
        let bp = self.set0(LOUD_BP, w)?;
        Ok(bp.iter().map(|h| C64::ONE + h.scale(loud_band)).collect())
    }

    pub(super) fn neutral_responses(&self, src: usize, loud_band: f64, w: &[f64]) -> Result<[Spectrum; 5], ResponseError> {
        let input = self.loudness_stage(loud_band, w)?;
        let hpf = self.set0(SAT_HPF, w)?;
        let sat = product(&product(&input, &hpf), &hpf);
        let bass = scaled(&product(&input, &self.set0(BASS_LPF, w)?), BASS_SHARE);
        let zero = vec![C64::ZERO; w.len()];
        Ok(if src == 0 {
            [sat.clone(), zero.clone(), sat, zero, bass]
        } else {
            [zero.clone(), sat.clone(), zero, sat, bass]
        })
    }
}
