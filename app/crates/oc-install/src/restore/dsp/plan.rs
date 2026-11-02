use oc_ais::eeprom::layout::{BOOT0, BODY, HDR, PROFILES, STATE};

use super::survey::{Boot0Kind, RestoreSurvey};
use crate::error::{refused, Result};
use crate::host::Plan;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RestoreStep {
    /// So that boot1 stays in rescue until the end.
    InvalidateHeader,
    Body { pages: Vec<(usize, usize)> },
    Tail { pages: Vec<(usize, usize)> },
    /// Last write guarded by rescue mode.
    StockHeader,
    /// The critical window.
    Boot0,
    /// Only once the stock firmware boots again.
    Boot1Area { pages: Vec<(usize, usize)> },
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RestorePlan {
    pub steps: Vec<RestoreStep>,
    pub warnings: Vec<String>,
}

impl RestorePlan {
    pub fn is_empty(&self) -> bool {
        self.steps.is_empty()
    }

    pub fn has_boot0(&self) -> bool {
        self.steps.contains(&RestoreStep::Boot0)
    }

    pub fn describe(&self) -> Plan {
        let span = |p: &[(usize, usize)]| match (p.first(), p.last()) {
            (Some(f), Some(l)) => format!("0x{:04x}..0x{:04x}, {} page(s)", f.0, l.0 + l.1 - 1, p.len()),
            _ => "-".into(),
        };
        let writes = self
            .steps
            .iter()
            .map(|s| match s {
                RestoreStep::InvalidateHeader => {
                    format!("R1  U7001 0x{HDR:04x}..0x{:04x} OpenCompanion image header := zeros (rescue mode until R3)", HDR + 127)
                }
                RestoreStep::Body { pages } => format!("R1  U7001 {} stock image body (our boot chain still guards it)", span(pages)),
                RestoreStep::Tail { pages } => format!("R2  U7001 {} boot-state page / profiles := stock (profiles are lost)", span(pages)),
                RestoreStep::StockHeader => format!("R2  U7001 0x{HDR:04x}..0x{:04x} stock bytes over the image header page", HDR + 127),
                RestoreStep::Boot0 => format!(
                    "R3  U7001 0x{:04x}..0x{:04x} stock boot0 (2 pages) in ONE updater command: CRITICAL WINDOW (~17 ms)",
                    BOOT0.start,
                    BOOT0.end - 1
                ),
                RestoreStep::Boot1Area { pages } => {
                    format!("R4  U7001 {} boot1 area := stock (the stock firmware already boots)", span(pages))
                }
            })
            .collect();
        let mut warnings = self.warnings.clone();
        if self.has_boot0() {
            warnings.push(
                "During R3 (~17 ms) a power loss or a DSP reset bricks the speaker (U7001 then needs a programmer). \
                 Keep the speaker powered and the USB cable in place; the computer is kept awake."
                    .into(),
            );
            warnings.push("After R3 the OpenCompanion rescue mode is gone: the DSP runs the Bose firmware again.".into());
        }
        if !self.steps.is_empty() {
            warnings.push("Audio stops during the restore. Keep the volume low.".into());
        }
        Plan { title: format!("DSP EEPROM (U7001) back to the Bose firmware: {} step(s)", self.steps.len()), writes, warnings }
    }
}

fn within(list: &[(usize, usize)], keep: impl Fn(usize) -> bool) -> Vec<(usize, usize)> {
    list.iter().copied().filter(|p| keep(p.0)).collect()
}

pub fn plan(s: &RestoreSurvey) -> Result<RestorePlan> {
    if !s.last_page_ok {
        return refused(
            "the last U7001 page (never written by OpenCompanion) differs from the backup: this backup does not come \
             from this speaker",
        );
    }
    let mut p = RestorePlan::default();
    if s.boot0 == Boot0Kind::Stock {
        if !s.low_bad.is_empty() {
            return refused(format!(
                "boot0 is stock but {} page(s) below 0xDB00 differ from the backup: unknown content, not touching it",
                s.low_bad.len()
            ));
        }
    } else {
        match (s.boot0, s.boot1_ok) {
            (Boot0Kind::Unknown, _) => p.warnings.push(
                "boot0 is neither stock nor OpenCompanion (interrupted critical window): the DSP cannot boot until R3. \
                 Do not power off the speaker before the end."
                    .into(),
            ),
            (_, Some(false)) => p.warnings.push(
                "boot1 fails its own CRC: rescue mode is not available, so an interruption before R3 leaves a DSP \
                 that cannot boot."
                    .into(),
            ),
            _ => {}
        }
        if s.oc_header {
            p.steps.push(RestoreStep::InvalidateHeader);
        }
        let body = within(&s.low_bad, |a| BODY.contains(&a));
        if !body.is_empty() {
            p.steps.push(RestoreStep::Body { pages: body });
        }
        let tail = within(&s.low_bad, |a| a == STATE || PROFILES.contains(&a));
        if !tail.is_empty() {
            p.steps.push(RestoreStep::Tail { pages: tail });
        }
        if s.oc_header || s.low_bad.iter().any(|q| q.0 == HDR) {
            p.steps.push(RestoreStep::StockHeader);
        }
        p.steps.push(RestoreStep::Boot0);
    }
    if !s.high_bad.is_empty() {
        p.steps.push(RestoreStep::Boot1Area { pages: s.high_bad.clone() });
    }
    Ok(p)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn survey(boot0: Boot0Kind, low: &[usize], high: &[usize], oc_header: bool) -> RestoreSurvey {
        RestoreSurvey {
            boot0,
            oc_header,
            boot1_ok: (boot0 != Boot0Kind::Stock).then_some(true),
            low_bad: low.iter().map(|&a| (a, 128)).collect(),
            high_bad: high.iter().map(|&a| (a, 128)).collect(),
            last_page_ok: true,
        }
    }

    #[test]
    fn order_is_body_then_tail_then_header_then_boot0_then_boot1() {
        let s = survey(Boot0Kind::OpenCompanion, &[0x0100, 0x0180, 0x0200, 0xC200], &[0xDB00], true);
        let kinds: Vec<_> = plan(&s).unwrap().steps.iter().map(std::mem::discriminant).collect();
        let want = [
            RestoreStep::InvalidateHeader,
            RestoreStep::Body { pages: vec![] },
            RestoreStep::Tail { pages: vec![] },
            RestoreStep::StockHeader,
            RestoreStep::Boot0,
            RestoreStep::Boot1Area { pages: vec![] },
        ];
        assert_eq!(kinds, want.iter().map(std::mem::discriminant).collect::<Vec<_>>());
    }

    #[test]
    fn stock_boot0_with_a_different_image_is_refused() {
        assert!(plan(&survey(Boot0Kind::Stock, &[0x4000], &[], false)).is_err());
        let only_r4 = plan(&survey(Boot0Kind::Stock, &[], &[0xDB00], false)).unwrap();
        assert_eq!(only_r4.steps, vec![RestoreStep::Boot1Area { pages: vec![(0xDB00, 128)] }]);
        assert!(plan(&survey(Boot0Kind::Stock, &[], &[], false)).unwrap().is_empty());
    }

    #[test]
    fn foreign_backup_is_refused() {
        let mut s = survey(Boot0Kind::OpenCompanion, &[], &[], true);
        s.last_page_ok = false;
        assert!(plan(&s).is_err());
    }
}
