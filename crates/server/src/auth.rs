//! The auth and API-version gate in front of every server function (phase-1.md step 7,
//! phase-2.md milestone 2).
//!
//! One Axum middleware guards **every** path under [`API_PATH_PREFIX`], so a new server
//! function is protected without anyone remembering to opt in. For each such request:
//! 1. **Who is calling:** a session token, from the session cookie (web) or
//!    `Authorization: Bearer` (desktop), is looked up ([`crate::sessions::resolve`]). The
//!    signed-in user ([`SignedIn`]) is attached to the request's extensions, where server
//!    functions read it (`app::api`).
//! 2. **Authenticated?** A valid session passes. Until step 2e retires it, so does the
//!    Phase 1 test credential (`Authorization: Bearer <credential>`, constant-time compare).
//!    Only the few [`PUBLIC_API_PATHS`] (handshake, sign-in, the session check) are allowed
//!    without either.
//! 3. The client's API version header must be one this server supports.
//!
//! Authentication is checked before the version, so an unauthenticated caller learns
//! nothing else about the server. Paths outside `/api/` (pages, assets) pass through
//! untouched so the UI can load.

use std::fmt;
use std::sync::Arc;

use domain::identity::APP_ID;
use domain::time::Clock;
use storage::Database;

use crate::sessions::{self, SignedIn, token_hash};

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, COOKIE, HeaderMap, HeaderValue};
use axum::http::{StatusCode, Uri};
use axum::middleware::{self, Next};
use axum::response::Response;
use domain::api::{
    API_PATH_PREFIX, API_VERSION_HEADER, ApiError, check_client_api_version, parse_api_version,
};
use domain::identity::env_var_name;
use domain::log_area;
use subtle::ConstantTimeEq;

/// The environment variable holding the test credential, without the app prefix
/// (`domain::identity::env_var_name` turns it into `VEHICLEMANAGER_TEST_CREDENTIAL`).
const CREDENTIAL_ENV_NAME: &str = "TEST_CREDENTIAL";

/// The prefix of an `Authorization` header carrying a bearer credential.
const BEARER_PREFIX: &str = "Bearer ";

/// The name of the environment variable the server reads its credential from.
pub fn credential_env_var() -> String {
    env_var_name(CREDENTIAL_ENV_NAME)
}

/// The `/api/` paths that work without signing in. Each is still version-checked, and still
/// gets the signed-in user attached if a valid session comes with it. Kept short and
/// explicit; a test checks each one is a real server function.
pub const PUBLIC_API_PATHS: &[&str] = &["/api/v1/handshake", "/api/v1/sign-in", "/api/v1/session"];

/// The session cookie's name, from the app ID: `vehiclemanager_session`.
#[must_use]
pub fn session_cookie_name() -> String {
    format!("{APP_ID}_session")
}

/// Where the gate finds sessions: the database (once open) and the real clock.
#[derive(Clone)]
pub struct SessionSource {
    database: Arc<dyn Fn() -> Option<Database> + Send + Sync>,
    clock: Arc<dyn Clock + Send + Sync>,
}

impl SessionSource {
    /// Sessions from whatever `database` returns when a request arrives (the server opens
    /// its database after the router is built), stamped with `clock`.
    pub fn new(
        database: impl Fn() -> Option<Database> + Send + Sync + 'static,
        clock: Arc<dyn Clock + Send + Sync>,
    ) -> Self {
        Self {
            database: Arc::new(database),
            clock,
        }
    }
}

/// What the auth gate accepts. Cheap to clone; shared by every request.
#[derive(Clone)]
pub struct AuthConfig {
    /// The Phase 1 test credential; `None` if not configured.
    credential: Option<Arc<str>>,
    /// Where sessions are checked; `None` only in tests of the credential path.
    sessions: Option<SessionSource>,
}

