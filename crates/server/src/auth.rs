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
//!    Only the few [`PUBLIC_API_PATHS`] (handshake, sign-in, the session check, first-Admin
//!    setup) are allowed
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
use domain::log_area;

/// The prefix of an `Authorization` header carrying a session token (the desktop app).
const BEARER_PREFIX: &str = "Bearer ";

/// The `/api/` paths that work without signing in. Each is still version-checked, and still
/// gets the signed-in user attached if a valid session comes with it. Kept short and
/// explicit; a test checks each one is a real server function.
pub const PUBLIC_API_PATHS: &[&str] = &[
    "/api/v1/handshake",
    "/api/v1/sign-in",
    "/api/v1/session",
    "/api/v1/setup",
    "/api/v1/setup/code",
];

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
    /// Where sessions are checked; `None` refuses every protected request.
    sessions: Option<SessionSource>,
}

impl AuthConfig {
    /// Accepts signed-in sessions from `source`.
    #[must_use]
    pub fn new(source: SessionSource) -> Self {
        Self {
            sessions: Some(source),
        }
    }

    /// Refuses every protected request (only public paths work).
    #[must_use]
    pub const fn refuse_all() -> Self {
        Self { sessions: None }
    }
}

impl fmt::Debug for AuthConfig {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("AuthConfig")
            .field("sessions", &self.sessions.is_some())
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
        return Err(missing_session_reason(config, headers));
    }
    check_api_version_header(headers)
}

/// Why a protected request without a valid session is refused: sessions can't be checked
/// at all, none was sent, or the one sent isn't valid (ended, signed out, or made up).
fn missing_session_reason(config: &AuthConfig, headers: &HeaderMap) -> ApiError {
    // Sessions can't be checked without the database (not open yet, or failed to open).
    let can_check = config
        .sessions
        .as_ref()
        .is_some_and(|source| (source.database)().is_some());
    if !can_check {
        ApiError::AuthNotConfigured
    } else if presented_session_token(headers).is_some() {
        ApiError::InvalidCredential
    } else {
        ApiError::MissingCredential
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

    /// A public path, a protected path, and a page, behind the gate.
    fn router(config: AuthConfig) -> Router {
        let routes = Router::new()
            .route("/api/v1/handshake", get(|| async { "hello" }))
            .route("/api/v1/ping", get(|| async { "pong" }))
            .route("/garage", get(|| async { "page" }));
        protect(routes, config)
    }

    /// Sends a GET with an optional version header; returns status and body.
    async fn send(config: AuthConfig, path: &str, version: Option<&str>) -> (StatusCode, String) {
        let mut request = Request::builder().uri(path);
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

    /// The current API version header value (`API_VERSION` is 1).
    const CURRENT: Option<&str> = Some("1");

    /// The reason code a refusal carried.
    fn reason(body: &str) -> ApiError {
        let value: serde_json::Value = serde_json::from_str(body).unwrap();
        serde_json::from_value(value["data"].clone()).unwrap()
    }

    #[tokio::test]
    async fn protected_paths_are_refused_when_sessions_cant_be_checked() {
        let (status, body) = send(AuthConfig::refuse_all(), "/api/v1/ping", CURRENT).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reason(&body), ApiError::AuthNotConfigured);
    }

    #[tokio::test]
    async fn public_paths_work_without_a_session_but_still_check_the_version() {
        let (status, body) = send(AuthConfig::refuse_all(), "/api/v1/handshake", CURRENT).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "hello"));

        let (status, body) = send(AuthConfig::refuse_all(), "/api/v1/handshake", None).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(reason(&body), ApiError::MissingApiVersion);

        let (_, body) = send(AuthConfig::refuse_all(), "/api/v1/handshake", Some("v1")).await;
        assert_eq!(reason(&body), ApiError::MalformedApiVersion);
    }

    #[tokio::test]
    async fn old_client_is_426_and_newer_client_is_400() {
        let old = (MIN_SUPPORTED_API_VERSION - 1).to_string();
        let (status, body) = send(AuthConfig::refuse_all(), "/api/v1/handshake", Some(&old)).await;
        assert_eq!(status, StatusCode::UPGRADE_REQUIRED);
        assert_eq!(reason(&body), ApiError::ClientTooOld);

        let newer = (API_VERSION + 1).to_string();
        let (status, body) =
            send(AuthConfig::refuse_all(), "/api/v1/handshake", Some(&newer)).await;
        assert_eq!(status, StatusCode::BAD_REQUEST);
        assert_eq!(reason(&body), ApiError::ClientTooNew);
    }

    #[tokio::test]
    async fn authentication_is_checked_before_version() {
        // No session and a bad version reports only the session problem.
        let (status, body) = send(AuthConfig::refuse_all(), "/api/v1/ping", Some("999")).await;
        assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE);
        assert_eq!(reason(&body), ApiError::AuthNotConfigured);
    }

    #[tokio::test]
    async fn non_api_paths_pass_through() {
        let (status, body) = send(AuthConfig::refuse_all(), "/garage", None).await;
        assert_eq!((status, body.as_str()), (StatusCode::OK, "page"));
    }

    #[tokio::test]
    async fn refusals_use_the_dioxus_error_payload_shape() {
        let (_, body) = send(AuthConfig::refuse_all(), "/api/v1/ping", CURRENT).await;
        let value: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(value["code"], 503);
        assert_eq!(value["data"], "auth-not-configured");
        assert!(value["message"].is_string());
    }

    #[test]
    fn the_session_cookie_is_named_after_the_app_id() {
        assert_eq!(session_cookie_name(), "vehiclemanager_session");
    }
}
