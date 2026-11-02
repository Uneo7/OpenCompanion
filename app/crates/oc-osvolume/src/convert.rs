//! Only needed where the OS gives raw dB (ALSA). Same mapping as `alsamixer` (alsa-utils `volume_mapping.c`),
//! so both sliders agree: linear in dB for small ranges, else `10^((dB - max) / 60)` rescaled so `min` maps to 0.

pub const MAX_LINEAR_DB_SCALE: f32 = 24.0;

pub fn clamp_scalar(scalar: f32) -> f32 {
    if scalar.is_nan() {
        0.0
    } else {
        scalar.clamp(0.0, 1.0)
    }
}

/// `min_db_is_mute`: the bottom of the range means silence (ALSA `SND_CTL_TLV_DB_GAIN_MUTE`), no rescaling.
pub fn db_to_scalar(db: f32, min_db: f32, max_db: f32, min_db_is_mute: bool) -> f32 {
    // Also rejects NaN bounds.
    if max_db.partial_cmp(&min_db) != Some(std::cmp::Ordering::Greater) {
        return 0.0;
    }
    let db = db.clamp(min_db, max_db);
    if max_db - min_db <= MAX_LINEAR_DB_SCALE {
        return (db - min_db) / (max_db - min_db);
    }
    let norm = exp10((db - max_db) / 60.0);
    let scalar = if min_db_is_mute {
        norm
    } else {
        let min_norm = exp10((min_db - max_db) / 60.0);
        (norm - min_norm) / (1.0 - min_norm)
    };
    clamp_scalar(scalar)
}

pub fn scalar_to_db(scalar: f32, min_db: f32, max_db: f32, min_db_is_mute: bool) -> f32 {
    // Also rejects NaN bounds.
    if max_db.partial_cmp(&min_db) != Some(std::cmp::Ordering::Greater) {
        return min_db;
    }
    let scalar = clamp_scalar(scalar);
    if max_db - min_db <= MAX_LINEAR_DB_SCALE {
        return min_db + scalar * (max_db - min_db);
    }
    let norm = if min_db_is_mute {
        scalar
    } else {
        let min_norm = exp10((min_db - max_db) / 60.0);
        scalar * (1.0 - min_norm) + min_norm
    };
    if norm <= 0.0 {
        return min_db;
    }
    (60.0 * norm.log10() + max_db).clamp(min_db, max_db)
}

/// For controls without dB info.
pub fn raw_to_scalar(raw: i64, min: i64, max: i64) -> f32 {
    if max <= min {
        return 0.0;
    }
    clamp_scalar((raw - min) as f32 / (max - min) as f32)
}

/// Rounded to the nearest step.
pub fn scalar_to_raw(scalar: f32, min: i64, max: i64) -> i64 {
    if max <= min {
        return min;
    }
    min + (clamp_scalar(scalar) as f64 * (max - min) as f64).round() as i64
}

fn exp10(x: f32) -> f32 {
    10f32.powf(x)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn close(a: f32, b: f32, eps: f32) -> bool {
        (a - b).abs() <= eps
    }

    #[test]
    fn clamp() {
        assert_eq!(clamp_scalar(-1.0), 0.0);
        assert_eq!(clamp_scalar(2.0), 1.0);
        assert_eq!(clamp_scalar(f32::NAN), 0.0);
        assert_eq!(clamp_scalar(0.25), 0.25);
    }

    #[test]
    fn endpoints() {
        for &(min, max) in &[(-127.0, 0.0), (-60.0, 0.0), (-20.0, 0.0), (-100.0, 6.0)] {
            assert!(close(db_to_scalar(min, min, max, false), 0.0, 1e-6));
            assert!(close(db_to_scalar(max, min, max, false), 1.0, 1e-6));
            assert!(close(scalar_to_db(0.0, min, max, false), min, 1e-3));
            assert!(close(scalar_to_db(1.0, min, max, false), max, 1e-3));
        }
    }

    #[test]
    fn round_trip() {
        for &(min, max, mute) in &[
            (-127.0, 0.0, false),
            (-127.0, 0.0, true),
            (-20.0, 0.0, false),
            (-64.0, 6.0, false),
        ] {
            for i in 0..=100 {
                let s = i as f32 / 100.0;
                let db = scalar_to_db(s, min, max, mute);
                assert!(db >= min && db <= max, "{db} out of range");
                if !(mute && i == 0) {
                    let back = db_to_scalar(db, min, max, mute);
                    assert!(close(back, s, 1e-4), "{min}..{max} {s} -> {db} -> {back}");
                }
            }
        }
    }

    #[test]
    fn small_range_is_linear_in_db() {
        assert!(close(db_to_scalar(-10.0, -20.0, 0.0, false), 0.5, 1e-6));
        assert!(close(scalar_to_db(0.25, -20.0, 0.0, false), -15.0, 1e-5));
    }

    #[test]
    fn large_range_matches_alsamixer_curve() {
        // -20 dB below max on a -127..0 range: 10^(-1/3) rescaled.
        let min_norm = 10f32.powf(-127.0 / 60.0);
        let expect = (10f32.powf(-20.0 / 60.0) - min_norm) / (1.0 - min_norm);
        assert!(close(db_to_scalar(-20.0, -127.0, 0.0, false), expect, 1e-6));
        let mut prev = -1.0;
        for db in (-127..=0).map(|d| d as f32) {
            let s = db_to_scalar(db, -127.0, 0.0, false);
            assert!(s > prev);
            prev = s;
        }
    }

    #[test]
    fn degenerate_ranges() {
        assert_eq!(db_to_scalar(0.0, 0.0, 0.0, false), 0.0);
        assert_eq!(scalar_to_db(0.5, -3.0, -3.0, false), -3.0);
        assert_eq!(raw_to_scalar(5, 10, 10), 0.0);
        assert_eq!(scalar_to_raw(0.5, 10, 10), 10);
    }

    #[test]
    fn raw() {
        assert_eq!(raw_to_scalar(0, -100, 100), 0.5);
        assert_eq!(raw_to_scalar(-200, -100, 100), 0.0);
        assert_eq!(scalar_to_raw(0.5, 0, 255), 128);
        assert_eq!(scalar_to_raw(1.5, 0, 255), 255);
        for raw in 0..=255 {
            assert_eq!(scalar_to_raw(raw_to_scalar(raw, 0, 255), 0, 255), raw);
        }
    }
}
