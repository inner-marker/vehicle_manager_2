//! Hybrid logical clock (HLC): the timestamp on every change (decisions #004, #043).
//!
//! Device wall clocks drift or get set wrong, so plain timestamps could let an older
//! edit beat a newer one. An HLC follows real time, but never goes backward and
//! always sorts after every stamp the device has already seen (plan §3 "Hybrid
//! logical clocks").
//!
//! An [`Hlc`] packs **48 bits of milliseconds since 1970 and a 16-bit counter** into
//! one `u64` that sorts as a plain integer. When two stamps are equal, the device ID
//! breaks the tie (see `domain::id`).
//!
//! [`HlcClock`] produces stamps from a passed-in [`Clock`]: `domain` never reads the
//! real clock. The caller persists [`HlcClock::last`] and passes it back after a
//! restart, so stamps stay increasing across restarts.
//!
//! # Examples
//!
//! ```
//! use domain::hlc::{Hlc, HlcClock};
//! use domain::time::{Clock, UnixMillis};
//!
//! // A wall clock that stands still, to show the counter.
//! struct Fixed;
//! impl Clock for Fixed {
//!     fn now(&self) -> UnixMillis {
//!         UnixMillis::from_millis(1_700_000_000_000)
//!     }
//! }
//!
//! let mut clock = HlcClock::new(Fixed, Hlc::ZERO);
//!
//! let first = clock.tick().unwrap();
//! let second = clock.tick().unwrap(); // same millisecond: the counter moves instead
//! assert!(second > first);
//! assert_eq!(second.physical(), first.physical());
//!
//! // A stamp from a device whose clock is ahead: ours moves past it.
//! let incoming = Hlc::new(UnixMillis::from_millis(1_700_000_060_000), 3).unwrap();
//! let after = clock.receive(incoming).unwrap();
//! assert!(after > incoming);
//! ```

use core::fmt;

use crate::time::{Clock, UnixMillis};

/// Bits used by the counter, below the physical time.
const COUNTER_BITS: u32 = 16;

/// The largest physical time an HLC can hold: 2^48 − 1 ms after 1970 (year ~10889).
const MAX_PHYSICAL_MILLIS: i64 = (1 << 48) - 1;

/// A hybrid logical clock stamp: physical milliseconds and a counter, packed so that
/// comparing two stamps compares their packed `u64` values.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Hlc(u64);

/// Why an HLC couldn't be made.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HlcError {
    /// The physical time is before 1970 or after the 48-bit limit.
    PhysicalTimeOutOfRange(UnixMillis),
}

impl fmt::Display for HlcError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::PhysicalTimeOutOfRange(time) => write!(
                formatter,
                "physical time {} ms is outside the HLC range 0..={MAX_PHYSICAL_MILLIS}",
                time.as_millis()
            ),
        }
    }
}

impl std::error::Error for HlcError {}

impl Hlc {
    /// The lowest stamp: 1970, counter 0. The starting point for a new device.
    pub const ZERO: Self = Self(0);

    /// Makes a stamp from a physical time and a counter.
    ///
    /// # Errors
    ///
    /// [`HlcError::PhysicalTimeOutOfRange`] if `physical` is before 1970 or past the
    /// 48-bit limit.
    pub fn new(physical: UnixMillis, counter: u16) -> Result<Self, HlcError> {
        let millis = u64::try_from(physical.as_millis())
            .ok()
            .filter(|&millis| millis <= MAX_PHYSICAL_MILLIS.unsigned_abs())
            .ok_or(HlcError::PhysicalTimeOutOfRange(physical))?;
        Ok(Self((millis << COUNTER_BITS) | u64::from(counter)))
    }

    /// Rebuilds a stamp from its packed form (e.g. read from storage).
    #[must_use]
    pub const fn from_packed(packed: u64) -> Self {
        Self(packed)
    }

    /// The packed form: physical milliseconds in the top 48 bits, the counter below.
    #[must_use]
    pub const fn packed(self) -> u64 {
        self.0
    }

    /// The physical part, as an instant.
    #[must_use]
    pub fn physical(self) -> UnixMillis {
        // At most 48 bits, so it always fits in an i64.
        UnixMillis::from_millis(i64::try_from(self.0 >> COUNTER_BITS).unwrap_or(i64::MAX))
    }

    /// The counter part.
    #[must_use]
    pub fn counter(self) -> u16 {
        // The low 16 bits are the counter by construction, so this never falls back.
        u16::try_from(self.0 & u64::from(u16::MAX)).unwrap_or(u16::MAX)
    }

