//! Over 1 MB the file moves to `install.old.log`.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::PathBuf;
use std::sync::Mutex;

const MAX_BYTES: u64 = 1 << 20;

/// The log file path: `OC_LOG_DIR` when set, a temporary folder in this crate's tests (they must never write
/// into the user's log), else the OS data dir.
pub fn path() -> PathBuf {
    if cfg!(test) && std::env::var_os("OC_LOG_DIR").is_none() {
        return std::env::temp_dir().join(format!("oc-device-test-logs-{}", std::process::id())).join("install.log");
    }
    crate::paths::log_dir().join("install.log")
}

/// Appends `lines` (best effort: a log that cannot be written never stops an install).
pub fn append<S: AsRef<str>>(lines: &[S]) {
    static LOCK: Mutex<()> = Mutex::new(());
    if lines.is_empty() {
        return;
    }
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let p = path();
    if let Some(dir) = p.parent() {
        let _ = fs::create_dir_all(dir);
    }
    if fs::metadata(&p).is_ok_and(|m| m.len() > MAX_BYTES) {
        let _ = fs::rename(&p, p.with_file_name("install.old.log"));
    }
    let Ok(mut f) = OpenOptions::new().create(true).append(true).open(&p) else { return };
    let stamp = oc_install::backup::stamp_now();
    for l in lines {
        let _ = writeln!(f, "{stamp} {}", l.as_ref());
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn tests_never_write_the_users_log() {
        let p = super::path();
        assert!(p.starts_with(std::env::temp_dir()) || std::env::var_os("OC_LOG_DIR").is_some(), "{}", p.display());
    }
}
