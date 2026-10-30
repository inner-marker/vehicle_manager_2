//! Signing in, sessions, modes, and the auth gate, against a real database.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod sessions {
    use std::sync::Arc;

    use axum::Router;
    use axum::body::Body;
    use axum::extract::Request;
    use axum::http::StatusCode;
    use axum::routing::get;
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::{Clock, UnixMillis};
    use domain::user::{User, UserPatch, Username};
    use server::auth::{AuthConfig, SessionSource, protect, session_cookie_name};
    use server::login_limiter::LoginLimiter;
    use server::passwords::hash_password;
    use server::sessions::{
        ADMIN_IDLE_MILLIS, IDLE_LIFETIME_MILLIS, SignInError, SignedIn, SwitchModeError, resolve,
        sign_in, sign_out, switch_mode, token_hash,
    };
    use storage::commands::RecordWriter;
    use storage::users::{RoleGrantWriter, UserWriter};
    use storage::{Database, NewDatabaseIds};
    use tower::ServiceExt;

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const PASSWORD: &str = "correct horse battery";

    fn at(offset_millis: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + offset_millis)
    }

    /// A database with user `stuart` (Admin on all assets, password [`PASSWORD`]).
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        add_user(&database, 1, "stuart", Role::Admin).await;
        (temp, database)
    }

    async fn add_user(database: &Database, n: u32, username: &str, role: Role) {
        let user = User {
            username: Username::parse(username).unwrap(),
            display_name: username.into(),
            disabled: false,
            must_change_password: false,
        };
        let grant = RoleGrant {
            user: test_id(n),
            target: GrantTarget::AllAssets,
            role,
        };
        let mut connection = database.pool().acquire().await.unwrap();
        UserWriter
            .create(&mut connection, test_id(n), &UserPatch::all(user), 1)
            .await
            .unwrap();
        RoleGrantWriter
            .create(
                &mut connection,
                test_id(n + 100),
                &RoleGrantPatch::all(grant),
                2,
            )
            .await
            .unwrap();
        drop(connection);
        let hash = hash_password(PASSWORD.into()).await.unwrap();
        database
            .set_password_hash(test_id(n), &hash, NOW)
            .await
            .unwrap();
    }

    async fn signed_in(database: &Database) -> (String, SignedIn) {
        let (token, signed_in) = sign_in(
            database,
            &LoginLimiter::new(),
            "Stuart",
            PASSWORD.into(),
            "phone",
            NOW,
        )
        .await
        .unwrap();
        (token.as_str().to_owned(), signed_in)
    }

    #[tokio::test]
    async fn signing_in_starts_a_driver_mode_session_the_token_resolves_to() {
        let (_temp, database) = database().await;

        let (token, signed_in) = signed_in(&database).await;

        assert_eq!(signed_in.username.as_str(), "stuart");
        assert_eq!(
            signed_in.mode,
            ActiveMode::Driver,
            "never starts in Admin mode"
        );
        let resolved = resolve(&database, token_hash(&token).unwrap(), at(1_000))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(resolved.user, signed_in.user);
        assert_eq!(resolved.device, signed_in.device);
    }

    #[tokio::test]
    async fn wrong_password_unknown_user_and_disabled_user_all_look_the_same() {
        let (_temp, database) = database().await;
        let limiter = LoginLimiter::new();

        let wrong = sign_in(
            &database,
            &limiter,
            "stuart",
            "wrong password!".into(),
            "a",
            NOW,
        )
        .await;
        let unknown = sign_in(&database, &limiter, "nobody", PASSWORD.into(), "b", NOW).await;
        let mut connection = database.pool().acquire().await.unwrap();
        let disable = UserPatch {
            disabled: Some(true),
            ..UserPatch::default()
        };
        UserWriter
            .update(&mut connection, test_id(1), &disable, 3)
            .await
            .unwrap();
        drop(connection);
        let disabled = sign_in(&database, &limiter, "stuart", PASSWORD.into(), "c", NOW).await;

        for result in [wrong, unknown, disabled] {
            assert!(
                matches!(result, Err(SignInError::InvalidCredentials)),
                "{result:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_sixth_wrong_password_in_15_minutes_is_refused_even_if_right() {
        let (_temp, database) = database().await;
        let limiter = LoginLimiter::new();
        for attempt in 0..5 {
            let result = sign_in(
                &database,
                &limiter,
                "stuart",
                "wrong password!".into(),
                "phone",
                at(attempt),
            )
            .await;
            assert!(matches!(result, Err(SignInError::InvalidCredentials)));
        }

        let sixth = sign_in(
            &database,
            &limiter,
            "stuart",
            PASSWORD.into(),
            "laptop",
            at(10),
        )
        .await;

        assert!(
            matches!(sixth, Err(SignInError::RateLimited { .. })),
            "{sixth:?}"
        );
    }

    #[tokio::test]
    async fn sessions_end_after_30_idle_days_and_each_use_extends_them() {
        let (_temp, database) = database().await;
        let (token, _) = signed_in(&database).await;
        let hash = token_hash(&token).unwrap();

        // Used after 20 days: extended to 30 days from then.
        let day = 24 * 60 * 60 * 1000;
        assert!(
            resolve(&database, hash, at(20 * day))
                .await
                .unwrap()
                .is_some()
        );
        assert!(
            resolve(&database, hash, at(45 * day))
                .await
                .unwrap()
                .is_some()
        );
        // 30 idle days after that last use, it's gone.
        assert!(
            resolve(&database, hash, at(45 * day + IDLE_LIFETIME_MILLIS))
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            database.session(hash).await.unwrap().is_none(),
            "and deleted"
        );
    }

    #[tokio::test]
    async fn admin_mode_needs_the_password_and_lapses_after_15_idle_minutes() {
        let (_temp, database) = database().await;
        let (token, signed_in) = signed_in(&database).await;
        let hash = token_hash(&token).unwrap();

        assert_eq!(
            switch_mode(
                &database,
                &signed_in,
                ActiveMode::Admin,
                Some("wrong password!".into()),
                NOW
            )
            .await,
            Err(SwitchModeError::WrongPassword)
        );
        assert_eq!(
            switch_mode(&database, &signed_in, ActiveMode::Admin, None, NOW).await,
            Err(SwitchModeError::WrongPassword)
        );
        assert_eq!(
            switch_mode(
                &database,
                &signed_in,
                ActiveMode::Admin,
                Some(PASSWORD.into()),
                NOW
            )
            .await,
            Ok(ActiveMode::Admin)
        );
        assert_eq!(
            resolve(&database, hash, at(60_000))
                .await
                .unwrap()
                .unwrap()
                .mode,
            ActiveMode::Admin
        );

        let lapsed = resolve(&database, hash, at(60_000 + ADMIN_IDLE_MILLIS))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(lapsed.mode, ActiveMode::Driver);
    }

    #[tokio::test]
    async fn entering_admin_mode_after_a_long_idle_stretch_doesnt_drop_straight_back() {
        let (_temp, database) = database().await;
        let (token, signed_in) = signed_in(&database).await;
        let twenty_minutes = 20 * 60 * 1000;

        // Signed in at NOW, idle for 20 minutes, then enters Admin mode.
        switch_mode(
            &database,
            &signed_in,
            ActiveMode::Admin,
            Some(PASSWORD.into()),
            at(twenty_minutes),
        )
        .await
        .unwrap();

        let next_click = resolve(
            &database,
            token_hash(&token).unwrap(),
            at(twenty_minutes + 5_000),
        )
        .await
        .unwrap()
        .unwrap();
        assert_eq!(next_click.mode, ActiveMode::Admin);
    }

    #[tokio::test]
    async fn a_driver_cant_switch_to_maintainer_or_admin() {
        let (_temp, database) = database().await;
        add_user(&database, 2, "teen", Role::Driver).await;
        let (_, teen) = sign_in(
            &database,
            &LoginLimiter::new(),
            "teen",
            PASSWORD.into(),
            "x",
            NOW,
        )
        .await
        .unwrap();

        assert_eq!(
            switch_mode(&database, &teen, ActiveMode::Maintainer, None, NOW).await,
            Err(SwitchModeError::NotAllowed)
        );
        assert_eq!(
            switch_mode(
                &database,
                &teen,
                ActiveMode::Admin,
                Some(PASSWORD.into()),
                NOW
            )
            .await,
            Err(SwitchModeError::NotAllowed)
        );
    }

    #[tokio::test]
    async fn disabling_a_user_ends_their_sessions_and_signing_out_ends_one() {
        let (_temp, database) = database().await;
        let (first, _) = signed_in(&database).await;
        let (second, _) = signed_in(&database).await;

        sign_out(&database, token_hash(&first).unwrap())
            .await
            .unwrap();
        assert!(
            resolve(&database, token_hash(&first).unwrap(), NOW)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            resolve(&database, token_hash(&second).unwrap(), NOW)
                .await
                .unwrap()
                .is_some()
        );

        let mut connection = database.pool().acquire().await.unwrap();
        let disable = UserPatch {
            disabled: Some(true),
            ..UserPatch::default()
        };
        UserWriter
            .update(&mut connection, test_id(1), &disable, 3)
            .await
            .unwrap();
        drop(connection);
        assert!(
            resolve(&database, token_hash(&second).unwrap(), NOW)
                .await
                .unwrap()
                .is_none()
        );
    }

    // --- The auth gate with sessions -------------------------------------------------------

    /// A router whose `/api/v1/who` answers with the signed-in username the gate attached.
    fn router(database: Database) -> Router {
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
            .route("/api/v1/session", get(|| async { "public" }));
        let clock: Arc<dyn Clock + Send + Sync> = Arc::new(ManualClock::new(NOW));
        let config = AuthConfig::unconfigured()
            .with_sessions(SessionSource::new(move || Some(database.clone()), clock));
        protect(routes, config)
    }

    async fn get_with(
        router: Router,
        path: &str,
        header: Option<(&str, String)>,
    ) -> (StatusCode, String) {
        let mut request = Request::builder().uri(path).header("x-api-version", "1");
        if let Some((name, value)) = header {
            request = request.header(name, value);
        }
        let response = router
            .oneshot(request.body(Body::empty()).unwrap())
            .await
            .unwrap();
        let status = response.status();
        let body = axum::body::to_bytes(response.into_body(), usize::MAX)
            .await
            .unwrap();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn the_gate_accepts_a_session_cookie_or_bearer_token_and_attaches_the_user() {
        let (_temp, database) = database().await;
        let (token, _) = signed_in(&database).await;
        let cookie = format!("theme=dark; {}={token}", session_cookie_name());

        let by_cookie = get_with(
            router(database.clone()),
            "/api/v1/who",
            Some(("cookie", cookie)),
        )
        .await;
        let by_bearer = get_with(
            router(database.clone()),
            "/api/v1/who",
            Some(("authorization", format!("Bearer {token}"))),
        )
        .await;

        assert_eq!(by_cookie, (StatusCode::OK, "stuart".to_owned()));
        assert_eq!(by_bearer, (StatusCode::OK, "stuart".to_owned()));
    }

    #[tokio::test]
    async fn the_gate_refuses_no_session_and_a_bad_session_but_not_public_paths() {
        let (_temp, database) = database().await;

        let none = get_with(router(database.clone()), "/api/v1/who", None).await;
        let bad = get_with(
            router(database.clone()),
            "/api/v1/who",
            Some((
                "cookie",
                format!("{}={}", session_cookie_name(), "0".repeat(64)),
            )),
        )
        .await;
        let public = get_with(router(database.clone()), "/api/v1/session", None).await;

        assert_eq!(none.0, StatusCode::UNAUTHORIZED);
        assert!(none.1.contains("missing-credential"), "{}", none.1);
        assert_eq!(bad.0, StatusCode::UNAUTHORIZED);
        assert!(bad.1.contains("invalid-credential"), "{}", bad.1);
        assert_eq!(public, (StatusCode::OK, "public".to_owned()));
    }
}
