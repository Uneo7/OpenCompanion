use std::time::{Duration, Instant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rates {
    pub probe: Duration,
    pub usb: Duration,
    pub profile: Duration,
    pub meters: Duration,
    pub slow: Duration,
    pub recheck: Duration,
}

impl Default for Rates {
    fn default() -> Self {
        Self {
            probe: Duration::from_millis(1000),
            usb: Duration::from_millis(250),
            profile: Duration::from_millis(500),
            // ~15 Hz. Each relayed console command costs ~25 ms (20 ms TAS reply delay), so 2 per tick.
            meters: Duration::from_millis(66),
            slow: Duration::from_millis(400),
            // Liveness of a stock USB controller and "what does the DSP run" while it is not ours: a speaker that
            // comes back as OpenCompanion (same 05A7:1020) is found within a few seconds.
            recheck: Duration::from_millis(1500),
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Every {
    period: Duration,
    next: Instant,
}

impl Every {
    pub fn now(period: Duration) -> Self {
        Self { period, next: Instant::now() }
    }

    pub fn next(&self) -> Instant {
        self.next
    }

    /// Re-armed one period later, never in the past: no burst after a long command.
    pub fn due(&mut self, now: Instant) -> bool {
        if now < self.next {
            return false;
        }
        let next = self.next + self.period;
        self.next = if next > now { next } else { now + self.period };
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn timers_rearm_without_bursts() {
        let mut t = Every::now(Duration::from_millis(100));
        let t0 = Instant::now();
        assert!(t.due(t0));
        assert!(!t.due(t0 + Duration::from_millis(50)));
        let late = t0 + Duration::from_millis(1000);
        assert!(t.due(late));
        assert!(!t.due(late), "a late timer fires once, then waits");
    }
}
