use std::time::{SystemTime, UNIX_EPOCH};

/// Proleptic Gregorian, H. Hinnant's algorithm.
fn civil(days: i64) -> (i64, u32, u32) {
    let z = days + 719_468;
    let era = z.div_euclid(146_097);
    let doe = z.rem_euclid(146_097);
    let yoe = (doe - doe / 1460 + doe / 36_524 - doe / 146_096) / 365;
    let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
    let mp = (5 * doy + 2) / 153;
    let d = (doy - (153 * mp + 2) / 5 + 1) as u32;
    let m = if mp < 10 { mp + 3 } else { mp - 9 } as u32;
    (yoe + era * 400 + i64::from(m <= 2), m, d)
}

pub fn stamp_at(secs: i64) -> String {
    let (y, m, d) = civil(secs.div_euclid(86_400));
    let t = secs.rem_euclid(86_400);
    format!("{y:04}-{m:02}-{d:02}_{:02}{:02}{:02}", t / 3600, t / 60 % 60, t % 60)
}

pub fn stamp_now() -> String {
    stamp_at(SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs() as i64))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stamps() {
        assert_eq!(stamp_at(0), "1970-01-01_000000");
        assert_eq!(stamp_at(20_758 * 86_400 + 3600 * 14 + 61), "2026-11-01_140101");
        assert_eq!(stamp_at(11_016 * 86_400), "2000-02-29_000000");
        assert_eq!(stamp_now().len(), 17);
    }
}
