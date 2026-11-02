use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::layout::{BOOT0, BOOT1, EE_PAGE, HDR, LAST_PAGE};
use oc_ais::MAGIC;

use super::image::DspRestoreInputs;
use crate::dsp::bundle::pages;
use crate::dsp::survey::spans;
use crate::dsp::Updater;
use crate::error::Result;

/// AIS Section Load opcode: second word of our boot0.
const OP_SECTION_LOAD: u32 = 0x5853_5901;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boot0Kind {
    Stock,
    OpenCompanion,
    /// Interrupted critical window: the DSP cannot boot until boot0 is rewritten.
    Unknown,
}

#[derive(Debug, Clone)]
pub struct RestoreSurvey {
    pub boot0: Boot0Kind,
    pub oc_header: bool,
    /// boot1's CRC-32 at 0xFF7C; only read when boot0 is not stock.
    pub boot1_ok: Option<bool>,
    pub low_bad: Vec<(usize, usize)>,
    pub high_bad: Vec<(usize, usize)>,
    /// The last page is never writable.
    pub last_page_ok: bool,
}

impl RestoreSurvey {
    pub fn is_stock(&self) -> bool {
        self.boot0 == Boot0Kind::Stock && self.low_bad.is_empty() && self.high_bad.is_empty() && self.last_page_ok
    }
}

fn bad_pages(u: &Updater, img: &DspRestoreInputs, lo: usize, hi: usize) -> Result<Vec<(usize, usize)>> {
    let mut bad = Vec::new();
    for (a, n) in spans(lo, hi) {
        if u.ee_crc(a, n)? == crc32fast::hash(img.at(a, n)) {
            continue;
        }
        let inside = pages(a, n);
        if inside.len() == 1 {
            bad.push((a, n));
            continue;
        }
        for (p, m) in inside {
            if u.ee_crc(p, m)? != crc32fast::hash(img.at(p, m)) {
                bad.push((p, m));
            }
        }
    }
    Ok(bad)
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

pub fn survey(u: &Updater, img: &DspRestoreInputs) -> Result<RestoreSurvey> {
    let b0 = u.ee_read(BOOT0.start, BOOT0.len())?;
    let boot0 = if b0 == img.at(BOOT0.start, BOOT0.len()) {
        Boot0Kind::Stock
    } else if le32(&b0, 0) == MAGIC && le32(&b0, 4) == OP_SECTION_LOAD {
        Boot0Kind::OpenCompanion
    } else {
        Boot0Kind::Unknown
    };
    let oc_header = ImageHeader::parse(&u.ee_read(HDR, EE_PAGE)?).is_ok();
    let boot1_ok = if boot0 == Boot0Kind::Stock {
        None
    } else {
        let area = u.ee_read(BOOT1.start, BOOT1.len())?;
        let n = area.len() - 4;
        Some(crc32fast::hash(&area[..n]) == le32(&area, n))
    };
    let low_bad = bad_pages(u, img, HDR, BOOT1.start)?;
    let high_bad = bad_pages(u, img, BOOT1.start, BOOT1.end)?;
    let last = img.at(LAST_PAGE.start, LAST_PAGE.len());
    let last_page_ok = u.ee_crc(LAST_PAGE.start, LAST_PAGE.len())? == crc32fast::hash(last);
    Ok(RestoreSurvey { boot0, oc_header, boot1_ok, low_bad, high_bad, last_page_ok })
}
