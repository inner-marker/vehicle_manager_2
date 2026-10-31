//! The `preferences` table: the household's display preferences and each user's (#029, #048).
//!
//! Values are stored as the same names they have in JSON (e.g. `kilometers`), so the table
//! reads the same as the API. NULL means "not chosen".

use domain::id::Id;
use domain::preferences::{
    Choice, PreferenceChoices, PreferencesId, PreferencesPatch, PreferencesRecord,
};
use serde::Serialize;
use serde::de::DeserializeOwned;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::id_to_sql;
use crate::{Database, StorageError};

/// A value's stored name: its JSON string (every preference serializes to a string).
fn to_sql<T: Serialize>(value: &T) -> Option<String> {
    serde_json::to_value(value)
        .ok()?
        .as_str()
        .map(str::to_owned)
}

/// A stored name back to its value; a name this build doesn't know reads as "not chosen".
fn from_sql<T: DeserializeOwned>(name: Option<String>) -> Option<T> {
    serde_json::from_value(serde_json::Value::String(name?)).ok()
}

/// What happens to one column.
enum ColumnChange {
    /// Left as it is.
    Unchanged,
    /// Set to this name, or to NULL ("not chosen").
    Set(Option<String>),
}

impl ColumnChange {
    fn of<T: Serialize + Copy>(choice: Option<Choice<T>>) -> Self {
        choice.map_or(Self::Unchanged, |choice| {
            Self::Set(choice.value().as_ref().and_then(to_sql))
        })
    }

    const fn is_change(&self) -> bool {
        matches!(self, Self::Set(_))
    }

    fn value(self) -> Option<String> {
        match self {
            Self::Unchanged => None,
            Self::Set(value) => value,
        }
    }
}

/// Every setting's change, in column order.
fn changes(patch: &PreferencesPatch) -> [ColumnChange; 7] {
    [
        ColumnChange::of(patch.distance),
        ColumnChange::of(patch.volume),
        ColumnChange::of(patch.economy),
        ColumnChange::of(patch.pressure),
        ColumnChange::of(patch.date_format),
        ColumnChange::of(patch.currency),
        ColumnChange::of(patch.language),
    ]
}

/// Writes the `preferences` table for the command pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct PreferencesWriter;

impl RecordWriter<PreferencesRecord> for PreferencesWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<PreferencesRecord>,
        values: &PreferencesPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let [
            distance,
            volume,
            economy,
            pressure,
            date_format,
            currency,
            language,
        ] = changes(values);
        sqlx::query(
            "INSERT INTO preferences
                 (id, distance, volume, economy, pressure, date_format, currency, language, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(distance.value())
        .bind(volume.value())
        .bind(economy.value())
        .bind(pressure.value())
        .bind(date_format.value())
        .bind(currency.value())
        .bind(language.value())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<PreferencesRecord>,
        patch: &PreferencesPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // Each column has a "changed?" flag and a value, because setting a column back to
        // NULL ("not chosen") is a real change that `coalesce` can't express.
        let mut query = sqlx::query(
            "UPDATE preferences SET
                 distance    = CASE WHEN ? THEN ? ELSE distance END,
                 volume      = CASE WHEN ? THEN ? ELSE volume END,
                 economy     = CASE WHEN ? THEN ? ELSE economy END,
                 pressure    = CASE WHEN ? THEN ? ELSE pressure END,
                 date_format = CASE WHEN ? THEN ? ELSE date_format END,
                 currency    = CASE WHEN ? THEN ? ELSE currency END,
                 language    = CASE WHEN ? THEN ? ELSE language END,
                 sequence = ?
             WHERE id = ?",
        );
        for column in changes(patch) {
            query = query.bind(column.is_change()).bind(column.value());
        }
        query
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<PreferencesRecord>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE preferences SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn choices_from_row(row: &SqliteRow) -> Result<PreferenceChoices, StorageError> {
    Ok(PreferenceChoices {
        distance: from_sql(row.try_get("distance")?),
        volume: from_sql(row.try_get("volume")?),
        economy: from_sql(row.try_get("economy")?),
        pressure: from_sql(row.try_get("pressure")?),
        date_format: from_sql(row.try_get("date_format")?),
        currency: from_sql(row.try_get("currency")?),
        language: from_sql(row.try_get("language")?),
    })
}

impl Database {
    /// The choices stored in one preferences record (all "not chosen" if it doesn't exist).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn preference_choices(
        &self,
        id: PreferencesId,
    ) -> Result<PreferenceChoices, StorageError> {
        sqlx::query(
            "SELECT distance, volume, economy, pressure, date_format, currency, language
             FROM preferences WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map_or_else(|| Ok(PreferenceChoices::default()), choices_from_row)
    }

    /// Whether a preferences record exists (the first change creates it; later ones update it).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn preferences_exist(&self, id: PreferencesId) -> Result<bool, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM preferences WHERE id = ?)")
                .bind(id_to_sql(id))
                .fetch_one(self.pool())
                .await?,
        )
    }
}
