//! The `vehicle_types` table (phase-6.md 3e; decision #090): the household's vehicle types,
//! each built on one of the app's kinds.

use domain::asset::AssetKind;
use domain::id::Id;
use domain::label::Label;
use domain::vehicle_type::{TypeIcon, VehicleType, VehicleTypeId, VehicleTypePatch};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{
    clearable, id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, name, required,
    required_name,
};
use crate::{Database, StorageError};

/// Writes the `vehicle_types` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct VehicleTypeWriter;

impl RecordWriter<VehicleType> for VehicleTypeWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<VehicleType>,
        values: &VehicleTypePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let (_, template) = clearable(values.template.as_ref(), id_to_sql);
        sqlx::query(
            "INSERT INTO vehicle_types
                 (id, name, icon, kind, documents, fields_on, catalogue, template, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(name(&values.icon.unwrap_or_default(), "icon")?)
        .bind(required(values.kind.as_ref(), "kind")?.as_str())
        .bind(json_to_sql(
            values.documents.as_ref().unwrap_or(&Vec::new()),
            "vehicle_types.documents",
        )?)
        .bind(json_to_sql(
            values.fields_on.as_ref().unwrap_or(&Vec::new()),
            "vehicle_types.fields_on",
        )?)
        .bind(json_to_sql(
            values.catalogue.as_ref().unwrap_or(&Vec::new()),
            "vehicle_types.catalogue",
        )?)
        .bind(template)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<VehicleType>,
        patch: &VehicleTypePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let template = clearable(patch.template.as_ref(), id_to_sql);
        sqlx::query(
            "UPDATE vehicle_types SET
                 name = coalesce(?, name),
                 icon = coalesce(?, icon),
                 kind = coalesce(?, kind),
                 documents = coalesce(?, documents),
                 fields_on = coalesce(?, fields_on),
                 catalogue = coalesce(?, catalogue),
                 template = CASE WHEN ? THEN ? ELSE template END,
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(patch.icon.map(|icon| name(&icon, "icon")).transpose()?)
        .bind(patch.kind.as_ref().map(|kind| kind.as_str().to_owned()))
        .bind(
            patch
                .documents
                .as_ref()
                .map(|value| json_to_sql(value, "vehicle_types.documents"))
                .transpose()?,
        )
        .bind(
            patch
                .fields_on
                .as_ref()
                .map(|value| json_to_sql(value, "vehicle_types.fields_on"))
                .transpose()?,
        )
        .bind(
            patch
                .catalogue
                .as_ref()
                .map(|value| json_to_sql(value, "vehicle_types.catalogue"))
                .transpose()?,
        )
        .bind(template.0)
        .bind(template.1)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<VehicleType>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE vehicle_types SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn type_from_row(row: &SqliteRow) -> Result<(VehicleTypeId, VehicleType), StorageError> {
    let label: String = row.try_get("name")?;
    let kind: String = row.try_get("kind")?;
    let documents: String = row.try_get("documents")?;
    let fields_on: String = row.try_get("fields_on")?;
    let catalogue: String = row.try_get("catalogue")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "vehicle_types.id")?,
        VehicleType {
            name: Label::parse(&label).map_err(invalid("vehicle_types.name"))?,
            icon: required_name::<TypeIcon>(row.try_get("icon")?, "vehicle_types.icon")?,
            kind: AssetKind::parse(&kind).map_err(invalid("vehicle_types.kind"))?,
            documents: json_from_sql(&documents, "vehicle_types.documents")?,
            fields_on: json_from_sql(&fields_on, "vehicle_types.fields_on")?,
            catalogue: json_from_sql(&catalogue, "vehicle_types.catalogue")?,
            template: row
                .try_get::<Option<&[u8]>, _>("template")?
                .map(|bytes| id_from_sql(bytes, "vehicle_types.template"))
                .transpose()?,
        },
    ))
}

impl Database {
    /// One vehicle type, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn vehicle_type(
        &self,
        id: VehicleTypeId,
    ) -> Result<Option<VehicleType>, StorageError> {
        sqlx::query("SELECT * FROM vehicle_types WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(type_from_row)
            .transpose()
            .map(|found| found.map(|(_, kept)| kept))
    }

    /// The household's vehicle types, by name.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn vehicle_types(&self) -> Result<Vec<(VehicleTypeId, VehicleType)>, StorageError> {
        sqlx::query(
            "SELECT * FROM vehicle_types WHERE deleted = 0 ORDER BY name COLLATE NOCASE, id",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(type_from_row)
        .collect()
    }
}
