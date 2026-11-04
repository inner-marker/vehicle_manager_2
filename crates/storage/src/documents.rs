//! The `asset_documents` and `license_plates` tables (milestone 8; decision #066): their
//! writers for the command pipeline, and the queries the server needs.

use domain::asset::AssetId;
use domain::dates::CalendarDate;
use domain::document::{AssetDocument, AssetDocumentId, AssetDocumentPatch, DocumentType};
use domain::id::Id;
use domain::label::Label;
use domain::notes::Notes;
use domain::plate::{Authority, LicensePlate, LicensePlateId, LicensePlatePatch, PlateNumber};
use domain::time::UnixMillis;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{
    clearable, id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required,
};
use crate::{Database, StorageError};

fn date_to_sql(date: CalendarDate) -> String {
    String::from(date)
}

fn date_from_row(
    row: &SqliteRow,
    column: &'static str,
) -> Result<Option<CalendarDate>, StorageError> {
    row.try_get::<Option<String>, _>(column)?
        .map(|text| CalendarDate::parse(&text).map_err(invalid(column)))
        .transpose()
}

fn optional_id<T>(row: &SqliteRow, column: &'static str) -> Result<Option<Id<T>>, StorageError> {
    row.try_get::<Option<&[u8]>, _>(column)?
        .map(|bytes| id_from_sql(bytes, column))
        .transpose()
}

// --- License plates ------------------------------------------------------------------------

/// Writes the `license_plates` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct LicensePlateWriter;

impl RecordWriter<LicensePlate> for LicensePlateWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<LicensePlate>,
        values: &LicensePlatePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let date = |value: &Option<domain::command::Clearable<CalendarDate>>| {
            value
                .clone()
                .and_then(domain::command::Clearable::into_option)
                .map(date_to_sql)
        };
        let authority = values
            .authority
            .clone()
            .and_then(domain::command::Clearable::into_option)
            .map(|authority| json_to_sql(&authority, "license_plates.authority"))
            .transpose()?;
        sqlx::query(
            "INSERT INTO license_plates
                 (id, asset_id, authority, number, issued_on, expires_on, retired_on, notes,
                  added_ms, added_by, received_ms, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(
            values.asset,
            "license_plates.asset_id",
        )?))
        .bind(authority)
        .bind(required(values.number.as_ref(), "license_plates.number")?.as_str())
        .bind(date(&values.issued_on))
        .bind(date(&values.expires_on))
        .bind(date(&values.retired_on))
        .bind(values.notes.as_ref().map_or("", Notes::as_str).to_owned())
        .bind(required(values.added_at, "license_plates.added_ms")?.as_millis())
        .bind(id_to_sql(required(
            values.added_by,
            "license_plates.added_by",
        )?))
        .bind(required(values.received_at, "license_plates.received_ms")?.as_millis())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<LicensePlate>,
        patch: &LicensePlatePatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle, who added it, and when it arrived are fixed once added.
        let authority = match patch.authority.clone() {
            None => (false, None),
            Some(change) => (
                true,
                change
                    .into_option()
                    .map(|authority| json_to_sql(&authority, "license_plates.authority"))
                    .transpose()?,
            ),
        };
        let issued = clearable(patch.issued_on.as_ref(), date_to_sql);
        let expires = clearable(patch.expires_on.as_ref(), date_to_sql);
        let retired = clearable(patch.retired_on.as_ref(), date_to_sql);
        sqlx::query(
            "UPDATE license_plates SET
                 authority  = CASE WHEN ? THEN ? ELSE authority END,
                 number     = coalesce(?, number),
                 issued_on  = CASE WHEN ? THEN ? ELSE issued_on END,
                 expires_on = CASE WHEN ? THEN ? ELSE expires_on END,
                 retired_on = CASE WHEN ? THEN ? ELSE retired_on END,
                 notes      = coalesce(?, notes),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(authority.0)
        .bind(authority.1)
        .bind(
            patch
                .number
                .as_ref()
                .map(|number| number.as_str().to_owned()),
        )
        .bind(issued.0)
        .bind(issued.1)
        .bind(expires.0)
        .bind(expires.1)
        .bind(retired.0)
        .bind(retired.1)
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
        id: Id<LicensePlate>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE license_plates SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn plate_from_row(row: &SqliteRow) -> Result<(LicensePlateId, LicensePlate), StorageError> {
    let number: String = row.try_get("number")?;
    let notes: String = row.try_get("notes")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "license_plates.id")?,
        LicensePlate {
            asset: id_from_sql(
                row.try_get::<&[u8], _>("asset_id")?,
                "license_plates.asset_id",
            )?,
            authority: row
                .try_get::<Option<String>, _>("authority")?
                .map(|text| json_from_sql::<Authority>(&text, "license_plates.authority"))
                .transpose()?,
            number: PlateNumber::parse(&number).map_err(invalid("license_plates.number"))?,
            issued_on: date_from_row(row, "issued_on")?,
            expires_on: date_from_row(row, "expires_on")?,
            retired_on: date_from_row(row, "retired_on")?,
            notes: Notes::parse(&notes).map_err(invalid("license_plates.notes"))?,
            added_at: UnixMillis::from_millis(row.try_get("added_ms")?),
            added_by: id_from_sql(
                row.try_get::<&[u8], _>("added_by")?,
                "license_plates.added_by",
            )?,
            received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        },
    ))
}

