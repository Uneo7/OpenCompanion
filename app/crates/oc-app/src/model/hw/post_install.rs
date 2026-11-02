//! After every completed firmware install (any chip, any flow), once the speaker is back: the OS volume it had just before
//! the install is set again, then the OS mute is turned on (the user unmutes deliberately). Without a recorded
//! volume, only the mute. Both go through the OS mixer of the speaker's own endpoint (`oc-osvolume`), and every
//! step is written to the install log.

use super::os_volume::{self, OsLink};

/// How long the speaker may take to come back (re-enumeration, DSP reboot) before the restore is given up.
pub const GIVE_UP_S: f64 = 120.0;
/// Pause after the end of the flow before the restore (the endpoint settles after a re-enumeration).
pub const SETTLE_S: f64 = 3.0;

pub trait OsVolumeCtl {
    fn volume(&self) -> Option<u8>;
    fn set_volume(&mut self, percent: u8) -> Result<(), String>;
    fn set_muted(&mut self, muted: bool) -> Result<(), String>;
}

impl OsVolumeCtl for OsLink {
    fn volume(&self) -> Option<u8> {
        self.state().map(|s| os_volume::percent(&s))
    }

    fn set_volume(&mut self, percent: u8) -> Result<(), String> {
        OsLink::set_volume(self, percent)
    }

    fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        OsLink::set_muted(self, muted)
    }
}

#[derive(Debug, Default, Clone, PartialEq)]
pub enum PostInstall {
    #[default]
    Idle,
    /// A flow runs; the volume recorded before its first write (`None`: the endpoint was not found).
    Running { volume: Option<u8> },
    Pending { volume: Option<u8>, since: f64 },
}

impl PostInstall {
    pub fn started(&mut self, os: &dyn OsVolumeCtl) -> String {
        let volume = os.volume();
        *self = PostInstall::Running { volume };
        match volume {
            Some(v) => format!("OS volume before the install: {v} %"),
            None => "OS volume before the install: not readable (the mute only will be set afterwards)".into(),
        }
    }

    pub fn ended(&mut self, now: f64) {
        if let PostInstall::Running { volume } = *self {
            *self = PostInstall::Pending { volume, since: now };
        }
    }

    /// Once the speaker is ready and its endpoint found: volume, then mute. Returns the log lines.
    pub fn tick(&mut self, now: f64, speaker_ready: bool, os: &mut dyn OsVolumeCtl) -> Vec<String> {
        let PostInstall::Pending { volume, since } = *self else { return Vec::new() };
        let mut log = Vec::new();
        if now - since < SETTLE_S || !speaker_ready || os.volume().is_none() {
            if now - since > GIVE_UP_S {
                *self = PostInstall::Idle;
                log.push(format!("after the install: the speaker's OS volume was not found within {GIVE_UP_S:.0} s, nothing restored"));
            }
            return log;
        }
        *self = PostInstall::Idle;
        if let Some(v) = volume {
            match os.set_volume(v) {
                Ok(()) => log.push(format!("after the install: OS volume set back to {v} %")),
                Err(e) => log.push(format!("after the install: setting the OS volume to {v} % failed: {e}")),
            }
        }
        match os.set_muted(true) {
            Ok(()) => log.push("after the install: OS mute on (unmute to listen)".into()),
            Err(e) => log.push(format!("after the install: setting the OS mute failed: {e}")),
        }
        log
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Default)]
    struct FakeOs {
        found: bool,
        volume: u8,
        muted: bool,
        calls: Vec<String>,
    }

    impl OsVolumeCtl for FakeOs {
        fn volume(&self) -> Option<u8> {
            self.found.then_some(self.volume)
        }
        fn set_volume(&mut self, p: u8) -> Result<(), String> {
            self.volume = p;
            self.calls.push(format!("volume {p}"));
            Ok(())
        }
        fn set_muted(&mut self, m: bool) -> Result<(), String> {
            self.muted = m;
            self.calls.push(format!("mute {m}"));
            Ok(())
        }
    }

    #[test]
    fn volume_then_mute_once_the_speaker_is_back() {
        let mut os = FakeOs { found: true, volume: 42, ..Default::default() };
        let mut p = PostInstall::default();
        p.started(&os);
        // The install moves the slider (TAS reboot: the OS shows 100 %), then the speaker leaves.
        os.volume = 100;
        os.found = false;
        p.ended(10.0);
        assert!(p.tick(11.0, false, &mut os).is_empty());
        assert!(p.tick(14.0, true, &mut os).is_empty(), "endpoint not found yet");
        os.found = true;
        assert!(p.tick(12.0, true, &mut os).is_empty(), "still settling");
        let log = p.tick(15.0, true, &mut os);
        assert_eq!(os.calls, ["volume 42", "mute true"]);
        assert!(os.muted && log.len() == 2);
        assert_eq!(p, PostInstall::Idle);
        assert!(p.tick(16.0, true, &mut os).is_empty());
    }

    #[test]
    fn without_a_recorded_volume_only_the_mute() {
        let mut os = FakeOs::default();
        let mut p = PostInstall::default();
        assert!(p.started(&os).contains("not readable"));
        p.ended(0.0);
        os.found = true;
        p.tick(SETTLE_S, true, &mut os);
        assert_eq!(os.calls, ["mute true"]);
    }

    #[test]
    fn gives_up_when_the_speaker_never_comes_back() {
        let mut os = FakeOs::default();
        let mut p = PostInstall::Running { volume: Some(30) };
        p.ended(0.0);
        assert!(p.tick(GIVE_UP_S + 1.0, false, &mut os)[0].contains("nothing restored"));
        assert!(os.calls.is_empty() && p == PostInstall::Idle);
    }
}
