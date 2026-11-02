//! TAS1020B EEPROM / DFU image format (Data Manual SLES025B, §2.2.2.3).
//!
//! Layout: 18-byte header (+ optional USB strings, counted in `header_size`), then the application code,
//! which is loaded at program address 0x0000. Multi-byte header fields are big-endian.
//!
//! | Offset | Field | Notes |
//! |---|---|---|
//! | 0 | checksum | low byte of the sum of bytes 1..header_size-1 (strings included) |
//! | 1 | header_size | header length in bytes, strings included |
//! | 2 | signature | 0x1234 |
//! | 4 / 6 | vendor_id / product_id | |
//! | 8 / 9 | product_version / firmware_version | |
//! | 10 | usb_attributes | bit 0 strings present, bit 2 self powered, bit 3 bus powered |
//! | 11 | max_power | units of 2 mA |
//! | 12 | attributes | bit 0 24 MHz CPU, bit 1 header overwrite (DFU), bit 3 400 kHz EEPROM |
//! | 13 | write_page | max I2C write page |
//! | 14 | data_type | 1 application, 2 device EEPROM (ROM boots in DFU), 3 download in progress |
//! | 15 | read_page | max I2C read page (0 = whole payload at once) |
//! | 16 | payload_size | application size in bytes |
//!
//! [`data_area`]: the OpenCompanion data area at 0x1800..0x1FFF of the same EEPROM (descriptors, tables).

pub mod data_area;

/// Header length without strings.
pub const HEADER_LEN: usize = 18;
pub const SIGNATURE: u16 = 0x1234;
/// `attributes` bit 1: during a DFU download, write the downloaded header into the EEPROM.
pub const ATTR_HEADER_OVERWRITE: u8 = 0x02;

/// Stock Bose header of U6002 (firmware 01.01.03), as dumped.
pub const BOSE_HEADER: [u8; HEADER_LEN] =
    [0x69, 0x12, 0x12, 0x34, 0x05, 0xA7, 0x10, 0x20, 0x0A, 0x04, 0x04, 0x01, 0x08, 0x20, 0x01, 0x00, 0x15, 0xE4];

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("image too short ({0} bytes)")]
    TooShort(usize),
    #[error("bad header signature 0x{0:04x}")]
    BadSignature(u16),
    #[error("bad header checksum: stored 0x{stored:02x}, computed 0x{computed:02x}")]
    BadChecksum { stored: u8, computed: u8 },
    #[error("payload size {declared} exceeds the {available} bytes available")]
    PayloadTooLarge { declared: usize, available: usize },
    #[error("application code too large ({0} bytes, 6016 max in TAS1020B RAM)")]
    CodeTooLarge(usize),
    #[error("data area: {0}")]
    DataArea(String),
}

pub type Result<T> = std::result::Result<T, Error>;

/// Program RAM of the TAS1020B available to the application.
pub const MAX_CODE: usize = 6016;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataType {
    /// Header + code: the ROM loads and runs the code.
    Application,
    /// Header only: the ROM boots in DFU mode (download to RAM) with the header VID/PID.
    Device,
    /// A DFU download was interrupted: the ROM boots in DFU mode.
    Updating,
    Other(u8),
}

impl From<u8> for DataType {
    fn from(v: u8) -> Self {
        match v {
            1 => Self::Application,
            2 => Self::Device,
            3 => Self::Updating,
            v => Self::Other(v),
        }
    }
}

