//! The `requirements` and `bundles` tables (milestone 7): their writers for the command
//! pipeline, and the queries the server needs. When a requirement was last done comes from the
//! tasks of a vehicle's counted maintenance and repair events.

use domain::asset::AssetId;
use domain::id::Id;
use domain::label::Label;
use domain::notes::Notes;
use domain::requirement::{
    Bundle, BundleId, BundlePatch, DoneTask, IntervalRule, Requirement, RequirementId,
    RequirementPatch,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `requirements` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct RequirementWriter;

/// Writes the `bundles` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct BundleWriter;

impl RecordWriter<Requirement> for RequirementWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Requirement>,
        values: &RequirementPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO requirements (id, asset_id, name, notes, enabled, rules, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(values.notes.as_ref().map_or("", Notes::as_str).to_owned())
        .bind(values.enabled.unwrap_or(true))
        .bind(json_to_sql(
            &values.rules.clone().unwrap_or_default(),
            "requirements.rules",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Requirement>,
        patch: &RequirementPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle is fixed once made.
        sqlx::query(
            "UPDATE requirements SET
                 name = coalesce(?, name),
                 notes = coalesce(?, notes),
                 enabled = coalesce(?, enabled),
                 rules = coalesce(?, rules),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(patch.notes.as_ref().map(|notes| notes.as_str().to_owned()))
        .bind(patch.enabled)
        .bind(
            patch
                .rules
                .as_ref()
                .map(|rules| json_to_sql(rules, "requirements.rules"))
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
        id: Id<Requirement>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE requirements SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

impl RecordWriter<Bundle> for BundleWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Bundle>,
        values: &BundlePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO bundles (id, asset_id, name, requirements, sequence)
             VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.asset, "asset_id")?))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(json_to_sql(
            &values.requirements.clone().unwrap_or_default(),
            "bundles.requirements",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Bundle>,
        patch: &BundlePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle is fixed once made.
        sqlx::query(
            "UPDATE bundles SET
                 name = coalesce(?, name),
                 requirements = coalesce(?, requirements),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(
            patch
                .requirements
                .as_ref()
                .map(|list| json_to_sql(list, "bundles.requirements"))
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
        id: Id<Bundle>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE bundles SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn requirement_from_row(row: &SqliteRow) -> Result<(RequirementId, Requirement), StorageError> {
    let name: String = row.try_get("name")?;
    let notes: String = row.try_get("notes")?;
    let rules: String = row.try_get("rules")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "requirements.id")?,
        Requirement {
            asset: id_from_sql(
                row.try_get::<&[u8], _>("asset_id")?,
                "requirements.asset_id",
            )?,
            name: Label::parse(&name).map_err(invalid("requirements.name"))?,
            notes: Notes::parse(&notes).map_err(invalid("requirements.notes"))?,
            enabled: row.try_get("enabled")?,
            rules: json_from_sql::<Vec<IntervalRule>>(&rules, "requirements.rules")?,
        },
    ))
}

fn bundle_from_row(row: &SqliteRow) -> Result<(BundleId, Bundle), StorageError> {
    let name: String = row.try_get("name")?;
    let requirements: String = row.try_get("requirements")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "bundles.id")?,
        Bundle {
            asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "bundles.asset_id")?,
            name: Label::parse(&name).map_err(invalid("bundles.name"))?,
            requirements: json_from_sql(&requirements, "bundles.requirements")?,
        },
    ))
}

impl Database {
    /// One requirement, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn requirement(
        &self,
        id: RequirementId,
    ) -> Result<Option<Requirement>, StorageError> {
        sqlx::query("SELECT * FROM requirements WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| requirement_from_row(row).map(|(_, requirement)| requirement))
            .transpose()
    }

    /// One vehicle's requirements (not deleted; turned-off ones too), by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn requirements_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(RequirementId, Requirement)>, StorageError> {
        sqlx::query(
            "SELECT * FROM requirements WHERE asset_id = ? AND deleted = 0
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(requirement_from_row)
        .collect()
    }

    /// One bundle, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn bundle(&self, id: BundleId) -> Result<Option<Bundle>, StorageError> {
        sqlx::query("SELECT * FROM bundles WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| bundle_from_row(row).map(|(_, bundle)| bundle))
            .transpose()
    }

    /// One vehicle's bundles (not deleted), by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn bundles_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(BundleId, Bundle)>, StorageError> {
        sqlx::query(
            "SELECT * FROM bundles WHERE asset_id = ? AND deleted = 0
             ORDER BY name COLLATE NOCASE, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(bundle_from_row)
        .collect()
    }

    /// Every task done for a requirement on one vehicle (from its counted events), oldest
    /// first; [`domain::requirement::last_done`] picks each requirement's latest.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn done_tasks_of(&self, asset: AssetId) -> Result<Vec<DoneTask>, StorageError> {
        let mut done: Vec<DoneTask> = self
            .counted_events_of(asset)
            .await?
            .iter()
            .flat_map(|(id, event)| event.done_tasks(*id))
            .collect();
        done.sort_by_key(|task| (task.at, task.event));
        Ok(done)
    }
}
