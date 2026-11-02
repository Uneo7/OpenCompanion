//! Protocol: `firmware/dsp/boot/docs/RESCUE.md`. Little-endian; replies `06 …` (ACK) or `15 [code]` (NAK).

use std::time::Duration;

use oc_ais::eeprom::layout::EE_PAGE;
use oc_protocol::Link;

use crate::error::{Error, Result};

const TIMEOUT: Duration = Duration::from_millis(1000);
const RETRIES: usize = 3;

const RC_INFO: u8 = 0x00;
const RC_WRITE: u8 = 0x01;
const RC_CRC: u8 = 0x02;
const RC_REBOOT: u8 = 0x06;
const RC_EE_READ: u8 = 0x10;
const RC_EE_WRITE: u8 = 0x11;
const RC_EE_CRC: u8 = 0x12;
const RC_EE_UNLOCK_BOOT: u8 = 0x13;
const RC_EE_WRITE_BOOT0: u8 = 0x14;

/// Largest EEPROM range per `EE_CRC`. The stock TAS reads the `E` reply 20 ms after the request and the
/// updater does not service I2C0 while it reads U7001 (~23 µs/byte at ~394 kHz): 256 bytes ≈ 6 ms.
/// [HW] not measured; keep well below ~600 bytes.
pub const MAX_EE_CRC: usize = 256;
/// Data bytes per `EE_WRITE` / `EE_READ` (updater limits).
const EE_WRITE_MAX: usize = 56;
const EE_READ_MAX: usize = 60;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Reply {
    Ack(Vec<u8>),
    /// NAK code: 1 refused, 2 I2C error, 3 read-back mismatch, 4 staged-data CRC mismatch (0 = no code).
    Nak(u8),
}

fn nak_text(code: u8) -> &'static str {
    match code {
        1 => "refused (range / window / locked)",
        2 => "I2C error",
        3 => "read-back mismatch",
        4 => "staged data CRC mismatch",
        _ => "refused",
    }
}

pub struct Updater<'a> {
    link: &'a dyn Link,
}

impl<'a> Updater<'a> {
    pub fn new(link: &'a dyn Link) -> Self {
        Self { link }
    }

