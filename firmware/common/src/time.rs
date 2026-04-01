use chrono::{DateTime, Duration, NaiveDate, Utc};
use embassy_time::Instant;
use log::debug;

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

/// Real time clock that calculates the current time by adding the relative uptime (duration since
/// system start) to the absolute system start time (must be provided at least once)
#[must_use]
pub struct Rtc {
    /// Absolute time at system start (epoch of `Instant`)
    system_start_time: Option<DateTime<Utc>>,
}

impl Default for Rtc {
    fn default() -> Self {
        Self::new()
    }
}

impl Rtc {
    /// Create new real time clock
    pub fn new() -> Self {
        Self {
            system_start_time: None,
        }
    }

    /// Get current date (if known)
    #[must_use]
    pub fn today(&self) -> Option<NaiveDate> {
        Some(self.now()?.date_naive())
    }

    /// Get current date and time (if known)
    #[must_use]
    pub fn now(&self) -> Option<DateTime<Utc>> {
        // Calculate current date and time by adding the uptime duration to the previously set
        // system start time
        Some(self.system_start_time? + Self::uptime())
    }

    /// Set current date and time
    ///
    /// # Panics
    ///
    /// Panics when system uptime is longer than `i64::MAX` microseconds (~292,277 years)
    pub fn set(&mut self, time_now: DateTime<Utc>) {
        self.set_by_reference(&TimeReference::now(time_now));
    }

    /// Set current date and time by a known time at the given reference
    ///
    /// # Panics
    ///
    /// Panics when reference time is later than `i64::MAX` microseconds (~292,277 years) after start
    pub fn set_by_reference(&mut self, reference: &TimeReference) {
        // Remember current date and time by calculating the system start time
        self.system_start_time = Some(reference.epoch());
        // Safe to unwrap after setting self.system_start_time
        debug!("Time: Current time set to {}", self.now().unwrap());
    }

    /// Converts an `Instant` to a `DateTime<Utc>` if current date and time is known
    #[must_use]
    pub fn instant_to_datetime(&self, time: Instant) -> Option<DateTime<Utc>> {
        Some(self.system_start_time? + instant_to_duration(time)?)
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
}

/// Helper to convert `Instant` to `Duration`
fn instant_to_duration(time: Instant) -> Option<Duration> {
    let us = i64::try_from(time.as_micros()).ok()?;
    Some(Duration::microseconds(us))
}
