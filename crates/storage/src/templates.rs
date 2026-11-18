//! The `schedule_templates` table (Phase 5 milestone 4; decisions #031, #084): the
//! household's named templates, each part kept as JSON.

use domain::id::Id;
use domain::label::Label;
use domain::template::{ScheduleTemplate, ScheduleTemplatePatch, TemplateId};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `schedule_templates` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct TemplateWriter;

/// A part's JSON, if it's in the patch.
fn part<T: serde::Serialize>(
    value: Option<&T>,
    column: &'static str,
) -> Result<Option<String>, StorageError> {
    value.map(|value| json_to_sql(value, column)).transpose()
}

impl RecordWriter<ScheduleTemplate> for TemplateWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ScheduleTemplate>,
        values: &ScheduleTemplatePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO schedule_templates (id, name, meters, requirements, bundles, specs,
                                             sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(json_to_sql(
            values.meters.as_ref().unwrap_or(&Vec::new()),
            "schedule_templates.meters",
        )?)
        .bind(json_to_sql(
            values.requirements.as_ref().unwrap_or(&Vec::new()),
            "schedule_templates.requirements",
        )?)
        .bind(json_to_sql(
            values.bundles.as_ref().unwrap_or(&Vec::new()),
            "schedule_templates.bundles",
        )?)
        .bind(json_to_sql(
            values.specs.as_ref().unwrap_or(&Vec::new()),
            "schedule_templates.specs",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ScheduleTemplate>,
        patch: &ScheduleTemplatePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE schedule_templates SET
                 name = coalesce(?, name),
                 meters = coalesce(?, meters),
                 requirements = coalesce(?, requirements),
                 bundles = coalesce(?, bundles),
                 specs = coalesce(?, specs),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(part(patch.meters.as_ref(), "schedule_templates.meters")?)
        .bind(part(
            patch.requirements.as_ref(),
            "schedule_templates.requirements",
        )?)
        .bind(part(patch.bundles.as_ref(), "schedule_templates.bundles")?)
        .bind(part(patch.specs.as_ref(), "schedule_templates.specs")?)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ScheduleTemplate>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE schedule_templates SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn template_from_row(row: &SqliteRow) -> Result<(TemplateId, ScheduleTemplate), StorageError> {
    let name: String = row.try_get("name")?;
    let text = |column: &'static str| row.try_get::<String, _>(column);
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "schedule_templates.id")?,
        ScheduleTemplate {
            name: Label::parse(&name).map_err(invalid("schedule_templates.name"))?,
            meters: json_from_sql(&text("meters")?, "schedule_templates.meters")?,
            requirements: json_from_sql(&text("requirements")?, "schedule_templates.requirements")?,
            bundles: json_from_sql(&text("bundles")?, "schedule_templates.bundles")?,
            specs: json_from_sql(&text("specs")?, "schedule_templates.specs")?,
            usages: vec![],
        },
    ))
}

impl Database {
    /// One template, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn template(&self, id: TemplateId) -> Result<Option<ScheduleTemplate>, StorageError> {
        sqlx::query("SELECT * FROM schedule_templates WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| template_from_row(row).map(|(_, template)| template))
            .transpose()
    }

    /// The household's templates (not deleted), by name (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn templates(&self) -> Result<Vec<(TemplateId, ScheduleTemplate)>, StorageError> {
        sqlx::query(
            "SELECT * FROM schedule_templates WHERE deleted = 0 ORDER BY name COLLATE NOCASE, id",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(template_from_row)
        .collect()
    }
}
