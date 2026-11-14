//! Offline changes (phase-3.md milestone 5 step 5b, decision #076; migration 0019).
//!
//! **On a desktop's copy:**
//! - the **outbox** ([`Database::add_to_outbox`]): each change made on the desktop, in the order
//!   it was made, until the server acknowledges it ([`Database::remove_from_outbox`]) or
//!   refuses it ([`Database::refuse_in_outbox`]; kept for Messages until opened or discarded);
//! - the desktop's **clock** ([`Database::device_hlc`], [`Database::advance_device_hlc`]): the
//!   newest hybrid logical clock time it has made or received, so its next one is later.
//!
//! **On the server:**
//! - **applied operations** ([`Database::operation_answer`], [`Database::record_operation`]):
//!   each pushed operation and its answer, so a resent one gets the same answer;
//! - a **clock-ahead flag** on a device ([`Database::flag_clock_ahead`]) for the Admin.

use domain::hlc::Hlc;
use domain::id::DeviceId;
use domain::operation::{Operation, OperationId, Refusal};
use domain::time::UnixMillis;
use domain::user::UserId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::convert::{
    hlc_from_sql, hlc_to_sql, id_from_sql, id_to_sql, json_from_sql, json_to_sql,
};
use crate::{Database, StorageError};

/// Where a change in the outbox stands.
#[derive(Debug, Clone, PartialEq)]
pub enum OutboxState {
    /// Not yet sent, or sent without an answer.
    Waiting,
    /// The server refused it, and why.
    Refused {
        /// The server's answer.
        refusal: Refusal,
        /// When the desktop heard.
        at: UnixMillis,
    },
}

/// One change in the outbox.
#[derive(Debug, Clone, PartialEq)]
pub struct OutboxEntry {
    /// The change.
    pub operation: Operation,
    /// When it was made (the desktop's clock).
    pub made_at: UnixMillis,
    /// Waiting or refused.
    pub state: OutboxState,
}

/// How the outbox stands, for the top bar, Diagnostics, and the warnings (#076).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct OutboxSummary {
    /// Changes waiting to be sent.
    pub waiting: u64,
    /// Changes the server refused, not yet opened or discarded.
    pub refused: u64,
    /// When the oldest waiting change was made.
    pub oldest_waiting: Option<UnixMillis>,
}

/// An operation the server received, and its answer.
#[derive(Debug, Clone, PartialEq)]
pub struct AppliedOperation {
    /// Which operation.
    pub id: OperationId,
    /// The device that sent it (its session's).
    pub device: DeviceId,
    /// The person who sent it.
    pub user: UserId,
    /// Its time.
    pub hlc: Hlc,
    /// The format it came in.
    pub format_version: u32,
    /// Why it was refused; `None` when it was applied.
    pub refusal: Option<Refusal>,
    /// When the server received it.
    pub received_at: UnixMillis,
}

/// A desktop whose clock was found far ahead of the server's.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ClockAhead {
    /// Which device.
    pub device: DeviceId,
    /// Whose it is.
    pub user: UserId,
    /// How far ahead, in milliseconds.
    pub ahead_millis: i64,
    /// When the server noticed.
    pub at: UnixMillis,
}

fn entry_from_row(row: &SqliteRow) -> Result<OutboxEntry, StorageError> {
    let operation: String = row.try_get("operation")?;
    let state: String = row.try_get("state")?;
    let refusal: Option<String> = row.try_get("refusal")?;
    let refused_at: Option<i64> = row.try_get("refused_at_ms")?;
    let state = match (state.as_str(), refusal, refused_at) {
        ("waiting", None, None) => OutboxState::Waiting,
        ("refused", Some(refusal), Some(at)) => OutboxState::Refused {
            refusal: json_from_sql(&refusal, "refusal")?,
            at: UnixMillis::from_millis(at),
        },
        _ => return Err(StorageError::InvalidValue { column: "state" }),
    };
    Ok(OutboxEntry {
        operation: json_from_sql(&operation, "operation")?,
        made_at: UnixMillis::from_millis(row.try_get("made_at_ms")?),
        state,
    })
}

/// A count from SQLite (never negative) as a `u64`.
fn count(value: i64) -> u64 {
    u64::try_from(value).unwrap_or(0)
}

