//! How long the app's timers are: normal, or shortened for testing (decision #054).
//!
//! **Test timers** exist only in test images (the `test-timers` build option, never in a
//! release) and only when the server's `VEHICLEMANAGER_TEST_MINUTES` is set to a value
//! N from 1 to 5. Then the idle and job timers take minutes instead of hours, so a manual
//! test plan doesn't need a night or an hour per step:
//!
//! | Timer | Normal | Test |
//! |---|---|---|
//! | Admin mode idle | 15 minutes | N minutes |
//! | Sign out when idle: 1 h / 4 h / 8 h / 24 h | as named | N / 2N / 3N / 4N minutes |
//! | Admin keep-alive (and the server's "last used") | 1 minute | 15 seconds |
//! | Messages reload on an open screen | 5 minutes | 1 minute |
//! | Nightly snapshot, hourly database check | 07:00 UTC daily, hourly | every N minutes |
//! | Recovery code lifetime | 15 minutes | N minutes |
//! | A fuel-up or reading finalizes by itself | 24 hours after its last edit | N minutes |
//! | A maintenance or repair draft is a forgotten job | 14 days after its last edit | N minutes |
//!
//! Never shortened: the sign-in attempt limit (it protects against guessing) and the
//! 30-day session.
//!
//! # Examples
//!
//! ```
//! use domain::preferences::IdleSignOut;
//! use domain::timers::Timers;
//!
//! assert_eq!(Timers::NORMAL.admin_idle_millis(), 15 * 60_000);
//! let test = Timers::test(2).unwrap();
//! assert_eq!(test.admin_idle_millis(), 2 * 60_000);
//! assert_eq!(test.sign_out_after_millis(IdleSignOut::EightHours), Some(6 * 60_000));
//! assert!(Timers::test(6).is_none(), "1 to 5 minutes only");
//! ```

use serde::{Deserialize, Serialize};

use crate::preferences::IdleSignOut;

/// Milliseconds in one minute.
const MINUTE: i64 = 60_000;

/// The shortest and longest test minute settings.
const TEST_MINUTES: core::ops::RangeInclusive<u8> = 1..=5;

/// The timer lengths in effect. The server decides and tells the app with the session.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Timers {
    /// `Some(N)` for test timers of N minutes; `None` for normal timers.
    test_minutes: Option<u8>,
}

/// Why a test-minutes setting wasn't used.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TestTimersError {
    /// This build doesn't include test timers (a release).
    NotInThisBuild,
    /// Not a whole number from 1 to 5.
    OutOfRange,
}

impl core::fmt::Display for TestTimersError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str(match self {
            Self::NotInThisBuild => "this build doesn't include test timers; using normal timers",
            Self::OutOfRange => {
                "test minutes must be a whole number from 1 to 5; using normal timers"
            }
        })
    }
}

impl Timers {
    /// The normal timers.
    pub const NORMAL: Self = Self { test_minutes: None };

    /// Test timers of `minutes` (1 to 5); `None` otherwise.
    #[must_use]
    pub fn test(minutes: u8) -> Option<Self> {
        TEST_MINUTES.contains(&minutes).then_some(Self {
            test_minutes: Some(minutes),
        })
    }

    /// The timers for the server's setting `text` (empty or missing means normal), when
    /// `allowed` says this build includes test timers.
    ///
    /// # Errors
    ///
    /// [`TestTimersError`] when a setting is given but can't be used; the caller then uses
    /// [`Timers::NORMAL`] and says why.
    pub fn from_setting(text: Option<&str>, allowed: bool) -> Result<Self, TestTimersError> {
        let Some(text) = text.map(str::trim).filter(|text| !text.is_empty()) else {
            return Ok(Self::NORMAL);
        };
        if !allowed {
            return Err(TestTimersError::NotInThisBuild);
        }
        text.parse::<u8>()
            .ok()
            .and_then(Self::test)
            .ok_or(TestTimersError::OutOfRange)
    }

    /// `Some(N)` when test timers of N minutes are on.
    #[must_use]
    pub fn test_minutes(self) -> Option<u8> {
        // Clamped so a value from elsewhere (e.g. deserialized) can't make a timer zero.
        self.test_minutes
            .map(|minutes| minutes.clamp(*TEST_MINUTES.start(), *TEST_MINUTES.end()))
    }

    /// N minutes in milliseconds, for test timers.
    fn test_millis(self) -> Option<i64> {
        self.test_minutes()
            .map(|minutes| i64::from(minutes) * MINUTE)
    }

    /// How long a fuel-up or reading stays a draft after its last edit (#034).
    #[must_use]
    pub fn finalize_after_millis(self) -> i64 {
        self.test_millis()
            .unwrap_or(crate::event::FINALIZE_AFTER_MILLIS)
    }

    /// How long a maintenance or repair draft sits untouched before it's a forgotten job
    /// (#034).
    #[must_use]
    pub fn forgotten_after_millis(self) -> i64 {
        self.test_millis()
            .unwrap_or(crate::event::FORGOTTEN_AFTER_MILLIS)
    }

