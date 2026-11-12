//! The desktop's stored sign-in, sealed with its PIN and its password (phase-3.md 3b,
//! decision #074; sub-plan 04 §4.1).
//!
//! The device key, session token, and the key of this computer's copy of the server's records
//! (`crate::device_credential::Credential`; the copy is milestone 4's, #075) are stored as
//! **two sealed copies**: one opened by the PIN, one by the password, so either unlocks the
//! desktop with no connection. Each copy has its own random salt and nonce:
//!
//! - **The key:** Argon2id over the PIN or password and the salt ([`KDF_MEMORY_KIB`] of memory,
//!   [`KDF_PASSES`] passes), so every guess costs real time even on a copy of the file.
//! - **The seal:** XChaCha20-Poly1305 over the sign-in, with the server's address as associated
//!   data, so a sealed sign-in can't be moved to another server's entry.
//!
//! A wrong PIN or password, or a changed byte, opens nothing. The wrong-try count
//! (`domain::pin::WrongTries`) is kept beside the seals, unsealed.

use argon2::{Algorithm, Argon2, Params, Version};
use chacha20poly1305::aead::{Aead, KeyInit, Payload};
use chacha20poly1305::{XChaCha20Poly1305, XNonce};
use domain::pin::WrongTries;
use domain::secret::Secret;
use serde::{Deserialize, Serialize};

use crate::device_credential::Credential;

/// The format's version (a new one is a new kind of seal). 3 added the copy's key (phase-3.md
/// 4c); an older seal is forgotten, and the desktop signs in once more.
const VERSION: u32 = 3;

/// Argon2id memory, in KiB: 256 MiB. With [`KDF_PASSES`], one unlock takes about 0.3 s on an
/// Apple M-series Mac (measured 2026-11-11), so trying every 6-digit PIN on a copy of the file
/// takes days on one core, and each try needs 256 MiB, which limits running many at once.
#[cfg(not(test))]
pub const KDF_MEMORY_KIB: u32 = 256 * 1024;

/// Tests use a small cost, so they stay quick; the format is the same.
#[cfg(test)]
pub const KDF_MEMORY_KIB: u32 = 1024;

/// Argon2id passes.
pub const KDF_PASSES: u32 = 3;

/// One sealed copy: its salt, nonce, and the sealed bytes (hex).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Seal {
    salt: String,
    nonce: String,
    sealed: String,
}

/// The stored sign-in: one copy for the PIN, one for the password, and the wrong tries.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SealedCredential {
    version: u32,
    pin: Seal,
    password: Seal,
    /// Wrong PINs and passwords since the last unlock.
    pub tries: WrongTries,
    /// Whose sign-in it is (their display name), for the lock screen. Not sealed: it says only
    /// whose computer this is.
    #[serde(default)]
    pub person: Option<String>,
    /// Their username, to open the app as them with no connection (phase-3.md 4e). Not sealed,
    /// like `person`.
    #[serde(default)]
    pub username: Option<String>,
    /// When the last wrong try was (Unix milliseconds), for the report after unlocking
    /// (phase-3.md 3d).
    #[serde(default)]
    pub last_wrong_ms: Option<i64>,
}

/// What's sealed: the keys and token as text.
#[derive(Serialize, Deserialize)]
struct Plain {
    secret_key: Secret,
    token: Secret,
    database_key: Secret,
}

fn to_hex(bytes: &[u8]) -> String {
    use std::fmt::Write;
    bytes.iter().fold(String::new(), |mut text, byte| {
        let _ = write!(text, "{byte:02x}");
        text
    })
}

fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) || !text.is_ascii() {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).ok())
        .collect()
}

fn random<const N: usize>() -> Result<[u8; N], String> {
    let mut bytes = [0; N];
    getrandom::fill(&mut bytes).map_err(|error| error.to_string())?;
    Ok(bytes)
}

/// The 32-byte key made from `secret` and `salt` (Argon2id).
fn derive(secret: &Secret, salt: &[u8]) -> Option<[u8; 32]> {
    let params = Params::new(KDF_MEMORY_KIB, KDF_PASSES, 1, Some(32)).ok()?;
    let mut key = [0; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, params)
        .hash_password_into(secret.expose().as_bytes(), salt, &mut key)
        .ok()?;
    Some(key)
}

fn seal_with(secret: &Secret, plain: &[u8], server: &str) -> Result<Seal, String> {
    let salt: [u8; 16] = random()?;
    let nonce: [u8; 24] = random()?;
    let key = derive(secret, &salt).ok_or("can't make the key")?;
    let cipher = XChaCha20Poly1305::new(&key.into());
    let sealed = cipher
        .encrypt(
            &XNonce::from(nonce),
            Payload {
                msg: plain,
                aad: server.as_bytes(),
            },
        )
        .map_err(|_| "can't seal".to_owned())?;
    Ok(Seal {
        salt: to_hex(&salt),
        nonce: to_hex(&nonce),
        sealed: to_hex(&sealed),
    })
}

fn open_with(seal: &Seal, secret: &Secret, server: &str) -> Option<Credential> {
    let salt = from_hex(&seal.salt)?;
    let nonce: [u8; 24] = from_hex(&seal.nonce)?.try_into().ok()?;
    let sealed = from_hex(&seal.sealed)?;
    let key = derive(secret, &salt)?;
    let plain = XChaCha20Poly1305::new(&key.into())
        .decrypt(
            &XNonce::from(nonce),
            Payload {
                msg: &sealed,
                aad: server.as_bytes(),
            },
        )
        .ok()?;
    let plain: Plain = serde_json::from_slice(&plain).ok()?;
    Some(Credential {
        secret_key: domain::device_signature::parse_secret_key(plain.secret_key.expose())?,
        token: plain.token,
        database_key: from_hex(plain.database_key.expose())?.try_into().ok()?,
    })
}

