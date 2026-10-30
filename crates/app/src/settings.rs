//! Device settings saved in `settings.json` in the data folder (`crate::data_folder`).
//!
//! For now it holds only the desktop app's server address (decision #027). The app
//! must always start, so a missing, unreadable, or invalid file means "nothing saved",
//! with a warning in the log; it never stops the app.
//!
//! **Never put the session token here:** the desktop app keeps it in memory only (#027;
//! phase-2.md milestone 2).
//! Native-only; the web app has no files.

use std::io;
use std::path::Path;

use domain::log_area;
use domain::server_address::{ServerAddress, ServerAddressError};
use serde::{Deserialize, Serialize};

/// The settings file inside the data folder.
pub const SETTINGS_FILE_NAME: &str = "settings.json";

/// Everything in `settings.json`. Missing fields take their defaults and unknown ones
/// are ignored, so older and newer versions of the app can read each other's files.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(default)]
pub struct Settings {
    /// The server the desktop app talks to, as typed by the user (checked again on load).
    pub server_address: Option<String>,
}

/// Why saving a server address failed.
#[derive(Debug)]
pub enum SaveAddressError {
    /// The address breaks a rule.
    Invalid(ServerAddressError),
    /// The platform reports no data folder to save into.
    NoDataFolder,
    /// The file couldn't be written.
    Io(io::Error),
}

/// Reads the settings in `data_folder`, or the defaults if there are none or the file
/// can't be read (logged as a warning).
#[must_use]
pub fn load(data_folder: &Path) -> Settings {
    let path = data_folder.join(SETTINGS_FILE_NAME);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(error) if error.kind() == io::ErrorKind::NotFound => {
            tracing::debug!(target: log_area::APP, path = %path.display(), "no settings file yet");
            return Settings::default();
        }
        Err(error) => {
            tracing::warn!(target: log_area::APP, path = %path.display(), %error, "can't read settings; using defaults");
            return Settings::default();
        }
    };
    serde_json::from_str(&text).unwrap_or_else(|error| {
        tracing::warn!(target: log_area::APP, path = %path.display(), %error, "settings file is invalid; using defaults");
        Settings::default()
    })
}

/// Writes `settings` to `data_folder` (created if missing). Writes a temporary file
/// first and renames it, so a crash mid-write can't leave a half-written file.
///
/// # Errors
///
/// Returns the I/O error if the folder or file can't be written.
pub fn save(data_folder: &Path, settings: &Settings) -> io::Result<()> {
    std::fs::create_dir_all(data_folder)?;
    let text = serde_json::to_string_pretty(settings).map_err(io::Error::other)?;
    let temporary = data_folder.join(format!("{SETTINGS_FILE_NAME}.tmp"));
    std::fs::write(&temporary, text)?;
    std::fs::rename(&temporary, data_folder.join(SETTINGS_FILE_NAME))
}

/// The saved server address, if there is one and it still follows the rules. An
/// address that breaks them (e.g. a hand-edited file) is ignored with a warning.
#[must_use]
pub fn saved_server_address(data_folder: &Path) -> Option<ServerAddress> {
    let saved = load(data_folder).server_address?;
    match ServerAddress::parse(&saved) {
        Ok(address) => Some(address),
        Err(error) => {
            tracing::warn!(target: log_area::APP, address = %saved, ?error, "saved server address is invalid; ignoring it");
            None
        }
    }
}

/// Checks `input` and saves it as the server address in `data_folder`, keeping the
/// other settings. Returns the normalized address that was saved.
///
/// # Errors
///
/// [`SaveAddressError::Invalid`] if the address breaks a rule (nothing is written),
/// [`SaveAddressError::NoDataFolder`] if `data_folder` is `None`, or
/// [`SaveAddressError::Io`] if the file can't be written.
pub fn save_server_address(
    data_folder: Option<&Path>,
    input: &str,
) -> Result<ServerAddress, SaveAddressError> {
    let address = ServerAddress::parse(input).map_err(SaveAddressError::Invalid)?;
    let data_folder = data_folder.ok_or(SaveAddressError::NoDataFolder)?;

    let mut settings = load(data_folder);
    settings.server_address = Some(address.as_str().to_owned());
    save(data_folder, &settings).map_err(SaveAddressError::Io)?;

    tracing::info!(target: log_area::APP, address = address.as_str(), "server address saved; it applies after a restart");
    Ok(address)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_settings_file_gives_defaults() {
        let temp = tempfile::tempdir().unwrap();
        assert_eq!(load(temp.path()), Settings::default());
        assert_eq!(saved_server_address(temp.path()), None);
    }

    #[test]
    fn corrupt_settings_file_gives_defaults() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(temp.path().join(SETTINGS_FILE_NAME), "{ not json").unwrap();
        assert_eq!(load(temp.path()), Settings::default());
    }

    #[test]
    fn settings_round_trip_through_the_file() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("vehiclemanager");
        let settings = Settings {
            server_address: Some("https://nas.example.ts.net".into()),
        };
        save(&folder, &settings).unwrap();
        assert_eq!(load(&folder), settings);
        assert!(!folder.join(format!("{SETTINGS_FILE_NAME}.tmp")).exists());
    }

    #[test]
    fn unknown_fields_are_ignored() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(SETTINGS_FILE_NAME),
            r#"{"server_address":"https://nas.example.ts.net","from_a_newer_app":1}"#,
        )
        .unwrap();
        assert_eq!(
            load(temp.path()).server_address.as_deref(),
            Some("https://nas.example.ts.net")
        );
    }

    #[test]
    fn saved_address_is_normalized_and_read_back() {
        let temp = tempfile::tempdir().unwrap();
        let saved =
            save_server_address(Some(temp.path()), " HTTPS://NAS.example.ts.net/ ").unwrap();
        assert_eq!(saved.as_str(), "https://nas.example.ts.net");
        assert_eq!(saved_server_address(temp.path()), Some(saved));
    }

    #[test]
    fn invalid_address_is_refused_and_nothing_is_written() {
        let temp = tempfile::tempdir().unwrap();
        let result = save_server_address(Some(temp.path()), "http://nas.example.ts.net");
        assert!(matches!(
            result,
            Err(SaveAddressError::Invalid(
                ServerAddressError::InsecureRemote
            ))
        ));
        assert!(!temp.path().join(SETTINGS_FILE_NAME).exists());
    }

    #[test]
    fn saving_without_a_data_folder_is_an_error() {
        assert!(matches!(
            save_server_address(None, "https://nas.example.ts.net"),
            Err(SaveAddressError::NoDataFolder)
        ));
    }

    #[test]
    fn hand_edited_invalid_address_is_ignored() {
        let temp = tempfile::tempdir().unwrap();
        std::fs::write(
            temp.path().join(SETTINGS_FILE_NAME),
            r#"{"server_address":"http://nas.example.ts.net"}"#,
        )
        .unwrap();
        assert_eq!(saved_server_address(temp.path()), None);
    }

    #[test]
    fn unwritable_data_folder_is_an_io_error() {
        let temp = tempfile::tempdir().unwrap();
        // A regular file where the folder should be: the folder can't be created.
        let blocked = temp.path().join("not-a-folder");
        std::fs::write(&blocked, "").unwrap();
        assert!(matches!(
            save_server_address(Some(&blocked), "https://nas.example.ts.net"),
            Err(SaveAddressError::Io(_))
        ));
    }
}
