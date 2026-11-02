//! Same format as `firmware/usb/app/data/ee_layout.h` (built there by `scripts/mkeedata.py` into
//! `build/oc_tas/oc_tas_data.bin`):
//!
//! | Offset | Field |
//! |---|---|
//! | 0 | magic `"OCDA"` |
//! | 4 | layout version (must match the firmware) |
//! | 5 | 0 (reserved) |
//! | 6 | payload length, little-endian |
//! | 8 | CRC-32 of the payload (zlib / IEEE, reflected), little-endian |
//! | 12 | payload |
//!
//! Placement: `0x1800..0x1FFF`. The stock Bose image ends at `0x15F5` (header + 5604 bytes) and the largest
//! OpenCompanion image at `0x1791` (header + 6016 bytes), so the area can be written while either is installed.
//! The firmware checks the header and CRC at boot and stays off USB when the area is invalid.

use crate::{Error, Result};

/// First EEPROM address of the data area (32-byte page aligned).
pub const START: usize = 0x1800;
/// End of the data area (exclusive) = end of the AT24C64.
pub const END: usize = 0x2000;
pub const HEADER_LEN: usize = 12;
pub const MAGIC: [u8; 4] = *b"OCDA";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataArea {
    pub version: u8,
    pub payload: Vec<u8>,
}

impl DataArea {
    pub fn new(version: u8, payload: Vec<u8>) -> Result<Self> {
        if HEADER_LEN + payload.len() > END - START {
            return Err(Error::DataArea(format!("{} bytes do not fit in 0x{START:04x}..0x{END:04x}", payload.len())));
        }
        Ok(Self { version, payload })
    }

    /// Parses and checks a blob (e.g. `oc_tas_data.bin`, or the EEPROM from 0x1800): magic, length, CRC-32.
    /// Bytes after the payload are ignored (0xFF in the EEPROM).
    pub fn parse(data: &[u8]) -> Result<Self> {
        if data.len() < HEADER_LEN {
            return Err(Error::DataArea(format!("{} bytes, shorter than the header", data.len())));
        }
        if data[..4] != MAGIC {
            return Err(Error::DataArea("no \"OCDA\" magic".into()));
        }
        if data[5] != 0 {
            return Err(Error::DataArea(format!("reserved byte is 0x{:02x}", data[5])));
        }
        let len = u16::from_le_bytes([data[6], data[7]]) as usize;
        let payload = data
            .get(HEADER_LEN..HEADER_LEN + len)
            .ok_or_else(|| Error::DataArea(format!("payload length {len} beyond the data")))?;
        let stored = u32::from_le_bytes([data[8], data[9], data[10], data[11]]);
        let computed = crc32fast::hash(payload);
        if stored != computed {
            return Err(Error::DataArea(format!("CRC-32 0x{stored:08x} stored, 0x{computed:08x} computed")));
        }
        Self::new(data[4], payload.to_vec())
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut b = Vec::with_capacity(HEADER_LEN + self.payload.len());
        b.extend_from_slice(&MAGIC);
        b.push(self.version);
        b.push(0);
        b.extend_from_slice(&(self.payload.len() as u16).to_le_bytes());
        b.extend_from_slice(&crc32fast::hash(&self.payload).to_le_bytes());
        b.extend_from_slice(&self.payload);
        b
    }

    /// The whole area `START..END` as it must be in the EEPROM: blob then 0xFF.
    pub fn area_bytes(&self) -> Vec<u8> {
        let mut b = self.to_bytes();
        b.resize(END - START, 0xFF);
        b
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip_and_crc() {
        let a = DataArea::new(1, b"123456789".to_vec()).unwrap();
        let b = a.to_bytes();
        assert_eq!(&b[8..12], &0xCBF4_3926u32.to_le_bytes(), "CRC-32 check value of \"123456789\"");
        assert_eq!(DataArea::parse(&b).unwrap(), a);
        let area = a.area_bytes();
        assert_eq!(area.len(), 2048);
        assert!(area[b.len()..].iter().all(|&x| x == 0xFF));
        assert_eq!(DataArea::parse(&area).unwrap(), a, "padding is ignored");
    }

    #[test]
    fn rejects_corruption() {
        let mut b = DataArea::new(1, vec![7; 300]).unwrap().to_bytes();
        b[100] ^= 1;
        assert!(DataArea::parse(&b).is_err());
        assert!(DataArea::parse(&[0xFF; 2048]).is_err(), "blank EEPROM");
        assert!(DataArea::new(1, vec![0; 2048]).is_err(), "too large");
    }

    #[test]
    fn placement_clears_both_images() {
        assert!(crate::HEADER_LEN + 5604 <= START, "stock Bose image");
        assert!(crate::HEADER_LEN + crate::MAX_CODE <= START, "largest OpenCompanion image");
        assert_eq!(START % 32, 0);
    }
}
