//! Instants in time and the [`Clock`] abstraction that supplies "now".
//!
//! Code in `domain` **never reads the real clock.** It receives the current time
//! from its caller, either as a plain [`UnixMillis`] value or, when it needs to
//! read the time more than once, through a [`Clock`]. This keeps HLC, interval,
//! and "due soon" logic exactly repeatable in tests.
//!
//! Real-clock implementations live outside `domain` (native and web need different
//! ones, and `std::time::SystemTime::now` panics on `wasm32-unknown-unknown`).
//! Tests use `test_support::ManualClock`.

/// A UTC instant as Unix epoch milliseconds.
///
/// This is the storage form of every instant (decisions.md #008): exact, sortable
/// as a plain integer, and the same representation as the HLC's physical time.
/// Negative values are instants before 1970. The time zone where something
/// happened is stored separately; this type never carries one.
///
/// # Examples
///
/// ```
/// use domain::time::UnixMillis;
///
/// let earlier = UnixMillis::from_millis(1_700_000_000_000);
/// let later = UnixMillis::from_millis(1_700_000_000_001);
/// assert!(earlier < later);
/// assert_eq!(later.as_millis(), 1_700_000_000_001);
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct UnixMillis(i64);

impl UnixMillis {
    /// The Unix epoch, 1970-01-01T00:00:00Z.
    pub const EPOCH: Self = Self(0);

    /// Creates an instant from milliseconds since the Unix epoch (UTC).
    pub const fn from_millis(millis: i64) -> Self {
        Self(millis)
    }

    /// Returns the milliseconds since the Unix epoch (UTC).
    pub const fn as_millis(self) -> i64 {
        self.0
    }
}

/// A source of the current time, supplied to `domain` by its caller.
///
/// The app and server provide real-clock implementations; tests provide a
/// controllable one. Prefer passing a plain [`UnixMillis`] when a function only
/// needs "now" once.
///
/// # Examples
///
/// ```
/// use domain::time::{Clock, UnixMillis};
///
/// struct FixedClock(UnixMillis);
///
/// impl Clock for FixedClock {
///     fn now(&self) -> UnixMillis {
///         self.0
///     }
/// }
///
/// let clock = FixedClock(UnixMillis::from_millis(42));
/// assert_eq!(clock.now(), UnixMillis::from_millis(42));
/// ```
pub trait Clock {
    /// Returns the current instant.
    fn now(&self) -> UnixMillis;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unix_millis_orders_like_its_integer_value() {
        let before_epoch = UnixMillis::from_millis(-1);
        let later = UnixMillis::from_millis(1_700_000_000_000);

        assert!(before_epoch < UnixMillis::EPOCH);
        assert!(UnixMillis::EPOCH < later);
        assert_eq!(
            UnixMillis::from_millis(5).cmp(&UnixMillis::from_millis(5)),
            core::cmp::Ordering::Equal
        );
    }

    #[test]
    fn unix_millis_round_trips_through_millis() {
        for millis in [i64::MIN, -1, 0, 1, 1_700_000_000_000, i64::MAX] {
            assert_eq!(UnixMillis::from_millis(millis).as_millis(), millis);
        }
    }
}
