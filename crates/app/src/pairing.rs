//! Pairing a desktop with its server (phase-3.md 2c, decision #073): the screens' side.
//!
//! Signing in on a desktop pairs it: [`new_pairing`] makes a device key and names the device
//! (the computer's name), the sign-in sends the public half, and [`finish_sign_in`] keeps the
//! key and token (in memory for this run, and in the keychain for the next: see
//! `crate::device_credential`). At launch, [`restore`] picks them up again, so the desktop
//! stays signed in. [`forget`] undoes it (signing out, or the server no longer accepts it).
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
/// too, for this run and the next.
pub fn finish_sign_in(token: Option<Secret>, key: Option<[u8; 32]>) {
    let Some(token) = token else {
        return;
    };
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if let Some(secret_key) = key {
        let credential = crate::device_credential::Credential {
            secret_key,
            token: token.clone(),
        };
        match crate::device_credential::save(server(), &credential) {
            Ok(()) => {
                tracing::info!(target: domain::log_area::AUTH, "paired; this device stays signed in");
            }
            Err(error) => {
                tracing::warn!(target: domain::log_area::AUTH, %error, "paired, but couldn't keep the sign-in; it lasts until the app quits");
            }
        }
    }
    // The token first, so the key's first signature is over it.
    client::set_session_token(Some(token));
    client::set_device_key(key);
}

/// At launch: picks up this desktop's pairing with its server, if it has one. Returns whether
/// it did (the session check then confirms the server still accepts it).
pub fn restore() -> bool {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if let Some(credential) = crate::device_credential::load(server()) {
        client::set_session_token(Some(credential.token));
        client::set_device_key(Some(credential.secret_key));
        tracing::info!(target: domain::log_area::AUTH, "restored this device's sign-in");
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
