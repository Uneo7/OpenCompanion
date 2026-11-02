//! Execution of the restore steps. Every write is read back by the updater; every step ends with page CRCs.

use oc_ais::eeprom::layout::{BOOT1, EE_PAGE, HDR};

use super::image::DspRestoreInputs;
use super::plan::RestoreStep;
use crate::dsp::critical;
use crate::dsp::steps::{check_pages, write_pages};
use crate::dsp::survey::spans;
use crate::dsp::Updater;
use crate::error::{Error, Result};
use crate::host::Host;
use crate::ui::UiStep;

const RERUN: &str = "oc install restore --dsp";

fn stock(img: &DspRestoreInputs) -> impl Fn(usize, usize) -> Vec<u8> + '_ {
    move |a, n| img.at(a, n).to_vec()
}

/// R3, with its precondition checked right before the window: the whole stock image below boot1 is in place.
fn boot0(u: &Updater, img: &DspRestoreInputs, host: &mut dyn Host) -> Result<()> {
    host.step(UiStep::DspCritical);
    host.log("R3: checking the stock image below 0xDB00 before the switch…");
    check_pages(u, &spans(HDR, BOOT1.start), stock(img), "stock image")
        .map_err(|e| Error::Refused(format!("the stock image is not complete, boot0 NOT written: {e}")))?;
    critical::write_boot0(u, &img.boot0(), "R3", RERUN, host)
}

pub(crate) fn run(u: &Updater, img: &DspRestoreInputs, step: &RestoreStep, host: &mut dyn Host) -> Result<()> {
    match step {
        RestoreStep::InvalidateHeader => {
            host.step(UiStep::RestoreDsp);
            host.log("R1: invalidating the OpenCompanion image header (rescue mode until the switch)…");
            u.ee_write(HDR, &[0u8; EE_PAGE])?;
            check_pages(u, &[(HDR, EE_PAGE)], |_, n| vec![0; n], "header invalidation")?;
        }
        RestoreStep::Body { pages } => {
            host.step(UiStep::RestoreDsp);
            host.log("R1: writing the stock image body (our boot chain still guards it)…");
            write_pages(u, pages, stock(img), "stock body", host)?;
            check_pages(u, pages, stock(img), "stock body")?;
        }
        RestoreStep::Tail { pages } => {
            host.step(UiStep::RestoreDsp);
            host.log("R2: restoring the boot-state page and the profile area…");
            write_pages(u, pages, stock(img), "stock tail", host)?;
            check_pages(u, pages, stock(img), "stock tail")?;
        }
        RestoreStep::StockHeader => {
            host.step(UiStep::RestoreDsp);
            host.log("R2: writing the stock bytes of the header page…");
            u.ee_write(HDR, img.at(HDR, EE_PAGE))?;
            check_pages(u, &[(HDR, EE_PAGE)], stock(img), "header page")?;
        }
        RestoreStep::Boot0 => boot0(u, img, host)?,
        RestoreStep::Boot1Area { pages } => {
            host.step(UiStep::DspVerify);
            host.log("R4: restoring the boot1 area 0xDB00..0xFF7F (the stock firmware already boots)…");
            u.unlock_boot()?; // a resumed run starts a new updater session: the unlock is gone
            write_pages(u, pages, stock(img), "boot1 area", host)?;
            check_pages(u, pages, stock(img), "boot1 area")?;
        }
    }
    Ok(())
}
