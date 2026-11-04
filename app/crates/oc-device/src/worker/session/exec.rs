use std::time::{Duration, Instant};

use super::poll::ReadProfile;
use super::Session;
use crate::console::{Dsp, OpInfo, SLOTS};
use crate::error::Error;
use crate::params::Profile;
use crate::throttle::{Pace, LIVE, SETTLE};
use crate::worker::messages::{Command, Event, LOCKED_SLOT};

/// The locked slot ("Bose d'origine") is never saved, renamed or recoloured: only the initial fill of an empty
/// slot ([`Command::Provision`]) may create it.
fn writes_locked_slot(c: &Command) -> bool {
    matches!(c, Command::Save { slot, .. } | Command::Patch { slot, .. } | Command::Duplicate { to: slot, .. } if *slot == LOCKED_SLOT)
}

/// Longest wait for an asynchronous EEPROM job (load ~ms, save ~50 ms).
const JOB_TIMEOUT: Duration = Duration::from_millis(2000);

/// What the throttle keeps one value of ([`crate::throttle`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    Profile,
    SubLevel,
    BassSource,
    Route21,
    Options,
    Mode,
}

/// The throttled commands: audible ones at 20 Hz (last value always sent), the others once, 250 ms after the last
/// change. Everything else runs in order, after the pending throttled values.
pub fn pace_of(c: &Command) -> Option<(Key, Pace)> {
    Some(match c {
        Command::Write(_) => (Key::Profile, Pace::Live(LIVE)),
        Command::SetSubLevel(_) => (Key::SubLevel, Pace::Live(LIVE)),
        Command::SetBassSource(_) => (Key::BassSource, Pace::Settle(SETTLE)),
        Command::SetRoute21(_) => (Key::Route21, Pace::Settle(SETTLE)),
        Command::SetOptions(_) => (Key::Options, Pace::Settle(SETTLE)),
        Command::SetMode(_) => (Key::Mode, Pace::Settle(SETTLE)),
        _ => return None,
    })
}

pub fn coalesce(batch: Vec<Command>) -> Vec<Command> {
    let mut out: Vec<Command> = Vec::with_capacity(batch.len());
    for c in batch {
        if let (Some(Command::Write(_)), Command::Write(_)) = (out.last(), &c) {
            out.pop();
        }
        out.push(c);
    }
    out
}

fn wait_job(d: &Dsp) -> crate::Result<OpInfo> {
    let deadline = Instant::now() + JOB_TIMEOUT;
    loop {
        let info = d.op_info()?;
        if !info.busy {
            return Ok(info);
        }
        if Instant::now() >= deadline {
            return Err(Error::Reply { cmd: "OPI", reply: "EEPROM job still running after 2 s".into() });
        }
        std::thread::sleep(Duration::from_millis(25));
    }
}

fn job_ok(cmd: &'static str, info: OpInfo) -> crate::Result<OpInfo> {
    if info.last_failed {
        return Err(Error::Nak { cmd, code: info.last_result });
    }
    Ok(info)
}

impl Session {
    pub(super) fn execute(&mut self, c: Command) {
        if !self.full {
            if let Command::Console { id, .. } = c {
                self.emit(Event::Console { id, reply: "the DSP does not run the OpenCompanion firmware".into() });
            } else {
                self.emit(Event::Failed { what: "command".into(), error: "the DSP does not run the OpenCompanion firmware".into() });
            }
            return;
        }
        let what = label(&c);
        if writes_locked_slot(&c) {
            // The app never sends these: a refusal here is a bug to trace, never a message for the user.
            #[cfg(debug_assertions)]
            eprintln!("[oc-device] {what}: profile {} is locked, not written", LOCKED_SLOT + 1);
            return;
        }
        let r = self.run_command(c);
        if let Err(e) = r {
            let transport = e.is_transport();
            self.emit(Event::Failed { what: what.into(), error: e.to_string() });
            if transport {
                self.failures += 1;
            }
        } else {
            self.failures = 0;
        }
    }

