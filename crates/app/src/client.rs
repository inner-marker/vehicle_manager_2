//! What every server-function call carries, set in one place (web, desktop, and mobile).
//!
//! - **The API version** (`x-api-version`), which the server checks on every `/api/` request.
//! - **The desktop app's session token** (`Authorization: Bearer`), held in memory only
//!   (decision #027): signing in sets it, signing out or a restart clears it. The web app
//!   never has one: its session travels in an `HttpOnly` cookie its scripts can't read.
//!
//! - **A paired desktop's signature** (`x-device-signature`; phase-3.md 2c, #073): Dioxus keeps
//!   one set of extra headers for all server-function calls, with no per-request hook, so they
//!   carry a **session** signature, renewed every 30 seconds ([`apply_request_headers`], called
//!   by the desktop's renewal loop); the app's own requests (files) get a **request**
//!   signature each ([`current_headers_for`]). Signatures use the server's time (this
//!   computer's plus the offset the handshake showed), so a wrong clock still works.
//!
//! [`apply_request_headers`] runs at startup, whenever the token or key changes, and on the
//! desktop every 30 seconds.

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicI64, Ordering};

use dioxus::prelude::dioxus_fullstack::{HeaderMap, HeaderValue, set_request_headers};
use domain::api::{API_VERSION, API_VERSION_HEADER};
use domain::device_signature::{self, DeviceSignature, Scope};
use domain::secret::Secret;
use domain::time::{Clock, UnixMillis};

/// The desktop app's session token, while signed in.
static SESSION_TOKEN: Mutex<Option<Secret>> = Mutex::new(None);

/// A paired desktop's device key (its secret seed), while signed in.
static DEVICE_KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);

/// The server's clock minus this computer's, in milliseconds, from the last handshake.
static SERVER_OFFSET_MILLIS: AtomicI64 = AtomicI64::new(0);

/// Whether [`SERVER_OFFSET_MILLIS`] has been measured yet.
static SERVER_OFFSET_KNOWN: AtomicBool = AtomicBool::new(false);

/// Whether this build gets a session token from sign-in (and sends it) rather than a cookie.
pub const USES_SESSION_TOKEN: bool = cfg!(any(feature = "desktop", feature = "mobile"));

/// Remembers (or, with `None`, forgets) the session token, and updates the request headers.
pub fn set_session_token(token: Option<Secret>) {
    *SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = token;
    apply_request_headers();
}

/// Remembers (or forgets) a paired desktop's device key, and updates the request headers.
pub fn set_device_key(key: Option<[u8; 32]>) {
    *DEVICE_KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = key;
    apply_request_headers();
}

/// The device key held now (a paired desktop, signed in), to seal it with a PIN.
#[must_use]
pub fn device_key() -> Option<[u8; 32]> {
    *DEVICE_KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The session token held now (the desktop), to seal it with a PIN.
#[must_use]
pub fn session_token() -> Option<Secret> {
    SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()
}

/// Whether this app holds a device key (a paired desktop, signed in).
#[must_use]
pub fn has_device_key() -> bool {
    DEVICE_KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .is_some()
}

/// Measures the server's clock against this computer's, from a handshake's `server_time_ms`.
pub fn set_server_time(server_time_ms: i64) {
    let local = crate::logging::SystemClock.now().as_millis();
    SERVER_OFFSET_MILLIS.store(server_time_ms - local, Ordering::Relaxed);
    SERVER_OFFSET_KNOWN.store(true, Ordering::Relaxed);
}

/// Whether the server's clock has been measured since the app started.
#[must_use]
pub fn server_time_known() -> bool {
    SERVER_OFFSET_KNOWN.load(Ordering::Relaxed)
}

/// Now, on the server's clock.
fn server_now() -> UnixMillis {
    let local = crate::logging::SystemClock.now().as_millis();
    UnixMillis::from_millis(local + SERVER_OFFSET_MILLIS.load(Ordering::Relaxed))
}

/// A random nonce (native only: the web app never signs).
#[cfg(not(target_arch = "wasm32"))]
fn nonce() -> Option<[u8; 16]> {
    let mut nonce = [0; 16];
    getrandom::fill(&mut nonce).ok()?;
    Some(nonce)
}

#[cfg(target_arch = "wasm32")]
fn nonce() -> Option<[u8; 16]> {
    None
}

/// The signature header for `scope` (and, for a request, its method and path), if this app
/// holds a device key and a token.
fn signature_header(scope: Scope, method: &str, path_and_query: &str) -> Option<HeaderValue> {
    let key = (*DEVICE_KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner))?;
    let token = SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone()?;
    let token_hash = device_signature::token_hash(token.expose())?;
    let signature = DeviceSignature::sign(
        &key,
        scope,
        method,
        path_and_query,
        server_now(),
        nonce()?,
        &token_hash,
    );
    HeaderValue::from_str(&signature.to_header()).ok()
}

/// Sets the headers every server-function call carries: the API version, the token, and a
/// paired desktop's session signature (renewed by calling this again).
pub fn apply_request_headers() {
    let token = SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let mut headers = request_headers(token.as_ref().map(Secret::expose));
    if let Some(signature) = signature_header(Scope::Session, "", "") {
        headers.insert(device_signature::HEADER, signature);
    }
    set_request_headers(headers);
}

/// The headers one of the app's own requests (the file routes, which aren't server functions)
/// carries: as [`apply_request_headers`], but with a signature for this request alone.
#[must_use]
pub fn current_headers_for(method: &str, path_and_query: &str) -> HeaderMap {
    let token = SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    let mut headers = request_headers(token.as_ref().map(Secret::expose));
    if let Some(signature) = signature_header(Scope::Request, method, path_and_query) {
        headers.insert(device_signature::HEADER, signature);
    }
    headers
}

/// The API version header, plus `Authorization: Bearer` when there's a token. A token with
/// characters a header can't carry is left out (the server then says you're not signed in).
#[must_use]
pub fn request_headers(session_token: Option<&str>) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(API_VERSION_HEADER, HeaderValue::from(API_VERSION));
    if let Some(Ok(value)) =
        session_token.map(|token| HeaderValue::from_str(&format!("Bearer {token}")))
    {
        headers.insert("authorization", value);
    }
    headers
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn requests_carry_the_api_version_and_a_session_token_when_there_is_one() {
        let web = request_headers(None);
        assert_eq!(
            web.get(API_VERSION_HEADER).and_then(|v| v.to_str().ok()),
            Some(API_VERSION.to_string().as_str())
        );
        assert!(
            web.get("authorization").is_none(),
            "the web app uses its cookie"
        );

        let desktop = request_headers(Some("abc123"));
        assert_eq!(
            desktop.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer abc123")
        );
    }
}
