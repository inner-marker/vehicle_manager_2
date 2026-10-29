//! The client ↔ server API contract: version, headers, handshake, and refusal reasons.
//!
//! The **API version** is separate from the app version (plan §8): it changes only
//! when the contract between app and server changes. Every request to a path under
//! [`API_PATH_PREFIX`] carries the client's API version in [`API_VERSION_HEADER`]
//! and its credential in the standard `Authorization: Bearer <credential>` header.
//! The server checks both before any server function runs (phase-1.md step 7).
//!
//! # Examples
//!
//! ```
//! use domain::api::{API_VERSION, ApiError, check_client_api_version};
//!
//! assert_eq!(check_client_api_version(API_VERSION), Ok(()));
//! assert_eq!(check_client_api_version(API_VERSION + 1), Err(ApiError::ClientTooNew));
//! ```

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, messages};

/// The API version this build speaks.
pub const API_VERSION: u32 = 1;

/// The oldest client API version this build's server still accepts.
pub const MIN_SUPPORTED_API_VERSION: u32 = 1;

/// Header carrying the client's API version (a plain integer, e.g. `1`).
pub const API_VERSION_HEADER: &str = "x-api-version";

/// Every server function lives under this path, and the auth layer guards all of it.
pub const API_PATH_PREFIX: &str = "/api/";

/// Why the server refused a request. Sent as a kebab-case string (e.g.
/// `"invalid-credential"`) so the client can show a translated message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiError {
    /// No `Authorization: Bearer ...` header.
    MissingCredential,
    /// A credential was sent but isn't the one the server expects.
    InvalidCredential,
    /// The server has no credential configured, so it refuses everything (fail closed).
    AuthNotConfigured,
    /// No API version header.
    MissingApiVersion,
    /// The API version header isn't a whole number.
    MalformedApiVersion,
    /// The client's API version is older than the server still supports: update the app.
    ClientTooOld,
    /// The client's API version is newer than the server's: update the server first.
    ClientTooNew,
}

impl ApiError {
    /// Every reason, for exhaustive tests and the UI.
    pub const ALL: [Self; 7] = [
        Self::MissingCredential,
        Self::InvalidCredential,
        Self::AuthNotConfigured,
        Self::MissingApiVersion,
        Self::MalformedApiVersion,
        Self::ClientTooOld,
        Self::ClientTooNew,
    ];

    /// The HTTP status the server responds with.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::MissingCredential | Self::InvalidCredential => 401,
            Self::AuthNotConfigured => 503,
            Self::MissingApiVersion | Self::MalformedApiVersion | Self::ClientTooNew => 400,
            // 426 Upgrade Required: the client must update to talk to this server.
            Self::ClientTooOld => 426,
        }
    }

    /// The user-facing explanation, through the translation layer.
    pub const fn message(self) -> MessageId {
        match self {
            Self::MissingCredential => messages::API_ERROR_MISSING_CREDENTIAL,
            Self::InvalidCredential => messages::API_ERROR_INVALID_CREDENTIAL,
            Self::AuthNotConfigured => messages::API_ERROR_AUTH_NOT_CONFIGURED,
            Self::MissingApiVersion => messages::API_ERROR_MISSING_API_VERSION,
            Self::MalformedApiVersion => messages::API_ERROR_MALFORMED_API_VERSION,
            Self::ClientTooOld => messages::API_ERROR_CLIENT_TOO_OLD,
            Self::ClientTooNew => messages::API_ERROR_CLIENT_TOO_NEW,
        }
    }

    /// Whether this is a credential problem (as opposed to a version problem).
    pub const fn is_auth_failure(self) -> bool {
        matches!(
            self,
            Self::MissingCredential | Self::InvalidCredential | Self::AuthNotConfigured
        )
    }

    /// A short English description for logs and the response's `message` field.
    /// Never shown in the UI, which uses [`ApiError::message`].
    pub const fn description(self) -> &'static str {
        match self {
            Self::MissingCredential => "missing credential",
            Self::InvalidCredential => "invalid credential",
            Self::AuthNotConfigured => "server has no credential configured",
            Self::MissingApiVersion => "missing API version header",
            Self::MalformedApiVersion => "malformed API version header",
            Self::ClientTooOld => "client API version is too old",
            Self::ClientTooNew => "client API version is newer than the server",
        }
    }
}

