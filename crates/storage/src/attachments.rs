//! The `attachments` table (milestone 8; decisions #036, #037, #065; a reference document's,
//! on no vehicle, from milestone 9, #068): the attachment writer for
//! the command pipeline, and the queries the server needs. The files themselves are in the
//! file store ([`crate::files`]).

use domain::asset::AssetId;
use domain::attachment::{
    Attachment, AttachmentId, AttachmentOwner, AttachmentPatch, Content, FileHash, FileName,
    StoredFile,
};
use domain::id::Id;
use domain::reference::ReferenceDocumentId;
use domain::time::UnixMillis;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, name, required};
use crate::{Database, StorageError};

/// An owner as its columns.
#[derive(Debug, Default)]
struct OwnerColumns {
    owner: &'static str,
    event: Option<Vec<u8>>,
    task: Option<i64>,
    squawk: Option<Vec<u8>>,
    document: Option<Vec<u8>>,
    reference: Option<Vec<u8>>,
}

fn owner_to_sql(owner: AttachmentOwner) -> OwnerColumns {
    match owner {
        AttachmentOwner::Vehicle => OwnerColumns {
            owner: "vehicle",
            ..OwnerColumns::default()
        },
        AttachmentOwner::Event { event, task } => OwnerColumns {
            owner: "event",
            event: Some(id_to_sql(event)),
            task: task.map(i64::from),
            ..OwnerColumns::default()
        },
        AttachmentOwner::Squawk { squawk } => OwnerColumns {
            owner: "squawk",
            squawk: Some(id_to_sql(squawk)),
            ..OwnerColumns::default()
        },
        AttachmentOwner::Document { document } => OwnerColumns {
            owner: "document",
            document: Some(id_to_sql(document)),
            ..OwnerColumns::default()
        },
        AttachmentOwner::Reference { reference } => OwnerColumns {
            owner: "reference",
            reference: Some(id_to_sql(reference)),
            ..OwnerColumns::default()
        },
    }
}

fn owner_from_row(row: &SqliteRow) -> Result<AttachmentOwner, StorageError> {
    let id = |column: &'static str| -> Result<Option<Vec<u8>>, StorageError> {
        Ok(row.try_get::<Option<Vec<u8>>, _>(column)?)
    };
    let owner: String = row.try_get("owner")?;
    let column = "attachments.owner";
    match owner.as_str() {
        "vehicle" => Ok(AttachmentOwner::Vehicle),
        "event" => Ok(AttachmentOwner::Event {
            event: id_from_sql(
                &id("event_id")?.ok_or(StorageError::InvalidValue { column })?,
                column,
            )?,
            task: row
                .try_get::<Option<i64>, _>("task_position")?
                .map(|task| u16::try_from(task).map_err(invalid("attachments.task_position")))
                .transpose()?,
        }),
        "squawk" => Ok(AttachmentOwner::Squawk {
            squawk: id_from_sql(
                &id("squawk_id")?.ok_or(StorageError::InvalidValue { column })?,
                column,
            )?,
        }),
        "document" => Ok(AttachmentOwner::Document {
            document: id_from_sql(
                &id("document_id")?.ok_or(StorageError::InvalidValue { column })?,
                column,
            )?,
        }),
        "reference" => Ok(AttachmentOwner::Reference {
            reference: id_from_sql(
                &id("reference_id")?.ok_or(StorageError::InvalidValue { column })?,
                column,
            )?,
        }),
        _ => Err(StorageError::InvalidValue { column }),
    }
}

/// A file's hash and size as columns.
fn file_columns(file: &StoredFile, column: &'static str) -> Result<(String, i64), StorageError> {
    Ok((
        file.hash.as_str().to_owned(),
        i64::try_from(file.bytes).map_err(invalid(column))?,
    ))
}

/// Writes the `attachments` table.
#[derive(Debug, Clone, Copy, Default)]
pub struct AttachmentWriter;

impl RecordWriter<Attachment> for AttachmentWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Attachment>,
        values: &AttachmentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let owner = owner_to_sql(required(values.owner, "attachments.owner")?);
        let content = required(values.content.as_ref(), "attachments.content")?;
        let (file, file_bytes) = file_columns(
            required(values.file.as_ref(), "attachments.file_sha256")?,
            "attachments.file_bytes",
        )?;
        let thumbnail = values
            .thumbnail
            .as_ref()
            .and_then(Option::as_ref)
            .map(|thumbnail| file_columns(thumbnail, "attachments.thumbnail_bytes"))
            .transpose()?;
        sqlx::query(
            "INSERT INTO attachments
                 (id, asset_id, owner, event_id, task_position, squawk_id, document_id,
                  reference_id, kind, file_sha256, file_bytes, thumbnail_sha256, thumbnail_bytes,
                  name, content, added_ms, added_by, received_ms, sequence)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.asset, "attachments.asset_id")?.map(id_to_sql))
        .bind(owner.owner)
        .bind(owner.event)
        .bind(owner.task)
        .bind(owner.squawk)
        .bind(owner.document)
        .bind(owner.reference)
        .bind(name(&content.kind(), "attachments.kind")?)
        .bind(file)
        .bind(file_bytes)
        .bind(thumbnail.as_ref().map(|(hash, _)| hash.clone()))
        .bind(thumbnail.map(|(_, bytes)| bytes))
        .bind(required(values.name.as_ref(), "attachments.name")?.as_str())
        .bind(json_to_sql(content, "attachments.content")?)
        .bind(required(values.added_at, "attachments.added_ms")?.as_millis())
        .bind(id_to_sql(required(
            values.added_by,
            "attachments.added_by",
        )?))
        .bind(required(values.received_at, "attachments.received_ms")?.as_millis())
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Attachment>,
        patch: &AttachmentPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // Only its owner changes (the server refuses anything else, `only_moves`).
        let Some(owner) = patch.owner else {
            return Ok(());
        };
        let owner = owner_to_sql(owner);
        sqlx::query(
            "UPDATE attachments SET owner = ?, event_id = ?, task_position = ?, squawk_id = ?,
                                    document_id = ?, reference_id = ?, sequence = ?
             WHERE id = ?",
        )
        .bind(owner.owner)
        .bind(owner.event)
        .bind(owner.task)
        .bind(owner.squawk)
        .bind(owner.document)
        .bind(owner.reference)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Attachment>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE attachments SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn file_from_row(
    row: &SqliteRow,
    hash: &'static str,
    bytes: &'static str,
) -> Result<Option<StoredFile>, StorageError> {
    let Some(text) = row.try_get::<Option<String>, _>(hash)? else {
        return Ok(None);
    };
    let size: i64 = row.try_get(bytes)?;
    Ok(Some(StoredFile {
        hash: FileHash::parse(&text).map_err(invalid(hash))?,
        bytes: u64::try_from(size).map_err(invalid(bytes))?,
    }))
}

