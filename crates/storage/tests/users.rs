//! Users, grants, passwords, and sessions against a real database.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod users {
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::commands::RecordWriter;
    use storage::users::{RoleGrantWriter, Session, UserWriter};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        (temp, database)
    }

    fn stuart() -> User {
        User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: false,
        }
    }

    async fn create_user(database: &Database, n: u32, user: User) {
        let mut connection = database.pool().acquire().await.unwrap();
        UserWriter
            .create(&mut connection, test_id(n), &UserPatch::all(user), 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_user_is_created_found_updated_and_soft_deleted() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;

        let found = database
            .user_by_username(&Username::parse("STUART").unwrap())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(found.id, test_id(1));
        assert_eq!(found.user, stuart());
        assert_eq!(database.user_count().await.unwrap(), 1);

        let mut connection = database.pool().acquire().await.unwrap();
        let patch = UserPatch {
            display_name: Some("Stu".into()),
            ..UserPatch::default()
        };
        UserWriter
            .update(&mut connection, test_id(1), &patch, 2)
            .await
            .unwrap();
        assert_eq!(
            database
                .user(test_id(1))
                .await
                .unwrap()
                .unwrap()
                .user
                .display_name,
            "Stu"
        );
        assert_eq!(
            database
                .user(test_id(1))
                .await
                .unwrap()
                .unwrap()
                .user
                .username
                .as_str(),
            "stuart",
            "unset fields stay"
        );

        UserWriter
            .delete(&mut connection, test_id(1), 3)
            .await
            .unwrap();
        drop(connection);
        assert_eq!(database.user(test_id(1)).await.unwrap(), None);
        assert_eq!(database.user_count().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn usernames_are_unique_but_a_deleted_users_name_can_be_reused() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let mut connection = database.pool().acquire().await.unwrap();

        let duplicate = UserWriter
            .create(&mut connection, test_id(2), &UserPatch::all(stuart()), 2)
            .await;
        assert!(duplicate.is_err(), "two users can't share a username");

        UserWriter
            .delete(&mut connection, test_id(1), 3)
            .await
            .unwrap();
        UserWriter
            .create(&mut connection, test_id(2), &UserPatch::all(stuart()), 4)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn password_hashes_are_stored_apart_and_replaced() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        assert_eq!(database.password_hash(test_id(1)).await.unwrap(), None);

        database
            .set_password_hash(test_id(1), "$argon2id$first", NOW)
            .await
            .unwrap();
        database
            .set_password_hash(test_id(1), "$argon2id$second", NOW)
            .await
            .unwrap();

        assert_eq!(
            database.password_hash(test_id(1)).await.unwrap().as_deref(),
            Some("$argon2id$second")
        );
    }

    #[tokio::test]
    async fn grants_on_all_assets_and_one_asset_round_trip() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let everywhere = RoleGrant {
            user: test_id(1),
            target: GrantTarget::AllAssets,
            role: Role::Admin,
        };
        let truck = RoleGrant {
            user: test_id(1),
            target: GrantTarget::Asset(test_id(50)),
            role: Role::Maintainer,
        };
        let mut connection = database.pool().acquire().await.unwrap();
        RoleGrantWriter
            .create(
                &mut connection,
                test_id(10),
                &RoleGrantPatch::all(everywhere),
                2,
            )
            .await
            .unwrap();
        RoleGrantWriter
            .create(&mut connection, test_id(11), &RoleGrantPatch::all(truck), 3)
            .await
            .unwrap();

        let mut grants: Vec<RoleGrant> = database
            .grants_for(test_id(1))
            .await
            .unwrap()
            .into_iter()
            .map(|(_, grant)| grant)
            .collect();
        grants.sort_by_key(|grant| grant.role);
        assert_eq!(grants, vec![truck, everywhere]);

        RoleGrantWriter
            .delete(&mut connection, test_id(10), 4)
            .await
            .unwrap();
        assert_eq!(database.all_grants().await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn a_grant_for_a_missing_user_is_refused() {
        let (_temp, database) = database().await;
        let orphan = RoleGrant {
            user: test_id(77),
            target: GrantTarget::AllAssets,
            role: Role::Viewer,
        };
        let mut connection = database.pool().acquire().await.unwrap();

        let result = RoleGrantWriter
            .create(
                &mut connection,
                test_id(10),
                &RoleGrantPatch::all(orphan),
                1,
            )
            .await;

        assert!(result.is_err(), "foreign keys are on");
    }

    #[tokio::test]
    async fn sessions_are_found_extended_switched_and_ended() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let session = Session {
            token_hash: [7; 32],
            user: test_id(1),
            device: test_id(300),
            created_at: NOW,
            last_used: NOW,
            expires_at: UnixMillis::from_millis(NOW.as_millis() + 1_000),
            active_mode: ActiveMode::Driver,
        };
        database.insert_session(&session).await.unwrap();
        assert_eq!(
            database.session([7; 32]).await.unwrap(),
            Some(session.clone())
        );
        assert_eq!(database.session([8; 32]).await.unwrap(), None);

        let later = UnixMillis::from_millis(NOW.as_millis() + 500);
        let new_end = UnixMillis::from_millis(NOW.as_millis() + 5_000);
        database
            .touch_session([7; 32], later, new_end)
            .await
            .unwrap();
        database
            .set_session_mode([7; 32], ActiveMode::Admin)
            .await
            .unwrap();
        let updated = database.session([7; 32]).await.unwrap().unwrap();
        assert_eq!(
            (updated.last_used, updated.expires_at, updated.active_mode),
            (later, new_end, ActiveMode::Admin)
        );

        assert_eq!(database.delete_expired_sessions(later).await.unwrap(), 0);
        assert_eq!(database.delete_expired_sessions(new_end).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn signing_a_user_out_everywhere_ends_all_their_sessions() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        for n in 0..3 {
            database
                .insert_session(&Session {
                    token_hash: [n; 32],
                    user: test_id(1),
                    device: test_id(300 + u32::from(n)),
                    created_at: NOW,
                    last_used: NOW,
                    expires_at: UnixMillis::from_millis(NOW.as_millis() + 1_000),
                    active_mode: ActiveMode::Driver,
                })
                .await
                .unwrap();
        }

        assert_eq!(database.delete_sessions_of(test_id(1)).await.unwrap(), 3);
        assert_eq!(database.session([0; 32]).await.unwrap(), None);
    }
}
