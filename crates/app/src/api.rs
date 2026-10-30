//! Server functions: the app's HTTP API, all under `/api/` (`domain::api::API_PATH_PREFIX`).
//!
//! Every function here is guarded by the auth layer from `server::auth`, which checks
//! the signed-in session and API version before the body runs; only the few public paths
//! (`server::auth::PUBLIC_API_PATHS`) work without a session. Tests prove no other
//! function escapes it. Keep these thin: authenticate (done by the layer), call into
//! `server`/`domain`, respond (AGENTS.md §3).
//!
//! **Never call a server function during server-side rendering.** On the server such
//! a call runs in-process and skips the HTTP layer (and so the auth check); call them
//! from event handlers or effects, which only run in the browser or app.

use dioxus::prelude::*;
use domain::api::{
    ChangePasswordOutcome, Handshake, Pong, SessionState, SetupOutcome, SignInOutcome,
    SwitchModeOutcome,
};

// Imports used only inside function bodies are declared there: the `#[get]` macro
// keeps bodies in the server build only, so top-level imports would be unused on web.

/// Liveness check: answers with the server's clock.
///
/// # Errors
///
/// Refused by the auth layer (401 / 400 / 426 / 503 with an `ApiError` reason) without a
/// valid session or with a wrong API version; otherwise never fails.
#[get("/api/v1/ping")]
// Server functions must be async; these two don't await anything yet. The lint fires
// on the server-side wrapper `#[get]` generates, so it applies to the server build only.
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; no awaits yet"
    )
)]
pub(crate) async fn ping() -> ServerFnResult<Pong> {
    use crate::logging::SystemClock;
    use domain::time::Clock;

    Ok(Pong {
        server_time_ms: SystemClock.now().as_millis(),
    })
}

/// What the server is: app version, API versions it speaks and accepts, and its clock.
///
/// # Errors
///
/// Refused by the auth layer like [`ping`]; otherwise never fails.
#[get("/api/v1/handshake")]
// Server functions must be async; these two don't await anything yet. The lint fires
// on the server-side wrapper `#[get]` generates, so it applies to the server build only.
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; no awaits yet"
    )
)]
pub(crate) async fn handshake() -> ServerFnResult<Handshake> {
    use crate::logging::SystemClock;
    use domain::api::{API_VERSION, MIN_SUPPORTED_API_VERSION};
    use domain::time::Clock;

    Ok(Handshake {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        api_version: API_VERSION,
        min_supported_api_version: MIN_SUPPORTED_API_VERSION,
        server_time_ms: SystemClock.now().as_millis(),
        healthy: ::server::health::current().is_healthy(),
    })
}

/// Signs in. On success the web app gets an `HttpOnly` session cookie; `want_token` (the
/// desktop app) also returns the token, to send as `Authorization: Bearer`.
///
/// A public path ([`::server::auth::PUBLIC_API_PATHS`]): it works without a session.
///
/// # Errors
///
/// Only transport errors; refusals are [`SignInOutcome`] values.
#[post("/api/v1/sign-in")]
pub(crate) async fn sign_in(
    username: String,
    password: String,
    want_token: bool,
) -> ServerFnResult<SignInOutcome> {
    use ::server::sessions::SignInError;

    let Some(database) = ::server::startup::database() else {
        return Ok(SignInOutcome::Unavailable);
    };
    let now = server_side::now();
    let client = server_side::client_key();
    match ::server::sessions::sign_in(
        database,
        &server_side::LOGIN_LIMITER,
        &username,
        password,
        &client,
        now,
    )
    .await
    {
        Ok((token, signed_in)) => {
            server_side::set_session_cookie(Some(token.as_str()));
            Ok(SignInOutcome::SignedIn {
                session: server_side::session_info(&signed_in),
                token: want_token.then(|| token.as_str().to_owned()),
            })
        }
        Err(SignInError::InvalidCredentials) => Ok(SignInOutcome::InvalidCredentials),
        Err(SignInError::RateLimited { retry_after_millis }) => {
            Ok(SignInOutcome::TooManyAttempts {
                // Rounded up, so "try again in 0 seconds" never appears.
                retry_after_seconds: (retry_after_millis + 999) / 1000,
            })
        }
        Err(SignInError::Unavailable) => Ok(SignInOutcome::Unavailable),
    }
}

