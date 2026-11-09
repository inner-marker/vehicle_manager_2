//! A paired desktop's requests must be signed with its key (phase-3.md 2b, decision #073),
//! through the real auth layer: a copied token alone, a replayed or reworked signature, an old
//! one, or another device's key is refused; a browser's session needs none.

#![cfg(test)]

mod common;

use std::sync::Arc;

use axum::Router;
use axum::body::Body;
use axum::extract::Request;
use axum::http::StatusCode;
use axum::routing::get;
use common::NOW;
use domain::access::ActiveMode;
use domain::device::DeviceKind;
use domain::device_signature::{
    DeviceSignature, HEADER, REQUEST_WINDOW_MILLIS, Scope, public_key_of, token_hash,
};
use domain::test_support::{ManualClock, test_id};
use domain::time::{Clock, UnixMillis};
use domain::timers::Timers;
use server::auth::{AuthConfig, SessionSource, protect};
use server::sessions::{IDLE_LIFETIME_MILLIS, SignedIn};
use storage::Database;
use storage::devices::StoredDevice;
use storage::users::Session;
use tower::ServiceExt;

/// The paired desktop's secret key (its seed).
const SECRET: [u8; 32] = [7; 32];

/// Another device's.
const OTHER_SECRET: [u8; 32] = [8; 32];

/// The desktop's session token, as it holds it.
fn token() -> String {
    "ab".repeat(32)
}

fn at(offset: i64) -> UnixMillis {
    UnixMillis::from_millis(NOW.as_millis() + offset)
}

/// Stuart's paired desktop, with a session, added to `database`.
async fn pair_desktop(database: &Database) {
    let device = StoredDevice {
        id: test_id(500),
        user: test_id(1),
        kind: DeviceKind::Desktop,
        name: None,
        public_key: Some(public_key_of(&SECRET)),
        added_at: NOW,
        last_seen: NOW,
        signed_out_at: None,
    };
    let session = Session {
        token_hash: token_hash(&token()).unwrap(),
        user: test_id(1),
        device: device.id,
        created_at: NOW,
        last_used: NOW,
        expires_at: at(IDLE_LIFETIME_MILLIS),
        active_mode: ActiveMode::Driver,
    };
    database
        .insert_device_with_session(&device, &session)
        .await
        .unwrap();
}

/// A protected route that says who's signed in, and a public one, behind the auth layer with
/// the clock at `now`.
fn router(database: Database, now: UnixMillis) -> Router {
    let routes = Router::new()
        .route(
            "/api/v1/who",
            get(|request: Request| async move {
                request.extensions().get::<SignedIn>().map_or_else(
                    || "nobody".to_owned(),
                    |signed_in| signed_in.username.as_str().to_owned(),
                )
            }),
        )
        .route("/api/v1/other", get(|| async { "other" }))
        .route(
            "/api/v1/session",
            get(|request: Request| async move {
                if request.extensions().get::<SignedIn>().is_some() {
                    "signed in"
                } else {
                    "not signed in"
                }
            }),
        );
    let clock: Arc<dyn Clock + Send + Sync> = Arc::new(ManualClock::new(now));
    let config = AuthConfig::new(SessionSource::new(move || Some(database.clone()), clock));
    protect(routes, config)
}

/// Sends GET `path` with the desktop's bearer token and, if given, a signature header.
async fn send(
    router: &Router,
    path: &str,
    token: &str,
    signature: Option<String>,
) -> (StatusCode, String) {
    let mut request = Request::builder()
        .uri(path)
        .header("x-api-version", "1")
        .header("authorization", format!("Bearer {token}"));
    if let Some(signature) = signature {
        request = request.header(HEADER, signature);
    }
    let response = router
        .clone()
        .oneshot(request.body(Body::empty()).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), usize::MAX)
        .await
        .unwrap();
    (status, String::from_utf8(body.to_vec()).unwrap())
}

/// A signature by `secret` for GET `path` (request scope) or the session.
fn signature(secret: &[u8; 32], scope: Scope, path: &str, time: UnixMillis, nonce: u8) -> String {
    DeviceSignature::sign(
        secret,
        scope,
        "GET",
        path,
        time,
        [nonce; 16],
        &token_hash(&token()).unwrap(),
    )
    .to_header()
}

fn refused_for_signature(response: &(StatusCode, String)) -> bool {
    response.0 == StatusCode::UNAUTHORIZED && response.1.contains("device-signature")
}

#[tokio::test]
async fn a_signed_request_passes_and_a_copied_token_alone_does_not() {
    let (_temp, database, _, _) = common::setup().await;
    pair_desktop(&database).await;
    let router = router(database, NOW);

    let signed = signature(&SECRET, Scope::Request, "/api/v1/who", NOW, 1);
    assert_eq!(
        send(&router, "/api/v1/who", &token(), Some(signed)).await,
        (StatusCode::OK, "stuart".into())
    );
    let session = signature(&SECRET, Scope::Session, "", NOW, 2);
    assert_eq!(
        send(&router, "/api/v1/who", &token(), Some(session)).await,
        (StatusCode::OK, "stuart".into())
    );
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), None).await
    ));
}

