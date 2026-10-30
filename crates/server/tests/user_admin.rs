//! Managing users in Admin mode, against a real database through the command pipeline.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod user_admin {
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::api::{AccountStatus, UserAdminOutcome, UserSummary};
    use domain::command::Change;
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::user::{User, UserId, UserPatch, Username};
    use server::commands::issue;
    use server::health::Health;
    use server::login_limiter::LoginLimiter;
    use server::passwords::hash_password;
    use server::sessions::{SignInError, SignedIn, resolve, sign_in, switch_mode, token_hash};
    use server::user_admin::{
        AdminContext, add_user, list_users, reset_password, set_disabled, set_role_on_all,
    };
    use storage::users::{RoleGrantWriter, UserWriter};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const PASSWORD: &str = "correct horse battery";

    /// A database with Admin `stuart` (user 1), signed in and switched to Admin mode.
    async fn admin_session() -> (tempfile::TempDir, Database, SignedIn) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let clock = ManualClock::new(NOW);
        let user = User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: false,
        };
        let grant = RoleGrant {
            user: test_id(1),
            target: GrantTarget::AllAssets,
            role: Role::Admin,
        };
        issue(
            &database,
            &UserWriter,
            Change::Create {
                id: test_id(1),
                values: UserPatch::all(user),
            },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        issue(
            &database,
            &RoleGrantWriter,
            Change::Create {
                id: test_id(101),
                values: RoleGrantPatch::all(grant),
            },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        database
            .set_password_hash(
                test_id(1),
                &hash_password(PASSWORD.into()).await.unwrap(),
                NOW,
            )
            .await
            .unwrap();
        let (_, mut admin) = sign_in(
            &database,
            &LoginLimiter::new(),
            "stuart",
            PASSWORD.into(),
            "a",
            NOW,
        )
        .await
        .unwrap();
        admin.mode = switch_mode(
            &database,
            &admin,
            ActiveMode::Admin,
            Some(PASSWORD.into()),
            NOW,
        )
        .await
        .unwrap();
        (temp, database, admin)
    }

    fn context<'a>(
        database: &'a Database,
        admin: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> AdminContext<'a> {
        AdminContext {
            database,
            admin,
            health: Health::Healthy,
            clock,
        }
    }

    async fn users(context: &AdminContext<'_>) -> Vec<UserSummary> {
        let outcome = list_users(context).await;
        let UserAdminOutcome::Users { users } = outcome else {
            panic!("expected users, got {outcome:?}");
        };
        users
    }

    async fn add(context: &AdminContext<'_>, username: &str, role: Role) -> (UserId, String) {
        let UserAdminOutcome::TemporaryPassword { password } =
            add_user(context, username, username, role).await
        else {
            panic!("add failed");
        };
        let id = users(context)
            .await
            .into_iter()
            .find(|user| user.username.as_str() == username)
            .unwrap()
            .id;
        (id, password)
    }

    #[tokio::test]
    async fn nothing_works_outside_admin_mode() {
        let (_temp, database, mut admin) = admin_session().await;
        admin.mode = ActiveMode::Maintainer;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);

        assert_eq!(list_users(&context).await, UserAdminOutcome::NotAllowed);
        assert_eq!(
            add_user(&context, "sam", "Sam", Role::Driver).await,
            UserAdminOutcome::NotAllowed
        );
        assert_eq!(
            reset_password(&context, test_id(1)).await,
            UserAdminOutcome::NotAllowed
        );
    }

    #[tokio::test]
    async fn an_added_user_signs_in_with_the_temporary_password_and_must_change_it() {
        let (_temp, database, admin) = admin_session().await;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);

        let (sam, password) = add(&context, "sam", Role::Driver).await;

        assert_eq!(
            password.len(),
            14,
            "12 characters in groups of 4: {password}"
        );
        let summary = users(&context)
            .await
            .into_iter()
            .find(|user| user.id == sam)
            .unwrap();
        assert_eq!(
            (summary.role_on_all, summary.status),
            (Some(Role::Driver), AccountStatus::TemporaryPassword)
        );
        let (_, signed_in) = sign_in(&database, &LoginLimiter::new(), "sam", password, "b", NOW)
            .await
            .unwrap();
        assert!(signed_in.must_change_password);
        assert_eq!(
            add_user(&context, "SAM", "Sam again", Role::Viewer).await,
            UserAdminOutcome::UsernameTaken
        );
    }

    #[tokio::test]
    async fn the_last_admin_cant_be_demoted_or_disabled_until_there_is_another() {
        let (_temp, database, admin) = admin_session().await;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);
        let me = test_id(1);

        assert!(users(&context).await[0].is_last_admin);
        assert_eq!(
            set_role_on_all(&context, me, Some(Role::Maintainer)).await,
            UserAdminOutcome::LastAdmin
        );
        assert_eq!(
            set_role_on_all(&context, me, None).await,
            UserAdminOutcome::LastAdmin
        );
        assert_eq!(
            set_disabled(&context, me, true).await,
            UserAdminOutcome::LastAdmin
        );

        let (alex, _) = add(&context, "alex", Role::Admin).await;
        assert!(!users(&context).await.iter().any(|user| user.is_last_admin));
        assert_eq!(
            set_role_on_all(&context, me, Some(Role::Maintainer)).await,
            UserAdminOutcome::Done
        );
        // Now Alex is the last Admin.
        assert_eq!(
            set_disabled(&context, alex, true).await,
            UserAdminOutcome::LastAdmin
        );
    }

    #[tokio::test]
    async fn changing_a_role_replaces_the_grant_and_records_the_admin() {
        let (_temp, database, admin) = admin_session().await;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);
        let (sam, _) = add(&context, "sam", Role::Driver).await;

        assert_eq!(
            set_role_on_all(&context, sam, Some(Role::Maintainer)).await,
            UserAdminOutcome::Done
        );

        let grants = database.grants_for(sam).await.unwrap();
        assert_eq!(grants.len(), 1, "the old grant is gone");
        assert_eq!(grants[0].1.role, Role::Maintainer);
        let by_admin: i64 = sqlx::query_scalar(
            "SELECT count(*) FROM applied_commands WHERE user_id = ? AND active_mode = 'admin'",
        )
        .bind(test_id::<User>(1).to_bytes().to_vec())
        .fetch_one(database.pool())
        .await
        .unwrap();
        assert_eq!(by_admin, 4, "add user + grant, then delete + create");

        assert_eq!(
            set_role_on_all(&context, sam, None).await,
            UserAdminOutcome::Done
        );
        assert!(database.grants_for(sam).await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn disabling_signs_the_user_out_and_enabling_lets_them_back_in() {
        let (_temp, database, admin) = admin_session().await;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);
        let (sam, password) = add(&context, "sam", Role::Driver).await;
        let (token, _) = sign_in(
            &database,
            &LoginLimiter::new(),
            "sam",
            password.clone(),
            "b",
            NOW,
        )
        .await
        .unwrap();

        assert_eq!(
            set_disabled(&context, sam, true).await,
            UserAdminOutcome::Done
        );
        assert!(
            resolve(&database, token_hash(token.as_str()).unwrap(), NOW)
                .await
                .unwrap()
                .is_none()
        );
        let refused = sign_in(
            &database,
            &LoginLimiter::new(),
            "sam",
            password.clone(),
            "c",
            NOW,
        )
        .await;
        assert!(matches!(refused, Err(SignInError::InvalidCredentials)));

        assert_eq!(
            set_disabled(&context, sam, false).await,
            UserAdminOutcome::Done
        );
        assert!(
            sign_in(&database, &LoginLimiter::new(), "sam", password, "d", NOW)
                .await
                .is_ok()
        );
    }

    #[tokio::test]
    async fn a_reset_gives_a_new_temporary_password_and_signs_the_user_out() {
        let (_temp, database, admin) = admin_session().await;
        let clock = ManualClock::new(NOW);
        let context = context(&database, &admin, &clock);
        let (sam, first) = add(&context, "sam", Role::Driver).await;
        let (token, _) = sign_in(
            &database,
            &LoginLimiter::new(),
            "sam",
            first.clone(),
            "b",
            NOW,
        )
        .await
        .unwrap();

        let UserAdminOutcome::TemporaryPassword { password: second } =
            reset_password(&context, sam).await
        else {
            panic!("reset failed");
        };

        assert_ne!(second, first);
        assert!(
            resolve(&database, token_hash(token.as_str()).unwrap(), NOW)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            sign_in(&database, &LoginLimiter::new(), "sam", first, "c", NOW)
                .await
                .is_err()
        );
        let (_, again) = sign_in(&database, &LoginLimiter::new(), "sam", second, "d", NOW)
            .await
            .unwrap();
        assert!(again.must_change_password);
        assert_eq!(
            reset_password(&context, test_id(999)).await,
            UserAdminOutcome::NotFound
        );
    }
}
