pub fn fr(n: f64, d: usize) -> String {
    let s = format!("{:.*}", d, n.abs()).replace('.', ",");
    // "-0,0" is printed as "0,0".
    let negative = n < 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0');
    if negative {
        format!("−{s}")
    } else {
        s
    }
}

pub fn frs(n: f64, d: usize) -> String {
    let s = fr(n, d);
    if n > 0.0 && s.chars().any(|c| c.is_ascii_digit() && c != '0') {
        format!("+{s}")
    } else {
        s
    }
}

/// Frequency as shown in the band panel: "140 Hz", "2,90 kHz", "11,0 kHz".
pub fn freq(f: f64) -> String {
    if f >= 1000.0 {
        format!("{} kHz", fr(f / 1000.0, if f >= 10_000.0 { 1 } else { 2 }))
    } else {
        format!("{} Hz", f.round())
    }
}

pub fn volume_db(v: u8) -> String {
    if v == 0 {
        "−∞ dB".into()
    } else {
        format!("{} dB", fr(-60.0 + f64::from(v) * 0.93, 1))
    }
}

/// Parses a typed value: comma or point, "−" or "-", and a "k" suffix for kilohertz ("2,5k" = 2500).
pub fn parse_num(s: &str) -> Option<f64> {
    let s = s.trim().replace('−', "-").replace(',', ".");
    let kilo = s.to_ascii_lowercase().contains('k');
    let start = s.find(|c: char| c.is_ascii_digit() || c == '-' || c == '+' || c == '.')?;
    let rest = &s[start..];
    let end = rest
        .char_indices()
        .skip(1)
        .find(|(_, c)| !(c.is_ascii_digit() || *c == '.'))
        .map_or(rest.len(), |(i, _)| i);
    let v: f64 = rest[..end].parse().ok()?;
    Some(if kilo { v * 1000.0 } else { v })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_are_shown_without_prefix_and_stamp() {
        assert_eq!(display_version("OC0.1.0+20261101183911"), "0.1.0");
        assert_eq!(display_version("OC-0.4.1"), "0.4.1");
        assert_eq!(display_version("0.4.1"), "0.4.1");
    }

    #[test]
    fn french_numbers() {
        assert_eq!(fr(-20.94, 1), "−20,9");
        assert_eq!(fr(2.2, 2), "2,20");
        assert_eq!(fr(-0.01, 1), "0,0");
        assert_eq!(frs(1.5, 1), "+1,5");
        assert_eq!(frs(0.0, 1), "0,0");
        assert_eq!(frs(-1.5, 1), "−1,5");
    }

    #[test]
    fn frequencies() {
        assert_eq!(freq(140.0), "140 Hz");
        assert_eq!(freq(2900.0), "2,90 kHz");
        assert_eq!(freq(11_000.0), "11,0 kHz");
    }

    #[test]
    fn volume_levels() {
        assert_eq!(volume_db(42), "−20,9 dB");
        assert_eq!(volume_db(0), "−∞ dB");
    }

    #[test]
    fn parsing() {
        assert_eq!(parse_num("2,5k"), Some(2500.0));
        assert_eq!(parse_num("−1,5 dB"), Some(-1.5));
        assert_eq!(parse_num(" 140 Hz"), Some(140.0));
        assert_eq!(parse_num("2.90 kHz"), Some(2900.0));
        assert_eq!(parse_num("abc"), None);
    }
}

/// A version as shown: `0.1.0` for `OC0.1.0+20261101183911`, `OC-0.4.1` or `0.4.1` (the build stamp stays in
/// the logs).
pub fn display_version(v: &str) -> &str {
    let v = v.trim();
    let v = v.strip_prefix("OC-").or_else(|| v.strip_prefix("OC")).unwrap_or(v);
    v.split('+').next().unwrap_or(v)
}

/// A firmware version as shown: the version, "Version Bose®" for the stock firmware, "inconnue" when our
/// firmware runs but its version could not be read.
pub fn version_label(v: &Option<String>, stock: bool) -> String {
    from_label(v, stock, false)
}

/// [`version_label`] of a running version that may be "probably Bose" (garbled reply).
pub fn from_label(v: &Option<String>, stock: bool, probable: bool) -> String {
    use oc_device::install::versions::{label_of, VersionLabel};
    match label_of(v, stock, probable) {
        VersionLabel::ProbablyStock => crate::i18n::tr("inst.ver.stock_probable").to_owned(),
        VersionLabel::Version(s) => display_version(s).to_owned(),
        VersionLabel::Stock => crate::i18n::tr("inst.ver.stock").to_owned(),
        VersionLabel::Unknown => crate::i18n::tr("settings.firmware.version_unknown").to_owned(),
    }
}