    /// Runs `c` (the checks of [`Session::execute`] done) and returns its outcome.
    pub(super) fn run_command(&mut self, c: Command) -> crate::Result<()> {
        match c {
            Command::Write(p) => self.write(&p),
            Command::Load(slot) => self.load(slot),
            Command::Save { slot, profile } => self.save(slot, &profile),
            Command::Patch { slot, name, color, active, live } => self.patch(slot, name.as_deref(), color, active, &live),
            Command::Provision(slots) => self.provision(&slots),
            Command::Duplicate { from, to, name, color } => self.duplicate(from, to, &name, color),
            Command::LoadBose => self.load_bose(),
            Command::SetBassSource(src) => {
                let r = self.with_dsp(|d| d.op_bass_source(src).and_then(|_| wait_job(d)).and_then(|i| job_ok("OPS", i)));
                r.map(|_| self.refresh_profiles(ReadProfile::No))
            }
            Command::SetSubLevel(db10) => {
                self.with_dsp(|d| d.set_sub_level(db10).and_then(|_| d.bass_device())).map(|b| self.emit(Event::Bass(b)))
            }
            Command::SetRoute21(on) => self.with_dsp(|d| d.op_set_route21(on)).map(|r| self.emit(Event::Route21(r))),
            Command::SetOptions(o) => self.with_dsp(|d| d.op_set_options(o)).map(|r| self.emit(Event::Route21(r))),
            Command::SetMode(m) => match (self.conn.oc(), self.last_usb) {
                (Some(link), Some(info)) => crate::link::mode::set_mode(link, &info, m).map_err(Error::Unsupported),
                _ => Err(Error::Unsupported("the USB controller does not run the OpenCompanion firmware".into())),
            },
            Command::Console { id, line } => {
                let reply = self.with_dsp(|d| d.line(&line)).unwrap_or_else(|e| format!("error: {e}"));
                self.emit(Event::Console { id, reply });
                Ok(())
            }
            Command::Restart(crate::worker::messages::RestartTarget::Dsp) => {
                self.restart_dsp();
                Ok(())
            }
            Command::Restart(crate::worker::messages::RestartTarget::Usb) => self.restart_usb(),
            Command::Refresh => {
                self.refresh_profiles(ReadProfile::Active);
                Ok(())
            }
        }
    }

    /// The mirror is sanitized like the DSP does.
    pub(super) fn write(&mut self, p: &Profile) -> crate::Result<()> {
        let target = p.clone().sanitized();
        let from = match &self.mirror {
            Some(m) => m.clone(),
            None => self.with_dsp(|d| d.read_profile())?,
        };
        let r = self.with_dsp(|d| d.write_profile(&from, &target));
        match r {
            Ok(_) => {
                self.mirror = Some(target);
                Ok(())
            }
            Err(e) => {
                // Partly written: the next write starts from what the DSP really holds.
                self.mirror = None;
                Err(e)
            }
        }
    }

    /// An empty slot starts from the Bose base with a flat EQ (`OPK`, the firmware's default for a new slot).
    fn load(&mut self, slot: u8) -> crate::Result<()> {
        let r = self.with_dsp(|d| d.op_load(slot).and_then(|_| wait_job(d)).and_then(|i| job_ok("OPL", i)));
        match r {
            Ok(_) => {}
            Err(Error::Nak { code: 6, .. }) => self.with_dsp(|d| d.op_bose())?,
            Err(e) => return Err(e),
        }
        self.mirror = None;
        self.refresh_profiles(ReadProfile::Slot(slot));
        Ok(())
    }

    fn duplicate(&mut self, from: u8, to: u8, name: &str, color: u8) -> crate::Result<()> {
        let loaded = self.with_dsp(|d| d.op_load(from).and_then(|_| wait_job(d)).and_then(|i| job_ok("OPL", i)));
        match loaded {
            Ok(_) => {}
            Err(Error::Nak { code: 6, .. }) => self.with_dsp(|d| d.op_bose())?, // empty: the Bose base
            Err(e) => return Err(e),
        }
        let mut p = self.with_dsp(|d| d.read_profile())?;
        self.mirror = Some(p.clone());
        p.set_name(name);
        p.color = color;
        self.save(to, &p)?;
        // The copy becomes the active slot (loaded back: the DSP and the app agree on it).
        self.load(to)
    }

