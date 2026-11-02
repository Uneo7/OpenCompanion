//! `systemd-inhibit` holds the logind lock until it exits; killing it on drop releases it.

use std::process::{Child, Command, Stdio};
use std::time::Duration;

pub struct Guard {
    child: Child,
}

impl Guard {
    pub fn acquire(reason: &str) -> Result<Self, String> {
        let mut child = Command::new("systemd-inhibit")
            .args(["--what=sleep:idle", "--who=OpenCompanion", "--mode=block"])
            .arg(format!("--why={reason}"))
            .args(["sleep", "infinity"])
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map_err(|e| format!("systemd-inhibit not available ({e}): no logind inhibitor"))?;
        // A refused lock (no logind, polkit denial) makes systemd-inhibit exit at once.
        std::thread::sleep(Duration::from_millis(200));
        match child.try_wait() {
            Ok(None) => Ok(Self { child }),
            Ok(Some(st)) => Err(format!("systemd-inhibit exited ({st}): logind refused the inhibitor")),
            Err(e) => Err(format!("systemd-inhibit: {e}")),
        }
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}