    /// The next stamp after `self` in the same or a later millisecond: counter + 1,
    /// or the next millisecond with counter 0 when the counter is full.
    fn successor(self) -> Result<Self, HlcError> {
        match self.counter().checked_add(1) {
            Some(counter) => Self::new(self.physical(), counter),
            None => Self::new(
                UnixMillis::from_millis(self.physical().as_millis().saturating_add(1)),
                0,
            ),
        }
    }
}

/// Produces HLC stamps for one device from a wall [`Clock`].
#[derive(Debug)]
pub struct HlcClock<C> {
    wall: C,
    last: Hlc,
}

impl<C: Clock> HlcClock<C> {
    /// Starts from `last`, the newest stamp this device has issued or received (persisted
    /// by the caller; [`Hlc::ZERO`] for a new device).
    pub const fn new(wall: C, last: Hlc) -> Self {
        Self { wall, last }
    }

    /// The newest stamp so far. Persist it so a restart continues after it.
    #[must_use]
    pub const fn last(&self) -> Hlc {
        self.last
    }

    /// A stamp for a local change: the wall time if it's ahead of the last stamp,
    /// otherwise the last stamp's successor, so stamps never go backward.
    ///
    /// # Errors
    ///
    /// [`HlcError::PhysicalTimeOutOfRange`] if the wall clock is after the 48-bit limit.
    /// A wall clock before 1970 is treated as behind and never fails.
    pub fn tick(&mut self) -> Result<Hlc, HlcError> {
        let wall = self.wall_stamp()?;
        self.last = if wall > self.last {
            wall
        } else {
            self.last.successor()?
        };
        Ok(self.last)
    }

    /// Takes in a stamp from another device or the server: the result sorts after both
    /// it and every stamp this clock has issued, and follows the wall time if that's
    /// ahead of both.
    ///
    /// Callers that accept stamps from devices check [`crate::hlc_utils::check_not_too_far_ahead`]
    /// first, since an incoming stamp far in the future would drag this clock with it.
    ///
    /// # Errors
    ///
    /// [`HlcError::PhysicalTimeOutOfRange`] if the result would pass the 48-bit limit.
    pub fn receive(&mut self, incoming: Hlc) -> Result<Hlc, HlcError> {
        let wall = self.wall_stamp()?;
        let newest_seen = self.last.max(incoming);
        // The wall time wins only if it's in a later millisecond than everything seen;
        // otherwise continue counting from the newest stamp seen.
        self.last = if wall.physical() > newest_seen.physical() {
            wall
        } else {
            newest_seen.successor()?
        };
        Ok(self.last)
    }

