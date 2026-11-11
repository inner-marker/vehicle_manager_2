//! Pairing a desktop with its server (phase-3.md 2c, decision #073): the screens' side.
//!
//! Signing in on a desktop pairs it: [`new_pairing`] makes a device key and names the device
//! (the computer's name), the sign-in sends the public half, and [`finish_sign_in`] keeps the
//! key and token for this run. They're kept for the next run sealed with a PIN (and the
//! password; `crate::sealed`, phase-3.md 3b), opened on the lock screen (3c). [`forget`] undoes
//! it (signing out, or the server no longer accepts it).
//! Browsers never pair: these do nothing there but keep the token.

use domain::api::Pairing;
use domain::secret::Secret;

use crate::client;

/// A new pairing for a sign-in from this app: on a desktop, a new device key and what to send
/// the server; elsewhere `None`.
#[must_use]
pub fn new_pairing() -> Option<([u8; 32], Pairing)> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        use domain::device_signature::{public_key_of, public_key_to_hex};

        let key = match crate::device_credential::new_secret_key() {
            Ok(key) => key,
            Err(error) => {
                tracing::error!(target: domain::log_area::AUTH, %error, "can't make a device key; signing in without pairing");
                return None;
            }
        };
        let name = gethostname::gethostname().to_string_lossy().into_owned();
        Some((
            key,
            Pairing {
                public_key: public_key_to_hex(&public_key_of(&key)),
                name: Some(name),
            },
        ))
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    None
}

/// The server this desktop is pointed at, as its credentials are filed under.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
fn server() -> &'static str {
    dioxus::prelude::dioxus_fullstack::get_server_url()
}

/// After a successful sign-in: keeps `token`, and with `key` (a desktop that paired) the key
/// too, for this run. Keeping them for the next run means sealing them with a PIN, which the
/// "Choose a PIN" screen does (phase-3.md 3c, #074).
pub fn finish_sign_in(token: Option<Secret>, key: Option<[u8; 32]>) {
    let Some(token) = token else {
        return;
    };
    // The token first, so the key's first signature is over it.
    client::set_session_token(Some(token));
    client::set_device_key(key);
}

/// At launch: whether this desktop has a sealed sign-in for its server. It's opened by the PIN
/// or password on the lock screen (phase-3.md 3c); until then nothing is in memory.
#[must_use]
pub fn has_sealed_sign_in() -> bool {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if crate::device_credential::load(server()).is_some() {
        return true;
    }
    false
}

/// Forgets the token and, on a desktop, the pairing (here and in the keychain).
pub fn forget() {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if client::has_device_key() {
        crate::device_credential::forget(server());
    }
    client::set_device_key(None);
    client::set_session_token(None);
}
