use std::time::Duration;

use oc_protocol::dsp::DspConsole;
use oc_protocol::Link;

use crate::{fail, Result, HIJACK_MODULE, HIJACK_ORIGINAL, LOADER_HI, LOADER_LO, RAM_HI, RAM_LO};

fn words_of(data: &[u8]) -> Vec<u32> {
    data.chunks(4)
        .map(|c| {
            let mut b = [0u8; 4];
            b[..c.len()].copy_from_slice(c);
            u32::from_le_bytes(b)
        })
        .collect()
}

/// Flat RAM image (0x10000000..0x10030000) of the stock DSP firmware as loaded at boot (segments only),
/// from the stock AIS image (`u7001_dsp.bin`).
pub fn stock_ram(stock_ais: &[u8]) -> Result<Vec<u8>> {
    let ais = oc_ais::parse(stock_ais)?;
    let mut flat = vec![0u8; (RAM_HI - RAM_LO) as usize];
    for s in ais.segments() {
        if (RAM_LO..RAM_HI).contains(&s.addr) {
            let off = (s.addr - RAM_LO) as usize;
            let end = (off + s.data.len()).min(flat.len());
            flat[off..end].copy_from_slice(&s.data[..end - off]);
        }
    }
    Ok(flat)
}

/// The loader area must still hold the stock `.cinit` table, sampled twice one second apart.
///
/// The table is only read by `c_int00` at boot; an exact match proves the firmware is the expected stock one and
/// that nothing at runtime uses this area. `stock` is the output of [`stock_ram`].
pub fn check_area_free<L: Link + ?Sized>(console: &DspConsole<L>, stock: &[u8]) -> Result<()> {
    for attempt in 0..2 {
        for addr in (LOADER_LO..LOADER_HI).step_by(0x100) {
            let off = (addr - RAM_LO) as usize;
            let want: Vec<u32> = stock[off..off + 16].chunks(4).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect();
            let got = console.read_words(addr, 4)?;
            if got != want {
                return fail(format!(
                    "loader area does not match the stock .cinit table at 0x{addr:08x}: {got:08x?} != {want:08x?}"
                ));
            }
        }
        if attempt == 0 {
            console.link().delay(Duration::from_secs(1));
        }
    }
    Ok(())
}

pub fn install_stub<L: Link + ?Sized>(console: &DspConsole<L>, segments: &[(u32, Vec<u8>)], mut log: impl FnMut(String)) -> Result<()> {
    for (addr, data) in segments {
        let words = words_of(data);
        for (i, chunk) in words.chunks(5).enumerate() {
            console.write_words(addr + 20 * i as u32, chunk)?;
        }
        for (i, chunk) in words.chunks(4).enumerate() {
            let at = addr + 16 * i as u32;
            if console.read_words(at, chunk.len())? != chunk {
                return fail(format!("stub verify failed at 0x{at:08x}"));
            }
        }
        log(format!("  stub segment 0x{addr:08x} +0x{:x} written and verified", data.len()));
    }
    Ok(())
}

/// Redirects the output meter's process pointer to the stub entry, after checking its signature (D01.01.01).
pub fn hijack<L: Link + ?Sized>(console: &DspConsole<L>, entry: u32, log: impl FnMut(String)) -> Result<()> {
    hijack_with(console, HIJACK_MODULE, HIJACK_ORIGINAL, entry, log)
}

/// [`hijack`] for a given stock revision: `module` (flags word at +16, process pointer at +20) must read
/// flags 3 and process `original` before the pointer is written.
pub fn hijack_with<L: Link + ?Sized>(
    console: &DspConsole<L>,
    module: u32,
    original: u32,
    entry: u32,
    mut log: impl FnMut(String),
) -> Result<()> {
    let v = console.read_words(module + 16, 2)?;
    let (flags, slot) = (v[0], v[1]);
    if flags != 0x3 || slot != original {
        return fail(format!(
            "unexpected hijack target 0x{module:08x}: flags 0x{flags:x}, process 0x{slot:08x} (expected 3 / \
             0x{original:08x})"
        ));
    }
    // The stub may take over before the console answers.
    if let Err(e) = console.write_words(module + 20, &[entry]) {
        log(format!("  (no clean answer to the hijack write, expected: {e})"));
    }
    Ok(())
}
