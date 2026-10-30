//! Helpers for checking hybrid logical clock stamps ([`crate::hlc`]).

use crate::hlc::Hlc;
use crate::time::UnixMillis;

/// How far ahead of the receiver's clock an incoming stamp may be: 5 minutes.
///
/// A device clock set far in the future is HLC's one weakness: its stamps would beat
/// every later edit. The server refuses changes further ahead than this and flags the
/// device (plan §3 "Hybrid logical clocks").
pub const MAX_AHEAD_MILLIS: i64 = 5 * 60 * 1000;

/// An incoming stamp is too far ahead of the receiver's clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TooFarAhead {
    /// How far ahead it is, in milliseconds.
    pub ahead_millis: i64,
}

/// Checks that `incoming` is at most `max_ahead_millis` ahead of `now`.
///
/// # Errors
///
/// [`TooFarAhead`] with the distance if it's further ahead.
///
/// # Examples
///
/// ```
/// use domain::hlc::Hlc;
/// use domain::hlc_utils::{MAX_AHEAD_MILLIS, check_not_too_far_ahead};
/// use domain::time::UnixMillis;
///
/// let now = UnixMillis::from_millis(1_700_000_000_000);
/// let one_hour_ahead = Hlc::new(UnixMillis::from_millis(1_700_003_600_000), 0).unwrap();
/// assert!(check_not_too_far_ahead(one_hour_ahead, now, MAX_AHEAD_MILLIS).is_err());
/// ```
pub fn check_not_too_far_ahead(
    incoming: Hlc,
    now: UnixMillis,
    max_ahead_millis: i64,
) -> Result<(), TooFarAhead> {
    let ahead_millis = incoming
        .physical()
        .as_millis()
        .saturating_sub(now.as_millis());
    if ahead_millis > max_ahead_millis {
        Err(TooFarAhead { ahead_millis })
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    fn at(millis: i64) -> Hlc {
        Hlc::new(UnixMillis::from_millis(millis), 0).unwrap()
    }

    #[test]
    fn stamps_up_to_the_limit_ahead_are_accepted() {
        assert_eq!(
            check_not_too_far_ahead(at(NOW.as_millis()), NOW, MAX_AHEAD_MILLIS),
            Ok(())
        );
        assert_eq!(
            check_not_too_far_ahead(
                at(NOW.as_millis() + MAX_AHEAD_MILLIS),
                NOW,
                MAX_AHEAD_MILLIS
            ),
            Ok(())
        );
        // Stamps in the past are never "too far ahead".
        assert_eq!(
            check_not_too_far_ahead(at(0), NOW, MAX_AHEAD_MILLIS),
            Ok(())
        );
    }

    #[test]
    fn stamps_beyond_the_limit_are_refused_with_the_distance() {
        assert_eq!(
            check_not_too_far_ahead(
                at(NOW.as_millis() + MAX_AHEAD_MILLIS + 1),
                NOW,
                MAX_AHEAD_MILLIS
            ),
            Err(TooFarAhead {
                ahead_millis: MAX_AHEAD_MILLIS + 1
            })
        );
    }
}
