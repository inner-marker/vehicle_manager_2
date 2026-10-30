//! Users: the people who sign in (plan §6 "Users, roles & permissions", decision #028).
//!
//! A [`User`] holds who someone is and whether they may sign in. Their **password hash is
//! not part of it**: it stays on the server, is never sent to devices, and never goes
//! through sync. What a user may do comes from their role grants (`crate::access`).
//!
//! # Examples
//!
//! ```
//! use domain::user::{PasswordError, Username, check_new_password};
//!
//! let name = Username::parse("  Stuart.R ").unwrap();
//! assert_eq!(name.as_str(), "stuart.r");
//!
//! assert_eq!(check_new_password("short"), Err(PasswordError::TooShort));
//! assert!(check_new_password("correct horse battery staple").is_ok());
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;

/// The ID of a user.
pub type UserId = Id<User>;

/// The fewest characters a username may have.
pub const USERNAME_MIN_CHARS: usize = 3;
/// The most characters a username may have.
pub const USERNAME_MAX_CHARS: usize = 32;
/// The fewest characters a password may have (decision #028, phase-2.md milestone 2).
pub const PASSWORD_MIN_CHARS: usize = 12;
/// The most characters a password may have: well above any real password, and it keeps
/// hashing a submitted password cheap enough that it can't be used to slow the server.
pub const PASSWORD_MAX_CHARS: usize = 256;
/// The most characters a display name may have.
pub const DISPLAY_NAME_MAX_CHARS: usize = 64;

/// A person who can sign in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct User {
    /// What they sign in with: unique, lowercase.
    pub username: Username,
    /// What the app calls them, e.g. "Stuart".
    pub display_name: String,
    /// A disabled user can't sign in, and their sessions end.
    pub disabled: bool,
    /// They must choose a new password at their next sign-in (e.g. after an Admin set a
    /// temporary one).
    pub must_change_password: bool,
}

/// A username: 3–32 characters, lowercase letters, digits, `.`, `-`, and `_`, starting with a
/// letter or digit. Stored lowercase, so "Stuart" and "stuart" are the same user.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Username(String);

/// Why a username was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UsernameError {
    /// Fewer than 3 or more than 32 characters.
    WrongLength,
    /// A character other than letters, digits, `.`, `-`, `_`.
    InvalidCharacter,
    /// Starts with `.`, `-`, or `_`.
    MustStartWithLetterOrDigit,
}

impl UsernameError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::WrongLength => messages::USERNAME_WRONG_LENGTH,
            Self::InvalidCharacter => messages::USERNAME_INVALID_CHARACTER,
            Self::MustStartWithLetterOrDigit => messages::USERNAME_MUST_START_WITH_LETTER_OR_DIGIT,
        }
    }
}

impl Username {
    /// Checks and normalizes a username: surrounding spaces are ignored, and letters are
    /// lowercased.
    ///
    /// # Errors
    ///
    /// The first rule it breaks; see [`UsernameError`].
    pub fn parse(input: &str) -> Result<Self, UsernameError> {
        let name = input.trim().to_ascii_lowercase();
        if !(USERNAME_MIN_CHARS..=USERNAME_MAX_CHARS).contains(&name.chars().count()) {
            return Err(UsernameError::WrongLength);
        }
        let allowed = |character: char| {
            character.is_ascii_lowercase()
                || character.is_ascii_digit()
                || ".-_".contains(character)
        };
        if !name.chars().all(allowed) {
            return Err(UsernameError::InvalidCharacter);
        }
        if name.starts_with(['.', '-', '_']) {
            return Err(UsernameError::MustStartWithLetterOrDigit);
        }
        Ok(Self(name))
    }

    /// The normalized username.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Username {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<Username> for String {
    fn from(username: Username) -> Self {
        username.0
    }
}

/// Why a new password was refused. Only length is checked (current NIST guidance: long
/// passwords, no composition rules).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasswordError {
    /// Fewer than 12 characters.
    TooShort,
    /// More than 256 characters.
    TooLong,
}

impl PasswordError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::TooShort => messages::PASSWORD_TOO_SHORT,
            Self::TooLong => messages::PASSWORD_TOO_LONG,
        }
    }
}

/// Checks a password someone is choosing. Characters are counted, not bytes, so every
/// language is treated alike. Spaces count; nothing is trimmed.
///
/// # Errors
///
/// [`PasswordError`] if it's too short or too long.
pub fn check_new_password(password: &str) -> Result<(), PasswordError> {
    let length = password.chars().count();
    if length < PASSWORD_MIN_CHARS {
        Err(PasswordError::TooShort)
    } else if length > PASSWORD_MAX_CHARS {
        Err(PasswordError::TooLong)
    } else {
        Ok(())
    }
}

/// A display name: trimmed, 1–64 characters.
///
/// # Errors
///
/// [`DisplayNameError`] if it's empty or too long.
pub fn check_display_name(input: &str) -> Result<String, DisplayNameError> {
    let name = input.trim();
    match name.chars().count() {
        0 => Err(DisplayNameError::Empty),
        length if length > DISPLAY_NAME_MAX_CHARS => Err(DisplayNameError::TooLong),
        _ => Ok(name.to_owned()),
    }
}

