use oc_protocol::Link;

use super::bundle::DspBundle;
use super::enter::{enter_updater, reboot, settle_after_verify};
use super::plan::{plan, DspPlan};
use super::runtime::DspRuntime;
use super::steps;
use super::survey::{survey, Boot0State, Survey};
use super::updater::Updater;
use crate::error::{Error, Result};
use crate::host::Host;

#[derive(Debug, Clone, Copy, Default)]
pub struct DspOptions {
    /// No EEPROM write; the DSP is rebooted back to what its EEPROM holds.
    pub dry_run: bool,
}

#[derive(Debug, Clone)]
pub struct DspReport {
    pub before: DspRuntime,
    pub plan: DspPlan,
    pub after: DspRuntime,
    pub installed: bool,
}

fn log_survey(s: &Survey, host: &mut dyn Host) {
    host.log(&format!(
        "survey: boot0 {:?}, boot1 {} page(s) to write, header {}, body {} page(s) to write, {} stale page(s), {} graph page(s) to write, last page {}",
        s.boot0,
        s.boot1_bad.len(),
        s.header.map_or("invalid".to_string(), |h| format!("seq {} fw 0x{:08x}", h.seq, h.fw_version)),
        s.body_bad.len(),
        s.stale_pages.len(),
        s.graph_bad.len(),
        if s.last_page_ok { "stock" } else { "differs from the backup (never written)" }
    ));
}

fn apply(u: &Updater, b: &DspBundle, p: &DspPlan, host: &mut dyn Host) -> Result<()> {
    for s in &p.steps {
        steps::run(u, b, s, host)?;
    }
    host.log("M5: verifying the whole layout again…");
    let after = survey(u, b)?;
    log_survey(&after, host);
    let rest = plan(&after, b)?;
    if after.boot0 != Boot0State::Ours || !after.image_is_target(b) || !rest.is_empty() {
        return Err(Error::Verify(format!("the EEPROM still needs {} step(s) after writing (re-run to resume)", rest.steps.len())));
    }
    Ok(())
}

/// Safe to re-run after any interruption.
pub fn install(link: &dyn Link, b: &DspBundle, host: &mut dyn Host, opts: DspOptions) -> Result<DspReport> {
    b.validate()?;
    let before = enter_updater(link, &b.loaders(), host, opts.dry_run)?;
    let u = Updater::new(link);
    match u.info()? {
        (b'U', _) => {}
        (m, r) => return Err(Error::Device(format!("expected the updater, INFO says mode {} reason {r}", m as char))),
    }
    host.log("reading U7001 (no write)…");
    let s = survey(&u, b)?;
    log_survey(&s, host);
    let p = match plan(&s, b) {
        Ok(p) => p,
        Err(e) => {
            if s.boot0 == Boot0State::Stock {
                reboot(&u, host);
            }
            return Err(e);
        }
    };
    let summary = p.describe(b);
    for line in summary.lines() {
        host.log(&line);
    }
    if opts.dry_run || p.is_empty() {
        if opts.dry_run {
            host.log("dry run: no EEPROM byte written; rebooting the DSP from its EEPROM.");
        } else {
            host.log("U7001 already holds this firmware; rebooting the DSP.");
        }
        if s.boot0 == Boot0State::Broken {
            host.log("WARNING: boot0 is not valid: NOT rebooting (the DSP would not boot). Run the install now.");
            return Ok(DspReport { before, plan: p, after: DspRuntime::Updater { boot1: 0 }, installed: false });
        }
        let after = reboot(&u, host);
        let installed = !opts.dry_run && {
            let (_, ok) = settle_after_verify(u.link(), after.clone(), |rt| matches!(rt, DspRuntime::OcFirmware { .. }), "the DSP firmware", host);
            ok
        };
        return Ok(DspReport { before, plan: p, after, installed });
    }
    if !host.confirm(&summary) {
        if s.boot0 == Boot0State::Broken {
            host.log("WARNING: boot0 is not valid; the DSP will not boot after a power loss. Re-run soon.");
            return Err(Error::Cancelled);
        }
        reboot(&u, host);
        return Err(Error::Cancelled);
    }
    host.keep_awake(true);
    let r = apply(&u, b, &p, host);
    host.keep_awake(false);
    r?;
    host.log("M5: rebooting the DSP from U7001…");
    let after = reboot(&u, host);
    // Every write was read back: a DSP that does not answer properly now is a warning, not a failure.
    let (after, installed) = settle_after_verify(u.link(), after, |rt| matches!(rt, DspRuntime::OcFirmware { .. }), "the new DSP firmware", host);
    if !installed {
        return Err(Error::Verify(format!("the new firmware did not answer SVM after the reboot (DSP: {after})")));
    }
    Ok(DspReport { before, plan: p, after, installed })
}
