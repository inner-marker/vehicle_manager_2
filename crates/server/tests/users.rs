//! Users and grants through the real command pipeline.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod pipeline {
    use domain::access::{GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::command::{COMMAND_FORMAT_VERSION, Change, Command, CommandKind};
    use domain::hlc::{Hlc, Stamp};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use server::commands::{Outcome, apply};
    use server::health::Health;
    use storage::users::{RoleGrantWriter, UserWriter};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    fn stamp(offset: i64) -> Stamp {
        Stamp {
            hlc: Hlc::new(UnixMillis::from_millis(NOW.as_millis() + offset), 0).unwrap(),
            device: test_id(500),
        }
    }

    fn command<R: domain::command::Record>(n: u32, change: Change<R>) -> Command<R> {
        Command {
            id: test_id::<CommandKind>(n),
            stamp: stamp(i64::from(n)),
            format_version: COMMAND_FORMAT_VERSION,
            change,
        }
    }

    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        (temp, database)
    }

    #[tokio::test]
    async fn a_user_and_their_grant_are_created_and_edited_through_commands() {
        let (_temp, database) = database().await;
        let clock = ManualClock::new(NOW);
        let user = User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: true,
        };
        let grant = RoleGrant {
            user: test_id(1),
            target: GrantTarget::AllAssets,
            role: Role::Admin,
        };
        let rename = UserPatch {
            display_name: Some("Stu".into()),
            ..UserPatch::default()
        };

        let outcomes = [
            apply(
                &database,
                &UserWriter,
                &command(
                    1,
                    Change::Create {
                        id: test_id(1),
                        values: UserPatch::all(user),
                    },
                ),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap(),
            apply(
                &database,
                &RoleGrantWriter,
                &command(
                    2,
                    Change::Create {
                        id: test_id(2),
                        values: RoleGrantPatch::all(grant),
                    },
                ),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap(),
            apply(
                &database,
                &UserWriter,
                &command(
                    3,
                    Change::Update {
                        id: test_id(1),
                        patch: rename,
                    },
                ),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap(),
        ];

        assert!(
            outcomes
                .iter()
                .all(|outcome| matches!(outcome, Outcome::Applied { .. })),
            "{outcomes:?}"
        );
        let stored = database.user(test_id(1)).await.unwrap().unwrap();
        assert_eq!(stored.user.display_name, "Stu");
        assert!(stored.user.must_change_password);
        assert_eq!(
            database.grants_for(test_id(1)).await.unwrap(),
            vec![(test_id(2), grant)]
        );
    }
}