fn attachment_from_row(row: &SqliteRow) -> Result<(AttachmentId, Attachment), StorageError> {
    let name: String = row.try_get("name")?;
    let content: String = row.try_get("content")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "attachments.id")?,
        Attachment {
            asset: row
                .try_get::<Option<Vec<u8>>, _>("asset_id")?
                .map(|bytes| id_from_sql(&bytes, "attachments.asset_id"))
                .transpose()?,
            owner: owner_from_row(row)?,
            file: file_from_row(row, "file_sha256", "file_bytes")?.ok_or(
                StorageError::InvalidValue {
                    column: "attachments.file_sha256",
                },
            )?,
            thumbnail: file_from_row(row, "thumbnail_sha256", "thumbnail_bytes")?,
            name: FileName::try_from(name).map_err(invalid("attachments.name"))?,
            content: json_from_sql::<Content>(&content, "attachments.content")?,
            added_at: UnixMillis::from_millis(row.try_get("added_ms")?),
            added_by: id_from_sql(row.try_get::<&[u8], _>("added_by")?, "attachments.added_by")?,
            received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        },
    ))
}

impl Database {
    /// One attachment, if it exists and isn't removed.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn attachment(&self, id: AttachmentId) -> Result<Option<Attachment>, StorageError> {
        sqlx::query("SELECT * FROM attachments WHERE id = ? AND deleted = 0")
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(|row| attachment_from_row(row).map(|(_, attachment)| attachment))
            .transpose()
    }

    /// One vehicle's attachments (on anything), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn attachments_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(AttachmentId, Attachment)>, StorageError> {
        sqlx::query(
            "SELECT * FROM attachments WHERE asset_id = ? AND deleted = 0
             ORDER BY added_ms, id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(attachment_from_row)
        .collect()
    }

    /// What's attached to `owner` on `asset` (an event's includes its tasks'), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn attachments_on(
        &self,
        asset: AssetId,
        owner: AttachmentOwner,
    ) -> Result<Vec<(AttachmentId, Attachment)>, StorageError> {
        let same = |candidate: AttachmentOwner| match (owner, candidate) {
            (AttachmentOwner::Event { event, .. }, AttachmentOwner::Event { event: other, .. }) => {
                event == other
            }
            (
                AttachmentOwner::Vehicle
                | AttachmentOwner::Event { .. }
                | AttachmentOwner::Squawk { .. }
                | AttachmentOwner::Document { .. }
                | AttachmentOwner::Reference { .. },
                _,
            ) => candidate == owner,
        };
        Ok(self
            .attachments_of(asset)
            .await?
            .into_iter()
            .filter(|(_, attachment)| same(attachment.owner))
            .collect())
    }

    /// A reference document's attachments (on no vehicle), oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn attachments_of_reference(
        &self,
        reference: ReferenceDocumentId,
    ) -> Result<Vec<(AttachmentId, Attachment)>, StorageError> {
        sqlx::query(
            "SELECT * FROM attachments WHERE reference_id = ? AND deleted = 0
             ORDER BY added_ms, id",
        )
        .bind(id_to_sql(reference))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(attachment_from_row)
        .collect()
    }

    /// Every attachment that isn't removed (to find the files nothing uses).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn all_attachments(&self) -> Result<Vec<(AttachmentId, Attachment)>, StorageError> {
        sqlx::query("SELECT * FROM attachments WHERE deleted = 0 ORDER BY id")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(attachment_from_row)
            .collect()
    }

    /// Attachments removed at or after `since` (a removal with no time recorded counts as
    /// recent), whose files a snapshot from before then may still need.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn attachments_removed_since(
        &self,
        since: UnixMillis,
    ) -> Result<Vec<(AttachmentId, Attachment)>, StorageError> {
        sqlx::query(
            "SELECT attachments.* FROM attachments
             LEFT JOIN tombstones
                 ON tombstones.entity = 'attachment' AND tombstones.record_id = attachments.id
             WHERE attachments.deleted = 1
               AND (tombstones.deleted_at_ms IS NULL OR tombstones.deleted_at_ms >= ?)
             ORDER BY attachments.id",
        )
        .bind(since.as_millis())
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(attachment_from_row)
        .collect()
    }
}
