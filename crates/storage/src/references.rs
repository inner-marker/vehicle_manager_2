//! The `reference_documents` table (milestone 9; decision #068): recalls, bulletins, and
//! schedules shared by the household. The writer for the command pipeline, and the queries the
//! server needs. Which vehicles link one comes from their requirements
//! ([`Database::requirements_linking`]).

use domain::command::Clearable;
use domain::dates::CalendarDate;
use domain::id::Id;
use domain::notes::Notes;
use domain::reference::{
    ReferenceDocument, ReferenceDocumentId, ReferenceDocumentPatch, ReferenceNumber, ReferenceType,
};
use domain::time::UnixMillis;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, required};
use crate::{Database, StorageError};

/// Writes the `reference_documents` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct ReferenceDocumentWriter;

/// Sets `target` to `value`, if there is one.
fn set<T>(target: &mut T, value: Option<T>) {
    if let Some(value) = value {
        *target = value;
    }
}

/// Sets or clears `target` as `value` says, if there is one.
fn clear<T>(target: &mut Option<T>, value: Option<Clearable<T>>) {
    if let Some(value) = value {
        *target = value.into_option();
    }
}

/// `document` with `patch` applied; who added it and when stay as they were.
fn applied(mut document: ReferenceDocument, patch: ReferenceDocumentPatch) -> ReferenceDocument {
    let ReferenceDocumentPatch {
        doc_type,
        authority,
        numbers,
        title,
        description,
        remedy,
        issued_on,
        effective_on,
        revision,
        supersedes,
        applicability,
        url,
        ..
    } = patch;
    set(&mut document.doc_type, doc_type);
    clear(&mut document.authority, authority);
    set(&mut document.numbers, numbers);
    set(&mut document.title, title);
    set(&mut document.description, description);
    set(&mut document.remedy, remedy);
    clear(&mut document.issued_on, issued_on);
    clear(&mut document.effective_on, effective_on);
    clear(&mut document.revision, revision);
    clear(&mut document.supersedes, supersedes);
    set(&mut document.applicability, applicability);
    clear(&mut document.url, url);
    document
}

/// A new document from a create's values.
fn from_values(values: &ReferenceDocumentPatch) -> Result<ReferenceDocument, StorageError> {
    let option = |value: &Option<Clearable<String>>| value.clone().and_then(Clearable::into_option);
    let date =
        |value: &Option<Clearable<CalendarDate>>| value.clone().and_then(Clearable::into_option);
    Ok(ReferenceDocument {
        doc_type: required(values.doc_type.clone(), "reference_documents.doc_type")?,
        authority: option(&values.authority),
        numbers: values.numbers.clone().unwrap_or_default(),
        title: required(values.title.clone(), "reference_documents.title")?,
        description: values.description.clone().unwrap_or_default(),
        remedy: values.remedy.clone().unwrap_or_default(),
        issued_on: date(&values.issued_on),
        effective_on: date(&values.effective_on),
        revision: option(&values.revision),
        supersedes: values.supersedes.and_then(Clearable::into_option),
        applicability: values.applicability.clone().unwrap_or_default(),
        url: option(&values.url),
        added_at: required(values.added_at, "reference_documents.added_ms")?,
        added_by: required(values.added_by, "reference_documents.added_by")?,
        received_at: required(values.received_at, "reference_documents.received_ms")?,
    })
}

/// Writes every changeable column of `document` (the insert's or the update's values, in the
/// order both statements list them).
fn bind_document<'q>(
    query: sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>,
    document: &ReferenceDocument,
) -> Result<sqlx::query::Query<'q, sqlx::Sqlite, sqlx::sqlite::SqliteArguments>, StorageError> {
    Ok(query
        .bind(json_to_sql(
            &document.doc_type,
            "reference_documents.doc_type",
        )?)
        .bind(document.authority.clone())
        .bind(json_to_sql(
            &document.numbers,
            "reference_documents.numbers",
        )?)
        .bind(document.title.clone())
        .bind(document.description.as_str().to_owned())
        .bind(document.remedy.as_str().to_owned())
        .bind(document.issued_on.clone().map(String::from))
        .bind(document.effective_on.clone().map(String::from))
        .bind(document.revision.clone())
        .bind(document.supersedes.map(id_to_sql))
        .bind(document.applicability.as_str().to_owned())
        .bind(document.url.clone()))
}

