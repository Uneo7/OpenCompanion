use std::ops::Range;

use super::target::{TasTarget, CODE_START, EE_SIZE, HEADER_LEN, PAGE};
use crate::error::{refused, Result};

/// Device audio mode record of the OpenCompanion TAS firmware (`firmware/usb/app/data/ee_layout.h` EE_MODE_ADDR):
/// a user setting, written only by the HID MODE command. Installs keep the speaker's value (any value but 2 means
/// 5.1, so a stock speaker's blank byte is fine) and never compare or write it.
pub const MODE_ADDR: usize = 0x17FF;

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TasPlan {
    /// Writes `(address, bytes)`, each inside one 32-byte page and inside one writable range.
    pub writes: Vec<(usize, Vec<u8>)>,
    pub commit: bool,
}

impl TasPlan {
    pub fn is_empty(&self) -> bool {
        self.writes.is_empty() && !self.commit
    }

    pub fn bytes(&self) -> usize {
        self.writes.iter().map(|(_, d)| d.len()).sum()
    }
}

/// `writable` = ranges the transport accepts (bytes outside them must already match). Each page is cut along
/// these ranges: a page that straddles the end of one (e.g. the end of the code area) is written in parts.
pub fn plan(current: &[u8], target: &TasTarget, writable: &[Range<usize>]) -> Result<TasPlan> {
    if current.len() != EE_SIZE {
        return refused(format!("TAS EEPROM read returned {} bytes, expected {EE_SIZE}", current.len()));
    }
    let mut want = target.bytes.clone();
    want[MODE_ADDR] = current[MODE_ADDR];
    let mut p = TasPlan { commit: current[..HEADER_LEN] != target.header, ..Default::default() };
    for page in (0..EE_SIZE).step_by(PAGE) {
        let lo = page.max(CODE_START);
        let hi = page + PAGE;
        if lo >= hi || current[lo..hi] == want[lo..hi] {
            continue;
        }
        let mut covered = vec![false; hi - lo];
        for r in writable {
            let (a, b) = (lo.max(r.start), hi.min(r.end));
            if a >= b {
                continue;
            }
            covered[a - lo..b - lo].iter_mut().for_each(|c| *c = true);
            if current[a..b] != want[a..b] {
                p.writes.push((a, want[a..b].to_vec()));
            }
        }
        if let Some(i) = (lo..hi).find(|&i| !covered[i - lo] && current[i] != want[i]) {
            return refused(format!(
                "TAS EEPROM 0x{i:04x} differs from the target but this transport can only write {}",
                writable.iter().map(|r| format!("0x{:04x}..0x{:04x}", r.start, r.end)).collect::<Vec<_>>().join(", ")
            ));
        }
    }
    Ok(p)
}
