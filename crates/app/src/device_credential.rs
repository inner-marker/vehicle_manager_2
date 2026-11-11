//! The desktop's pairing with a server: its device key and session token (phase-3.md 2c,
//! decision #073), kept so the desktop stays signed in across restarts.
//!
//! **Where:** the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret
//! Service), one entry per server address. **Debug builds** use a file in the data folder
//! instead (`device-credentials.json`, readable only by you), so `dx serve` rebuilds don't set
//! off Keychain prompts; so does a computer with no keychain service (some Linux desktops),
//! with a warning in the log. The file is weaker: anyone who can read your files can read it
//! (milestone 3's PIN then wraps it; sub-plan 04 §6.1).
//!
//! Never logged: the secret key and token are only ever in [`Credential`], whose `Debug`
//! shows neither.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use domain::device_signature::{parse_secret_key, secret_key_to_hex};
use domain::log_area;
use domain::secret::Secret;
use serde::{Deserialize, Serialize};

/// The file the debug build (or a computer without a keychain) keeps credentials in.
pub const CREDENTIALS_FILE_NAME: &str = "device-credentials.json";

/// One pairing: this device's secret key and its session token on one server.
#[derive(Clone, PartialEq, Eq)]
pub struct Credential {
    /// The device's Ed25519 secret key (its seed).
    pub secret_key: [u8; 32],
    /// The session token the server gave it.
    pub token: Secret,
}

impl std::fmt::Debug for Credential {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("Credential([redacted])")
    }
}

/// How a credential is stored (in the keychain or the file): text only.
#[derive(Serialize, Deserialize)]
struct Stored {
    secret_key: Secret,
    token: Secret,
}

impl Stored {
    fn from_credential(credential: &Credential) -> Self {
        Self {
            secret_key: Secret::new(secret_key_to_hex(&credential.secret_key)),
            token: credential.token.clone(),
        }
    }

    fn into_credential(self) -> Option<Credential> {
        Some(Credential {
            secret_key: parse_secret_key(self.secret_key.expose())?,
            token: self.token,
        })
    }
}

// ---------- The file (debug builds, or no keychain) ----------

