//! Server functions: the app's HTTP API, all under `/api/` (`domain::api::API_PATH_PREFIX`).
//!
//! Every function here is guarded by the auth layer from `server::auth`, which checks
//! the credential and API version before the body runs; a test proves no function
//! escapes it. Keep these thin: authenticate (done by the layer), call into
//! `server`/`domain`, respond (AGENTS.md §3).
//!
//! **Never call a server function during server-side rendering.** On the server such
//! a call runs in-process and skips the HTTP layer (and so the auth check); call them
//! from event handlers or effects, which only run in the browser or app.

use dioxus::prelude::*;
use domain::api::{Handshake, Pong};

// Imports used only inside function bodies are declared there: the `#[get]` macro
// keeps bodies in the server build only, so top-level imports would be unused on web.

/// Liveness check: answers with the server's clock.
///
/// # Errors
///
/// Refused by the auth layer (401 / 400 / 426 / 503 with an `ApiError` reason) if the
/// credential or API version is wrong; otherwise never fails.
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
    })
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

    #[tokio::test]
    async fn every_server_function_rejects_unauthenticated_requests() {
        // The real server-function routes, behind the real auth layer.
        let server_functions =
            Router::<FullstackState>::new().serve_api_application(ServeConfig::new(), App);
        let guarded = ::server::auth::protect(
            server_functions,
            ::server::auth::AuthConfig::with_credential("test-only"),
        );

        for function in ServerFunction::collect() {
            let unauthenticated = Request::builder()
                .method(function.method())
                .uri(function.path())
                .body(Body::empty())
                .unwrap();
            let response = guarded.clone().oneshot(unauthenticated).await.unwrap();
            assert_eq!(
                response.status(),
                StatusCode::UNAUTHORIZED,
                "{} {} answered without a credential",
                function.method(),
                function.path()
            );
        }
    }
}
