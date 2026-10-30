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

#[cfg(test)]
mod tests {
    use super::*;

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