impl AuthConfig {
    /// Accepts exactly this credential. Blank (empty or whitespace) means none is configured.
    pub fn with_credential(credential: &str) -> Self {
        let credential = credential.trim();
        Self {
            credential: (!credential.is_empty()).then(|| Arc::from(credential)),
            sessions: None,
        }
    }

    /// Also accepts signed-in sessions from `source`.
    #[must_use]
    pub fn with_sessions(mut self, source: SessionSource) -> Self {
        self.sessions = Some(source);
        self
    }

    /// Refuses every API request.
    pub const fn unconfigured() -> Self {
        Self {
            credential: None,
            sessions: None,
        }
    }

    /// Reads the credential from [`credential_env_var`]; unset or blank means refuse everything.
    pub fn from_env() -> Self {
        let name = credential_env_var();
        let config = std::env::var(&name).map_or_else(
            |_| Self::unconfigured(),
            |value| Self::with_credential(&value),
        );
        if config.is_configured() {
            tracing::info!(target: log_area::AUTH, env_var = %name, "API credential configured");
        } else {
            tracing::warn!(
                target: log_area::AUTH,
                env_var = %name,
                "no API credential configured; every API request will be refused"
            );
        }
        config
    }

    /// Whether a credential is configured.
    pub const fn is_configured(&self) -> bool {
        self.credential.is_some()
    }

    /// Whether anything at all can authenticate a request.
    const fn can_authenticate(&self) -> bool {
        self.credential.is_some() || self.sessions.is_some()
    }
}

impl fmt::Debug for AuthConfig {
    /// Never prints the credential itself.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthConfig")
            .field("configured", &self.is_configured())
            .finish()
    }
}

/// Puts the auth gate in front of every route of `router`. Only `/api/` paths are checked.
pub fn protect<S>(router: Router<S>, config: AuthConfig) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(middleware::from_fn_with_state(config, require_auth))
}

/// The middleware: lets the request through (with the signed-in user attached, if any), or
/// answers it with an [`ApiError`].
async fn require_auth(
    State(config): State<AuthConfig>,
    mut request: Request,
    next: Next,
) -> Response {
    if !is_api_path(request.uri()) {
        return next.run(request).await;
    }
    let signed_in = signed_in_from(&config, request.headers()).await;
    let public = PUBLIC_API_PATHS.contains(&request.uri().path());
    match check_request(&config, request.headers(), signed_in.is_some(), public) {
        Ok(()) => {
            if let Some(signed_in) = signed_in {
                request.extensions_mut().insert(signed_in);
            }
            next.run(request).await
        }
        Err(error) => {
            // Logs why, never the credential or token.
            tracing::debug!(
                target: log_area::AUTH,
                path = request.uri().path(),
                reason = error.description(),
                "API request refused"
            );
            refusal_response(error)
        }
    }
}

/// The signed-in user behind the request's session token, if it has a valid one.
async fn signed_in_from(config: &AuthConfig, headers: &HeaderMap) -> Option<SignedIn> {
    let source = config.sessions.as_ref()?;
    let hash = presented_session_token(headers).and_then(|token| token_hash(&token))?;
    let database = (source.database)()?;
    match sessions::resolve(&database, hash, source.clock.now()).await {
        Ok(signed_in) => signed_in,
        Err(error) => {
            tracing::error!(target: log_area::AUTH, %error, "can't check the session");
            None
        }
    }
}

/// The session token from the session cookie, or else from `Authorization: Bearer`.
fn presented_session_token(headers: &HeaderMap) -> Option<String> {
    let cookie_name = session_cookie_name();
    let from_cookie = headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|value| value.to_str().ok())
        .flat_map(|cookies| cookies.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == cookie_name)
        .map(|(_, value)| value.to_owned());
    from_cookie.or_else(|| bearer(headers).map(str::to_owned))
}

/// The value after `Bearer ` in the `Authorization` header.
fn bearer(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix(BEARER_PREFIX)
}

