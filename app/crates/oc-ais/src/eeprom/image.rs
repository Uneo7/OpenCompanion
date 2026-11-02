use super::body::{self, Body};
use super::graph::{self, GraphData};
use super::boot::{app_declares_layout, boot0_from_elf, boot1_from_elf, APP_LAYOUT_SYMBOL};
use super::header::{ImageHeader, HF_TRIAL};
use super::layout::{BOOT0, BOOT1, BODY, EE_PAGE, EE_SIZE, HDR, LAST_PAGE, PROFILES};
use crate::{segments_from_elf, Error, Result};

#[derive(Debug, Clone, Default)]
pub struct AppOptions {
    /// Sequence number written in the header (the updater uses previous + 1).
    pub seq: u32,
    pub fw_version: u32,
    /// Set HF_TRIAL: only for a firmware that confirms itself (see docs/UPDATE.md).
    pub trial: bool,
    /// Accept an ELF without the layout symbol (RAM tests only: never write such an image).
    pub allow_legacy_app: bool,
}

#[derive(Debug, Clone)]
pub struct AppImage {
    pub header: ImageHeader,
    pub header_page: [u8; EE_PAGE],
    pub body: Body,
    /// Stock graph program tables (`.bose_tab`), written to the graph data pages, not to the body. `None` for a
    /// firmware without them.
    pub graph: Option<GraphData>,
}

pub fn app_image(app_elf: &[u8], opts: &AppOptions) -> Result<AppImage> {
    if !opts.allow_legacy_app && !app_declares_layout(app_elf)? {
        return Err(Error::Layout(format!(
            "the firmware does not define `{APP_LAYOUT_SYMBOL}`: it may still write profiles at 0xE000..0xF8FF, over boot1"
        )));
    }
    let (mut segs, fills, entry) = segments_from_elf(app_elf)?;
    let graph = graph::split(app_elf, &mut segs)?;
    let body = body::build(&segs, &fills, entry)?;
    let header = ImageHeader {
        seq: opts.seq,
        body_len: body.bytes.len() as u32,
        body_crc: crc32fast::hash(&body.bytes),
        entry,
        flags: if opts.trial { HF_TRIAL } else { 0 },
        fw_version: opts.fw_version,
        raw_len: body.raw_len,
    };
    Ok(AppImage { header, header_page: header.to_page(), body, graph })
}

pub struct EepromInputs<'a> {
    pub tramp_elf: &'a [u8],
    pub boot1_elf: &'a [u8],
    pub app: Option<&'a AppImage>,
    /// Current EEPROM content (64 KB dump). The last page is always taken from it; the profile area only
    /// with `keep_profiles` (it must already be in the new layout, otherwise it holds stock image bytes), except
    /// the graph data pages, which always come from the application.
    pub dump: Option<&'a [u8]>,
    pub keep_profiles: bool,
}

pub fn eeprom_image(inp: &EepromInputs) -> Result<Vec<u8>> {
    let mut ee = vec![0xFFu8; EE_SIZE];
    ee[BOOT0].copy_from_slice(&boot0_from_elf(inp.tramp_elf)?);
    ee[BOOT1].copy_from_slice(&boot1_from_elf(inp.boot1_elf)?);
    if let Some(app) = inp.app {
        ee[HDR..HDR + EE_PAGE].copy_from_slice(&app.header_page);
        ee[BODY.start..BODY.start + app.body.bytes.len()].copy_from_slice(&app.body.bytes);
    }
    if let Some(d) = inp.dump {
        if d.len() != EE_SIZE {
            return Err(Error::Layout(format!("dump is {} bytes, expected {EE_SIZE}", d.len())));
        }
        ee[LAST_PAGE].copy_from_slice(&d[LAST_PAGE]);
        if inp.keep_profiles {
            ee[PROFILES].copy_from_slice(&d[PROFILES]);
        }
    }
    if let Some(g) = inp.app.and_then(|a| a.graph.as_ref()) {
        for (a, p) in g.pages() {
            ee[a..a + EE_PAGE].copy_from_slice(&p);
        }
    }
    Ok(ee)
}
