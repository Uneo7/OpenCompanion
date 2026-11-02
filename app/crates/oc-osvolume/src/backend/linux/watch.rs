use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::thread::JoinHandle;

use alsa::poll::{poll, pollfd};

use super::mixer::Control;
use crate::{Callback, Error, Result, VolumeState};

/// Poll timeout, bounding how long dropping a subscription can block.
const POLL_MS: i32 = 200;

pub struct Subscription {
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Subscription {
    pub(super) fn spawn(ctl: String, cb: Callback) -> Result<Self> {
        // A dedicated mixer handle, opened here so errors reach the caller.
        let control = Control::open(&ctl)?;
        let fds = control.poll_fds()?;
        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            std::thread::Builder::new()
                .name("oc-osvolume-alsa".into())
                .spawn(move || run(control, fds, &stop, cb))
                .map_err(|e| Error::os("thread spawn", e))?
        };
        Ok(Self {
            stop,
            thread: Some(thread),
        })
    }
}

fn run(control: Control, mut fds: Vec<pollfd>, stop: &AtomicBool, cb: Callback) {
    let mut last: Option<VolumeState> = control.read().ok();
    while !stop.load(Ordering::Relaxed) {
        match poll(&mut fds, POLL_MS) {
            Ok(0) => continue,
            Ok(_) => {}
            Err(_) => break,
        }
        // `read` drains the pending events (snd_mixer_handle_events) and
        // reports only real changes.
        if let Ok(state) = control.read() {
            if last != Some(state) {
                last = Some(state);
                cb(state);
            }
        }
    }
}

impl Drop for Subscription {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::Relaxed);
        if let Some(t) = self.thread.take() {
            let _ = t.join();
        }
    }
}
