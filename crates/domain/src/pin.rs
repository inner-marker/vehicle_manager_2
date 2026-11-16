//! The desktop's PIN and app lock (phase-3.md milestone 3, decision #074; sub-plan 04 §4.1).
//!
//! - [`check_new_pin`]: the rules a new PIN must follow: 6 to 12 digits, not one digit repeated
//!   (`111111`), not a run up or down (`123456`, `654321`).
//! - [`WrongTries`]: what happens after wrong PINs and passwords: after
//!   [`PIN_TRIES`] wrong PINs only the password works; after [`ALL_TRIES`] wrong tries in all,
//!   the desktop signs out and forgets its sign-in.
//! - [`LockAfter`]: how long the desktop may sit idle before it locks.
//!
//! The PIN never leaves the computer: it unlocks the sign-in stored there (the app's
//! `device_credential`).

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, messages};
use crate::secret::Secret;

/// The fewest digits in a PIN.
pub const PIN_MIN_DIGITS: usize = 6;

/// The most digits in a PIN.
pub const PIN_MAX_DIGITS: usize = 12;

/// Why a new PIN was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PinError {
    /// Something other than digits.
    NotDigits,
    /// Fewer than [`PIN_MIN_DIGITS`].
    TooShort,
    /// More than [`PIN_MAX_DIGITS`].
    TooLong,
    /// One digit repeated (`111111`).
    Repeated,
    /// A run up or down (`123456`, `654321`).
    Run,
}

impl PinError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NotDigits => messages::PIN_NOT_DIGITS,
            Self::TooShort => messages::PIN_TOO_SHORT,
            Self::TooLong => messages::PIN_TOO_LONG,
            Self::Repeated => messages::PIN_REPEATED,
            Self::Run => messages::PIN_RUN,
        }
    }
}

/// Whether `digits` (ASCII digits) step by `step` from each to the next (+1 up, -1 down).
fn steps_by(digits: &[u8], step: i16) -> bool {
    digits
        .windows(2)
        .all(|pair| i16::from(pair[1]) - i16::from(pair[0]) == step)
}

/// `input` as a new PIN, if it follows the rules (see the module docs). Spaces around it are
/// ignored.
///
/// # Errors
///
/// The first rule it breaks.
pub fn check_new_pin(input: &str) -> Result<Secret, PinError> {
    let pin = input.trim();
    if !pin.bytes().all(|byte| byte.is_ascii_digit()) {
        return Err(PinError::NotDigits);
    }
    if pin.len() < PIN_MIN_DIGITS {
        return Err(PinError::TooShort);
    }
    if pin.len() > PIN_MAX_DIGITS {
        return Err(PinError::TooLong);
    }
    let digits = pin.as_bytes();
    if steps_by(digits, 0) {
        return Err(PinError::Repeated);
    }
    if steps_by(digits, 1) || steps_by(digits, -1) {
        return Err(PinError::Run);
    }
    Ok(Secret::new(pin))
}

/// Wrong PINs allowed before only the password works.
pub const PIN_TRIES: u32 = 5;

/// Wrong tries (PINs and passwords together) before the desktop signs out and forgets its
/// sign-in.
pub const ALL_TRIES: u32 = 10;

/// What unlocking allows after some wrong tries.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UnlockStep {
    /// The PIN or the password.
    PinOrPassword {
        /// Wrong PINs left before only the password works.
        pin_left: u32,
        /// Wrong tries of either left before signing out.
        left: u32,
    },
    /// Only the password; `left` more wrong tries before the desktop signs out.
    PasswordOnly {
        /// Wrong tries left before signing out.
        left: u32,
    },
    /// Too many wrong tries: sign out and forget the sign-in.
    SignOut,
}

/// The wrong tries since the last unlock (kept beside the stored sign-in).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct WrongTries {
    /// Wrong PINs.
    pub pins: u32,
    /// Wrong passwords.
    pub passwords: u32,
}

impl WrongTries {
    /// All wrong tries.
    #[must_use]
    pub const fn total(self) -> u32 {
        self.pins.saturating_add(self.passwords)
    }

    /// What unlocking allows now.
    #[must_use]
    pub const fn step(self) -> UnlockStep {
        let total = self.total();
        if total >= ALL_TRIES {
            UnlockStep::SignOut
        } else if self.pins >= PIN_TRIES {
            UnlockStep::PasswordOnly {
                left: ALL_TRIES - total,
            }
        } else {
            UnlockStep::PinOrPassword {
                pin_left: PIN_TRIES - self.pins,
                left: ALL_TRIES - total,
            }
        }
    }

    /// After one more wrong PIN.
    #[must_use]
    pub const fn wrong_pin(self) -> Self {
        Self {
            pins: self.pins.saturating_add(1),
            ..self
        }
    }

    /// After one more wrong password.
    #[must_use]
    pub const fn wrong_password(self) -> Self {
        Self {
            passwords: self.passwords.saturating_add(1),
            ..self
        }
    }
}

