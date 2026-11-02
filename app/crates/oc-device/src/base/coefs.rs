//! Everything is computed in f64 and rounded once to float32.

use std::f64::consts::PI;

use super::model::Coefs;
use super::topology::SAMPLE_RATE;
use super::BaseError;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Roots {
    Pair { f_hz: f64, r: f64 },
    Real([f64; 2]),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RbjKind {
    Peak,
    LowShelf,
    HighShelf,
    Lowpass,
    Highpass,
    Bandpass,
    Notch,
    Allpass,
}

impl RbjKind {
    pub const ALL: [RbjKind; 8] = [
        RbjKind::Peak,
        RbjKind::LowShelf,
        RbjKind::HighShelf,
        RbjKind::Lowpass,
        RbjKind::Highpass,
        RbjKind::Bandpass,
        RbjKind::Notch,
        RbjKind::Allpass,
    ];

    pub fn key(self) -> &'static str {
        match self {
            RbjKind::Peak => "peak",
            RbjKind::LowShelf => "low_shelf",
            RbjKind::HighShelf => "high_shelf",
            RbjKind::Lowpass => "lowpass",
            RbjKind::Highpass => "highpass",
            RbjKind::Bandpass => "bandpass",
            RbjKind::Notch => "notch",
            RbjKind::Allpass => "allpass",
        }
    }

    pub fn has_gain(self) -> bool {
        matches!(self, RbjKind::Peak | RbjKind::LowShelf | RbjKind::HighShelf)
    }
}

/// Bounds of the RBJ parameters (spec 3.3).
pub const F_MIN: f64 = 1.0;
pub const F_MAX: f64 = 23_999.0;
pub const Q_MIN: f64 = 0.05;
pub const Q_MAX: f64 = 50.0;
pub const RBJ_GAIN_MIN: f64 = -60.0;
pub const RBJ_GAIN_MAX: f64 = 30.0;

fn omega(f_hz: f64) -> f64 {
    2.0 * PI * f_hz / f64::from(SAMPLE_RATE)
}

/// Monic polynomial 1 + c1 z⁻¹ + c2 z⁻² with these roots: (c1, c2).
fn poly(r: Roots) -> (f64, f64) {
    match r {
        Roots::Pair { f_hz, r } => (-2.0 * r * omega(f_hz).cos(), r * r),
        Roots::Real([x1, x2]) => (-(x1 + x2), x1 * x2),
    }
}

/// Pole / zero form → coefficients (Bose convention: the denominator is 1 − a1 z⁻¹ − a2 z⁻²).
pub fn from_zpk(zeros: Roots, poles: Roots) -> Coefs {
    let (b1, b2) = poly(zeros);
    let (p1, p2) = poly(poles);
    [b1 as f32, b2 as f32, (-p1) as f32, (-p2) as f32]
}

/// RBJ section → (gain factor b0 / a0 to fold into the set gain, monic coefficients).
pub fn from_rbj(kind: RbjKind, f_hz: f64, q: f64, gain_db: f64) -> Result<(f64, Coefs), BaseError> {
    let bad = |s: String| BaseError::Invalid(s);
    if !(F_MIN..=F_MAX).contains(&f_hz) {
        return Err(bad(format!("{}: f_hz {f_hz} outside {F_MIN}..{F_MAX}", kind.key())));
    }
    if !(Q_MIN..=Q_MAX).contains(&q) {
        return Err(bad(format!("{}: q {q} outside {Q_MIN}..{Q_MAX}", kind.key())));
    }
    if kind.has_gain() && !(RBJ_GAIN_MIN..=RBJ_GAIN_MAX).contains(&gain_db) {
        return Err(bad(format!("{}: gain_db {gain_db} outside {RBJ_GAIN_MIN}..{RBJ_GAIN_MAX}", kind.key())));
    }
    let w = omega(f_hz);
    let (cw, sw) = (w.cos(), w.sin());
    let alpha = sw / (2.0 * q);
    let a = 10f64.powf(gain_db / 40.0);
    let sa = 2.0 * a.sqrt() * alpha;
    let (b, d): ([f64; 3], [f64; 3]) = match kind {
        RbjKind::Peak => ([1.0 + alpha * a, -2.0 * cw, 1.0 - alpha * a], [1.0 + alpha / a, -2.0 * cw, 1.0 - alpha / a]),
        RbjKind::LowShelf => (
            [a * ((a + 1.0) - (a - 1.0) * cw + sa), 2.0 * a * ((a - 1.0) - (a + 1.0) * cw), a * ((a + 1.0) - (a - 1.0) * cw - sa)],
            [(a + 1.0) + (a - 1.0) * cw + sa, -2.0 * ((a - 1.0) + (a + 1.0) * cw), (a + 1.0) + (a - 1.0) * cw - sa],
        ),
        RbjKind::HighShelf => (
            [a * ((a + 1.0) + (a - 1.0) * cw + sa), -2.0 * a * ((a - 1.0) + (a + 1.0) * cw), a * ((a + 1.0) + (a - 1.0) * cw - sa)],
            [(a + 1.0) - (a - 1.0) * cw + sa, 2.0 * ((a - 1.0) - (a + 1.0) * cw), (a + 1.0) - (a - 1.0) * cw - sa],
        ),
        RbjKind::Lowpass => ([(1.0 - cw) / 2.0, 1.0 - cw, (1.0 - cw) / 2.0], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        RbjKind::Highpass => ([(1.0 + cw) / 2.0, -(1.0 + cw), (1.0 + cw) / 2.0], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        RbjKind::Bandpass => ([alpha, 0.0, -alpha], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        RbjKind::Notch => ([1.0, -2.0 * cw, 1.0], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
        RbjKind::Allpass => ([1.0 - alpha, -2.0 * cw, 1.0 + alpha], [1.0 + alpha, -2.0 * cw, 1.0 - alpha]),
    };
    if b[0].abs() < 1e-12 {
        return Err(bad(format!("{}: leading coefficient is zero", kind.key())));
    }
    let c = [b[1] / b[0], b[2] / b[0], -d[1] / d[0], -d[2] / d[0]];
    Ok((b[0] / d[0], c.map(|x| x as f32)))
}

fn roots(c1: f64, c2: f64) -> Roots {
    let disc = c1 * c1 - 4.0 * c2;
    if disc >= 0.0 {
        let s = disc.sqrt();
        Roots::Real([(-c1 + s) / 2.0, (-c1 - s) / 2.0])
    } else {
        let r = c2.sqrt();
        let w = (-c1 / (2.0 * r)).clamp(-1.0, 1.0).acos();
        Roots::Pair { f_hz: w * f64::from(SAMPLE_RATE) / (2.0 * PI), r }
    }
}

pub fn zpk(c: &Coefs) -> (Roots, Roots) {
    let [b1, b2, a1, a2] = c.map(f64::from);
    (roots(b1, b2), roots(-a1, -a2))
}

pub fn pole_radius(c: &Coefs) -> f64 {
    match zpk(c).1 {
        Roots::Pair { r, .. } => r,
        Roots::Real([x1, x2]) => x1.abs().max(x2.abs()),
    }
}
