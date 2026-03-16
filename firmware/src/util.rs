use alloc::string::String;
use core::fmt;
use core::ops::Deref;
use embassy_time::Duration;
use esp_hal::rtc_cntl::SocResetReason;
use esp_hal::system;
use serde::Deserialize;

/// Restart system
pub fn restart() -> ! {
    system::software_reset()
}

/// Check if system was recently restarted.
/// Only takes software restarts into account, no other reasons like power on or watchdog failure.
/// Can be used to avoid update loops if updates are checked on restart and something went wrong
/// with release versioning.
pub fn recently_restarted() -> bool {
    use esp_hal::time::{Duration, Instant};

    system::reset_reason() == Some(SocResetReason::CoreSw)
        && Instant::now().duration_since_epoch() < Duration::from_secs(300)
}

/// String with sensitive content (debug and display output redacted)
#[derive(Default, Deserialize)]
#[serde(transparent)]
pub struct SensitiveString(pub String);

impl fmt::Debug for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl fmt::Display for SensitiveString {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            self.0.fmt(f)
        } else {
            "<redacted>".fmt(f)
        }
    }
}

impl Deref for SensitiveString {
    type Target = str;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// Option display helper
pub struct DisplayOption<T: fmt::Display>(pub Option<T>);

impl<T: fmt::Display> fmt::Display for DisplayOption<T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match &self.0 {
            None => write!(f, "-"),
            Some(value) => value.fmt(f),
        }
    }
}

/// List slice helper
pub struct DisplaySlice<'a, T: fmt::Display>(pub &'a [T]);

impl<T: fmt::Display> fmt::Display for DisplaySlice<'_, T> {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0.is_empty() {
            write!(f, "-")?;
        } else {
            for (i, elem) in self.0.iter().enumerate() {
                if i > 0 {
                    write!(f, ", ")?;
                }
                elem.fmt(f)?;
            }
        }
        Ok(())
    }
}

/// Duration display helper
pub struct DisplayDuration(pub Duration);

impl fmt::Display for DisplayDuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let hours = self.0.as_secs() / 3600;
        let min = self.0.as_secs() % 3600 / 60;
        let secs = self.0.as_secs() % 60;
        write!(f, "{hours}h{min}m{secs}s")
    }
}
