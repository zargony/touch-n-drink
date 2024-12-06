use core::fmt;
use embassy_time::Timer;
use embassy_time::{Duration, Instant};
use log::info;

/// Simple time interval of 24h
#[cfg(not(debug_assertions))]
const DAILY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
#[cfg(debug_assertions)]
const DAILY_INTERVAL: Duration = Duration::from_secs(30 * 60);

/// Duration display helper
struct DisplayDuration(Duration);

impl fmt::Display for DisplayDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hours = self.0.as_secs() / 3600;
        let min = self.0.as_secs() % 3600 / 60;
        let secs = self.0.as_secs() % 60;
        write!(f, "{hours}h{min}m{secs}s")
    }
}

/// Scheduler for daily events
#[derive(Debug)]
pub struct Daily {
    next: Instant,
}

impl Daily {
    /// Create new daily scheduler
    pub fn new() -> Self {
        let mut daily = Self {
            next: Instant::now(),
        };
        daily.schedule_next();
        daily
    }

    /// Returns true when schedule time is expired
    pub fn is_expired(&self) -> bool {
        self.next <= Instant::now()
    }

    /// Time left until schedule time
    pub fn time_left(&self) -> Duration {
        self.next.saturating_duration_since(Instant::now())
    }

    /// Timer that can be awaited on to wait for schedule time
    pub fn timer(&self) -> Timer {
        Timer::at(self.next)
    }

    /// After expiring, schedule next event
    pub fn schedule_next(&mut self) {
        if self.is_expired() {
            // Simple schedule: run again 24h later
            self.next += DAILY_INTERVAL;
        }
        if self.is_expired() {
            // Simple schedule: run in 24h from now
            self.next = Instant::now() + DAILY_INTERVAL;
        }
        info!(
            "Schedule: next daily event scheduled in {} from now",
            DisplayDuration(self.time_left())
        );
    }
}
