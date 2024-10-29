use chrono::{DateTime, TimeDelta, Utc};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::CriticalSectionMutex;
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

/// Duration of system run time
pub fn uptime() -> Option<TimeDelta> {
    let millis = esp_hal::time::now().duration_since_epoch().to_millis();
    TimeDelta::try_milliseconds(i64::try_from(millis).ok()?)
}

/// Current time
#[allow(dead_code)]
pub fn now() -> Option<DateTime<Utc>> {
    if let (Some(start_time), Some(uptime)) = (start_time(), uptime()) {
        Some(start_time + uptime)
    } else {
        None
    }
}

/// Set current time by using the given current time to calculate the time of system start
pub fn set(now: DateTime<Utc>) {
    if let Some(uptime) = uptime() {
        set_start_time(now - uptime);
        debug!("Time: Current time set to {}", now);
    }
}
