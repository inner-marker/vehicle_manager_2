//! No password, token, or code reaches a log, except the setup and recovery codes, which are
//! written on purpose to the console only (`console_only = true`; decisions #028, #047)
//! (phase-3.md 1d; `docs/sub-plans/04-security.md` §7.4).
//!
//! Every path a secret travels runs here with all logging on (trace level), into a buffer:
//! first-Admin setup, signing in, a wrong password, Admin mode, changing a password, adding a
//! user and resetting their password, and a recovery. Then the buffer is searched for every
//! secret used or given out.

#![cfg(test)]

mod common;

use std::io::Write;
use std::sync::{Arc, Mutex, PoisonError};

use common::NOW;
use domain::access::{ActiveMode, Role};
use domain::api::UserAdminOutcome;
use domain::secret::Secret;
use domain::test_support::{ManualClock, test_id};
use domain::timers::Timers;
use server::health::Health;
use server::login_limiter::LoginLimiter;
use server::sessions::{change_password, sign_in, switch_mode};
use server::setup::{SetupRequest, create_first_admin, open_if_needed};
use server::user_admin::{AdminContext, add_user, reset_password};
use storage::{Database, NewDatabaseIds};

/// Log output, collected.
#[derive(Clone, Default)]
struct Captured(Arc<Mutex<Vec<u8>>>);

impl Captured {
    fn text(&self) -> String {
        String::from_utf8_lossy(&self.0.lock().unwrap_or_else(PoisonError::into_inner)).into_owned()
    }
}

impl Write for Captured {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .extend_from_slice(bytes);
        Ok(bytes.len())
    }

    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

/// The value after `field=` on the line that has it (codes are written as `field=K7QM-…`).
fn field_value(log: &str, field: &str) -> Option<String> {
    let start = log.find(&format!("{field}="))? + field.len() + 1;
    let value: String = log[start..]
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
        .collect();
    (!value.is_empty()).then_some(value)
}

/// Every line mentioning `secret` is a console-only one.
fn only_on_console(log: &str, secret: &str) -> bool {
    log.lines()
        .filter(|line| line.contains(secret))
        .all(|line| line.contains("console_only=true"))
}

/// First-Admin setup on an empty server: its code and password.
async fn first_admin(clock: &ManualClock) -> (String, &'static str) {
    let empty = tempfile::tempdir().unwrap();
    let ids = NewDatabaseIds {
        database_id: test_id(800),
        server_device_id: test_id(801),
    };
    let (fresh, _) = Database::open(empty.path(), NOW, ids).await.unwrap();
    let setup_code = open_if_needed(0).expect("a setup code on an empty server");
    let setup_password = "first admin secret phrase";
    create_first_admin(
        &fresh,
        test_id(801),
        Health::Healthy,
        clock,
        SetupRequest {
            code: Secret::new(setup_code.as_str()),
            username: "first".into(),
            display_name: "First".into(),
            password: Secret::new(setup_password),
        },
    )
    .await
    .unwrap();
    (setup_code, setup_password)
}

/// The secrets the session steps used.
struct SessionSecrets {
    token: String,
    wrong: &'static str,
    changed: &'static str,
}

/// Signing in, a wrong password, Admin mode, and a password change; returns Jordan's Admin
/// session too.
async fn sessions(
    database: &Database,
    limiter: &LoginLimiter,
    clock: &ManualClock,
) -> (SessionSecrets, server::sessions::SignedIn) {
    let (token, jordan) = sign_in(
        database,
        limiter,
        "jordan",
        "a long password".into(),
        "client-1",
        NOW,
    )
    .await
    .unwrap();
    let wrong = "a wrong password guess";
    assert!(
        sign_in(database, limiter, "sam", wrong.into(), "client-2", NOW)
            .await
            .is_err()
    );
    switch_mode(
        database,
        &jordan,
        ActiveMode::Admin,
        Some("a long password".into()),
        NOW,
    )
    .await
    .unwrap();
    let admin = server::sessions::resolve(database, jordan.token_hash, NOW, Timers::NORMAL)
        .await
        .unwrap()
        .expect("still signed in");
    assert_eq!(admin.mode, ActiveMode::Admin);
    let changed = "a changed secret phrase";
    let (_, sam) = sign_in(
        database,
        limiter,
        "sam",
        "a long password".into(),
        "c3",
        NOW,
    )
    .await
    .unwrap();
    change_password(
        database,
        &sam,
        "a long password".into(),
        changed.into(),
        Health::Healthy,
        clock,
    )
    .await
    .unwrap();
    let secrets = SessionSecrets {
        token: token.as_str().to_owned(),
        wrong,
        changed,
    };
    (secrets, admin)
}

