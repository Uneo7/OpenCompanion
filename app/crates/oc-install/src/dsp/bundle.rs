use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::image::AppImage;
use oc_ais::eeprom::layout::{APP_RAM, BOOT0, BOOT1, BODY, EE_PAGE, EE_SIZE};
use oc_ais::eeprom::update::BOOT0_STAGING_RAM;
use oc_ais::MAGIC;
use oc_ramload::{LoadImage, LOADER_HI, LOADER_LO};

use super::enter::{Loaders, StockRef};
use crate::error::{refused, Result};

/// AIS "Disable CRC" opcode: second word of the stock boot0 (`u7001_dsp.bin`).
pub(crate) const OP_DISABLE_CRC: u32 = 0x5853_5904;

#[derive(Debug, Clone)]
pub struct DspBundle {
    pub stock: Vec<u8>,
    pub stub: LoadImage,
    pub updater: LoadImage,
    pub boot0: [u8; 256],
    /// 0xDB00..0xFF7F, CRC-32 in its last 4 bytes.
    pub boot1: Vec<u8>,
    /// Header seq is replaced at write time.
    pub app: AppImage,
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl DspBundle {
    /// Nothing is sent to the device before this passes.
    pub fn validate(&self) -> Result<()> {
        if self.stock.len() != EE_SIZE {
            return refused(format!("stock backup is {} bytes, expected {EE_SIZE}", self.stock.len()));
        }
        if le32(&self.stock, 0) != MAGIC || le32(&self.stock, 4) != OP_DISABLE_CRC {
            return refused("the stock backup does not start like the stock Bose boot image");
        }
        if self.stock[BOOT1].iter().any(|&b| b != 0xFF) {
            return refused("the stock backup is not blank over the boot1 area 0xDB00..0xFF7F");
        }
        if self.boot1.len() != BOOT1.len() {
            return refused(format!("boot1 area is {} bytes, expected {}", self.boot1.len(), BOOT1.len()));
        }
        let n = self.boot1.len() - 4;
        if crc32fast::hash(&self.boot1[..n]) != le32(&self.boot1, n) {
            return refused("boot1 area CRC-32 is wrong");
        }
        if le32(&self.boot0, 0) != MAGIC {
            return refused("boot0 does not start with the AIS magic");
        }
        if self.boot0[..] == self.stock[BOOT0] {
            return refused("boot0 equals the stock boot0");
        }
        self.stub.check_ranges(LOADER_LO, LOADER_HI, None)?;
        self.updater.check_ranges(APP_RAM.start, APP_RAM.end, Some((LOADER_LO, LOADER_HI)))?;
        let staging = (BOOT0_STAGING_RAM, BOOT0_STAGING_RAM + 256);
        self.updater.check_ranges(APP_RAM.start, APP_RAM.end, Some(staging))?;
        let body = &self.app.body.bytes;
        if body.len() > BODY.len() || body.len() < 12 || !body.len().is_multiple_of(4) {
            return refused(format!("application body is {} bytes", body.len()));
        }
        if self.app.header.body_crc != crc32fast::hash(body) || self.app.header.body_len as usize != body.len() {
            return refused("application header does not describe its body");
        }
        if oc_ais::eeprom::body::parse(body).map(|l| l.entry) != Ok(self.app.header.entry) {
            return refused("application body is not a valid boot1 body for its entry point");
        }
        Ok(())
    }

    pub fn loaders(&self) -> Loaders<'_> {
        Loaders { stock: StockRef::Image(&self.stock), stub: &self.stub, updater: &self.updater }
    }

    pub fn header(&self, seq: u32) -> ImageHeader {
        ImageHeader { seq, ..self.app.header }
    }

    pub fn header_page(&self, seq: u32) -> [u8; EE_PAGE] {
        self.header(seq).to_page()
    }

    /// The last page may be partial.
    pub fn body_pages(&self) -> Vec<(usize, usize)> {
        pages(BODY.start, self.app.body.bytes.len())
    }

    pub fn body_at(&self, addr: usize, len: usize) -> &[u8] {
        &self.app.body.bytes[addr - BODY.start..addr - BODY.start + len]
    }

    /// Empty for a firmware before layout v3.
    pub fn graph_pages(&self) -> Vec<(usize, Vec<u8>)> {
        self.app.graph.as_ref().map_or_else(Vec::new, |g| g.pages())
    }

    pub fn graph_at(&self, addr: usize) -> Vec<u8> {
        self.graph_pages().into_iter().find(|p| p.0 == addr).map(|p| p.1).unwrap_or_default()
    }

    pub fn boot1_at(&self, addr: usize, len: usize) -> &[u8] {
        &self.boot1[addr - BOOT1.start..addr - BOOT1.start + len]
    }
}

/// `base` must be page aligned.
pub fn pages(base: usize, len: usize) -> Vec<(usize, usize)> {
    (0..len).step_by(EE_PAGE).map(|o| (base + o, EE_PAGE.min(len - o))).collect()
}
