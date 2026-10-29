//! The auth and API-version gate in front of every server function (phase-1.md step 7).
//!
//! One Axum middleware guards **every** path under [`API_PATH_PREFIX`], so a new
//! server function is protected without anyone remembering to opt in. For each
//! such request, in order:
//! 1. The server must have a credential configured; otherwise everything is refused
//!    (fail closed, decisions.md #024).
//! 2. `Authorization: Bearer <credential>` must match it, compared in constant time.
//! 3. The client's API version header must be one this server supports.
//!
//! The credential is checked before the version, so an unauthenticated caller
//! learns nothing else about the server. Paths outside `/api/` (pages, assets) pass
//! through untouched so the UI can load.
//!
//! This is the Phase 1 stub: one shared test credential from an environment
//! variable. Logins, session cookies, and per-device tokens replace it later.

use std::fmt;
use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::{Request, State};
use axum::http::header::{AUTHORIZATION, CONTENT_TYPE, HeaderMap, HeaderValue};
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

/// What the auth gate accepts. Cheap to clone; shared by every request.
#[derive(Clone)]
pub struct AuthConfig {
    /// `None` means no credential is configured, so every API request is refused.
    credential: Option<Arc<str>>,
}

impl AuthConfig {
    /// Accepts exactly this credential. Blank (empty or whitespace) means none is configured.
    pub fn with_credential(credential: &str) -> Self {
        let credential = credential.trim();
        Self {
            credential: (!credential.is_empty()).then(|| Arc::from(credential)),
        }
    }

    /// Refuses every API request.
    pub const fn unconfigured() -> Self {
        Self { credential: None }
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

/// The middleware: lets the request through, or answers it with an [`ApiError`].
async fn require_auth(State(config): State<AuthConfig>, request: Request, next: Next) -> Response {
    if !is_api_path(request.uri()) {
        return next.run(request).await;
    }
    match check_request(&config, request.headers()) {
        Ok(()) => next.run(request).await,
        Err(error) => {
            // Logs why, never the credential.
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

fn is_api_path(uri: &Uri) -> bool {
    uri.path().starts_with(API_PATH_PREFIX)
}

/// Checks one API request's headers: configured, credential, then version.
///
/// # Errors
///
/// The first [`ApiError`] that applies.
pub fn check_request(config: &AuthConfig, headers: &HeaderMap) -> Result<(), ApiError> {
    let expected = config
        .credential
        .as_deref()
        .ok_or(ApiError::AuthNotConfigured)?;
    check_credential(expected, headers)?;
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
