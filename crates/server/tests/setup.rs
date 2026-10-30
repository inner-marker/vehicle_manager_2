//! First-Admin setup, start to finish. One test, because the setup code is one per process.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod first_admin {
    use domain::access::{GrantTarget, Role};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::user::{PasswordError, UsernameError};
    use server::health::{Health, HealthProblem};
    use server::setup::{SetupError, SetupRequest, create_first_admin, is_open, open_if_needed};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const PASSWORD: &str = "correct horse battery";

    fn request(code: &str, username: &str, display_name: &str, password: String) -> SetupRequest {
        SetupRequest {
            code: code.to_owned(),
            username: username.to_owned(),
            display_name: display_name.to_owned(),
            password,
        }
    }

    /// Wrong codes, an unhealthy server, and mistakes in the details are refused, and the
    /// code stays valid.
    async fn assert_refusals(database: &Database, clock: &ManualClock, code: &str) {
        let device = test_id(901);
        let wrong = create_first_admin(
            database,
            device,
            Health::Healthy,
            clock,
            request("AAAA-AAAA-AAAA-AAAA", "stuart", "Stuart", PASSWORD.into()),
        )
        .await;
        assert!(matches!(wrong, Err(SetupError::WrongCode)), "{wrong:?}");
        let bad_name = create_first_admin(
            database,
            device,
            Health::Healthy,
            clock,
            request(code, "st", "Stuart", PASSWORD.into()),
        )
        .await;
        assert!(matches!(
            bad_name,
            Err(SetupError::InvalidUsername(UsernameError::WrongLength))
        ));
        let short = create_first_admin(
            database,
            device,
            Health::Healthy,
            clock,
            request(code, "stuart", "Stuart", "short".into()),
        )
        .await;
        assert!(matches!(
            short,
            Err(SetupError::InvalidPassword(PasswordError::TooShort))
        ));
        let unhealthy = Health::Unhealthy(HealthProblem::IntegrityCheckFailed);
        let refused = create_first_admin(
            database,
            device,
            unhealthy,
            clock,
            request(code, "stuart", "Stuart", PASSWORD.into()),
        )
        .await;
        assert!(
            matches!(refused, Err(SetupError::Unavailable)),
            "{refused:?}"
        );
        assert_eq!(database.user_count().await.unwrap(), 0);
    }

    /// The Admin exists with Admin on all assets and a password, made by the server itself.
    async fn assert_admin_created(database: &Database, admin: domain::user::UserId) {
        let stored = database.user(admin).await.unwrap().unwrap();
        assert_eq!(stored.user.username.as_str(), "stuart");
        assert_eq!(stored.user.display_name, "Stuart R");
        let grants = database.grants_for(admin).await.unwrap();
        assert_eq!(grants.len(), 1);
        assert_eq!(
            (grants[0].1.target, grants[0].1.role),
            (GrantTarget::AllAssets, Role::Admin)
        );
        assert!(database.password_hash(admin).await.unwrap().is_some());
        // In the history with the server's device and no user.
        let (with_actor, from_server): (i64, i64) = sqlx::query_as(
            "SELECT count(user_id), count(*) FROM applied_commands WHERE device_id = ?",
        )
        .bind(test_id::<domain::id::Device>(901).to_bytes().to_vec())
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!((with_actor, from_server), (0, 2));
    }

    #[tokio::test]
    async fn the_setup_code_creates_exactly_one_admin_then_stops_working() {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let clock = ManualClock::new(NOW);

        // No users: setup opens with a code like K7QM-4XRT-9PWD-H2CN.
        let code = open_if_needed(database.user_count().await.unwrap()).unwrap();
        assert!(is_open());
        assert_eq!((code.len(), code.matches('-').count()), (19, 3));

        assert_refusals(&database, &clock, &code).await;

        // The right code, typed loosely.
        let loose = code.to_lowercase().replace('-', " ");
        let admin = create_first_admin(
            &database,
            test_id(901),
            Health::Healthy,
            &clock,
            request(&loose, "Stuart", " Stuart R ", PASSWORD.into()),
        )
        .await
        .unwrap();
        assert_admin_created(&database, admin).await;

        // Closed: the same code no longer works, and a restart with a user makes no new one.
        assert!(!is_open());
        let again = create_first_admin(
            &database,
            test_id(901),
            Health::Healthy,
            &clock,
            request(&code, "second", "Second", PASSWORD.into()),
        )
        .await;
        assert!(matches!(again, Err(SetupError::AlreadySetUp)), "{again:?}");
        assert_eq!(open_if_needed(database.user_count().await.unwrap()), None);
        assert!(!is_open());
    }
}