impl From<DataType> for u8 {
    fn from(t: DataType) -> u8 {
        match t {
            DataType::Application => 1,
            DataType::Device => 2,
            DataType::Updating => 3,
            DataType::Other(v) => v,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Header {
    pub header_size: u8,
    pub signature: u16,
    pub vendor_id: u16,
    pub product_id: u16,
    pub product_version: u8,
    pub firmware_version: u8,
    pub usb_attributes: u8,
    pub max_power: u8,
    pub attributes: u8,
    pub write_page: u8,
    pub data_type: DataType,
    pub read_page: u8,
    pub payload_size: u16,
    /// USB strings following the 18 fixed bytes (empty when `usb_attributes` bit 0 is clear).
    pub strings: Vec<u8>,
}

pub fn checksum(header: &[u8]) -> u8 {
    header[1..].iter().fold(0u8, |acc, b| acc.wrapping_add(*b))
}

impl Header {
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(Error::TooShort(data.len()));
        }
        let size = (data[1] as usize).max(HEADER_LEN);
        if data.len() < size {
            return Err(Error::TooShort(data.len()));
        }
        let be = |i: usize| u16::from_be_bytes([data[i], data[i + 1]]);
        let signature = be(2);
        if signature != SIGNATURE {
            return Err(Error::BadSignature(signature));
        }
        let computed = checksum(&data[..size]);
        if computed != data[0] {
            return Err(Error::BadChecksum { stored: data[0], computed });
        }
        Ok(Self {
            header_size: data[1],
            signature,
            vendor_id: be(4),
            product_id: be(6),
            product_version: data[8],
            firmware_version: data[9],
            usb_attributes: data[10],
            max_power: data[11],
            attributes: data[12],
            write_page: data[13],
            data_type: data[14].into(),
            read_page: data[15],
            payload_size: be(16),
            strings: data[HEADER_LEN..size].to_vec(),
        })
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = vec![0u8; HEADER_LEN];
        b[1] = (HEADER_LEN + self.strings.len()) as u8;
        b[2..4].copy_from_slice(&self.signature.to_be_bytes());
        b[4..6].copy_from_slice(&self.vendor_id.to_be_bytes());
        b[6..8].copy_from_slice(&self.product_id.to_be_bytes());
        b[8] = self.product_version;
        b[9] = self.firmware_version;
        b[10] = self.usb_attributes;
        b[11] = self.max_power;
        b[12] = self.attributes;
        b[13] = self.write_page;
        b[14] = self.data_type.into();
        b[15] = self.read_page;
        b[16..18].copy_from_slice(&self.payload_size.to_be_bytes());
        b.extend_from_slice(&self.strings);
        b[0] = checksum(&b);
        b
    }

    /// True when this is the stock Bose header, ignoring `data_type` and the header-overwrite attribute bit.
    pub fn matches_bose(&self) -> bool {
        let mut canonical = Header::parse(&BOSE_HEADER).expect("valid constant");
        canonical.data_type = self.data_type;
        canonical.attributes = (canonical.attributes & !ATTR_HEADER_OVERWRITE) | (self.attributes & ATTR_HEADER_OVERWRITE);
        *self == canonical
    }

    /// The stock Bose header with another data type (fresh checksum, header-overwrite bit clear).
    pub fn bose_with(data_type: DataType) -> Self {
        let mut h = Header::parse(&BOSE_HEADER).expect("valid constant");
        h.data_type = data_type;
        h
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Image {
    pub header: Header,
    pub code: Vec<u8>,
}

impl Image {
    pub fn parse(data: &[u8]) -> Result<Self> {
        let header = Header::parse(data)?;
        let start = (header.header_size as usize).max(HEADER_LEN);
        let declared = header.payload_size as usize;
        let available = data.len() - start;
        if declared > available {
            return Err(Error::PayloadTooLarge { declared, available });
        }
        Ok(Self { code: data[start..start + declared].to_vec(), header })
    }

    /// Builds an image around raw code, with `header` as template (payload size and checksum recomputed).
    pub fn build(mut header: Header, code: Vec<u8>) -> Result<Self> {
        if code.len() > MAX_CODE {
            return Err(Error::CodeTooLarge(code.len()));
        }
        header.payload_size = code.len() as u16;
        Ok(Self { header, code })
    }

    pub fn set_header_overwrite(&mut self, on: bool) {
        if on {
            self.header.attributes |= ATTR_HEADER_OVERWRITE;
        } else {
            self.header.attributes &= !ATTR_HEADER_OVERWRITE;
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut out = self.header.to_bytes();
        out.extend_from_slice(&self.code);
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bose_header_is_valid() {
        let h = Header::parse(&BOSE_HEADER).unwrap();
        assert_eq!((h.vendor_id, h.product_id), (0x05A7, 0x1020));
        assert_eq!(h.data_type, DataType::Application);
        assert_eq!(h.payload_size, 0x15E4);
        assert!(h.matches_bose());
        assert_eq!(h.to_bytes(), BOSE_HEADER.to_vec());
    }

    #[test]
    fn device_mode_header() {
        let b = Header::bose_with(DataType::Device).to_bytes();
        assert_eq!(b[14], 2);
        assert_eq!(b[0], checksum(&b));
        assert_eq!(b[0], 0x6A); // 0x69 + 1
        assert!(Header::parse(&b).unwrap().matches_bose());
    }

    #[test]
    fn rejects_bad_checksum_and_foreign_header() {
        let mut b = BOSE_HEADER;
        b[14] = 2;
        assert!(matches!(Header::parse(&b), Err(Error::BadChecksum { .. })));
        let mut h = Header::parse(&BOSE_HEADER).unwrap();
        h.product_id = 0x1021;
        assert!(!h.matches_bose());
    }

    #[test]
    fn image_round_trip() {
        let code = vec![0x02, 0x0D, 0x84, 0x00];
        let mut img = Image::build(Header::bose_with(DataType::Application), code.clone()).unwrap();
        img.set_header_overwrite(true);
        let bytes = img.to_bytes();
        assert_eq!(bytes[12] & ATTR_HEADER_OVERWRITE, ATTR_HEADER_OVERWRITE);
        let back = Image::parse(&bytes).unwrap();
        assert_eq!(back.code, code);
        assert_eq!(back.header.payload_size, 4);
        assert!(back.header.matches_bose() == false); // payload size differs from the stock header
    }
}
