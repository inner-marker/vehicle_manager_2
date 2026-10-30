//! What every server-function call carries, set in one place (web, desktop, and mobile).
//!
//! - **The API version** (`x-api-version`), which the server checks on every `/api/` request.
//! - **The desktop app's session token** (`Authorization: Bearer`), held in memory only
//!   (decision #027): signing in sets it, signing out or a restart clears it. The web app
//!   never has one: its session travels in an `HttpOnly` cookie its scripts can't read.
//!
//! Dioxus keeps one set of extra headers for all server-function calls, so
//! [`apply_request_headers`] runs at startup and whenever the token changes.

use std::sync::Mutex;

use dioxus::prelude::dioxus_fullstack::{HeaderMap, HeaderValue, set_request_headers};
use domain::api::{API_VERSION, API_VERSION_HEADER};

/// The desktop app's session token, while signed in.
static SESSION_TOKEN: Mutex<Option<String>> = Mutex::new(None);

/// Whether this build gets a session token from sign-in (and sends it) rather than a cookie.
pub const USES_SESSION_TOKEN: bool = cfg!(any(feature = "desktop", feature = "mobile"));

/// Remembers (or, with `None`, forgets) the session token, and updates the request headers.
pub fn set_session_token(token: Option<String>) {
    *SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = token;
    apply_request_headers();
}

/// Sets the headers every server-function call carries, from the current token.
pub fn apply_request_headers() {
    let token = SESSION_TOKEN
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
        .clone();
    set_request_headers(request_headers(token.as_deref()));
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
            Some("1")
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
