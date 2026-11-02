//! Safety cap of the user EQ: the combined boost of any output never exceeds [`LIMIT`] (dB) at any frequency
//! of the graph, as the app's own biquad response computes it. Stacked high-Q bands could otherwise reach tens
//! of dB and make the DSP filters ring. Cuts are not limited.
//!
//! Simple mode checks the global list; Advanced checks each output's own list.

use super::biquad::response;
use super::eq::{Band, ProfileBands, Target};

pub const LIMIT: f64 = 15.0;
/// Rounding margin of the displayed values (0.1 dB, 0.01 Q).
const SLACK: f64 = 0.05;
/// Butterworth Q: a pass filter at or below it has no resonance peak.
const FLAT_Q: f64 = 0.707;

/// Highest point of the combined response (dB).
pub fn peak(bands: &[Band]) -> f64 {
    response(bands).into_iter().fold(f64::NEG_INFINITY, f64::max)
}

fn over(bands: &[Band]) -> bool {
    peak(bands) > LIMIT + SLACK
}

/// The lists that make a sound: the global one (Simple) or each output's (Advanced).
fn checked(p: &ProfileBands) -> &'static [Target] {
    if p.advanced { &Target::ADVANCED } else { &[Target::Global] }
}

/// The same band with no boost: gain at most 0, no resonance.
fn neutral(b: &Band) -> Band {
    let mut n = *b;
    if n.kind.has_gain() {
        n.gain = n.gain.min(0.0);
    } else {
        n.q = n.q.min(FLAT_Q);
    }
    n
}

/// From `a` (t = 0) to `b` (t = 1): frequency and Q on a log scale, gain linearly; `b`'s shape and state.
fn blend(a: &Band, b: &Band, t: f64) -> Band {
    let geo = |x: f64, y: f64| x * (y / x).powf(t);
    Band { kind: b.kind, freq: geo(a.freq, b.freq), gain: a.gain + (b.gain - a.gain) * t, q: geo(a.q, b.q), on: b.on }
}

/// Largest `t` in [0, 1] for which `ok(t)` holds, `ok(0)` being true.
fn bisect(ok: impl Fn(f64) -> bool) -> f64 {
    if ok(1.0) {
        return 1.0;
    }
    let (mut lo, mut hi) = (0.0, 1.0);
    for _ in 0..40 {
        let mid = (lo + hi) / 2.0;
        if ok(mid) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo
}

/// Keeps an edit within the limit: the bands that differ from `prev` (the state before the gesture) are moved
/// back towards it, or towards a neutral version for a new band or a new shape, just as far as needed. True when
/// the limit bit.
pub fn limit_edit(prev: &[Band], next: &mut [Band]) -> bool {
    if !over(next) {
        return false;
    }
    let changed: Vec<usize> = (0..next.len()).filter(|&i| prev.get(i) != Some(&next[i])).collect();
    let from: Vec<Band> = changed
        .iter()
        .map(|&i| match prev.get(i) {
            Some(p) if p.kind == next[i].kind => *p,
            _ => neutral(&next[i]),
        })
        .collect();
    let to: Vec<Band> = changed.iter().map(|&i| next[i]).collect();
    let at = |t: f64| {
        let mut out = next.to_vec();
        for (k, &i) in changed.iter().enumerate() {
            out[i] = blend(&from[k], &to[k], t);
        }
        out
    };
    if changed.is_empty() || over(&at(0.0)) {
        // Already over before the gesture (or nothing to move back): scale the whole list.
        return scale_down(next);
    }
    let t = bisect(|t| !over(&at(t)));
    next.copy_from_slice(&at(t));
    true
}

/// Scales every boost of the list (gains above 0, pass-filter resonance) down until the list is within the
/// limit. True when it had to.
pub fn scale_down(bands: &mut [Band]) -> bool {
    if !over(bands) {
        return false;
    }
    let orig = bands.to_vec();
    let at = |s: f64| -> Vec<Band> {
        orig.iter()
            .map(|b| {
                let mut b = *b;
                if b.kind.has_gain() {
                    if b.gain > 0.0 {
                        b.gain *= s;
                    }
                } else if b.q > FLAT_Q {
                    b.q = FLAT_Q * (b.q / FLAT_Q).powf(s);
                }
                b
            })
            .collect()
    };
    let s = bisect(|s| !over(&at(s)));
    bands.copy_from_slice(&at(s));
    true
}

/// [`limit_edit`] on every list that makes a sound.
pub fn limit_profile_edit(prev: &ProfileBands, next: &mut ProfileBands) -> bool {
    let mut hit = false;
    for &t in checked(next) {
        hit |= limit_edit(prev.get(t), next.get_mut(t));
    }
    hit
}

/// A profile loaded from the speaker or a file: every list that makes a sound is scaled down if needed.
pub fn fit(p: &mut ProfileBands) -> bool {
    let mut hit = false;
    for &t in checked(p) {
        hit |= scale_down(p.get_mut(t));
    }
    hit
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::BandType;

    fn bell(f: f64, g: f64, q: f64) -> Band {
        Band::new(BandType::Peak, f, g, q)
    }

    #[test]
    fn two_bells_at_the_same_frequency_are_clamped() {
        let prev = vec![bell(1000.0, 12.0, 4.0), bell(1000.0, 0.0, 4.0)];
        let mut next = prev.clone();
        next[1].gain = 12.0; // the drag of the second bell
        assert!(peak(&next) > 23.0);
        assert!(limit_edit(&prev, &mut next));
        assert!((peak(&next) - LIMIT).abs() < 0.1, "at the limit: {}", peak(&next));
        assert_eq!(next[0], prev[0], "only the dragged band moves");
        assert!((next[1].gain - 3.0).abs() < 0.2, "{}", next[1].gain);
    }

    #[test]
    fn stacked_high_q_low_passes_are_clamped() {
        let lp = |q| Band::new(BandType::LowPass, 2000.0, 0.0, q);
        let prev = vec![lp(12.0), lp(0.707)];
        let mut next = vec![lp(12.0), lp(12.0)];
        assert!(peak(&next) > 40.0);
        assert!(limit_edit(&prev, &mut next));
        assert!(peak(&next) <= LIMIT + SLACK);
        // A single resonant low-pass over the limit on its own, added in one go: brought back too.
        let mut added = vec![lp(12.0)];
        assert!(limit_edit(&[], &mut added));
        assert!(peak(&added) <= LIMIT + SLACK);
    }

    #[test]
    fn cuts_are_unaffected() {
        let prev = vec![bell(100.0, -12.0, 8.0)];
        let mut next = vec![bell(100.0, -12.0, 8.0), bell(100.0, -12.0, 8.0), bell(5000.0, -15.0, 12.0)];
        let before = next.clone();
        assert!(!limit_edit(&prev, &mut next));
        assert_eq!(next, before);
        let mut cut = next.clone();
        assert!(!scale_down(&mut cut));
    }

    #[test]
    fn profiles_over_the_limit_are_scaled_on_load() {
        let mut p = ProfileBands::flat();
        p.get_mut(Target::Global).extend([bell(300.0, 12.0, 6.0), bell(300.0, 12.0, 6.0), bell(4000.0, -6.0, 1.0)]);
        assert!(fit(&mut p));
        let g = p.get(Target::Global);
        assert!(peak(g) <= LIMIT + SLACK);
        assert_eq!(g[2].gain, -6.0, "cuts are kept");
        assert!(!fit(&mut p), "once is enough");
    }
}