fn read_file(data_folder: &Path) -> BTreeMap<String, Stored> {
    std::fs::read_to_string(data_folder.join(CREDENTIALS_FILE_NAME))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_file(data_folder: &Path, all: &BTreeMap<String, Stored>) -> io::Result<()> {
    std::fs::create_dir_all(data_folder)?;
    let text = serde_json::to_string(all).map_err(io::Error::other)?;
    let temporary = data_folder.join(format!("{CREDENTIALS_FILE_NAME}.tmp"));
    write_private(&temporary, text.as_bytes())?;
    std::fs::rename(&temporary, data_folder.join(CREDENTIALS_FILE_NAME))
}

/// Writes `bytes` to a new file only its owner can read and write.
fn write_private(path: &Path, bytes: &[u8]) -> io::Result<()> {
    use std::io::Write;
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create(true).truncate(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    options.open(path)?.write_all(bytes)
}

/// The credential for `server` in the file in `data_folder`, if there is one.
#[must_use]
pub fn load_from_file(data_folder: &Path, server: &str) -> Option<Credential> {
    read_file(data_folder).remove(server)?.into_credential()
}

/// Saves `credential` for `server` in the file in `data_folder`, keeping the others.
///
/// # Errors
///
/// The I/O error if the file can't be written.
pub fn save_to_file(data_folder: &Path, server: &str, credential: &Credential) -> io::Result<()> {
    let mut all = read_file(data_folder);
    all.insert(server.to_owned(), Stored::from_credential(credential));
    write_file(data_folder, &all)
}

/// Removes the credential for `server` from the file in `data_folder`.
///
/// # Errors
///
/// The I/O error if the file can't be written.
pub fn forget_in_file(data_folder: &Path, server: &str) -> io::Result<()> {
    let mut all = read_file(data_folder);
    if all.remove(server).is_some() {
        write_file(data_folder, &all)?;
    }
    Ok(())
}

// ---------- The keychain (release desktop builds) ----------

/// The keychain entry for `server`, if the keychain can be used here.
#[cfg(feature = "desktop")]
fn keychain_entry(server: &str) -> Option<keyring::Entry> {
    if cfg!(debug_assertions) {
        return None;
    }
    match keyring::Entry::new(domain::identity::BUNDLE_ID, &format!("device@{server}")) {
        Ok(entry) => Some(entry),
        Err(error) => {
            tracing::warn!(target: log_area::AUTH, %error, "no keychain here; keeping this device's sign-in in a file instead");
            None
        }
    }
}

// ---------- Where it lives ----------

/// This device's credential for `server`: from the keychain, or the file (see the module
/// docs). `None` if it isn't paired with that server.
#[must_use]
pub fn load(server: &str) -> Option<Credential> {
    #[cfg(feature = "desktop")]
    if let Some(entry) = keychain_entry(server) {
        return match entry.get_secret() {
            Ok(bytes) => serde_json::from_slice::<Stored>(&bytes)
                .ok()
                .and_then(Stored::into_credential),
            Err(keyring::Error::NoEntry) => None,
            Err(error) => {
                tracing::warn!(target: log_area::AUTH, %error, "can't read this device's sign-in from the keychain");
                None
            }
        };
    }
    let data_folder = crate::data_folder::platform_data_folder()?;
    load_from_file(&data_folder, server)
}

/// Keeps `credential` for `server` (see the module docs for where).
///
/// # Errors
///
/// What went wrong storing it, as text for the log.
pub fn save(server: &str, credential: &Credential) -> Result<(), String> {
    #[cfg(feature = "desktop")]
    if let Some(entry) = keychain_entry(server) {
        let bytes = serde_json::to_vec(&Stored::from_credential(credential))
            .map_err(|error| error.to_string())?;
        return entry.set_secret(&bytes).map_err(|error| error.to_string());
    }
    if cfg!(debug_assertions) {
        tracing::warn!(target: log_area::AUTH, "debug build: keeping this device's sign-in in a file, not the keychain");
    }
    let data_folder =
        crate::data_folder::platform_data_folder().ok_or("no data folder on this computer")?;
    save_to_file(&data_folder, server, credential).map_err(|error| error.to_string())
}

/// Forgets this device's credential for `server` (signing out).
pub fn forget(server: &str) {
    #[cfg(feature = "desktop")]
    if let Some(entry) = keychain_entry(server) {
        match entry.delete_credential() {
            Ok(()) | Err(keyring::Error::NoEntry) => {}
            Err(error) => {
                tracing::warn!(target: log_area::AUTH, %error, "can't remove this device's sign-in from the keychain");
            }
        }
        return;
    }
    if let Some(data_folder) = crate::data_folder::platform_data_folder()
        && let Err(error) = forget_in_file(&data_folder, server)
    {
        tracing::warn!(target: log_area::AUTH, %error, "can't remove this device's sign-in from its file");
    }
}

/// A new device key: 32 random bytes from the operating system.
///
/// # Errors
///
/// What went wrong, if the OS has no randomness.
pub fn new_secret_key() -> Result<[u8; 32], String> {
    let mut key = [0; 32];
    getrandom::fill(&mut key).map_err(|error| error.to_string())?;
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn credential(n: u8) -> Credential {
        Credential {
            secret_key: [n; 32],
            token: Secret::new("ab".repeat(32)),
        }
    }

    #[test]
    fn a_credential_round_trips_through_the_file_per_server() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("vehiclemanager");
        save_to_file(&folder, "https://nas.example.ts.net", &credential(1)).unwrap();
        save_to_file(&folder, "https://nas.example.ts.net:8443", &credential(2)).unwrap();
        assert_eq!(
            load_from_file(&folder, "https://nas.example.ts.net"),
            Some(credential(1))
        );
        forget_in_file(&folder, "https://nas.example.ts.net").unwrap();
        assert_eq!(load_from_file(&folder, "https://nas.example.ts.net"), None);
        assert_eq!(
            load_from_file(&folder, "https://nas.example.ts.net:8443"),
            Some(credential(2)),
            "the other server's is kept"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        save_to_file(temp.path(), "https://nas.example.ts.net", &credential(1)).unwrap();
        let mode = std::fs::metadata(temp.path().join(CREDENTIALS_FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_credential_never_prints_its_key_or_token() {
        let printed = format!("{:?}", credential(1));
        assert!(
            !printed.contains("abab") && !printed.contains("0101"),
            "{printed}"
        );
    }

    #[test]
    fn new_keys_are_random() {
        assert_ne!(new_secret_key().unwrap(), new_secret_key().unwrap());
    }
}
