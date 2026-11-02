use std::path::Path;

use oc_protocol::Link;
use oc_tas_image::BOSE_HEADER;

use super::files::{list, save, BackupEntry, Content};
use super::read::{read_dsp, read_tas, speaker_id};
use super::stamp::stamp_now;
use crate::dsp::{detect, DspRuntime, Loaders};
use crate::error::{refused, Result};
use crate::host::Host;
use crate::tas::{TasPort, HEADER_LEN};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StockBackup {
    pub entry: BackupEntry,
    pub created: bool,
}

pub fn stock_backup_status(root: &Path) -> Option<BackupEntry> {
    list(root).into_iter().filter(BackupEntry::is_stock).min_by(|a, b| a.stamp.cmp(&b.stamp))
}

/// Reads only, never writes an EEPROM. On a speaker already on OpenCompanion without a stock backup this is
/// refused: the stock images can no longer be read from it.
///
/// `loaders`: before the first backup the stock image is unknown, so pass
/// `StockRef::Digests(&STOCK_LOADER_DIGESTS)` for the RAM-loader check.
pub fn ensure_stock_backup(root: &Path, tas: &dyn TasPort, dsp: &dyn Link, loaders: &Loaders, host: &mut dyn Host) -> Result<StockBackup> {
    let existing = stock_backup_status(root);
    let header = tas.read(0, HEADER_LEN)?;
    let rt = detect(dsp);
    let stock = header == BOSE_HEADER && matches!(rt, DspRuntime::Stock { .. });
    if !stock {
        return match existing {
            Some(entry) => Ok(StockBackup { entry, created: false }),
            None => refused(format!(
                "no backup of the Bose firmware exists and the speaker no longer runs it (DSP: {rt}): it cannot be \
                 read any more"
            )),
        };
    }
    if let Some(e) = &existing {
        let once = tas.read(0, crate::tas::EE_SIZE)?;
        let same = list(root).into_iter().find(|b| b.is_stock() && b.tas.is_some_and(|d| d.crc32 == crc32fast::hash(&once)));
        if let Some(entry) = same {
            host.log(&format!("backup: the Bose firmware of this speaker is already saved ({}).", entry.dir.display()));
            return Ok(StockBackup { entry, created: false });
        }
        host.log(&format!("backup: a stock backup exists ({}) but holds another USB controller image.", e.dir.display()));
    }
    let tas_bytes = read_tas(tas, host)?;
    let dump = read_dsp(dsp, loaders, host)?;
    if Content::of_tas(&tas_bytes) != Content::Stock || Content::of_dsp(&dump.bytes) != Content::Stock {
        return refused("the EEPROMs read do not hold a complete stock Bose image: not saved as the stock backup");
    }
    let entry = save(root, &speaker_id(&dump.bytes), &stamp_now(), Some(&tas_bytes), Some(&dump.bytes))?;
    host.log(&format!("backup: Bose firmware of both chips saved in {}", entry.dir.display()));
    Ok(StockBackup { entry, created: true })
}