fn plain_bytes(credential: &Credential) -> Result<Vec<u8>, String> {
    serde_json::to_vec(&Plain {
        secret_key: Secret::new(domain::device_signature::secret_key_to_hex(
            &credential.secret_key,
        )),
        token: credential.token.clone(),
        database_key: Secret::new(to_hex(&credential.database_key)),
    })
    .map_err(|error| error.to_string())
}

impl SealedCredential {
    /// Seals `credential` for `server` with `pin` and `password` (no wrong tries yet).
    ///
    /// # Errors
    ///
    /// What went wrong, if the OS has no randomness or sealing fails.
    pub fn seal(
        credential: &Credential,
        server: &str,
        pin: &Secret,
        password: &Secret,
    ) -> Result<Self, String> {
        let plain = plain_bytes(credential)?;
        Ok(Self {
            version: VERSION,
            pin: seal_with(pin, &plain, server)?,
            password: seal_with(password, &plain, server)?,
            tries: WrongTries::default(),
            person: None,
            username: None,
            last_wrong_ms: None,
        })
    }

    /// Whether this is the current format (an older one is forgotten: the desktop signs in once
    /// more).
    #[must_use]
    pub const fn is_current(&self) -> bool {
        self.version == VERSION
    }

    /// The sign-in, if `pin` is this computer's PIN for `server`.
    #[must_use]
    pub fn open_with_pin(&self, server: &str, pin: &Secret) -> Option<Credential> {
        (self.version == VERSION)
            .then(|| open_with(&self.pin, pin, server))
            .flatten()
    }

    /// The sign-in, if `password` is the one it was sealed with.
    #[must_use]
    pub fn open_with_password(&self, server: &str, password: &Secret) -> Option<Credential> {
        (self.version == VERSION)
            .then(|| open_with(&self.password, password, server))
            .flatten()
    }

    /// This sign-in with a new PIN (the password's copy is kept as it is). `credential` is the
    /// sign-in, already opened.
    ///
    /// # Errors
    ///
    /// What went wrong sealing.
    pub fn with_new_pin(
        &self,
        credential: &Credential,
        server: &str,
        pin: &Secret,
    ) -> Result<Self, String> {
        Ok(Self {
            pin: seal_with(pin, &plain_bytes(credential)?, server)?,
            tries: WrongTries::default(),
            last_wrong_ms: None,
            ..self.clone()
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SERVER: &str = "https://nas.example.ts.net:8443";

    fn credential() -> Credential {
        Credential {
            secret_key: [7; 32],
            token: Secret::new("ab".repeat(32)),
            database_key: [9; 32],
        }
    }

    fn sealed() -> SealedCredential {
        SealedCredential::seal(
            &credential(),
            SERVER,
            &Secret::new("284917"),
            &Secret::new("a long password"),
        )
        .unwrap()
    }

    #[test]
    fn the_pin_or_the_password_opens_it_and_nothing_else_does() {
        let sealed = sealed();
        assert_eq!(
            sealed.open_with_pin(SERVER, &"284917".into()),
            Some(credential())
        );
        assert_eq!(
            sealed.open_with_password(SERVER, &"a long password".into()),
            Some(credential())
        );
        assert_eq!(sealed.open_with_pin(SERVER, &"284918".into()), None);
        assert_eq!(
            sealed.open_with_password(SERVER, &"a long passwork".into()),
            None
        );
        assert_eq!(
            sealed.open_with_pin(SERVER, &"a long password".into()),
            None,
            "the password doesn't open the PIN's copy"
        );
        assert_eq!(
            sealed.open_with_pin("https://other.ts.net", &"284917".into()),
            None,
            "it belongs to its server"
        );
    }

    #[test]
    fn a_changed_byte_opens_nothing_and_nothing_readable_is_stored() {
        let sealed = sealed();
        let mut changed = sealed.clone();
        let flip = if changed.pin.sealed.starts_with('0') {
            "1"
        } else {
            "0"
        };
        changed.pin.sealed.replace_range(0..1, flip);
        assert_eq!(changed.open_with_pin(SERVER, &"284917".into()), None);
        let stored = serde_json::to_string(&sealed).unwrap();
        assert!(
            !stored.contains(&"ab".repeat(32)),
            "the token isn't readable"
        );
        assert!(!stored.contains("0707070707"), "the key isn't readable");
        assert!(
            !stored.contains("0909090909"),
            "the copy's key isn't readable"
        );
        assert!(!stored.contains("284917") && !stored.contains("long password"));
        let back: SealedCredential = serde_json::from_str(&stored).unwrap();
        assert_eq!(
            back.open_with_pin(SERVER, &"284917".into()),
            Some(credential())
        );
    }

    #[test]
    fn each_seal_is_different_even_for_the_same_pin() {
        let (a, b) = (sealed(), sealed());
        assert_ne!(a.pin.salt, b.pin.salt);
        assert_ne!(a.pin.sealed, b.pin.sealed);
    }

    #[test]
    fn a_new_pin_replaces_the_old_and_keeps_the_password() {
        let mut tried = sealed();
        tried.tries = tried.tries.wrong_pin();
        let renewed = tried
            .with_new_pin(&credential(), SERVER, &"519203".into())
            .unwrap();
        assert_eq!(
            renewed.open_with_pin(SERVER, &"519203".into()),
            Some(credential())
        );
        assert_eq!(renewed.open_with_pin(SERVER, &"284917".into()), None);
        assert_eq!(
            renewed.open_with_password(SERVER, &"a long password".into()),
            Some(credential())
        );
        assert_eq!(
            renewed.tries,
            WrongTries::default(),
            "a new PIN clears the count"
        );
    }
}
