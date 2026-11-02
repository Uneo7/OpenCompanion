//! EEPROM encoding of a base (EEPROM layout v4, spec section 5): a 16-byte header, then the payload
//! (float32 little-endian in [`Base::floats`] order, then one byte per delay).
//!
//! Header: `OCB1`, format version (u8), topology (u8), payload length (LE16), sequence (LE32), CRC-32 (zlib)
//! of header bytes 0..11 followed by the payload.

use super::model::Base;
use super::topology::{payload_floats, payload_len, ID, SETS};
use super::validate;
use super::BaseError;

pub const MAGIC: &[u8; 4] = b"OCB1";
pub const FORMAT_VERSION: u8 = 1;
pub const HDR_LEN: usize = 16;
/// Payload capacity of layout v4: 8 pages + 2 × 112 config tails − header.
pub const CAPACITY: usize = 8 * 128 + 2 * 112 - HDR_LEN;

pub fn encode(b: &Base, seq: u32) -> Result<Vec<u8>, BaseError> {
    validate::check(b)?;
    let floats = b.floats();
    debug_assert_eq!(floats.len(), payload_floats());
    let len = payload_len();
    if len > CAPACITY {
        return Err(BaseError::Payload(format!("{len} bytes, capacity {CAPACITY}")));
    }
    let mut out = Vec::with_capacity(HDR_LEN + len);
    out.extend_from_slice(MAGIC);
    out.push(FORMAT_VERSION);
    out.push(ID);
    out.extend_from_slice(&(len as u16).to_le_bytes());
    out.extend_from_slice(&seq.to_le_bytes());
    let body: Vec<u8> = floats.iter().flat_map(|x| x.to_le_bytes()).chain(b.delays).collect();
    let mut h = crc32fast::Hasher::new();
    h.update(&out[..12]);
    h.update(&body);
    out.extend_from_slice(&h.finalize().to_le_bytes());
    out.extend_from_slice(&body);
    Ok(out)
}

/// Checks the header, the CRC and the hard rules.
pub fn decode(bytes: &[u8], name: &str) -> Result<(u32, Base), BaseError> {
    let bad = |s: &str| Err(BaseError::Payload(s.to_string()));
    let len = payload_len();
    if bytes.len() < HDR_LEN + len || &bytes[..4] != MAGIC {
        return bad("no OCB1 header");
    }
    if bytes[4] != FORMAT_VERSION || bytes[5] != ID || u16::from_le_bytes([bytes[6], bytes[7]]) as usize != len {
        return bad("unknown format version, topology or length");
    }
    let body = &bytes[HDR_LEN..HDR_LEN + len];
    let mut h = crc32fast::Hasher::new();
    h.update(&bytes[..12]);
    h.update(body);
    if h.finalize().to_le_bytes() != bytes[12..16] {
        return bad("CRC mismatch");
    }
    let seq = u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]);
    let f: Vec<f32> = body[..4 * payload_floats()].as_chunks::<4>().0.iter().map(|c| f32::from_le_bytes(*c)).collect();
    let mut b = Base::blank(name);
    b.set_scalars(&f);
    let mut i = super::topology::SCALARS;
    for (d, s) in SETS.iter().zip(b.sets.iter_mut()) {
        s.gain = f[i];
        for (k, c) in s.sections.iter_mut().enumerate() {
            let o = i + 1 + 4 * k;
            *c = [f[o], f[o + 1], f[o + 2], f[o + 3]];
        }
        i += 1 + 4 * d.nsec;
    }
    b.delays.copy_from_slice(&body[4 * payload_floats()..]);
    validate::check(&b)?;
    Ok((seq, b))
}
