//! The command engine against a real database, with a test-only "note" record.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod engine {
    use domain::command::{COMMAND_FORMAT_VERSION, Change, Command, CommandId, Patch, Record};
    use domain::hlc::{Hlc, Stamp};
    use domain::id::{DeviceId, Id};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use server::commands::{ApplyError, NoChange, Outcome, Rejection, apply};
    use server::health::{Health, HealthProblem};
    use sqlx::SqliteConnection;
    use storage::commands::{self as bookkeeping, RecordWriter};
    use storage::convert::id_to_sql;
    use storage::{Database, NewDatabaseIds, StorageError};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    // --- A test-only record: a note with a title and a body ---------------------------------

    #[derive(Debug)]
    struct Note;

    #[derive(Debug, Clone, Default, PartialEq)]
    struct NotePatch {
        title: Option<String>,
        body: Option<String>,
    }

    impl Patch for NotePatch {
        fn changed_fields(&self) -> Vec<&'static str> {
            [
                ("title", self.title.is_some()),
                ("body", self.body.is_some()),
            ]
            .into_iter()
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
        }

        fn keep_only(&mut self, fields: &[&str]) {
            if !fields.contains(&"title") {
                self.title = None;
            }
            if !fields.contains(&"body") {
                self.body = None;
            }
        }
    }

    impl Record for Note {
        const ENTITY: &'static str = "test_note";
        type Patch = NotePatch;
    }

    /// Writes `test_notes`. A title of "fail" makes the write fail, to test rollback.
    struct NoteWriter;

    impl RecordWriter<Note> for NoteWriter {
        async fn create(
            &self,
            connection: &mut SqliteConnection,
            id: Id<Note>,
            values: &NotePatch,
            sequence: i64,
        ) -> Result<(), StorageError> {
            fail_on_request(values)?;
            sqlx::query("INSERT INTO test_notes (id, title, body, sequence) VALUES (?, ?, ?, ?)")
                .bind(id_to_sql(id))
                .bind(&values.title)
                .bind(&values.body)
                .bind(sequence)
                .execute(connection)
                .await?;
            Ok(())
        }

        async fn update(
            &self,
            connection: &mut SqliteConnection,
            id: Id<Note>,
            patch: &NotePatch,
            sequence: i64,
        ) -> Result<(), StorageError> {
            fail_on_request(patch)?;
            sqlx::query(
                "UPDATE test_notes SET title = coalesce(?, title), body = coalesce(?, body), sequence = ?
                 WHERE id = ?",
            )
            .bind(&patch.title)
            .bind(&patch.body)
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
            Ok(())
        }

        async fn delete(
            &self,
            connection: &mut SqliteConnection,
            id: Id<Note>,
            sequence: i64,
        ) -> Result<(), StorageError> {
            sqlx::query("UPDATE test_notes SET deleted = 1, sequence = ? WHERE id = ?")
                .bind(sequence)
                .bind(id_to_sql(id))
                .execute(connection)
                .await?;
            Ok(())
        }
    }

    fn fail_on_request(patch: &NotePatch) -> Result<(), StorageError> {
        if patch.title.as_deref() == Some("fail") {
            return Err(StorageError::InvalidValue { column: "title" });
        }
        Ok(())
    }

    // --- Helpers -----------------------------------------------------------------------------

    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        sqlx::query(
            "CREATE TABLE test_notes (
                 id BLOB PRIMARY KEY, title TEXT, body TEXT,
                 sequence INTEGER NOT NULL, deleted INTEGER NOT NULL DEFAULT 0
             ) STRICT",
        )
        .execute(database.pool())
        .await
        .unwrap();
        (temp, database)
    }

    fn laptop() -> DeviceId {
        test_id(100)
    }

    fn phone() -> DeviceId {
        test_id(200)
    }

    /// A stamp `offset_millis` after NOW, from `device`.
    fn at(offset_millis: i64, device: DeviceId) -> Stamp {
        Stamp {
            hlc: Hlc::new(UnixMillis::from_millis(NOW.as_millis() + offset_millis), 0).unwrap(),
            device,
        }
    }

    fn command(n: u32, stamp: Stamp, change: Change<Note>) -> Command<Note> {
        Command {
            id: test_id::<domain::command::CommandKind>(n),
            stamp,
            format_version: COMMAND_FORMAT_VERSION,
            change,
        }
    }

    fn note() -> Id<Note> {
        test_id(1)
    }

    fn set(title: Option<&str>, body: Option<&str>) -> NotePatch {
        NotePatch {
            title: title.map(str::to_owned),
            body: body.map(str::to_owned),
        }
    }

    fn create(n: u32, stamp: Stamp, title: &str, body: &str) -> Command<Note> {
        command(
            n,
            stamp,
            Change::Create {
                id: note(),
                values: set(Some(title), Some(body)),
            },
        )
    }

    fn update(n: u32, stamp: Stamp, patch: NotePatch) -> Command<Note> {
        command(n, stamp, Change::Update { id: note(), patch })
    }

    async fn run(database: &Database, command: &Command<Note>) -> Result<Outcome, ApplyError> {
        apply(
            database,
            &NoteWriter,
            command,
            Health::Healthy,
            &ManualClock::new(NOW),
        )
        .await
    }

    async fn stored(database: &Database) -> (Option<String>, Option<String>, i64, bool) {
        sqlx::query_as("SELECT title, body, sequence, deleted FROM test_notes WHERE id = ?")
            .bind(id_to_sql(note()))
            .fetch_one(database.pool())
            .await
            .unwrap()
    }

    async fn applied_count(database: &Database) -> i64 {
        sqlx::query_scalar("SELECT count(*) FROM applied_commands")
            .fetch_one(database.pool())
            .await
            .unwrap()
    }

    // --- Tests -------------------------------------------------------------------------------

    #[tokio::test]
    async fn a_create_writes_the_record_with_the_first_sequence_number() {
        let (_temp, database) = database().await;

        let outcome = run(&database, &create(1, at(0, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();

        assert_eq!(
            outcome,
            Outcome::Applied {
                sequence: 1,
                applied: vec!["title", "body"],
                superseded: vec![],
            }
        );
        assert_eq!(
            stored(&database).await,
            (Some("Oil".into()), Some("5W-30".into()), 1, false)
        );
    }

    #[tokio::test]
    async fn a_retried_command_is_never_applied_twice() {
        let (_temp, database) = database().await;
        let first = create(1, at(0, laptop()), "Oil", "5W-30");
        run(&database, &first).await.unwrap();

        assert_eq!(
            run(&database, &first).await.unwrap(),
            Outcome::AlreadyApplied
        );
        assert_eq!(stored(&database).await.2, 1, "no second sequence number");
        assert_eq!(applied_count(&database).await, 1);
    }

    #[tokio::test]
    async fn edits_to_different_fields_merge_whatever_order_they_arrive_in() {
        let (_temp, database) = database().await;
        run(&database, &create(1, at(0, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();

        // The laptop changes the title at +2 s; the phone, offline, changed the body at +1 s
        // and syncs later. Per-record last-write-wins would drop the phone's edit.
        run(
            &database,
            &update(2, at(2_000, laptop()), set(Some("Oil change"), None)),
        )
        .await
        .unwrap();
        let late = run(
            &database,
            &update(3, at(1_000, phone()), set(None, Some("0W-20"))),
        )
        .await
        .unwrap();

        assert!(matches!(late, Outcome::Applied { ref applied, .. } if applied == &vec!["body"]));
        let (title, body, ..) = stored(&database).await;
        assert_eq!(
            (title.as_deref(), body.as_deref()),
            (Some("Oil change"), Some("0W-20"))
        );
    }

    #[tokio::test]
    async fn an_older_edit_to_the_same_field_loses_and_the_rest_of_it_still_applies() {
        let (_temp, database) = database().await;
        run(&database, &create(1, at(0, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();
        run(
            &database,
            &update(2, at(2_000, laptop()), set(Some("Newest title"), None)),
        )
        .await
        .unwrap();

        let outcome = run(
            &database,
            &update(
                3,
                at(1_000, phone()),
                set(Some("Older title"), Some("0W-20")),
            ),
        )
        .await
        .unwrap();

        assert_eq!(
            outcome,
            Outcome::Applied {
                sequence: 3,
                applied: vec!["body"],
                superseded: vec!["title"],
            }
        );
        let (title, body, ..) = stored(&database).await;
        assert_eq!(
            (title.as_deref(), body.as_deref()),
            (Some("Newest title"), Some("0W-20"))
        );
    }

    #[tokio::test]
    async fn an_edit_older_than_every_field_changes_nothing_and_gets_no_sequence() {
        let (_temp, database) = database().await;
        run(&database, &create(1, at(5_000, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();

        let outcome = run(
            &database,
            &update(2, at(1_000, phone()), set(Some("Old"), None)),
        )
        .await
        .unwrap();

        assert_eq!(outcome, Outcome::NoChange(NoChange::Superseded));
        assert_eq!(stored(&database).await.2, 1);
        assert_eq!(
            applied_count(&database).await,
            2,
            "still recorded, so a retry does nothing"
        );
    }

    #[tokio::test]
    async fn equal_stamps_are_decided_by_device_id_the_same_way_every_time() {
        let (_temp, database) = database().await;
        run(&database, &create(1, at(0, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();

        // Same HLC from both devices: the higher device ID (the phone's) wins, in either order.
        run(
            &database,
            &update(2, at(1_000, phone()), set(Some("Phone"), None)),
        )
        .await
        .unwrap();
        let laptop_edit = run(
            &database,
            &update(3, at(1_000, laptop()), set(Some("Laptop"), None)),
        )
        .await
        .unwrap();

        assert_eq!(laptop_edit, Outcome::NoChange(NoChange::Superseded));
        assert_eq!(stored(&database).await.0.as_deref(), Some("Phone"));
    }

    #[tokio::test]
    async fn a_deleted_record_is_never_changed_again() {
        let (_temp, database) = database().await;
        run(&database, &create(1, at(0, laptop()), "Oil", "5W-30"))
            .await
            .unwrap();

        let deleted = run(
            &database,
            &command(2, at(1_000, laptop()), Change::Delete { id: note() }),
        )
        .await
        .unwrap();
        let newer_edit = run(
            &database,
            &update(3, at(9_000, phone()), set(Some("Back?"), None)),
        )
        .await
        .unwrap();

        assert!(matches!(deleted, Outcome::Applied { sequence: 2, .. }));
        assert_eq!(newer_edit, Outcome::NoChange(NoChange::RecordDeleted));
        let (title, _, sequence, is_deleted) = stored(&database).await;
        assert_eq!(
            (title.as_deref(), sequence, is_deleted),
            (Some("Oil"), 2, true)
        );
        let mut connection = database.pool().acquire().await.unwrap();
        assert!(
            bookkeeping::tombstone(&mut connection, "test_note", note().to_bytes())
                .await
                .unwrap()
                .is_some()
        );
    }

    #[tokio::test]
    async fn refused_commands_write_nothing() {
        let (_temp, database) = database().await;
        let clock = ManualClock::new(NOW);

        let unhealthy = apply(
            &database,
            &NoteWriter,
            &create(1, at(0, laptop()), "Oil", "5W-30"),
            Health::Unhealthy(HealthProblem::IntegrityCheckFailed),
            &clock,
        )
        .await;
        let mut future_format = create(2, at(0, laptop()), "Oil", "5W-30");
        future_format.format_version = COMMAND_FORMAT_VERSION + 1;
        let unknown_format = run(&database, &future_format).await;
        let far_ahead = run(
            &database,
            &create(3, at(10 * 60 * 1_000, laptop()), "Oil", "5W-30"),
        )
        .await;

        assert!(matches!(
            unhealthy,
            Err(ApplyError::Rejected(Rejection::ServerUnhealthy))
        ));
        assert!(matches!(
            unknown_format,
            Err(ApplyError::Rejected(Rejection::UnsupportedFormat(2)))
        ));
        assert!(matches!(
            far_ahead,
            Err(ApplyError::Rejected(Rejection::ClockTooFarAhead {
                ahead_millis: 600_000
            }))
        ));
        assert_eq!(applied_count(&database).await, 0);
    }

    #[tokio::test]
    async fn a_failed_write_rolls_back_everything() {
        let (_temp, database) = database().await;

        let result = run(&database, &create(1, at(0, laptop()), "fail", "x")).await;

        assert!(matches!(result, Err(ApplyError::Storage(_))));
        assert_eq!(applied_count(&database).await, 0, "not recorded as applied");
        let sequence: i64 = sqlx::query_scalar("SELECT last_sequence FROM database_meta")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(sequence, 0, "the sequence number was given back");
        let field_clocks: i64 = sqlx::query_scalar("SELECT count(*) FROM field_clocks")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(field_clocks, 0);
    }

    #[tokio::test]
    async fn the_server_clock_moves_past_every_stamp_it_accepts() {
        let (_temp, database) = database().await;
        let incoming = at(60_000, phone());

        run(&database, &create(1, incoming, "Oil", "5W-30"))
            .await
            .unwrap();

        let mut connection = database.pool().acquire().await.unwrap();
        let server_hlc = bookkeeping::server_last_hlc(&mut connection).await.unwrap();
        assert!(server_hlc > incoming.hlc);
    }

    #[tokio::test]
    async fn command_ids_are_their_own_kind() {
        // Compile-time check that the test helper makes command IDs, not note IDs.
        let id: CommandId = test_id(1);
        assert_eq!(
            id,
            command(1, at(0, laptop()), Change::Delete { id: note() }).id
        );
    }

    // --- Property: order doesn't matter --------------------------------------------------------

    mod convergence {
        use proptest::prelude::*;

        use super::*;

        /// One edit: which device, how many ms after NOW, and which fields it sets.
        fn edit() -> impl Strategy<Value = (bool, i64, Option<u8>, Option<u8>)> {
            (
                any::<bool>(),
                0_i64..5_000,
                any::<Option<u8>>(),
                any::<Option<u8>>(),
            )
        }

        fn commands(edits: &[(bool, i64, Option<u8>, Option<u8>)]) -> Vec<Command<Note>> {
            let mut commands = vec![create(1, at(0, laptop()), "start", "start")];
            // Each edit is its own command (its own ID), with values naming the edit so the
            // final state shows which edit won each field.
            for (index, (on_phone, offset, title, body)) in edits.iter().enumerate() {
                let n = u32::try_from(index).unwrap() + 2;
                let device = if *on_phone { phone() } else { laptop() };
                let patch = NotePatch {
                    title: title.map(|value| format!("t{value}")),
                    body: body.map(|value| format!("b{value}")),
                };
                // A real device never issues the same HLC twice (step 1a), so each edit's stamp is
                // made unique with the edit's index as the HLC counter. Without that, two edits
                // from one device with equal stamps would tie, and arrival order would decide.
                let mut stamp = at(1 + offset, device);
                stamp.hlc = Hlc::new(stamp.hlc.physical(), u16::try_from(index).unwrap()).unwrap();
                commands.push(update(n, stamp, patch));
            }
            commands
        }

        async fn final_state(order: &[Command<Note>]) -> (Option<String>, Option<String>) {
            let (_temp, database) = database().await;
            for command in order {
                run(&database, command).await.unwrap();
            }
            let (title, body, ..) = stored(&database).await;
            database.close().await;
            (title, body)
        }

        proptest! {
            // Each case opens two databases, so keep the count modest.
            #![proptest_config(ProptestConfig::with_cases(48))]

            #[test]
            fn any_order_and_any_retries_reach_the_same_state(
                edits in prop::collection::vec(edit(), 0..12),
                shuffle_seed in any::<u64>(),
            ) {
                let in_order = commands(&edits);
                // The create stays first (its record must exist before edits in this test); the
                // edits arrive shuffled, and every command is sent twice, as if retried.
                let mut shuffled = in_order[1..].to_vec();
                shuffle(&mut shuffled, shuffle_seed);
                let mut arrival = vec![in_order[0].clone()];
                for command in shuffled {
                    arrival.push(command.clone());
                    arrival.push(command);
                }

                let runtime = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
                let expected = runtime.block_on(final_state(&in_order));
                let actual = runtime.block_on(final_state(&arrival));
                prop_assert_eq!(actual, expected);
            }
        }

        /// A small deterministic shuffle (Fisher–Yates with an xorshift generator), so a
        /// failing case is reproducible from its seed without a random-number crate.
        fn shuffle<T>(items: &mut [T], seed: u64) {
            let mut state = seed | 1;
            for index in (1..items.len()).rev() {
                state ^= state << 13;
                state ^= state >> 7;
                state ^= state << 17;
                let swap_with = usize::try_from(state % (index as u64 + 1)).unwrap();
                items.swap(index, swap_with);
            }
        }
    }
}