impl Database {
    /// Adds a change to the end of the outbox, waiting to be sent.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails, or the operation is already in the outbox.
    pub async fn add_to_outbox(
        &self,
        operation: &Operation,
        made_at: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO outbox (operation_id, operation, made_at_ms) VALUES (?, ?, ?)")
            .bind(id_to_sql(operation.id))
            .bind(json_to_sql(operation, "operation")?)
            .bind(made_at.as_millis())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The changes waiting to be sent, oldest first, at most `limit`.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn outbox_waiting(&self, limit: u32) -> Result<Vec<Operation>, StorageError> {
        sqlx::query(
            "SELECT operation, state, refusal, refused_at_ms, made_at_ms FROM outbox
             WHERE state = 'waiting' ORDER BY position LIMIT ?",
        )
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(|row| entry_from_row(row).map(|entry| entry.operation))
        .collect()
    }

    /// Every change in the outbox, waiting or refused, oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn outbox_entries(&self) -> Result<Vec<OutboxEntry>, StorageError> {
        sqlx::query(
            "SELECT operation, state, refusal, refused_at_ms, made_at_ms FROM outbox
             ORDER BY position",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(entry_from_row)
        .collect()
    }

    /// One change in the outbox, if it's there.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn outbox_entry(&self, id: OperationId) -> Result<Option<OutboxEntry>, StorageError> {
        sqlx::query(
            "SELECT operation, state, refusal, refused_at_ms, made_at_ms FROM outbox
             WHERE operation_id = ?",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(entry_from_row)
        .transpose()
    }

    /// Puts `operation` back in its place in the outbox, changed (an add's files, once
    /// uploaded, become the server's, 6d). Whether it was there.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or it can't be stored.
    pub async fn replace_in_outbox(&self, operation: &Operation) -> Result<bool, StorageError> {
        Ok(
            sqlx::query("UPDATE outbox SET operation = ? WHERE operation_id = ?")
                .bind(json_to_sql(operation, "operation")?)
                .bind(id_to_sql(operation.id))
                .execute(self.pool())
                .await?
                .rows_affected()
                > 0,
        )
    }

    /// Marks a waiting change refused, with the server's answer. Whether it was waiting.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn refuse_in_outbox(
        &self,
        id: OperationId,
        refusal: &Refusal,
        at: UnixMillis,
    ) -> Result<bool, StorageError> {
        Ok(sqlx::query(
            "UPDATE outbox SET state = 'refused', refusal = ?, refused_at_ms = ?
             WHERE operation_id = ? AND state = 'waiting'",
        )
        .bind(json_to_sql(refusal, "refusal")?)
        .bind(at.as_millis())
        .bind(id_to_sql(id))
        .execute(self.pool())
        .await?
        .rows_affected()
            > 0)
    }

