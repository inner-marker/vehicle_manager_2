//! The app's permanent identity: bundle ID, internal app ID, and data-folder name.
//!
//! These are defined once here, and every platform config and path derivation
//! reads them from this module. They are deliberately independent of the
//! display name: the display name may change freely, but changing any of these
//! makes operating systems treat the app as a brand-new app (local databases,
//! keychain entries, and permissions stay behind with the old identity).
//!
//! **Never derive file paths, data directories, or keychain entries from the
//! display name.** Use [`DATA_DIR_NAME`] and [`BUNDLE_ID`] instead.
//!
//! # Examples
//!
//! ```
//! use domain::identity::{APP_ID, BUNDLE_ID, DATA_DIR_NAME};
//!
//! assert_eq!(BUNDLE_ID, "com.innermarker.vehiclemanager");
//! assert_eq!(APP_ID, "vehiclemanager");
//! assert_eq!(DATA_DIR_NAME, "vehiclemanager");
//! ```

/// Reverse-DNS bundle ID that identifies the app to every operating system.
///
/// Must never change once release packages exist.
pub const BUNDLE_ID: &str = "com.innermarker.vehiclemanager";

/// Neutral internal identifier for the app, used where a short ID is needed.
///
/// Matches the last segment of [`BUNDLE_ID`].
pub const APP_ID: &str = "vehiclemanager";

/// The app's name as people see it: the desktop window's title, for now.
///
/// The working name: decision #016 leaves the final display name open. Unlike the IDs above,
/// it can change freely (nothing is named after it).
pub const DISPLAY_NAME: &str = "Vehicle Manager";

/// Name of the per-user folder that holds the app's local data (database, logs).
///
/// A single path segment; platform code joins it onto the OS data directory.
pub const DATA_DIR_NAME: &str = "vehiclemanager";

/// The name of one of the app's environment variables: the app ID in capitals, an
/// underscore, then `name` (e.g. `env_var_name("DATA_DIR")` is `VEHICLEMANAGER_DATA_DIR`).
///
/// Every environment variable the app reads is named through this, so all of them
/// share one prefix that comes from [`APP_ID`], never from the display name.
///
/// # Examples
///
/// ```
/// use domain::identity::env_var_name;
///
/// assert_eq!(env_var_name("DATA_DIR"), "VEHICLEMANAGER_DATA_DIR");
/// ```
pub fn env_var_name(name: &str) -> String {
    format!("{}_{name}", APP_ID.to_ascii_uppercase())
}

/// The most characters a server instance name may have.
pub const INSTANCE_MAX_CHARS: usize = 16;

/// A server's instance name, for a second server beside the household's own (decision #062):
/// e.g. `test`. The main server has none. 1–16 lowercase letters and digits, so it's safe in
/// a cookie name.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct InstanceName(String);

/// Why an instance name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidInstanceName;

impl core::fmt::Display for InvalidInstanceName {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("an instance name is 1 to 16 lowercase letters and digits")
    }
}

impl InstanceName {
    /// The instance named by a setting: `None` for nothing (or only spaces), the main server.
    ///
    /// # Errors
    ///
    /// [`InvalidInstanceName`] unless it's 1–16 lowercase letters and digits.
    pub fn parse(text: &str) -> Result<Option<Self>, InvalidInstanceName> {
        let name = text.trim();
        if name.is_empty() {
            return Ok(None);
        }
        let valid = name.len() <= INSTANCE_MAX_CHARS
            && name
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit());
        valid
            .then(|| Some(Self(name.to_owned())))
            .ok_or(InvalidInstanceName)
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

/// The session cookie's name: `vehiclemanager_session` on the main server, and e.g.
/// `vehiclemanager_test_session` on the `test` instance. Servers on one hostname (different
/// ports) share cookies, so each needs its own (decision #062).
#[must_use]
pub fn session_cookie_name(instance: Option<&InstanceName>) -> String {
    match instance {
        None => format!("{APP_ID}_session"),
        Some(instance) => format!("{APP_ID}_{}_session", instance.as_str()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_instance_name_is_short_lowercase_and_names_its_own_cookie() {
        assert_eq!(InstanceName::parse(""), Ok(None));
        assert_eq!(InstanceName::parse("  "), Ok(None));
        let test = InstanceName::parse(" test ").unwrap().unwrap();
        assert_eq!(test.as_str(), "test");
        for bad in ["Test", "test-1", "te st", "a;b", "seventeenletters1"] {
            assert_eq!(InstanceName::parse(bad), Err(InvalidInstanceName), "{bad}");
        }
        assert_eq!(session_cookie_name(None), "vehiclemanager_session");
        assert_eq!(
            session_cookie_name(Some(&test)),
            "vehiclemanager_test_session"
        );
    }

    #[test]
    fn bundle_id_ends_with_app_id() {
        assert_eq!(BUNDLE_ID.rsplit('.').next(), Some(APP_ID));
    }

    #[test]
    fn bundle_id_is_reverse_dns() {
        let segments: Vec<&str> = BUNDLE_ID.split('.').collect();
        assert!(segments.len() >= 3, "expected at least three segments");
        for segment in segments {
            assert!(!segment.is_empty(), "empty segment in {BUNDLE_ID}");
            assert!(
                segment
                    .chars()
                    .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit()),
                "segment {segment:?} must be lowercase ASCII letters or digits"
            );
        }
    }

    #[test]
    fn env_var_names_share_the_app_id_prefix() {
        assert_eq!(env_var_name("DATA_DIR"), "VEHICLEMANAGER_DATA_DIR");
        assert!(env_var_name("X").starts_with(&APP_ID.to_ascii_uppercase()));
    }

    #[test]
    fn data_dir_name_is_a_single_path_segment() {
        assert!(!DATA_DIR_NAME.is_empty());
        assert!(
            !DATA_DIR_NAME.contains(['/', '\\', '.']),
            "{DATA_DIR_NAME:?} must not contain path separators or dots"
        );
    }
}
