//! The `specs` table (Phase 5 milestone 4; decision #084): each vehicle's specs, written for
//! the command pipeline and read for the server. An amount is stored as entered: thousandths
//! of its unit, and the unit's name.

use domain::asset::AssetId;
use domain::command::Clearable;
use domain::id::Id;
use domain::label::Label;
use domain::notes::Notes;
use domain::spec::{Spec, SpecAmount, SpecId, SpecPatch};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{clearable, id_from_sql, id_to_sql, invalid, name, required, required_name};
use crate::{Database, StorageError};

/// Writes the `specs` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct SpecWriter;

/// An amount's two columns: thousandths and the unit's name (both or neither).
fn amount_columns(
    amount: Option<SpecAmount>,
) -> Result<(Option<i64>, Option<String>), StorageError> {
    Ok(match amount {
        Some(amount) => (
            Some(amount.milli),
            Some(name(&amount.unit, "specs.amount_unit")?),
        ),
        None => (None, None),
    })
}

impl RecordWriter<Spec> for SpecWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Spec>,
        values: &SpecPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let (milli, unit) = amount_columns(values.amount.and_then(Clearable::into_option))?;
        sqlx::query(
            "INSERT INTO specs (id, asset_id, name, amount_milli, amount_unit, detail,
                                part_number, notes, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(milli)
        .bind(unit)
        .bind(values.detail.clone().and_then(Clearable::into_option))
        .bind(values.part_number.clone().and_then(Clearable::into_option))
        .bind(values.notes.as_ref().map_or("", Notes::as_str).to_owned())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Spec>,
        patch: &SpecPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle is fixed once made. The amount's two columns change together.
        let (amount_changed, (milli, unit)) = match patch.amount {
            None => (false, (None, None)),
            Some(change) => (true, amount_columns(change.into_option())?),
        };
        let detail = clearable(patch.detail.as_ref(), |text: String| text);
        let part_number = clearable(patch.part_number.as_ref(), |text: String| text);
        sqlx::query(
            "UPDATE specs SET
                 name = coalesce(?, name),
                 amount_milli = CASE WHEN ? THEN ? ELSE amount_milli END,
                 amount_unit = CASE WHEN ? THEN ? ELSE amount_unit END,
                 detail = CASE WHEN ? THEN ? ELSE detail END,
                 part_number = CASE WHEN ? THEN ? ELSE part_number END,
                 notes = coalesce(?, notes),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(amount_changed)
        .bind(milli)
        .bind(amount_changed)
        .bind(unit)
        .bind(detail.0)
        .bind(detail.1)
        .bind(part_number.0)
        .bind(part_number.1)
        .bind(patch.notes.as_ref().map(|notes| notes.as_str().to_owned()))
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Spec>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE specs SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn spec_from_row(row: &SqliteRow) -> Result<(SpecId, Spec), StorageError> {
    let name: String = row.try_get("name")?;
    let notes: String = row.try_get("notes")?;
    let milli: Option<i64> = row.try_get("amount_milli")?;
    let unit: Option<String> = row.try_get("amount_unit")?;
    let amount = match (milli, unit) {
        (Some(milli), Some(unit)) => Some(SpecAmount {
            milli,
            unit: required_name(unit, "specs.amount_unit")?,
        }),
        _ => None,
    };
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "specs.id")?,
        Spec {
            asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "specs.asset_id")?,
            name: Label::parse(&name).map_err(invalid("specs.name"))?,
            amount,
            detail: row.try_get("detail")?,
            part_number: row.try_get("part_number")?,
            notes: Notes::parse(&notes).map_err(invalid("specs.notes"))?,
        },
    ))
}

impl Database {
    /// One spec, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn spec(&self, id: SpecId) -> Result<Option<Spec>, StorageError> {
        sqlx::query("SELECT * FROM specs WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| spec_from_row(row).map(|(_, spec)| spec))
            .transpose()
    }

    /// One vehicle's specs (not deleted), by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn specs_of(&self, asset: AssetId) -> Result<Vec<(SpecId, Spec)>, StorageError> {
        sqlx::query(
            "SELECT * FROM specs WHERE asset_id = ? AND deleted = 0
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(spec_from_row)
        .collect()
    }
}
