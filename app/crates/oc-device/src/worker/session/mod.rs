mod exec;
mod poll;
mod restart;

use std::sync::mpsc::{Receiver, RecvTimeoutError, Sender};
use std::time::{Duration, Instant};

use oc_install::dsp::{detect, DspRuntime};

use super::messages::{Command, DspState, Event};
use super::schedule::{Every, Rates};
use crate::console::{Dsp, OpInfo};
use crate::link::{Conn, Presence};
use crate::params::Profile;
use crate::Result;

const MAX_FAILURES: u32 = 3;

pub enum End {
    Lost,
    Shutdown,
    Leased,
}

pub struct Session {
    conn: Conn,
    tx: Sender<Event>,
    full: bool,
    failures: u32,
    /// Consecutive `OPI` failures of any kind: the DSP may have left our firmware (reset, rescue).
    dsp_errors: u32,
    mirror: Option<Profile>,
    last_info: Option<OpInfo>,
    last_usb: Option<crate::link::TasInfo>,
    timers: Timers,
    meter_ticks: u32,
    slow_turn: usize,
    meters: crate::console::Meters,
    throttle: crate::throttle::Throttle<exec::Key, Command>,
}

struct Timers {
    usb: Every,
    profile: Every,
    meters: Every,
    slow: Every,
    recheck: Every,
}

impl Timers {
    fn new(r: &Rates) -> Self {
        Self {
            usb: Every::now(r.usb),
            profile: Every::now(r.profile),
            meters: Every::now(r.meters),
            slow: Every::now(r.slow),
            recheck: Every::now(r.recheck),
        }
    }

    fn next(&self, full: bool) -> Instant {
        let mut t = self.usb.next().min(self.recheck.next());
        if full {
            t = t.min(self.profile.next()).min(self.meters.next()).min(self.slow.next());
        }
        t
    }
}

impl Session {
    pub fn start(conn: Conn, presence: Presence, tx: Sender<Event>, rates: Rates) -> Self {
        let _ = tx.send(Event::Presence(presence));
        let mut s = Self {
            conn,
            tx,
            timers: Timers::new(&rates),
            full: false,
            failures: 0,
            dsp_errors: 0,
            mirror: None,
            last_info: None,
            last_usb: None,
            meter_ticks: 0,
            slow_turn: 0,
            meters: Default::default(),
            throttle: crate::throttle::Throttle::default(),
        };
        if let Some(Ok(info)) = s.conn.tas_info() {
            s.emit(Event::TasVersion(info.base.version_string()));
        }
        s.check_dsp();
        s
    }

    fn emit(&self, e: Event) {
        let _ = self.tx.send(e);
    }

    fn with_dsp<R>(&self, f: impl FnOnce(&Dsp) -> R) -> R {
        self.conn.with_dsp(|link| f(&Dsp::new(link)))
    }

    fn track<T>(&mut self, r: Result<T>) -> Option<T> {
        match r {
            Ok(v) => {
                self.failures = 0;
                Some(v)
            }
            Err(e) => {
                if e.is_transport() {
                    self.failures += 1;
                }
                None
            }
        }
    }

    fn lost(&self) -> bool {
        self.failures >= MAX_FAILURES
    }

    fn check_dsp(&mut self) {
        let rt = self.conn.with_dsp(detect);
        let full = matches!(rt, DspRuntime::OcFirmware { .. });
        self.emit(Event::Dsp(DspState::Runtime(rt)));
        if full && !self.full {
            self.full = true;
            if let Ok(v) = self.with_dsp(|d| d.versions()) {
                self.emit(Event::Versions(v));
            }
            if let Ok(b) = self.with_dsp(|d| d.boot_status()) {
                self.emit(Event::Boot(b));
            }
            self.refresh_profiles(true);
        } else if !full {
            self.full = false;
            if let Ok(v) = self.with_dsp(|d| d.versions()) {
                self.emit(Event::Versions(v));
            }
        }
    }

    /// Sends the throttled values that are due (all of them with `force`), one at a time. A failure is
    /// retried silently; one that lasts after the user stopped is reported.
    fn flush_throttled(&mut self, force: bool) {
        if !self.throttle.pending() {
            return;
        }
        for (key, cmd) in self.throttle.take_due(Instant::now(), force) {
            let what = exec::label(&cmd);
            let keep = cmd.clone();
            // The same checks as any command (our DSP firmware running, …).
            if !self.full && !matches!(cmd, Command::SetMode(_)) {
                continue; // nothing to apply the edit to: dropped, as before
            }
            let r = self.run_command(cmd);
            let now = Instant::now();
            match r {
                Ok(()) => {
                    self.failures = 0;
                    self.throttle.sent(&key, now);
                }
                Err(e) => {
                    if e.is_transport() {
                        self.failures += 1;
                    }
                    if self.throttle.failed(&key, keep, now) {
                        self.emit(Event::Failed { what: what.into(), error: e.to_string() });
                    }
                }
            }
            if self.lost() {
                return;
            }
        }
    }

    pub fn run(&mut self, rx: &Receiver<Command>, leased: impl Fn() -> bool) -> End {
        loop {
            if leased() {
                return End::Leased;
            }
            if self.lost() {
                return End::Lost;
            }
            let mut next = self.timers.next(self.full);
            if let Some(d) = self.throttle.next_due() {
                next = next.min(d);
            }
            let wait = next.saturating_duration_since(Instant::now()).min(Duration::from_millis(100));
            match rx.recv_timeout(wait) {
                Ok(first) => {
                    let mut batch = vec![first];
                    batch.extend(rx.try_iter());
                    for c in exec::coalesce(batch) {
                        // Settings are throttled per key; anything else first sends what is pending (order kept).
                        match exec::pace_of(&c) {
                            Some((key, pace)) => self.throttle.request(key, pace, c, Instant::now()),
                            None => {
                                self.flush_throttled(true);
                                self.execute(c);
                            }
                        }
                        if self.lost() {
                            return End::Lost;
                        }
                    }
                }
                Err(RecvTimeoutError::Timeout) => {}
                Err(RecvTimeoutError::Disconnected) => return End::Shutdown,
            }
            self.flush_throttled(false);
            if self.lost() {
                return End::Lost;
            }
            self.poll(Instant::now());
        }
    }
}
