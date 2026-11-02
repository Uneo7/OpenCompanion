use std::path::Path;

use crate::{fail, Result};

#[derive(Debug, Clone, Default)]
pub struct LoadImage {
    pub segments: Vec<(u32, Vec<u8>)>,
    pub fills: Vec<(u32, u32, u8)>,
    pub entry: u32,
}

impl LoadImage {
    pub fn check_ranges(&self, lo: u32, hi: u32, forbid: Option<(u32, u32)>) -> Result<()> {
        let overlaps = |a: u32, n: u32| forbid.is_some_and(|(f0, f1)| a < f1 && a.saturating_add(n) > f0);
        for (a, data) in &self.segments {
            let n = data.len() as u32;
            if *a < lo || a.saturating_add(n) > hi {
                return fail(format!("segment 0x{a:08x}+0x{n:x} outside 0x{lo:08x}..0x{hi:08x}"));
            }
            if overlaps(*a, n) {
                return fail(format!("segment 0x{a:08x}+0x{n:x} overlaps the loader area"));
            }
        }
        for (a, n, _) in &self.fills {
            if *a < lo || a.saturating_add(*n) > hi || overlaps(*a, *n) {
                return fail(format!("fill 0x{a:08x}+0x{n:x} not allowed"));
            }
        }
        if !(lo..hi).contains(&self.entry) {
            return fail(format!("entry point 0x{:08x} outside 0x{lo:08x}..0x{hi:08x}", self.entry));
        }
        Ok(())
    }
}

/// Reads an ELF (by physical load address, PT_LOAD only) or an AIS image.
pub fn read_image(path: &Path) -> Result<LoadImage> {
    let data = std::fs::read(path)?;
    if data.starts_with(b"TIPA") {
        read_ais(&data)
    } else {
        read_elf(&data, path)
    }
}

/// Minimal ELF32 little-endian reader.
fn read_elf(d: &[u8], path: &Path) -> Result<LoadImage> {
    if d.len() < 0x34 || &d[..4] != b"\x7fELF" || d[4] != 1 || d[5] != 1 {
        return fail(format!("{}: not an ELF32 little-endian file", path.display()));
    }
    let u16_at = |o: usize| u16::from_le_bytes([d[o], d[o + 1]]);
    let u32_at = |o: usize| u32::from_le_bytes(d[o..o + 4].try_into().unwrap());
    let entry = u32_at(0x18);
    let phoff = u32_at(0x1C) as usize;
    let (phentsize, phnum) = (u16_at(0x2A) as usize, u16_at(0x2C) as usize);
    let mut img = LoadImage { entry, ..Default::default() };
    for k in 0..phnum {
        let o = phoff + k * phentsize;
        if o + 24 > d.len() {
            return fail("truncated ELF program header table");
        }
        let (p_type, p_offset, p_paddr, p_filesz, p_memsz) =
            (u32_at(o), u32_at(o + 4) as usize, u32_at(o + 12), u32_at(o + 16), u32_at(o + 20));
        if p_type != 1 || p_memsz == 0 {
            continue; // PT_LOAD only
        }
        if p_filesz > 0 {
            let end = p_offset + p_filesz as usize;
            if end > d.len() {
                return fail("truncated ELF segment");
            }
            img.segments.push((p_paddr, d[p_offset..end].to_vec()));
        }
        if p_memsz > p_filesz {
            img.fills.push((p_paddr + p_filesz, p_memsz - p_filesz, 0));
        }
    }
    Ok(img)
}

fn read_ais(data: &[u8]) -> Result<LoadImage> {
    use oc_ais::Command;
    let ais = oc_ais::parse(data)?;
    let mut img = LoadImage { entry: ais.entry().unwrap_or(0), ..Default::default() };
    let mut skipped = false;
    for c in &ais.commands {
        match c {
            Command::SectionLoad(s) | Command::BoseZlib(s) => img.segments.push((s.addr, s.data.clone())),
            Command::SectionFill { addr, size, pattern, .. } => {
                let b = pattern.to_le_bytes();
                if b.iter().any(|x| *x != b[0]) {
                    return fail(format!("fill 0x{addr:08x}: pattern 0x{pattern:08x} is not a repeated byte"));
                }
                img.fills.push((*addr, *size, b[0]));
            }
            Command::Jump(_) | Command::Set { .. } => skipped = true,
            _ => {}
        }
    }
    if skipped {
        eprintln!("  note: AIS 'Jump' (type 5) and 'Set' (type 7) records are not replayed by the RAM loader");
    }
    Ok(img)
}
