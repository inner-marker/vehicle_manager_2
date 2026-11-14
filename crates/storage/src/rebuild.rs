//! Rebuilding a fresh server from a desktop's copy (phase-3.md 7b, decision #078; migration
//! 0021): the bookkeeping. `server::rebuild` decides who may and when.

use domain::attachment::{AttachmentId, Content, FileHash};
use domain::id::DatabaseId;
use domain::time::UnixMillis;
use sqlx::Row;

use crate::convert::{id_from_sql, id_to_sql, json_to_sql};
use crate::{Database, StorageError};

/// Where a rebuild stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RebuildState {
    /// The server whose copy it's rebuilt from.
    pub from: DatabaseId,
    /// When it started.
    pub started: UnixMillis,
    /// When the records were in (the people's passwords set).
    pub finished: Option<UnixMillis>,
    /// When its files were settled.
    pub settled: Option<UnixMillis>,
}

impl Database {
    /// Whether this server is fresh: no people and no vehicles (only then can it be rebuilt).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails.
    pub async fn is_fresh(&self) -> Result<bool, StorageError> {
        let users: i64 = sqlx::query_scalar("SELECT count(*) FROM users")
            .fetch_one(self.pool())
            .await?;
        let assets: i64 = sqlx::query_scalar("SELECT count(*) FROM assets")
            .fetch_one(self.pool())
            .await?;
        Ok(users == 0 && assets == 0)
    }

    /// Starts a rebuild from `from`'s copy: records it, and removes what a fresh server made
    /// itself (its default group), which the copy's records replace. Fails if one was started.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails or a rebuild was already started.
    pub async fn start_rebuild(
        &self,
        from: DatabaseId,
        now: UnixMillis,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO rebuild_state (id, from_database_id, started_ms) VALUES (1, ?, ?)",
        )
        .bind(id_to_sql(from))
        .bind(now.as_millis())
        .execute(&mut *transaction)
        .await?;
        sqlx::query("DELETE FROM field_clocks WHERE entity = 'group'")
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM groups")
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Forgets a rebuild that brought nothing in (started before a restart; the server is
    /// still fresh), so it can start again.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn forget_rebuild(&self) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM rebuild_state WHERE id = 1")
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The rebuild, if this server was (or is being) rebuilt.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn rebuild_state(&self) -> Result<Option<RebuildState>, StorageError> {
        let Some(row) = sqlx::query(
            "SELECT from_database_id, started_ms, finished_ms, settled_ms FROM rebuild_state WHERE id = 1",
        )
        .fetch_optional(self.pool())
        .await?
        else {
            return Ok(None);
        };
        let from: Vec<u8> = row.try_get("from_database_id")?;
        let finished: Option<i64> = row.try_get("finished_ms")?;
        let settled: Option<i64> = row.try_get("settled_ms")?;
        Ok(Some(RebuildState {
            from: id_from_sql(&from, "from_database_id")?,
            started: UnixMillis::from_millis(row.try_get("started_ms")?),
            finished: finished.map(UnixMillis::from_millis),
            settled: settled.map(UnixMillis::from_millis),
        }))
    }

    /// Notes the rebuild's records are in.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn finish_rebuild(&self, now: UnixMillis) -> Result<(), StorageError> {
        sqlx::query("UPDATE rebuild_state SET finished_ms = ? WHERE id = 1")
            .bind(now.as_millis())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Notes the rebuild's files are settled.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn settle_rebuild(&self, now: UnixMillis) -> Result<(), StorageError> {
        sqlx::query("UPDATE rebuild_state SET settled_ms = ? WHERE id = 1")
            .bind(now.as_millis())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Whether some attachment's records name the file `hash` (its file, thumbnail, original,
    /// or metadata), so a rebuild may restore it.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn some_attachment_names(&self, hash: &FileHash) -> Result<bool, StorageError> {
        // An original's and metadata's hashes are inside `content` (JSON): found there, then
        // checked.
        let rows = sqlx::query(
            "SELECT * FROM attachments
             WHERE file_sha256 = ?1 OR thumbnail_sha256 = ?1 OR instr(content, ?1) > 0",
        )
        .bind(hash.as_str())
        .fetch_all(self.pool())
        .await?;
        for row in &rows {
            let (_, attachment) = crate::attachments::attachment_from_row(row)?;
            if attachment.files().iter().any(|file| file.hash == *hash) {
                return Ok(true);
            }
        }
        Ok(false)
    }

    /// Leaves an attachment out of a rebuild (its file never came): its row and field clocks
    /// go (it was never this server's to remove).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails.
    pub async fn leave_out_attachment(&self, id: AttachmentId) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query("DELETE FROM field_clocks WHERE entity = 'attachment' AND record_id = ?")
            .bind(id_to_sql(id))
            .execute(&mut *transaction)
            .await?;
        sqlx::query("DELETE FROM attachments WHERE id = ?")
            .bind(id_to_sql(id))
            .execute(&mut *transaction)
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    /// Replaces a rebuilt attachment's content (an original or metadata that never came, taken
    /// out of it).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the content can't be stored.
    pub async fn replace_attachment_content(
        &self,
        id: AttachmentId,
        content: &Content,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE attachments SET content = ? WHERE id = ?")
            .bind(json_to_sql(content, "content")?)
            .bind(id_to_sql(id))
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
