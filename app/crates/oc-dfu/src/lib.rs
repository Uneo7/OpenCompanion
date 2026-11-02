//! USB DFU 1.0 download to the TAS1020B ROM boot loader (Data Manual SLES025B §2.2.2.2 and §2.2.2.5).
//! The ROM enumerates as a DFU device when the EEPROM is missing/invalid (VID 0xFFFF, PID 0xFFFE) or holds a
//! "device EEPROM" header (DataType 2, VID/PID from the header). The download stream is a full TAS image:
//! header + application code. Destination:
//! - no / invalid EEPROM: RAM (header overwrite bit must be 0) — or EEPROM when the EEPROM is present but
//!   invalid (header overwrite bit must be 1);
//! - device EEPROM: code to RAM; the header is also written to the EEPROM when the header overwrite bit is 1.
//!
//! After the final zero-length block the host issues a USB reset and the TAS starts the application.
//! On Windows the DFU interface needs the WinUSB driver (install it with Zadig).

use std::thread::sleep;
use std::time::Duration;

use nusb::transfer::{Control, ControlType, Recipient};

const DFU_DNLOAD: u8 = 1;
const DFU_GETSTATUS: u8 = 3;
const DFU_CLRSTATUS: u8 = 4;
const DFU_ABORT: u8 = 6;
const TIMEOUT: Duration = Duration::from_secs(2);
/// Bounds on GETSTATUS polling, so that a bogus bwPollTimeout cannot hang the host.
const MAX_POLLS: usize = 200;
const MAX_POLL_SLEEP: Duration = Duration::from_millis(500);

const CLASS_APP_SPECIFIC: u8 = 0xFE;
const SUBCLASS_DFU: u8 = 0x01;
const DESC_DFU_FUNCTIONAL: u8 = 0x21;
/// Default transfer size (the TAS1020B uses 64-byte transfers).
pub const DEFAULT_TRANSFER_SIZE: usize = 64;

/// VID/PID reported by the ROM when there is no valid EEPROM header.
pub const ROM_DFU_ID: (u16, u16) = (0xFFFF, 0xFFFE);

