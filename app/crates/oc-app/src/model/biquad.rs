//! EQ maths: RBJ "Audio EQ Cookbook" biquads at 48 kHz, and their magnitude response.
//! The graph draws exactly what the DSP will run: one second-order section per band, coefficients from the
//! cookbook formulas, magnitude evaluated on the unit circle.

use std::f64::consts::PI;
use std::sync::LazyLock;

use super::eq::{Band, BandType};

pub const FS: f64 = 48_000.0;
pub const F_MIN: f64 = 20.0;
pub const F_MAX: f64 = 20_000.0;
pub const POINTS: usize = 300;

/// Biquad coefficients, not normalized: H(z) = (b0 + b1 z⁻¹ + b2 z⁻²) / (a0 + a1 z⁻¹ + a2 z⁻²).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Coeffs {
    pub b0: f64,
    pub b1: f64,
    pub b2: f64,
    pub a0: f64,
    pub a1: f64,
    pub a2: f64,
}

impl Coeffs {
    pub fn for_band(band: &Band) -> Self {
        let a = 10f64.powf(band.gain / 40.0);
        let w = 2.0 * PI * band.freq / FS;
        let (sn, cs) = w.sin_cos();
        let al = sn / (2.0 * band.q);
        let sa = a.sqrt();
        let (b0, b1, b2, a0, a1, a2) = match band.kind {
            BandType::Peak => (1.0 + al * a, -2.0 * cs, 1.0 - al * a, 1.0 + al / a, -2.0 * cs, 1.0 - al / a),
            BandType::LowShelf => (
                a * ((a + 1.0) - (a - 1.0) * cs + 2.0 * sa * al),
                2.0 * a * ((a - 1.0) - (a + 1.0) * cs),
                a * ((a + 1.0) - (a - 1.0) * cs - 2.0 * sa * al),
                (a + 1.0) + (a - 1.0) * cs + 2.0 * sa * al,
                -2.0 * ((a - 1.0) + (a + 1.0) * cs),
                (a + 1.0) + (a - 1.0) * cs - 2.0 * sa * al,
            ),
            BandType::HighShelf => (
                a * ((a + 1.0) + (a - 1.0) * cs + 2.0 * sa * al),
                -2.0 * a * ((a - 1.0) + (a + 1.0) * cs),
                a * ((a + 1.0) + (a - 1.0) * cs - 2.0 * sa * al),
                (a + 1.0) - (a - 1.0) * cs + 2.0 * sa * al,
                2.0 * ((a - 1.0) - (a + 1.0) * cs),
                (a + 1.0) - (a - 1.0) * cs - 2.0 * sa * al,
            ),
            BandType::LowPass => ((1.0 - cs) / 2.0, 1.0 - cs, (1.0 - cs) / 2.0, 1.0 + al, -2.0 * cs, 1.0 - al),
            BandType::HighPass => ((1.0 + cs) / 2.0, -(1.0 + cs), (1.0 + cs) / 2.0, 1.0 + al, -2.0 * cs, 1.0 - al),
            BandType::Notch => (1.0, -2.0 * cs, 1.0, 1.0 + al, -2.0 * cs, 1.0 - al),
        };
        Self { b0, b1, b2, a0, a1, a2 }
    }

    pub fn magnitude_db(&self, f: f64) -> f64 {
        let w = 2.0 * PI * f / FS;
        let (s1, c1) = w.sin_cos();
        let (s2, c2) = (2.0 * w).sin_cos();
        let nr = self.b0 + self.b1 * c1 + self.b2 * c2;
        let ni = self.b1 * s1 + self.b2 * s2;
        let dr = self.a0 + self.a1 * c1 + self.a2 * c2;
        let di = self.a1 * s1 + self.a2 * s2;
        let num = (nr * nr + ni * ni).max(1e-300);
        let den = (dr * dr + di * di).max(1e-300);
        10.0 * (num / den).log10()
    }
}

pub static FREQS: LazyLock<[f64; POINTS]> =
    LazyLock::new(|| std::array::from_fn(|i| F_MIN * (F_MAX / F_MIN).powf(i as f64 / (POINTS - 1) as f64)));

pub fn response<'a>(bands: impl IntoIterator<Item = &'a Band>) -> [f64; POINTS] {
    let mut out = [0.0; POINTS];
    for band in bands.into_iter().filter(|b| b.on) {
        let c = Coeffs::for_band(band);
        for (o, f) in out.iter_mut().zip(FREQS.iter()) {
            *o += c.magnitude_db(*f);
        }
    }
    out
}

pub fn band_response(band: &Band) -> [f64; POINTS] {
    let c = Coeffs::for_band(band);
    std::array::from_fn(|i| c.magnitude_db(FREQS[i]))
}

pub fn sample_at(curve: &[f64; POINTS], f: f64) -> f64 {
    let k = (f / F_MIN).ln() / (F_MAX / F_MIN).ln() * (POINTS - 1) as f64;
    let i0 = (k.floor() as isize).clamp(0, POINTS as isize - 2) as usize;
    let t = k - i0 as f64;
    curve[i0] * (1.0 - t) + curve[i0 + 1] * t
}

#[cfg(test)]
mod tests {
    use super::*;

