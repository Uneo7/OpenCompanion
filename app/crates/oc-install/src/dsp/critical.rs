//! Both boot0 pages are written by ONE updater command, staged in RAM and CRC-checked first: the window is
//! only the two page writes and the read-back (~17 ms, `docs/UPDATE.md`), with no USB round trip inside it.
//! The caller checks every precondition right before calling.

use oc_ais::eeprom::layout::BOOT0;
use oc_ais::eeprom::update::BOOT0_STAGING_RAM;

use super::updater::{Reply, Updater};
use crate::error::{Error, Result};
use crate::host::Host;

/// The DSP keeps running from RAM between attempts.
const BOOT0_ATTEMPTS: usize = 5;

/// `rerun` is the command the user must run at once if the window could not be closed.
pub(crate) fn write_boot0(u: &Updater, boot0: &[u8; 256], tag: &str, rerun: &str, host: &mut dyn Host) -> Result<()> {
    u.unlock_boot()?;
    let crc = crc32fast::hash(boot0);
    let stage = |u: &Updater| -> Result<()> {
        u.ram_write(BOOT0_STAGING_RAM, boot0)?;
        if u.ram_crc(BOOT0_STAGING_RAM, 256)? != crc {
            return Err(Error::Verify("staged boot0 CRC mismatch in DSP RAM".into()));
        }
        Ok(())
    };
    stage(u)?;
    host.log(&format!("{tag}: entering the critical window (boot0, both pages in one updater command)…"));
    let mut last = String::new();
    for attempt in 1..=BOOT0_ATTEMPTS {
        match u.write_boot0(BOOT0_STAGING_RAM, crc) {
            Ok(Reply::Ack(_)) => {}
            Ok(Reply::Nak(4)) => {
                last = "staged data CRC mismatch".into();
                stage(u)?;
                continue;
            }
            Ok(Reply::Nak(c)) => last = format!("updater NAK {c}"),
            Err(e) => last = e.to_string(),
        }
        // Whatever the reply, the EEPROM decides.
        match u.ee_read(BOOT0.start, BOOT0.len()) {
            Ok(back) if back == boot0[..] => {
                host.log(&format!("{tag}: boot0 written and read back (attempt {attempt}). Critical window closed."));
                return Ok(());
            }
            Ok(_) => last = format!("{last}; boot0 read-back differs"),
            Err(e) => last = format!("{last}; boot0 read-back failed: {e}"),
        }
        host.log(&format!("{tag}: attempt {attempt} failed ({last}), re-issuing at once"));
    }
    Err(Error::Device(format!(
        "CRITICAL: boot0 could not be verified after {BOOT0_ATTEMPTS} attempts ({last}). DO NOT power off the \
         speaker: the DSP still runs the updater from RAM. Re-run `{rerun}` immediately."
    )))
}
