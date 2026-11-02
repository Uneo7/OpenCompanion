//! The stock DSP RAM after start-up, rebuilt from the user's own U7001 backup (no Bose byte is bundled):
//! AIS image (raw and zlib sections) flattened at 0x10000000, then the TI `.cinit` table applied, exactly as
//! `re/dsp/build_dis.py` builds `out/rt.bin`. Addresses are those of the stock firmware D01.01.01.

use oc_ais::eeprom::layout::BOOT1;
use oc_install::backup::Content;

use super::ResponseError;

pub const BASE: u32 = 0x1000_0000;
/// `.cinit` records `[length][address][data, padded to 8]` of the stock image.
const CINIT: (u32, u32) = (0x1001_DB40, 0x1002_4E40);

#[derive(Debug, Clone)]
pub struct Ram {
    bytes: Vec<u8>,
}

fn bad(s: impl Into<String>) -> ResponseError {
    ResponseError::Image(s.into())
}

impl Ram {
    pub fn from_stock_dump(dsp: &[u8]) -> Result<Self, ResponseError> {
        if Content::of_dsp(dsp) != Content::Stock {
            return Err(bad("the DSP backup does not hold the stock Bose firmware"));
        }
        let img = oc_ais::parse(&dsp[..BOOT1.start]).map_err(|e| bad(format!("stock image: {e}")))?;
        let flat = img.flatten(BASE);
        let mut rt = flat.clone();
        let raw = Self { bytes: flat };
        let mut a = CINIT.0;
        while a < CINIT.1 {
            let (n, dst) = (raw.u(a)?, raw.u(a + 4)?);
            let src = raw.slice(a + 8, n)?;
            if dst >= BASE && ((dst - BASE) as usize) + src.len() <= rt.len() {
                let o = (dst - BASE) as usize;
                rt[o..o + src.len()].copy_from_slice(src);
            }
            a = a.checked_add(8 + ((n + 7) & !7)).ok_or_else(|| bad(".cinit table overflow"))?;
        }
        Ok(Self { bytes: rt })
    }

    fn slice(&self, a: u32, n: u32) -> Result<&[u8], ResponseError> {
        let o = a.checked_sub(BASE).ok_or_else(|| bad(format!("address 0x{a:08x} below the image")))? as usize;
        self.bytes.get(o..o + n as usize).ok_or_else(|| bad(format!("0x{a:08x}+{n} outside the image")))
    }

    pub fn u(&self, a: u32) -> Result<u32, ResponseError> {
        let b = self.slice(a, 4)?;
        Ok(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    }

    /// The stock graph works in single precision.
    pub fn f(&self, a: u32) -> Result<f64, ResponseError> {
        Ok(f64::from(f32::from_bits(self.u(a)?)))
    }

    pub fn floats(&self, a: u32, n: usize) -> Result<Vec<f64>, ResponseError> {
        (0..n as u32).map(|k| self.f(a + 4 * k)).collect()
    }
}
