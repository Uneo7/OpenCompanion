//! Development only: writes ONLY the data area (0x1800..0x1FFF) while the stock firmware runs (`T`), so the
//! OpenCompanion TAS firmware can be tested from RAM: it stays off USB without a valid data area. The area lies
//! after the stock image (ends 0x15F5) and the largest OpenCompanion image (ends 0x1791): neither the stock
//! firmware nor the ROM reads it. [`DataOnly`] refuses any byte outside it; the boot header is never written.

use std::ops::Range;

use oc_tas_image::data_area::DataArea;

use super::port::TasPort;
use super::target::{header_shape_ok, DATA_AREA, HEADER_LEN, PAGE};
use crate::error::{refused, Error, Result};
use crate::host::{Host, Plan};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DataReport {
    pub pages_written: usize,
    pub written: bool,
}

pub struct DataOnly<'a>(pub &'a dyn TasPort);

impl DataOnly<'_> {
    pub fn write(&self, addr: usize, data: &[u8]) -> Result<()> {
        if addr < DATA_AREA.start || addr + data.len() > DATA_AREA.end {
            return refused(format!(
                "write 0x{addr:04x}+{} is outside the data area 0x{:04x}..0x{:04x}",
                data.len(),
                DATA_AREA.start,
                DATA_AREA.end
            ));
        }
        self.0.write(addr, data)
    }
}

pub fn data_plan(current: &[u8], target: &[u8]) -> Vec<Range<usize>> {
    (0..target.len())
        .step_by(PAGE)
        .filter(|&o| current[o..o + PAGE] != target[o..o + PAGE])
        .map(|o| DATA_AREA.start + o..DATA_AREA.start + o + PAGE)
        .collect()
}

pub fn write_data_area(port: &dyn TasPort, area: &DataArea, host: &mut dyn Host, dry_run: bool) -> Result<DataReport> {
    let target = area.area_bytes();
    host.log(&format!("reading the TAS EEPROM header and data area through the {}…", port.name()));
    if !header_shape_ok(&port.read(0, HEADER_LEN)?) {
        return refused("the TAS EEPROM has no valid boot header: not touching it");
    }
    let current = port.read(DATA_AREA.start, DATA_AREA.len())?;
    let pages = data_plan(&current, &target);
    if pages.is_empty() {
        host.log("the data area already holds this blob.");
        return Ok(DataReport { pages_written: 0, written: true });
    }
    let plan = Plan {
        title: format!("TAS data area (layout {}, {} bytes)", area.version, area.to_bytes().len()),
        writes: vec![format!(
            "U6002 0x{:04x}..0x{:04x}: {} page write(s) of {PAGE} bytes, each read back; header and code untouched",
            pages[0].start,
            pages[pages.len() - 1].end - 1,
            pages.len()
        )],
        warnings: vec!["Development only: the stock firmware ignores this area; nothing changes at boot.".into()],
    };
    for line in plan.lines() {
        host.log(&line);
    }
    if dry_run {
        host.log("dry run: nothing written.");
        return Ok(DataReport { pages_written: 0, written: false });
    }
    if !host.confirm(&plan) {
        return Err(Error::Cancelled);
    }
    let guard = DataOnly(port);
    host.keep_awake(true);
    let r = pages.iter().try_for_each(|p| guard.write(p.start, &target[p.start - DATA_AREA.start..p.end - DATA_AREA.start]));
    host.keep_awake(false);
    r?;

    let back = port.read(DATA_AREA.start, DATA_AREA.len())?;
    if back != target {
        return Err(Error::Verify("data area differs after writing (re-run to resume)".into()));
    }
    DataArea::parse(&back).map_err(|e| Error::Verify(e.to_string()))?;
    host.log("data area written and verified (CRC-32 OK).");
    Ok(DataReport { pages_written: pages.len(), written: true })
}
