//! The bookkeeping queries behind command application (`server::commands`).
//!
//! Each runs on the connection of the one transaction a command is applied in, so a
//! command is recorded completely or not at all:
//! - `applied_commands`: which commands were already applied (retries do nothing);
//! - `field_clocks`: the stamp of the last accepted change to each field (#043);
//! - `tombstones`: deleted records;
//! - `database_meta`: the sequence counter and the server's newest HLC.
//!
//! Writing the record's own row is up to its [`RecordWriter`].

use std::collections::HashMap;

use domain::command::{CommandId, Record};
use domain::hlc::{Hlc, Stamp};
use domain::id::Id;
use domain::time::UnixMillis;
use sqlx::{Row, SqliteConnection};

use crate::StorageError;
use crate::convert::{hlc_from_sql, hlc_to_sql, id_from_sql, id_to_sql};

/// Writes one kind of record's own table. Implemented once per record type; called by the
/// command engine with only the fields that won, inside the command's transaction.
pub trait RecordWriter<R: Record> {
    /// Inserts a new record with `values`.
    fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<R>,
        values: &R::Patch,
        sequence: i64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Changes the fields set in `patch` (never empty).
    fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<R>,
        patch: &R::Patch,
        sequence: i64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;

    /// Marks the record deleted (a soft delete: the row may stay, flagged).
    fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<R>,
        sequence: i64,
    ) -> impl Future<Output = Result<(), StorageError>> + Send;
}

/// Whether `command_id` was already applied.
///
/// # Errors
///
/// [`StorageError::Sqlx`] if the query fails.
pub async fn is_applied(
    connection: &mut SqliteConnection,
    command_id: CommandId,
) -> Result<bool, StorageError> {
    let applied: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM applied_commands WHERE command_id = ?)")
            .bind(id_to_sql(command_id))
            .fetch_one(connection)
            .await?;
    Ok(applied)
}

/// Records `command_id` as applied; `sequence` is `None` if it changed nothing.
///
/// # Errors
///
/// [`StorageError`] if the query fails or the HLC can't be stored.
pub async fn record_applied(
    connection: &mut SqliteConnection,
    command_id: CommandId,
    stamp: Stamp,
    sequence: Option<i64>,
    now: UnixMillis,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO applied_commands (command_id, device_id, hlc, sequence, applied_at_ms)
         VALUES (?, ?, ?, ?, ?)",
    )
    .bind(id_to_sql(command_id))
    .bind(id_to_sql(stamp.device))
    .bind(hlc_to_sql(stamp.hlc, "applied_commands.hlc")?)
    .bind(sequence)
    .bind(now.as_millis())
    .execute(connection)
    .await?;
    Ok(())
}

/// The stamp of the last accepted change to each field of one record.
///
/// # Errors
///
/// [`StorageError`] if the query fails or a stored value is invalid.
pub async fn field_stamps(
    connection: &mut SqliteConnection,
    entity: &str,
    record_id: [u8; 16],
) -> Result<HashMap<String, Stamp>, StorageError> {
    let rows = sqlx::query(
        "SELECT field, hlc, device_id FROM field_clocks WHERE entity = ? AND record_id = ?",
    )
    .bind(entity)
    .bind(record_id.to_vec())
    .fetch_all(connection)
    .await?;
    rows.iter()
        .map(|row| {
            let stamp = stamp_from_row(row, "field_clocks")?;
            Ok((row.try_get::<String, _>("field")?, stamp))
        })
        .collect()
}

