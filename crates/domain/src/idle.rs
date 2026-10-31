//! What the app does when nobody is using it: leave Admin mode, sign out, or keep the
//! server's Admin mode alive (decision #049).
//!
//! The app watches for real input (pointer, keys, scrolling, touch) and asks [`decide`] what
//! to do. Background requests don't count as activity. The server stays the authority: its own
//! limits (15 idle minutes in Admin mode, 30 idle days per session) apply even if the app is
//! closed. The keep-alive exists because the server only sees requests: without it, someone
//! working on a long form in Admin mode would lose Admin mode on the server while the app still
//! showed it.
//!
//! # Examples
//!
//! ```
//! use domain::access::ActiveMode;
//! use domain::idle::{IdleAction, IdleState, decide};
//! use domain::preferences::IdleSignOut;
//! use domain::time::UnixMillis;
//!
//! let minute = 60_000;
//! let state = IdleState {
//!     now: UnixMillis::from_millis(20 * minute),
//!     last_input: UnixMillis::from_millis(4 * minute),
//!     last_keep_alive: UnixMillis::from_millis(4 * minute),
//! };
//! // 16 idle minutes in Admin mode: leave it.
//! assert_eq!(decide(state, ActiveMode::Admin, IdleSignOut::EightHours), IdleAction::LeaveAdmin);
//! // The same 16 minutes in Driver mode: nothing to do.
//! assert_eq!(decide(state, ActiveMode::Driver, IdleSignOut::EightHours), IdleAction::Nothing);
//! ```

use crate::access::ActiveMode;
use crate::preferences::IdleSignOut;
use crate::time::UnixMillis;

/// Admin mode ends after this long without activity: 15 minutes. The server uses the same
/// limit, counted from its last request.
pub const ADMIN_IDLE_MILLIS: i64 = 15 * 60 * 1000;

/// While someone is active in Admin mode, the app tells the server at most this often (once a
/// minute). It matches how often the server records a session's use, so the server's idle
/// clock is never more than about a minute behind the app's.
pub const KEEP_ALIVE_EVERY_MILLIS: i64 = 60 * 1000;

/// The times the decision depends on, all from the same clock.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct IdleState {
    /// Now.
    pub now: UnixMillis,
    /// The last real input (or when watching started, if there's been none since).
    pub last_input: UnixMillis,
    /// The last time the server heard from the app on the person's behalf (or when Admin mode
    /// was entered).
    pub last_keep_alive: UnixMillis,
}

impl IdleState {
    /// How long since the last input; never negative, even if the clock steps back.
    #[must_use]
    pub const fn idle_millis(self) -> i64 {
        millis_between(self.last_input, self.now)
    }
}

/// What the app should do now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IdleAction {
    /// Nothing.
    Nothing,
    /// Check the session with the server, so its Admin mode stays alive while the person is
    /// working (and the app learns if the server ended it).
    KeepAlive,
    /// Leave Admin mode (back to Driver), as **Leave** does.
    LeaveAdmin,
    /// Sign out.
    SignOut,
}

/// Decides what the app should do, given the times in `state`, the active mode, and the
/// person's sign-out setting.
///
/// In order: signing out wins (it also ends Admin mode); then leaving Admin mode after
/// [`ADMIN_IDLE_MILLIS`]; then, in Admin mode, a keep-alive if there was input since the last
/// one and at least [`KEEP_ALIVE_EVERY_MILLIS`] has passed.
#[must_use]
pub fn decide(state: IdleState, mode: ActiveMode, sign_out: IdleSignOut) -> IdleAction {
    let idle = state.idle_millis();
    let in_admin = mode == ActiveMode::Admin;

    if sign_out.after_millis().is_some_and(|limit| idle >= limit) {
        IdleAction::SignOut
    } else if in_admin && idle >= ADMIN_IDLE_MILLIS {
        IdleAction::LeaveAdmin
    } else if in_admin
        && state.last_input > state.last_keep_alive
        && millis_between(state.last_keep_alive, state.now) >= KEEP_ALIVE_EVERY_MILLIS
    {
        IdleAction::KeepAlive
    } else {
        IdleAction::Nothing
    }
}