    /// Removes a change from the outbox: acknowledged by the server, or a refused one
    /// discarded (or opened again, as a new change). Whether it was there.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn remove_from_outbox(&self, id: OperationId) -> Result<bool, StorageError> {
        Ok(sqlx::query("DELETE FROM outbox WHERE operation_id = ?")
            .bind(id_to_sql(id))
            .execute(self.pool())
            .await?
            .rows_affected()
            > 0)
    }

    /// How many changes wait, how many were refused, and when the oldest waiting one was made.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn outbox_summary(&self) -> Result<OutboxSummary, StorageError> {
        let row = sqlx::query(
            "SELECT
                 count(*) FILTER (WHERE state = 'waiting') AS waiting,
                 count(*) FILTER (WHERE state = 'refused') AS refused,
                 min(made_at_ms) FILTER (WHERE state = 'waiting') AS oldest
             FROM outbox",
        )
        .fetch_one(self.pool())
        .await?;
        let oldest: Option<i64> = row.try_get("oldest")?;
        Ok(OutboxSummary {
            waiting: count(row.try_get("waiting")?),
            refused: count(row.try_get("refused")?),
            oldest_waiting: oldest.map(UnixMillis::from_millis),
        })
    }

    /// The newest clock time this desktop has made or received ([`Hlc::ZERO`] before its
    /// first).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored value is invalid.
    pub async fn device_hlc(&self) -> Result<Hlc, StorageError> {
        let stored: Option<i64> =
            sqlx::query_scalar("SELECT last_hlc FROM device_clock WHERE id = 1")
                .fetch_optional(self.pool())
                .await?;
        stored.map_or(Ok(Hlc::ZERO), |value| hlc_from_sql(value, "last_hlc"))
    }

    /// Moves this desktop's clock up to `hlc`, if that's later than what it has (never back).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or `hlc` can't be stored.
    pub async fn advance_device_hlc(&self, hlc: Hlc) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO device_clock (id, last_hlc) VALUES (1, ?)
             ON CONFLICT (id) DO UPDATE SET last_hlc = max(last_hlc, excluded.last_hlc)",
        )
        .bind(hlc_to_sql(hlc, "last_hlc")?)
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// The newest clock time on any field in this copy (pulled from the server, or written
    /// here): the desktop's clock moves past it, so a change made after one it has seen is
    /// always newer ([`Hlc::ZERO`] if there's none).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored value is invalid.
    pub async fn newest_field_hlc(&self) -> Result<Hlc, StorageError> {
        let newest: Option<i64> = sqlx::query_scalar("SELECT max(hlc) FROM field_clocks")
            .fetch_one(self.pool())
            .await?;
        newest.map_or(Ok(Hlc::ZERO), |value| hlc_from_sql(value, "hlc"))
    }

    /// The answer already given to a pushed operation: `Some(None)` if it was applied,
    /// `Some(Some(refusal))` if it was refused, `None` if it's new.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn operation_answer(
        &self,
        id: OperationId,
    ) -> Result<Option<Option<Refusal>>, StorageError> {
        let row: Option<Option<String>> =
            sqlx::query_scalar("SELECT refusal FROM applied_operations WHERE operation_id = ?")
                .bind(id_to_sql(id))
                .fetch_optional(self.pool())
                .await?;
        row.map(|refusal| {
            refusal
                .map(|text| json_from_sql(&text, "refusal"))
                .transpose()
        })
        .transpose()
    }

    /// Records a pushed operation and its answer. The first answer stands: recording the same
    /// operation again changes nothing.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a value can't be stored.
    pub async fn record_operation(&self, operation: &AppliedOperation) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO applied_operations
                 (operation_id, device_id, user_id, hlc, format_version, refusal, received_at_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?)
             ON CONFLICT (operation_id) DO NOTHING",
        )
        .bind(id_to_sql(operation.id))
        .bind(id_to_sql(operation.device))
        .bind(id_to_sql(operation.user))
        .bind(hlc_to_sql(operation.hlc, "hlc")?)
        .bind(i64::from(operation.format_version))
        .bind(
            operation
                .refusal
                .as_ref()
                .map(|refusal| json_to_sql(refusal, "refusal"))
                .transpose()?,
        )
        .bind(operation.received_at.as_millis())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Flags `device`: a change came with a clock `ahead_millis` ahead of the server's.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn flag_clock_ahead(
        &self,
        device: DeviceId,
        ahead_millis: i64,
        at: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE devices SET clock_ahead_ms = ?, clock_ahead_at_ms = ? WHERE id = ?")
            .bind(ahead_millis.max(1))
            .bind(at.as_millis())
            .bind(id_to_sql(device))
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Clears `device`'s flag: its clock is right again.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn clear_clock_ahead(&self, device: DeviceId) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE devices SET clock_ahead_ms = NULL, clock_ahead_at_ms = NULL
             WHERE id = ? AND clock_ahead_ms IS NOT NULL",
        )
        .bind(id_to_sql(device))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// The devices flagged for a clock far ahead, most recent first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn devices_with_clock_ahead(&self) -> Result<Vec<ClockAhead>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, clock_ahead_ms, clock_ahead_at_ms FROM devices
             WHERE clock_ahead_ms IS NOT NULL ORDER BY clock_ahead_at_ms DESC, id",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(|row| {
            let device: &[u8] = row.try_get("id")?;
            let user: &[u8] = row.try_get("user_id")?;
            let at: Option<i64> = row.try_get("clock_ahead_at_ms")?;
            Ok(ClockAhead {
                device: id_from_sql(device, "id")?,
                user: id_from_sql(user, "user_id")?,
                ahead_millis: row.try_get("clock_ahead_ms")?,
                at: UnixMillis::from_millis(at.unwrap_or_default()),
            })
        })
        .collect()
    }
}
