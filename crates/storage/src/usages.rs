//! The `usages` table (Phase 5 milestone 6; decision #086): each vehicle's usages, written for
//! the command pipeline and read for the server.

use domain::asset::AssetId;
use domain::id::Id;
use domain::label::Label;
use domain::usage::{Usage, UsageId, UsagePatch};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `usages` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct UsageWriter;

impl RecordWriter<Usage> for UsageWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Usage>,
        values: &UsagePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO usages (id, asset_id, name, is_default, documents, sequence)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(values.default.unwrap_or(false))
        .bind(json_to_sql(
            values.documents.as_ref().unwrap_or(&Vec::new()),
            "usages.documents",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Usage>,
        patch: &UsagePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle, and whether it's the default, are fixed once made.
        sqlx::query(
            "UPDATE usages SET
                 name = coalesce(?, name),
                 documents = coalesce(?, documents),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(
            patch
                .documents
                .as_ref()
                .map(|documents| json_to_sql(documents, "usages.documents"))
                .transpose()?,
        )
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Usage>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE usages SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn usage_from_row(row: &SqliteRow) -> Result<(UsageId, Usage), StorageError> {
    let name: String = row.try_get("name")?;
    let documents: String = row.try_get("documents")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "usages.id")?,
        Usage {
            asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "usages.asset_id")?,
            name: Label::parse(&name).map_err(invalid("usages.name"))?,
            default: row.try_get("is_default")?,
            documents: json_from_sql(&documents, "usages.documents")?,
        },
    ))
}

impl Database {
    /// One usage, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn usage(&self, id: UsageId) -> Result<Option<Usage>, StorageError> {
        sqlx::query("SELECT * FROM usages WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| usage_from_row(row).map(|(_, usage)| usage))
            .transpose()
    }

    /// One vehicle's usages (not deleted): its default first, then by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn usages_of(&self, asset: AssetId) -> Result<Vec<(UsageId, Usage)>, StorageError> {
        sqlx::query(
            "SELECT * FROM usages WHERE asset_id = ? AND deleted = 0
             ORDER BY is_default DESC, name COLLATE NOCASE, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(usage_from_row)
        .collect()
    }
}
