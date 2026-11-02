//! Vendor HID interface of the TAS1020B (interface 2, usage page 0xFF00, report ID 2).

use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use crate::{Error, Result};

pub const VID: u16 = 0x05A7;
pub const PID: u16 = 0x1020;
pub const REPORT_ID: u8 = 2;
pub const REPORT_SIZE: usize = 64;
pub const MAX_PAYLOAD: usize = REPORT_SIZE - 3;

pub struct Companion {
    dev: HidDevice,
}

impl Companion {
    pub fn open() -> Result<Self> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            // never the OpenCompanion firmware, which shares 05A7:1020 since TAS 0.5.0 (oc_hid::is_stock)
            .filter(|d| crate::oc_hid::is_stock(d.vendor_id(), d.product_id(), d.release_number(), d.product_string()))
            .max_by_key(|d| d.usage_page() == 0xFF00) // prefer the vendor interface
            .ok_or(Error::NotFound)?;
        Ok(Self { dev: info.open_device(&api)? })
    }

    fn flush(&self) -> Result<()> {
        let mut buf = [0u8; REPORT_SIZE];
        while self.dev.read_timeout(&mut buf, 20)? > 0 {}
        Ok(())
    }

    pub fn transact(&self, cmd: u8, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        if data.len() > MAX_PAYLOAD {
            return Err(Error::TooLong(data.len()));
        }
        let mut report = [0u8; REPORT_SIZE];
        report[0] = REPORT_ID;
        report[1] = cmd;
        report[2] = data.len() as u8;
        report[3..3 + data.len()].copy_from_slice(data);
        self.flush()?;
        self.dev.write(&report)?;

        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; REPORT_SIZE + 1];
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 50)?;
            if n == 0 {
                continue;
            }
            // Some drivers omit the leading report ID.
            let rx = &buf[..n];
            let rx = if rx[0] == REPORT_ID { &rx[1..] } else { rx };
            if rx.len() < 2 {
                return Err(Error::Unexpected(format!("report too short ({} bytes)", rx.len())));
            }
            let len = (rx[1] as usize).min(rx.len() - 2);
            return Ok((rx[0], rx[2..2 + len].to_vec()));
        }
        Err(Error::Timeout(cmd as char))
    }
}

impl crate::link::Link for Companion {
    fn transact(&self, cmd: u8, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        Companion::transact(self, cmd, data, timeout)
    }
}
