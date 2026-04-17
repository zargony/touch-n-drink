//! Real time clock that calculates the current time by adding the relative uptime (duration since
//! system start) to the absolute system start time (must be provided at least once)

use chrono::{DateTime, Duration, NaiveDate, Utc};
use embassy_sync::blocking_mutex::CriticalSectionMutex;
use embassy_time::Instant;
use log::debug;

/// Absolute time at system start (epoch of `Instant`)
static SYSTEM_START_TIME: CriticalSectionMutex<Option<DateTime<Utc>>> =
    CriticalSectionMutex::new(None);

/// Time reference of a known absolute time to a given system time
#[derive(Debug, Clone)]
#[must_use]
pub struct TimeReference {
    /// Known absolute time of the given system time
    pub absolute_time: DateTime<Utc>,
    /// Relative system time (ticks since system start)
    pub system_time: Instant,
}

impl TimeReference {
    /// Create a new time reference with the given absolute time and system time
    pub fn new(absolute_time: DateTime<Utc>, system_time: Instant) -> Self {
        Self {
            absolute_time: absolute_time.to_utc(),
            system_time,
        }
    }

    /// Create a new time reference with the given absolute time for the current point in time
    pub fn now(absolute_time: DateTime<Utc>) -> Self {
        Self::new(absolute_time, Instant::now())
    }

    /// Calculate the epoch (system start time) from this time reference
    ///
    /// # Panics
    ///
    /// Panics when system time is later than `i64::MAX` microseconds (~292,277 years) after start
    #[must_use]
    pub fn epoch(&self) -> DateTime<Utc> {
        // Safe to unwrap as long as system time is less than ~292,277 years
        let duration = instant_to_duration(self.system_time).unwrap();
        self.absolute_time - duration
    }
}

/// Helper to convert `Instant` to `Duration`
fn instant_to_duration(time: Instant) -> Option<Duration> {
    let us = i64::try_from(time.as_micros()).ok()?;
    Some(Duration::microseconds(us))
}

/// Get system start time (if known)
fn system_start_time() -> Option<DateTime<Utc>> {
    SYSTEM_START_TIME.lock(|system_start_time| *system_start_time)
}

/// Set system start time
fn set_system_start_time(time: DateTime<Utc>) {
    // Safety: lock_mut() is safe if not called re-entrantly, which can't happen here
    unsafe {
        SYSTEM_START_TIME.lock_mut(|system_start_time| *system_start_time = Some(time));
    }
}

/// Duration of system run time
///
/// # Panics
///
/// Panics when system uptime is longer than `i64::MAX` microseconds (~292,277 years)
#[must_use]
pub fn uptime() -> Duration {
    // Safe to unwrap as long as system uptime is less than ~292,277 years
    instant_to_duration(Instant::now()).unwrap()
}

/// Get current date (if known)
#[must_use]
pub fn today() -> Option<NaiveDate> {
    Some(now()?.date_naive())
}

/// Get current date and time (if known)
#[must_use]
pub fn now() -> Option<DateTime<Utc>> {
    // Calculate current date and time by adding the uptime duration to the previously set
    // system start time
    Some(system_start_time()? + uptime())
}

/// Set current date and time
///
/// # Panics
///
/// Panics when system uptime is longer than `i64::MAX` microseconds (~292,277 years)
#[expect(dead_code)]
pub fn set(time_now: DateTime<Utc>) {
    set_by_reference(&TimeReference::now(time_now));
}

/// Set current date and time by a known time at the given reference
///
/// # Panics
///
/// Panics when reference time is later than `i64::MAX` microseconds (~292,277 years) after start
pub fn set_by_reference(reference: &TimeReference) {
    // Remember current date and time by calculating the system start time
    set_system_start_time(reference.epoch());
    // Safe to unwrap after setting self.system_start_time
    debug!("Time: Current time set to {}", now().unwrap());
}

/// Converts an `Instant` to a `DateTime<Utc>` if current date and time is known
#[must_use]
pub fn instant_to_datetime(time: Instant) -> Option<DateTime<Utc>> {
    Some(system_start_time()? + instant_to_duration(time)?)
}
