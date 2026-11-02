//! Says why an install, a backup, a restore or the firmware lookup cannot go on, in words the user can act on
//! (the full English error stays in the install log).

use oc_device::install::{BackupState, ReleaseError, ReleaseStatus, NOT_WRITABLE, NO_FIRMWARE, NO_USB};

use crate::i18n::{t_with, tr};

pub fn release_error(e: &ReleaseError) -> String {
    match e {
        ReleaseError::NoConnection(d) => t_with("fw.release.err.offline", &[("error", d)]),
        ReleaseError::RateLimited => tr("fw.release.err.rate").to_owned(),
        ReleaseError::Http { status } => t_with("fw.release.err.http", &[("status", &status.to_string())]),
        ReleaseError::NoRelease { repo } => t_with("fw.release.err.none", &[("repo", repo)]),
        ReleaseError::NoFirmware { repo } => t_with("fw.release.err.no_fw", &[("repo", repo)]),
        ReleaseError::BadManifest(d) => t_with("fw.release.err.manifest", &[("error", d)]),
    }
}

pub fn release_line(s: &ReleaseStatus) -> String {
    match s {
        ReleaseStatus::InUse { tag } => t_with("fw.release.in_use", &[("tag", tag)]),
        ReleaseStatus::Available { tag } => t_with("fw.release.available", &[("tag", tag)]),
        ReleaseStatus::NeedsApp { tag, min } => t_with("fw.release.needs_app", &[("tag", tag), ("min", min)]),
        ReleaseStatus::Unavailable { cached: Some(tag), error } => {
            t_with("fw.release.unavailable", &[("error", &release_error(error)), ("tag", tag)])
        }
        ReleaseStatus::Unavailable { cached: None, error } => t_with("fw.release.unavailable.none", &[("error", &release_error(error))]),
    }
}

/// "No firmware to install": the GitHub reason when releases are used, else the English detail.
fn no_firmware(detail: &str, release: Option<&ReleaseStatus>) -> String {
    let why = release.map_or_else(|| detail.trim_start_matches(':').trim().to_owned(), release_line);
    t_with("inst.err.no_fw", &[("why", &why)])
}

/// The Bose firmware version the installer does not know (`D01.01.03` from "unknown DSP firmware
/// (D01.01.03U01.01.04)"), when that is why it stopped.
pub fn unsupported_stock(error: &str) -> Option<String> {
    let start = error.find("unknown DSP firmware (")? + "unknown DSP firmware (".len();
    let inner = &error[start..];
    let inner = &inner[..inner.find(')')?];
    Some(inner.split('U').next().unwrap_or(inner).trim().to_owned())
}

/// The DSP did not answer (as opposed to answering something unknown).
fn silent(e: &str) -> bool {
    let e = e.to_ascii_lowercase();
    ["not answer", "no answer", "no reply", "silent", "timeout", "timed out"].iter().any(|w| e.contains(w))
}

/// An install error the user can act on; `None`: a lost connection (the generic line).
pub fn run_error(error: &str, release: Option<&ReleaseStatus>) -> Option<String> {
    if let Some(v) = unsupported_stock(error) {
        return Some(t_with("inst.err.unsupported", &[("version", &v)]));
    }
    if let Some(rest) = error.strip_prefix(NO_FIRMWARE) {
        return Some(no_firmware(rest, release));
    }
    if let Some(rest) = error.strip_prefix("backup: ") {
        return Some(backup_failure(rest, release));
    }
    None
}

/// Why the backup failed, in a few words (the English detail stays in the log).
pub fn backup_failure(e: &str, release: Option<&ReleaseStatus>) -> String {
    if let Some(v) = unsupported_stock(e) {
        t_with("inst.err.unsupported", &[("version", &v)])
    } else if e.starts_with(NO_USB) {
        tr("fw.backup.err.usb").to_owned()
    } else if e.starts_with(NOT_WRITABLE) {
        tr("fw.backup.err.dir").to_owned()
    } else if let Some(rest) = e.strip_prefix(NO_FIRMWARE) {
        no_firmware(rest, release)
    } else if silent(e) {
        tr("fw.backup.err.dsp").to_owned()
    } else {
        tr("fw.backup.failed").to_owned()
    }
}

/// Why the restore cannot start; `None`: it can.
pub fn restore_blocker(backup: &BackupState, stock: bool, restore_pending: bool, release: Option<&ReleaseStatus>) -> Option<String> {
    if stock && !restore_pending {
        return Some(tr("inst.restore.stock").to_owned());
    }
    match backup {
        BackupState::Saved { .. } => None,
        BackupState::Running => Some(tr("fw.backup.running").to_owned()),
        BackupState::Missing => Some(tr("fw.backup.missing").to_owned()),
        BackupState::Failed(e) => Some(backup_failure(e, release)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn errors_are_explicit() {
        let st = ReleaseStatus::Unavailable { cached: None, error: ReleaseError::NoRelease { repo: "o/r".into() } };
        let line = run_error(&format!("{NO_FIRMWARE}: whatever"), Some(&st)).unwrap();
        assert!(line.contains("o/r"), "{line}");
        assert!(!run_error(&format!("backup: {NO_USB}access denied"), None).unwrap().contains("access denied"), "no raw text");
        assert!(run_error("the DSP stopped answering", None).is_none());
        assert!(restore_blocker(&BackupState::Missing, true, false, None).is_some());
        assert!(restore_blocker(&BackupState::Saved { date: "d".into(), dir: Default::default(), created: false }, true, true, None).is_none());
        let failed = restore_blocker(&BackupState::Failed(format!("{NOT_WRITABLE}/x: denied")), false, false, None).unwrap();
        assert!(!failed.contains("denied"), "{failed}");
        assert!(!release_error(&ReleaseError::Http { status: 502 }).is_empty());
    }

    #[test]
    fn an_unknown_bose_version_is_named_not_called_silent() {
        let field = "backup: refused: no backup of the Bose firmware exists and the speaker no longer runs it (DSP: unknown DSP \
                     firmware (D01.01.03U01.01.04)): it cannot be read any more";
        assert_eq!(unsupported_stock(field).as_deref(), Some("D01.01.03"));
        let line = run_error(field, None).unwrap();
        assert!(line.contains("D01.01.03") && !line.contains("refused"), "{line}");
        assert_eq!(backup_failure("the DSP did not answer (timeout)", None), tr("fw.backup.err.dsp"));
        assert_eq!(backup_failure("something else in English", None), tr("fw.backup.failed"));
        assert_eq!(unsupported_stock("the DSP did not answer"), None);
    }
}