    fn provision(&mut self, slots: &[(u8, String, u8)]) -> crate::Result<()> {
        let info = self.with_dsp(|d| d.op_info())?;
        let mut written = Vec::new();
        for (slot, name, color) in slots {
            if usize::from(*slot) >= SLOTS || info.slot_valid(usize::from(*slot)) {
                continue;
            }
            self.with_dsp(|d| d.op_bose())?;
            let mut p = self.with_dsp(|d| d.read_profile())?;
            self.mirror = Some(p.clone());
            p.set_name(name);
            p.color = *color;
            self.save(*slot, &p)?;
            written.push(*slot);
        }
        if !written.is_empty() {
            // Back to the slot that played (the first slot when none did).
            let back = info.active.unwrap_or(0);
            self.with_dsp(|d| d.op_load(back).and_then(|_| wait_job(d)).map(drop))?;
            self.mirror = None;
            self.refresh_profiles(ReadProfile::Slot(back));
        }
        self.emit(Event::Provisioned(written));
        Ok(())
    }

    fn save(&mut self, slot: u8, p: &Profile) -> crate::Result<()> {
        self.write(p)?;
        self.with_dsp(|d| d.op_save(slot).and_then(|_| wait_job(d)).and_then(|i| job_ok("OPV", i)))?;
        self.refresh_profiles(ReadProfile::No);
        Ok(())
    }

    fn patch(&mut self, slot: u8, name: Option<&str>, color: Option<u8>, active: Option<u8>, live: &Profile) -> crate::Result<()> {
        let loaded = self.with_dsp(|d| d.op_load(slot).and_then(|_| wait_job(d)).and_then(|i| job_ok("OPL", i)));
        match loaded {
            Ok(_) => {}
            Err(Error::Nak { code: 6, .. }) => self.with_dsp(|d| d.op_bose())?, // empty slot: Bose base
            Err(e) => return Err(e),
        }
        let mut p = self.with_dsp(|d| d.read_profile())?;
        self.mirror = Some(p.clone());
        if let Some(n) = name {
            p.set_name(n);
        }
        if let Some(c) = color {
            p.color = c;
        }
        let saved = self.save(slot, &p);
        let back = match active {
            Some(a) => self.with_dsp(|d| d.op_load(a).and_then(|_| wait_job(d)).map(drop)),
            None => self.with_dsp(|d| d.op_defaults()),
        };
        self.mirror = None;
        back.and_then(|_| self.write(live))?;
        self.refresh_profiles(ReadProfile::No);
        saved
    }

    fn load_bose(&mut self) -> crate::Result<()> {
        self.with_dsp(|d| d.op_bose())?;
        self.mirror = None;
        self.refresh_profiles(ReadProfile::Unsaved);
        Ok(())
    }
}

pub(super) fn label(c: &Command) -> &'static str {
    match c {
        Command::Write(_) => "write profile",
        Command::Load(_) => "load profile",
        Command::Save { .. } => "save profile",
        Command::Patch { .. } => "rename profile",
        Command::Provision(_) => "create the default profiles",
        Command::Duplicate { .. } => "duplicate profile",
        Command::LoadBose => "load the Bose profile",
        Command::SetBassSource(_) => "bass source",
        Command::SetSubLevel(_) => "sub level",
        Command::SetRoute21(_) => "2.1 routing",
        Command::SetOptions(_) => "device options",
        Command::SetMode(_) => "audio mode",
        Command::Console { .. } => "console",
        Command::Restart(_) => "restart",
        Command::Refresh => "refresh",
    }
}

#[cfg(test)]
mod lock_tests {
    use super::*;

    #[test]
    fn the_locked_slot_is_never_written() {
        let p = Profile::default();
        assert!(writes_locked_slot(&Command::Save { slot: LOCKED_SLOT, profile: p.clone() }));
        assert!(writes_locked_slot(&Command::Patch { slot: LOCKED_SLOT, name: Some("x".into()), color: None, active: None, live: p.clone() }));
        assert!(!writes_locked_slot(&Command::Save { slot: 1, profile: p.clone() }));
        let dup = |to| Command::Duplicate { from: LOCKED_SLOT, to, name: "Bose (copie)".into(), color: 1 };
        assert!(writes_locked_slot(&dup(LOCKED_SLOT)) && !writes_locked_slot(&dup(2)));
        assert!(!writes_locked_slot(&Command::Load(LOCKED_SLOT)));
        assert!(!writes_locked_slot(&Command::Write(p)));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn consecutive_writes_collapse() {
        let a = Profile::defaults();
        let mut b = a.clone();
        b.flags = 4;
        let out = coalesce(vec![Command::Write(a.clone()), Command::Write(b.clone()), Command::Load(1), Command::Write(a.clone())]);
        assert_eq!(out, vec![Command::Write(b), Command::Load(1), Command::Write(a)]);
    }
}
