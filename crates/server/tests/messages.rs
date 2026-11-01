//! The message center against a real database: messages derived from job results and health,
//! and marks saved through the command pipeline (#050).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod messages {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{InboxOutcome, MarkMessage};
    use domain::inbox::{Inbox, MessageKey, Severity};
    use domain::test_support::ManualClock;
    use domain::time::UnixMillis;
    use server::health::{Health, HealthProblem};
    use server::messages::{MessagesContext, mark, view};
    use server::sessions::SignedIn;
    use storage::Database;
    use storage::jobs::{JobResult, JobRun};

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> MessagesContext<'a> {
        MessagesContext {
            database,
            user,
            health: Health::Healthy,
            problem: None,
            clock,
        }
    }

    fn unwrap_inbox(outcome: InboxOutcome) -> Inbox {
        let InboxOutcome::Inbox { inbox } = outcome else {
            panic!("expected an inbox, got {outcome:?}");
        };
        inbox
    }

    /// Records a failed nightly snapshot that finished at `finished`.
    async fn failed_snapshot(database: &Database, finished: i64) {
        let finished = UnixMillis::from_millis(finished);
        database
            .record_job_run(
                "nightly-snapshot",
                &JobRun {
                    started: finished,
                    finished,
                    result: JobResult::Failed,
                    message: "can't write the snapshot: disk full".into(),
                    next_run: finished,
                },
            )
            .await
            .unwrap();
    }

    fn snapshot_key(finished: i64) -> MessageKey {
        MessageKey::parse(&format!("job-failed:nightly-snapshot:{finished}")).unwrap()
    }

    #[tokio::test]
    async fn a_new_server_has_no_messages() {
        let (_temp, database, mut stuart, _sam) = setup().await;
        stuart.mode = ActiveMode::Admin;
        let clock = ManualClock::new(NOW);
        let inbox = unwrap_inbox(view(&context(&database, &stuart, &clock)).await);
        assert!(inbox.items.is_empty());
        assert_eq!(inbox.unread, 0);
    }

    #[tokio::test]
    async fn a_failed_snapshot_shows_to_the_admin_in_admin_mode_only() {
        let (_temp, database, mut stuart, sam) = setup().await;
        failed_snapshot(&database, 5_000).await;
        let clock = ManualClock::new(NOW);

        let driver = unwrap_inbox(view(&context(&database, &stuart, &clock)).await);
        assert!(driver.items.is_empty(), "Driver mode");
        let others = unwrap_inbox(view(&context(&database, &sam, &clock)).await);
        assert!(others.items.is_empty(), "not an Admin");

        stuart.mode = ActiveMode::Admin;
        let admin = unwrap_inbox(view(&context(&database, &stuart, &clock)).await);
        assert_eq!(admin.unread, 1);
        assert_eq!(admin.items[0].message.key, snapshot_key(5_000));
        assert_eq!(
            admin.items[0].message.detail,
            "can't write the snapshot: disk full"
        );
    }

    #[tokio::test]
    async fn marks_are_saved_per_person_and_dismissed_messages_stay_hidden() {
        let (_temp, database, mut stuart, _sam) = setup().await;
        stuart.mode = ActiveMode::Admin;
        failed_snapshot(&database, 5_000).await;
        let clock = ManualClock::new(NOW);
        let ctx = context(&database, &stuart, &clock);

        let read = unwrap_inbox(mark(&ctx, snapshot_key(5_000), MarkMessage::Read).await);
        assert_eq!(read.unread, 0);
        assert!(read.items[0].read);
        // Marking it again updates the same record.
        let dismissed = unwrap_inbox(mark(&ctx, snapshot_key(5_000), MarkMessage::Dismiss).await);
        assert!(dismissed.items.is_empty());
        assert_eq!(
            database.message_marks_of(stuart.user).await.unwrap().len(),
            1
        );

        // The next night's failure is a new message.
        failed_snapshot(&database, 9_000).await;
        let later = unwrap_inbox(view(&ctx).await);
        assert_eq!(later.unread, 1);
        assert_eq!(later.items[0].message.key, snapshot_key(9_000));
    }

    #[tokio::test]
    async fn nobody_can_mark_a_message_they_cant_see() {
        let (_temp, database, stuart, sam) = setup().await;
        failed_snapshot(&database, 5_000).await;
        let clock = ManualClock::new(NOW);
        for person in [&stuart, &sam] {
            let outcome = mark(
                &context(&database, person, &clock),
                snapshot_key(5_000),
                MarkMessage::Read,
            )
            .await;
            assert_eq!(outcome, InboxOutcome::NotFound, "Admin-mode message");
        }
        let made_up = MessageKey::parse("job-failed:nightly-snapshot:1").unwrap();
        let mut admin = stuart.clone();
        admin.mode = ActiveMode::Admin;
        let outcome = mark(
            &context(&database, &admin, &clock),
            made_up,
            MarkMessage::Read,
        )
        .await;
        assert_eq!(outcome, InboxOutcome::NotFound, "no such message");
        assert!(
            database
                .message_marks_of(stuart.user)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn an_unhealthy_server_tells_everyone_and_refuses_marks() {
        let (_temp, database, _stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let ctx = MessagesContext {
            health: Health::Unhealthy(HealthProblem::IntegrityCheckFailed),
            problem: Some((
                HealthProblem::IntegrityCheckFailed,
                UnixMillis::from_millis(7),
            )),
            ..context(&database, &sam, &clock)
        };
        let inbox = unwrap_inbox(view(&ctx).await);
        assert_eq!(inbox.items.len(), 1);
        let message = &inbox.items[0].message;
        assert_eq!(message.severity, Severity::Urgent);
        assert!(message.detail.is_empty(), "details are for Admin mode");

        let outcome = mark(&ctx, message.key.clone(), MarkMessage::Dismiss).await;
        assert_eq!(outcome, InboxOutcome::Unavailable, "changes are refused");
    }
}
