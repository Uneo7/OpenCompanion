//! Same write order as the install: header DataType := 3 first, code and data page by page, then the stock
//! header with DataType 1 written alone, last. Through [`crate::tas::OcPort`] only the code and data areas are
//! writable: 0x1792..0x17FF must already match (0xFF in both images).
//!
//! Power loss: before the DataType 3 write the old image boots; until the final DataType write the TAS ROM
//! boots its DFU (`docs/recovery.md`), never a half-written image; after it the stock firmware boots.

use oc_protocol::oc_hid::{cmd, key, OcLink};
use oc_tas_image::BOSE_HEADER;
use std::time::Duration;

use crate::error::{refused, Error, Result};
use crate::host::Host;
use crate::tas::{install, TasContent, TasOptions, TasPort, TasTarget};
use crate::ui::UiStep;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TasRestoreReport {
    pub before: TasContent,
    pub pages_written: usize,
    pub restored: bool,
}

/// Refused unless the backup has the exact stock header.
pub fn stock_tas_target(backup: &[u8]) -> Result<TasTarget> {
    let t = TasTarget::from_dump(backup)?;
    if t.header != BOSE_HEADER {
        return refused("the TAS backup does not hold the stock Bose header: it was not taken before the install");
    }
    Ok(t)
}

/// Safe to re-run after any interruption.
pub fn restore_tas(port: &dyn TasPort, backup: &[u8], host: &mut dyn Host, opts: TasOptions) -> Result<TasRestoreReport> {
    let target = stock_tas_target(backup)?;
    host.step(UiStep::RestoreUsb);
    let rep = install(port, &target, host, opts)?;
    if rep.installed {
        host.step(UiStep::UsbVerify);
        host.log(&format!("U6002 equals the backup (8 KB, CRC-32 0x{:08x}).", crc32fast::hash(backup)));
    }
    Ok(TasRestoreReport { before: rep.before, pages_written: rep.pages_written, restored: rep.installed })
}

/// The ROM then boots the stock image from U6002; a power cycle of the speaker does the same.
pub fn reboot_tas(link: &dyn OcLink, host: &mut dyn Host) -> Result<()> {
    host.log("restarting the USB controller (the Bose TAS firmware starts from U6002)…");
    link.call(cmd::REBOOT, &key(cmd::REBOOT), Duration::from_millis(500))
        .map(drop)
        .map_err(|e| Error::Device(format!("REBOOT: {e} (power-cycle the speaker instead)")))
}
