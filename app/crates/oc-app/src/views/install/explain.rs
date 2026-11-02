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

/// An install error the user can act on; `None`: a lost connection (the generic line).
pub fn run_error(error: &str, release: Option<&ReleaseStatus>) -> Option<String> {
    if let Some(rest) = error.strip_prefix(NO_FIRMWARE) {
        return Some(no_firmware(rest, release));
    }
    if let Some(rest) = error.strip_prefix("backup: ") {
        return Some(backup_failure(rest, release));
    }
    None
}

pub fn backup_failure(e: &str, release: Option<&ReleaseStatus>) -> String {
    if let Some(rest) = e.strip_prefix(NO_USB) {
        t_with("fw.backup.err.usb", &[("error", rest)])
    } else if let Some(rest) = e.strip_prefix(NOT_WRITABLE) {
        t_with("fw.backup.err.dir", &[("error", rest)])
    } else if let Some(rest) = e.strip_prefix(NO_FIRMWARE) {
        no_firmware(rest, release)
    } else if e.contains("DSP") {
        t_with("fw.backup.err.dsp", &[("error", e)])
    } else {
        t_with("fw.backup.failed", &[("error", e)])
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
        assert!(run_error(&format!("backup: {NO_USB}access denied"), None).unwrap().contains("access denied"));
        assert!(run_error("the DSP stopped answering", None).is_none());
        assert!(restore_blocker(&BackupState::Missing, true, false, None).is_some());
        assert!(restore_blocker(&BackupState::Saved { date: "d".into(), dir: Default::default(), created: false }, true, true, None).is_none());
        let failed = restore_blocker(&BackupState::Failed(format!("{NOT_WRITABLE}/x: denied")), false, false, None).unwrap();
        assert!(failed.contains("/x: denied"), "{failed}");
        assert!(!release_error(&ReleaseError::Http { status: 502 }).is_empty());
    }
}