#[tokio::test]
async fn a_replayed_reworked_old_or_foreign_signature_is_refused() {
    let (_temp, database, _, _) = common::setup().await;
    pair_desktop(&database).await;
    let router = router(database, NOW);

    // Used once, then replayed.
    let once = signature(&SECRET, Scope::Request, "/api/v1/who", NOW, 3);
    assert_eq!(
        send(&router, "/api/v1/who", &token(), Some(once.clone()))
            .await
            .0,
        StatusCode::OK
    );
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), Some(once)).await
    ));
    // Made for another path.
    let elsewhere = signature(&SECRET, Scope::Request, "/api/v1/other", NOW, 4);
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), Some(elsewhere)).await
    ));
    // Too old.
    let old = signature(
        &SECRET,
        Scope::Request,
        "/api/v1/who",
        at(-REQUEST_WINDOW_MILLIS - 1),
        5,
    );
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), Some(old)).await
    ));
    // Another device's key.
    let foreign = signature(&OTHER_SECRET, Scope::Request, "/api/v1/who", NOW, 6);
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), Some(foreign)).await
    ));
    // Not the format.
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), Some("v1;nonsense".into())).await
    ));
}

#[tokio::test]
async fn a_public_path_goes_on_as_signed_out_without_a_signature() {
    let (_temp, database, _, _) = common::setup().await;
    pair_desktop(&database).await;
    let router = router(database, NOW);
    assert_eq!(
        send(&router, "/api/v1/session", &token(), None).await,
        (StatusCode::OK, "not signed in".into())
    );
    let signed = signature(&SECRET, Scope::Request, "/api/v1/session", NOW, 7);
    assert_eq!(
        send(&router, "/api/v1/session", &token(), Some(signed)).await,
        (StatusCode::OK, "signed in".into())
    );
}

#[tokio::test]
async fn a_refused_signature_writes_nothing_and_resolve_never_lets_a_paired_device_in() {
    let (_temp, database, _, _) = common::setup().await;
    pair_desktop(&database).await;
    let hash = token_hash(&token()).unwrap();
    // A day later, a request with the token but no signature: the session isn't extended.
    let later = at(24 * 60 * 60 * 1000);
    let router = router(database.clone(), later);
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", &token(), None).await
    ));
    assert_eq!(
        database.session(hash).await.unwrap().unwrap().last_used,
        NOW
    );
    // The unchecked lookup refuses a paired device's session outright.
    assert!(
        server::sessions::resolve(&database, hash, later, Timers::NORMAL)
            .await
            .unwrap()
            .is_none()
    );
    // With a good signature, the same request extends it.
    let signed = signature(&SECRET, Scope::Session, "", later, 8);
    assert_eq!(
        send(&router, "/api/v1/who", &token(), Some(signed)).await.0,
        StatusCode::OK
    );
    assert_eq!(
        database.session(hash).await.unwrap().unwrap().last_used,
        later
    );
}

#[tokio::test]
async fn a_browser_session_needs_no_signature() {
    let (_temp, database, stuart, _) = common::setup().await;
    let router = router(database.clone(), NOW);
    // The browser's own token isn't known here, so check through resolve, as the gate does.
    let signed_in = server::sessions::resolve(&database, stuart.token_hash, NOW, Timers::NORMAL)
        .await
        .unwrap();
    assert!(signed_in.is_some(), "a browser session needs no key");
    // And a made-up token is simply not signed in (not a signature problem).
    let (status, body) = send(&router, "/api/v1/who", &"cd".repeat(32), None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!body.contains("device-signature"), "{body}");
}

#[tokio::test]
async fn pairing_at_sign_in_stores_the_key_and_the_token_then_needs_signatures() {
    use server::login_limiter::LoginLimiter;
    use server::sessions::{SigningInDevice, sign_in_from};

    let (_temp, database, _, _) = common::setup().await;
    let name = domain::device::DeviceName::parse("Stuarts-MacBook-Pro").unwrap();
    let (paired_token, signed_in) = sign_in_from(
        &database,
        &LoginLimiter::new(),
        "stuart",
        "a long password".into(),
        "laptop",
        NOW,
        SigningInDevice::Desktop {
            public_key: public_key_of(&SECRET),
            name: Some(name.clone()),
        },
    )
    .await
    .unwrap();
    let device = database.device(signed_in.device).await.unwrap().unwrap();
    assert_eq!(device.kind, DeviceKind::Desktop);
    assert_eq!(device.name, Some(name));
    assert_eq!(device.public_key, Some(public_key_of(&SECRET)));

    let router = router(database.clone(), NOW);
    let paired = paired_token.as_str();
    let hash = token_hash(paired).unwrap();
    let signed =
        DeviceSignature::sign(&SECRET, Scope::Session, "", "", NOW, [9; 16], &hash).to_header();
    assert_eq!(
        send(&router, "/api/v1/who", paired, Some(signed)).await,
        (StatusCode::OK, "stuart".into())
    );
    assert!(refused_for_signature(
        &send(&router, "/api/v1/who", paired, None).await
    ));

    // An older desktop app (API version 1) gets a token and no key: no signature needed.
    let (older_token, older) = sign_in_from(
        &database,
        &LoginLimiter::new(),
        "stuart",
        "a long password".into(),
        "old-laptop",
        NOW,
        SigningInDevice::OlderDesktop,
    )
    .await
    .unwrap();
    let older_device = database.device(older.device).await.unwrap().unwrap();
    assert_eq!(
        (older_device.kind, older_device.public_key),
        (DeviceKind::Desktop, None)
    );
    assert_eq!(
        send(&router, "/api/v1/who", older_token.as_str(), None).await,
        (StatusCode::OK, "stuart".into())
    );
}
