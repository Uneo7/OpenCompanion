//! Exclusive use of the speaker by someone else (the installer): the worker closes its HID handle and stops
//! polling while a [`LinkGuard`] is held, so that no request of the poller interleaves with an install.

use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

#[derive(Debug, Default)]
struct Flags {
    wanted: AtomicBool,
    released: AtomicBool,
}

#[derive(Debug, Clone, Default)]
pub struct LinkLease(Arc<Flags>);

#[derive(Debug)]
pub struct LinkGuard(LinkLease);

impl Drop for LinkGuard {
    fn drop(&mut self) {
        self.0 .0.wanted.store(false, Ordering::SeqCst);
    }
}

impl LinkLease {
    /// Asks the worker to release the speaker and waits for it (up to `timeout`). A worker that is not running
    /// never answers: `None` then, and the caller must not open the device.
    pub fn acquire(&self, timeout: Duration) -> Option<LinkGuard> {
        self.0.wanted.store(true, Ordering::SeqCst);
        let deadline = Instant::now() + timeout;
        loop {
            if self.0.released.load(Ordering::SeqCst) {
                return Some(LinkGuard(self.clone()));
            }
            if Instant::now() >= deadline {
                break;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        self.0.wanted.store(false, Ordering::SeqCst);
        None
    }

    /// A lease nobody polls (no worker): acquiring it succeeds at once.
    pub fn standalone() -> Self {
        let l = Self::default();
        l.0.released.store(true, Ordering::SeqCst);
        l
    }

    pub(crate) fn wanted(&self) -> bool {
        self.0.wanted.load(Ordering::SeqCst)
    }

    pub(crate) fn set_released(&self, released: bool) {
        self.0.released.store(released, Ordering::SeqCst);
    }

    pub fn is_held(&self) -> bool {
        self.wanted()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn acquire_waits_for_the_release() {
        let lease = LinkLease::default();
        assert!(lease.acquire(Duration::from_millis(30)).is_none(), "nobody released");
        assert!(!lease.wanted());
        let worker = lease.clone();
        let t = std::thread::spawn(move || {
            while !worker.wanted() {
                std::thread::sleep(Duration::from_millis(5));
            }
            worker.set_released(true);
        });
        let g = lease.acquire(Duration::from_secs(2)).expect("released");
        assert!(lease.is_held());
        drop(g);
        assert!(!lease.is_held());
        t.join().unwrap();
        assert!(LinkLease::standalone().acquire(Duration::ZERO).is_some());
    }
}
