//! Write plans for the host app: normal update and migration. Each step is one EEPROM page (or less),
//! written through the updater (`E` 11, at most 56 data bytes per message) and read back by it.
//! Rationale and power-loss analysis: `firmware/dsp/boot/docs/UPDATE.md`.

use super::image::AppImage;
use super::layout::{BODY, BOOT1, EE_PAGE, HDR};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Write `data` at `addr` (inside one page).
    Write { addr: usize, data: Vec<u8> },
    /// Read back the EEPROM CRC-32 of [addr, addr + len) (`E` 12) and compare.
    CheckCrc { addr: usize, len: usize, crc: u32 },
}

fn pages(base: usize, data: &[u8]) -> impl Iterator<Item = Step> + '_ {
    data.chunks(EE_PAGE).enumerate().map(move |(i, d)| Step::Write { addr: base + i * EE_PAGE, data: d.to_vec() })
}

/// Normal update (also used for recovery from rescue mode):
/// 1. invalidate the header (one page of zeros) -> from here on boot1 always enters rescue mode;
/// 2. write the body page by page;
/// 3. check the CRC-32 of the whole body as stored in the EEPROM;
/// 4. graph data pages (layout v3), each checked (not boot-critical: without them the firmware falls back);
/// 5. write the header page LAST, then check it.
pub fn update_plan(img: &AppImage) -> Vec<Step> {
    let mut plan = vec![Step::Write { addr: HDR, data: vec![0; EE_PAGE] }];
    plan.extend(pages(BODY.start, &img.body.bytes));
    plan.push(Step::CheckCrc { addr: BODY.start, len: img.body.bytes.len(), crc: img.header.body_crc });
    for (addr, data) in img.graph.iter().flat_map(|g| g.pages()) {
        let crc = crc32fast::hash(&data);
        plan.push(Step::Write { addr, data });
        plan.push(Step::CheckCrc { addr, len: EE_PAGE, crc });
    }
    plan.push(Step::Write { addr: HDR, data: img.header_page.to_vec() });
    plan.push(Step::CheckCrc { addr: HDR, len: EE_PAGE, crc: crc32fast::hash(&img.header_page) });
    plan
}

/// Migration, part 1 (stock firmware still boots): write and check boot1 in its area above the stock image.
/// Requires the updater's migration unlock (`E` 13 "UNLK").
pub fn migration_boot1_plan(boot1_area: &[u8]) -> Vec<Step> {
    let mut plan: Vec<Step> = pages(BOOT1.start, boot1_area).collect();
    plan.push(Step::CheckCrc { addr: BOOT1.start, len: boot1_area.len(), crc: crc32fast::hash(boot1_area) });
    plan
}

/// Migration, part 2: boot0 is NOT a list of host page writes: it is staged in RAM (`E` 01) and written by the
/// updater in one command (`E` 14 ram crc), both pages back to back. This is the only irrecoverable window.
pub const BOOT0_STAGING_RAM: u32 = 0x1002_0000;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::eeprom::header::ImageHeader;
    use crate::eeprom::body::Body;

    #[test]
    fn header_is_invalidated_first_and_written_last() {
        let h = ImageHeader { seq: 1, body_len: 300, body_crc: 5, entry: 0x1000_1000, flags: 0, fw_version: 0, raw_len: 0 };
        let img = AppImage { header: h, header_page: h.to_page(), body: Body { bytes: vec![7; 300], entry: 0x1000_1000, raw_len: 0 }, graph: None };
        let plan = update_plan(&img);
        assert!(matches!(&plan[0], Step::Write { addr, data } if *addr == HDR && data.iter().all(|&b| b == 0)));
        assert!(matches!(&plan[plan.len() - 2], Step::Write { addr, data } if *addr == HDR && data[..4] == *b"OCI1"));
        let body_pages = plan.iter().filter(|s| matches!(s, Step::Write { addr, .. } if *addr >= BODY.start)).count();
        assert_eq!(body_pages, 3);
    }

    #[test]
    fn graph_pages_before_the_header() {
        use crate::eeprom::graph::GraphData;
        use crate::eeprom::layout::graph_page;
        let h = ImageHeader { seq: 1, body_len: 300, body_crc: 5, entry: 0x1000_1000, flags: 0, fw_version: 0, raw_len: 0 };
        let g = GraphData::new(0x1002_7B2C, vec![3; 921]).unwrap();
        let img = AppImage { header: h, header_page: h.to_page(), body: Body { bytes: vec![7; 300], entry: 0x1000_1000, raw_len: 0 }, graph: Some(g) };
        let plan = update_plan(&img);
        let pos = |a: usize| plan.iter().position(|s| matches!(s, Step::Write { addr, .. } if *addr == a)).unwrap();
        let hdr = plan.iter().rposition(|s| matches!(s, Step::Write { addr, .. } if *addr == HDR)).unwrap();
        for k in 0..8 {
            let p = pos(graph_page(k));
            assert!(p > pos(BODY.start) && p < hdr);
            assert!(matches!(&plan[p + 1], Step::CheckCrc { addr, len, .. } if *addr == graph_page(k) && *len == EE_PAGE));
        }
    }
}
