//! U7001 read through the stock firmware's raw EEPROM read: HID `d` relayed by the stock TAS to the DSP
//! (`[address H][address L][n]`, routine 0x10016A80, 1..128 bytes; `re/dsp/notes/eeprom_safety.md` §5).
//! Works on any stock revision without the RAM-loader hijack, and writes nothing.
//!
//! The command byte is a constant of this module: no caller can choose it (`D` is the raw UNPROTECTED write).

use std::time::Duration;

use oc_ais::eeprom::layout::EE_SIZE;
use oc_protocol::Link;

use crate::error::{Error, Result};
use crate::host::Host;

/// Raw EEPROM READ of the stock firmware. Never anything else here.
const RAW_READ: u8 = b'd';
/// Bytes per request: the reply must fit one 64-byte HID report with its header (the TAS answers `ERROR4`
/// beyond 64 bytes).
const CHUNK: usize = 32;
const TIMEOUT: Duration = Duration::from_millis(600);
const TRIES: usize = 3;

fn request(link: &dyn Link, addr: usize) -> Result<Vec<u8>> {
    let req = [(addr >> 8) as u8, addr as u8, CHUNK as u8];
    let (cmd, d) = link.transact(RAW_READ, &req, TIMEOUT)?;
    if cmd != RAW_READ {
        return Err(Error::Device(format!("raw read at 0x{addr:04x}: reply '{}' {d:02x?}", cmd as char)));
    }
    match d.len() {
        CHUNK => Ok(d),
        // Echo of the request first (as the TAS `t` read does).
        n if n == CHUNK + 3 && d[..3] == req => Ok(d[3..].to_vec()),
        _ => Err(Error::Device(format!("raw read at 0x{addr:04x}: unexpected reply {d:02x?}"))),
    }
}

fn chunk(link: &dyn Link, addr: usize) -> Result<Vec<u8>> {
    let mut last = None;
    for _ in 0..TRIES {
        match request(link, addr) {
            Ok(d) => return Ok(d),
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("at least one try"))
}

fn read_once(link: &dyn Link, pass: usize, host: &mut dyn Host) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(EE_SIZE);
    for a in (0..EE_SIZE).step_by(CHUNK) {
        out.extend(chunk(link, a)?);
        if (a + CHUNK) % 4096 == 0 {
            host.log(&format!("  U7001 raw read {}: {}/{} pages", pass + 1, out.len() / 128, EE_SIZE / 128));
        }
    }
    Ok(out)
}

/// The first request tells whether this revision answers `d` at all (`None`: use the RAM loader instead).
pub fn available(link: &dyn Link) -> bool {
    chunk(link, 0).is_ok()
}

/// U7001 read twice with the stock raw read; refused when the two reads (and their CRC-32) differ.
pub fn read_u7001(link: &dyn Link, host: &mut dyn Host) -> Result<Vec<u8>> {
    host.log("backup: reading U7001 twice with the stock raw EEPROM read (no RAM loader, nothing written)…");
    let a = read_once(link, 0, host)?;
    let b = read_once(link, 1, host)?;
    let (ca, cb) = (crc32fast::hash(&a), crc32fast::hash(&b));
    if a != b || ca != cb {
        return Err(Error::Verify(format!(
            "the two raw reads of U7001 differ (CRC-32 0x{ca:08x} / 0x{cb:08x}): nothing saved"
        )));
    }
    host.log(&format!("backup: U7001 read twice, identical (CRC-32 0x{ca:08x})"));
    Ok(a)
}