/// Whether a client speaking `client_version` may talk to this server.
///
/// # Errors
///
/// [`ApiError::ClientTooOld`] below [`MIN_SUPPORTED_API_VERSION`];
/// [`ApiError::ClientTooNew`] above [`API_VERSION`] (the server must be upgraded first).
pub const fn check_client_api_version(client_version: u32) -> Result<(), ApiError> {
    if client_version < MIN_SUPPORTED_API_VERSION {
        Err(ApiError::ClientTooOld)
    } else if client_version > API_VERSION {
        Err(ApiError::ClientTooNew)
    } else {
        Ok(())
    }
}

/// Parses the [`API_VERSION_HEADER`] value.
///
/// # Errors
///
/// [`ApiError::MalformedApiVersion`] if it isn't a whole, non-negative number.
pub fn parse_api_version(header_value: &str) -> Result<u32, ApiError> {
    header_value
        .trim()
        .parse()
        .map_err(|_| ApiError::MalformedApiVersion)
}

/// What the server reports about itself in the handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    /// The server's app version (e.g. `0.1.0`).
    pub app_version: String,
    /// The API version the server speaks.
    pub api_version: u32,
    /// The oldest client API version the server accepts.
    pub min_supported_api_version: u32,
    /// The server's clock, as Unix epoch milliseconds (UTC).
    pub server_time_ms: i64,
}

/// The server's reply to a ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The server's clock, as Unix epoch milliseconds (UTC).
    pub server_time_ms: i64,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::i18n::Translator;

    #[test]
    fn api_version_check_accepts_supported_range() {
        for version in MIN_SUPPORTED_API_VERSION..=API_VERSION {
            assert_eq!(check_client_api_version(version), Ok(()));
        }
    }

    #[test]
    fn client_older_than_minimum_is_too_old() {
        assert_eq!(
            check_client_api_version(MIN_SUPPORTED_API_VERSION - 1),
            Err(ApiError::ClientTooOld)
        );
    }

    #[test]
    fn client_newer_than_server_is_too_new() {
        assert_eq!(
            check_client_api_version(API_VERSION + 1),
            Err(ApiError::ClientTooNew)
        );
    }

    #[test]
    fn api_version_header_parses_whole_numbers_only() {
        assert_eq!(parse_api_version("1"), Ok(1));
        assert_eq!(parse_api_version(" 2 "), Ok(2));
        for bad in ["", "v1", "1.0", "-1", "one"] {
            assert_eq!(
                parse_api_version(bad),
                Err(ApiError::MalformedApiVersion),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn every_api_error_has_a_status_and_message() {
        let english = Translator::english();
        for error in ApiError::ALL {
            assert!((400..600).contains(&error.http_status()), "{error:?}");
            assert!(english.has(error.message()), "{error:?}");
            assert!(!error.description().is_empty());
        }
    }

    #[test]
    fn api_errors_serialize_as_kebab_case_codes() {
        assert_eq!(
            serde_json::to_string(&ApiError::InvalidCredential)
                .ok()
                .as_deref(),
            Some(r#""invalid-credential""#)
        );
        assert_eq!(
            serde_json::from_str::<ApiError>(r#""client-too-old""#).ok(),
            Some(ApiError::ClientTooOld)
        );
    }

    #[test]
    fn only_credential_problems_count_as_auth_failures() {
        let auth: Vec<ApiError> = ApiError::ALL
            .into_iter()
            .filter(|e| e.is_auth_failure())
            .collect();
        assert_eq!(
            auth,
            [
                ApiError::MissingCredential,
                ApiError::InvalidCredential,
                ApiError::AuthNotConfigured
            ]
        );
    }
}
