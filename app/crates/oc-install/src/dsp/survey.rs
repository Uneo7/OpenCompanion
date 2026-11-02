use oc_ais::eeprom::header::{BootState, ImageHeader, STATE_LEN};
use oc_ais::eeprom::layout::{BOOT0, BOOT1, EE_PAGE, HDR, LAST_PAGE, PROFILES, STATE, STOCK_IMAGE_END};

use super::bundle::{pages, DspBundle};
use super::updater::{Updater, MAX_EE_CRC};
use crate::error::Result;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Boot0State {
    Stock,
    Ours,
    /// The ROM cannot boot (interrupted M3): must be rewritten before any power loss.
    Broken,
}

#[derive(Debug, Clone)]
pub struct Survey {
    pub boot0: Boot0State,
    /// Only checked while boot0 is stock: 0x0000..0xDAD8 equals the stock backup.
    pub stock_intact: Option<bool>,
    pub boot1_bad: Vec<(usize, usize)>,
    pub header: Option<ImageHeader>,
    pub body_bad: Vec<(usize, usize)>,
    /// Graph data pages are never listed here: they are rewritten as a whole (`graph_bad`).
    pub stale_pages: Vec<(usize, usize)>,
    pub graph_bad: Vec<(usize, usize)>,
    /// Never written by us.
    pub last_page_ok: bool,
    pub prev_seq: u32,
}

impl Survey {
    /// Any sequence number.
    pub fn image_is_target(&self, b: &DspBundle) -> bool {
        self.body_bad.is_empty() && self.header.is_some_and(|h| ImageHeader { seq: 0, ..h } == b.header(0))
    }
}

fn crc_pages(u: &Updater, list: &[(usize, usize)], expected: impl Fn(usize, usize) -> u32) -> Result<Vec<(usize, usize)>> {
    let mut bad = Vec::new();
    for &(a, n) in list {
        if u.ee_crc(a, n)? != expected(a, n) {
            bad.push((a, n));
        }
    }
    Ok(bad)
}

pub(crate) fn spans(lo: usize, hi: usize) -> Vec<(usize, usize)> {
    (lo..hi).step_by(MAX_EE_CRC).map(|a| (a, MAX_EE_CRC.min(hi - a))).collect()
}

pub fn survey(u: &Updater, b: &DspBundle) -> Result<Survey> {
    let stock = &b.stock;
    let boot0 = u.ee_read(BOOT0.start, BOOT0.len())?;
    let boot0 = if boot0 == b.boot0 {
        Boot0State::Ours
    } else if boot0 == stock[BOOT0] {
        Boot0State::Stock
    } else {
        Boot0State::Broken
    };
    let stock_intact = if boot0 == Boot0State::Stock {
        let bad = crc_pages(u, &spans(BOOT0.end, STOCK_IMAGE_END), |a, n| crc32fast::hash(&stock[a..a + n]))?;
        Some(bad.is_empty())
    } else {
        None
    };
    let boot1_bad = crc_pages(u, &pages(BOOT1.start, BOOT1.len()), |a, n| crc32fast::hash(b.boot1_at(a, n)))?;

    let hdr_page = u.ee_read(HDR, EE_PAGE)?;
    let header = ImageHeader::parse(&hdr_page).ok();
    let state = BootState::parse(&u.ee_read(STATE, STATE_LEN)?);
    let prev_seq = header.map_or(0, |h| h.seq).max(state.map_or(0, |s| s.seq));

    let body_bad = crc_pages(u, &b.body_pages(), |a, n| crc32fast::hash(b.body_at(a, n)))?;

    // State page and profile area: pages whose stock content is not blank, still equal to the stock bytes.
    let mut candidates = vec![(STATE, EE_PAGE)];
    candidates.extend(pages(PROFILES.start, PROFILES.len()));
    let graph: Vec<(usize, usize)> = b.graph_pages().iter().map(|p| (p.0, EE_PAGE)).collect();
    candidates.retain(|&(a, n)| stock[a..a + n].iter().any(|&x| x != 0xFF) && !graph.contains(&(a, n)));
    let mut stale_pages = Vec::new();
    for &(a, n) in &candidates {
        if u.ee_crc(a, n)? == crc32fast::hash(&stock[a..a + n]) {
            stale_pages.push((a, n));
        }
    }
    let graph_bad = crc_pages(u, &graph, |a, _| crc32fast::hash(&b.graph_at(a)))?;
    let last_page_ok = u.ee_crc(LAST_PAGE.start, LAST_PAGE.len())? == crc32fast::hash(&stock[LAST_PAGE]);

    Ok(Survey { boot0, stock_intact, boot1_bad, header, body_bad, stale_pages, graph_bad, last_page_ok, prev_seq })
}
