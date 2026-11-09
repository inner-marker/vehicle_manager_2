//! Devices: where someone is signed in (phase-3.md milestone 2, decision #073).
//!
//! Every session belongs to a device: a **desktop** app paired to the server (it has a key
//! and signs its requests) or a **browser** (a cookie, no key). The household sees them in
//! Settings → Devices and can sign any of them out. The device's ID is the one its changes
//! are stamped with (`crate::hlc`), so a device's row is kept after it's signed out.

use serde::{Deserialize, Serialize};

/// What kind of device it is. Stored as text (`as_str`), with no fixed list in the database,
/// so a kind can be added later (a phone app) without rebuilding the table.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceKind {
    /// A desktop app, paired with its own key.
    Desktop,
    /// A web browser (a cookie, no key).
    Browser,
}

impl DeviceKind {
    /// Every kind.
    pub const ALL: [Self; 2] = [Self::Desktop, Self::Browser];

    /// The stored text.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Desktop => "desktop",
            Self::Browser => "browser",
        }
    }

    /// The kind stored as `text`, if it's one this version knows.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|kind| kind.as_str() == text)
    }
}

/// The most characters in a device's name.
pub const DEVICE_NAME_MAX_CHARS: usize = 64;

/// A device's name, as shown in Settings → Devices: a computer's name ("Stuarts-MacBook-Pro"),
/// a browser's description ("Safari on iPhone"), or what someone renamed it to.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct DeviceName(String);

/// Why a device name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DeviceNameError {
    /// Nothing but spaces.
    Empty,
    /// More than [`DEVICE_NAME_MAX_CHARS`] characters.
    TooLong,
    /// A control character (a line break, a tab).
    ControlCharacter,
}

impl std::fmt::Display for DeviceNameError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(match self {
            Self::Empty => "the name is empty",
            Self::TooLong => "the name is too long",
            Self::ControlCharacter => "the name has a control character",
        })
    }
}

impl std::error::Error for DeviceNameError {}

impl DeviceName {
    /// `input`, trimmed, if it's a usable name.
    ///
    /// # Errors
    ///
    /// [`DeviceNameError`] if it's empty, too long, or has a control character.
    pub fn parse(input: &str) -> Result<Self, DeviceNameError> {
        let name = input.trim();
        if name.is_empty() {
            return Err(DeviceNameError::Empty);
        }
        if name.chars().count() > DEVICE_NAME_MAX_CHARS {
            return Err(DeviceNameError::TooLong);
        }
        if name.chars().any(char::is_control) {
            return Err(DeviceNameError::ControlCharacter);
        }
        Ok(Self(name.to_owned()))
    }

    /// `input` made into a usable name: control characters become spaces, and it's cut to
    /// [`DEVICE_NAME_MAX_CHARS`]. `None` if nothing is left. For names the app makes itself
    /// (a computer's name), which mustn't stop a sign-in.
    #[must_use]
    pub fn tidy(input: &str) -> Option<Self> {
        let cleaned: String = input
            .chars()
            .map(|c| if c.is_control() { ' ' } else { c })
            .collect();
        let cut: String = cleaned.trim().chars().take(DEVICE_NAME_MAX_CHARS).collect();
        Self::parse(&cut).ok()
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for DeviceName {
    type Error = DeviceNameError;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<DeviceName> for String {
    fn from(name: DeviceName) -> Self {
        name.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_round_trip_through_their_text() {
        for kind in DeviceKind::ALL {
            assert_eq!(DeviceKind::parse(kind.as_str()), Some(kind));
        }
        assert_eq!(DeviceKind::parse("phone"), None);
    }

    #[test]
    fn names_are_trimmed_and_checked() {
        assert_eq!(
            DeviceName::parse("  Stuarts-MacBook-Pro ")
                .unwrap()
                .as_str(),
            "Stuarts-MacBook-Pro"
        );
        assert_eq!(DeviceName::parse("   "), Err(DeviceNameError::Empty));
        assert_eq!(
            DeviceName::parse(&"x".repeat(65)),
            Err(DeviceNameError::TooLong)
        );
        assert!(
            DeviceName::parse(&"é".repeat(64)).is_ok(),
            "characters, not bytes"
        );
        assert_eq!(
            DeviceName::parse("two\nlines"),
            Err(DeviceNameError::ControlCharacter)
        );
    }

    #[test]
    fn a_name_the_app_makes_is_tidied_rather_than_refused() {
        assert_eq!(DeviceName::tidy("odd\tname").unwrap().as_str(), "odd name");
        assert_eq!(
            DeviceName::tidy(&"y".repeat(100)).unwrap().as_str().len(),
            64
        );
        assert_eq!(DeviceName::tidy(" \n "), None);
    }

    #[test]
    fn a_stored_name_that_breaks_the_rules_is_refused_when_read() {
        assert!(serde_json::from_str::<DeviceName>("\"Safari on iPhone\"").is_ok());
        assert!(serde_json::from_str::<DeviceName>("\"\"").is_err());
    }
}