// --- Official documents --------------------------------------------------------------------

/// A document type as its two columns: (key, name), one of them set.
fn type_to_sql(doc_type: &DocumentType) -> (Option<String>, Option<String>) {
    match doc_type {
        DocumentType::Listed { key } => (Some(key.clone()), None),
        DocumentType::Custom { name } => (None, Some(name.as_str().to_owned())),
    }
}

/// Writes the `asset_documents` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct AssetDocumentWriter;

impl RecordWriter<AssetDocument> for AssetDocumentWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<AssetDocument>,
        values: &AssetDocumentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let option = |value: &Option<domain::command::Clearable<String>>| {
            value
                .clone()
                .and_then(domain::command::Clearable::into_option)
        };
        let date = |value: &Option<domain::command::Clearable<CalendarDate>>| {
            value
                .clone()
                .and_then(domain::command::Clearable::into_option)
                .map(date_to_sql)
        };
        let (type_key, type_name) = type_to_sql(required(
            values.doc_type.as_ref(),
            "asset_documents.type_key",
        )?);
        sqlx::query(
            "INSERT INTO asset_documents
                 (id, asset_id, type_key, type_name, number, issuer, issued_on, effective_on,
                  expires_on, supersedes_id, plate_id, notes, added_ms, added_by, received_ms,
                  sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(
            values.asset,
            "asset_documents.asset_id",
        )?))
        .bind(type_key)
        .bind(type_name)
        .bind(option(&values.number))
        .bind(option(&values.issuer))
        .bind(date(&values.issued_on))
        .bind(date(&values.effective_on))
        .bind(date(&values.expires_on))
        .bind(
            values
                .supersedes
                .and_then(domain::command::Clearable::into_option)
                .map(id_to_sql),
        )
        .bind(
            values
                .plate
                .and_then(domain::command::Clearable::into_option)
                .map(id_to_sql),
        )
        .bind(values.notes.as_ref().map_or("", Notes::as_str).to_owned())
        .bind(required(values.added_at, "asset_documents.added_ms")?.as_millis())
        .bind(id_to_sql(required(
            values.added_by,
            "asset_documents.added_by",
        )?))
        .bind(required(values.received_at, "asset_documents.received_ms")?.as_millis())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<AssetDocument>,
        patch: &AssetDocumentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // The vehicle, what it renews, who added it, and when it arrived are fixed once added.
        let (type_changed, (type_key, type_name)) = patch
            .doc_type
            .as_ref()
            .map_or((false, (None, None)), |doc_type| {
                (true, type_to_sql(doc_type))
            });
        let number = clearable(patch.number.as_ref(), |text| text);
        let issued_by = clearable(patch.issuer.as_ref(), |text| text);
        let issued = clearable(patch.issued_on.as_ref(), date_to_sql);
        let effective = clearable(patch.effective_on.as_ref(), date_to_sql);
        let expires = clearable(patch.expires_on.as_ref(), date_to_sql);
        let plate = clearable(patch.plate.as_ref(), id_to_sql);
        sqlx::query(
            "UPDATE asset_documents SET
                 type_key     = CASE WHEN ? THEN ? ELSE type_key END,
                 type_name    = CASE WHEN ? THEN ? ELSE type_name END,
                 number       = CASE WHEN ? THEN ? ELSE number END,
                 issuer       = CASE WHEN ? THEN ? ELSE issuer END,
                 issued_on    = CASE WHEN ? THEN ? ELSE issued_on END,
                 effective_on = CASE WHEN ? THEN ? ELSE effective_on END,
                 expires_on   = CASE WHEN ? THEN ? ELSE expires_on END,
                 plate_id     = CASE WHEN ? THEN ? ELSE plate_id END,
                 notes        = coalesce(?, notes),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(type_changed)
        .bind(type_key)
        .bind(type_changed)
        .bind(type_name)
        .bind(number.0)
        .bind(number.1)
        .bind(issued_by.0)
        .bind(issued_by.1)
        .bind(issued.0)
        .bind(issued.1)
        .bind(effective.0)
        .bind(effective.1)
        .bind(expires.0)
        .bind(expires.1)
        .bind(plate.0)
        .bind(plate.1)
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
        id: Id<AssetDocument>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE asset_documents SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn document_from_row(row: &SqliteRow) -> Result<(AssetDocumentId, AssetDocument), StorageError> {
    let notes: String = row.try_get("notes")?;
    let doc_type = match (
        row.try_get::<Option<String>, _>("type_key")?,
        row.try_get::<Option<String>, _>("type_name")?,
    ) {
        (Some(key), None) => {
            DocumentType::listed(&key).map_err(invalid("asset_documents.type_key"))?
        }
        (None, Some(name)) => DocumentType::Custom {
            name: Label::parse(&name).map_err(invalid("asset_documents.type_name"))?,
        },
        (Some(_), Some(_)) | (None, None) => {
            return Err(StorageError::InvalidValue {
                column: "asset_documents.type_key",
            });
        }
    };
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "asset_documents.id")?,
        AssetDocument {
            asset: id_from_sql(
                row.try_get::<&[u8], _>("asset_id")?,
                "asset_documents.asset_id",
            )?,
            doc_type,
            number: row.try_get("number")?,
            issuer: row.try_get("issuer")?,
            issued_on: date_from_row(row, "issued_on")?,
            effective_on: date_from_row(row, "effective_on")?,
            expires_on: date_from_row(row, "expires_on")?,
            supersedes: optional_id(row, "supersedes_id")?,
            plate: optional_id(row, "plate_id")?,
            notes: Notes::parse(&notes).map_err(invalid("asset_documents.notes"))?,
            added_at: UnixMillis::from_millis(row.try_get("added_ms")?),
            added_by: id_from_sql(
                row.try_get::<&[u8], _>("added_by")?,
                "asset_documents.added_by",
            )?,
            received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        },
    ))
}

impl Database {
    /// One vehicle's license plates (not removed), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn plates_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(LicensePlateId, LicensePlate)>, StorageError> {
        sqlx::query(
            "SELECT * FROM license_plates WHERE asset_id = ? AND deleted = 0
             ORDER BY coalesce(issued_on, ''), id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(plate_from_row)
        .collect()
    }

    /// One license plate, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn plate(&self, id: LicensePlateId) -> Result<Option<LicensePlate>, StorageError> {
        sqlx::query("SELECT * FROM license_plates WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| plate_from_row(row).map(|(_, plate)| plate))
            .transpose()
    }

    /// One vehicle's official documents (not removed), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn documents_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(AssetDocumentId, AssetDocument)>, StorageError> {
        sqlx::query(
            "SELECT * FROM asset_documents WHERE asset_id = ? AND deleted = 0
             ORDER BY added_ms, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(document_from_row)
        .collect()
    }

    /// One official document, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn document(
        &self,
        id: AssetDocumentId,
    ) -> Result<Option<AssetDocument>, StorageError> {
        sqlx::query("SELECT * FROM asset_documents WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| document_from_row(row).map(|(_, document)| document))
            .transpose()
    }
}