/// Sets the stamp of each of `fields` on one record to `stamp`.
///
/// # Errors
///
/// [`StorageError`] if a query fails or the HLC can't be stored.
pub async fn set_field_stamps(
    connection: &mut SqliteConnection,
    entity: &str,
    record_id: [u8; 16],
    fields: &[&str],
    stamp: Stamp,
) -> Result<(), StorageError> {
    let hlc = hlc_to_sql(stamp.hlc, "field_clocks.hlc")?;
    for field in fields {
        sqlx::query(
            "INSERT INTO field_clocks (entity, record_id, field, hlc, device_id)
             VALUES (?, ?, ?, ?, ?)
             ON CONFLICT (entity, record_id, field)
             DO UPDATE SET hlc = excluded.hlc, device_id = excluded.device_id",
        )
        .bind(entity)
        .bind(record_id.to_vec())
        .bind(field)
        .bind(hlc)
        .bind(id_to_sql(stamp.device))
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

/// The stamp of the record's deletion, if it's deleted.
///
/// # Errors
///
/// [`StorageError`] if the query fails or a stored value is invalid.
pub async fn tombstone(
    connection: &mut SqliteConnection,
    entity: &str,
    record_id: [u8; 16],
) -> Result<Option<Stamp>, StorageError> {
    let row =
        sqlx::query("SELECT hlc, device_id FROM tombstones WHERE entity = ? AND record_id = ?")
            .bind(entity)
            .bind(record_id.to_vec())
            .fetch_optional(connection)
            .await?;
    row.map(|row| stamp_from_row(&row, "tombstones"))
        .transpose()
}

/// Records the deletion of one record.
///
/// # Errors
///
/// [`StorageError`] if the query fails or the HLC can't be stored.
pub async fn insert_tombstone(
    connection: &mut SqliteConnection,
    entity: &str,
    record_id: [u8; 16],
    stamp: Stamp,
    sequence: i64,
    now: UnixMillis,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO tombstones (entity, record_id, hlc, device_id, sequence, deleted_at_ms)
         VALUES (?, ?, ?, ?, ?, ?)",
    )
    .bind(entity)
    .bind(record_id.to_vec())
    .bind(hlc_to_sql(stamp.hlc, "tombstones.hlc")?)
    .bind(id_to_sql(stamp.device))
    .bind(sequence)
    .bind(now.as_millis())
    .execute(connection)
    .await?;
    Ok(())
}

/// Takes the next sequence number (1, 2, 3, …) for an accepted change.
///
/// # Errors
///
/// [`StorageError::Sqlx`] if the query fails.
pub async fn next_sequence(connection: &mut SqliteConnection) -> Result<i64, StorageError> {
    let sequence: i64 = sqlx::query_scalar(
        "UPDATE database_meta SET last_sequence = last_sequence + 1 WHERE id = 1
         RETURNING last_sequence",
    )
    .fetch_one(connection)
    .await?;
    Ok(sequence)
}

/// The server's newest HLC.
///
/// # Errors
///
/// [`StorageError`] if the query fails or the stored value is invalid.
pub async fn server_last_hlc(connection: &mut SqliteConnection) -> Result<Hlc, StorageError> {
    let value: i64 = sqlx::query_scalar("SELECT last_hlc FROM database_meta WHERE id = 1")
        .fetch_one(connection)
        .await?;
    hlc_from_sql(value, "database_meta.last_hlc")
}

/// Saves the server's newest HLC.
///
/// # Errors
///
/// [`StorageError`] if the query fails or the HLC can't be stored.
pub async fn set_server_last_hlc(
    connection: &mut SqliteConnection,
    hlc: Hlc,
) -> Result<(), StorageError> {
    sqlx::query("UPDATE database_meta SET last_hlc = ? WHERE id = 1")
        .bind(hlc_to_sql(hlc, "database_meta.last_hlc")?)
        .execute(connection)
        .await?;
    Ok(())
}

/// A stamp from a row's `hlc` and `device_id` columns.
fn stamp_from_row(
    row: &sqlx::sqlite::SqliteRow,
    table: &'static str,
) -> Result<Stamp, StorageError> {
    Ok(Stamp {
        hlc: hlc_from_sql(row.try_get("hlc")?, table)?,
        device: id_from_sql(row.try_get::<&[u8], _>("device_id")?, table)?,
    })
}
