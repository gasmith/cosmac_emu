use std::time::{Duration, Instant};

/// A time tracker for synchronizing an emulated clock with the wallclock.
#[derive(Debug)]
pub struct TimeTracker {
    start: Instant,
    now: Duration,
}
impl Default for TimeTracker {
    fn default() -> Self {
        Self {
            start: Instant::now(),
            now: Duration::default(),
        }
    }
}
impl TimeTracker {
    /// Sleeps until the specified offset.
    pub fn sleep_until(&mut self, when: Duration) {
        let delta = when.saturating_sub(self.start.elapsed());
        if delta >= Duration::from_micros(1) {
            std::thread::sleep(delta);
        }
        self.now = when;
    }
}
