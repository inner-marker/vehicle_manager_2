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

/// A device's name, as shown in Settings → Devices: a computer's name ("Jordans-MacBook-Pro"),
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

/// A browser's name from its `User-Agent`, e.g. "Safari on iPhone" (phase-3.md 2e), so the
/// Devices list can tell browsers apart. `None` if neither the browser nor the system is
/// recognized (it's then labelled just "Browser"). Only the browser's and system's names are
/// kept, never versions or the rest of the string.
#[must_use]
pub fn browser_label(user_agent: &str) -> Option<DeviceName> {
    // Order matters: Edge and Chrome on iPhone also say "Safari", and Chrome says "Safari".
    let browser = [
        ("Edg", "Edge"),
        ("Firefox/", "Firefox"),
        ("FxiOS/", "Firefox"),
        ("CriOS/", "Chrome"),
        ("Chrome/", "Chrome"),
        ("Safari/", "Safari"),
    ]
    .into_iter()
    .find(|(marker, _)| user_agent.contains(marker))
    .map(|(_, name)| name);
    let system = [
        ("iPhone", "iPhone"),
        ("iPad", "iPad"),
        ("Android", "Android"),
        ("CrOS", "ChromeOS"),
        ("Windows", "Windows"),
        ("Macintosh", "Mac"),
        ("Mac OS X", "Mac"),
        ("Linux", "Linux"),
    ]
    .into_iter()
    .find(|(marker, _)| user_agent.contains(marker))
    .map(|(_, name)| name);
    let label = match (browser, system) {
        (Some(browser), Some(system)) => format!("{browser} on {system}"),
        (Some(browser), None) => browser.to_owned(),
        (None, Some(system)) => format!("Browser on {system}"),
        (None, None) => return None,
    };
    DeviceName::parse(&label).ok()
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
            DeviceName::parse("  Jordans-MacBook-Pro ")
                .unwrap()
                .as_str(),
            "Jordans-MacBook-Pro"
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

    #[test]
    fn browsers_are_named_from_their_user_agent() {
        let label = |agent: &str| browser_label(agent).map(|name| name.as_str().to_owned());
        let iphone_safari = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1";
        let mac_chrome = "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36";
        let windows_edge = "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Safari/537.36 Edg/131.0.0.0";
        let iphone_chrome = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) CriOS/131.0.6778.73 Mobile/15E148 Safari/604.1";
        let linux_firefox =
            "Mozilla/5.0 (X11; Linux x86_64; rv:132.0) Gecko/20100101 Firefox/132.0";
        let android_chrome = "Mozilla/5.0 (Linux; Android 14; Pixel 8) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/131.0.0.0 Mobile Safari/537.36";
        assert_eq!(label(iphone_safari).as_deref(), Some("Safari on iPhone"));
        assert_eq!(label(mac_chrome).as_deref(), Some("Chrome on Mac"));
        assert_eq!(label(windows_edge).as_deref(), Some("Edge on Windows"));
        assert_eq!(label(iphone_chrome).as_deref(), Some("Chrome on iPhone"));
        assert_eq!(label(linux_firefox).as_deref(), Some("Firefox on Linux"));
        assert_eq!(label(android_chrome).as_deref(), Some("Chrome on Android"));
        assert_eq!(label("curl/8.7.1"), None);
        assert_eq!(label(""), None);
    }
}
