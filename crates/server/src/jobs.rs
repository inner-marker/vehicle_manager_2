//! The server's scheduled jobs (decision #044): one small scheduler for all timed work.
//!
//! Jobs so far:
//! - **Nightly snapshot** at [`SNAPSHOT_HOUR_UTC`]:00 UTC: `VACUUM INTO` the snapshot
//!   folder, integrity-check the copy, keep the newest [`SNAPSHOTS_KEPT`] (#035).
//! - **Hourly quick check:** `PRAGMA quick_check`; problems make the server unhealthy
//!   until it's restarted and looked at.
//!
//! Each run is logged and recorded in the `jobs` table (last run, result, next run). A job
//! that was due while the server was off runs soon after it starts.
//!
//! The timing logic ([`next_run`], [`run_due_jobs`]) takes "now" as a value so tests control
//! time; only [`spawn`]'s loop sleeps for real.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use domain::log_area;
use domain::time::{Clock, UnixMillis};
use storage::Database;
use storage::jobs::{JobResult, JobRun};
use storage::snapshot::{rotate_snapshots, verify_snapshot};

use crate::health::{self, HealthProblem};

/// The nightly snapshot's hour, in UTC: 07:00 UTC is 03:00 US Eastern daylight time
/// (02:00 standard). Local-time scheduling waits for the time-zone library (milestone 6).
pub const SNAPSHOT_HOUR_UTC: i64 = 7;

/// How many nightly snapshots are kept (#035).
pub const SNAPSHOTS_KEPT: usize = 7;

const MILLIS_PER_HOUR: i64 = 3_600_000;
const MILLIS_PER_DAY: i64 = 24 * MILLIS_PER_HOUR;

/// How long the scheduler waits when nothing is due soon, so a changed clock (e.g. the NAS
/// syncing its time) is noticed within the hour.
const MAX_SLEEP: Duration = Duration::from_hours(1);

/// The jobs the server runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Job {
    /// The nightly `VACUUM INTO` snapshot.
    NightlySnapshot,
    /// The hourly `PRAGMA quick_check`.
    QuickCheck,
}

impl Job {
    /// Every job, in the order they run when due together.
    pub const ALL: [Self; 2] = [Self::NightlySnapshot, Self::QuickCheck];

    /// Its name in the `jobs` table and the log. Never changes once released.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NightlySnapshot => "nightly-snapshot",
            Self::QuickCheck => "quick-check",
        }
    }

    const fn schedule(self) -> Schedule {
        match self {
            Self::NightlySnapshot => Schedule::DailyAtUtcHour(SNAPSHOT_HOUR_UTC),
            Self::QuickCheck => Schedule::Every(MILLIS_PER_HOUR),
        }
    }
}

/// When a job repeats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Schedule {
    /// Every this many milliseconds.
    Every(i64),
    /// Once a day at this hour, UTC.
    DailyAtUtcHour(i64),
}

/// The first time after `after` that `job` is due.
#[must_use]
pub fn next_run(job: Job, after: UnixMillis) -> UnixMillis {
    let after = after.as_millis();
    let next = match job.schedule() {
        Schedule::Every(interval) => after.saturating_add(interval),
        Schedule::DailyAtUtcHour(hour) => {
            let today = after - after.rem_euclid(MILLIS_PER_DAY) + hour * MILLIS_PER_HOUR;
            if today > after {
                today
            } else {
                today + MILLIS_PER_DAY
            }
        }
    };
    UnixMillis::from_millis(next)
}

/// Where nightly snapshots go.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SnapshotDir {
    /// The folder.
    pub path: PathBuf,
    /// `true` for the default `<data dir>/snapshots`, which is created if missing. A folder
    /// set with `VEHICLEMANAGER_SNAPSHOT_DIR` is never created: if it's missing (e.g. a USB
    /// disk not mounted), the snapshot is skipped rather than written to the wrong disk.
    pub create_if_missing: bool,
}