    /// How long Admin mode lasts without activity.
    #[must_use]
    pub fn admin_idle_millis(self) -> i64 {
        self.test_millis().unwrap_or(15 * MINUTE)
    }

    /// How often the app keeps Admin mode alive while there's input, and how often the server
    /// records a session's use.
    #[must_use]
    pub fn keep_alive_millis(self) -> i64 {
        // A quarter of the shortest test Admin timeout (1 minute), so it stays well inside it.
        if self.test_minutes.is_some() {
            15_000
        } else {
            MINUTE
        }
    }

    /// How often an open app reloads the messages.
    #[must_use]
    pub fn inbox_refresh_millis(self) -> i64 {
        if self.test_minutes.is_some() {
            MINUTE
        } else {
            5 * MINUTE
        }
    }

    /// How long a recovery code works.
    #[must_use]
    pub fn recovery_code_millis(self) -> i64 {
        self.test_millis().unwrap_or(15 * MINUTE)
    }

    /// With test timers, every scheduled job runs every N minutes instead of on its own
    /// schedule; `None` with normal timers.
    #[must_use]
    pub fn job_interval_millis(self) -> Option<i64> {
        self.test_millis()
    }

    /// How long idle before signing out with `choice`; `None` for Never. Test timers keep the
    /// choices in order: 1 hour → N minutes, 4 hours → 2N, 8 hours → 3N, 24 hours → 4N.
    #[must_use]
    pub fn sign_out_after_millis(self, choice: IdleSignOut) -> Option<i64> {
        let Some(minute) = self.test_millis() else {
            return choice.after_millis();
        };
        let multiple = match choice {
            IdleSignOut::OneHour => 1,
            IdleSignOut::FourHours => 2,
            IdleSignOut::EightHours => 3,
            IdleSignOut::TwentyFourHours => 4,
            IdleSignOut::Never => return None,
        };
        Some(multiple * minute)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn normal_timers_are_the_agreed_ones() {
        let timers = Timers::NORMAL;
        assert_eq!(timers.test_minutes(), None);
        assert_eq!(timers.admin_idle_millis(), 15 * MINUTE);
        assert_eq!(timers.keep_alive_millis(), MINUTE);
        assert_eq!(timers.inbox_refresh_millis(), 5 * MINUTE);
        assert_eq!(timers.recovery_code_millis(), 15 * MINUTE);
        assert_eq!(timers.job_interval_millis(), None);
        assert_eq!(
            timers.sign_out_after_millis(IdleSignOut::EightHours),
            Some(8 * 60 * MINUTE)
        );
        assert_eq!(timers.sign_out_after_millis(IdleSignOut::Never), None);
    }

    #[test]
    fn test_timers_use_n_minutes_and_keep_the_sign_out_choices_in_order() {
        let timers = Timers::test(2).unwrap();
        assert_eq!(timers.test_minutes(), Some(2));
        assert_eq!(timers.admin_idle_millis(), 2 * MINUTE);
        assert_eq!(timers.keep_alive_millis(), 15_000);
        assert_eq!(timers.inbox_refresh_millis(), MINUTE);
        assert_eq!(timers.recovery_code_millis(), 2 * MINUTE);
        assert_eq!(timers.job_interval_millis(), Some(2 * MINUTE));
        let choices = [
            IdleSignOut::OneHour,
            IdleSignOut::FourHours,
            IdleSignOut::EightHours,
            IdleSignOut::TwentyFourHours,
        ];
        let minutes: Vec<Option<i64>> = choices
            .iter()
            .map(|choice| timers.sign_out_after_millis(*choice).map(|ms| ms / MINUTE))
            .collect();
        assert_eq!(minutes, vec![Some(2), Some(4), Some(6), Some(8)]);
        assert_eq!(timers.sign_out_after_millis(IdleSignOut::Never), None);
    }

    #[test]
    fn the_keep_alive_stays_inside_the_shortest_admin_timeout() {
        let shortest = Timers::test(1).unwrap();
        assert!(shortest.keep_alive_millis() * 4 <= shortest.admin_idle_millis());
    }

    #[test]
    fn the_setting_needs_a_test_build_and_one_to_five() {
        assert_eq!(Timers::from_setting(None, false), Ok(Timers::NORMAL));
        assert_eq!(Timers::from_setting(Some("  "), false), Ok(Timers::NORMAL));
        assert_eq!(
            Timers::from_setting(Some("2"), false),
            Err(TestTimersError::NotInThisBuild)
        );
        assert_eq!(
            Timers::from_setting(Some(" 3 "), true),
            Ok(Timers::test(3).unwrap())
        );
        for bad in ["0", "6", "two", "-1", "2.5"] {
            assert_eq!(
                Timers::from_setting(Some(bad), true),
                Err(TestTimersError::OutOfRange),
                "{bad}"
            );
        }
    }

    #[test]
    fn a_zero_from_elsewhere_still_gives_a_one_minute_timer() {
        let timers: Timers = serde_json::from_str(r#"{"test_minutes":0}"#).unwrap();
        assert_eq!(timers.admin_idle_millis(), MINUTE);
    }
}
