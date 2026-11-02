//! The OS volume and mute are the only master volume (`firmware/dsp/README.md`, "Volume, pod and the TAS
//! protocol"): the OS sends UAC SET_CUR to the speaker, and the TAS reports the current Feature Unit values in
//! its `INFO` reply, which is what the app shows. Setting them from the app must therefore go through the OS
//! mixer, so that the OS slider, the speaker and the app stay in step.

/// Lowest Feature Unit volume (dB): MIN 0x8100 = −127 dB.
pub const MIN_DB: f32 = -127.0;

/// Slider position (0–100) of a UAC volume, linear in dB over −127..0 dB: the law the TAS uses to turn the
/// pod knob into the Feature Unit volume (`−(127 − knob × 127 / 99)` dB).
pub fn percent_from_db(db: f32) -> u8 {
    ((db - MIN_DB) / -MIN_DB * 100.0).round().clamp(0.0, 100.0) as u8
}

pub fn db_from_percent(p: u8) -> f32 {
    MIN_DB + f32::from(p.min(100)) / 100.0 * -MIN_DB
}

/// A value the user just set, shown for [`HOLD_S`] whatever the speaker reports (no jump while dragging).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Pending<T> {
    pub value: T,
    pub until: f64,
}

/// How long a value set in the app is shown before the speaker's value wins again (s).
pub const HOLD_S: f64 = 1.5;

impl<T: Copy> Pending<T> {
    pub fn new(value: T, now: f64) -> Self {
        Self { value, until: now + HOLD_S }
    }

    pub fn shown(p: Option<Self>, device: T, now: f64) -> T {
        match p {
            Some(x) if now < x.until => x.value,
            _ => device,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn volume_law_matches_the_tas() {
        assert_eq!(percent_from_db(0.0), 100);
        assert_eq!(percent_from_db(-127.0), 0);
        assert_eq!(percent_from_db(-200.0), 0);
        assert_eq!(percent_from_db(-63.5), 50);
        for p in [0u8, 1, 37, 99, 100] {
            assert_eq!(percent_from_db(db_from_percent(p)), p);
        }
    }

    #[test]
    fn pending_values_hold_then_follow_the_device() {
        let p = Some(Pending::new(40u8, 10.0));
        assert_eq!(Pending::shown(p, 20, 10.5), 40);
        assert_eq!(Pending::shown(p, 20, 12.0), 20);
        assert_eq!(Pending::shown(None, 20, 0.0), 20);
    }
}
