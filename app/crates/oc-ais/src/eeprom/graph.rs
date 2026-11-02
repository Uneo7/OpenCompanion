//! The program tables of the "Bose d'origine" engine (output section `.bose_tab`, linked in DSP RAM) are not
//! part of the image body: they go to the eight graph data pages, the last page of each profile copy, which no
//! profile record uses and no firmware writes. The firmware copies them back to RAM at start and checks them
//! against the CRC generated with them; if they are missing it falls back to the EQ path.
//! Page 0: `"OCG1"`, version (LE16) = 1, payload length (LE16), RAM address (LE32), CRC-32 of the payload (LE32),
//! then the payload; pages 1..7: payload continued; unused bytes 0xFF.

use super::boot::symbol;
use super::layout::{graph_page, EE_PAGE, EE_SIZE, GRAPH_PAGES};
use crate::format::Segment;
use crate::{Error, Result};

pub const MAGIC: &[u8; 4] = b"OCG1";
pub const VERSION: u16 = 1;
pub const HDR_LEN: usize = 16;
pub const CAPACITY: usize = GRAPH_PAGES * EE_PAGE - HDR_LEN;
/// Linker symbols of the section (lnk.cmd: RUN_START / SIZE of `.bose_tab`).
pub const SYM_START: &str = "__oc_bose_tab_start";
pub const SYM_SIZE: &str = "__oc_bose_tab_size";
/// Marker of a firmware that keeps the graph data pages out of its write window.
pub const LAYOUT_V3_SYMBOL: &str = "oc_eeprom_layout_v3";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GraphData {
    pub addr: u32,
    pub payload: Vec<u8>,
}

fn le16(b: &[u8], o: usize) -> usize {
    u16::from_le_bytes([b[o], b[o + 1]]) as usize
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl GraphData {
    pub fn new(addr: u32, payload: Vec<u8>) -> Result<Self> {
        if payload.is_empty() || payload.len() > CAPACITY {
            return Err(Error::Layout(format!("graph tables are {} bytes, the graph data pages hold {CAPACITY}", payload.len())));
        }
        Ok(GraphData { addr, payload })
    }

    pub fn bytes(&self) -> Vec<u8> {
        let mut v = Vec::with_capacity(GRAPH_PAGES * EE_PAGE);
        v.extend_from_slice(MAGIC);
        v.extend_from_slice(&VERSION.to_le_bytes());
        v.extend_from_slice(&(self.payload.len() as u16).to_le_bytes());
        v.extend_from_slice(&self.addr.to_le_bytes());
        v.extend_from_slice(&crc32fast::hash(&self.payload).to_le_bytes());
        v.extend_from_slice(&self.payload);
        v.resize(GRAPH_PAGES * EE_PAGE, 0xFF);
        v
    }

    pub fn pages(&self) -> Vec<(usize, Vec<u8>)> {
        self.bytes().chunks(EE_PAGE).enumerate().map(|(k, p)| (graph_page(k), p.to_vec())).collect()
    }

    pub fn parse(ee: &[u8]) -> std::result::Result<GraphData, String> {
        if ee.len() != EE_SIZE {
            return Err("not a 64 KB dump".into());
        }
        let raw: Vec<u8> = (0..GRAPH_PAGES).flat_map(|k| ee[graph_page(k)..graph_page(k) + EE_PAGE].iter().copied()).collect();
        if &raw[..4] != MAGIC {
            return Err("no graph data (magic)".into());
        }
        let (ver, len, addr, crc) = (le16(&raw, 4), le16(&raw, 6), le32(&raw, 8), le32(&raw, 12));
        if ver != VERSION as usize || len == 0 || len > CAPACITY {
            return Err(format!("unknown graph data version {ver} or length {len}"));
        }
        let payload = raw[HDR_LEN..HDR_LEN + len].to_vec();
        if crc32fast::hash(&payload) != crc {
            return Err("graph data CRC-32 mismatch".into());
        }
        Ok(GraphData { addr, payload })
    }
}

/// Takes the `.bose_tab` segment out of `segs` (it must be a segment of its own) and returns it as graph data.
/// `None` for a firmware without the section (before layout v3).
pub fn split(app_elf: &[u8], segs: &mut Vec<Segment>) -> Result<Option<GraphData>> {
    let Some(start) = symbol(app_elf, SYM_START)? else { return Ok(None) };
    let size = symbol(app_elf, SYM_SIZE)?.ok_or_else(|| Error::Layout(format!("{SYM_START} without {SYM_SIZE}")))? as usize;
    if symbol(app_elf, LAYOUT_V3_SYMBOL)?.is_none() {
        return Err(Error::Layout(format!("the firmware has graph tables but does not define `{LAYOUT_V3_SYMBOL}`")));
    }
    let i = segs
        .iter()
        .position(|s| s.addr == start && s.data.len() == size)
        .ok_or_else(|| Error::Layout(format!("graph tables 0x{start:08x}+0x{size:x} are not a segment of their own")))?;
    let s = segs.remove(i);
    GraphData::new(s.addr, s.data).map(Some)
}

#[cfg(test)]
mod tests {
    use super::super::layout::profile_copy;
    use super::*;

    #[test]
    fn pages_round_trip() {
        let g = GraphData::new(0x1002_7B2C, (0..921u32).map(|i| (i * 7) as u8).collect()).unwrap();
        let pages = g.pages();
        assert_eq!(pages.len(), GRAPH_PAGES);
        assert_eq!(pages[0].0, 0xC480);
        assert_eq!(pages[7].0, 0xD980);
        let mut ee = vec![0xFFu8; EE_SIZE];
        for (a, p) in &pages {
            assert_eq!(p.len(), EE_PAGE);
            ee[*a..*a + EE_PAGE].copy_from_slice(p);
        }
        assert_eq!(GraphData::parse(&ee).unwrap(), g);
        ee[0xC4FF] ^= 1; // last payload byte of page 0
        assert!(GraphData::parse(&ee).is_err());
    }

    #[test]
    fn refuses_oversize() {
        assert!(GraphData::new(0, vec![0; CAPACITY + 1]).is_err());
        assert!(GraphData::new(0, vec![0; CAPACITY]).is_ok());
    }

    #[test]
    fn pages_are_outside_profile_records() {
        // a profile record is 16 + 616 bytes: the graph page of its copy starts after it and ends the copy
        for k in 0..GRAPH_PAGES {
            let copy = profile_copy(k / 2, k % 2);
            assert!(graph_page(k) >= copy + 632 && graph_page(k) + EE_PAGE == copy + 0x300);
        }
    }
}