/// What jobs need.
#[derive(Clone)]
pub struct JobContext {
    /// The live database.
    pub database: Database,
    /// The real clock (tests pass a manual one).
    pub clock: Arc<dyn Clock + Send + Sync>,
    /// Where snapshots go.
    pub snapshot_dir: SnapshotDir,
}

/// Runs every job due at `now`, in [`Job::ALL`] order, and records each run. A job never
/// scheduled before (a new database) is scheduled now rather than run at once.
///
/// Never fails as a whole: a job's failure is recorded as its result, and a failure to
/// record it is logged.
pub async fn run_due_jobs(context: &JobContext, now: UnixMillis) {
    for job in Job::ALL {
        let due = match context.database.job_next_run(job.name()).await {
            Ok(Some(due)) => due,
            Ok(None) => {
                schedule_first_run(context, job, now).await;
                continue;
            }
            Err(error) => {
                tracing::error!(target: log_area::DATABASE, job = job.name(), %error, "can't read the job schedule");
                continue;
            }
        };
        if due <= now {
            run_and_record(context, job).await;
        }
    }
}

/// The earliest time any job is due, for the scheduler's sleep.
async fn earliest_due(context: &JobContext) -> Option<UnixMillis> {
    let mut earliest = None;
    for job in Job::ALL {
        if let Ok(Some(due)) = context.database.job_next_run(job.name()).await {
            earliest = Some(earliest.map_or(due, |current: UnixMillis| current.min(due)));
        }
    }
    earliest
}

/// Starts the scheduler on the current tokio runtime. Call once, after startup.
pub fn spawn(context: JobContext) {
    tokio::spawn(async move {
        loop {
            run_due_jobs(&context, context.clock.now()).await;
            let wait = earliest_due(&context).await.map_or(MAX_SLEEP, |due| {
                let millis = due
                    .as_millis()
                    .saturating_sub(context.clock.now().as_millis());
                Duration::from_millis(u64::try_from(millis).unwrap_or(0)).min(MAX_SLEEP)
            });
            tokio::time::sleep(wait).await;
        }
    });
}

async fn schedule_first_run(context: &JobContext, job: Job, now: UnixMillis) {
    let run = JobRun {
        started: now,
        finished: now,
        result: JobResult::Skipped,
        message: "scheduled".to_owned(),
        next_run: next_run(job, now),
    };
    if let Err(error) = context.database.record_job_run(job.name(), &run).await {
        tracing::error!(target: log_area::DATABASE, job = job.name(), %error, "can't record the job schedule");
    }
}

async fn run_and_record(context: &JobContext, job: Job) {
    let started = context.clock.now();
    tracing::debug!(target: log_area::DATABASE, job = job.name(), "job starting");
    let (result, message) = match job {
        Job::NightlySnapshot => snapshot(context, started).await,
        Job::QuickCheck => quick_check(context).await,
    };
    let finished = context.clock.now();
    let run = JobRun {
        started,
        finished,
        result,
        message,
        next_run: next_run(job, finished),
    };
    log_result(job, &run);
    if let Err(error) = context.database.record_job_run(job.name(), &run).await {
        tracing::error!(target: log_area::DATABASE, job = job.name(), %error, "can't record the job run");
    }
}

fn log_result(job: Job, run: &JobRun) {
    let name = job.name();
    let message = run.message.as_str();
    match run.result {
        JobResult::Ok => {
            tracing::info!(target: log_area::DATABASE, job = name, message, "job finished");
        }
        JobResult::Skipped => {
            tracing::warn!(target: log_area::DATABASE, job = name, message, "job skipped");
        }
        JobResult::Failed => {
            tracing::error!(target: log_area::DATABASE, job = name, message, "job failed");
        }
    }
}