impl RecordWriter<ReferenceDocument> for ReferenceDocumentWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ReferenceDocument>,
        values: &ReferenceDocumentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let document = from_values(values)?;
        let query = sqlx::query(
            "INSERT INTO reference_documents
                 (doc_type, authority, numbers, title, description, remedy, issued_on,
                  effective_on, revision, supersedes_id, applicability, url, added_ms, added_by,
                  received_ms, sequence, id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        bind_document(query, &document)?
            .bind(document.added_at.as_millis())
            .bind(id_to_sql(document.added_by))
            .bind(document.received_at.as_millis())
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ReferenceDocument>,
        patch: &ReferenceDocumentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let row = sqlx::query("SELECT * FROM reference_documents WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(&mut *connection)
            .await?
            .ok_or(StorageError::InvalidValue {
                column: "reference_documents.id",
            })?;
        let (_, current) = reference_from_row(&row)?;
        let document = applied(current, patch.clone());
        let query = sqlx::query(
            "UPDATE reference_documents SET
                 doc_type = ?, authority = ?, numbers = ?, title = ?, description = ?,
                 remedy = ?, issued_on = ?, effective_on = ?, revision = ?, supersedes_id = ?,
                 applicability = ?, url = ?, sequence = ?
             WHERE id = ?",
        );
        bind_document(query, &document)?
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<ReferenceDocument>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE reference_documents SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn reference_from_row(
    row: &SqliteRow,
) -> Result<(ReferenceDocumentId, ReferenceDocument), StorageError> {
    let text = |column: &'static str| -> Result<String, StorageError> { Ok(row.try_get(column)?) };
    let date = |column: &'static str| -> Result<Option<CalendarDate>, StorageError> {
        row.try_get::<Option<String>, _>(column)?
            .map(|text| CalendarDate::parse(&text).map_err(invalid(column)))
            .transpose()
    };
    let notes = |column: &'static str| -> Result<Notes, StorageError> {
        Notes::parse(&text(column)?).map_err(invalid(column))
    };
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "reference_documents.id")?,
        ReferenceDocument {
            doc_type: json_from_sql::<ReferenceType>(
                &text("doc_type")?,
                "reference_documents.doc_type",
            )?,
            authority: row.try_get("authority")?,
            numbers: json_from_sql::<Vec<ReferenceNumber>>(
                &text("numbers")?,
                "reference_documents.numbers",
            )?,
            title: text("title")?,
            description: notes("description")?,
            remedy: notes("remedy")?,
            issued_on: date("issued_on")?,
            effective_on: date("effective_on")?,
            revision: row.try_get("revision")?,
            supersedes: row
                .try_get::<Option<Vec<u8>>, _>("supersedes_id")?
                .map(|bytes| id_from_sql(&bytes, "reference_documents.supersedes_id"))
                .transpose()?,
            applicability: notes("applicability")?,
            url: row.try_get("url")?,
            added_at: UnixMillis::from_millis(row.try_get("added_ms")?),
            added_by: id_from_sql(
                row.try_get::<&[u8], _>("added_by")?,
                "reference_documents.added_by",
            )?,
            received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        },
    ))
}

impl Database {
    /// One reference document, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn reference_document(
        &self,
        id: ReferenceDocumentId,
    ) -> Result<Option<ReferenceDocument>, StorageError> {
        sqlx::query("SELECT * FROM reference_documents WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| reference_from_row(row).map(|(_, document)| document))
            .transpose()
    }

    /// Every reference document the household has (not removed), by title (ignoring case).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn reference_documents(
        &self,
    ) -> Result<Vec<(ReferenceDocumentId, ReferenceDocument)>, StorageError> {
        sqlx::query(
            "SELECT * FROM reference_documents WHERE deleted = 0
             ORDER BY title COLLATE NOCASE, id",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(reference_from_row)
        .collect()
    }
}
