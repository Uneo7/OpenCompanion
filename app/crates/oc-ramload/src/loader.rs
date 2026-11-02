//! Loader protocol, carried by the TAS `E` command (`firmware/dsp/README.md`, RAM loader).

use std::time::Duration;

use oc_protocol::Link;

use crate::{fail, Error, LoadImage, Result};

const ACK: u8 = 0x06;
const LD_INFO: u8 = 0;
const LD_WRITE: u8 = 1;
const LD_CRC: u8 = 2;
const LD_JUMP: u8 = 3;
const LD_FILL: u8 = 5;
/// 1 + 4 + 56 = 61 bytes, the TAS relay limit.
pub const WRITE_CHUNK: usize = 56;
pub const CRC_CHUNK: usize = 0x10000;

/// WRITE data size for this link (56 on the stock TAS, less through the OpenCompanion DSP relay).
pub fn write_chunk<L: Link + ?Sized>(link: &L) -> usize {
    WRITE_CHUNK.min(link.max_payload().saturating_sub(5)).max(4)
}

fn call<L: Link + ?Sized>(link: &L, payload: &[u8], retries: usize) -> Result<Vec<u8>> {
    let mut last = String::from("no attempt");
    for _ in 0..retries {
        match link.transact(b'E', payload, Duration::from_millis(1000)) {
            Err(e) => last = e.to_string(),
            Ok((_, data)) if data.starts_with(b"ERROR") => {
                last = format!("TAS relay error {}", String::from_utf8_lossy(&data));
            }
            Ok((_, data)) => {
                return match data.split_first() {
                    Some((&ACK, rest)) => Ok(rest.to_vec()),
                    _ => fail(format!("loader refused command 0x{:02x}: {data:02x?}", payload[0])),
                };
            }
        }
    }
    fail(format!("no valid answer from the loader: {last}"))
}

/// Asks the running stub for its version (`INFO` → `OCL` + version).
pub fn probe<L: Link + ?Sized>(link: &L) -> Result<u8> {
    for _ in 0..10 {
        if let Ok(info) = call(link, &[LD_INFO], 1) {
            if info.len() >= 4 && &info[..3] == b"OCL" {
                return Ok(info[3]);
            }
        }
        link.delay(Duration::from_millis(200));
    }
    fail("the loader does not answer")
}

pub fn load_image<L: Link + ?Sized>(link: &L, img: &LoadImage, mut log: impl FnMut(String)) -> Result<()> {
    let chunk_len = write_chunk(link);
    for (addr, data) in &img.segments {
        for (i, chunk) in data.chunks(chunk_len).enumerate() {
            let mut req = vec![LD_WRITE];
            req.extend_from_slice(&(addr + (i * chunk_len) as u32).to_le_bytes());
            req.extend_from_slice(chunk);
            call(link, &req, 3)?;
        }
        for (i, part) in data.chunks(CRC_CHUNK).enumerate() {
            let at = addr + (i * CRC_CHUNK) as u32;
            let mut req = vec![LD_CRC];
            req.extend_from_slice(&at.to_le_bytes());
            req.extend_from_slice(&(part.len() as u32).to_le_bytes());
            let resp = call(link, &req, 3)?;
            let got = resp.get(..4).map(|b| u32::from_le_bytes(b.try_into().unwrap()));
            if got != Some(crc32fast::hash(part)) {
                return Err(Error::Load(format!("CRC mismatch at 0x{at:08x}")));
            }
        }
        log(format!("  segment 0x{addr:08x} +0x{:x} written, CRC OK", data.len()));
    }
    for (addr, n, byte) in &img.fills {
        let mut req = vec![LD_FILL];
        req.extend_from_slice(&addr.to_le_bytes());
        req.extend_from_slice(&n.to_le_bytes());
        req.push(*byte);
        call(link, &req, 3)?;
        log(format!("  fill 0x{addr:08x} +0x{n:x} = 0x{byte:02x}"));
    }
    let mut req = vec![LD_JUMP];
    req.extend_from_slice(&img.entry.to_le_bytes());
    call(link, &req, 3)?;
    log(format!("  jump to 0x{:08x} requested", img.entry));
    Ok(())
}