/// The nightly snapshot: write, verify, rotate.
async fn snapshot(context: &JobContext, now: UnixMillis) -> (JobResult, String) {
    let dir = &context.snapshot_dir;
    if !dir.path.is_dir() {
        if !dir.create_if_missing {
            return (
                JobResult::Skipped,
                format!(
                    "snapshot folder {} is missing (not mounted?)",
                    dir.path.display()
                ),
            );
        }
        if let Err(error) = std::fs::create_dir_all(&dir.path) {
            return (
                JobResult::Failed,
                format!("can't create {}: {error}", dir.path.display()),
            );
        }
    }

    let path = match context.database.snapshot_into(&dir.path, now).await {
        Ok(path) => path,
        Err(error) => {
            return (
                JobResult::Failed,
                format!("can't write the snapshot: {error}"),
            );
        }
    };
    match verify_snapshot(&path).await {
        Ok(problems) if problems.is_empty() => {}
        Ok(problems) => {
            return (
                JobResult::Failed,
                format!(
                    "snapshot {} failed its check: {}",
                    path.display(),
                    problems.join("; ")
                ),
            );
        }
        Err(error) => {
            return (
                JobResult::Failed,
                format!("can't check {}: {error}", path.display()),
            );
        }
    }
    match rotate_snapshots(&dir.path, SNAPSHOTS_KEPT) {
        Ok(removed) => (
            JobResult::Ok,
            format!("wrote {}; removed {} older", path.display(), removed.len()),
        ),
        Err(error) => (
            JobResult::Failed,
            format!(
                "wrote {}, but couldn't remove older snapshots: {error}",
                path.display()
            ),
        ),
    }
}

/// The hourly quick check. A problem makes the server unhealthy.
async fn quick_check(context: &JobContext) -> (JobResult, String) {
    match context.database.quick_check().await {
        Ok(problems) if problems.is_empty() => (JobResult::Ok, "no problems".to_owned()),
        Ok(problems) => {
            health::set(health::Health::Unhealthy(
                HealthProblem::IntegrityCheckFailed,
            ));
            (
                JobResult::Failed,
                format!("problems found: {}", problems.join("; ")),
            )
        }
        Err(error) => {
            health::set(health::Health::Unhealthy(
                HealthProblem::IntegrityCheckFailed,
            ));
            (JobResult::Failed, format!("couldn't run: {error}"))
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 2023-11-14T22:13:20Z.
    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    fn utc(millis: i64) -> String {
        domain::time_utils::format_utc_iso8601(UnixMillis::from_millis(millis))
    }

    #[test]
    fn the_nightly_snapshot_is_next_due_at_seven_utc() {
        // 22:13 is past 07:00, so the next one is tomorrow.
        assert_eq!(
            utc(next_run(Job::NightlySnapshot, NOW).as_millis()),
            "2023-11-15T07:00:00.000Z"
        );
        // At 06:59 it's later the same day; exactly at 07:00 it's the next day.
        let early = NOW.as_millis() - NOW.as_millis().rem_euclid(MILLIS_PER_DAY)
            + 7 * MILLIS_PER_HOUR
            - 60_000;
        assert_eq!(
            utc(next_run(Job::NightlySnapshot, UnixMillis::from_millis(early)).as_millis()),
            "2023-11-14T07:00:00.000Z"
        );
        let exactly = early + 60_000;
        assert_eq!(
            utc(next_run(Job::NightlySnapshot, UnixMillis::from_millis(exactly)).as_millis()),
            "2023-11-15T07:00:00.000Z"
        );
    }

    #[test]
    fn the_quick_check_repeats_hourly() {
        assert_eq!(
            next_run(Job::QuickCheck, NOW).as_millis(),
            NOW.as_millis() + MILLIS_PER_HOUR
        );
    }

    #[test]
    fn job_names_are_stable() {
        assert_eq!(Job::NightlySnapshot.name(), "nightly-snapshot");
        assert_eq!(Job::QuickCheck.name(), "quick-check");
    }
}