/// Milliseconds from `earlier` to `later`, or 0 if `later` is before it.
const fn millis_between(earlier: UnixMillis, later: UnixMillis) -> i64 {
    let difference = later.as_millis().saturating_sub(earlier.as_millis());
    if difference > 0 { difference } else { 0 }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    const MINUTE: i64 = 60_000;
    const HOUR: i64 = 60 * MINUTE;

    /// A state at `now_minutes`, with the last input and keep-alive at the given minutes.
    fn at(now_minutes: i64, input_minutes: i64, keep_alive_minutes: i64) -> IdleState {
        IdleState {
            now: UnixMillis::from_millis(now_minutes * MINUTE),
            last_input: UnixMillis::from_millis(input_minutes * MINUTE),
            last_keep_alive: UnixMillis::from_millis(keep_alive_minutes * MINUTE),
        }
    }

    #[test]
    fn a_driver_idle_for_twenty_minutes_stays_signed_in() {
        let action = decide(at(20, 0, 0), ActiveMode::Driver, IdleSignOut::EightHours);
        assert_eq!(action, IdleAction::Nothing);
    }

    #[test]
    fn admin_mode_ends_after_fifteen_idle_minutes_not_before() {
        let sign_out = IdleSignOut::EightHours;
        assert_eq!(
            decide(at(14, 0, 0), ActiveMode::Admin, sign_out),
            IdleAction::Nothing
        );
        assert_eq!(
            decide(at(15, 0, 0), ActiveMode::Admin, sign_out),
            IdleAction::LeaveAdmin
        );
    }

    #[test]
    fn maintainer_mode_doesnt_time_out() {
        let action = decide(
            at(60, 0, 0),
            ActiveMode::Maintainer,
            IdleSignOut::EightHours,
        );
        assert_eq!(action, IdleAction::Nothing);
    }

    #[test]
    fn signing_out_happens_at_the_chosen_time_in_any_mode() {
        for mode in [ActiveMode::Driver, ActiveMode::Maintainer] {
            assert_eq!(
                decide(at(59, 0, 0), mode, IdleSignOut::OneHour),
                IdleAction::Nothing
            );
            assert_eq!(
                decide(at(60, 0, 0), mode, IdleSignOut::OneHour),
                IdleAction::SignOut
            );
        }
    }

    #[test]
    fn signing_out_wins_over_leaving_admin_mode() {
        let action = decide(at(8 * 60, 0, 0), ActiveMode::Admin, IdleSignOut::EightHours);
        assert_eq!(action, IdleAction::SignOut);
    }

    #[test]
    fn never_means_never_signing_out() {
        let a_year = 365 * 24 * 60;
        let action = decide(at(a_year, 0, 0), ActiveMode::Driver, IdleSignOut::Never);
        assert_eq!(action, IdleAction::Nothing);
        let in_admin = decide(at(a_year, 0, 0), ActiveMode::Admin, IdleSignOut::Never);
        assert_eq!(in_admin, IdleAction::LeaveAdmin, "Admin mode still ends");
    }

    #[test]
    fn working_in_admin_mode_keeps_it_alive_once_a_minute() {
        let sign_out = IdleSignOut::EightHours;
        // Input now, but the last keep-alive was only 30 seconds ago: too soon.
        let too_soon = IdleState {
            last_keep_alive: UnixMillis::from_millis(3 * MINUTE - 30_000),
            ..at(3, 3, 0)
        };
        assert_eq!(
            decide(too_soon, ActiveMode::Admin, sign_out),
            IdleAction::Nothing
        );
        // A minute after the last keep-alive, with input since: keep alive.
        assert_eq!(
            decide(at(3, 3, 1), ActiveMode::Admin, sign_out),
            IdleAction::KeepAlive
        );
    }

    #[test]
    fn no_keep_alive_without_input_since_the_last_one() {
        let action = decide(at(10, 1, 2), ActiveMode::Admin, IdleSignOut::EightHours);
        assert_eq!(action, IdleAction::Nothing);
    }

    #[test]
    fn no_keep_alive_outside_admin_mode() {
        let action = decide(
            at(10, 9, 0),
            ActiveMode::Maintainer,
            IdleSignOut::EightHours,
        );
        assert_eq!(action, IdleAction::Nothing);
    }

    #[test]
    fn a_clock_that_steps_back_counts_as_not_idle() {
        let state = at(5, 10, 10);
        assert_eq!(state.idle_millis(), 0);
        assert_eq!(
            decide(state, ActiveMode::Admin, IdleSignOut::OneHour),
            IdleAction::Nothing
        );
    }

    fn any_mode() -> impl Strategy<Value = ActiveMode> {
        prop_oneof![
            Just(ActiveMode::Driver),
            Just(ActiveMode::Maintainer),
            Just(ActiveMode::Admin),
        ]
    }

    fn any_sign_out() -> impl Strategy<Value = IdleSignOut> {
        prop_oneof![
            Just(IdleSignOut::OneHour),
            Just(IdleSignOut::FourHours),
            Just(IdleSignOut::EightHours),
            Just(IdleSignOut::TwentyFourHours),
            Just(IdleSignOut::Never),
        ]
    }

    proptest! {
        /// Someone who used the app within the last 15 minutes is never signed out or moved
        /// out of Admin mode, whatever the other times are.
        #[test]
        fn recent_input_never_ends_anything(
            now in 0..(48 * HOUR),
            idle in 0..ADMIN_IDLE_MILLIS,
            keep_alive in 0..(48 * HOUR),
            mode in any_mode(),
            sign_out in any_sign_out(),
        ) {
            let state = IdleState {
                now: UnixMillis::from_millis(now),
                last_input: UnixMillis::from_millis(now - idle),
                last_keep_alive: UnixMillis::from_millis(keep_alive),
            };
            let action = decide(state, mode, sign_out);
            prop_assert!(matches!(action, IdleAction::Nothing | IdleAction::KeepAlive));
        }

        /// The app signs out exactly when the idle time reaches the chosen limit.
        #[test]
        fn sign_out_matches_the_setting(
            idle in 0..(48 * HOUR),
            mode in any_mode(),
            sign_out in any_sign_out(),
        ) {
            let now = 100 * HOUR;
            let state = IdleState {
                now: UnixMillis::from_millis(now),
                last_input: UnixMillis::from_millis(now - idle),
                last_keep_alive: UnixMillis::from_millis(now - idle),
            };
            let signs_out = decide(state, mode, sign_out) == IdleAction::SignOut;
            let expected = sign_out.after_millis().is_some_and(|limit| idle >= limit);
            prop_assert_eq!(signs_out, expected);
        }
    }
}
