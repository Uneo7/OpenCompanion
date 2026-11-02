//! A chip running our firmware whose version cannot be read is "unknown", never "the Bose firmware". A DSP whose
//! reply comes back garbled through the relay (TAS 0.4.9 with the stock DSP) is "probably Bose".

use std::time::Duration;

use oc_install::dsp::{detect, DspRuntime};

use crate::link::Conn;

const READ_TRIES: usize = 3;
const READ_PAUSE: Duration = Duration::from_millis(300);

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum Running {
    #[default]
    Unknown,
    Stock,
    /// The relay returned a frame, unreadable: most likely the Bose firmware, not certain.
    ProbablyStock,
    Ours(Option<String>),
    Other(String),
}

impl Running {
    pub fn version(&self) -> Option<String> {
        match self {
            Running::Ours(v) => v.clone(),
            Running::Other(s) => Some(s.clone()),
            Running::Unknown | Running::Stock | Running::ProbablyStock => None,
        }
    }

    pub fn is_stock(&self) -> bool {
        *self == Running::Stock
    }

    pub fn is_probably_stock(&self) -> bool {
        *self == Running::ProbablyStock
    }

    pub fn or(self, known: &Running) -> Running {
        match (&self, known) {
            (Running::Unknown, k) => k.clone(),
            (Running::Ours(None), Running::Ours(Some(v))) => Running::Ours(Some(v.clone())),
            _ => self,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RunningVersions {
    pub dsp: Running,
    pub usb: Running,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum VersionLabel<'a> {
    Version(&'a str),
    Stock,
    /// No version, a garbled reply: most likely the Bose firmware.
    ProbablyStock,
    Unknown,
}

pub fn label(version: &Option<String>, stock: bool) -> VersionLabel<'_> {
    label_of(version, stock, false)
}

/// [`label`], with `probable`: a garbled reply that most likely comes from the Bose firmware.
pub fn label_of(version: &Option<String>, stock: bool, probable: bool) -> VersionLabel<'_> {
    match (version, stock) {
        (Some(v), _) => VersionLabel::Version(v),
        (None, true) => VersionLabel::Stock,
        (None, false) if probable => VersionLabel::ProbablyStock,
        (None, false) => VersionLabel::Unknown,
    }
}

/// A `Silent` DSP whose relay still returned a frame, unreadable (not a link failure).
pub fn garbled(silent: &str) -> bool {
    silent.starts_with("unexpected SVM reply")
}

fn dsp_of(rt: DspRuntime) -> Running {
    match rt {
        DspRuntime::Stock { .. } => Running::Stock,
        DspRuntime::OcFirmware { version } => Running::Ours(Some(version)),
        DspRuntime::Silent(e) if garbled(&e) => Running::ProbablyStock,
        DspRuntime::Silent(_) => Running::Unknown,
        other => Running::Other(other.to_string()),
    }
}

/// The DSP gave no usable answer (silent, or a garbled reply).
fn unanswered(r: &Running) -> bool {
    matches!(r, Running::Unknown | Running::ProbablyStock)
}

/// The stock USB controller relays only the stock DSP's own protocol: our DSP answers through it, a DSP that
/// does not is the stock one (it cannot be versioned there, but it is known).
fn under_stock_tas(dsp: Running, stock_tas: bool) -> Running {
    if stock_tas && unanswered(&dsp) {
        Running::Stock
    } else {
        dsp
    }
}

/// Waits up to `budget` for the DSP to answer (it reboots after an install), polling every second.
pub fn wait_dsp(conn: &Conn, budget: Duration) -> Running {
    let deadline = std::time::Instant::now() + budget;
    loop {
        let r = dsp_of(conn.with_dsp(detect));
        if !unanswered(&r) || std::time::Instant::now() >= deadline {
            return r;
        }
        std::thread::sleep(Duration::from_secs(1));
    }
}

/// Reads both chips from `conn`, with retries. The USB controller's version comes from its own `INFO`, whatever
/// the DSP answers.
pub fn read(conn: &Conn) -> RunningVersions {
    let mut dsp = Running::Unknown;
    for k in 0..READ_TRIES {
        dsp = dsp_of(conn.with_dsp(detect));
        if !unanswered(&dsp) {
            break;
        }
        if k + 1 < READ_TRIES {
            std::thread::sleep(READ_PAUSE);
        }
    }
    let dsp = under_stock_tas(dsp, matches!(conn, Conn::Stock(_)));
    let usb = match conn {
        Conn::Stock(_) => Running::Stock,
        Conn::Oc(_) => {
            let mut v = None;
            for k in 0..READ_TRIES {
                v = conn.tas_info().and_then(Result::ok).map(|i| i.base.version_string());
                if v.is_some() {
                    break;
                }
                if k + 1 < READ_TRIES {
                    std::thread::sleep(READ_PAUSE);
                }
            }
            Running::Ours(v)
        }
    };
    RunningVersions { dsp, usb }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_is_never_stock() {
        assert_eq!(label(&None, false), VersionLabel::Unknown);
        assert_eq!(label(&None, true), VersionLabel::Stock);
        assert_eq!(label(&Some("0.3.0".into()), false), VersionLabel::Version("0.3.0"));
        let known = Running::Ours(Some("0.3.0".into()));
        assert_eq!(Running::Ours(None).or(&known), known);
        assert_eq!(Running::Unknown.or(&Running::Stock), Running::Stock);
        assert_eq!(Running::Stock.or(&known), Running::Stock);
        assert!(Running::Stock.version().is_none() && Running::Stock.is_stock());
    }

    #[test]
    fn under_the_stock_usb_controller_a_silent_dsp_is_the_stock_one() {
        assert_eq!(under_stock_tas(Running::Unknown, true), Running::Stock);
        assert_eq!(under_stock_tas(Running::ProbablyStock, true), Running::Stock);
        assert_eq!(under_stock_tas(Running::Unknown, false), Running::Unknown);
        let ours = Running::Ours(Some("0.2.3".into()));
        assert_eq!(under_stock_tas(ours.clone(), true), ours);
    }

    #[test]
    fn a_garbled_reply_is_probably_the_bose_firmware() {
        assert_eq!(dsp_of(DspRuntime::Silent("unexpected SVM reply [00, 00]".into())), Running::ProbablyStock);
        assert_eq!(dsp_of(DspRuntime::Silent("timeout".into())), Running::Unknown);
        assert_eq!(label_of(&None, false, true), VersionLabel::ProbablyStock);
        assert_eq!(label_of(&Some("0.4.9".into()), false, true), VersionLabel::Version("0.4.9"));
        assert!(Running::ProbablyStock.version().is_none() && !Running::ProbablyStock.is_stock());
    }
}
