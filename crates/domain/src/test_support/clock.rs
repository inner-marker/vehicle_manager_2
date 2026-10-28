//! A controllable [`Clock`] for deterministic tests.

use core::sync::atomic::{AtomicI64, Ordering};

use crate::time::{Clock, UnixMillis};

/// A [`Clock`] that returns whatever time the test sets.
///
/// Uses an atomic internally so it can be shared (`&ManualClock`, or in an `Arc`)
/// with the code under test while the test moves time forward.
#[derive(Debug)]
pub struct ManualClock {
    now_millis: AtomicI64,
}

impl ManualClock {
    /// Creates a clock that reads `start` until it is changed.
    pub fn new(start: UnixMillis) -> Self {
        Self {
            now_millis: AtomicI64::new(start.as_millis()),
        }
    }

    /// Sets the current time, forward or backward (e.g. to simulate a wrong device clock).
    pub fn set(&self, now: UnixMillis) {
        self.now_millis.store(now.as_millis(), Ordering::SeqCst);
    }

    /// Moves the current time by `millis` (negative moves it back) and returns the new time.
    ///
    /// Saturates at the `i64` limits instead of overflowing; tests never get near them.
    /// Meant to be called from the test's own thread: two concurrent `advance` calls
    /// could lose one update.
    pub fn advance(&self, millis: i64) -> UnixMillis {
        let next = UnixMillis::from_millis(self.now().as_millis().saturating_add(millis));
        self.set(next);
        next
    }
}

impl Clock for ManualClock {
    fn now(&self) -> UnixMillis {
        UnixMillis::from_millis(self.now_millis.load(Ordering::SeqCst))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const START: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    #[test]
    fn manual_clock_returns_the_time_it_was_given() {
        let clock = ManualClock::new(START);
        assert_eq!(clock.now(), START);
        assert_eq!(clock.now(), START, "reading the clock must not move it");
    }

    #[test]
    fn manual_clock_advance_moves_by_exactly_the_given_amount() {
        let clock = ManualClock::new(START);

        let after = clock.advance(1_500);

        assert_eq!(after, UnixMillis::from_millis(1_700_000_001_500));
        assert_eq!(clock.now(), after);
    }

    #[test]
    fn manual_clock_can_move_backward_like_a_misset_device_clock() {
        let clock = ManualClock::new(START);

        clock.advance(-60_000);
        assert_eq!(clock.now(), UnixMillis::from_millis(1_699_999_940_000));

        clock.set(UnixMillis::EPOCH);
        assert_eq!(clock.now(), UnixMillis::EPOCH);
    }

    #[test]
    fn manual_clock_advance_saturates_instead_of_overflowing() {
        let clock = ManualClock::new(UnixMillis::from_millis(i64::MAX - 1));
        assert_eq!(clock.advance(10), UnixMillis::from_millis(i64::MAX));
    }
}
