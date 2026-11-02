use object::{Object, ObjectSymbol};

use super::layout::{APP_RAM, BOOT0, BOOT1, BOOT1_CODE_MAX, RAM_BOOT1, RAM_TRAMP, TRAMP_MAX};
use crate::format::{Segment, MAGIC, OP_JUMP_CLOSE, OP_SECTION_LOAD};
use crate::{segments_from_elf, Error, Result};

/// Tags stored in `boot_tag` by each build (`boot1/src/main.c`, `updater/src/main.c`).
pub const TAG_BOOT1: &[u8; 8] = b"OCBOOT1\0";
pub const TAG_UPDATER: &[u8; 8] = b"OCUPDT1\0";
/// Symbol an application ELF must define to prove it uses this EEPROM layout (profiles at 0xB700, writes
/// limited to the window): an older firmware would overwrite boot1 with its profiles.
pub const APP_LAYOUT_SYMBOL: &str = "oc_eeprom_layout_v2";

fn bad(msg: impl Into<String>) -> Error {
    Error::Layout(msg.into())
}

pub(crate) fn symbol(elf: &[u8], name: &str) -> Result<Option<u32>> {
    let file = object::File::parse(elf).map_err(|e| Error::Elf(e.to_string()))?;
    Ok(file.symbols().find(|s| s.name() == Ok(name)).map(|s| s.address() as u32))
}

fn flatten(segs: &[Segment], base: u32, len: usize, fill: u8) -> Result<Vec<u8>> {
    let mut out = vec![fill; len];
    for s in segs {
        let off = s.addr.checked_sub(base).map(|o| o as usize).filter(|o| o + s.data.len() <= len);
        let off = off.ok_or_else(|| bad(format!("segment 0x{:08x}+0x{:x} outside 0x{base:08x}+0x{len:x}", s.addr, s.data.len())))?;
        out[off..off + s.data.len()].copy_from_slice(&s.data);
    }
    Ok(out)
}

fn check_tag(elf: &[u8], flat: &[u8], base: u32, want: &[u8; 8]) -> Result<()> {
    let at = symbol(elf, "boot_tag")?.ok_or_else(|| bad("no boot_tag symbol"))?;
    let off = at.checked_sub(base).map(|o| o as usize).filter(|o| o + 8 <= flat.len()).ok_or_else(|| bad("boot_tag outside the image"))?;
    if &flat[off..off + 8] != want {
        return Err(bad(format!("wrong build: boot_tag {:?}, expected {:?}", String::from_utf8_lossy(&flat[off..off + 8]), String::from_utf8_lossy(want))));
    }
    Ok(())
}

/// boot0 (256 bytes): AIS magic, Section Load of the trampoline at RAM_TRAMP, Jump_Close RAM_TRAMP, 0xFF.
pub fn boot0_from_elf(tramp_elf: &[u8]) -> Result<[u8; 256]> {
    let (segs, _, entry) = segments_from_elf(tramp_elf)?;
    if entry != RAM_TRAMP || segs.len() != 1 || segs[0].addr != RAM_TRAMP {
        return Err(bad("trampoline must be one segment at 0x1002AF00 with its entry there"));
    }
    let mut code = segs[0].data.clone();
    while code.len() >= 4 && code[code.len() - 4..] == [0; 4] {
        code.truncate(code.len() - 4); // fetch-packet padding (NOP words), never executed
    }
    if code.len() > TRAMP_MAX || code.len() % 4 != 0 {
        return Err(bad(format!("trampoline is {} bytes, boot0 allows {TRAMP_MAX}", code.len())));
    }
    let mut out = [0xFFu8; 256];
    let mut v = Vec::with_capacity(256);
    for w in [MAGIC, OP_SECTION_LOAD, RAM_TRAMP, code.len() as u32] {
        v.extend_from_slice(&w.to_le_bytes());
    }
    v.extend_from_slice(&code);
    for w in [OP_JUMP_CLOSE, RAM_TRAMP] {
        v.extend_from_slice(&w.to_le_bytes());
    }
    debug_assert!(v.len() <= BOOT0.len());
    out[..v.len()].copy_from_slice(&v);
    Ok(out)
}

/// boot1 area content (0x2F80 bytes): code/const at RAM_BOOT1, 0xFF up to the last 4 bytes, CRC-32.
pub fn boot1_from_elf(boot1_elf: &[u8]) -> Result<Vec<u8>> {
    let (segs, _, entry) = segments_from_elf(boot1_elf)?;
    if entry != RAM_BOOT1 {
        return Err(bad(format!("boot1 entry 0x{entry:08x}, expected 0x{RAM_BOOT1:08x}")));
    }
    let mut flat = flatten(&segs, RAM_BOOT1, BOOT1_CODE_MAX, 0xFF)?;
    check_tag(boot1_elf, &flat, RAM_BOOT1, TAG_BOOT1)?;
    let crc = crc32fast::hash(&flat);
    flat.extend_from_slice(&crc.to_le_bytes());
    debug_assert_eq!(flat.len(), BOOT1.len());
    Ok(flat)
}

pub fn boot1_used(boot1_elf: &[u8]) -> Result<usize> {
    let (segs, _, _) = segments_from_elf(boot1_elf)?;
    Ok(segs.iter().map(|s| (s.addr - RAM_BOOT1) as usize + s.data.len()).max().unwrap_or(0))
}

pub fn updater_from_elf(elf: &[u8]) -> Result<(Vec<Segment>, u32)> {
    let (segs, _, entry) = segments_from_elf(elf)?;
    for s in &segs {
        if s.addr < APP_RAM.start || s.addr as u64 + s.data.len() as u64 > APP_RAM.end as u64 {
            return Err(Error::OutOfRange { addr: s.addr, len: s.data.len() });
        }
    }
    let lo = segs.iter().map(|s| s.addr).min().unwrap_or(APP_RAM.start);
    let hi = segs.iter().map(|s| s.addr + s.data.len() as u32).max().unwrap_or(lo);
    let flat = flatten(&segs, lo, (hi - lo) as usize, 0)?;
    check_tag(elf, &flat, lo, TAG_UPDATER)?;
    Ok((segs, entry))
}

pub fn app_declares_layout(elf: &[u8]) -> Result<bool> {
    Ok(symbol(elf, APP_LAYOUT_SYMBOL)?.is_some())
}