fn is_api_path(uri: &Uri) -> bool {
    uri.path().starts_with(API_PATH_PREFIX)
}

/// Checks one API request: authentication (unless `public`), then the version.
/// `has_session` says whether a valid session came with it.
///
/// # Errors
///
/// The first [`ApiError`] that applies.
pub fn check_request(
    config: &AuthConfig,
    headers: &HeaderMap,
    has_session: bool,
    public: bool,
) -> Result<(), ApiError> {
    if !public && !has_session {
        if !config.can_authenticate() {
            return Err(ApiError::AuthNotConfigured);
        }
        match config.credential.as_deref() {
            Some(expected) => check_credential(expected, headers)?,
            None if headers.contains_key(AUTHORIZATION) || headers.contains_key(COOKIE) => {
                return Err(ApiError::InvalidCredential);
            }
            None => return Err(ApiError::MissingCredential),
        }
    }
    check_api_version_header(headers)
}

fn check_credential(expected: &str, headers: &HeaderMap) -> Result<(), ApiError> {
    let value = headers
        .get(AUTHORIZATION)
        .ok_or(ApiError::MissingCredential)?;
    let presented = value
        .to_str()
        .ok()
        .and_then(|text| text.strip_prefix(BEARER_PREFIX))
        .ok_or(ApiError::InvalidCredential)?;
    // Constant time for equal-length inputs, so response timing doesn't reveal how
    // much of a guess was right. (A length mismatch fails fast; length isn't secret enough
    // to matter for this stub.)
    if bool::from(presented.as_bytes().ct_eq(expected.as_bytes())) {
        Ok(())
    } else {
        Err(ApiError::InvalidCredential)
    }
}

fn check_api_version_header(headers: &HeaderMap) -> Result<(), ApiError> {
    let value = headers
        .get(API_VERSION_HEADER)
        .ok_or(ApiError::MissingApiVersion)?;
    let text = value.to_str().map_err(|_| ApiError::MalformedApiVersion)?;
    check_client_api_version(parse_api_version(text)?)
}

/// A refusal in the shape Dioxus clients decode (`{"message","code","data"}`), so the
/// client receives `ServerFnError::ServerError { code, details: Some("<reason>") }`.
fn refusal_response(error: ApiError) -> Response {
    let status = StatusCode::from_u16(error.http_status()).unwrap_or(StatusCode::BAD_REQUEST);
    let body = serde_json::json!({
        "message": error.description(),
        "code": status.as_u16(),
        "data": error,
    });
    let mut response = Response::new(Body::from(body.to_string()));
    *response.status_mut() = status;
    response
        .headers_mut()
        .insert(CONTENT_TYPE, HeaderValue::from_static("application/json"));
    response
}

#[cfg(test)]
mod tests {
    use axum::routing::get;
    use domain::api::{API_VERSION, MIN_SUPPORTED_API_VERSION};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    const CREDENTIAL: &str = "correct horse battery staple";

    fn router(config: AuthConfig) -> Router {
        let routes = Router::new()
            .route("/api/v1/ping", get(|| async { "pong" }))
            .route("/garage", get(|| async { "page" }));
        protect(routes, config)
    }

