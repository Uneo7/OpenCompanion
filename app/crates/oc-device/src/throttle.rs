//! A keyed throttle for everything the app sends while the user drags or clicks: one pending value per key
//! (the latest wins), sent at a bounded rate, the last one always sent.
//!
//! - [`Pace::Live`] (audible settings: EQ, levels, OS volume): at most one send per interval (20 Hz), the first
//!   one right away, the last value always sent once the user stops (trailing send).
//! - [`Pace::Settle`] (settings that are not heard while dragging: device options, mode): sent once, a delay
//!   after the last change.
//!
//! The caller sends what [`Throttle::take_due`] returns, one at a time (one write in flight), and reports the
//! outcome: a failure is retried with the latest value after [`RETRY`], silently, and reported only when it
//! still fails [`REPORT_AFTER_IDLE`] after the last change. EEPROM saves are debounced by their own callers
//! (≥ 1 s after the last change).

use std::time::{Duration, Instant};

/// 20 Hz for settings heard while dragging.
pub const LIVE: Duration = Duration::from_millis(50);
/// Settings not heard while dragging: once, this long after the last change.
pub const SETTLE: Duration = Duration::from_millis(250);
/// Pause before a failed send is retried.
pub const RETRY: Duration = Duration::from_millis(120);
/// A send still failing this long after the last change is reported (and dropped).
pub const REPORT_AFTER_IDLE: Duration = Duration::from_secs(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pace {
    /// At most one send per interval; the first at once; the last always.
    Live(Duration),
    /// One send, this long after the last change.
    Settle(Duration),
}

#[derive(Debug)]
struct Slot<K, V> {
    key: K,
    pace: Pace,
    value: Option<V>,
    due: Option<Instant>,
    last_sent: Option<Instant>,
    last_request: Option<Instant>,
}

#[derive(Debug)]
pub struct Throttle<K, V> {
    slots: Vec<Slot<K, V>>,
}

impl<K, V> Default for Throttle<K, V> {
    fn default() -> Self {
        Self { slots: Vec::new() }
    }
}

impl<K: PartialEq + Clone, V> Throttle<K, V> {
    fn slot(&mut self, key: &K, pace: Pace) -> &mut Slot<K, V> {
        let i = match self.slots.iter().position(|s| s.key == *key) {
            Some(i) => i,
            None => {
                self.slots.push(Slot { key: key.clone(), pace, value: None, due: None, last_sent: None, last_request: None });
                self.slots.len() - 1
            }
        };
        &mut self.slots[i]
    }

    /// The user wants `value` for `key` (replaces any value not sent yet).
    pub fn request(&mut self, key: K, pace: Pace, value: V, now: Instant) {
        let s = self.slot(&key, pace);
        s.pace = pace;
        s.value = Some(value);
        s.last_request = Some(now);
        s.due = Some(match pace {
            Pace::Live(min) => s.due.unwrap_or_else(|| s.last_sent.map_or(now, |t| (t + min).max(now))),
            Pace::Settle(delay) => now + delay,
        });
    }

    pub fn pending(&self) -> bool {
        self.slots.iter().any(|s| s.value.is_some())
    }

    /// When the next value is due.
    pub fn next_due(&self) -> Option<Instant> {
        self.slots.iter().filter(|s| s.value.is_some()).filter_map(|s| s.due).min()
    }

    /// The values due now (all pending ones with `force`, e.g. before another command), in request order of
    /// the keys. Each must be sent, then reported with [`Throttle::sent`] or [`Throttle::failed`].
    pub fn take_due(&mut self, now: Instant, force: bool) -> Vec<(K, V)> {
        self.slots
            .iter_mut()
            .filter(|s| s.value.is_some() && (force || s.due.is_none_or(|d| now >= d)))
            .map(|s| {
                s.due = None;
                (s.key.clone(), s.value.take().expect("filtered"))
            })
            .collect()
    }

    pub fn sent(&mut self, key: &K, now: Instant) {
        if let Some(s) = self.slots.iter_mut().find(|s| s.key == *key) {
            s.last_sent = Some(now);
        }
    }

    /// `value` for `key` failed: retried after [`RETRY`] (unless a newer value is waiting), or, when the user
    /// stopped [`REPORT_AFTER_IDLE`] ago, dropped. Returns true when the failure must be reported.
    pub fn failed(&mut self, key: &K, value: V, now: Instant) -> bool {
        let Some(s) = self.slots.iter_mut().find(|s| s.key == *key) else { return true };
        if s.last_request.is_none_or(|t| now.saturating_duration_since(t) >= REPORT_AFTER_IDLE) {
            return true;
        }
        if s.value.is_none() {
            s.value = Some(value);
        }
        s.due = Some(now + RETRY);
        false
    }

    /// Forgets every pending value (e.g. the speaker is not ready for them).
    pub fn clear(&mut self) {
        for s in &mut self.slots {
            s.value = None;
            s.due = None;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::cell::RefCell;

    use super::*;

    /// A fake link: counts the writes per key, refuses the first `nak` ones.
    #[derive(Default)]
    struct FakeLink {
        writes: RefCell<Vec<(&'static str, i32)>>,
        nak: std::cell::Cell<usize>,
    }

    impl FakeLink {
        fn send(&self, k: &'static str, v: i32) -> Result<(), String> {
            if self.nak.get() > 0 {
                self.nak.set(self.nak.get() - 1);
                return Err("busy".into());
            }
            self.writes.borrow_mut().push((k, v));
            Ok(())
        }
        fn count(&self, k: &str) -> usize {
            self.writes.borrow().iter().filter(|w| w.0 == k).count()
        }
    }

    fn pump(t: &mut Throttle<&'static str, i32>, link: &FakeLink, now: Instant, force: bool) -> usize {
        let mut reports = 0;
        for (k, v) in t.take_due(now, force) {
            match link.send(k, v) {
                Ok(()) => t.sent(&k, now),
                Err(_) => reports += usize::from(t.failed(&k, v, now)),
            }
        }
        reports
    }

    #[test]
    fn a_fast_drag_is_paced_and_ends_on_its_last_value() {
        let link = FakeLink::default();
        let mut t = Throttle::default();
        let t0 = Instant::now();
        // Two sliders dragged at 120 Hz for 500 ms, plus an option toggled 10 times.
        for k in 0..60u32 {
            let now = t0 + Duration::from_micros(8_333 * u64::from(k));
            t.request("eq", Pace::Live(LIVE), k as i32, now);
            t.request("sub", Pace::Live(LIVE), -(k as i32), now);
            if k % 6 == 0 {
                t.request("options", Pace::Settle(SETTLE), k as i32, now);
            }
            pump(&mut t, &link, now, false);
        }
        let end = t0 + Duration::from_millis(1000);
        pump(&mut t, &link, end, false);
        for k in ["eq", "sub"] {
            let n = link.count(k);
            assert!((9..=12).contains(&n), "{k}: {n} writes for a 500 ms drag at 20 Hz");
        }
        assert_eq!(link.count("options"), 1, "a settle setting is sent once");
        let w = link.writes.borrow();
        assert_eq!(w.iter().rev().find(|x| x.0 == "eq").unwrap().1, 59, "the last value always goes");
        assert_eq!(w.iter().rev().find(|x| x.0 == "options").unwrap().1, 54);
        drop(w);
        assert!(!t.pending());
    }

    #[test]
    fn failures_are_retried_silently_then_reported_once() {
        let link = FakeLink { nak: 2.into(), ..Default::default() };
        let mut t = Throttle::default();
        let t0 = Instant::now();
        t.request("eq", Pace::Live(LIVE), 1, t0);
        assert_eq!(pump(&mut t, &link, t0, false), 0);
        t.request("eq", Pace::Live(LIVE), 2, t0 + Duration::from_millis(30));
        assert_eq!(pump(&mut t, &link, t0 + RETRY, false), 0);
        assert_eq!(pump(&mut t, &link, t0 + RETRY * 2, false), 0);
        assert_eq!(*link.writes.borrow(), vec![("eq", 2)], "retried with the latest value");
        // Failing for good: one report, after the user stopped.
        link.nak.set(usize::MAX);
        t.request("eq", Pace::Live(LIVE), 3, t0 + Duration::from_secs(1));
        let mut reports = 0;
        let mut now = t0 + Duration::from_secs(1);
        while now < t0 + Duration::from_secs(3) {
            reports += pump(&mut t, &link, now, false);
            now += Duration::from_millis(20);
        }
        assert_eq!(reports, 1);
        assert!(!t.pending());
    }

    #[test]
    fn force_sends_everything_pending_in_order() {
        let link = FakeLink::default();
        let mut t = Throttle::default();
        let t0 = Instant::now();
        t.request("eq", Pace::Live(LIVE), 1, t0);
        pump(&mut t, &link, t0, false);
        t.request("eq", Pace::Live(LIVE), 2, t0);
        t.request("options", Pace::Settle(SETTLE), 7, t0);
        pump(&mut t, &link, t0 + Duration::from_millis(1), true);
        assert_eq!(*link.writes.borrow(), vec![("eq", 1), ("eq", 2), ("options", 7)]);
    }
}
