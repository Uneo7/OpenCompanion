use std::ops::Range;

use oc_tas_image::data_area::{self, DataArea};
use oc_tas_image::{checksum, DataType, Header, Image, MAX_CODE, SIGNATURE};

use crate::error::{refused, Result};

pub const EE_SIZE: usize = 8192;
/// AT24C64 write page.
pub const PAGE: usize = 32;
pub const HEADER_LEN: usize = oc_tas_image::HEADER_LEN;
pub const CODE_START: usize = HEADER_LEN;
/// 6016 bytes of code.
pub const OC_CODE_END: usize = CODE_START + MAX_CODE;
pub const DATA_TYPE_AT: usize = 14;
/// OpenCompanion data area (descriptors, tables): after both the stock image (ends 0x15F5) and the largest
/// OpenCompanion image (ends 0x1791).
pub const DATA_AREA: Range<usize> = data_area::START..data_area::END;

#[derive(Debug, Clone)]
pub struct TasTarget {
    pub image: Image,
    /// The OpenCompanion firmware stays off USB without a valid one.
    pub data: Option<DataArea>,
    pub header: [u8; HEADER_LEN],
    pub bytes: Vec<u8>,
}

impl TasTarget {
    /// From a TAS image (e.g. `oc_tas.dfu`) and its data area (`oc_tas_data.bin`, same build): the image header
    /// is kept (VID/PID, attributes) with DataType 1, the data area goes to 0x1800.
    pub fn with_data(image: Image, data: DataArea) -> Result<Self> {
        let mut t = Self::new(image)?;
        t.bytes[DATA_AREA].copy_from_slice(&data.area_bytes());
        t.data = Some(data);
        Ok(t)
    }

    /// From a TAS image alone: the data area stays blank (0xFF).
    pub fn new(mut image: Image) -> Result<Self> {
        if !image.header.strings.is_empty() {
            return refused("the image header carries USB strings: the code would not start at 0x0012");
        }
        if image.code.is_empty() || image.code.len() > MAX_CODE {
            return refused(format!("code is {} bytes (1..{MAX_CODE} allowed)", image.code.len()));
        }
        if image.header.signature != SIGNATURE {
            return refused("bad header signature");
        }
        image.header.data_type = DataType::Application;
        image.header.payload_size = image.code.len() as u16;
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&image.header.to_bytes());
        let mut bytes = vec![0xFF; EE_SIZE];
        bytes[..HEADER_LEN].copy_from_slice(&header);
        bytes[CODE_START..CODE_START + image.code.len()].copy_from_slice(&image.code);
        Ok(Self { image, data: None, header, bytes })
    }

    /// From a full 8 KB dump (a backup), kept byte for byte: the header must be a valid DataType 1 header
    /// without USB strings and its code must fit the EEPROM.
    pub fn from_dump(bytes: &[u8]) -> Result<Self> {
        if bytes.len() != EE_SIZE {
            return refused(format!("the TAS backup is {} bytes, expected {EE_SIZE}", bytes.len()));
        }
        let image = Image::parse(bytes)?;
        if image.header.data_type != DataType::Application || !image.header.strings.is_empty() {
            return refused("the TAS backup header is not a plain application header (DataType 1, no strings)");
        }
        let mut header = [0u8; HEADER_LEN];
        header.copy_from_slice(&bytes[..HEADER_LEN]);
        Ok(Self { image, data: None, header, bytes: bytes.to_vec() })
    }

    pub fn describe(&self) -> String {
        let h = &self.image.header;
        let data = match &self.data {
            Some(d) => format!(", data area {} bytes (layout {})", data_area::HEADER_LEN + d.payload.len(), d.version),
            None => ", no data area".into(),
        };
        let what = if self.header == oc_tas_image::BOSE_HEADER { "stock Bose TAS image (01.01.03)" } else { "TAS image" };
        format!(
            "{what}: {} bytes of code, header VID:PID {:04x}:{:04x}, product/firmware version {}/{}{data}",
            self.image.code.len(),
            h.vendor_id,
            h.product_id,
            h.product_version,
            h.firmware_version
        )
    }
}

/// TI boot header shape: length 0x12, signature 0x1234.
pub fn header_shape_ok(h: &[u8]) -> bool {
    h.len() >= HEADER_LEN && h[1] == 0x12 && h[2] == 0x12 && h[3] == 0x34
}

/// The current header with DataType 3 and a fresh checksum (the ROM accepts DataType 3 even with a wrong
/// checksum, but a valid one costs nothing). Refuses a header that is not a TI boot header.
pub fn updating_header(current: &[u8]) -> Result<[u8; HEADER_LEN]> {
    if !header_shape_ok(current) {
        return refused("the current TAS EEPROM header is not a TI boot header: refusing to touch it");
    }
    let mut h = [0u8; HEADER_LEN];
    h.copy_from_slice(&current[..HEADER_LEN]);
    h[DATA_TYPE_AT] = DataType::Updating.into();
    h[0] = checksum(&h);
    Ok(h)
}

pub fn parse_header(h: &[u8]) -> Option<Header> {
    Header::parse(h.get(..HEADER_LEN)?).ok()
}