/// Who is signed in on this session (if anyone), and whether the server still needs its first
/// Admin. A public path, so the app can ask before signing in.
///
/// # Errors
///
/// Only transport errors.
#[get("/api/v1/session")]
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; this one only reads the request"
    )
)]
pub(crate) async fn session() -> ServerFnResult<SessionState> {
    Ok(SessionState {
        signed_in: server_side::signed_in().map(|signed_in| server_side::session_info(&signed_in)),
        setup_needed: ::server::setup::is_open(),
    })
}

/// Creates the first Admin with the setup code from the server log, and signs them in.
/// A public path; it works only while the server has no users.
///
/// # Errors
///
/// Only transport errors; refusals are [`SetupOutcome`] values.
#[post("/api/v1/setup")]
pub(crate) async fn setup(
    code: String,
    username: String,
    display_name: String,
    password: String,
    want_token: bool,
) -> ServerFnResult<SetupOutcome> {
    use ::server::setup::SetupError;

    let (Some(database), Some(device)) = (
        ::server::startup::database(),
        ::server::startup::server_device(),
    ) else {
        return Ok(SetupOutcome::Unavailable);
    };
    let now = server_side::now();
    // Wrong codes count against the same limit as wrong passwords, by client.
    let client_key = format!("setup:{}", server_side::client_key());
    if let Err(retry_after_millis) = server_side::LOGIN_LIMITER.check(&[&client_key], now) {
        return Ok(SetupOutcome::TooManyAttempts {
            retry_after_seconds: (retry_after_millis + 999) / 1000,
        });
    }
    let request = ::server::setup::SetupRequest {
        code,
        username: username.clone(),
        display_name,
        password: password.clone(),
    };
    let created = ::server::setup::create_first_admin(
        database,
        device,
        ::server::health::current(),
        &crate::logging::SystemClock,
        request,
    )
    .await;
    match created {
        Ok(_) => {}
        Err(SetupError::WrongCode) => {
            server_side::LOGIN_LIMITER.record_failure(&[&client_key], now);
            return Ok(SetupOutcome::WrongCode);
        }
        Err(SetupError::AlreadySetUp) => return Ok(SetupOutcome::AlreadySetUp),
        Err(SetupError::InvalidUsername(reason)) => {
            return Ok(SetupOutcome::InvalidUsername { reason });
        }
        Err(SetupError::InvalidDisplayName(reason)) => {
            return Ok(SetupOutcome::InvalidDisplayName { reason });
        }
        Err(SetupError::InvalidPassword(reason)) => {
            return Ok(SetupOutcome::InvalidPassword { reason });
        }
        Err(SetupError::Unavailable) => return Ok(SetupOutcome::Unavailable),
    }
    // Signed straight in, so the new Admin doesn't have to type it all again.
    Ok(match sign_in(username, password, want_token).await? {
        SignInOutcome::SignedIn { session, token } => SetupOutcome::SignedIn { session, token },
        SignInOutcome::InvalidCredentials
        | SignInOutcome::TooManyAttempts { .. }
        | SignInOutcome::Unavailable => SetupOutcome::Unavailable,
    })
}

