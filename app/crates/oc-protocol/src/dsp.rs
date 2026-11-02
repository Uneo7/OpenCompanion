//! 3-letter commands. Text (SVM, SYS...): terminated by `\r`. Binary (BRR, BWW): LE32 arguments.
//! `BRR n addr` → `06` + 4n bytes; `BWW n addr data` → `06` (ACK) or `15` (NAK).

use std::time::Duration;

use crate::link::Link;
use crate::transport::Companion;
use crate::{Error, Result};

pub const ACK: u8 = 0x06;
pub const NAK: u8 = 0x15;
const TIMEOUT: Duration = Duration::from_millis(500);
/// Safe BWW write size (DSP frame buffer size not measured).
pub const MAX_WRITE_WORDS: usize = 5;

pub struct DspConsole<'a, L: Link + ?Sized = Companion> {
    link: &'a L,
}

impl<'a, L: Link + ?Sized> DspConsole<'a, L> {
    pub fn new(link: &'a L) -> Self {
        Self { link }
    }

    pub fn link(&self) -> &'a L {
        self.link
    }

    pub fn raw(&self, payload: &[u8]) -> Result<Vec<u8>> {
        let (cmd, data) = self.link.transact(b'E', payload, TIMEOUT)?;
        if data.starts_with(b"ERROR") {
            return Err(Error::Relay(String::from_utf8_lossy(&data).into_owned()));
        }
        if cmd != b'E' {
            return Err(Error::Unexpected(format!("command '{}' instead of 'E'", cmd as char)));
        }
        Ok(data)
    }

    /// Text command (appends `\r`); response without the trailing `\r>` prompt.
    pub fn text(&self, cmd: &str) -> Result<String> {
        let resp = self.raw(format!("{cmd}\r").as_bytes())?;
        let s = String::from_utf8_lossy(&resp).into_owned();
        if s.ends_with("\r!") {
            return Err(Error::Nak);
        }
        Ok(s.trim_end_matches("\r>").replace('\r', "\n"))
    }

    /// Reads 1 to 4 32-bit words.
    pub fn read_words(&self, addr: u32, count: usize) -> Result<Vec<u32>> {
        assert!((1..=4).contains(&count), "BRR reads 1 to 4 words");
        let mut req = b"BRR".to_vec();
        req.extend_from_slice(&(count as u32).to_le_bytes());
        req.extend_from_slice(&addr.to_le_bytes());
        let resp = self.raw(&req)?;
        match resp.split_first() {
            Some((&ACK, body)) if body.len() >= 4 * count => {
                Ok(body.chunks_exact(4).take(count).map(|c| u32::from_le_bytes(c.try_into().unwrap())).collect())
            }
            Some((&NAK, _)) => Err(Error::Nak),
            _ => Err(Error::Unexpected(format!("BRR: {resp:02x?}"))),
        }
    }

    /// Writes 1 to [`MAX_WRITE_WORDS`] 32-bit words.
    pub fn write_words(&self, addr: u32, words: &[u32]) -> Result<()> {
        assert!((1..=MAX_WRITE_WORDS).contains(&words.len()), "BWW: 1 to {MAX_WRITE_WORDS} words");
        let mut req = b"BWW".to_vec();
        req.extend_from_slice(&(words.len() as u32).to_le_bytes());
        req.extend_from_slice(&addr.to_le_bytes());
        for w in words {
            req.extend_from_slice(&w.to_le_bytes());
        }
        match self.raw(&req)?.first() {
            Some(&ACK) => Ok(()),
            Some(&NAK) => Err(Error::Nak),
            other => Err(Error::Unexpected(format!("BWW: {other:02x?}"))),
        }
    }

    pub fn read_f32(&self, addr: u32, count: usize) -> Result<Vec<f32>> {
        Ok(self.read_words(addr, count)?.into_iter().map(f32::from_bits).collect())
    }

    pub fn write_f32(&self, addr: u32, values: &[f32]) -> Result<()> {
        self.write_words(addr, &values.iter().map(|v| v.to_bits()).collect::<Vec<_>>())
    }
}
