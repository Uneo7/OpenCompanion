//! The OS volume and mute of the speaker's own audio endpoint (`oc-osvolume`: Core Audio on Windows and macOS,
//! the ALSA mixer on Linux), never the default output device.
//! While the speaker is connected, the endpoint is looked for on a background thread (every [`RETRY_S`] until
//! found). Once found, a change subscription keeps the last state: the pod knob, the OS slider and the app all
//! move the same value, and the UI follows it. Not found (another OS name, no mixer control, a platform without
//! backend): [`OsLink::state`] is `None` and the app shows what the speaker reports; setting fails with a notice.

use std::sync::{Arc, Mutex, MutexGuard};

use oc_osvolume::{find_device, DeviceMatch, OsVolume, Subscription, VolumeState};

pub const RETRY_S: f64 = 3.0;

type Repaint = Arc<dyn Fn() + Send + Sync>;

struct Attached {
    /// Dropped before `dev` (fields drop in declaration order): the callback is unregistered first.
    _sub: Option<Subscription>,
    dev: OsVolume,
}

#[derive(Default)]
struct Shared {
    attached: Mutex<Option<Attached>>,
    state: Mutex<Option<VolumeState>>,
    searching: Mutex<bool>,
}

fn lock<T>(m: &Mutex<T>) -> MutexGuard<'_, T> {
    m.lock().unwrap_or_else(|e| e.into_inner())
}

pub struct OsLink {
    shared: Arc<Shared>,
    repaint: Repaint,
    next_try: f64,
}

impl OsLink {
    pub fn new(repaint: Repaint) -> Self {
        Self { shared: Arc::default(), repaint, next_try: 0.0 }
    }

    /// Looks for the endpoint while `connected` and not attached yet; lets it go when the speaker leaves (its
    /// endpoint goes with it).
    pub fn tick(&mut self, now: f64, connected: bool) {
        if !connected {
            self.detach();
            self.next_try = 0.0;
            return;
        }
        if lock(&self.shared.attached).is_some() || now < self.next_try {
            return;
        }
        {
            let mut searching = lock(&self.shared.searching);
            if *searching {
                return;
            }
            *searching = true;
        }
        self.next_try = now + RETRY_S;
        let (shared, repaint) = (self.shared.clone(), self.repaint.clone());
        let spawned = std::thread::Builder::new().name("oc-osvolume".into()).spawn(move || {
            attach(&shared, &repaint);
            *lock(&shared.searching) = false;
        });
        if spawned.is_err() {
            *lock(&self.shared.searching) = false;
        }
    }

    /// Lets the endpoint go and looks for it again (the speaker re-enumerates after a mode change).
    pub fn reset(&mut self) {
        self.detach();
        self.next_try = 0.0;
    }

    fn detach(&mut self) {
        let old = lock(&self.shared.attached).take();
        drop(old);
        *lock(&self.shared.state) = None;
    }

    pub fn state(&self) -> Option<VolumeState> {
        *lock(&self.shared.state)
    }

    fn with_device(&mut self, f: impl FnOnce(&OsVolume) -> oc_osvolume::Result<()>) -> Result<(), String> {
        let res = match lock(&self.shared.attached).as_ref() {
            Some(a) => f(&a.dev).map_err(|e| e.to_string()),
            None => return Err(String::new()),
        };
        if res.is_err() {
            // A stale endpoint (unplugged, re-enumerated): look for it again.
            self.detach();
        }
        res
    }

    /// Sets the OS volume (0–100, the OS slider position). `Err("")`: no endpoint.
    pub fn set_volume(&mut self, percent: u8) -> Result<(), String> {
        self.with_device(|d| d.set_scalar(f32::from(percent.min(100)) / 100.0))
    }

    pub fn set_muted(&mut self, muted: bool) -> Result<(), String> {
        self.with_device(|d| d.set_mute(muted))
    }
}

fn attach(shared: &Arc<Shared>, repaint: &Repaint) {
    let Ok(Some(dev)) = find_device(&DeviceMatch::companion()) else { return };
    let Ok(first) = dev.get() else { return };
    *lock(&shared.state) = Some(first);
    let (weak, rp) = (Arc::downgrade(shared), repaint.clone());
    // Without notifications the value is still right after the app's own changes; the OS slider and the knob
    // then show up through the speaker's report only.
    let sub = dev
        .subscribe(move |s| {
            if let Some(sh) = weak.upgrade() {
                *lock(&sh.state) = Some(s);
                rp();
            }
        })
        .ok();
    *lock(&shared.attached) = Some(Attached { _sub: sub, dev });
    repaint();
}

pub fn percent(s: &VolumeState) -> u8 {
    (s.scalar.clamp(0.0, 1.0) * 100.0).round() as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn without_endpoint_nothing_is_set_and_the_speaker_value_is_kept() {
        let mut l = OsLink::new(Arc::new(|| {}));
        l.tick(0.0, false);
        assert!(l.state().is_none());
        assert_eq!(l.set_volume(30), Err(String::new()));
        assert_eq!(l.set_muted(true), Err(String::new()));
        assert_eq!(percent(&VolumeState { scalar: 0.424, db: -20.0, muted: false }), 42);
        assert_eq!(percent(&VolumeState { scalar: 1.5, db: 0.0, muted: false }), 100);
    }
}
