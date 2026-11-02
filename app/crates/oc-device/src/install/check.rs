//! DSP: ELF version string vs `SVM`; the build stamp changes with every build, so two builds never compare equal.
//! TAS: equal versions are still an update when the EEPROM differs from the install plan (dev builds keep the version).

use std::sync::{Arc, Mutex};
use std::time::Duration;

use oc_install::tas::{self, OcPort, TasPort, EE_SIZE};

use super::flow::Chip;
use super::host::{Notice, Shared};
use super::runner;
use super::sources::{tas_version, FirmwareSources};
use super::versions::{self, RunningVersions};
use crate::link::Conn;
use crate::worker::LinkLease;

const LEASE_TIMEOUT: Duration = Duration::from_secs(5);
const TRIES: usize = 3;
/// Longest wait for a DSP that does not answer yet (rebooting after an install).
const DSP_WAIT: Duration = Duration::from_secs(20);
const PAUSE: Duration = Duration::from_millis(400);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChipUpdate {
    pub chip: Chip,
    /// Running version: `None` is the stock Bose firmware when [`ChipUpdate::from_stock`], else unknown
    /// ([`super::versions::label`]).
    pub from: Option<String>,
    pub from_stock: bool,
    /// `from` unknown, most likely the Bose firmware (garbled reply).
    pub from_probable: bool,
    /// `None` = no firmware file.
    pub to: Option<String>,
    pub available: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum UpdateCheck {
    #[default]
    Idle,
    Running,
    Done(Vec<ChipUpdate>),
    Failed(String),
}

impl UpdateCheck {
    pub fn available(&self) -> bool {
        matches!(self, Self::Done(c) if c.iter().any(|u| u.available))
    }

    pub fn chip(&self, chip: Chip) -> Option<&ChipUpdate> {
        match self {
            Self::Done(c) => c.iter().find(|u| u.chip == chip),
            _ => None,
        }
    }
}

/// `0.1.0+20261101183000` from `OC0.1.0+20261101183000` (SVM) or the same without the prefix.
pub fn normalize(v: &str) -> &str {
    v.trim().strip_prefix("OC").unwrap_or(v.trim())
}

/// What the check reports for the DSP: the stock firmware and other firmwares are updates; ours when its
/// version differs; a DSP that does not answer is never reported as an update.
fn dsp_available(running: &versions::Running, bundled: Option<&str>) -> bool {
    match running {
        versions::Running::Unknown | versions::Running::Ours(None) => false,
        // A garbled reply (most likely the Bose firmware next to our USB controller): the install applies.
        versions::Running::Stock | versions::Running::ProbablyStock | versions::Running::Other(_) => bundled.is_some(),
        versions::Running::Ours(Some(v)) => dsp_differs(Some(v), bundled),
    }
}

/// Unknown versions count as different.
pub fn dsp_differs(running: Option<&str>, bundled: Option<&str>) -> bool {
    match (running, bundled) {
        (_, None) => false,
        (Some(r), Some(b)) => normalize(r) != normalize(b),
        (None, Some(_)) => true,
    }
}

struct Targets {
    dsp: Option<String>,
    usb: Option<String>,
    tas: Option<oc_install::tas::TasTarget>,
}

fn local_targets(sources: &FirmwareSources) -> Targets {
    let tas = sources.tas_target(None).ok();
    Targets { dsp: sources.dsp_app_version(None), usb: tas.as_ref().and_then(tas_version), tas }
}

fn check(shared: &Shared, known: &RunningVersions) -> Result<Vec<ChipUpdate>, String> {
    let targets = refresh_release(shared).unwrap_or_else(|| local_targets(&shared.sources()));
    let Targets { dsp: dsp_to, usb: usb_to, tas: target } = targets;
    let conn = open_with_retries()?;
    let mut now = versions::read(&conn);
    // Right after an install the DSP may still be rebooting: wait for it (a silent DSP never means "update").
    if now.dsp == versions::Running::Unknown {
        now.dsp = versions::wait_dsp(&conn, DSP_WAIT);
    }
    // The DSP is what it says now (the worker's view may predate an install that just ended).
    let (dsp, usb) = (now.dsp, now.usb.or(&known.usb));
    let (dsp_from, usb_from) = (dsp.version().map(|v| normalize(&v).to_owned()), usb.version());
    let usb_available = match (&conn, &target) {
        // A release not downloaded yet: by version.
        (_, None) => usb_to.is_some() && usb_from.as_deref() != usb_to.as_deref(),
        (Conn::Oc(d), Some(t)) => {
            let port = OcPort::new(d);
            let current = read_ee(&port)?;
            tas::plan(&current, t, &port.writable()).map_or(true, |p| !p.is_empty())
        }
        // The stock Bose firmware: the first install applies.
        (Conn::Stock(_), Some(_)) => true,
    };
    Ok(vec![
        ChipUpdate {
            chip: Chip::Dsp,
            available: dsp_available(&dsp, dsp_to.as_deref()),
            from: dsp_from,
            from_stock: dsp.is_stock(),
            from_probable: dsp.is_probably_stock(),
            to: dsp_to,
        },
        ChipUpdate { chip: Chip::Usb, available: usb_available, from: usb_from, from_stock: usb.is_stock(), from_probable: false, to: usb_to },
    ])
}

/// With GitHub releases (feature `github`, enabled by the controller): reads the latest release. Downloaded
/// already: its folder becomes the installer's files. Not yet: the update will download it (`pending`). GitHub
/// not reachable (offline, rate limit): the last downloaded release stays, silently, with a status line.
/// `None`: releases are not used, or none is known (the local files then).
#[cfg(feature = "github")]
fn refresh_release(shared: &Shared) -> Option<Targets> {
    use super::release::{github, store};
    let (api, repo) = shared.release().remote.clone()?;
    let root = crate::paths::firmware_dir();
    let cached = store::latest(&root);
    let fetched = github::latest(&api, &repo).and_then(|r| github::available(&r));
    let mut rel = shared.release();
    match fetched {
        Ok(av) if !av.manifest.app_supported(env!("CARGO_PKG_VERSION")) => {
            rel.status = Some(super::host::ReleaseStatus::NeedsApp { tag: av.tag.clone(), min: av.manifest.min_app_version.clone() });
            rel.pending = None;
        }
        Ok(av) => {
            if let Some(dir) = store::cached(&root, &av.tag) {
                shared.set_sources(super::FirmwareSources::from_dir(&dir));
                rel.status = Some(super::host::ReleaseStatus::InUse { tag: av.tag.clone() });
                rel.pending = None;
            } else {
                rel.status = Some(super::host::ReleaseStatus::Available { tag: av.tag.clone() });
                let t = Targets { dsp: Some(av.manifest.dsp_version.clone()), usb: Some(av.manifest.tas_version.clone()), tas: None };
                rel.pending = Some(av);
                return Some(t);
            }
        }
        Err(e) => {
            rel.status = Some(super::host::ReleaseStatus::Offline { cached: cached.as_ref().map(|(tag, ..)| tag.clone()), error: e.to_string() });
        }
    }
    drop(rel);
    // No release folder in use yet (first run): the newest downloaded one.
    if let Some((_, dir, _)) = &cached {
        if !shared.sources().dsp_app.starts_with(&root) {
            shared.set_sources(super::FirmwareSources::from_dir(dir));
        }
    }
    Some(local_targets(&shared.sources()))
}

#[cfg(not(feature = "github"))]
fn refresh_release(_shared: &Shared) -> Option<Targets> {
    None
}

/// It may take a moment to open again after the worker closed it.
fn open_with_retries() -> Result<Conn, String> {
    for _ in 0..TRIES {
        if let Ok(c) = runner::open() {
            return Ok(c);
        }
        std::thread::sleep(PAUSE);
    }
    Err("no speaker found on USB".into())
}

/// Each attempt is retried by the port itself.
fn read_ee(port: &OcPort) -> Result<Vec<u8>, String> {
    let mut last = String::new();
    for _ in 0..TRIES {
        match port.read(0, EE_SIZE) {
            Ok(v) => return Ok(v),
            Err(e) => last = e.to_string(),
        }
        std::thread::sleep(PAUSE);
    }
    Err(format!("reading the USB controller EEPROM: {last}"))
}

/// Borrows the speaker from the worker for a few hundred ms.
pub(super) fn run(lease: &LinkLease, state: &Arc<Mutex<UpdateCheck>>, shared: &Shared) {
    let result = match lease.acquire(LEASE_TIMEOUT) {
        Some(guard) => {
            let r = check(shared, &shared.running());
            drop(guard);
            r
        }
        None => Err("the speaker is busy".into()),
    };
    super::log_file::append(&[format!("update check: {result:?}")]);
    *state.lock().unwrap_or_else(|e| e.into_inner()) = match result {
        Ok(c) => UpdateCheck::Done(c),
        Err(e) => UpdateCheck::Failed(e),
    };
    shared.notify(Notice::Changed);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn versions_compare_without_the_prefix_and_with_the_build_stamp() {
        assert_eq!(normalize("OC0.1.0+20261101"), "0.1.0+20261101");
        assert!(!dsp_differs(Some("OC0.1.0+1"), Some("0.1.0+1")));
        assert!(dsp_differs(Some("OC0.1.0+1"), Some("0.1.0+2")));
        assert!(dsp_differs(Some("OC0.0.1"), Some("0.1.0+2")));
        assert!(dsp_differs(None, Some("0.1.0+2")));
        assert!(!dsp_differs(Some("OC0.0.1"), None));
        // Unknown right after an install: never an update.
        assert!(!dsp_available(&versions::Running::Unknown, Some("0.1.0+2")));
        assert!(!dsp_available(&versions::Running::Ours(None), Some("0.1.0+2")));
        assert!(dsp_available(&versions::Running::Stock, Some("0.1.0+2")));
        assert!(dsp_available(&versions::Running::Ours(Some("OC0.0.1".into())), Some("0.1.0+2")));
        let c = UpdateCheck::Done(vec![ChipUpdate { chip: Chip::Usb, from: None, from_stock: false, from_probable: false, to: None, available: true }]);
        assert!(c.available() && c.chip(Chip::Dsp).is_none());
    }
}
