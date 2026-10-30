//! The client ↔ server API contract: version, headers, handshake, and refusal reasons.
//!
//! The **API version** is separate from the app version (plan §8): it changes only
//! when the contract between app and server changes. Every request to a path under
//! [`API_PATH_PREFIX`] carries the client's API version in [`API_VERSION_HEADER`]
//! and is made by a signed-in session: the session cookie (web) or `Authorization: Bearer
//! <session token>` (desktop).
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
    /// No session was sent: not signed in.
    MissingCredential,
    /// A session was sent but isn't valid (ended, signed out, or unknown).
    InvalidCredential,
    /// The server can't check sessions (its database isn't open), so it refuses (fail closed).
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

    /// Whether this is a sign-in problem (as opposed to a version problem).
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
            Self::MissingCredential => "not signed in",
            Self::InvalidCredential => "session not valid",
            Self::AuthNotConfigured => "server can't check sessions",
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
    /// Whether the server's database is open and passed its last check. An unhealthy
    /// server refuses changes. Missing from servers before Phase 2, which counts as healthy.
    #[serde(default = "healthy_by_default")]
    pub healthy: bool,
}

/// Who is signed in, as the client sees it (from the session check and sign-in).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Their username.
    pub username: crate::user::Username,
    /// Their display name.
    pub display_name: String,
    /// They must choose a new password before anything else.
    pub must_change_password: bool,
    /// The mode the session is in.
    pub mode: crate::access::ActiveMode,
    /// The modes they may switch to (empty for a Viewer-only user, who works read-only).
    pub available_modes: Vec<crate::access::ActiveMode>,
}

/// The session check's answer: who is signed in (if anyone), and whether the server still
/// needs its first Admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    /// The signed-in user, or `None`.
    pub signed_in: Option<SessionInfo>,
    /// No users exist yet: the app should offer first-Admin setup instead of sign-in.
    pub setup_needed: bool,
}

/// The answer to a first-Admin setup attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SetupOutcome {
    /// The Admin was created and is signed in (as for [`SignInOutcome::SignedIn`]).
    SignedIn {
        /// The new Admin.
        session: SessionInfo,
        /// The session token, if asked for (the desktop app).
        token: Option<String>,
    },
    /// The setup code is wrong.
    WrongCode,
    /// Too many wrong codes; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The username breaks a rule.
    InvalidUsername {
        /// Which rule.
        reason: crate::user::UsernameError,
    },
    /// The display name breaks a rule.
    InvalidDisplayName {
        /// Which rule.
        reason: crate::user::DisplayNameError,
    },
    /// The password breaks a rule.
    InvalidPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// An Admin already exists: sign in instead.
    AlreadySetUp,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to setup step 1, checking the setup code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum CheckSetupCodeOutcome {
    /// The code is right: go on to step 2.
    Valid,
    /// The code is wrong.
    WrongCode,
    /// Too many wrong codes; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// An Admin already exists: sign in instead.
    AlreadySetUp,
}

/// Whether a user can sign in, and with what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountStatus {
    /// Signs in with their own password.
    Active,
    /// Has a temporary password they must change at their next sign-in.
    TemporaryPassword,
    /// Can't sign in.
    Disabled,
}

/// One user, as the Users screen (Admin mode) shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSummary {
    /// The user's ID.
    pub id: crate::user::UserId,
    /// Their username.
    pub username: crate::user::Username,
    /// Their display name.
    pub display_name: String,
    /// Whether they can sign in, and with what.
    pub status: AccountStatus,
    /// Their role on all vehicles, if any (per-vehicle roles arrive in milestone 4).
    pub role_on_all: Option<crate::access::Role>,
    /// This is the signed-in Admin.
    pub is_you: bool,
    /// They're the last enabled Admin: they can't be disabled or lose the Admin role.
    pub is_last_admin: bool,
}

/// The answer to a Users screen request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum UserAdminOutcome {
    /// The household's users, by username.
    Users {
        /// Everyone (not deleted).
        users: Vec<UserSummary>,
    },
    /// Done.
    Done,
    /// Done; give this temporary password to the user. Shown once.
    TemporaryPassword {
        /// The password, e.g. `QX7M-K2PD-9WRT`, typed exactly as shown.
        password: String,
    },
    /// Not in Admin mode, or not the household's Admin.
    NotAllowed,
    /// It would leave the household without an enabled Admin.
    LastAdmin,
    /// Someone already has that username.
    UsernameTaken,
    /// The username breaks a rule.
    InvalidUsername {
        /// Which rule.
        reason: crate::user::UsernameError,
    },
    /// The display name breaks a rule.
    InvalidDisplayName {
        /// Which rule.
        reason: crate::user::DisplayNameError,
    },
    /// There's no such user.
    NotFound,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to completing a password recovery (decision #047).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RecoveryOutcome {
    /// The new password is set; signed in (as for [`SignInOutcome::SignedIn`]).
    SignedIn {
        /// Who signed in.
        session: SessionInfo,
        /// The session token, if asked for (the desktop app).
        token: Option<String>,
    },
    /// The code is wrong, used, or expired.
    WrongCode,
    /// Too many attempts; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The new password breaks a rule.
    InvalidPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to a password change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum ChangePasswordOutcome {
    /// Changed. Other sessions of this user were signed out.
    Changed,
    /// The current password is wrong.
    WrongPassword,
    /// The new password breaks a rule.
    InvalidNewPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to a sign-in attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SignInOutcome {
    /// Signed in. The web app gets a session cookie; `token` is set only when the caller
    /// asked for it (the desktop app, which sends it as `Authorization: Bearer`), so a
    /// web page's scripts never see it.
    SignedIn {
        /// Who signed in.
        session: SessionInfo,
        /// The session token, for the desktop app.
        token: Option<String>,
    },
    /// Unknown username or wrong password (deliberately not saying which).
    InvalidCredentials,
    /// Too many failed attempts; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The server couldn't check right now.
    Unavailable,
}

/// The answer to a mode switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SwitchModeOutcome {
    /// Now in this mode.
    Switched {
        /// The new mode.
        mode: crate::access::ActiveMode,
    },
    /// The user's roles don't allow that mode.
    NotAllowed,
    /// Admin mode needs the password, and it was wrong.
    WrongPassword,
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't switch right now.
    Unavailable,
}

/// Servers from before the `healthy` field had no health check, so treat them as healthy.
const fn healthy_by_default() -> bool {
    true
}

/// The server's reply to a ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The server's clock, as Unix epoch milliseconds (UTC).
    pub server_time_ms: i64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn handshakes_from_older_servers_count_as_healthy() {
        let older = r#"{"app_version":"0.1.0","api_version":1,"min_supported_api_version":1,"server_time_ms":0}"#;
        let handshake: Handshake = serde_json::from_str(older).unwrap();
        assert!(handshake.healthy);
    }

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
