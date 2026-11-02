//! Image header page (`OCI1`) and boot-state page (`OCBS`). Formats: `docs/LAYOUT.md`.

use super::layout::EE_PAGE;

pub const HDR_MAGIC: &[u8; 4] = b"OCI1";
pub const HDR_VERSION: u16 = 1;
pub const HDR_SIZE: usize = 48;
const HDR_CRC_OFF: usize = 44;

/// Header flag: boot1 counts unconfirmed starts and stays in rescue after 2 of them.
pub const HF_TRIAL: u32 = 1 << 0;
const HF_KNOWN: u32 = HF_TRIAL;

pub const STATE_MAGIC: &[u8; 4] = b"OCBS";
pub const STATE_CONFIRMED: u8 = 0xA5;
pub const STATE_LEN: usize = 16;
pub const BOOT_TRIALS: u8 = 2;

fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageHeader {
    pub seq: u32,
    pub body_len: u32,
    pub body_crc: u32,
    pub entry: u32,
    pub flags: u32,
    pub fw_version: u32,
    /// Bytes loaded into RAM (informative, not checked by boot1).
    pub raw_len: u32,
}

impl ImageHeader {
    /// Full header page: 48-byte header, then 0xFF (erased state).
    pub fn to_page(&self) -> [u8; EE_PAGE] {
        let mut p = [0xFFu8; EE_PAGE];
        let h = &mut p[..HDR_SIZE];
        h.fill(0);
        h[0..4].copy_from_slice(HDR_MAGIC);
        h[4..6].copy_from_slice(&HDR_VERSION.to_le_bytes());
        h[6..8].copy_from_slice(&(HDR_SIZE as u16).to_le_bytes());
        for (o, v) in [(8, self.seq), (12, self.body_len), (16, self.body_crc), (20, self.entry), (24, self.flags), (28, self.fw_version), (32, self.raw_len)] {
            h[o..o + 4].copy_from_slice(&v.to_le_bytes());
        }
        let crc = crc32fast::hash(&h[..HDR_CRC_OFF]);
        h[HDR_CRC_OFF..HDR_SIZE].copy_from_slice(&crc.to_le_bytes());
        p
    }

    /// Same checks as boot1 (`image_read_header`), except the RAM range of the entry (checked by the caller).
    pub fn parse(page: &[u8]) -> Result<Self, HeaderError> {
        let b = page.get(..HDR_SIZE).ok_or(HeaderError::Missing)?;
        if b.iter().all(|&x| x == 0xFF) || &b[0..4] != HDR_MAGIC {
            return Err(HeaderError::Missing);
        }
        if crc32fast::hash(&b[..HDR_CRC_OFF]) != le32(b, HDR_CRC_OFF) {
            return Err(HeaderError::Crc);
        }
        if u16::from_le_bytes([b[4], b[5]]) != HDR_VERSION || u16::from_le_bytes([b[6], b[7]]) as usize != HDR_SIZE {
            return Err(HeaderError::Version);
        }
        let h = Self {
            seq: le32(b, 8),
            body_len: le32(b, 12),
            body_crc: le32(b, 16),
            entry: le32(b, 20),
            flags: le32(b, 24),
            fw_version: le32(b, 28),
            raw_len: le32(b, 32),
        };
        if h.flags & !HF_KNOWN != 0 {
            return Err(HeaderError::Flags(h.flags));
        }
        Ok(h)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
pub enum HeaderError {
    #[error("no header (blank or not OCI1)")]
    Missing,
    #[error("header CRC mismatch")]
    Crc,
    #[error("unsupported header version or size")]
    Version,
    #[error("unknown header flags 0x{0:08x}")]
    Flags(u32),
}

/// Boot-state record (first 16 bytes of the state page).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct BootState {
    pub seq: u32,
    pub attempts: u8,
    pub confirmed: bool,
}

impl BootState {
    pub fn to_bytes(&self) -> [u8; STATE_LEN] {
        let mut b = [0u8; STATE_LEN];
        b[0..4].copy_from_slice(STATE_MAGIC);
        b[4..8].copy_from_slice(&self.seq.to_le_bytes());
        b[8] = self.attempts;
        b[9] = if self.confirmed { STATE_CONFIRMED } else { 0 };
        let crc = crc32fast::hash(&b[..12]);
        b[12..16].copy_from_slice(&crc.to_le_bytes());
        b
    }

    /// `None` if the record is blank or invalid (boot1 then counts 0 attempts).
    pub fn parse(b: &[u8]) -> Option<Self> {
        let b = b.get(..STATE_LEN)?;
        if &b[0..4] != STATE_MAGIC || crc32fast::hash(&b[..12]) != le32(b, 12) {
            return None;
        }
        Some(Self { seq: le32(b, 4), attempts: b[8], confirmed: b[9] == STATE_CONFIRMED })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn header_round_trip() {
        let h = ImageHeader { seq: 7, body_len: 1024, body_crc: 0xDEAD_BEEF, entry: 0x1000_1000, flags: HF_TRIAL, fw_version: 0x0001_0002, raw_len: 4000 };
        let p = h.to_page();
        assert_eq!(ImageHeader::parse(&p).unwrap(), h);
        let mut bad = p;
        bad[10] ^= 1;
        assert_eq!(ImageHeader::parse(&bad), Err(HeaderError::Crc));
        assert_eq!(ImageHeader::parse(&[0xFF; 128]), Err(HeaderError::Missing));
    }

    #[test]
    fn state_round_trip() {
        let s = BootState { seq: 3, attempts: 1, confirmed: false };
        assert_eq!(BootState::parse(&s.to_bytes()), Some(s));
        assert_eq!(BootState::parse(&[0xFF; 16]), None);
    }
}