    /// The wall time as a stamp with counter 0; times before 1970 become [`Hlc::ZERO`].
    fn wall_stamp(&self) -> Result<Hlc, HlcError> {
        let now = self.wall.now();
        if now.as_millis() < 0 {
            return Ok(Hlc::ZERO);
        }
        Hlc::new(now, 0)
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_support::ManualClock;

    const START: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    fn stamp(millis: i64, counter: u16) -> Hlc {
        Hlc::new(UnixMillis::from_millis(millis), counter).unwrap()
    }

    #[test]
    fn packed_stamp_round_trips_and_sorts_by_time_then_counter() {
        let hlc = stamp(1_700_000_000_000, 7);
        assert_eq!(hlc.physical(), START);
        assert_eq!(hlc.counter(), 7);
        assert_eq!(Hlc::from_packed(hlc.packed()), hlc);

        assert!(stamp(1, 0) > stamp(0, u16::MAX));
        assert!(stamp(1, 2) > stamp(1, 1));
    }

    #[test]
    fn physical_time_outside_48_bits_is_rejected() {
        assert!(Hlc::new(UnixMillis::from_millis(-1), 0).is_err());
        assert!(Hlc::new(UnixMillis::from_millis(MAX_PHYSICAL_MILLIS), 0).is_ok());
        assert_eq!(
            Hlc::new(UnixMillis::from_millis(MAX_PHYSICAL_MILLIS + 1), 0),
            Err(HlcError::PhysicalTimeOutOfRange(UnixMillis::from_millis(
                MAX_PHYSICAL_MILLIS + 1
            )))
        );
    }

    #[test]
    fn tick_follows_the_wall_clock_when_it_moves_forward() {
        let wall = ManualClock::new(START);
        let mut clock = HlcClock::new(&wall, Hlc::ZERO);

        assert_eq!(clock.tick().unwrap(), stamp(START.as_millis(), 0));
        wall.advance(5);
        assert_eq!(clock.tick().unwrap(), stamp(START.as_millis() + 5, 0));
    }

    #[test]
    fn tick_counts_up_when_the_wall_clock_stands_still_or_goes_back() {
        let wall = ManualClock::new(START);
        let mut clock = HlcClock::new(&wall, Hlc::ZERO);
        clock.tick().unwrap();

        assert_eq!(clock.tick().unwrap(), stamp(START.as_millis(), 1));
        wall.advance(-60_000);
        assert_eq!(clock.tick().unwrap(), stamp(START.as_millis(), 2));
    }

    #[test]
    fn full_counter_moves_to_the_next_millisecond() {
        let wall = ManualClock::new(START);
        let mut clock = HlcClock::new(&wall, stamp(START.as_millis(), u16::MAX));

        assert_eq!(clock.tick().unwrap(), stamp(START.as_millis() + 1, 0));
    }

    #[test]
    fn restart_continues_after_the_persisted_stamp() {
        let wall = ManualClock::new(START);
        let persisted = stamp(START.as_millis() + 10_000, 4);
        let mut clock = HlcClock::new(&wall, persisted);

        assert!(clock.tick().unwrap() > persisted);
    }

    #[test]
    fn receive_moves_past_a_stamp_from_a_faster_clock() {
        let wall = ManualClock::new(START);
        let mut clock = HlcClock::new(&wall, Hlc::ZERO);
        let incoming = stamp(START.as_millis() + 60_000, 3);

        assert_eq!(
            clock.receive(incoming).unwrap(),
            stamp(START.as_millis() + 60_000, 4)
        );
        // Local edits after syncing sort after everything received.
        assert!(clock.tick().unwrap() > incoming);
    }

    #[test]
    fn receive_follows_the_wall_clock_when_it_is_ahead() {
        let wall = ManualClock::new(START);
        let mut clock = HlcClock::new(&wall, Hlc::ZERO);

        assert_eq!(
            clock.receive(stamp(1_000, 9)).unwrap(),
            stamp(START.as_millis(), 0)
        );
    }

    #[test]
    fn a_wall_clock_before_1970_never_fails() {
        let wall = ManualClock::new(UnixMillis::from_millis(-5_000));
        let mut clock = HlcClock::new(&wall, Hlc::ZERO);

        assert_eq!(clock.tick().unwrap(), stamp(0, 1));
    }

    /// One step of a simulated device: the wall clock moves (possibly backward), then
    /// the device either makes a local change or receives a stamp.
    #[derive(Debug, Clone)]
    enum Step {
        Tick {
            clock_moves: i64,
        },
        Receive {
            clock_moves: i64,
            incoming_millis: i64,
            incoming_counter: u16,
        },
    }

    fn step() -> impl Strategy<Value = Step> {
        // Clock jumps of up to an hour either way, and incoming stamps within a day of START.
        let moves = -3_600_000_i64..3_600_000;
        prop_oneof![
            moves
                .clone()
                .prop_map(|clock_moves| Step::Tick { clock_moves }),
            (moves, -86_400_000_i64..86_400_000, any::<u16>()).prop_map(
                |(clock_moves, offset, incoming_counter)| Step::Receive {
                    clock_moves,
                    incoming_millis: START.as_millis() + offset,
                    incoming_counter,
                }
            ),
        ]
    }

    proptest! {
        #[test]
        fn stamps_strictly_increase_whatever_the_wall_clock_does(steps in prop::collection::vec(step(), 1..200)) {
            let wall = ManualClock::new(START);
            let mut clock = HlcClock::new(&wall, Hlc::ZERO);
            let mut previous = Hlc::ZERO;

            for step in steps {
                let issued = match step {
                    Step::Tick { clock_moves } => {
                        wall.advance(clock_moves);
                        clock.tick().unwrap()
                    }
                    Step::Receive { clock_moves, incoming_millis, incoming_counter } => {
                        wall.advance(clock_moves);
                        let incoming = stamp(incoming_millis, incoming_counter);
                        let received = clock.receive(incoming).unwrap();
                        prop_assert!(received > incoming, "{received:?} <= incoming {incoming:?}");
                        received
                    }
                };
                prop_assert!(issued > previous, "{issued:?} <= previous {previous:?}");
                previous = issued;
            }
        }

        #[test]
        fn packing_round_trips(millis in 0..=MAX_PHYSICAL_MILLIS, counter in any::<u16>()) {
            let hlc = stamp(millis, counter);
            prop_assert_eq!(hlc.physical().as_millis(), millis);
            prop_assert_eq!(hlc.counter(), counter);
            prop_assert_eq!(Hlc::from_packed(hlc.packed()), hlc);
        }
    }
}
