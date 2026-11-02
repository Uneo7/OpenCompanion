//! Nothing of the Bose firmware is shipped with the app: only the user's own backup is read.

use std::path::Path;

use oc_install::backup::{load, stock_backup_status};

use super::image::Ram;
use super::stock::StockGraph;
use super::ResponseError;

impl StockGraph {
    pub fn from_stock_dump(dsp: &[u8]) -> Result<Self, ResponseError> {
        StockGraph::new(Ram::from_stock_dump(dsp)?)
    }

    pub fn from_backups(root: &Path) -> Result<Self, ResponseError> {
        let entry = stock_backup_status(root).ok_or(ResponseError::NoBackup)?;
        let files = load(&entry.dir).map_err(|e| ResponseError::Image(e.to_string()))?;
        let dsp = files.dsp.ok_or(ResponseError::NoBackup)?;
        Self::from_stock_dump(&dsp)
    }
}