/// Changes the signed-in user's password; their other sessions are signed out.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ChangePasswordOutcome`] values.
#[post("/api/v1/password")]
pub(crate) async fn change_password(
    current: String,
    new: String,
) -> ServerFnResult<ChangePasswordOutcome> {
    use ::server::sessions::ChangePasswordError;

    let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
    else {
        return Ok(ChangePasswordOutcome::NotSignedIn);
    };
    Ok(
        match ::server::sessions::change_password(
            database,
            &signed_in,
            current,
            new,
            ::server::health::current(),
            &crate::logging::SystemClock,
        )
        .await
        {
            Ok(()) => ChangePasswordOutcome::Changed,
            Err(ChangePasswordError::WrongPassword) => ChangePasswordOutcome::WrongPassword,
            Err(ChangePasswordError::Invalid(reason)) => {
                ChangePasswordOutcome::InvalidNewPassword { reason }
            }
            Err(ChangePasswordError::Unavailable) => ChangePasswordOutcome::Unavailable,
        },
    )
}

/// Signs out: ends this session and clears the cookie.
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[post("/api/v1/sign-out")]
pub(crate) async fn sign_out() -> ServerFnResult<()> {
    if let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
        && let Err(error) = ::server::sessions::sign_out(database, signed_in.token_hash).await
    {
        tracing::error!(target: domain::log_area::AUTH, %error, "sign-out failed");
    }
    server_side::set_session_cookie(None);
    Ok(())
}

/// Switches this session's mode. Admin mode needs `password`.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SwitchModeOutcome`] values.
#[post("/api/v1/mode")]
pub(crate) async fn switch_mode(
    mode: domain::access::ActiveMode,
    password: Option<String>,
) -> ServerFnResult<SwitchModeOutcome> {
    use ::server::sessions::SwitchModeError;

    let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
    else {
        return Ok(SwitchModeOutcome::NotSignedIn);
    };
    Ok(
        match ::server::sessions::switch_mode(
            database,
            &signed_in,
            mode,
            password,
            server_side::now(),
        )
        .await
        {
            Ok(mode) => SwitchModeOutcome::Switched { mode },
            Err(SwitchModeError::NotAllowed) => SwitchModeOutcome::NotAllowed,
            Err(SwitchModeError::WrongPassword) => SwitchModeOutcome::WrongPassword,
            Err(SwitchModeError::Unavailable) => SwitchModeOutcome::Unavailable,
        },
    )
}

/// Server-side helpers for the server functions above: the current request's signed-in
/// user and client, the session cookie, and the login limiter.
#[cfg(feature = "server")]
mod server_side {
    use std::sync::LazyLock;

    use dioxus::prelude::dioxus_fullstack::FullstackContext;
    use dioxus::prelude::dioxus_server::http::HeaderValue;
    use dioxus::prelude::dioxus_server::http::header::SET_COOKIE;
    use domain::api::SessionInfo;
    use domain::time::{Clock, UnixMillis};
    use server::login_limiter::LoginLimiter;
    use server::sessions::{IDLE_LIFETIME_MILLIS, SignedIn};

    use crate::logging::SystemClock;

    /// Failed sign-ins, shared by every request.
    pub(super) static LOGIN_LIMITER: LazyLock<LoginLimiter> = LazyLock::new(LoginLimiter::new);

    pub(super) fn now() -> UnixMillis {
        SystemClock.now()
    }

    /// The signed-in user the auth layer attached to this request.
    pub(super) fn signed_in() -> Option<SignedIn> {
        FullstackContext::current()?.extension::<SignedIn>()
    }

    /// Where the request came from, for the login limit. The server listens only on the
    /// NAS's loopback, behind `tailscale serve`, which names the real client in
    /// `X-Forwarded-For`; without it, all requests share one key.
    pub(super) fn client_key() -> String {
        FullstackContext::current()
            .and_then(|context| {
                context
                    .parts_mut()
                    .headers
                    .get("x-forwarded-for")
                    .and_then(|value| value.to_str().ok())
                    .and_then(|value| value.split(',').next())
                    .map(|first| first.trim().to_owned())
            })
            .unwrap_or_else(|| "unknown".to_owned())
    }