/// Why a display name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DisplayNameError {
    /// Nothing was entered.
    Empty,
    /// More than 64 characters.
    TooLong,
}

impl DisplayNameError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Empty => messages::DISPLAY_NAME_EMPTY,
            Self::TooLong => messages::DISPLAY_NAME_TOO_LONG,
        }
    }
}

/// The changeable fields of a [`User`]; `Some` means "set to this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserPatch {
    /// See [`User::username`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub username: Option<Username>,
    /// See [`User::display_name`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub display_name: Option<String>,
    /// See [`User::disabled`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub disabled: Option<bool>,
    /// See [`User::must_change_password`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub must_change_password: Option<bool>,
}

impl UserPatch {
    /// Every field of `user`, for creating it.
    #[must_use]
    pub fn all(user: User) -> Self {
        Self {
            username: Some(user.username),
            display_name: Some(user.display_name),
            disabled: Some(user.disabled),
            must_change_password: Some(user.must_change_password),
        }
    }
}

impl Patch for UserPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("username", self.username.is_some()),
            ("display_name", self.display_name.is_some()),
            ("disabled", self.disabled.is_some()),
            ("must_change_password", self.must_change_password.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, set)| set.then_some(field))
        .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        if !fields.contains(&"username") {
            self.username = None;
        }
        if !fields.contains(&"display_name") {
            self.display_name = None;
        }
        if !fields.contains(&"disabled") {
            self.disabled = None;
        }
        if !fields.contains(&"must_change_password") {
            self.must_change_password = None;
        }
    }
}

impl Record for User {
    const ENTITY: &'static str = "user";
    type Patch = UserPatch;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn usernames_are_trimmed_and_lowercased() {
        assert_eq!(Username::parse(" Stuart ").unwrap().as_str(), "stuart");
        assert_eq!(Username::parse("a.b-c_9").unwrap().as_str(), "a.b-c_9");
        assert_eq!(Username::parse("abc"), Username::parse("ABC"));
    }

    #[test]
    fn usernames_breaking_a_rule_are_refused_with_that_rule() {
        let cases = [
            ("ab", UsernameError::WrongLength),
            (&"a".repeat(33), UsernameError::WrongLength),
            ("stu art", UsernameError::InvalidCharacter),
            ("stuart@home", UsernameError::InvalidCharacter),
            ("stüart", UsernameError::InvalidCharacter),
            ("_stuart", UsernameError::MustStartWithLetterOrDigit),
            (".stuart", UsernameError::MustStartWithLetterOrDigit),
        ];
        for (input, expected) in cases {
            assert_eq!(Username::parse(input), Err(expected), "{input:?}");
        }
    }

    #[test]
    fn passwords_need_twelve_characters_counted_not_bytes() {
        assert_eq!(
            check_new_password("elevenchars"),
            Err(PasswordError::TooShort)
        );
        assert_eq!(check_new_password("twelve chars"), Ok(()));
        // 12 characters, but more than 12 bytes.
        assert_eq!(
            check_new_password("ééééééééééé"),
            Err(PasswordError::TooShort)
        );
        assert_eq!(check_new_password("éééééééééééé"), Ok(()));
        assert_eq!(
            check_new_password(&"x".repeat(257)),
            Err(PasswordError::TooLong)
        );
    }

    #[test]
    fn display_names_are_trimmed_and_limited() {
        assert_eq!(check_display_name("  Stuart  "), Ok("Stuart".to_owned()));
        assert_eq!(check_display_name("   "), Err(DisplayNameError::Empty));
        assert_eq!(
            check_display_name(&"x".repeat(65)),
            Err(DisplayNameError::TooLong)
        );
    }

    #[test]
    fn usernames_round_trip_through_json_and_bad_ones_are_refused() {
        let name = Username::parse("stuart").unwrap();
        let json = serde_json::to_string(&name).unwrap();
        assert_eq!(json, "\"stuart\"");
        assert_eq!(serde_json::from_str::<Username>(&json).unwrap(), name);
        assert!(serde_json::from_str::<Username>("\"no spaces\"").is_err());
    }

    #[test]
    fn every_user_error_has_an_english_message() {
        let english = crate::i18n::Translator::english();
        let ids = [
            UsernameError::WrongLength.message(),
            UsernameError::InvalidCharacter.message(),
            UsernameError::MustStartWithLetterOrDigit.message(),
            PasswordError::TooShort.message(),
            PasswordError::TooLong.message(),
            DisplayNameError::Empty.message(),
            DisplayNameError::TooLong.message(),
        ];
        for id in ids {
            assert!(english.has(id), "{id:?}");
        }
    }

    #[test]
    fn a_user_patch_names_only_its_set_fields() {
        let mut patch = UserPatch {
            display_name: Some("S".into()),
            disabled: Some(true),
            ..UserPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["display_name", "disabled"]);
        patch.keep_only(&["disabled"]);
        assert_eq!(patch.changed_fields(), vec!["disabled"]);
    }
}
