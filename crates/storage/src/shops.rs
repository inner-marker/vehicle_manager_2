//! The `shops` table (phase-6.md 2f; decision #089): the household's shops, each with a contact
//! and, if it differs from the household's, its own labor rate.

use domain::id::Id;
use domain::label::Label;
use domain::shop::{LaborRate, Shop, ShopId, ShopPatch};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{clearable, id_from_sql, id_to_sql, invalid, required};
use crate::{Database, StorageError};

/// Writes the `shops` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct ShopWriter;

impl RecordWriter<Shop> for ShopWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Shop>,
        values: &ShopPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let (_, contact) = clearable(values.contact.as_ref(), |contact| contact);
        let (_, rate) = clearable(values.labor_rate.as_ref(), LaborRate::minor_per_hour);
        sqlx::query(
            "INSERT INTO shops (id, name, contact, labor_rate, sequence) VALUES (?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.name.as_ref(), "name")?.as_str())
        .bind(contact)
        .bind(rate)
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Shop>,
        patch: &ShopPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // A flag for each clearable column, since clearing one (NULL) is a real change that
        // `coalesce` can't express.
        let (contact_changes, contact) = clearable(patch.contact.as_ref(), |contact| contact);
        let (rate_changes, rate) = clearable(patch.labor_rate.as_ref(), LaborRate::minor_per_hour);
        sqlx::query(
            "UPDATE shops SET
                 name = coalesce(?, name),
                 contact = CASE WHEN ? THEN ? ELSE contact END,
                 labor_rate = CASE WHEN ? THEN ? ELSE labor_rate END,
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.name.as_ref().map(|name| name.as_str().to_owned()))
        .bind(contact_changes)
        .bind(contact)
        .bind(rate_changes)
        .bind(rate)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Shop>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE shops SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn shop_from_row(row: &SqliteRow) -> Result<(ShopId, Shop), StorageError> {
    let name: String = row.try_get("name")?;
    let rate = row
        .try_get::<Option<i64>, _>("labor_rate")?
        .map(LaborRate::new)
        .transpose()
        .map_err(invalid("shops.labor_rate"))?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "shops.id")?,
        Shop {
            name: Label::parse(&name).map_err(invalid("shops.name"))?,
            contact: row.try_get("contact")?,
            labor_rate: rate,
        },
    ))
}

impl Database {
    /// One shop, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn shop(&self, id: ShopId) -> Result<Option<Shop>, StorageError> {
        sqlx::query("SELECT * FROM shops WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(shop_from_row)
            .transpose()
            .map(|found| found.map(|(_, shop)| shop))
    }

    /// The household's shops, by name.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn shops(&self) -> Result<Vec<(ShopId, Shop)>, StorageError> {
        sqlx::query("SELECT * FROM shops WHERE deleted = 0 ORDER BY name COLLATE NOCASE, id")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(shop_from_row)
            .collect()
    }
}
