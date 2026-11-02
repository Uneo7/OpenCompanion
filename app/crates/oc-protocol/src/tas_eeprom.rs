//! TAS EEPROM (U6002, AT24C64, I2C 0x50) access through the stock Bose TAS firmware.
//! Read: request `t [addr H][addr L][n]`; response `t` + these 3 bytes + n bytes.
//! Write: request `T [addr H][addr L][n][n bytes]`; response `T` + the same 3 bytes (+ data).
//! Writes must stay inside one 32-byte EEPROM page. Only higher-level, guarded commands should call
//! [`write_block`] (a bad header makes the speaker boot in DFU mode or not at all).

use std::time::Duration;

use crate::link::Link;
use crate::{Error, Result};

pub const SIZE: usize = 8192;
const CHUNK: usize = 32;
pub const PAGE: usize = 32;

pub fn read_block<L: Link + ?Sized>(link: &L, addr: u16, n: u8) -> Result<Vec<u8>> {
    let req = [(addr >> 8) as u8, addr as u8, n];
    let (cmd, data) = link.transact(b't', &req, Duration::from_millis(500))?;
    if data.starts_with(b"ERROR") {
        return Err(Error::Relay(format!("EEPROM read failed at 0x{addr:04x}")));
    }
    if cmd != b't' || data.len() < 3 + n as usize || data[..3] != req {
        return Err(Error::Unexpected(format!("inconsistent 't' response at 0x{addr:04x}")));
    }
    Ok(data[3..3 + n as usize].to_vec())
}

pub fn dump<L: Link + ?Sized>(link: &L, mut progress: impl FnMut(usize, usize)) -> Result<Vec<u8>> {
    let mut out = Vec::with_capacity(SIZE);
    for addr in (0..SIZE).step_by(CHUNK) {
        let n = CHUNK.min(SIZE - addr) as u8;
        out.extend(read_block(link, addr as u16, n)?);
        progress(out.len(), SIZE);
    }
    Ok(out)
}

/// Writes `data` at `addr`. The range must fit in one 32-byte page. Waits for the EEPROM write cycle.
pub fn write_block<L: Link + ?Sized>(link: &L, addr: u16, data: &[u8]) -> Result<()> {
    let start = addr as usize;
    if data.is_empty() || start / PAGE != (start + data.len() - 1) / PAGE {
        return Err(Error::Unexpected(format!("write 0x{addr:04x}+{} crosses a {PAGE}-byte page", data.len())));
    }
    let mut req = vec![(addr >> 8) as u8, addr as u8, data.len() as u8];
    req.extend_from_slice(data);
    let (cmd, resp) = link.transact(b'T', &req, Duration::from_millis(500))?;
    if resp.starts_with(b"ERROR") {
        return Err(Error::Relay(format!("EEPROM write failed at 0x{addr:04x}")));
    }
    if cmd != b'T' || resp.len() < 3 || resp[..3] != req[..3] {
        return Err(Error::Unexpected(format!("inconsistent 'T' response at 0x{addr:04x}")));
    }
    link.delay(Duration::from_millis(10)); // AT24C64 write cycle: 5 ms max
    Ok(())
}