    /// Sends a GET with optional credential and version headers; returns status and body.
    async fn send(
        config: AuthConfig,
        path: &str,
        credential: Option<&str>,
        version: Option<&str>,
    ) -> (StatusCode, String) {
        let mut request = Request::builder().uri(path);
        if let Some(credential) = credential {
            request = request.header(AUTHORIZATION, format!("Bearer {credential}"));
        }
        if let Some(version) = version {
            request = request.header(API_VERSION_HEADER, version);
        }
        let response = router(config)
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn configured() -> AuthConfig {
        AuthConfig::with_credential(CREDENTIAL)
    }

    /// The current API version header value (`API_VERSION` is 1).
    const CURRENT: Option<&str> = Some("1");

    /// The reason code a refusal carried.
    fn reason(body: &str) -> ApiError {
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        serde_json::from_value(value["data"].clone()).unwrap()
    }

    #[tokio::test]
    async fn request_without_credential_is_401() {
        let (status, body) = send(configured(), "/api/v1/ping", None, CURRENT).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(reason(&body), ApiError::MissingCredential);
    }

    #[tokio::test]
    async fn wrong_credential_is_401() {
        let (status, body) = send(configured(), "/api/v1/ping", Some("guess"), CURRENT).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(reason(&body), ApiError::InvalidCredential);
    }

    #[tokio::test]
    async fn non_bearer_authorization_is_401() {
        let request = Request::builder()
            .uri("/api/v1/ping")
            .header(AUTHORIZATION, format!("Basic {CREDENTIAL}"))
            .header(API_VERSION_HEADER, "1")
            .body(Body::empty())
            .unwrap();
        let response = router(configured()).oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn unconfigured_auth_refuses_every_api_request() {
        let (status, body) = send(
            AuthConfig::unconfigured(),
            "/api/v1/ping",
            Some(CREDENTIAL),
            CURRENT,
        )
        .await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reason(&body), ApiError::AuthNotConfigured);
        // A blank credential counts as not configured.
        assert!(!AuthConfig::with_credential("   ").is_configured());
    }

    #[tokio::test]
    async fn missing_or_malformed_version_is_400() {
        let (status, body) = send(configured(), "/api/v1/ping", Some(CREDENTIAL), None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(reason(&body), ApiError::MissingApiVersion);

        let (status, body) = send(configured(), "/api/v1/ping", Some(CREDENTIAL), Some("v1")).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(reason(&body), ApiError::MalformedApiVersion);
    }

    #[tokio::test]
    async fn old_client_is_426() {
        let old = (MIN_SUPPORTED_API_VERSION - 1).to_string();
        let (status, body) = send(configured(), "/api/v1/ping", Some(CREDENTIAL), Some(&old)).await;
        assert_eq!(status, StatusCode::UPGRADE_REQUIRED);
        assert_eq!(reason(&body), ApiError::ClientTooOld);
    }

    #[tokio::test]
    async fn newer_client_is_400() {
        let newer = (API_VERSION + 1).to_string();
        let (status, body) =
            send(configured(), "/api/v1/ping", Some(CREDENTIAL), Some(&newer)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(reason(&body), ApiError::ClientTooNew);
    }

    #[tokio::test]
    async fn credential_is_checked_before_version() {
        // A wrong credential with a bad version reports only the credential problem.
        let (status, body) = send(configured(), "/api/v1/ping", Some("guess"), Some("999")).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(reason(&body), ApiError::InvalidCredential);
    }

    #[tokio::test]
    async fn valid_request_reaches_the_handler() {
        let (status, body) = send(configured(), "/api/v1/ping", Some(CREDENTIAL), CURRENT).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "pong");
    }

    #[tokio::test]
    async fn non_api_paths_pass_through() {
        let (status, body) = send(AuthConfig::unconfigured(), "/garage", None, None).await;
        assert_eq!(status, StatusCode::OK);
        assert_eq!(body, "page");
    }

    #[tokio::test]
    async fn refusals_use_the_dioxus_error_payload_shape() {
        let (_, body) = send(configured(), "/api/v1/ping", None, CURRENT).await;
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["code"], 401);
        assert_eq!(value["data"], "missing-credential");
        assert!(value["message"].is_string());
    }

    #[test]
    fn credential_env_var_name_comes_from_app_id() {
        assert_eq!(credential_env_var(), "VEHICLEMANAGER_TEST_CREDENTIAL");
    }

    #[test]
    fn debug_output_never_contains_the_credential() {
        let debug = format!("{:?}", configured());
        assert!(!debug.contains(CREDENTIAL), "{debug}");
        assert!(debug.contains("configured: true"), "{debug}");
    }
}
