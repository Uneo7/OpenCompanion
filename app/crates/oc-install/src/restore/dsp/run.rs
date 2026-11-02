use oc_ais::eeprom::layout::EE_SIZE;
use oc_protocol::Link;

use super::image::DspRestoreInputs;
use super::plan::{plan, RestorePlan};
use super::steps;
use super::survey::{survey, Boot0Kind, RestoreSurvey};
use crate::dsp::enter::{enter_updater, reboot, settle_after_verify};
use crate::dsp::{DspRuntime, Updater};
use crate::error::{Error, Result};
use crate::host::Host;
use crate::ui::UiStep;

#[derive(Debug, Clone, Copy, Default)]
pub struct DspRestoreOptions {
    /// No EEPROM write (the updater is loaded into DSP RAM); the DSP is rebooted after.
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct DspRestoreReport {
    pub before: DspRuntime,
    pub plan: RestorePlan,
    pub after: DspRuntime,
    pub restored: bool,
    pub crc32: Option<u32>,
}

fn log_survey(s: &RestoreSurvey, host: &mut dyn Host) {
    host.log(&format!(
        "survey: boot0 {:?}, OpenCompanion header {}, {} page(s) below 0xDB00 and {} in the boot1 area differ from \
         the backup, last page {}",
        s.boot0,
        if s.oc_header { "present" } else { "absent" },
        s.low_bad.len(),
        s.high_bad.len(),
        if s.last_page_ok { "matches" } else { "DIFFERS" }
    ));
}

fn verify_all(u: &Updater, img: &DspRestoreInputs, host: &mut dyn Host) -> Result<u32> {
    host.step(UiStep::DspVerify);
    host.log("R5: verifying the whole U7001 against the backup (page CRCs, then a full read)…");
    let s = survey(u, img)?;
    if !s.is_stock() {
        log_survey(&s, host);
        return Err(Error::Verify("U7001 still differs from the backup (re-run to resume)".into()));
    }
    let mut back = Vec::with_capacity(EE_SIZE);
    for (i, a) in (0..EE_SIZE).step_by(4096).enumerate() {
        back.extend(u.ee_read(a, 4096)?);
        host.log(&format!("  full read: {}/{} pages", (i + 1) * 32, EE_SIZE / 128));
    }
    if let Some(at) = back.iter().zip(&img.image).position(|(a, b)| a != b) {
        return Err(Error::Verify(format!("full read differs from the backup at 0x{at:04x} (re-run to resume)")));
    }
    let crc = crc32fast::hash(&back);
    host.log(&format!("R5: U7001 equals the backup (64 KB, CRC-32 0x{crc:08x})."));
    Ok(crc)
}

/// Reboots, unless boot0 is not valid (the DSP would not come back): then the updater keeps running.
fn leave(u: &Updater, s: &RestoreSurvey, host: &mut dyn Host) -> DspRuntime {
    if s.boot0 == Boot0Kind::Unknown {
        host.log("WARNING: boot0 is not valid: NOT rebooting (the DSP would not boot). Run the restore now.");
        return DspRuntime::Updater { boot1: 0 };
    }
    reboot(u, host)
}

/// `D01.01.03` of `D01.01.03U01.01.04` (the `U` part is the USB controller's, restored after the DSP).
fn dsp_part(svm: &str) -> &str {
    svm.split('U').next().unwrap_or(svm).trim()
}

fn finish(u: &Updater, img: &DspRestoreInputs, host: &mut dyn Host) -> Result<(DspRuntime, u32)> {
    let crc = verify_all(u, img, host)?;
    host.log("R5: rebooting the DSP from U7001…");
    let after = reboot(u, host);
    // U7001 is verified: a DSP that does not answer properly now does not undo that (warning, then the TAS).
    let want = img.stock_svm.as_deref().map(dsp_part);
    let is_backed_up = |rt: &DspRuntime| match rt {
        DspRuntime::Stock { svm } => want.is_none_or(|w| dsp_part(svm) == w),
        _ => false,
    };
    let (after, ok) = settle_after_verify(u.link(), after, is_backed_up, "the Bose DSP firmware", host);
    if !ok {
        let want = want.unwrap_or("D...");
        return Err(Error::Verify(format!("the Bose firmware did not answer SVM {want} (the backed-up revision) after the reboot (DSP: {after})")));
    }
    Ok((after, crc))
}

/// Safe to re-run after any interruption.
pub fn restore_dsp(link: &dyn Link, img: &DspRestoreInputs, host: &mut dyn Host, opts: DspRestoreOptions) -> Result<DspRestoreReport> {
    img.validate()?;
    host.step(UiStep::RestoreCheck);
    let before = enter_updater(link, &img.loaders(), host, opts.dry_run)?;
    let u = Updater::new(link);
    match u.info()? {
        (b'U', _) => {}
        (m, r) => return Err(Error::Device(format!("expected the updater, INFO says mode {} reason {r}", m as char))),
    }
    host.log("reading U7001 (no write)…");
    let s = survey(&u, img)?;
    log_survey(&s, host);
    let p = match plan(&s) {
        Ok(p) => p,
        Err(e) => {
            leave(&u, &s, host);
            return Err(e);
        }
    };
    let summary = p.describe();
    for line in summary.lines() {
        host.log(&line);
    }
    if opts.dry_run {
        host.log("dry run: no EEPROM byte written; rebooting the DSP from its EEPROM.");
        let after = leave(&u, &s, host);
        return Ok(DspRestoreReport { before, plan: p, after, restored: false, crc32: None });
    }
    if p.is_empty() {
        host.log("U7001 already holds the backup; verifying it fully.");
        let (after, crc) = finish(&u, img, host)?;
        return Ok(DspRestoreReport { before, plan: p, after, restored: true, crc32: Some(crc) });
    }
    if !host.confirm(&summary) {
        leave(&u, &s, host);
        return Err(Error::Cancelled);
    }
    host.keep_awake(true);
    let r = p.steps.iter().try_for_each(|st| steps::run(&u, img, st, host));
    host.keep_awake(false);
    r?;
    let (after, crc) = finish(&u, img, host)?;
    Ok(DspRestoreReport { before, plan: p, after, restored: true, crc32: Some(crc) })
}
