//! Vendor HID protocol of the OpenCompanion TAS firmware (`firmware/usb/app/proto/protocol.c`).
//! Interface 2, 64-byte reports without report ID:
//! - request `[cmd][tag][len][payload, len <= 61]`
//! - reply   `[cmd][tag][status][len][payload, len <= 60]`, exactly one per request.
//!
//! Guarded commands (EE_BEGIN, REBOOT, REBOOT_DFU) need the key payload `'O' 'C' cmd !cmd`.

use std::cell::Cell;
use std::time::{Duration, Instant};

use hidapi::{HidApi, HidDevice};

use crate::link::Link;
use crate::{Error, Result};

/// USB identity (`firmware/usb/README.md` "USB identity"). Since TAS firmware 0.5.0 OpenCompanion enumerates with
/// the stock Bose VID:PID 05A7:1020 in both audio modes, so it is told apart from the stock firmware by what every
/// OS reports at enumeration without opening anything: bcdDevice (ours >= 0x0500, stock 0x0100) and the product
/// string (ours contains "OpenCompanion", stock "Bose USB Audio"). 0.4.x used the pid.codes test IDs 1209:0001 /
/// 1209:0002, still accepted so that the app can update a speaker running 0.4.x.
pub const VID: u16 = 0x05A7;
pub const PID: u16 = 0x1020;
/// Lowest bcdDevice of an OpenCompanion firmware on 05A7:1020 (the stock firmware reports 0x0100).
pub const MIN_BCD: u16 = 0x0500;
/// Substring of the OpenCompanion product string ("Companion 5 (OpenCompanion)").
pub const PRODUCT_TAG: &str = "OpenCompanion";
/// 0.4.x identities (pid.codes test VID, one PID per audio mode).
pub const LEGACY_VID: u16 = 0x1209;
pub const LEGACY_PIDS: [u16; 2] = [0x0001, 0x0002];

/// The device is the OpenCompanion TAS firmware: 0.4.x on 1209:0001 / 0002, or 0.5.0+ on 05A7:1020 with our
/// bcdDevice or product string. Everything else on 05A7:1020 is the stock firmware (or the ROM DFU, which has no
/// HID interface). `product` = the USB product string when the OS gives it.
pub fn is_opencompanion(vid: u16, pid: u16, bcd: u16, product: Option<&str>) -> bool {
    if vid == LEGACY_VID && LEGACY_PIDS.contains(&pid) {
        return true;
    }
    vid == VID && pid == PID && (bcd >= MIN_BCD || product.is_some_and(|p| p.contains(PRODUCT_TAG)))
}

/// The device is the stock Bose TAS firmware: 05A7:1020 and not OpenCompanion (the exact complement on that ID, so
/// the stock and OpenCompanion paths never open each other's device).
pub fn is_stock(vid: u16, pid: u16, bcd: u16, product: Option<&str>) -> bool {
    vid == VID && pid == PID && !is_opencompanion(vid, pid, bcd, product)
}

pub(crate) fn info_is_opencompanion(d: &hidapi::DeviceInfo) -> bool {
    is_opencompanion(d.vendor_id(), d.product_id(), d.release_number(), d.product_string())
}
pub const REPORT_SIZE: usize = 64;
pub const MAX_REQUEST: usize = 61;
pub const MAX_REPLY: usize = 60;

pub mod cmd {
    pub const INFO: u8 = 0x01;
    pub const DSP: u8 = 0x10;
    pub const EE_READ: u8 = 0x20;
    pub const EE_BEGIN: u8 = 0x21;
    pub const EE_WRITE: u8 = 0x22;
    pub const EE_COMMIT: u8 = 0x23;
    pub const EE_END: u8 = 0x24;
    pub const REBOOT_DFU: u8 = 0x7E;
    pub const REBOOT: u8 = 0x7F;
}

