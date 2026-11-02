use oc_ais::eeprom::layout::{BOOT0, BOOT1, BODY, HDR};

use super::bundle::DspBundle;
use super::survey::{Boot0State, Survey};
use crate::error::{refused, Result};
use crate::host::Plan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Step {
    /// Above the stock image: the stock firmware keeps booting.
    Boot1 { pages: Vec<(usize, usize)> },
    /// M3: boot0, the only irrecoverable window.
    Boot0,
    /// M4: header page := zeros, so that boot1 enters rescue until the new header is written.
    InvalidateHeader,
    Body { pages: Vec<(usize, usize)> },
    Erase { pages: Vec<(usize, usize)> },
    /// Not boot-critical: without them the firmware falls back to the EQ path for that profile.
    Graph { pages: Vec<(usize, usize)> },
    /// Always last.
    Header { seq: u32 },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct DspPlan {
    pub steps: Vec<Step>,
}

impl DspPlan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn has_boot0(&self) -> bool {
        self.steps.contains(&Step::Boot0)
    }

    pub fn describe(&self, b: &DspBundle) -> Plan {
        let span = |p: &[(usize, usize)]| match (p.first(), p.last()) {
            (Some(f), Some(l)) => format!("0x{:04x}..0x{:04x}", f.0, l.0 + l.1 - 1),
            _ => "-".into(),
        };
        let mut writes = Vec::new();
        let mut warnings = Vec::new();
        for s in &self.steps {
            writes.push(match s {
                Step::Boot1 { pages } => format!(
                    "M2  U7001 {} boot1: {} of {} pages ({} bytes), above the stock image, the stock firmware still boots",
                    span(pages),
                    pages.len(),
                    BOOT1.len() / 128,
                    pages.iter().map(|p| p.1).sum::<usize>()
                ),
                Step::Boot0 => format!(
                    "M3  U7001 0x{:04x}..0x{:04x} boot0 (2 pages, CRC 0x{:08x}) in ONE updater command: CRITICAL WINDOW (~17 ms)",
                    BOOT0.start,
                    BOOT0.end - 1,
                    crc32fast::hash(&b.boot0)
                ),
                Step::InvalidateHeader => {
                    format!("M4  U7001 0x{HDR:04x}..0x{:04x} image header := zeros (DSP stays in rescue until the end)", HDR + 127)
                }
                Step::Body { pages } => format!(
                    "M4  U7001 {} firmware body: {} page(s), body {} bytes, CRC 0x{:08x}",
                    span(pages),
                    pages.len(),
                    b.app.body.bytes.len(),
                    b.app.header.body_crc
                ),
                Step::Erase { pages } => {
                    format!("M4  U7001 {} boot-state / profile pages still holding stock bytes := 0xFF ({} page(s))", span(pages), pages.len())
                }
                Step::Graph { pages } => format!(
                    "M4  U7001 {} graph data pages (last page of each profile copy, profiles untouched): {} page(s), {} bytes of stock graph tables",
                    span(pages),
                    pages.len(),
                    b.app.graph.as_ref().map_or(0, |g| g.payload.len())
                ),
                Step::Header { seq } => format!(
                    "M4  U7001 0x{HDR:04x}..0x{:04x} image header OCI1 seq {seq}, entry 0x{:08x}, fw 0x{:08x}, written last",
                    HDR + 127,
                    b.app.header.entry,
                    b.app.header.fw_version
                ),
            });
        }
        if self.has_boot0() {
            warnings.push(
                "During M3 (~17 ms) a power loss or a DSP reset bricks the speaker (U7001 then needs a programmer). \
                 Keep the speaker powered and the USB cable in place; the computer is kept awake."
                    .into(),
            );
        }
        if !self.steps.is_empty() {
            warnings.push("Audio stops during the install. Keep the volume low.".into());
        }
        Plan { title: format!("DSP EEPROM (U7001): {} step(s)", self.steps.len()), writes, warnings }
    }
}

pub fn plan(s: &Survey, b: &DspBundle) -> Result<DspPlan> {
    let mut steps = Vec::new();
    match s.boot0 {
        Boot0State::Stock if s.stock_intact != Some(true) => {
            return refused(
                "boot0 is stock but the EEPROM differs from the stock backup below 0xDAD8: unknown content, not touching it",
            )
        }
        _ => {}
    }
    // boot1 is written once, during the migration, while the stock boot0 still boots the stock image (every
    // interruption is harmless then). Once our boot0 is in place, boot0 jumps into boot1: rewriting it would turn
    // every page write into a brick window, so an installed boot1 is never rewritten (boot/docs/LAYOUT.md), even
    // if a newer boot1 build exists. Image, graph and profile updates do not depend on boot1's version.
    if !s.boot1_bad.is_empty() && s.boot0 != Boot0State::Ours {
        steps.push(Step::Boot1 { pages: s.boot1_bad.clone() });
    }
    if s.boot0 != Boot0State::Ours {
        steps.push(Step::Boot0);
    }
    let image_ok = s.image_is_target(b);
    if !image_ok {
        if s.header.is_some() {
            steps.push(Step::InvalidateHeader);
        }
        if !s.body_bad.is_empty() {
            steps.push(Step::Body { pages: s.body_bad.clone() });
        }
    }
    if !s.stale_pages.is_empty() {
        steps.push(Step::Erase { pages: s.stale_pages.clone() });
    }
    if !s.graph_bad.is_empty() {
        steps.push(Step::Graph { pages: s.graph_bad.clone() });
    }
    if !image_ok {
        steps.push(Step::Header { seq: s.prev_seq.wrapping_add(1).max(1) });
    }
    debug_assert!(steps.iter().all(|st| match st {
        Step::Body { pages } => pages.iter().all(|p| BODY.contains(&p.0)),
        _ => true,
    }));
    Ok(DspPlan { steps })
}
