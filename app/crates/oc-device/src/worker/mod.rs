//! The UI thread never blocks on USB: everything goes through the channels.

mod lease;
pub mod messages;
mod schedule;
mod session;

use std::sync::mpsc::{self, Receiver, RecvTimeoutError, Sender};
use std::thread::JoinHandle;
use std::time::Duration;

pub use lease::{LinkGuard, LinkLease};
pub use messages::{Command, DspState, Event, RestartTarget, LOCKED_SLOT};
pub use oc_install::dsp::DspRuntime;
pub use schedule::Rates;

use crate::link::{probe, Presence};
use session::{End, Session};

/// Dropping it stops the thread.
pub struct Worker {
    tx: Option<Sender<Command>>,
    rx: Receiver<Event>,
    lease: LinkLease,
    thread: Option<JoinHandle<()>>,
}

impl Worker {
    pub fn spawn(rates: Rates) -> Self {
        let (cmd_tx, cmd_rx) = mpsc::channel();
        let (ev_tx, ev_rx) = mpsc::channel();
        let lease = LinkLease::default();
        let l = lease.clone();
        let thread = std::thread::Builder::new()
            .name("oc-device".into())
            .spawn(move || run(cmd_rx, ev_tx, l, rates))
            .expect("spawning the device thread");
        Self { tx: Some(cmd_tx), rx: ev_rx, lease, thread: Some(thread) }
    }

    pub fn send(&self, c: Command) {
        if let Some(tx) = &self.tx {
            let _ = tx.send(c);
        }
    }

    /// Never blocks.
    pub fn events(&self) -> impl Iterator<Item = Event> + '_ {
        self.rx.try_iter()
    }

    pub fn lease(&self) -> LinkLease {
        self.lease.clone()
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        self.tx = None; // disconnects the command channel: the thread ends its loop
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}

fn run(rx: Receiver<Command>, tx: Sender<Event>, lease: LinkLease, rates: Rates) {
    let mut last: Option<Presence> = None;
    loop {
        // Handshake with `LinkLease::acquire` (both sides SeqCst): "not released" is published before `wanted` is
        // read, so an acquirer that sees "released" knows this loop will see `wanted` before opening anything.
        lease.set_released(false);
        if lease.wanted() {
            lease.set_released(true);
            match rx.recv_timeout(Duration::from_millis(50)) {
                Err(RecvTimeoutError::Disconnected) => return,
                Ok(c) => refuse(c, &tx, "the speaker is busy (installer running)"),
                Err(RecvTimeoutError::Timeout) => {}
            }
            continue;
        }
        let (presence, conn) = probe();
        match conn {
            Some(conn) => {
                last = Some(presence.clone());
                let mut s = Session::start(conn, presence, tx.clone(), rates);
                match s.run(&rx, || lease.wanted()) {
                    End::Shutdown => return,
                    End::Leased => {}
                    End::Lost => announce(&mut last, Presence::Absent, &tx),
                }
                // `s` (and its HID handle) is dropped here, before the lease is reported released.
            }
            None => {
                announce(&mut last, presence, &tx);
                match rx.recv_timeout(rates.probe) {
                    Err(RecvTimeoutError::Disconnected) => return,
                    Ok(c) => refuse(c, &tx, "no speaker connected"),
                    Err(RecvTimeoutError::Timeout) => {}
                }
            }
        }
    }
}

fn announce(last: &mut Option<Presence>, p: Presence, tx: &Sender<Event>) {
    if last.as_ref() != Some(&p) {
        *last = Some(p.clone());
        let _ = tx.send(Event::Presence(p));
    }
}

fn refuse(c: Command, tx: &Sender<Event>, why: &str) {
    let e = match c {
        Command::Console { id, .. } => Event::Console { id, reply: why.into() },
        Command::Write(_) => return, // live edits while unplugged: nothing to report
        _ => Event::Failed { what: "command".into(), error: why.into() },
    };
    let _ = tx.send(e);
}
