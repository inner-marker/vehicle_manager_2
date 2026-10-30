//! The scheduled jobs against a real database, with a manual clock.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod scheduler {
    use std::path::Path;
    use std::sync::Arc;

    use domain::test_support::{ManualClock, test_id};
    use domain::time::{Clock, UnixMillis};
    use server::jobs::{Job, JobContext, SnapshotDir, next_run, run_due_jobs};
    use storage::{Database, NewDatabaseIds};

    /// 2023-11-14T22:13:20Z.
    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const DAY: i64 = 86_400_000;

    async fn context(data_dir: &Path, snapshot_dir: SnapshotDir) -> (JobContext, Arc<ManualClock>) {
        let ids = NewDatabaseIds {
            database_id: test_id(1),
            server_device_id: test_id(2),
        };
        let (database, _) = Database::open(data_dir, NOW, ids).await.unwrap();
        let clock = Arc::new(ManualClock::new(NOW));
        let context = JobContext {
            database,
            clock: Arc::clone(&clock) as Arc<dyn Clock + Send + Sync>,
            snapshot_dir,
        };
        (context, clock)
    }

    fn snapshots_in(dir: &Path) -> Vec<String> {
        let mut names: Vec<String> = std::fs::read_dir(dir)
            .map(|entries| {
                entries
                    .map(|entry| entry.unwrap().file_name().into_string().unwrap())
                    .filter(|name| name.starts_with("snapshot-"))
                    .collect()
            })
            .unwrap_or_default();
        names.sort();
        names
    }

    /// Moves the clock to `job`'s next due time and runs whatever is due.
    async fn run_at_next(context: &JobContext, clock: &ManualClock, job: Job) {
        let due = context
            .database
            .job_next_run(job.name())
            .await
            .unwrap()
            .unwrap();
        clock.set(due);
        run_due_jobs(context, due).await;
    }

    #[tokio::test]
    async fn a_new_database_schedules_jobs_without_running_them() {
        let temp = tempfile::tempdir().unwrap();
        let snapshots = temp.path().join("snapshots");
        let (context, _clock) = context(
            temp.path(),
            SnapshotDir {
                path: snapshots.clone(),
                create_if_missing: true,
            },
        )
        .await;

        run_due_jobs(&context, NOW).await;

        assert_eq!(
            context
                .database
                .job_next_run(Job::NightlySnapshot.name())
                .await
                .unwrap(),
            Some(next_run(Job::NightlySnapshot, NOW))
        );
        assert!(snapshots_in(&snapshots).is_empty());
    }

    #[tokio::test]
    async fn the_nightly_snapshot_is_written_checked_and_rescheduled() {
        let temp = tempfile::tempdir().unwrap();
        let snapshots = temp.path().join("snapshots");
        let (context, clock) = context(
            temp.path(),
            SnapshotDir {
                path: snapshots.clone(),
                create_if_missing: true,
            },
        )
        .await;
        run_due_jobs(&context, NOW).await;

        run_at_next(&context, &clock, Job::NightlySnapshot).await;

        assert_eq!(
            snapshots_in(&snapshots),
            vec!["snapshot-20231115T070000Z.sqlite"]
        );
        let next = context
            .database
            .job_next_run(Job::NightlySnapshot.name())
            .await
            .unwrap()
            .unwrap();
        assert_eq!(
            domain::time_utils::format_utc_iso8601(next),
            "2023-11-16T07:00:00.000Z"
        );
    }

    #[tokio::test]
    async fn only_the_newest_seven_snapshots_are_kept() {
        let temp = tempfile::tempdir().unwrap();
        let snapshots = temp.path().join("snapshots");
        let (context, clock) = context(
            temp.path(),
            SnapshotDir {
                path: snapshots.clone(),
                create_if_missing: true,
            },
        )
        .await;
        run_due_jobs(&context, NOW).await;

        for _ in 0..9 {
            run_at_next(&context, &clock, Job::NightlySnapshot).await;
        }

        let kept = snapshots_in(&snapshots);
        assert_eq!(kept.len(), 7);
        assert_eq!(
            kept[0], "snapshot-20231117T070000Z.sqlite",
            "the two oldest went"
        );
    }

    #[tokio::test]
    async fn a_missing_snapshot_folder_that_was_set_explicitly_is_skipped_not_created() {
        let temp = tempfile::tempdir().unwrap();
        let usb = temp.path().join("usb-not-mounted");
        let (context, clock) = context(
            temp.path(),
            SnapshotDir {
                path: usb.clone(),
                create_if_missing: false,
            },
        )
        .await;
        run_due_jobs(&context, NOW).await;

        run_at_next(&context, &clock, Job::NightlySnapshot).await;

        assert!(
            !usb.exists(),
            "never write to a folder that should be a mount"
        );
        let result: String = sqlx::query_scalar("SELECT last_result FROM jobs WHERE name = ?")
            .bind(Job::NightlySnapshot.name())
            .fetch_one(context.database.pool())
            .await
            .unwrap();
        assert_eq!(result, "skipped");
    }

    #[tokio::test]
    async fn a_snapshot_missed_while_the_server_was_off_runs_at_the_next_start() {
        let temp = tempfile::tempdir().unwrap();
        let snapshots = temp.path().join("snapshots");
        let (context, _clock) = context(
            temp.path(),
            SnapshotDir {
                path: snapshots.clone(),
                create_if_missing: true,
            },
        )
        .await;
        run_due_jobs(&context, NOW).await;

        // The server was off for three days; on starting, the overdue snapshot runs once.
        let later = UnixMillis::from_millis(NOW.as_millis() + 3 * DAY);
        run_due_jobs(&context, later).await;

        assert_eq!(snapshots_in(&snapshots).len(), 1);
    }
}
