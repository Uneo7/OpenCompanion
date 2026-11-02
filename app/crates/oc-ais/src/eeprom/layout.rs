//! Mirror of `firmware/dsp/boot/boot1/include/layout.h`; specification in
//! `firmware/dsp/boot/docs/LAYOUT.md`. Any change here must be made there too (boot1 is written once).

use std::ops::Range;

/// EEPROM size (AT24C512).
pub const EE_SIZE: usize = 0x10000;
/// EEPROM page size: a write never crosses a page boundary.
pub const EE_PAGE: usize = 128;

/// boot0: AIS magic + Section Load of the trampoline + Jump_Close (2 pages, written once).
pub const BOOT0: Range<usize> = 0x0000..0x0100;
/// Image header page ("OCI1"), written last during an update.
pub const HDR: usize = 0x0100;
/// Boot-state page ("OCBS"): trial-boot counter and confirmation.
pub const STATE: usize = 0x0180;
/// Image body (AIS magic, LZ4 sections, Jump_Close).
pub const BODY: Range<usize> = 0x0200..0xC200;
/// Profiles: 4 slots x 2 copies x 0x300, then config copies A/B (2 x 0x80). Layout v3: the last page of each
/// copy is a graph data page ([`graph_page`]).
pub const PROFILES: Range<usize> = 0xC200..0xDB00;
/// boot1 (code + const, CRC-32 in its last 4 bytes), written once. Entirely above the stock image (which ends
/// at 0xDAD8), so that it can be written while the stock firmware still boots.
pub const BOOT1: Range<usize> = 0xDB00..0xFF80;
/// End of the stock Bose image (u7001_dsp.bin: Jump_Close at 0xDAD0..0xDAD7).
pub const STOCK_IMAGE_END: usize = 0xDAD8;
/// Last page: kept from the stock EEPROM (byte 0xFFFF = "CKT" value, 0x02 on this unit).
pub const LAST_PAGE: Range<usize> = 0xFF80..0x10000;
/// Window that boot1 / rescue / updater may write without the migration unlock.
pub const WRITE_WINDOW: Range<usize> = 0x0100..0xDB00;

/// RAM: boot0 trampoline (loaded by the ROM).
pub const RAM_TRAMP: u32 = 0x1002_AF00;
/// Largest trampoline so that boot0 fits in 2 pages: 256 - (magic 4 + SL header 12 + Jump_Close 8).
pub const TRAMP_MAX: usize = 232;
/// RAM: boot1 load address (= entry point).
pub const RAM_BOOT1: u32 = 0x1002_B000;
/// Bytes of boot1 code/const (the last 4 of the area hold its CRC-32).
pub const BOOT1_CODE_MAX: usize = BOOT1.end - BOOT1.start - 4;
/// RAM that application images (and the updater) may load into.
pub const APP_RAM: Range<u32> = 0x1000_0000..RAM_TRAMP;

pub const fn profile_copy(slot: usize, copy: usize) -> usize {
    PROFILES.start + (2 * slot + copy) * 0x300
}
/// Layout v3: offset of the graph data page in a profile copy (a profile record is 632 bytes: pages 0..4).
pub const GRAPH_PAGE_OFF: usize = 0x280;
/// Layout v3: number of graph data pages (one per profile copy).
pub const GRAPH_PAGES: usize = 8;
/// Graph data page `k` (0..7) = last page of profile copy `k` (slot k / 2, copy k % 2). Never written by the
/// firmware; holds the stock graph program tables (`super::graph`).
pub const fn graph_page(k: usize) -> usize {
    profile_copy(k / 2, k % 2) + GRAPH_PAGE_OFF
}
pub const CONFIG_COPIES: [usize; 2] = [0xDA00, 0xDA80];
