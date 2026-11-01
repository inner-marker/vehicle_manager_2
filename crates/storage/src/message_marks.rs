//! The `message_marks` table: each person's read and dismissed marks on messages (#050), and
//! the job results messages are derived from.

use domain::id::Id;
use domain::inbox::{
    JobOutcome, JobReport, MessageKey, MessageMark, MessageMarkId, MessageMarkPatch,
    MessageMarkRecord,
};
use domain::time::UnixMillis;
use domain::user::UserId;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `message_marks` table for the command pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct MessageMarkWriter;

impl RecordWriter<MessageMarkRecord> for MessageMarkWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<MessageMarkRecord>,
        values: &MessageMarkPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let key = required(values.key.as_ref(), "key")?;
        sqlx::query(
            "INSERT INTO message_marks (id, user_id, key, read, dismissed, sequence)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.user, "user_id")?))
        .bind(key.as_str())
        .bind(values.read.unwrap_or(false))
        .bind(values.dismissed.unwrap_or(false))
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<MessageMarkRecord>,
        patch: &MessageMarkPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE message_marks SET
                 user_id = coalesce(?, user_id),
                 key = coalesce(?, key),
                 read = coalesce(?, read),
                 dismissed = coalesce(?, dismissed),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.user.map(id_to_sql))
        .bind(patch.key.as_ref().map(MessageKey::as_str))
        .bind(patch.read)
        .bind(patch.dismissed)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<MessageMarkRecord>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE message_marks SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn mark_from_row(row: &SqliteRow) -> Result<(MessageMarkId, MessageMark), StorageError> {
    let key: String = row.try_get("key")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "message_marks.id")?,
        MessageMark {
            user: id_from_sql(row.try_get::<&[u8], _>("user_id")?, "message_marks.user_id")?,
            key: MessageKey::parse(&key).ok_or(StorageError::InvalidValue {
                column: "message_marks.key",
            })?,
            read: row.try_get("read")?,
            dismissed: row.try_get("dismissed")?,
        },
    ))
}

fn job_from_row(row: &SqliteRow) -> Result<Option<JobReport>, StorageError> {
    let result: Option<String> = row.try_get("last_result")?;
    let finished: Option<i64> = row.try_get("last_finished_ms")?;
    let outcome = match result.as_deref() {
        Some("ok") => JobOutcome::Ok,
        Some("skipped") => JobOutcome::Skipped,
        Some("failed") => JobOutcome::Failed,
        // Never run (only scheduled), or a result this build doesn't know.
        _ => return Ok(None),
    };
    let Some(finished) = finished else {
        return Ok(None);
    };
    Ok(Some(JobReport {
        name: row.try_get("name")?,
        outcome,
        finished: UnixMillis::from_millis(finished),
        detail: row
            .try_get::<Option<String>, _>("last_message")?
            .unwrap_or_default(),
    }))
}

impl Database {
    /// Every mark `user` has made (not deleted), with its record ID.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails; [`StorageError::InvalidValue`] for a
    /// malformed row.
    pub async fn message_marks_of(
        &self,
        user: UserId,
    ) -> Result<Vec<(MessageMarkId, MessageMark)>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, key, read, dismissed FROM message_marks
             WHERE user_id = ? AND deleted = 0",
        )
        .bind(id_to_sql(user))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(mark_from_row)
        .collect()
    }

    /// Each job's latest run, for deriving messages (jobs that have only been scheduled are
    /// left out).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn job_reports(&self) -> Result<Vec<JobReport>, StorageError> {
        let rows = sqlx::query(
            "SELECT name, last_finished_ms, last_result, last_message FROM jobs ORDER BY name",
        )
        .fetch_all(self.pool())
        .await?;
        let reports = rows
            .iter()
            .map(job_from_row)
            .collect::<Result<Vec<_>, _>>()?;
        Ok(reports.into_iter().flatten().collect())
    }
}