#[derive(Debug, thiserror::Error)]
pub enum Error {
    #[error("USB: {0}")]
    Usb(#[from] nusb::Error),
    #[error("USB transfer: {0}")]
    Transfer(#[from] nusb::transfer::TransferError),
    #[error("no DFU device found ({0})")]
    NotFound(String),
    #[error("short DFU status ({0} bytes)")]
    ShortStatus(usize),
    #[error("DFU error: status {status}, state {state}")]
    Dfu { status: u8, state: u8 },
    #[error("unexpected DFU state {0} (expected {1})")]
    State(u8, &'static str),
}

pub type Result<T> = std::result::Result<T, Error>;

/// DFU states (DFU 1.0 §6.1.2).
pub mod state {
    pub const DFU_IDLE: u8 = 2;
    pub const DNLOAD_SYNC: u8 = 3;
    pub const DNBUSY: u8 = 4;
    pub const DNLOAD_IDLE: u8 = 5;
    pub const MANIFEST_SYNC: u8 = 6;
    pub const MANIFEST: u8 = 7;
    pub const MANIFEST_WAIT_RESET: u8 = 8;
    pub const ERROR: u8 = 10;
}

#[derive(Debug, Clone, Copy)]
pub struct Status {
    pub status: u8,
    pub poll_timeout: Duration,
    pub state: u8,
}

#[derive(Debug, Clone)]
pub struct Found {
    pub vendor_id: u16,
    pub product_id: u16,
    pub interface: u8,
}

/// VID:PID of every USB device on the bus. Read-only: no device is opened, no driver needed.
pub fn present_ids() -> Result<Vec<(u16, u16)>> {
    Ok(nusb::list_devices()?.map(|d| (d.vendor_id(), d.product_id())).collect())
}

pub fn find(id: Option<(u16, u16)>) -> Result<Vec<(nusb::DeviceInfo, Found)>> {
    let mut out = Vec::new();
    for info in nusb::list_devices()? {
        if id.is_some_and(|(v, p)| info.vendor_id() != v || info.product_id() != p) {
            continue;
        }
        // nusb only lists interfaces of composite devices on Windows; the TAS ROM DFU device is not composite
        // (device class 0xFE), so fall back to reading the configuration descriptor.
        let dfu = info
            .interfaces()
            .find(|i| i.class() == CLASS_APP_SPECIFIC && i.subclass() == SUBCLASS_DFU)
            .map(|i| i.interface_number())
            .or_else(|| {
                let dev = info.open().ok()?;
                let found = dev.configurations().find_map(|c| {
                    c.interface_alt_settings()
                        .find(|g| g.class() == CLASS_APP_SPECIFIC && g.subclass() == SUBCLASS_DFU)
                        .map(|g| g.interface_number())
                });
                found
            });
        if let Some(interface) = dfu {
            let found = Found { vendor_id: info.vendor_id(), product_id: info.product_id(), interface };
            out.push((info, found));
        }
    }
    Ok(out)
}

pub struct DfuDevice {
    device: nusb::Device,
    iface: nusb::Interface,
    number: u8,
    pub transfer_size: usize,
}

impl DfuDevice {
    pub fn open(info: &nusb::DeviceInfo, found: &Found) -> Result<Self> {
        let device = info.open()?;
        let transfer_size = functional_transfer_size(&device, found.interface).unwrap_or(DEFAULT_TRANSFER_SIZE);
        let iface = device.claim_interface(found.interface)?;
        Ok(Self { device, iface, number: found.interface, transfer_size })
    }

    fn control(&self, request: u8, value: u16) -> Control {
        Control {
            control_type: ControlType::Class,
            recipient: Recipient::Interface,
            request,
            value,
            index: self.number as u16,
        }
    }

    pub fn get_status(&self) -> Result<Status> {
        let mut buf = [0u8; 6];
        let n = self.iface.control_in_blocking(self.control(DFU_GETSTATUS, 0), &mut buf, TIMEOUT)?;
        if n < 6 {
            return Err(Error::ShortStatus(n));
        }
        let ms = u32::from_le_bytes([buf[1], buf[2], buf[3], 0]);
        Ok(Status { status: buf[0], poll_timeout: Duration::from_millis(ms as u64), state: buf[4] })
    }

    pub fn clear_status(&self) -> Result<()> {
        self.iface.control_out_blocking(self.control(DFU_CLRSTATUS, 0), &[], TIMEOUT)?;
        Ok(())
    }

    pub fn abort(&self) -> Result<()> {
        self.iface.control_out_blocking(self.control(DFU_ABORT, 0), &[], TIMEOUT)?;
        Ok(())
    }

    /// Brings the device back to dfuIDLE (clears an error, aborts a pending transfer).
    pub fn ensure_idle(&self) -> Result<()> {
        let st = self.get_status()?;
        match st.state {
            state::DFU_IDLE => Ok(()),
            state::ERROR => {
                self.clear_status()?;
                self.expect_idle()
            }
            _ => {
                self.abort()?;
                self.expect_idle()
            }
        }
    }

    fn expect_idle(&self) -> Result<()> {
        let st = self.get_status()?;
        if st.state == state::DFU_IDLE {
            Ok(())
        } else {
            Err(Error::State(st.state, "dfuIDLE"))
        }
    }

    fn wait(&self) -> Result<Status> {
        let debug = std::env::var_os("OC_DFU_DEBUG").is_some();
        for _ in 0..MAX_POLLS {
            let st = self.get_status()?;
            if debug {
                eprintln!("dfu status {} state {} poll {:?}", st.status, st.state, st.poll_timeout);
            }
            if st.status != 0 || st.state == state::ERROR {
                return Err(Error::Dfu { status: st.status, state: st.state });
            }
            match st.state {
                state::DNLOAD_SYNC | state::DNBUSY | state::MANIFEST_SYNC | state::MANIFEST => {
                    sleep(st.poll_timeout.clamp(Duration::from_millis(5), MAX_POLL_SLEEP))
                }
                _ => return Ok(st),
            }
        }
        Err(Error::State(self.get_status()?.state, "a non-busy state (poll limit reached)"))
    }

    /// Downloads `image` (TAS header + code), then the zero-length block, then resets the device.
    pub fn download(&self, image: &[u8], mut progress: impl FnMut(usize, usize)) -> Result<()> {
        self.ensure_idle()?;
        let mut block: u16 = 0;
        for chunk in image.chunks(self.transfer_size) {
            self.iface.control_out_blocking(self.control(DFU_DNLOAD, block), chunk, TIMEOUT)?;
            let st = self.wait()?;
            if st.state != state::DNLOAD_IDLE {
                return Err(Error::State(st.state, "dfuDNLOAD-IDLE"));
            }
            block = block.wrapping_add(1);
            progress((block as usize * self.transfer_size).min(image.len()), image.len());
        }
        self.iface.control_out_blocking(self.control(DFU_DNLOAD, block), &[], TIMEOUT)?;
        let st = self.wait()?;
        if !matches!(st.state, state::DFU_IDLE | state::MANIFEST_WAIT_RESET) {
            return Err(Error::State(st.state, "dfuIDLE or dfuMANIFEST-WAIT-RESET"));
        }
        // The TAS1020B starts the application after a host-issued USB reset (Data Manual §2.2.3).
        if let Err(e) = self.device.reset() {
            eprintln!("warning: USB reset request failed ({e}); restart the device from an elevated prompt (tools\tas_dfu_run.ps1 does it; do NOT replug the cable, the ROM ignores a reset preceded by a suspend)");
        }
        Ok(())
    }
}

fn functional_transfer_size(device: &nusb::Device, interface: u8) -> Option<usize> {
    let config = device.active_configuration().ok()?;
    for alt in config.interface_alt_settings() {
        if alt.interface_number() != interface {
            continue;
        }
        for desc in alt.descriptors() {
            if desc.descriptor_type() == DESC_DFU_FUNCTIONAL && desc.len() >= 7 {
                let size = u16::from_le_bytes([desc[5], desc[6]]) as usize;
                return (size > 0).then_some(size);
            }
        }
    }
    None
}
