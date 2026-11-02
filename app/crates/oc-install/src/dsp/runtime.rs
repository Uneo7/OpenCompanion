//! `SVM\r` is asked first: the RAM loader, rescue and the updater answer `15` to it. Only then is the loader
//! `INFO` (`00`) sent, never to a console that would not understand it.

use std::fmt;
use std::time::Duration;

use oc_protocol::Link;

const TIMEOUT: Duration = Duration::from_millis(1000);
pub const STOCK_DSP_VERSION: &str = "01.01.01";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DspRuntime {
    Stock { svm: String },
    OcFirmware { version: String },
    OtherFirmware { svm: String },
    Stub { version: u8 },
    Rescue { reason: u8, boot1: u8 },
    Updater { boot1: u8 },
    Silent(String),
}

pub fn reason_text(r: u8) -> &'static str {
    match r {
        0 => "none",
        1 => "software request (RSC)",
        2 => "TAS request",
        3 => "no image header",
        4 => "bad image header",
        5 => "bad image body",
        6 => "image body CRC mismatch",
        7 => "EEPROM I/O error",
        8 => "trial exhausted",
        9 => "updater",
        _ => "unknown",
    }
}

impl fmt::Display for DspRuntime {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Stock { svm } => write!(f, "stock Bose DSP firmware ({svm})"),
            Self::OcFirmware { version } => write!(f, "OpenCompanion DSP firmware {version}"),
            Self::OtherFirmware { svm } => write!(f, "unknown DSP firmware ({svm})"),
            Self::Stub { version } => write!(f, "RAM loader stub v{version} (stock firmware hijacked)"),
            Self::Rescue { reason, boot1 } => {
                write!(f, "OpenCompanion rescue mode (boot1 v{boot1}), reason {reason}: {}", reason_text(*reason))
            }
            Self::Updater { boot1 } => write!(f, "OpenCompanion updater (boot1 protocol v{boot1}) running from RAM"),
            Self::Silent(e) => write!(f, "no answer from the DSP ({e})"),
        }
    }
}

fn info(link: &dyn Link) -> DspRuntime {
    match link.transact(b'E', &[0], TIMEOUT) {
        Ok((_, d)) if d.len() >= 5 && d[0] == 0x06 && &d[1..4] == b"OCL" => match (d.len(), d.get(5)) {
            (5, _) => DspRuntime::Stub { version: d[4] },
            (8.., Some(b'R')) => DspRuntime::Rescue { reason: d[6], boot1: d[7] },
            (8.., Some(b'U')) => DspRuntime::Updater { boot1: d[7] },
            _ => DspRuntime::Silent(format!("unexpected INFO reply {d:02x?}")),
        },
        Ok((_, d)) => DspRuntime::Silent(format!("unexpected INFO reply {d:02x?}")),
        Err(e) => DspRuntime::Silent(e.to_string()),
    }
}

fn from_svm(text: &str) -> DspRuntime {
    let t = text.trim_end_matches("\r>").trim_end_matches('\r');
    let version = t.strip_prefix('D').map(|r| r.split('U').next().unwrap_or(""));
    match version {
        Some(v) if v == STOCK_DSP_VERSION => DspRuntime::Stock { svm: t.replace('\r', " ") },
        Some(v) if v.starts_with("OC") => DspRuntime::OcFirmware { version: v.to_string() },
        _ => DspRuntime::OtherFirmware { svm: t.replace('\r', " ") },
    }
}

pub fn detect(link: &dyn Link) -> DspRuntime {
    let mut last = String::from("no attempt");
    for _ in 0..3 {
        match link.transact(b'E', b"SVM\r", TIMEOUT) {
            Ok((_, d)) if d.starts_with(b"ERROR") => last = String::from_utf8_lossy(&d).into_owned(),
            Ok((_, d)) if matches!(d.first(), Some(0x06 | 0x15)) => return info(link),
            Ok((_, d)) if d.first() == Some(&b'D') => return from_svm(&String::from_utf8_lossy(&d)),
            Ok((_, d)) => last = format!("unexpected SVM reply {d:02x?}"),
            Err(e) => last = e.to_string(),
        }
        link.delay(Duration::from_millis(300));
    }
    DspRuntime::Silent(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn svm_parsing() {
        assert!(matches!(from_svm("D01.01.01U01.01.03\r>"), DspRuntime::Stock { .. }));
        assert_eq!(from_svm("DOC0.0.1UOC-0.1.0\r>"), DspRuntime::OcFirmware { version: "OC0.0.1".into() });
        assert!(matches!(from_svm("D02.00.00U01\r>"), DspRuntime::OtherFirmware { .. }));
    }
}
