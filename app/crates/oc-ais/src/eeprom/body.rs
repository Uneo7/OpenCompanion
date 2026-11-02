//! Image body, read by boot1 (never by the ROM): `AIS magic, LZ4 section*, Jump_Close entry`.
//! LZ4 section = `0x58535980, address, raw size, compressed size, LZ4 block padded to 4 bytes`. Every ELF
//! segment and every zero fill (.bss / .far) is stored as an LZ4 section, so boot1 needs a single opcode.

use super::layout::{APP_RAM, BODY};
use super::lz4;
use crate::format::{Segment, MAGIC, OP_JUMP_CLOSE};
use crate::{Error, Result};

pub const OP_OC_LZ4_LOAD: u32 = 0x5853_5980;

#[derive(Debug, Clone)]
pub struct Body {
    pub bytes: Vec<u8>,
    pub entry: u32,
    /// Bytes written to RAM by boot1 (sections and zero fills).
    pub raw_len: u32,
}

fn in_app_ram(addr: u32, len: usize) -> bool {
    addr >= APP_RAM.start && (addr as u64 + len as u64) <= APP_RAM.end as u64
}

fn push(out: &mut Vec<u8>, v: u32) {
    out.extend_from_slice(&v.to_le_bytes());
}

fn section(out: &mut Vec<u8>, addr: u32, data: &[u8]) -> Result<()> {
    if !in_app_ram(addr, data.len()) {
        return Err(Error::OutOfRange { addr, len: data.len() });
    }
    let c = lz4::compress(data);
    for v in [OP_OC_LZ4_LOAD, addr, data.len() as u32, c.len() as u32] {
        push(out, v);
    }
    out.extend_from_slice(&c);
    out.resize(out.len() + (4 - c.len() % 4) % 4, 0);
    Ok(())
}

pub fn build(segments: &[Segment], zero_fills: &[(u32, u32)], entry: u32) -> Result<Body> {
    let mut out = Vec::new();
    let mut raw_len = 0u32;
    push(&mut out, MAGIC);
    for s in segments {
        section(&mut out, s.addr, &s.data)?;
        raw_len += s.data.len() as u32;
    }
    for &(addr, size) in zero_fills {
        section(&mut out, addr, &vec![0u8; size as usize])?;
        raw_len += size;
    }
    if !in_app_ram(entry, 4) {
        return Err(Error::OutOfRange { addr: entry, len: 4 });
    }
    push(&mut out, OP_JUMP_CLOSE);
    push(&mut out, entry);
    if out.len() > BODY.len() {
        return Err(Error::Layout(format!("image body is {} bytes, the body area holds {}", out.len(), BODY.len())));
    }
    Ok(Body { bytes: out, entry, raw_len })
}

#[derive(Debug, Clone, Default)]
pub struct Loaded {
    pub sections: Vec<Segment>,
    pub entry: u32,
}

/// Interprets a body exactly like boot1 (`boot1/src/image.c`): same opcodes, range checks, exact length.
pub fn parse(body: &[u8]) -> std::result::Result<Loaded, String> {
    let mut p = 0usize;
    let word = |p: &mut usize| -> std::result::Result<u32, String> {
        let b = body.get(*p..*p + 4).ok_or_else(|| format!("truncated at body offset 0x{:x}", *p))?;
        *p += 4;
        Ok(u32::from_le_bytes(b.try_into().unwrap()))
    };
    if word(&mut p)? != MAGIC {
        return Err("body does not start with the AIS magic".into());
    }
    let mut l = Loaded::default();
    loop {
        let at = p;
        match word(&mut p)? {
            OP_OC_LZ4_LOAD => {
                let (addr, raw, comp) = (word(&mut p)?, word(&mut p)? as usize, word(&mut p)? as usize);
                if !in_app_ram(addr, raw) {
                    return Err(format!("section 0x{addr:08x}+0x{raw:x} outside application RAM"));
                }
                let c = body.get(p..p + comp).ok_or_else(|| format!("truncated LZ4 data at body offset 0x{p:x}"))?;
                let (data, used) = lz4::decompress(c, raw).ok_or_else(|| format!("bad LZ4 data at body offset 0x{at:x}"))?;
                if used != comp {
                    return Err(format!("LZ4 section at body offset 0x{at:x}: {used} of {comp} bytes used"));
                }
                p += comp + (4 - comp % 4) % 4;
                l.sections.push(Segment { addr, data });
            }
            OP_JUMP_CLOSE => {
                l.entry = word(&mut p)?;
                break;
            }
            op => return Err(format!("opcode 0x{op:08x} not accepted by boot1 at body offset 0x{at:x}")),
        }
    }
    if p != body.len() {
        return Err(format!("{} bytes after Jump_Close (boot1 requires the exact length)", body.len() as isize - p as isize));
    }
    Ok(l)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn build_and_parse() {
        let code: Vec<u8> = (0..3000u32).map(|i| (i % 37) as u8).collect();
        let segs = vec![Segment { addr: 0x1000_1000, data: code }, Segment { addr: 0x1000_8000, data: vec![1, 2, 3] }];
        let b = build(&segs, &[(0x1000_9000, 64)], 0x1000_1000).unwrap();
        let l = parse(&b.bytes).unwrap();
        assert_eq!(&l.sections[..2], &segs[..]);
        assert_eq!(l.sections[2], Segment { addr: 0x1000_9000, data: vec![0; 64] });
        assert_eq!(l.entry, 0x1000_1000);
        assert_eq!(b.bytes.len() % 4, 0);
        assert_eq!(b.raw_len, 3000 + 3 + 64);
    }

    #[test]
    fn refuses_boot_area() {
        let segs = vec![Segment { addr: 0x1002_B000, data: vec![0; 4] }];
        assert!(build(&segs, &[], 0x1000_1000).is_err());
    }
}
