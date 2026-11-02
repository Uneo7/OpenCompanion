use std::ops::Range;

use super::band::BandSlot;
use super::layout::{off, BAND_SIZE, EQ_MAX_BANDS, EQ_OUTPUTS, EQ_TARGETS, NAME_LEN, PROFILE_SIZE};
use super::Profile;

#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("profile is {0} bytes, expected {PROFILE_SIZE}")]
pub struct LayoutError(pub usize);

fn put_u16(b: &mut [u8], o: usize, v: u16) {
    b[o..o + 2].copy_from_slice(&v.to_le_bytes());
}

fn get_u16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes([b[o], b[o + 1]])
}

impl Profile {
    pub fn to_bytes(&self) -> [u8; PROFILE_SIZE] {
        let mut b = [0u8; PROFILE_SIZE];
        b[off::VERSION] = self.version;
        b[off::FLAGS] = self.flags;
        b[off::COLOR] = self.color;
        b[off::RESERVED0] = self.reserved0;
        b[off::NAME..off::NAME + NAME_LEN].copy_from_slice(&self.name);
        put_u16(&mut b, off::SUB_LEVEL, self.sub_level_cdb as u16);
        put_u16(&mut b, off::CROSSOVER, self.crossover_hz);
        put_u16(&mut b, off::AUX_GAIN, self.aux_gain_cdb as u16);
        for (i, t) in self.trim_cdb.iter().enumerate() {
            put_u16(&mut b, off::TRIMS + 2 * i, *t as u16);
        }
        put_u16(&mut b, off::BASS, self.bass_milli as u16);
        b[off::RESERVED1..off::RESERVED1 + 2].copy_from_slice(&self.reserved1);
        for (i, slot) in self.bands.iter().flatten().enumerate() {
            let o = off::BANDS + i * BAND_SIZE;
            b[o..o + BAND_SIZE].copy_from_slice(&slot.to_bytes());
        }
        b
    }

    pub fn from_bytes(b: &[u8]) -> Result<Self, LayoutError> {
        if b.len() != PROFILE_SIZE {
            return Err(LayoutError(b.len()));
        }
        let mut name = [0u8; NAME_LEN];
        name.copy_from_slice(&b[off::NAME..off::NAME + NAME_LEN]);
        let mut trim_cdb = [0i16; EQ_OUTPUTS];
        for (i, t) in trim_cdb.iter_mut().enumerate() {
            *t = get_u16(b, off::TRIMS + 2 * i) as i16;
        }
        let mut bands = [[BandSlot::off_at(1000); EQ_MAX_BANDS]; EQ_TARGETS];
        for (i, slot) in bands.iter_mut().flatten().enumerate() {
            let o = off::BANDS + i * BAND_SIZE;
            *slot = BandSlot::from_bytes(b[o..o + BAND_SIZE].try_into().expect("8-byte slice"));
        }
        Ok(Self {
            version: b[off::VERSION],
            flags: b[off::FLAGS],
            color: b[off::COLOR],
            reserved0: b[off::RESERVED0],
            name,
            sub_level_cdb: get_u16(b, off::SUB_LEVEL) as i16,
            crossover_hz: get_u16(b, off::CROSSOVER),
            aux_gain_cdb: get_u16(b, off::AUX_GAIN) as i16,
            trim_cdb,
            bass_milli: get_u16(b, off::BASS) as i16,
            reserved1: [b[off::RESERVED1], b[off::RESERVED1 + 1]],
            bands,
        })
    }
}

/// Byte ranges where `to` differs from `from`, each at most `max` bytes long (`max` ≥ 16).
///
/// The header fields (bytes 0..40) give at most one range per `max` bytes, from the first to the last differing
/// byte. The band area is compared by pairs of slots (16 bytes), consecutive differing pairs sharing a range.
/// The pairing dates from raw biquad pairs (removed in DSP profile protocol 6); the firmware no longer needs it.
pub fn write_ranges(from: &Profile, to: &Profile, max: usize) -> Vec<Range<usize>> {
    const PAIR: usize = 2 * BAND_SIZE;
    assert!(max >= PAIR, "an OPW must hold a whole raw pair");
    let (a, b) = (from.to_bytes(), to.to_bytes());
    let mut out = Vec::new();
    let lo = (0..off::BANDS).find(|&i| a[i] != b[i]);
    let hi = (0..off::BANDS).rev().find(|&i| a[i] != b[i]);
    if let (Some(lo), Some(hi)) = (lo, hi) {
        let mut start = lo;
        while start <= hi {
            let end = (start + max).min(hi + 1);
            out.push(start..end);
            start = end;
        }
    }
    let per_range = max / PAIR;
    let mut run: Option<Range<usize>> = None;
    for p in (off::BANDS..PROFILE_SIZE).step_by(PAIR) {
        let differs = a[p..p + PAIR] != b[p..p + PAIR];
        run = match (run, differs) {
            (Some(r), true) if (r.end - r.start) / PAIR < per_range => Some(r.start..p + PAIR),
            (Some(r), true) => {
                out.push(r);
                Some(p..p + PAIR)
            }
            (None, true) => Some(p..p + PAIR),
            (Some(r), false) => {
                out.push(r);
                None
            }
            (None, false) => None,
        };
    }
    out.extend(run);
    out
}
