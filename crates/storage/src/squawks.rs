//! The `squawks` table (milestone 7; decisions #051, #063): the squawk writer for the command
//! pipeline, and the queries the server needs. A squawk's dispositions are stored with the
//! maintenance or repair event that made them ([`crate::events`]); they count once it's
//! finalized.

use domain::asset::AssetId;
use domain::command::Clearable;
use domain::id::Id;
use domain::notes::Notes;
use domain::squawk::{
    CountedDisposition, Squawk, SquawkId, SquawkPatch, SquawkText, current_severity, status,
};
use domain::time::UnixMillis;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{clearable, id_from_sql, id_to_sql, invalid, name, required, required_name};
use crate::{Database, StorageError};

/// Writes the `squawks` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct SquawkWriter;

impl RecordWriter<Squawk> for SquawkWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Squawk>,
        values: &SquawkPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO squawks (id, asset_id, description, notes, severity, opened_ms,
                                  opened_by, found_on_id, received_ms, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.description.as_ref(), "description")?.as_str())
        .bind(values.notes.as_ref().map_or("", Notes::as_str).to_owned())
        .bind(name(
            &values.severity.unwrap_or_default(),
            "squawks.severity",
        )?)
        .bind(required(values.opened_at, "opened_ms")?.as_millis())
        .bind(id_to_sql(required(values.opened_by, "opened_by")?))
        .bind(
            values
                .found_on
                .and_then(Clearable::into_option)
                .map(id_to_sql),
        )
        .bind(required(values.received_at, "received_ms")?.as_millis())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Squawk>,
        patch: &SquawkPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle, who opened it, and when it arrived are fixed once opened.
        let found_on = clearable(patch.found_on.as_ref(), id_to_sql);
        sqlx::query(
            "UPDATE squawks SET
                 description = coalesce(?, description),
                 notes = coalesce(?, notes),
                 severity = coalesce(?, severity),
                 opened_ms = coalesce(?, opened_ms),
                 found_on_id = CASE WHEN ? THEN ? ELSE found_on_id END,
                 sequence = ?
             WHERE id = ?",
        )
        .bind(
            patch
                .description
                .as_ref()
                .map(|text| text.as_str().to_owned()),
        )
        .bind(patch.notes.as_ref().map(|notes| notes.as_str().to_owned()))
        .bind(
            patch
                .severity
                .map(|severity| name(&severity, "squawks.severity"))
                .transpose()?,
        )
        .bind(patch.opened_at.map(UnixMillis::as_millis))
        .bind(found_on.0)
        .bind(found_on.1)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Squawk>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE squawks SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn squawk_from_row(row: &SqliteRow) -> Result<(SquawkId, Squawk), StorageError> {
    let description: String = row.try_get("description")?;
    let notes: String = row.try_get("notes")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "squawks.id")?,
        Squawk {
            asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "squawks.asset_id")?,
            description: SquawkText::parse(&description).map_err(invalid("squawks.description"))?,
            notes: Notes::parse(&notes).map_err(invalid("squawks.notes"))?,
            severity: required_name(row.try_get("severity")?, "squawks.severity")?,
            opened_at: UnixMillis::from_millis(row.try_get("opened_ms")?),
            opened_by: id_from_sql(row.try_get::<&[u8], _>("opened_by")?, "squawks.opened_by")?,
            found_on: row
                .try_get::<Option<&[u8]>, _>("found_on_id")?
                .map(|bytes| id_from_sql(bytes, "squawks.found_on_id"))
                .transpose()?,
            received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        },
    ))
}

/// A squawk with where it stands now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SquawkState {
    /// Which squawk.
    pub id: SquawkId,
    /// The squawk, as opened (and edited).
    pub squawk: Squawk,
    /// Its dispositions that count, oldest first.
    pub history: Vec<CountedDisposition>,
}

impl SquawkState {
    /// Where it stands: open, deferred, or resolved.
    #[must_use]
    pub fn status(&self) -> domain::squawk::SquawkStatus {
        status(&self.history)
    }

    /// Its severity now.
    #[must_use]
    pub fn severity(&self) -> domain::squawk::Severity {
        current_severity(&self.squawk, &self.history)
    }
}

impl Database {
    /// One squawk, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn squawk(&self, id: SquawkId) -> Result<Option<Squawk>, StorageError> {
        sqlx::query("SELECT * FROM squawks WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| squawk_from_row(row).map(|(_, squawk)| squawk))
            .transpose()
    }

    /// One vehicle's squawks (not deleted), newest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn squawks_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(SquawkId, Squawk)>, StorageError> {
        sqlx::query(
            "SELECT * FROM squawks WHERE asset_id = ? AND deleted = 0
             ORDER BY opened_ms DESC, id DESC",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(squawk_from_row)
        .collect()
    }

    /// The dispositions that count on one vehicle (from its counted events), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn counted_dispositions_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<CountedDisposition>, StorageError> {
        let mut counted: Vec<CountedDisposition> = self
            .counted_events_of(asset)
            .await?
            .into_iter()
            .flat_map(|(event, details)| {
                let at = details.dates.start;
                details
                    .work
                    .map(|work| work.dispositions)
                    .unwrap_or_default()
                    .into_iter()
                    .map(move |disposition| CountedDisposition {
                        at,
                        event,
                        disposition,
                    })
            })
            .collect();
        counted.sort_by_key(|counted| (counted.at, counted.event));
        Ok(counted)
    }

    /// One vehicle's squawks (newest first), each with its counted dispositions.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn squawk_states_of(&self, asset: AssetId) -> Result<Vec<SquawkState>, StorageError> {
        let dispositions = self.counted_dispositions_of(asset).await?;
        Ok(self
            .squawks_of(asset)
            .await?
            .into_iter()
            .map(|(id, squawk)| SquawkState {
                id,
                squawk,
                history: dispositions
                    .iter()
                    .filter(|counted| counted.disposition.squawk == id)
                    .cloned()
                    .collect(),
            })
            .collect())
    }

    /// One vehicle's outstanding squawks (open or deferred), which its next maintenance or
    /// repair must disposition, oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn outstanding_squawks_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<SquawkId>, StorageError> {
        let mut outstanding: Vec<SquawkId> = self
            .squawk_states_of(asset)
            .await?
            .iter()
            .filter(|state| state.status().is_outstanding())
            .map(|state| state.id)
            .collect();
        outstanding.reverse();
        Ok(outstanding)
    }
}