    pub fn link(&self) -> &'a dyn Link {
        self.link
    }

    /// One request; transport errors are retried (every command is idempotent).
    pub fn exchange(&self, payload: &[u8]) -> Result<Reply> {
        let mut last = None;
        for _ in 0..RETRIES {
            match self.link.transact(b'E', payload, TIMEOUT) {
                Ok((_, d)) if d.starts_with(b"ERROR") => {
                    last = Some(Error::Device(format!("TAS relay error {}", String::from_utf8_lossy(&d))))
                }
                Ok((_, d)) => {
                    return match d.split_first() {
                        Some((0x06, rest)) => Ok(Reply::Ack(rest.to_vec())),
                        Some((0x15, rest)) => Ok(Reply::Nak(rest.first().copied().unwrap_or(0))),
                        _ => Err(Error::Device(format!("updater reply {d:02x?} to command 0x{:02x}", payload[0]))),
                    };
                }
                Err(e) => last = Some(e.into()),
            }
        }
        Err(last.expect("at least one attempt"))
    }

    fn ack(&self, payload: &[u8], what: &str) -> Result<Vec<u8>> {
        match self.exchange(payload)? {
            Reply::Ack(d) => Ok(d),
            Reply::Nak(c) => Err(Error::Device(format!("{what}: updater NAK {c} ({})", nak_text(c)))),
        }
    }

    /// `(mode, reason)`: mode `U` for the updater, `R` for rescue.
    pub fn info(&self) -> Result<(u8, u8)> {
        let d = self.ack(&[RC_INFO], "INFO")?;
        if d.len() < 7 || &d[..3] != b"OCL" {
            return Err(Error::Device(format!("INFO reply {d:02x?}")));
        }
        Ok((d[4], d[5]))
    }

    pub fn ram_write(&self, addr: u32, data: &[u8]) -> Result<()> {
        let chunk = oc_ramload::write_chunk(self.link);
        for (i, part) in data.chunks(chunk).enumerate() {
            let mut req = vec![RC_WRITE];
            req.extend_from_slice(&(addr + (i * chunk) as u32).to_le_bytes());
            req.extend_from_slice(part);
            self.ack(&req, "RAM WRITE")?;
        }
        Ok(())
    }

    pub fn ram_crc(&self, addr: u32, len: u32) -> Result<u32> {
        let mut req = vec![RC_CRC];
        req.extend_from_slice(&addr.to_le_bytes());
        req.extend_from_slice(&len.to_le_bytes());
        crc_of(&self.ack(&req, "RAM CRC")?)
    }

    pub fn ee_read(&self, addr: usize, len: usize) -> Result<Vec<u8>> {
        let max = EE_READ_MAX.min(self.link.max_reply().saturating_sub(1));
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let a = addr + out.len();
            let n = max.min(len - out.len());
            let mut req = vec![RC_EE_READ];
            req.extend_from_slice(&(a as u16).to_le_bytes());
            req.push(n as u8);
            let d = self.ack(&req, "EE_READ")?;
            if d.len() != n {
                return Err(Error::Device(format!("EE_READ 0x{a:04x}: {} bytes instead of {n}", d.len())));
            }
            out.extend(d);
        }
        Ok(out)
    }

    /// As stored in the EEPROM; `len` ≤ [`MAX_EE_CRC`].
    pub fn ee_crc(&self, addr: usize, len: usize) -> Result<u32> {
        debug_assert!(len <= MAX_EE_CRC);
        let mut req = vec![RC_EE_CRC];
        req.extend_from_slice(&(addr as u16).to_le_bytes());
        req.extend_from_slice(&(len as u32).to_le_bytes());
        crc_of(&self.ack(&req, "EE_CRC")?)
    }

    /// Writes `data` at `addr`, split at page boundaries and into updater-sized messages; the updater reads
    /// every message back before acknowledging it.
    pub fn ee_write(&self, addr: usize, data: &[u8]) -> Result<()> {
        let max = EE_WRITE_MAX.min(self.link.max_payload().saturating_sub(3));
        let mut done = 0;
        while done < data.len() {
            let a = addr + done;
            let n = max.min(EE_PAGE - a % EE_PAGE).min(data.len() - done);
            let mut req = vec![RC_EE_WRITE];
            req.extend_from_slice(&(a as u16).to_le_bytes());
            req.extend_from_slice(&data[done..done + n]);
            self.ack(&req, &format!("EE_WRITE 0x{a:04x}"))?;
            done += n;
        }
        Ok(())
    }

    /// Migration unlock: boot0 / boot1 become writable until the updater restarts.
    pub fn unlock_boot(&self) -> Result<()> {
        self.ack(&[RC_EE_UNLOCK_BOOT, b'U', b'N', b'L', b'K'], "EE_UNLOCK_BOOT").map(drop)
    }

    /// The critical command: both boot0 pages from RAM, back to back. The raw reply is returned.
    pub fn write_boot0(&self, ram: u32, crc: u32) -> Result<Reply> {
        let mut req = vec![RC_EE_WRITE_BOOT0];
        req.extend_from_slice(&ram.to_le_bytes());
        req.extend_from_slice(&crc.to_le_bytes());
        // Single attempt: the caller reads boot0 back before deciding to re-issue it.
        match self.link.transact(b'E', &req, TIMEOUT) {
            Ok((_, d)) => match d.split_first() {
                Some((0x06, _)) => Ok(Reply::Ack(Vec::new())),
                Some((0x15, rest)) => Ok(Reply::Nak(rest.first().copied().unwrap_or(0))),
                _ => Err(Error::Device(format!("EE_WRITE_BOOT0 reply {d:02x?}"))),
            },
            Err(e) => Err(e.into()),
        }
    }

    /// RTI reset: the DSP ROM boots from the EEPROM.
    pub fn reboot(&self) -> Result<()> {
        self.ack(&[RC_REBOOT], "REBOOT").map(drop)
    }
}

fn crc_of(d: &[u8]) -> Result<u32> {
    d.get(..4)
        .map(|b| u32::from_le_bytes(b.try_into().unwrap()))
        .ok_or_else(|| Error::Device(format!("CRC reply {d:02x?}")))
}
