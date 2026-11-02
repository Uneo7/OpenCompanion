use crate::i18n::{t, tr};
use crate::model::{format, BandType, StreamFormat, Target};

fn decimal(s: String) -> String {
    let mark = tr("fmt.decimal");
    if mark == "," {
        s
    } else {
        s.replace(',', mark)
    }
}

/// `n` with `d` decimals and a true minus sign.
pub fn fr(n: f64, d: usize) -> String {
    decimal(format::fr(n, d))
}

pub fn frs(n: f64, d: usize) -> String {
    decimal(format::frs(n, d))
}

/// "140 Hz", "2,90 kHz".
pub fn freq(f: f64) -> String {
    decimal(format::freq(f))
}

pub fn freq_parts(f: f64) -> (String, &'static str) {
    if f >= 1000.0 {
        (fr(f / 1000.0, if f >= 10_000.0 { 1 } else { 2 }), "kHz")
    } else {
        (format!("{}", f.round()), "Hz")
    }
}

pub fn volume_db(v: u8) -> String {
    decimal(format::volume_db(v))
}

/// A level reported by the OS mixer: "−20,5 dB".
pub fn level_db(db: f32) -> String {
    format!("{} dB", fr(f64::from(db), 1))
}

pub fn type_key(k: BandType) -> &'static str {
    match k {
        BandType::Peak => "eq.type.peak",
        BandType::LowShelf => "eq.type.lowshelf",
        BandType::HighShelf => "eq.type.highshelf",
        BandType::HighPass => "eq.type.hp",
        BandType::LowPass => "eq.type.lp",
        BandType::Notch => "eq.type.notch",
    }
}

pub fn tr_type(k: BandType) -> &'static str {
    tr(type_key(k))
}

pub fn type_help(k: BandType) -> String {
    t(&format!("{}.help", type_key(k)))
}

pub fn target_key(t: Target) -> &'static str {
    match t {
        Target::Global => "eq.target.global",
        Target::Sub => "eq.target.sub",
        Target::InnerLeft => "eq.target.il",
        Target::InnerRight => "eq.target.ir",
        Target::OuterLeft => "eq.target.ol",
        Target::OuterRight => "eq.target.or",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parts_and_keys() {
        assert_eq!(freq_parts(140.0), ("140".to_owned(), "Hz"));
        assert_eq!(type_key(BandType::Notch), "eq.type.notch");
        assert_eq!(target_key(Target::OuterRight), "eq.target.or");
        // Every key exists in the French table.
        for k in BandType::ALL {
            assert!(crate::i18n::lookup(crate::i18n::Lang::Fr, type_key(k)).is_some());
        }
        for x in Target::ALL {
            assert!(crate::i18n::lookup(crate::i18n::Lang::Fr, target_key(x)).is_some());
        }
    }
}

/// The USB mode as shown in Settings › Device and Status: "5.1 · 16 bits · 48 kHz" / "2.1 · 24 bits · 48 kHz".
pub fn mode(m: crate::model::DeviceMode) -> &'static str {
    tr(match m {
        crate::model::DeviceMode::Surround51 => "settings.device.mode.51",
        crate::model::DeviceMode::Hq21 => "settings.device.mode.21",
    })
}

/// The stream the computer sends, e.g. "5.1 · 16 bits · 48 kHz" (48 kHz only).
pub fn stream(s: StreamFormat) -> String {
    let ch = match s {
        StreamFormat::Surround51 => "settings.device.rx.51",
        StreamFormat::Hq21 => "settings.device.rx.21",
        StreamFormat::Stereo => "settings.device.rx.stereo",
        StreamFormat::Idle | StreamFormat::Unknown(_) => return tr("settings.device.rx.none").to_owned(),
    };
    let bits = s.bits().unwrap_or(24);
    crate::i18n::t_with("settings.device.rx.fmt", &[("ch", &tr(ch)), ("bits", &bits)])
}