/// How long the desktop may sit idle before it locks (per computer; it also locks when the
/// app opens).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LockAfter {
    /// 5 minutes.
    FiveMinutes,
    /// 15 minutes (the default).
    #[default]
    FifteenMinutes,
    /// 30 minutes.
    ThirtyMinutes,
    /// 60 minutes.
    SixtyMinutes,
    /// Only when the app opens: no lock while it's open.
    AtLaunchOnly,
}

impl LockAfter {
    /// Every choice, in the order offered.
    pub const ALL: [Self; 5] = [
        Self::FiveMinutes,
        Self::FifteenMinutes,
        Self::ThirtyMinutes,
        Self::SixtyMinutes,
        Self::AtLaunchOnly,
    ];

    /// How long idle before locking, in milliseconds; `None` for [`LockAfter::AtLaunchOnly`].
    #[must_use]
    pub const fn after_millis(self) -> Option<i64> {
        const MINUTE: i64 = 60 * 1000;
        match self {
            Self::FiveMinutes => Some(5 * MINUTE),
            Self::FifteenMinutes => Some(15 * MINUTE),
            Self::ThirtyMinutes => Some(30 * MINUTE),
            Self::SixtyMinutes => Some(60 * MINUTE),
            Self::AtLaunchOnly => None,
        }
    }

    /// The choice's label.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::FiveMinutes => messages::LOCK_AFTER_5,
            Self::FifteenMinutes => messages::LOCK_AFTER_15,
            Self::ThirtyMinutes => messages::LOCK_AFTER_30,
            Self::SixtyMinutes => messages::LOCK_AFTER_60,
            Self::AtLaunchOnly => messages::LOCK_AFTER_LAUNCH,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_good_pin_passes_and_is_trimmed() {
        assert_eq!(check_new_pin(" 284917 ").unwrap().expose(), "284917");
        assert!(check_new_pin("40817263").is_ok());
        assert!(check_new_pin("112233").is_ok(), "pairs are fine");
        assert!(check_new_pin("135792").is_ok(), "steps of two are fine");
    }

    #[test]
    fn a_pin_breaking_a_rule_says_which() {
        assert_eq!(check_new_pin("28491"), Err(PinError::TooShort));
        assert_eq!(check_new_pin("2849172849172"), Err(PinError::TooLong));
        assert_eq!(check_new_pin("28 4917"), Err(PinError::NotDigits));
        assert_eq!(check_new_pin("28491x"), Err(PinError::NotDigits));
        assert_eq!(
            check_new_pin("٢٨٤٩١٧"),
            Err(PinError::NotDigits),
            "ASCII digits only"
        );
        assert_eq!(check_new_pin("111111"), Err(PinError::Repeated));
        assert_eq!(check_new_pin("123456"), Err(PinError::Run));
        assert_eq!(check_new_pin("3456789"), Err(PinError::Run));
        assert_eq!(check_new_pin("654321"), Err(PinError::Run));
    }

    #[test]
    fn five_wrong_pins_leave_the_password_and_ten_tries_sign_out() {
        let mut tries = WrongTries::default();
        assert_eq!(
            tries.step(),
            UnlockStep::PinOrPassword {
                pin_left: 5,
                left: 10
            }
        );
        for _ in 0..4 {
            tries = tries.wrong_pin();
        }
        assert_eq!(
            tries.step(),
            UnlockStep::PinOrPassword {
                pin_left: 1,
                left: 6
            }
        );
        tries = tries.wrong_pin();
        assert_eq!(tries.step(), UnlockStep::PasswordOnly { left: 5 });
        for _ in 0..4 {
            tries = tries.wrong_password();
        }
        assert_eq!(tries.step(), UnlockStep::PasswordOnly { left: 1 });
        tries = tries.wrong_password();
        assert_eq!(tries.step(), UnlockStep::SignOut);
    }

    #[test]
    fn wrong_passwords_count_toward_the_ten_but_not_the_five() {
        let mut tries = WrongTries::default();
        for _ in 0..9 {
            tries = tries.wrong_password();
        }
        assert_eq!(
            tries.step(),
            UnlockStep::PinOrPassword {
                pin_left: 5,
                left: 1
            }
        );
        assert_eq!(tries.wrong_pin().step(), UnlockStep::SignOut);
    }

    #[test]
    fn lock_after_offers_minutes_or_only_at_launch() {
        assert_eq!(LockAfter::default(), LockAfter::FifteenMinutes);
        assert_eq!(
            LockAfter::FifteenMinutes.after_millis(),
            Some(15 * 60 * 1000)
        );
        assert_eq!(LockAfter::AtLaunchOnly.after_millis(), None);
        let json = serde_json::to_string(&LockAfter::AtLaunchOnly).unwrap();
        assert_eq!(json, "\"at-launch-only\"");
    }
}