/// An Admin adds a user and resets their password: the two temporary passwords.
async fn temporary_passwords(context: &AdminContext<'_>) -> (Secret, Secret) {
    let UserAdminOutcome::TemporaryPassword { password: added } =
        add_user(context, "alex", "Alex", Role::Driver).await
    else {
        panic!("adding a user failed");
    };
    let UserAdminOutcome::Users { users } = server::user_admin::list_users(context).await else {
        panic!("listing users failed");
    };
    let alex = users
        .iter()
        .find(|user| user.username.as_str() == "alex")
        .unwrap()
        .id;
    let UserAdminOutcome::TemporaryPassword { password: reset } =
        reset_password(context, alex).await
    else {
        panic!("resetting a password failed");
    };
    (added, reset)
}

/// A recovery: the code goes to the console; completing it sets a new password. Returns the
/// code and the new password.
async fn recovery(
    database: &Database,
    limiter: &LoginLimiter,
    clock: &ManualClock,
    captured: &Captured,
) -> (String, &'static str) {
    server::recovery::request_code(database, limiter, "jordan", "c4", NOW, Timers::NORMAL)
        .await
        .unwrap();
    let code = field_value(&captured.text(), "recovery_code")
        .expect("the recovery code is written to the console");
    let recovered = "a recovered secret phrase";
    server::recovery::complete(
        database,
        limiter,
        "jordan",
        &Secret::new(code.as_str()),
        recovered.into(),
        "c4",
        clock,
    )
    .await
    .unwrap();
    (code, recovered)
}

#[tokio::test]
async fn no_secret_reaches_a_log() {
    let captured = Captured::default();
    let writer = captured.clone();
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::TRACE)
        .with_writer(move || writer.clone())
        .finish();
    let _guard = tracing::subscriber::set_default(subscriber);
    let clock = ManualClock::new(NOW);
    let limiter = LoginLimiter::new();

    let (setup_code, setup_password) = first_admin(&clock).await;
    let (_temp, database, _, _) = common::setup().await;
    let (session, admin) = sessions(&database, &limiter, &clock).await;
    let context = AdminContext {
        database: &database,
        admin: &admin,
        health: Health::Healthy,
        clock: &clock,
    };
    let (added, reset) = temporary_passwords(&context).await;
    let (recovery_code, recovered) = recovery(&database, &limiter, &clock, &captured).await;

    let log = captured.text();
    // The paths really logged (so an empty capture can't pass by accident).
    for expected in ["sign-in refused", "password recovered", "recovery_code="] {
        assert!(
            log.contains(expected),
            "expected a log line with {expected:?}"
        );
    }
    let never = [
        "a long password",
        setup_password,
        session.wrong,
        session.changed,
        recovered,
        added.expose(),
        reset.expose(),
        session.token.as_str(),
    ];
    for secret in never {
        assert!(
            !log.contains(secret),
            "a secret reached the log: {secret}\n{log}"
        );
    }
    // The codes are written on purpose, but only to the console.
    for code in [setup_code.as_str(), recovery_code.as_str()] {
        assert!(log.contains(code), "the code is in the console log");
        assert!(
            only_on_console(&log, code),
            "{code} is in a non-console line:\n{log}"
        );
    }
}
