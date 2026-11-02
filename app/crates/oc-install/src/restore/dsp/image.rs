use oc_ais::eeprom::header::ImageHeader;
use oc_ais::eeprom::layout::{APP_RAM, BOOT0, BOOT1, EE_PAGE, EE_SIZE, HDR};
use oc_ais::eeprom::update::BOOT0_STAGING_RAM;
use oc_ais::MAGIC;
use oc_ramload::{LoadImage, LOADER_HI, LOADER_LO};

use crate::dsp::bundle::OP_DISABLE_CRC;
use crate::dsp::{Loaders, StockRef};
use crate::error::{refused, Result};

#[derive(Debug, Clone)]
pub struct DspRestoreInputs {
    pub image: Vec<u8>,
    /// Only used when the stock firmware already runs (resume after R3).
    pub stub: LoadImage,
    pub updater: LoadImage,
}

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

impl DspRestoreInputs {
    /// The image must be complete stock, restorable in the R1..R4 order.
    pub fn validate(&self) -> Result<()> {
        let img = &self.image;
        if img.len() != EE_SIZE {
            return refused(format!("the DSP backup is {} bytes, expected {EE_SIZE}", img.len()));
        }
        if le32(img, 0) != MAGIC || le32(img, 4) != OP_DISABLE_CRC {
            return refused("the DSP backup does not start like the stock Bose boot image");
        }
        if ImageHeader::parse(&img[HDR..HDR + EE_PAGE]).is_ok() {
            return refused("the DSP backup holds an OpenCompanion image header: it was taken after the install");
        }
        // The ROM must find the whole stock AIS stream below boot1, which is only restored after boot0 (R4).
        match oc_ais::parse(&img[..BOOT1.start]) {
            Ok(ais) if ais.entry().is_some() => {}
            _ => return refused("the stock AIS stream of the DSP backup does not end below 0xDB00"),
        }
        self.stub.check_ranges(LOADER_LO, LOADER_HI, None)?;
        self.updater.check_ranges(APP_RAM.start, APP_RAM.end, Some((LOADER_LO, LOADER_HI)))?;
        self.updater.check_ranges(APP_RAM.start, APP_RAM.end, Some((BOOT0_STAGING_RAM, BOOT0_STAGING_RAM + 256)))?;
        Ok(())
    }

    pub fn loaders(&self) -> Loaders<'_> {
        Loaders { stock: StockRef::Image(&self.image), stub: &self.stub, updater: &self.updater }
    }

    pub fn boot0(&self) -> [u8; 256] {
        self.image[BOOT0].try_into().expect("256 bytes")
    }

    pub fn at(&self, addr: usize, len: usize) -> &[u8] {
        &self.image[addr..addr + len]
    }

    pub fn crc(&self) -> u32 {
        crc32fast::hash(&self.image)
    }
}
