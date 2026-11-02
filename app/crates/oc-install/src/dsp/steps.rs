//! Execution of the plan steps. Every write is read back by the updater; every step ends with page CRCs.

use oc_ais::eeprom::layout::{BOOT1, EE_PAGE, HDR};

use super::bundle::{pages, DspBundle};
use super::critical;
use super::plan::Step;
use super::updater::Updater;
use crate::error::{Error, Result};
use crate::host::Host;

pub(crate) fn check_pages(u: &Updater, list: &[(usize, usize)], expected: impl Fn(usize, usize) -> Vec<u8>, what: &str) -> Result<()> {
    for &(a, n) in list {
        if u.ee_crc(a, n)? != crc32fast::hash(&expected(a, n)) {
            return Err(Error::Verify(format!("{what}: CRC mismatch at 0x{a:04x} (re-run to resume)")));
        }
    }
    Ok(())
}

pub(crate) fn write_pages(u: &Updater, list: &[(usize, usize)], data: impl Fn(usize, usize) -> Vec<u8>, what: &str, host: &mut dyn Host) -> Result<()> {
    for (i, &(a, n)) in list.iter().enumerate() {
        u.ee_write(a, &data(a, n))?;
        if (i + 1) % 32 == 0 || i + 1 == list.len() {
            host.log(&format!("  {what}: {}/{} pages", i + 1, list.len()));
        }
    }
    Ok(())
}

fn boot1_all(u: &Updater, b: &DspBundle) -> Result<()> {
    check_pages(u, &pages(BOOT1.start, BOOT1.len()), |a, n| b.boot1_at(a, n).to_vec(), "boot1")
}

/// M3. Precondition checked here again, right before the window: boot1 verified page by page.
fn boot0(u: &Updater, b: &DspBundle, host: &mut dyn Host) -> Result<()> {
    boot1_all(u, b).map_err(|e| Error::Refused(format!("boot1 is not verified, boot0 NOT written: {e}")))?;
    critical::write_boot0(u, &b.boot0, "M3", "oc install dsp", host)
}

pub fn run(u: &Updater, b: &DspBundle, step: &Step, host: &mut dyn Host) -> Result<()> {
    match step {
        Step::Boot1 { pages: list } => {
            host.log("M2: writing boot1 (stock firmware still boots during this step)…");
            u.unlock_boot()?;
            write_pages(u, list, |a, n| b.boot1_at(a, n).to_vec(), "boot1", host)?;
            boot1_all(u, b)?;
            host.log("M2: boot1 verified (all 73 pages).");
        }
        Step::Boot0 => boot0(u, b, host)?,
        Step::InvalidateHeader => {
            host.log("M4: invalidating the image header…");
            u.ee_write(HDR, &[0u8; EE_PAGE])?;
            check_pages(u, &[(HDR, EE_PAGE)], |_, n| vec![0; n], "header invalidation")?;
        }
        Step::Body { pages: list } => {
            host.log("M4: writing the firmware body…");
            write_pages(u, list, |a, n| b.body_at(a, n).to_vec(), "body", host)?;
            check_pages(u, &b.body_pages(), |a, n| b.body_at(a, n).to_vec(), "body")?;
            host.log("M4: body verified.");
        }
        Step::Erase { pages: list } => {
            host.log("M4: erasing stock bytes in the boot-state page / profile area…");
            write_pages(u, list, |_, n| vec![0xFF; n], "erase", host)?;
            check_pages(u, list, |_, n| vec![0xFF; n], "erase")?;
        }
        Step::Graph { pages: list } => {
            host.log("M4: writing the graph data pages…");
            write_pages(u, list, |a, _| b.graph_at(a), "graph", host)?;
            check_pages(u, list, |a, _| b.graph_at(a), "graph data")?;
        }
        Step::Header { seq } => {
            host.log(&format!("M4: writing the image header (seq {seq}), last…"));
            let page = b.header_page(*seq);
            u.ee_write(HDR, &page)?;
            check_pages(u, &[(HDR, EE_PAGE)], |_, _| page.to_vec(), "header")?;
        }
    }
    Ok(())
}