    fn band(kind: BandType, freq: f64, gain: f64, q: f64) -> Band {
        Band::new(kind, freq, gain, q)
    }

    fn db(b: &Band, f: f64) -> f64 {
        Coeffs::for_band(b).magnitude_db(f)
    }

    #[test]
    fn peak_reaches_its_gain_at_the_centre() {
        for g in [-15.0, -6.0, 3.0, 12.0] {
            let b = band(BandType::Peak, 1000.0, g, 1.4);
            assert!((db(&b, 1000.0) - g).abs() < 1e-6, "gain {g}: {}", db(&b, 1000.0));
        }
    }

    #[test]
    fn peak_is_flat_far_away_and_symmetric_in_log_frequency() {
        let b = band(BandType::Peak, 1000.0, 6.0, 2.0);
        assert!(db(&b, 20.0).abs() < 0.05);
        // Bilinear warping makes the high side slightly narrower; the octave pair stays close.
        assert!((db(&b, 500.0) - db(&b, 2000.0)).abs() < 0.2);
    }

    #[test]
    fn peak_with_zero_gain_is_transparent() {
        let b = band(BandType::Peak, 3000.0, 0.0, 0.7);
        for f in FREQS.iter() {
            assert!(db(&b, *f).abs() < 1e-9);
        }
    }

    #[test]
    fn shelves_reach_their_gain_on_the_shelf_and_half_at_the_corner() {
        let low = band(BandType::LowShelf, 200.0, 6.0, std::f64::consts::FRAC_1_SQRT_2);
        assert!((db(&low, 20.0) - 6.0).abs() < 0.05);
        assert!(db(&low, 15_000.0).abs() < 0.05);
        assert!((db(&low, 200.0) - 3.0).abs() < 0.05);
        let high = band(BandType::HighShelf, 4000.0, -9.0, std::f64::consts::FRAC_1_SQRT_2);
        assert!((db(&high, 20_000.0) + 9.0).abs() < 0.3);
        assert!(db(&high, 50.0).abs() < 0.05);
        assert!((db(&high, 4000.0) + 4.5).abs() < 0.05);
    }

    #[test]
    fn butterworth_filters_are_minus_3db_at_the_cutoff() {
        let q = std::f64::consts::FRAC_1_SQRT_2;
        let hp = band(BandType::HighPass, 100.0, 0.0, q);
        let lp = band(BandType::LowPass, 5000.0, 0.0, q);
        assert!((db(&hp, 100.0) + 3.0103).abs() < 0.01);
        assert!((db(&lp, 5000.0) + 3.0103).abs() < 0.01);
        // 12 dB per octave far from the cutoff.
        assert!((db(&hp, 25.0) - db(&hp, 50.0) + 12.0).abs() < 0.3);
        assert!(db(&hp, 10_000.0).abs() < 0.01);
        assert!(db(&lp, 50.0).abs() < 0.01);
    }

    #[test]
    fn notch_removes_its_frequency_only() {
        let b = band(BandType::Notch, 50.0, 0.0, 10.0);
        assert!(db(&b, 50.0) < -100.0);
        assert!(db(&b, 200.0).abs() < 0.05);
    }

    #[test]
    fn normalized_coefficients_match_the_cookbook() {
        // Peak 1 kHz, +6 dB, Q 1 at 48 kHz (reference values computed from the cookbook formulas).
        let k = Coeffs::for_band(&band(BandType::Peak, 1000.0, 6.0, 1.0));
        // Divided by a0, as the DSP stores them: [b0, b1, b2, a1, a2].
        let c = [k.b0 / k.a0, k.b1 / k.a0, k.b2 / k.a0, k.a1 / k.a0, k.a2 / k.a0];
        let expected = [1.043953, -1.895322, 0.867724, -1.895322, 0.911675];
        for (a, b) in c.iter().zip(expected) {
            assert!((a - b).abs() < 1e-3, "{c:?}");
        }
        // Stable: poles inside the unit circle (|a2| < 1 and |a1| < 1 + a2).
        assert!(c[4].abs() < 1.0 && c[3].abs() < 1.0 + c[4]);
    }

    #[test]
    fn response_sums_enabled_bands_only() {
        let a = band(BandType::Peak, 100.0, 4.0, 1.0);
        let mut b = band(BandType::Peak, 5000.0, -3.0, 1.0);
        let both = response([&a, &b]);
        let ra = band_response(&a);
        let rb = band_response(&b);
        for i in 0..POINTS {
            assert!((both[i] - ra[i] - rb[i]).abs() < 1e-9);
        }
        b.on = false;
        let only_a = response([&a, &b]);
        assert_eq!(only_a, ra);
    }

    #[test]
    fn grid_spans_20_hz_to_20_khz_and_sampling_interpolates() {
        assert!((FREQS[0] - 20.0).abs() < 1e-9 && (FREQS[POINTS - 1] - 20_000.0).abs() < 1e-6);
        let b = band(BandType::Peak, 1000.0, 6.0, 1.0);
        let r = band_response(&b);
        assert!((sample_at(&r, 1000.0) - 6.0).abs() < 0.05);
        assert!((sample_at(&r, 20.0) - r[0]).abs() < 1e-9);
    }
}
