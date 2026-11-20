//! Message marks' snoozes and the household's starting step for due messages, against a real
//! database (Phase 5 milestone 3, migration 0024; decision #083).

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod messages {
    use domain::command::Clearable;
    use domain::due::DueStep;
    use domain::inbox::{MessageKey, MessageMarkPatch, Snooze, SnoozeMeter};
    use domain::preferences::{Choice, PreferencesPatch};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::commands::RecordWriter;
    use storage::message_marks::MessageMarkWriter;
    use storage::preferences::PreferencesWriter;
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const HOUSEHOLD: u32 = 900;
    const JORDAN: u32 = 1;
    const MARK: u32 = 40;

    /// A database with Jordan in it.
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(HOUSEHOLD),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let jordan = User {
            username: Username::parse("jordan").unwrap(),
            display_name: "Jordan".into(),
            disabled: false,
            must_change_password: false,
        };
        let mut connection = database.pool().acquire().await.unwrap();
        UserWriter
            .create(&mut connection, test_id(JORDAN), &UserPatch::all(jordan), 1)
            .await
            .unwrap();
        drop(connection);
        (temp, database)
    }

    async fn snooze_of(database: &Database) -> Option<Snooze> {
        let marks = database.message_marks_of(test_id(JORDAN)).await.unwrap();
        assert_eq!(marks.len(), 1);
        marks[0].1.snooze
    }

    #[tokio::test]
    async fn a_marks_snooze_is_kept_and_cleared() {
        let (_temp, database) = database().await;
        let snooze = Snooze {
            until: Some(UnixMillis::from_millis(1_700_600_000_000)),
            meter: Some(SnoozeMeter {
                meter: test_id(10),
                at: 135_000_000,
            }),
        };
        let mut connection = database.pool().acquire().await.unwrap();
        let created = MessageMarkPatch {
            user: Some(test_id(JORDAN)),
            key: MessageKey::parse("due:requirement:x:first"),
            read: Some(true),
            dismissed: None,
            snooze: Some(Clearable::Set(snooze)),
        };
        MessageMarkWriter
            .create(&mut connection, test_id(MARK), &created, 2)
            .await
            .unwrap();
        assert_eq!(snooze_of(&database).await, Some(snooze));

        // Read and dismissed changes leave it alone; Wake now clears it.
        let dismissed = MessageMarkPatch {
            dismissed: Some(true),
            ..MessageMarkPatch::default()
        };
        MessageMarkWriter
            .update(&mut connection, test_id(MARK), &dismissed, 3)
            .await
            .unwrap();
        assert_eq!(snooze_of(&database).await, Some(snooze));
        let woken = MessageMarkPatch {
            snooze: Some(Clearable::Clear),
            ..MessageMarkPatch::default()
        };
        MessageMarkWriter
            .update(&mut connection, test_id(MARK), &woken, 4)
            .await
            .unwrap();
        assert_eq!(snooze_of(&database).await, None);

        // One this build can't read counts as none, so the message shows.
        sqlx::query("UPDATE message_marks SET snooze = '{\"until\":\"soon\"}'")
            .execute(&mut *connection)
            .await
            .unwrap();
        assert_eq!(snooze_of(&database).await, None);
    }

    #[tokio::test]
    async fn the_households_starting_step_is_kept_and_put_back() {
        let (_temp, database) = database().await;
        let household = test_id(HOUSEHOLD);
        assert_eq!(
            database
                .preference_choices(household)
                .await
                .unwrap()
                .due_messages_from,
            None,
            "not chosen"
        );
        let mut connection = database.pool().acquire().await.unwrap();
        let chosen = PreferencesPatch {
            due_messages_from: Some(Choice::Set(DueStep::DueVerySoon)),
            ..PreferencesPatch::default()
        };
        PreferencesWriter
            .create(&mut connection, household, &chosen, 2)
            .await
            .unwrap();
        let stored: Option<String> =
            sqlx::query_scalar("SELECT due_messages_from FROM preferences")
                .fetch_one(&mut *connection)
                .await
                .unwrap();
        assert_eq!(stored.as_deref(), Some("due-very-soon"), "by name");
        assert_eq!(
            database
                .preference_choices(household)
                .await
                .unwrap()
                .due_messages_from,
            Some(DueStep::DueVerySoon)
        );
        let back = PreferencesPatch {
            due_messages_from: Some(Choice::Default),
            ..PreferencesPatch::default()
        };
        PreferencesWriter
            .update(&mut connection, household, &back, 3)
            .await
            .unwrap();
        assert_eq!(
            database
                .preference_choices(household)
                .await
                .unwrap()
                .due_messages_from,
            None
        );
    }
}
