use chrono::{DateTime, TimeDelta, Utc};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_time::Instant;
use log::debug;

/// Calculated time of system start
static SYSTEM_START_TIME: CriticalSectionMutex<RefCell<Option<DateTime<Utc>>>> =
    CriticalSectionMutex::new(RefCell::new(None));

/// Time of system start
fn start_time() -> Option<DateTime<Utc>> {
    SYSTEM_START_TIME.lock(|sst| *sst.borrow())
}

/// Set time of system start
fn set_start_time(time: DateTime<Utc>) {
    SYSTEM_START_TIME.lock(|sst| {
        *sst.borrow_mut() = Some(time);
    });
}

/// Date and time extension for relative time types
pub trait DateTimeExt {
    /// Relative time in milliseconds
    fn as_milliseconds(&self) -> u64;

    /// Convert the relative time type to a chrono duration type
    fn to_duration(&self) -> Option<TimeDelta> {
        TimeDelta::try_milliseconds(i64::try_from(self.as_milliseconds()).ok()?)
    }

    /// Convert relative time since system start to current time
    fn to_datetime(&self) -> Option<DateTime<Utc>> {
        let start_time = start_time()?;
        let duration = self.to_duration()?;
        Some(start_time + duration)
    }
}

impl DateTimeExt for Instant {
    fn as_milliseconds(&self) -> u64 {
        self.as_millis()
    }
}

/// Duration of system run time
pub fn uptime() -> Option<TimeDelta> {
    Instant::now().to_duration()
}

/// Current time
pub fn now() -> Option<DateTime<Utc>> {
    Instant::now().to_datetime()
}

/// Set current time by using the given current time to calculate the time of system start
pub fn set(now: DateTime<Utc>) {
    if let Some(uptime) = uptime() {
        set_start_time(now - uptime);
        debug!("Time: Current time set to {}", now);
    }
}
