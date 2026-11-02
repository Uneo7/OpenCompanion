//! Same hard rules (spec 4.1) as the DSP applies before it plays a base.

use super::model::{Base, Coefs};
use super::topology::{DELAY_MAX, SETS};
use super::BaseError;

/// `RAW_POLE_MAX`² of `params.h`: |a2| must stay below it.
pub const A2_MAX: f32 = 0.99998;
pub const B_MAX: f32 = 8.0;
pub const SET_GAIN_MAX: f32 = 16.0;
pub const SCALAR_MAX: f32 = 8.0;

/// The firmware's stability test (`params_sanitize` raw bands): |a2| < 0.99998 and |a1| < 1 − a2.
pub fn section_ok(c: &Coefs) -> Result<(), String> {
    let [b1, b2, a1, a2] = *c;
    if !c.iter().all(|x| x.is_finite()) {
        return Err("not a finite number".into());
    }
    if b1.abs() > B_MAX || b2.abs() > B_MAX {
        return Err(format!("numerator ({b1}, {b2}) beyond ±{B_MAX}"));
    }
    if a2.abs() >= A2_MAX || a1.abs() >= 1.0 - a2 {
        return Err(format!("poles too close to or outside the unit circle (a1 {a1}, a2 {a2})"));
    }
    Ok(())
}

pub fn check(b: &Base) -> Result<(), BaseError> {
    let bad = |s: String| Err(BaseError::Invalid(s));
    for (k, x) in b.scalars().iter().enumerate() {
        if !x.is_finite() || x.abs() > SCALAR_MAX {
            return bad(format!("scalar gain #{k} = {x} (bound ±{SCALAR_MAX})"));
        }
    }
    if b.sets.len() != SETS.len() {
        return bad(format!("{} sets, topology has {}", b.sets.len(), SETS.len()));
    }
    for (d, s) in SETS.iter().zip(&b.sets) {
        let name = if d.key.is_empty() { d.block.to_string() } else { format!("{}.{}", d.block, d.key) };
        if s.sections.len() != d.nsec {
            return bad(format!("{name}: {} sections, topology has {}", s.sections.len(), d.nsec));
        }
        if !s.gain.is_finite() || s.gain.abs() > SET_GAIN_MAX {
            return bad(format!("{name}: gain {} (bound ±{SET_GAIN_MAX})", s.gain));
        }
        for (k, c) in s.sections.iter().enumerate() {
            if let Err(e) = section_ok(c) {
                return bad(format!("{name} section {k}: {e}"));
            }
        }
    }
    if let Some(d) = b.delays.iter().find(|&&d| d > DELAY_MAX) {
        return bad(format!("delay {d} samples (bound {DELAY_MAX})"));
    }
    Ok(())
}