pub mod status {
    pub const OK: u8 = 0x00;
    pub const BAD_LEN: u8 = 0x01;
    pub const UNKNOWN: u8 = 0x02;
    pub const BAD_ARG: u8 = 0x03;
    pub const STATE: u8 = 0x04;
    pub const BAD_KEY: u8 = 0x05;
    pub const EE_IO: u8 = 0x20;
    pub const EE_VERIFY: u8 = 0x21;
    pub const EE_RANGE: u8 = 0x22;
    pub const EE_HEADER: u8 = 0x23;
}

pub fn key(command: u8) -> [u8; 4] {
    [b'O', b'C', command, !command]
}

pub trait OcLink {
    /// Returns `(status, payload)`; a non-zero status is not an error at this level.
    fn request(&self, cmd: u8, payload: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)>;

    fn delay(&self, d: Duration) {
        std::thread::sleep(d);
    }

    /// Like [`OcLink::request`], but a non-zero status becomes [`Error::Status`].
    fn call(&self, cmd: u8, payload: &[u8], timeout: Duration) -> Result<Vec<u8>> {
        match self.request(cmd, payload, timeout)? {
            (status::OK, data) => Ok(data),
            (st, _) => Err(Error::Status(st)),
        }
    }
}

impl<T: OcLink + ?Sized> OcLink for &T {
    fn request(&self, cmd: u8, payload: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        (**self).request(cmd, payload, timeout)
    }
    fn delay(&self, d: Duration) {
        (**self).delay(d)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct OcInfo {
    pub protocol: u8,
    pub version: (u8, u8, u8),
    pub flags0: u8,
    pub flags1: u8,
}

impl OcInfo {
    pub fn parse(p: &[u8]) -> Result<Self> {
        if p.len() < 7 {
            return Err(Error::Unexpected(format!("INFO reply too short ({} bytes)", p.len())));
        }
        Ok(Self { protocol: p[0], version: (p[1], p[2], p[3]), flags0: p[5], flags1: p[6] })
    }

    /// An EEPROM update session is open (header DataType 3 written by EE_BEGIN).
    pub fn ee_session(&self) -> bool {
        self.flags1 & 0x08 != 0
    }

    pub fn dsp_link_up(&self) -> bool {
        self.flags0 & 0x10 != 0
    }

    pub fn version_string(&self) -> String {
        format!("{}.{}.{}", self.version.0, self.version.1, self.version.2)
    }
}

pub fn info<O: OcLink + ?Sized>(link: &O) -> Result<OcInfo> {
    OcInfo::parse(&link.call(cmd::INFO, &[], Duration::from_millis(500))?)
}

pub struct OcDevice {
    dev: HidDevice,
    tag: Cell<u8>,
}

impl OcDevice {
    pub fn open() -> Result<Self> {
        let api = HidApi::new()?;
        let info = api
            .device_list()
            .filter(|d| info_is_opencompanion(d))
            .max_by_key(|d| d.usage_page() == 0xFF00)
            .ok_or(Error::NotFound)?;
        Ok(Self { dev: info.open_device(&api)?, tag: Cell::new(0) })
    }

    fn flush(&self) -> Result<()> {
        let mut buf = [0u8; REPORT_SIZE + 1];
        while self.dev.read_timeout(&mut buf, 10)? > 0 {}
        Ok(())
    }
}

impl OcLink for OcDevice {
    fn request(&self, command: u8, payload: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        if payload.len() > MAX_REQUEST {
            return Err(Error::TooLong(payload.len()));
        }
        let tag = self.tag.get().wrapping_add(1);
        self.tag.set(tag);
        // Leading 0 = "no report ID" for hidapi; it is not sent on the wire.
        let mut report = [0u8; REPORT_SIZE + 1];
        report[1] = command;
        report[2] = tag;
        report[3] = payload.len() as u8;
        report[4..4 + payload.len()].copy_from_slice(payload);
        self.flush()?;
        self.dev.write(&report)?;

        let deadline = Instant::now() + timeout;
        let mut buf = [0u8; REPORT_SIZE + 1];
        while Instant::now() < deadline {
            let n = self.dev.read_timeout(&mut buf, 50)?;
            if n == 0 {
                continue;
            }
            // Some backends keep a leading 0 report ID on devices without report IDs.
            let rx = if n == REPORT_SIZE + 1 && buf[0] == 0 { &buf[1..n] } else { &buf[..n] };
            if rx.len() < 4 || rx[0] != command || rx[1] != tag {
                continue; // stale reply of an earlier request
            }
            let len = (rx[3] as usize).min(rx.len() - 4);
            return Ok((rx[2], rx[4..4 + len].to_vec()));
        }
        Err(Error::Timeout(command as char))
    }
}

/// DSP relay of the OpenCompanion firmware seen as a stock-style [`Link`]:
/// `transact(c, data)` = DSP command `[c][reply delay][data]` → `[reply cmd][reply data]`.
pub struct OcRelay<'a, O: OcLink + ?Sized> {
    link: &'a O,
    /// Delay before the TAS reads the DSP reply (20 ms like the stock `E`).
    pub reply_delay_ms: u8,
}

impl<'a, O: OcLink + ?Sized> OcRelay<'a, O> {
    pub fn new(link: &'a O) -> Self {
        Self { link, reply_delay_ms: 20 }
    }
}

impl<O: OcLink + ?Sized> Link for OcRelay<'_, O> {
    fn transact(&self, c: u8, data: &[u8], timeout: Duration) -> Result<(u8, Vec<u8>)> {
        if data.len() > self.max_payload() {
            return Err(Error::TooLong(data.len()));
        }
        let mut req = vec![c, self.reply_delay_ms];
        req.extend_from_slice(data);
        match self.link.request(cmd::DSP, &req, timeout)? {
            (status::OK, rep) => match rep.split_first() {
                Some((&rc, rest)) => Ok((rc, rest.to_vec())),
                None => Err(Error::Unexpected("empty DSP relay reply".into())),
            },
            (st @ 0x10..=0x1F, _) => Err(Error::Relay(format!("DSP link error 0x{st:02x}"))),
            (st, _) => Err(Error::Status(st)),
        }
    }

