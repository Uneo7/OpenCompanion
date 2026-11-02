//! Backup of the Bose firmware, read from the user's speaker (`oc_install::backup`): never a bundled file.
//! Folders: `<root>/<speaker>/<YYYY-MM-DD_HHMMSS>/{u6002_tas.bin, u7001_dsp.bin, checksums.txt}`, never
//! overwritten; the first stock backup is kept forever.

use std::path::{Path, PathBuf};

use oc_install::backup::{self as ob, BackupEntry, BackupFiles, StockBackup};
use oc_install::dsp::{Loaders, StockRef};
use oc_install::tas::{OcPort, StockPort};
use oc_install::Host;
use oc_protocol::oc_hid::OcRelay;

use super::sources::FirmwareSources;
use crate::link::Conn;

/// UTC.
pub fn today() -> String {
    ob::stamp_now()[..10].to_string()
}

pub fn stock(root: &Path) -> Option<BackupEntry> {
    ob::stock_backup_status(root)
}

pub fn latest(root: &Path) -> Option<(String, PathBuf)> {
    stock(root).map(|e| (e.date().to_string(), e.dir))
}

/// Checksums verified; refused without one.
pub fn load_stock(root: &Path) -> Result<BackupFiles, String> {
    let e = stock(root).ok_or_else(|| {
        format!(
            "no verified backup of the Bose firmware in {}: it is made automatically when the app connects to the \
             speaker while it still runs the Bose firmware",
            root.display()
        )
    })?;
    ob::load(&e.dir).map_err(|e| e.to_string())
}

/// Reads only.
pub fn ensure(src: &FirmwareSources, conn: &Conn, host: &mut dyn Host) -> Result<StockBackup, String> {
    let (stub, updater) = src.loaders()?;
    let loaders = Loaders { stock: StockRef::Known, stub: &stub, updater: &updater };
    let r = match conn {
        Conn::Stock(c) => ob::ensure_stock_backup(&src.backups, &StockPort::new(c), c, &loaders, host),
        Conn::Oc(d) => ob::ensure_stock_backup(&src.backups, &OcPort::new(d), &OcRelay::new(d), &loaders, host),
    };
    r.map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn no_stock_backup_is_refused() {
        let root = std::env::temp_dir().join(format!("oc-device-backup-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        assert!(latest(&root).is_none());
        assert!(load_stock(&root).unwrap_err().contains("no verified backup"));
        assert_eq!(today().len(), 10);
    }
}
