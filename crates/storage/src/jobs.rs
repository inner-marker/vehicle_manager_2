//! The `jobs` table: when each scheduled job last ran, how it went, and when it runs next
//! (decision #044).

use domain::time::UnixMillis;
use sqlx::Row;

use crate::{Database, StorageError};

/// How a job run ended.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JobResult {
    /// It did its work.
    Ok,
    /// It had nothing to do, or couldn't run for an expected reason (e.g. no snapshot folder).
    Skipped,
    /// It failed.
    Failed,
}

impl JobResult {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Ok => "ok",
            Self::Skipped => "skipped",
            Self::Failed => "failed",
        }
    }
}

/// One finished run, to record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRun {
    /// When it started.
    pub started: UnixMillis,
    /// When it finished.
    pub finished: UnixMillis,
    /// How it ended.
    pub result: JobResult,
    /// A short description for the Admin (never secrets).
    pub message: String,
    /// When it runs next.
    pub next_run: UnixMillis,
}

impl Database {
    /// When `job` is next due, if it has ever been scheduled.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn job_next_run(&self, job: &str) -> Result<Option<UnixMillis>, StorageError> {
        let row = sqlx::query("SELECT next_run_ms FROM jobs WHERE name = ?")
            .bind(job)
            .fetch_optional(self.pool())
            .await?;
        Ok(row
            .map(|row| row.try_get::<Option<i64>, _>("next_run_ms"))
            .transpose()?
            .flatten()
            .map(UnixMillis::from_millis))
    }

    /// Records a finished run of `job` and its next due time.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn record_job_run(&self, job: &str, run: &JobRun) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO jobs (name, last_started_ms, last_finished_ms, last_result, last_message, next_run_ms)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (name) DO UPDATE SET
                 last_started_ms = excluded.last_started_ms,
                 last_finished_ms = excluded.last_finished_ms,
                 last_result = excluded.last_result,
                 last_message = excluded.last_message,
                 next_run_ms = excluded.next_run_ms",
        )
        .bind(job)
        .bind(run.started.as_millis())
        .bind(run.finished.as_millis())
        .bind(run.result.as_str())
        .bind(&run.message)
        .bind(run.next_run.as_millis())
        .execute(self.pool())
        .await?;
        Ok(())
    }
}
