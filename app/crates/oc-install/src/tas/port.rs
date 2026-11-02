use std::ops::Range;
use std::time::Duration;

use oc_protocol::oc_hid::{cmd, key, OcLink};
use oc_protocol::{tas_eeprom, Link};

use super::target::{updating_header, CODE_START, DATA_AREA, DATA_TYPE_AT, EE_SIZE, HEADER_LEN, OC_CODE_END, PAGE};
use crate::error::{Error, Result};

const RETRIES: usize = 3;
const TIMEOUT: Duration = Duration::from_millis(500);

/// Every write is verified before it returns.
pub trait TasPort {
    fn name(&self) -> &'static str;
    fn writable(&self) -> Vec<Range<usize>>;
    fn read(&self, addr: usize, len: usize) -> Result<Vec<u8>>;
    /// Header DataType 3.
    fn begin(&self, current: &[u8]) -> Result<()>;
    /// Inside one 32-byte page and one [`TasPort::writable`] range.
    fn write(&self, addr: usize, data: &[u8]) -> Result<()>;
    /// DataType byte last.
    fn commit(&self, header: &[u8; HEADER_LEN]) -> Result<()>;
}

fn retry<T>(mut f: impl FnMut() -> Result<T>) -> Result<T> {
    let mut last = None;
    for _ in 0..RETRIES {
        match f() {
            Ok(v) => return Ok(v),
            Err(e @ (Error::Refused(_) | Error::Cancelled)) => return Err(e),
            Err(e) => last = Some(e),
        }
    }
    Err(last.expect("at least one attempt"))
}

fn check_page(addr: usize, len: usize) -> Result<()> {
    if len == 0 || addr / PAGE != (addr + len - 1) / PAGE || addr + len > EE_SIZE {
        return Err(Error::Refused(format!("TAS write 0x{addr:04x}+{len} crosses a {PAGE}-byte page")));
    }
    Ok(())
}

pub struct StockPort<'a> {
    link: &'a dyn Link,
}

impl<'a> StockPort<'a> {
    pub fn new(link: &'a dyn Link) -> Self {
        Self { link }
    }

    fn write_verified(&self, addr: usize, data: &[u8]) -> Result<()> {
        check_page(addr, data.len())?;
        retry(|| {
            tas_eeprom::write_block(self.link, addr as u16, data)?;
            let back = tas_eeprom::read_block(self.link, addr as u16, data.len() as u8)?;
            if back != data {
                return Err(Error::Verify(format!("TAS EEPROM read-back mismatch at 0x{addr:04x}")));
            }
            Ok(())
        })
    }
}

impl TasPort for StockPort<'_> {
    fn name(&self) -> &'static str {
        "stock Bose TAS firmware (HID t/T)"
    }

    fn writable(&self) -> Vec<Range<usize>> {
        vec![CODE_START..EE_SIZE]
    }

    fn read(&self, addr: usize, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let a = addr + out.len();
            let n = (PAGE - a % PAGE).min(len - out.len());
            out.extend(retry(|| Ok(tas_eeprom::read_block(self.link, a as u16, n as u8)?))?);
        }
        Ok(out)
    }

    fn begin(&self, current: &[u8]) -> Result<()> {
        if current.get(DATA_TYPE_AT) == Some(&3) {
            return Ok(()); // already "download in progress" (resumed run)
        }
        self.write_verified(0, &updating_header(current)?)
    }

    fn write(&self, addr: usize, data: &[u8]) -> Result<()> {
        if addr < CODE_START {
            return Err(Error::Refused(format!("code write at 0x{addr:04x} would touch the header")));
        }
        self.write_verified(addr, data)
    }

    fn commit(&self, header: &[u8; HEADER_LEN]) -> Result<()> {
        // DataType stays 3 until its own one-byte write, the last one.
        self.write_verified(0, &header[..DATA_TYPE_AT])?;
        self.write_verified(DATA_TYPE_AT + 1, &header[DATA_TYPE_AT + 1..])?;
        self.write_verified(DATA_TYPE_AT, &header[DATA_TYPE_AT..DATA_TYPE_AT + 1])
    }
}

/// The firmware verifies each write itself.
pub struct OcPort<'a> {
    link: &'a dyn OcLink,
}

impl<'a> OcPort<'a> {
    pub fn new(link: &'a dyn OcLink) -> Self {
        Self { link }
    }

    fn call(&self, c: u8, payload: &[u8]) -> Result<Vec<u8>> {
        retry(|| match self.link.call(c, payload, TIMEOUT) {
            Ok(v) => Ok(v),
            Err(oc_protocol::Error::Status(st)) => {
                Err(Error::Device(format!("OpenCompanion TAS firmware refused command 0x{c:02x}: status 0x{st:02x}")))
            }
            Err(e) => Err(e.into()),
        })
    }
}

impl TasPort for OcPort<'_> {
    fn name(&self) -> &'static str {
        "OpenCompanion TAS firmware (HID EE_*)"
    }

    /// Same ranges as `firmware/usb/app/proto/eeprom.c` ee_write(): code area and data area.
    fn writable(&self) -> Vec<Range<usize>> {
        vec![CODE_START..OC_CODE_END, DATA_AREA]
    }

    fn read(&self, addr: usize, len: usize) -> Result<Vec<u8>> {
        let mut out = Vec::with_capacity(len);
        while out.len() < len {
            let a = addr + out.len();
            let n = (PAGE - a % PAGE).min(len - out.len());
            let data = self.call(cmd::EE_READ, &[(a >> 8) as u8, a as u8, n as u8])?;
            if data.len() != n {
                return Err(Error::Device(format!("EE_READ 0x{a:04x}: {} bytes instead of {n}", data.len())));
            }
            out.extend(data);
        }
        Ok(out)
    }

    fn begin(&self, _current: &[u8]) -> Result<()> {
        // Always sent: it (re)opens the firmware's session, which a resumed run needs too.
        self.call(cmd::EE_BEGIN, &key(cmd::EE_BEGIN)).map(drop)
    }

    fn write(&self, addr: usize, data: &[u8]) -> Result<()> {
        check_page(addr, data.len())?;
        let mut req = vec![(addr >> 8) as u8, addr as u8];
        req.extend_from_slice(data);
        self.call(cmd::EE_WRITE, &req).map(drop)
    }

    fn commit(&self, header: &[u8; HEADER_LEN]) -> Result<()> {
        self.call(cmd::EE_COMMIT, header).map(drop)
    }
}
