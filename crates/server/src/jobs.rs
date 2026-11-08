//! The server's scheduled jobs (decision #044): one small scheduler for all timed work.
//!
//! Jobs so far:
//! - **Nightly snapshot** at [`SNAPSHOT_HOUR_UTC`]:00 UTC: `VACUUM INTO` the snapshot
//!   folder, integrity-check the copy, keep the newest [`SNAPSHOTS_KEPT`] (#035).
//! - **Hourly quick check:** `PRAGMA quick_check`; problems make the server unhealthy
//!   until it's restarted and looked at.
//! - **Finalize drafts** every [`FINALIZE_EVERY_MINUTES`] minutes: fuel-ups and readings
//!   last edited 24 hours ago (or the test minutes) are finalized by the server (#034).
//! - **File check** at [`FILE_CHECK_HOUR_UTC`]:00 UTC: every photo and document file is checked
//!   against its SHA-256, and files nothing uses are erased once no kept snapshot could need
//!   them ([`keep_unused_files_for`], #065). A missing or damaged file fails the run, which the
//!   Admin sees in Messages.
//!
//! Each run is logged and recorded in the `jobs` table (last run, result, next run). A job
//! that was due while the server was off runs soon after it starts.
//!
//! The timing logic ([`next_run`], [`run_due_jobs`]) takes "now" as a value so tests control
//! time; only [`spawn`]'s loop sleeps for real.

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use domain::id::DeviceId;
use domain::log_area;
use domain::time::{Clock, UnixMillis};
use domain::timers::Timers;
use storage::Database;
use storage::files::{FileCheck, FileStore};
use storage::jobs::{JobResult, JobRun};
use storage::snapshot::{rotate_snapshots, verify_snapshot};

use crate::health::{self, HealthProblem};

/// The nightly snapshot's hour, in UTC: 07:00 UTC is 03:00 US Eastern daylight time
/// (02:00 standard). Local-time scheduling waits for the time-zone library (milestone 6).
pub const SNAPSHOT_HOUR_UTC: i64 = 7;

/// The file check's hour, in UTC: an hour after the snapshot.
pub const FILE_CHECK_HOUR_UTC: i64 = 8;

/// The file check's stable name (also used to bring it forward at startup).
pub const FILE_CHECK_JOB: &str = "file-check";

/// How many nightly snapshots are kept (#035).
pub const SNAPSHOTS_KEPT: usize = 7;

/// How often the server finalizes drafts that are due, so one is finalized at most this
/// late.
pub const FINALIZE_EVERY_MINUTES: i64 = 10;

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
    /// Finalizing drafts 24 hours after their last edit.
    FinalizeDrafts,
    /// Checking the photo and document files, and erasing unused ones.
    FileCheck,
}

impl Job {
    /// Every job, in the order they run when due together.
    pub const ALL: [Self; 4] = [
        Self::NightlySnapshot,
        Self::QuickCheck,
        Self::FinalizeDrafts,
        Self::FileCheck,
    ];

