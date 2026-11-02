//! Replies are `KEY:value` fields separated by spaces or line breaks; values may be comma lists.

use super::Dsp;
use crate::error::{Error, Result};

pub fn field<'a>(reply: &'a str, key: &str) -> Option<&'a str> {
    reply.split_whitespace().find_map(|tok| tok.strip_prefix(key)?.strip_prefix(':'))
}

fn int(reply: &str, key: &str) -> Option<i32> {
    field(reply, key)?.parse().ok()
}

/// `SVM`: `D<dsp version>U<tas version>`.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct Versions {
    pub dsp: String,
    pub tas: String,
}

impl Versions {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().strip_prefix('D')?;
        let (dsp, tas) = s.split_once('U').unwrap_or((s, ""));
        Some(Self { dsp: dsp.trim().to_owned(), tas: tas.trim().to_owned() })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct BootStatus {
    pub state: u8,
    /// 0 none, 1 busy, 2 OK, 3 I/O, 4 verify, 5 refused (no OpenCompanion header).
    pub result: u8,
    /// 0 waiting, 1 no header, 2 not a trial image, 3 already confirmed, 4 confirmation written, 5 skipped
    /// (RAM-loaded), 6 error.
    pub trial: u8,
    /// Started by boot1 / ROM (cold boot) rather than by a RAM load.
    pub cold: bool,
}

impl BootStatus {
    pub fn parse(s: &str) -> Option<Self> {
        Some(Self {
            state: int(s, "BS")? as u8,
            result: int(s, "R")? as u8,
            trial: int(s, "T")? as u8,
            cold: int(s, "C")? != 0,
        })
    }

    pub fn trial_text(&self) -> &'static str {
        match self.trial {
            0 => "waiting for confirmation",
            1 => "no image header",
            2 => "not a trial image",
            3 => "confirmed",
            4 => "confirmation written",
            5 => "RAM-loaded (not confirmed)",
            6 => "confirmation error",
            _ => "unknown",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct PodStatus {
    pub present: bool,
    /// Raw knob after the stock hysteresis, % × 10.
    pub knob_permille: i32,
    /// LOCAL mode (no USB host: the knob sets the volume).
    pub local: bool,
    /// Knob as reported in `S` (%).
    pub reported: i32,
    pub events: i32,
    /// Master volume before the ceiling, dB × 10.
    pub master_db10: i32,
    /// Loudness boost, dB × 10.
    pub loudness_db10: i32,
}

impl PodStatus {
    pub fn parse(s: &str) -> Option<Self> {
        Some(Self {
            present: int(s, "PR")? != 0,
            knob_permille: int(s, "KN").unwrap_or(0),
            local: field(s, "MD") == Some("L"),
            reported: int(s, "RK").unwrap_or(0),
            events: int(s, "EV").unwrap_or(0),
            master_db10: int(s, "DB").unwrap_or(0),
            loudness_db10: int(s, "LB").unwrap_or(0),
        })
    }
}

/// DSP profile protocol 5; replaced `VRW` and `GPI`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct ProtectStatus {
    pub fault: bool,
    pub vraw_mv: i32,
    pub hot: (bool, bool),
    /// Frames with CCLIP (satellite amplifiers) / BSCLIP (bass amplifier) since the DSP started.
    pub cclip: u32,
    pub bsclip: u32,
    /// Lowest compressor / look-ahead limiter gain, dB × 10.
    pub comp_db10: i32,
    pub lim_db10: i32,
}

impl ProtectStatus {
    pub fn parse(s: &str) -> Option<Self> {
        let n = |k: &str| int(s, k).unwrap_or(0).max(0) as u32;
        let mut t = field(s, "T").unwrap_or("").chars().map(|c| c == '1');
        let hot = (t.next().unwrap_or(false), t.next().unwrap_or(false));
        Some(Self {
            fault: int(s, "FT")? != 0,
            vraw_mv: int(s, "VR").unwrap_or(0),
            hot,
            cclip: n("CC"),
            bsclip: n("BC"),
            comp_db10: int(s, "CG").unwrap_or(0),
            lim_db10: int(s, "LG").unwrap_or(0),
        })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SysStatus {
    pub frames: u32,
    pub clips: u32,
}

impl SysStatus {
    pub fn parse(s: &str) -> Option<Self> {
        Some(Self { frames: int(s, "FR")?.max(0) as u32, clips: int(s, "CL").unwrap_or(0).max(0) as u32 })
    }
}

impl Dsp<'_> {
    /// Reply without the prompt (`\r` turned into line breaks).
    pub fn text(&self, cmd: &str) -> Result<String> {
        Ok(self.console().text(cmd)?)
    }

    fn text_parsed<T>(&self, cmd: &'static str, parse: impl Fn(&str) -> Option<T>) -> Result<T> {
        let s = self.text(cmd)?;
        parse(&s).ok_or(Error::Reply { cmd, reply: s })
    }

    pub fn versions(&self) -> Result<Versions> {
        self.text_parsed("SVM", Versions::parse)
    }

    pub fn boot_status(&self) -> Result<BootStatus> {
        self.text_parsed("BST", BootStatus::parse)
    }

    pub fn pod(&self) -> Result<PodStatus> {
        self.text_parsed("POD", PodStatus::parse)
    }

    pub fn protect(&self) -> Result<ProtectStatus> {
        self.text_parsed("PRT", ProtectStatus::parse)
    }

    pub fn sys(&self) -> Result<SysStatus> {
        self.text_parsed("SYS", SysStatus::parse)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn replies_parse() {
        assert_eq!(
            Versions::parse("DOC0.0.1UOC-0.1.0"),
            Some(Versions { dsp: "OC0.0.1".into(), tas: "OC-0.1.0".into() })
        );
        assert_eq!(Versions::parse("D01.01.01U01.01.03").unwrap().tas, "01.01.03");
        let b = BootStatus::parse("BS:0 R:2 T:5 C:0").unwrap();
        assert_eq!((b.result, b.trial, b.cold), (2, 5, false));
        let p = PodStatus::parse("PR:1 KN:215 MD:H RK:21 EV:3 DB:-235 LB:12").unwrap();
        assert!(p.present && !p.local);
        assert_eq!((p.knob_permille, p.master_db10), (215, -235));
        let pr = ProtectStatus::parse("FT:0 VR:13410 T:01 CC:3 BC:0 CG:-12 LG:0").unwrap();
        assert_eq!((pr.fault, pr.vraw_mv, pr.hot, pr.cclip, pr.comp_db10), (false, 13410, (false, true), 3, -12));
        let s = SysStatus::parse("FR:1234\nCL:2\nVOL:-20\nMU:0\nPK:-40/-41").unwrap();
        assert_eq!((s.frames, s.clips), (1234, 2));
        assert!(BootStatus::parse("garbage").is_none());
    }
}