    fn max_payload(&self) -> usize {
        MAX_REQUEST - 2
    }

    fn max_reply(&self) -> usize {
        MAX_REPLY - 1
    }

    fn delay(&self, d: Duration) {
        self.link.delay(d)
    }
}

#[cfg(test)]
mod identity_tests {
    use super::*;

    #[test]
    fn stock_and_opencompanion_partition_05a7_1020() {
        assert!(is_stock(0x05A7, 0x1020, 0x0100, Some("Bose USB Audio")));
        assert!(!is_opencompanion(0x05A7, 0x1020, 0x0100, Some("Bose USB Audio")));
        assert!(is_opencompanion(0x05A7, 0x1020, 0x0500, Some("Companion 5 (OpenCompanion)")));
        assert!(is_opencompanion(0x05A7, 0x1020, 0x0500, None), "bcdDevice alone");
        assert!(is_opencompanion(0x05A7, 0x1020, 0x0100, Some("Companion 5 (OpenCompanion)")), "product alone");
        assert!(!is_stock(0x05A7, 0x1020, 0x0500, None));
        assert!(is_opencompanion(0x1209, 0x0001, 0x0410, Some("Companion 5 dev")), "0.4.x");
        assert!(is_opencompanion(0x1209, 0x0002, 0x0410, None), "0.4.x, mode 2.1");
        assert!(!is_stock(0x1209, 0x0001, 0x0410, None));
        assert!(!is_opencompanion(0x05A7, 0x4000, 0x0500, None) && !is_stock(0x05A7, 0x4000, 0x0100, None));
    }
}
