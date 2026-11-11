//! The desktop's pairing with a server: its device key and session token (phase-3.md 2c,
//! decision #073), kept so the desktop can unlock without signing in again.
//!
//! **Sealed** (phase-3.md 3b, #074): what's stored is a [`SealedCredential`], opened only by
//! this computer's PIN or the person's password (`crate::sealed`). A pairing stored before
//! milestone 3 (unsealed) is recognized and forgotten: the desktop signs in once more.
//!
//! **Where:** the OS keychain (macOS Keychain, Windows Credential Manager, Linux Secret
//! Service), one entry per server address. **Debug builds** use a file in the data folder
//! instead (`device-credentials.json`, readable only by you), so `dx serve` rebuilds don't set
//! off Keychain prompts; so does a computer with no keychain service (some Linux desktops),
//! with a warning in the log. Either way it's sealed.
//!
//! Never logged: the secret key and token are only ever in [`Credential`], whose `Debug`
//! shows neither.

use std::collections::BTreeMap;
use std::io;
use std::path::Path;

use domain::log_area;
use domain::secret::Secret;

pub use crate::sealed::SealedCredential;

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

/// What an entry holds, as stored: a sealed sign-in, or (from before milestone 3) something
/// else, which is forgotten.
fn parse(value: &serde_json::Value) -> Option<SealedCredential> {
    serde_json::from_value(value.clone()).ok()
}

// ---------- The file (debug builds, or no keychain) ----------

fn read_file(data_folder: &Path) -> BTreeMap<String, serde_json::Value> {
    std::fs::read_to_string(data_folder.join(CREDENTIALS_FILE_NAME))
        .ok()
        .and_then(|text| serde_json::from_str(&text).ok())
        .unwrap_or_default()
}

fn write_file(data_folder: &Path, all: &BTreeMap<String, serde_json::Value>) -> io::Result<()> {
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

/// The sealed sign-in for `server` in the file in `data_folder`, if there is one. An entry from
/// before milestone 3 (unsealed) is removed.
#[must_use]
pub fn load_from_file(data_folder: &Path, server: &str) -> Option<SealedCredential> {
    let all = read_file(data_folder);
    let value = all.get(server)?;
    let sealed = parse(value);
    if sealed.is_none() {
        tracing::info!(target: log_area::AUTH, "this device's sign-in is from before the app lock; it signs in once more");
        let _ = forget_in_file(data_folder, server);
    }
    sealed
}

/// Saves `sealed` for `server` in the file in `data_folder`, keeping the others.
///
/// # Errors
///
/// The I/O error if the file can't be written.
pub fn save_to_file(data_folder: &Path, server: &str, sealed: &SealedCredential) -> io::Result<()> {
    let mut all = read_file(data_folder);
    all.insert(
        server.to_owned(),
        serde_json::to_value(sealed).map_err(io::Error::other)?,
    );
    write_file(data_folder, &all)
}

/// Removes the sign-in for `server` from the file in `data_folder`.
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

/// This device's sealed sign-in for `server`: from the keychain, or the file (see the module
/// docs). `None` if it isn't paired with that server (or its pairing is from before milestone 3,
/// which is then forgotten).
#[must_use]
pub fn load(server: &str) -> Option<SealedCredential> {
    #[cfg(feature = "desktop")]
    if let Some(entry) = keychain_entry(server) {
        return match entry.get_secret() {
            Ok(bytes) => {
                let sealed = serde_json::from_slice::<serde_json::Value>(&bytes)
                    .ok()
                    .as_ref()
                    .and_then(parse);
                if sealed.is_none() {
                    tracing::info!(target: log_area::AUTH, "this device's sign-in is from before the app lock; it signs in once more");
                    let _ = entry.delete_credential();
                }
                sealed
            }
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

/// Keeps `sealed` for `server` (see the module docs for where).
///
/// # Errors
///
/// What went wrong storing it, as text for the log.
pub fn save(server: &str, sealed: &SealedCredential) -> Result<(), String> {
    #[cfg(feature = "desktop")]
    if let Some(entry) = keychain_entry(server) {
        let bytes = serde_json::to_vec(sealed).map_err(|error| error.to_string())?;
        return entry.set_secret(&bytes).map_err(|error| error.to_string());
    }
    if cfg!(debug_assertions) {
        tracing::debug!(target: log_area::AUTH, "debug build: keeping this device's sealed sign-in in a file, not the keychain");
    }
    let data_folder =
        crate::data_folder::platform_data_folder().ok_or("no data folder on this computer")?;
    save_to_file(&data_folder, server, sealed).map_err(|error| error.to_string())
}

/// Forgets this device's sign-in for `server` (signing out, or too many wrong tries).
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

    const SERVER: &str = "https://nas.example.ts.net";

    fn sealed(n: u8) -> SealedCredential {
        let credential = Credential {
            secret_key: [n; 32],
            token: Secret::new("ab".repeat(32)),
        };
        SealedCredential::seal(
            &credential,
            SERVER,
            &"284917".into(),
            &"a long password".into(),
        )
        .unwrap()
    }

    #[test]
    fn a_sealed_sign_in_round_trips_through_the_file_per_server() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("vehiclemanager");
        let (one, two) = (sealed(1), sealed(2));
        save_to_file(&folder, SERVER, &one).unwrap();
        save_to_file(&folder, "https://nas.example.ts.net:8443", &two).unwrap();
        assert_eq!(load_from_file(&folder, SERVER), Some(one));
        forget_in_file(&folder, SERVER).unwrap();
        assert_eq!(load_from_file(&folder, SERVER), None);
        assert_eq!(
            load_from_file(&folder, "https://nas.example.ts.net:8443"),
            Some(two),
            "the other server's is kept"
        );
    }

    #[test]
    fn a_pairing_from_before_the_app_lock_is_forgotten() {
        let temp = tempfile::tempdir().unwrap();
        // Milestone 2 stored the key and token unsealed.
        let old = format!(
            r#"{{"{SERVER}":{{"secret_key":"{}","token":"{}"}}}}"#,
            "07".repeat(32),
            "ab".repeat(32)
        );
        std::fs::write(temp.path().join(CREDENTIALS_FILE_NAME), old).unwrap();
        assert_eq!(load_from_file(temp.path(), SERVER), None);
        let left = std::fs::read_to_string(temp.path().join(CREDENTIALS_FILE_NAME)).unwrap();
        assert!(
            !left.contains(&"ab".repeat(32)),
            "the unsealed token is gone: {left}"
        );
    }

    #[cfg(unix)]
    #[test]
    fn the_file_is_readable_only_by_its_owner() {
        use std::os::unix::fs::PermissionsExt;
        let temp = tempfile::tempdir().unwrap();
        save_to_file(temp.path(), SERVER, &sealed(1)).unwrap();
        let mode = std::fs::metadata(temp.path().join(CREDENTIALS_FILE_NAME))
            .unwrap()
            .permissions()
            .mode();
        assert_eq!(mode & 0o777, 0o600);
    }

    #[test]
    fn a_credential_never_prints_its_key_or_token() {
        let credential = Credential {
            secret_key: [1; 32],
            token: Secret::new("ab".repeat(32)),
        };
        let printed = format!("{credential:?}");
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