    /// Its name in the `jobs` table and the log. Never changes once released.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::NightlySnapshot => "nightly-snapshot",
            Self::QuickCheck => "quick-check",
            Self::FinalizeDrafts => "finalize-drafts",
            Self::FileCheck => FILE_CHECK_JOB,
        }
    }

    const fn schedule(self) -> Schedule {
        match self {
            Self::NightlySnapshot => Schedule::DailyAtUtcHour(SNAPSHOT_HOUR_UTC),
            Self::QuickCheck => Schedule::Every(MILLIS_PER_HOUR),
            Self::FinalizeDrafts => Schedule::Every(FINALIZE_EVERY_MINUTES * 60_000),
            Self::FileCheck => Schedule::DailyAtUtcHour(FILE_CHECK_HOUR_UTC),
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

/// The first time after `after` that `job` is due. With test timers (#054), every job is due
/// every N minutes instead of on its own schedule.
#[must_use]
pub fn next_run(job: Job, after: UnixMillis, timers: Timers) -> UnixMillis {
    let after = after.as_millis();
    let schedule = timers
        .job_interval_millis()
        .map_or_else(|| job.schedule(), Schedule::Every);
    let next = match schedule {
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
    /// Normal timers, or test timers (every job every N minutes, #054).
    pub timers: Timers,
    /// The server's own device, for the changes it makes (finalizing drafts).
    pub server_device: DeviceId,
    /// The photo and document files (none if the folder couldn't be opened).
    pub file_store: Option<FileStore>,
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
    // Only the due time: the job hasn't run, so it has no result (and no message, #050).
    if let Err(error) = context
        .database
        .schedule_job(job.name(), next_run(job, now, context.timers))
        .await
    {
        tracing::error!(target: log_area::DATABASE, job = job.name(), %error, "can't record the job schedule");
    }
}

async fn run_and_record(context: &JobContext, job: Job) {
    let started = context.clock.now();
    tracing::debug!(target: log_area::DATABASE, job = job.name(), "job starting");
    let (result, message) = match job {
        Job::NightlySnapshot => snapshot(context, started).await,
        Job::QuickCheck => quick_check(context).await,
        Job::FinalizeDrafts => finalize_drafts(context).await,
        Job::FileCheck => file_check(context).await,
    };
    let finished = context.clock.now();
    let run = JobRun {
        started,
        finished,
        result,
        message,
        next_run: next_run(job, finished, context.timers),
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

/// Finalizes the drafts that are due.
async fn finalize_drafts(context: &JobContext) -> (JobResult, String) {
    match crate::events::finalize_due(
        &context.database,
        context.server_device,
        health::current(),
        context.clock.as_ref(),
        context.timers,
    )
    .await
    {
        Ok(count) => (JobResult::Ok, format!("finalized {count}")),
        Err(message) => (JobResult::Failed, message),
    }
}

/// How long a file nothing uses is kept: past the oldest kept snapshot, so restoring any of
/// them finds its photos and documents (#065). A day longer than the snapshots go back, or
/// with test timers, as many test intervals.
#[must_use]
pub fn keep_unused_files_for(timers: Timers) -> i64 {
    let interval = timers.job_interval_millis().unwrap_or(MILLIS_PER_DAY);
    let kept = i64::try_from(SNAPSHOTS_KEPT).unwrap_or(i64::MAX);
    kept.saturating_add(1).saturating_mul(interval)
}

/// At most this many problem files are named in a run's message (the log has them all).
const FILES_NAMED: usize = 5;

/// The nightly file check: every listed file against its hash, then erasing unused ones.
async fn file_check(context: &JobContext) -> (JobResult, String) {
    let Some(store) = context.file_store.clone() else {
        return (
            JobResult::Failed,
            "the files folder couldn't be opened".to_owned(),
        );
    };
    let listed = match context.database.stored_files().await {
        Ok(listed) => listed,
        Err(error) => return (JobResult::Failed, format!("can't list the files: {error}")),
    };
    let count = listed.len();
    let checking = store.clone();
    let checked = tokio::task::spawn_blocking(move || {
        listed
            .into_iter()
            .map(|file| {
                let found = checking.check(&file.hash);
                (file, found)
            })
            .collect::<Vec<_>>()
    })
    .await;
    let Ok(checked) = checked else {
        return (JobResult::Failed, "the check stopped".to_owned());
    };
    let mut problems = Vec::new();
    for (file, found) in checked {
        let problem = match found {
            Ok(FileCheck::Good) => continue,
            Ok(FileCheck::Missing) => "missing",
            Ok(FileCheck::Damaged) => "damaged",
            Err(_) => "unreadable",
        };
        tracing::error!(target: log_area::DATABASE, file = file.hash.as_str(), problem, "stored file problem");
        problems.push(format!("{} {problem}", &file.hash.as_str()[..12]));
    }
    let erased = erase_unused(context, &store).await;
    match (problems.is_empty(), erased) {
        (true, Ok(erased)) => (
            JobResult::Ok,
            format!("checked {count} files; erased {erased} unused"),
        ),
        (false, erased) => {
            let named: Vec<&str> = problems
                .iter()
                .take(FILES_NAMED)
                .map(String::as_str)
                .collect();
            (
                JobResult::Failed,
                format!(
                    "{} of {count} files have a problem: {}{}; {}",
                    problems.len(),
                    named.join(", "),
                    if problems.len() > FILES_NAMED {
                        ", …"
                    } else {
                        ""
                    },
                    erased.map_or_else(|error| error, |erased| format!("erased {erased} unused")),
                ),
            )
        }
        (true, Err(error)) => (JobResult::Failed, error),
    }
}

/// Erases the files nothing needs any more, and how many.
async fn erase_unused(context: &JobContext, store: &FileStore) -> Result<usize, String> {
    let before = UnixMillis::from_millis(
        context
            .clock
            .now()
            .as_millis()
            .saturating_sub(keep_unused_files_for(context.timers)),
    );
    let unused = context
        .database
        .unused_files(before)
        .await
        .map_err(|error| format!("can't find unused files: {error}"))?;
    let mut erased = 0;
    for hash in unused {
        let erasing = store.clone();
        let file = hash.clone();
        match tokio::task::spawn_blocking(move || erasing.erase(&file)).await {
            Ok(Ok(())) => {}
            Ok(Err(error)) => return Err(format!("can't erase a file: {error}")),
            Err(_) => return Err("erasing stopped".to_owned()),
        }
        context
            .database
            .forget_file(&hash)
            .await
            .map_err(|error| format!("can't forget an erased file: {error}"))?;
        erased += 1;
    }
    Ok(erased)
}

/// At startup: if any listed file is missing, brings the file check forward so the Admin hears
/// about it within the minute rather than the next morning. Hashing every file is left to the
/// check itself.
pub async fn check_files_exist(database: &Database, store: &FileStore, now: UnixMillis) {
    let Ok(listed) = database.stored_files().await else {
        return;
    };
    let checking = store.clone();
    let missing = tokio::task::spawn_blocking(move || {
        listed
            .iter()
            .filter(|file| !checking.path(&file.hash).exists())
            .count()
    })
    .await
    .unwrap_or(0);
    if missing > 0 {
        tracing::warn!(target: log_area::DATABASE, missing, "stored files are missing; checking them now");
        if let Err(error) = database.bring_job_forward(FILE_CHECK_JOB, now).await {
            tracing::error!(target: log_area::DATABASE, %error, "can't bring the file check forward");
        }
    }
}

/// The hourly quick check. A problem makes the server unhealthy.
async fn quick_check(context: &JobContext) -> (JobResult, String) {
    match context.database.quick_check().await {
        Ok(problems) if problems.is_empty() => (JobResult::Ok, "no problems".to_owned()),
        Ok(problems) => {
            health::set(
                health::Health::Unhealthy(HealthProblem::IntegrityCheckFailed),
                context.clock.now(),
            );
            (
                JobResult::Failed,
                format!("problems found: {}", problems.join("; ")),
            )
        }
        Err(error) => {
            health::set(
                health::Health::Unhealthy(HealthProblem::IntegrityCheckFailed),
                context.clock.now(),
            );
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
    fn test_timers_run_every_job_every_n_minutes() {
        let timers = Timers::test(3).unwrap();
        for job in Job::ALL {
            assert_eq!(
                next_run(job, NOW, timers).as_millis(),
                NOW.as_millis() + 3 * 60_000,
                "{job:?}"
            );
        }
    }

    #[test]
    fn the_nightly_snapshot_is_next_due_at_seven_utc() {
        // 22:13 is past 07:00, so the next one is tomorrow.
        assert_eq!(
            utc(next_run(Job::NightlySnapshot, NOW, Timers::NORMAL).as_millis()),
            "2023-11-15T07:00:00.000Z"
        );
        // At 06:59 it's later the same day; exactly at 07:00 it's the next day.
        let early = NOW.as_millis() - NOW.as_millis().rem_euclid(MILLIS_PER_DAY)
            + 7 * MILLIS_PER_HOUR
            - 60_000;
        assert_eq!(
            utc(next_run(
                Job::NightlySnapshot,
                UnixMillis::from_millis(early),
                Timers::NORMAL
            )
            .as_millis()),
            "2023-11-14T07:00:00.000Z"
        );
        let exactly = early + 60_000;
        assert_eq!(
            utc(next_run(
                Job::NightlySnapshot,
                UnixMillis::from_millis(exactly),
                Timers::NORMAL
            )
            .as_millis()),
            "2023-11-15T07:00:00.000Z"
        );
    }

    #[test]
    fn the_quick_check_repeats_hourly() {
        assert_eq!(
            next_run(Job::QuickCheck, NOW, Timers::NORMAL).as_millis(),
            NOW.as_millis() + MILLIS_PER_HOUR
        );
    }

    #[test]
    fn job_names_are_stable() {
        assert_eq!(Job::NightlySnapshot.name(), "nightly-snapshot");
        assert_eq!(Job::QuickCheck.name(), "quick-check");
        assert_eq!(Job::FinalizeDrafts.name(), "finalize-drafts");
    }
}
