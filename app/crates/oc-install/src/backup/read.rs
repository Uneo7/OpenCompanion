use oc_ais::eeprom::layout::{BOOT0, EE_SIZE, LAST_PAGE};
use oc_ais::MAGIC;
use oc_protocol::Link;

use crate::dsp::enter::{enter_updater, reboot};
use crate::dsp::survey::spans;
use crate::dsp::{DspRuntime, Loaders, Updater};
use crate::error::{Error, Result};
use crate::host::Host;
use crate::tas::TasPort;
use crate::ui::UiStep;

pub fn read_tas(port: &dyn TasPort, host: &mut dyn Host) -> Result<Vec<u8>> {
    let size = crate::tas::EE_SIZE;
    host.step(UiStep::Backup);
    host.log(&format!("backup: reading U6002 twice through the {}…", port.name()));
    let mut reads = Vec::new();
    for pass in 0..2 {
        let mut out = Vec::with_capacity(size);
        for a in (0..size).step_by(1024) {
            out.extend(port.read(a, 1024)?);
            host.log(&format!("  U6002 read {}: {}/{} pages", pass + 1, out.len() / 32, size / 32));
        }
        reads.push(out);
    }
    if reads[0] != reads[1] {
        return Err(Error::Verify("the two reads of U6002 differ: nothing saved".into()));
    }
    Ok(reads.swap_remove(0))
}

#[derive(Debug, Clone)]
pub struct DspDump {
    pub bytes: Vec<u8>,
    pub before: DspRuntime,
    pub after: DspRuntime,
}

pub fn read_dsp(link: &dyn Link, loaders: &Loaders, host: &mut dyn Host) -> Result<DspDump> {
    host.step(UiStep::Backup);
    let before = enter_updater(link, loaders, host, true)?;
    let u = Updater::new(link);
    if u.info()?.0 != b'U' {
        return Err(Error::Device("the updater did not start".into()));
    }
    host.log("backup: reading U7001 (64 KB) through the updater…");
    let mut bytes = Vec::with_capacity(EE_SIZE);
    for a in (0..EE_SIZE).step_by(4096) {
        bytes.extend(u.ee_read(a, 4096)?);
        host.log(&format!("  U7001 read: {}/{} pages", bytes.len() / 128, EE_SIZE / 128));
    }
    for (a, n) in spans(0, EE_SIZE) {
        if u.ee_crc(a, n)? != crc32fast::hash(&bytes[a..a + n]) {
            return Err(Error::Verify(format!("U7001 read differs from the device CRC at 0x{a:04x}: nothing saved")));
        }
    }
    let le32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
    let bootable = le32(BOOT0.start) == MAGIC && matches!(le32(4), 0x5853_5904 | 0x5853_5901);
    let after = if bootable {
        reboot(&u, host)
    } else {
        host.log("WARNING: boot0 is not valid: the DSP is NOT rebooted (it would not boot). Restore or install now.");
        DspRuntime::Updater { boot1: 0 }
    };
    Ok(DspDump { bytes, before, after })
}

/// Neither chip exposes a serial number: the id is the CRC-32 of U7001's last page, the only bytes no
/// OpenCompanion flow ever writes. Identical last pages share a folder; the restore checks the last page again.
pub fn speaker_id(dsp: &[u8]) -> String {
    format!("c5-{:08x}", crc32fast::hash(&dsp[LAST_PAGE]))
}
