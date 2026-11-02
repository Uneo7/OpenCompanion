use std::path::Path;

use oc_protocol::Link;

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
/// `StockRef::Known` for the RAM-loader check.
pub fn ensure_stock_backup(root: &Path, tas: &dyn TasPort, dsp: &dyn Link, loaders: &Loaders, host: &mut dyn Host) -> Result<StockBackup> {
    let existing = stock_backup_status(root);
    let header = tas.read(0, HEADER_LEN)?;
    let rt = detect(dsp);
    // Any Bose revision: a stock-shaped USB controller header and a DSP answering as the Bose firmware.
    let stock = super::files::is_stock_tas(&header) && matches!(rt, DspRuntime::Stock { .. });
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
    host.log(&format!("backup: DSP answers {rt}"));
    let tas_bytes = read_tas(tas, host)?;
    // The stock raw read first (any revision, no RAM loader); the RAM loader only where `d` is not answered.
    let dsp_bytes = if super::raw::available(dsp) {
        super::raw::read_u7001(dsp, host)?
    } else {
        host.log("backup: the stock raw EEPROM read does not answer: through the RAM loader");
        read_dsp(dsp, loaders, host)?.bytes
    };
    if Content::of_tas(&tas_bytes) != Content::Stock || Content::of_dsp(&dsp_bytes) != Content::Stock {
        return refused("the EEPROMs read do not hold a complete stock Bose image: not saved as the stock backup");
    }
    let entry = save(root, &speaker_id(&tas_bytes, &dsp_bytes), &stamp_now(), Some(&tas_bytes), Some(&dsp_bytes))?;
    if let DspRuntime::Stock { svm } = &rt {
        // Informative only (not part of the checksums): the Bose revision this backup was read from.
        let _ = std::fs::write(entry.dir.join(super::files::VERSION_FILE), format!("{svm}\n"));
    }
    host.log(&format!("backup: Bose firmware of both chips saved in {}", entry.dir.display()));
    Ok(StockBackup { entry, created: true })
}
