//! The `custom_roles` table (phase-6.md 4c; sub-plan 05; decision #091): the household's own
//! roles, each a name and its vehicle permissions.

use domain::access::Permissions;
use domain::custom_role::{CustomRole, CustomRoleId, CustomRolePatch};
use domain::id::Id;
use domain::label::Label;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `custom_roles` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct CustomRoleWriter;

impl RecordWriter<CustomRole> for CustomRoleWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<CustomRole>,
        values: &CustomRolePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO custom_roles (id, name, permissions, sequence) VALUES (?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(json_to_sql(
            &values.permissions.unwrap_or_default(),
            "custom_roles.permissions",
        )?)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<CustomRole>,
        patch: &CustomRolePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE custom_roles SET
                 name = coalesce(?, name),
                 permissions = coalesce(?, permissions),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(
            patch
                .permissions
                .map(|permissions| json_to_sql(&permissions, "custom_roles.permissions"))
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
        id: Id<CustomRole>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE custom_roles SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn role_from_row(row: &SqliteRow) -> Result<(CustomRoleId, CustomRole), StorageError> {
    let name: String = row.try_get("name")?;
    let permissions: String = row.try_get("permissions")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "custom_roles.id")?,
        CustomRole {
            name: Label::parse(&name).map_err(invalid("custom_roles.name"))?,
            permissions: json_from_sql::<Permissions>(&permissions, "custom_roles.permissions")?,
        },
    ))
}

impl Database {
    /// One custom role, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn custom_role(&self, id: CustomRoleId) -> Result<Option<CustomRole>, StorageError> {
        sqlx::query("SELECT * FROM custom_roles WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(role_from_row)
            .transpose()
            .map(|found| found.map(|(_, role)| role))
    }

    /// The household's custom roles, by name.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn custom_roles(&self) -> Result<Vec<(CustomRoleId, CustomRole)>, StorageError> {
        sqlx::query("SELECT * FROM custom_roles WHERE deleted = 0 ORDER BY name COLLATE NOCASE, id")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(role_from_row)
            .collect()
    }
}
