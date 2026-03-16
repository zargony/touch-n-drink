use esp_hal::rtc_cntl::SocResetReason;
use esp_hal::system;
use esp_hal::time::{Duration, Instant};

/// Restart system
pub fn restart() -> ! {
    system::software_reset()
}

/// Check if system was recently restarted.
/// Only takes software restarts into account, no other reasons like power on or watchdog failure.
/// Can be used to avoid update loops if updates are checked on restart and something went wrong
/// with release versioning.
pub fn recently_restarted() -> bool {
    system::reset_reason() == Some(SocResetReason::CoreSw)
        && Instant::now().duration_since_epoch() < Duration::from_secs(300)
}
