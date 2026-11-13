//! Offline changes against a real database (phase-3.md 5b, decision #076; migration 0019):
//! the desktop's outbox and clock, and the server's record of pushed operations.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod outbox {
    use domain::api::{EventsOutcome, SquawksOutcome};
    use domain::hlc::Hlc;
    use domain::operation::{Operation, OperationChange, Refusal};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::commands::RecordWriter;
    use storage::outbox::{AppliedOperation, OutboxState, OutboxSummary};
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const STUART: u32 = 1;
    const MAC: u32 = 300;

    /// A database with one person and their desktop.
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        let user = User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: false,
        };
        UserWriter
            .create(&mut connection, test_id(STUART), &UserPatch::all(user), 1)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO devices (id, user_id, kind, added_at_ms, last_seen_ms)
             VALUES (?, ?, 'desktop', 0, 0)",
        )
        .bind(test_id::<()>(MAC).to_bytes().to_vec())
        .bind(test_id::<()>(STUART).to_bytes().to_vec())
        .execute(&mut *connection)
        .await
        .unwrap();
        drop(connection);
        (temp, database)
    }

    fn hlc(millis: i64) -> Hlc {
        Hlc::new(UnixMillis::from_millis(millis), 0).unwrap()
    }

    fn operation(n: u32) -> Operation {
        Operation::new(
            test_id(n),
            hlc(NOW.as_millis() + i64::from(n)),
            OperationChange::DeleteEvent {
                id: test_id(n + 1000),
            },
        )
    }

    fn at(offset: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + offset)
    }

    #[tokio::test]
    async fn the_outbox_keeps_changes_in_order_until_acknowledged() {
        let (_temp, database) = database().await;
        assert_eq!(
            database.outbox_summary().await.unwrap(),
            OutboxSummary::default()
        );
        for n in [10, 11, 12] {
            database
                .add_to_outbox(&operation(n), at(i64::from(n)))
                .await
                .unwrap();
        }
        assert!(
            database
                .add_to_outbox(&operation(10), at(99))
                .await
                .is_err(),
            "the same operation is never queued twice"
        );
        let waiting = database.outbox_waiting(10).await.unwrap();
        assert_eq!(waiting, vec![operation(10), operation(11), operation(12)]);
        assert_eq!(database.outbox_waiting(2).await.unwrap().len(), 2);

        // Acknowledged: gone; the next is now the oldest.
        assert!(database.remove_from_outbox(operation(10).id).await.unwrap());
        assert!(!database.remove_from_outbox(operation(10).id).await.unwrap());
        assert_eq!(
            database.outbox_summary().await.unwrap(),
            OutboxSummary {
                waiting: 2,
                refused: 0,
                oldest_waiting: Some(at(11)),
            }
        );
        // A change made later still goes after the others.
        database.add_to_outbox(&operation(9), at(20)).await.unwrap();
        let order: Vec<_> = database
            .outbox_waiting(10)
            .await
            .unwrap()
            .into_iter()
            .map(|operation| operation.id)
            .collect();
        assert_eq!(
            order,
            vec![operation(11).id, operation(12).id, operation(9).id]
        );
    }

    #[tokio::test]
    async fn a_refused_change_stays_with_its_reason_until_discarded() {
        let (_temp, database) = database().await;
        database.add_to_outbox(&operation(10), at(1)).await.unwrap();
        database.add_to_outbox(&operation(11), at(2)).await.unwrap();
        let refusal = Refusal::Squawks(SquawksOutcome::NotAllowed);
        assert!(
            database
                .refuse_in_outbox(operation(10).id, &refusal, at(5))
                .await
                .unwrap()
        );
        assert!(
            !database
                .refuse_in_outbox(operation(10).id, &refusal, at(6))
                .await
                .unwrap(),
            "only a waiting change is refused"
        );
        // No longer sent; still listed, with why.
        assert_eq!(
            database.outbox_waiting(10).await.unwrap(),
            vec![operation(11)]
        );
        let entry = database
            .outbox_entry(operation(10).id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entry.made_at, at(1));
        assert_eq!(entry.state, OutboxState::Refused { refusal, at: at(5) });
        assert_eq!(database.outbox_entries().await.unwrap().len(), 2);
        assert_eq!(
            database.outbox_summary().await.unwrap(),
            OutboxSummary {
                waiting: 1,
                refused: 1,
                oldest_waiting: Some(at(2)),
            }
        );
        assert!(database.remove_from_outbox(operation(10).id).await.unwrap());
        assert_eq!(database.outbox_summary().await.unwrap().refused, 0);
    }

    #[tokio::test]
    async fn the_desktops_clock_only_moves_forward() {
        let (_temp, database) = database().await;
        assert_eq!(database.device_hlc().await.unwrap(), Hlc::ZERO);
        database.advance_device_hlc(hlc(2_000)).await.unwrap();
        database.advance_device_hlc(hlc(1_000)).await.unwrap();
        assert_eq!(database.device_hlc().await.unwrap(), hlc(2_000));
        database.advance_device_hlc(hlc(3_000)).await.unwrap();
        assert_eq!(database.device_hlc().await.unwrap(), hlc(3_000));
    }

    #[tokio::test]
    async fn a_pushed_operations_first_answer_stands() {
        let (_temp, database) = database().await;
        let applied = AppliedOperation {
            id: operation(10).id,
            device: test_id(MAC),
            user: test_id(STUART),
            hlc: operation(10).hlc,
            format_version: 1,
            refusal: None,
            received_at: at(1),
        };
        assert_eq!(database.operation_answer(applied.id).await.unwrap(), None);
        database.record_operation(&applied).await.unwrap();
        assert_eq!(
            database.operation_answer(applied.id).await.unwrap(),
            Some(None)
        );
        // Recorded again (a resend), refused this time: the first answer stands.
        let again = AppliedOperation {
            refusal: Some(Refusal::Events(EventsOutcome::NotAllowed)),
            ..applied.clone()
        };
        database.record_operation(&again).await.unwrap();
        assert_eq!(
            database.operation_answer(applied.id).await.unwrap(),
            Some(None)
        );

        let refused = AppliedOperation {
            id: operation(11).id,
            refusal: Some(Refusal::Events(EventsOutcome::NotFound)),
            ..applied
        };
        database.record_operation(&refused).await.unwrap();
        assert_eq!(
            database.operation_answer(refused.id).await.unwrap(),
            Some(Some(Refusal::Events(EventsOutcome::NotFound)))
        );
    }

    #[tokio::test]
    async fn a_device_with_its_clock_ahead_is_flagged_until_cleared() {
        let (_temp, database) = database().await;
        assert!(
            database
                .devices_with_clock_ahead()
                .await
                .unwrap()
                .is_empty()
        );
        database
            .flag_clock_ahead(test_id(MAC), 3_600_000, at(7))
            .await
            .unwrap();
        let flagged = database.devices_with_clock_ahead().await.unwrap();
        assert_eq!(flagged.len(), 1);
        assert_eq!(flagged[0].device, test_id(MAC));
        assert_eq!(flagged[0].user, test_id(STUART));
        assert_eq!(flagged[0].ahead_millis, 3_600_000);
        assert_eq!(flagged[0].at, at(7));
        database.clear_clock_ahead(test_id(MAC)).await.unwrap();
        assert!(
            database
                .devices_with_clock_ahead()
                .await
                .unwrap()
                .is_empty()
        );
    }
}
