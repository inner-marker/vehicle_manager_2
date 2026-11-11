//! Users, grants, passwords, and sessions against a real database.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod users {
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::device::DeviceKind;
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::commands::RecordWriter;
    use storage::devices::StoredDevice;
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

    /// A browser device of `user`'s, added at `NOW`.
    fn browser(id: domain::id::DeviceId, user: domain::user::UserId) -> StoredDevice {
        StoredDevice {
            id,
            user,
            kind: DeviceKind::Browser,
            name: None,
            public_key: None,
            added_at: NOW,
            last_seen: NOW,
            signed_out_at: None,
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

    /// Asset `n`, in a group, so a grant can name it (grants have a foreign key to assets).
    async fn create_truck(database: &Database, n: u32) {
        use domain::asset::{Asset, AssetKind, AssetName, AssetPatch, LifecycleStatus};
        use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
        use storage::assets::{AssetWriter, GroupWriter};

        let mut connection = database.pool().acquire().await.unwrap();
        let group = Group {
            name: GroupName::parse("Garage").unwrap(),
            icon: GroupIcon::Garage,
        };
        GroupWriter
            .create(
                &mut connection,
                test_id(n + 1000),
                &GroupPatch::all(group),
                1,
            )
            .await
            .unwrap();
        let truck = Asset {
            kind: AssetKind::parse("truck").unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: None,
            model: None,
            year: None,
            colour: None,
            description: domain::notes::Notes::default(),
            group: test_id(n + 1000),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers: vec![],
        };
        AssetWriter
            .create(&mut connection, test_id(n), &AssetPatch::all(truck), 1)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn grants_on_all_assets_and_one_asset_round_trip() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        create_truck(&database, 50).await;
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
        database
            .insert_device_with_session(&browser(test_id(300), test_id(1)), &session)
            .await
            .unwrap();
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
            let device = test_id(300 + u32::from(n));
            database
                .insert_device_with_session(
                    &browser(device, test_id(1)),
                    &Session {
                        token_hash: [n; 32],
                        user: test_id(1),
                        device,
                        created_at: NOW,
                        last_used: NOW,
                        expires_at: UnixMillis::from_millis(NOW.as_millis() + 1_000),
                        active_mode: ActiveMode::Driver,
                    },
                )
                .await
                .unwrap();
        }

        assert_eq!(database.delete_sessions_of(test_id(1)).await.unwrap(), 3);
        assert_eq!(database.session([0; 32]).await.unwrap(), None);
    }

    /// A session on `device`, ending at `ends`.
    fn session_on(token: u8, device: domain::id::DeviceId, ends: UnixMillis) -> Session {
        Session {
            token_hash: [token; 32],
            user: test_id(1),
            device,
            created_at: NOW,
            last_used: NOW,
            expires_at: ends,
            active_mode: ActiveMode::Driver,
        }
    }

    #[tokio::test]
    async fn a_desktop_device_and_its_first_session_are_stored_together() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let desktop = StoredDevice {
            kind: DeviceKind::Desktop,
            name: Some(domain::device::DeviceName::parse("Stuarts-MacBook-Pro").unwrap()),
            public_key: Some([9; 32]),
            ..browser(test_id(310), test_id(1))
        };
        let ends = UnixMillis::from_millis(NOW.as_millis() + 1_000);
        database
            .insert_device_with_session(&desktop, &session_on(1, desktop.id, ends))
            .await
            .unwrap();
        assert_eq!(
            database.device(desktop.id).await.unwrap(),
            Some(desktop.clone())
        );
        assert_eq!(
            database.session([1; 32]).await.unwrap().unwrap().device,
            desktop.id
        );
        assert_eq!(database.device(test_id(311)).await.unwrap(), None);
    }

    #[tokio::test]
    async fn a_session_needs_its_device_and_a_failed_sign_in_stores_neither() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let ends = UnixMillis::from_millis(NOW.as_millis() + 1_000);
        assert!(
            database
                .insert_session(&session_on(1, test_id(320), ends))
                .await
                .is_err(),
            "no such device"
        );
        // A device for a user who doesn't exist: the device fails, so the session isn't kept.
        let stray = browser(test_id(321), test_id(99));
        let mut session = session_on(2, stray.id, ends);
        session.user = test_id(99);
        assert!(
            database
                .insert_device_with_session(&stray, &session)
                .await
                .is_err()
        );
        assert_eq!(database.session([2; 32]).await.unwrap(), None);
        assert_eq!(database.device(stray.id).await.unwrap(), None);
    }

    #[tokio::test]
    async fn signed_in_devices_are_those_with_a_live_session_newest_first() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let soon = UnixMillis::from_millis(NOW.as_millis() + 1_000);
        let later = UnixMillis::from_millis(NOW.as_millis() + 9_000);
        for (token, n, ends) in [(1, 330, soon), (2, 331, later), (3, 332, later)] {
            database
                .insert_device_with_session(
                    &browser(test_id(n), test_id(1)),
                    &session_on(token, test_id(n), ends),
                )
                .await
                .unwrap();
        }
        // Using 331's session makes it the most recent.
        let used = UnixMillis::from_millis(NOW.as_millis() + 500);
        database.touch_session([2; 32], used, later).await.unwrap();
        let ids = |devices: Vec<StoredDevice>| devices.iter().map(|d| d.id).collect::<Vec<_>>();
        assert_eq!(
            ids(database
                .signed_in_devices_of(test_id(1), NOW)
                .await
                .unwrap()),
            vec![test_id(331), test_id(330), test_id(332)]
        );
        assert_eq!(
            database
                .device(test_id(331))
                .await
                .unwrap()
                .unwrap()
                .last_seen,
            used
        );
        // At `soon`, 330's only session has ended.
        assert_eq!(
            ids(database
                .signed_in_devices_of(test_id(1), soon)
                .await
                .unwrap()),
            vec![test_id(331), test_id(332)]
        );
    }

    #[tokio::test]
    async fn a_device_is_renamed_and_signed_out_once() {
        let (_temp, database) = database().await;
        create_user(&database, 1, stuart()).await;
        let ends = UnixMillis::from_millis(NOW.as_millis() + 9_000);
        let device = browser(test_id(340), test_id(1));
        database
            .insert_device_with_session(&device, &session_on(1, device.id, ends))
            .await
            .unwrap();
        let name = domain::device::DeviceName::parse("Safari on iPhone").unwrap();
        database.rename_device(device.id, &name).await.unwrap();
        assert_eq!(
            database.device(device.id).await.unwrap().unwrap().name,
            Some(name)
        );

        let first = UnixMillis::from_millis(NOW.as_millis() + 100);
        let second = UnixMillis::from_millis(NOW.as_millis() + 200);
        assert_eq!(database.sign_out_device(device.id, first).await.unwrap(), 1);
        assert_eq!(
            database.sign_out_device(device.id, second).await.unwrap(),
            0
        );
        let stored = database.device(device.id).await.unwrap().unwrap();
        assert_eq!(stored.signed_out_at, Some(first), "stamped once");
        assert_eq!(database.session([1; 32]).await.unwrap(), None);
        assert!(
            database
                .signed_in_devices_of(test_id(1), NOW)
                .await
                .unwrap()
                .is_empty()
        );
    }
}
