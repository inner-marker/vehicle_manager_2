//! The real clock, for code outside `domain` that must supply "now".

use domain::time::{Clock, UnixMillis};
use web_time::{SystemTime, UNIX_EPOCH};

/// Reads the device's wall clock.
///
/// Uses `web-time`, which is `std::time` on native and the browser's clock on web
/// (where `std::time::SystemTime::now` panics).
#[derive(Debug, Clone, Copy, Default)]
pub struct SystemClock;

impl Clock for SystemClock {
    fn now(&self) -> UnixMillis {
        // `duration_since` fails only for times before 1970 (a badly set clock);
        // the error carries how far before, so negate it.
        let millis = match SystemTime::now().duration_since(UNIX_EPOCH) {
            Ok(since_epoch) => millis_to_i64(since_epoch.as_millis()),
            Err(error) => -millis_to_i64(error.duration().as_millis()),
        };
        UnixMillis::from_millis(millis)
    }
}

/// Saturates rather than wraps: `i64` milliseconds reach ~292 million years.
fn millis_to_i64(millis: u128) -> i64 {
    i64::try_from(millis).unwrap_or(i64::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_clock_reads_a_time_after_this_code_was_written() {
        // 2026-01-01T00:00:00Z. This only checks the clock is wired up and in
        // milliseconds; it doesn't depend on the exact current time.
        const JAN_1_2026_MS: i64 = 1_767_225_600_000;
        assert!(SystemClock.now().as_millis() > JAN_1_2026_MS);
    }
}