    /// Sets the session cookie to `token`, or clears it with `None`.
    pub(super) fn set_session_cookie(token: Option<&str>) {
        let name = server::auth::session_cookie_name();
        // HttpOnly: scripts can't read it. Secure: HTTPS only (tailscale serve; browsers also
        // allow it on localhost). SameSite=Strict: never sent from other sites' pages.
        let cookie = match token {
            Some(token) => format!(
                "{name}={token}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={}",
                IDLE_LIFETIME_MILLIS / 1000
            ),
            None => format!("{name}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0"),
        };
        if let (Some(context), Ok(value)) =
            (FullstackContext::current(), HeaderValue::from_str(&cookie))
        {
            context.add_response_header(SET_COOKIE, value);
        }
    }

    pub(super) fn session_info(signed_in: &SignedIn) -> SessionInfo {
        SessionInfo {
            username: signed_in.username.clone(),
            display_name: signed_in.display_name.clone(),
            must_change_password: signed_in.must_change_password,
            mode: signed_in.mode,
            available_modes: signed_in.available_modes(),
        }
    }
}

/// Tests of the auth coverage guarantee: every registered server function is under
/// `/api/` and refuses unauthenticated requests. Server-only (the registry and router
/// exist only with the `server` feature); CI runs them with `--features server`.
#[cfg(all(test, feature = "server"))]
mod tests {
    use dioxus::prelude::dioxus_server::FullstackState;
    use dioxus::prelude::dioxus_server::axum::Router;
    use dioxus::prelude::dioxus_server::axum::body::Body;
    use dioxus::prelude::dioxus_server::http::{Request, StatusCode};
    use dioxus::prelude::{DioxusRouterExt, ServeConfig, ServerFunction};
    use domain::api::API_PATH_PREFIX;
    use tower::ServiceExt;

    use crate::ui::App;

    #[test]
    fn every_server_function_is_under_api_prefix() {
        let functions = ServerFunction::collect();
        assert!(!functions.is_empty(), "no server functions registered");
        for function in functions {
            assert!(
                function.path().starts_with(API_PATH_PREFIX),
                "{} {} is outside {API_PATH_PREFIX}, so the auth layer wouldn't guard it",
                function.method(),
                function.path()
            );
        }
    }

    #[test]
    fn every_public_path_is_a_real_server_function() {
        // A typo in the public list would silently leave a function protected, or worse,
        // name a path that a future function could take over unprotected.
        let registered: Vec<&str> = ServerFunction::collect()
            .into_iter()
            .map(ServerFunction::path)
            .collect();
        for public in ::server::auth::PUBLIC_API_PATHS {
            assert!(
                registered.contains(public),
                "{public} isn't a registered server function"
            );
        }
    }

    #[tokio::test]
    async fn every_server_function_rejects_unauthenticated_requests() {
        // The real server-function routes, behind the real auth layer.
        let server_functions =
            Router::<FullstackState>::new().serve_api_application(ServeConfig::new(), App);
        let guarded = ::server::auth::protect(
            server_functions,
            // A session source with no database: nothing can be signed in.
            ::server::auth::AuthConfig::new(::server::auth::SessionSource::new(
                || None,
                std::sync::Arc::new(crate::logging::SystemClock),
            )),
        );

        let protected = ServerFunction::collect()
            .into_iter()
            .filter(|function| !::server::auth::PUBLIC_API_PATHS.contains(&function.path()));
        for function in protected {
            let unauthenticated = Request::builder()
                .method(function.method())
                .uri(function.path())
                .body(Body::empty())
                .unwrap();
            let response = guarded.clone().oneshot(unauthenticated).await.unwrap();
            // 503 here (no database, so no session can be valid); 401 with one.
            assert!(
                matches!(
                    response.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::SERVICE_UNAVAILABLE
                ),
                "{} {} answered without a session",
                function.method(),
                function.path()
            );
        }
    }
}
