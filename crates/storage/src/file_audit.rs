//! The audit record of erased files (phase-3.md 6a, decisions #072, #077; migration 0020).
//!
//! A removed attachment records who removed it and when
//! ([`Database::record_attachment_removal`]). When the nightly file check erases a file
//! nothing uses any more (#065), it writes one [`ErasedFile`] for each removed attachment that
//! used it ([`Database::record_erased_file`]): what the file was, what it belonged to, who
//! removed it and when, and when it was erased. No content.

use domain::attachment::{Attachment, AttachmentId, AttachmentOwner, Content, FileHash};
use domain::id::Id;
use domain::time::UnixMillis;
use domain::user::UserId;
use sqlx::Row;

use crate::attachments::{attachment_from_row, owner_to_sql};
use crate::convert::{id_from_sql, id_to_sql, invalid};
use crate::{Database, StorageError};

/// Which of an attachment's files a file is.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilePart {
    /// The file shown (the JPEG, or the PDF).
    File,
    /// Its thumbnail.
    Thumbnail,
    /// A photo's original.
    Original,
    /// A photo's raw metadata.
    Metadata,
}

impl FilePart {
    /// Its stored name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Thumbnail => "thumbnail",
            Self::Original => "original",
            Self::Metadata => "metadata",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [Self::File, Self::Thumbnail, Self::Original, Self::Metadata]
            .into_iter()
            .find(|part| part.as_str() == text)
    }

    /// Which part of `attachment` the file `hash` is, and its type, if it's one of them.
    #[must_use]
    pub fn of(attachment: &Attachment, hash: &FileHash) -> Option<(Self, Option<&'static str>)> {
        let document = matches!(attachment.content, Content::Document { .. });
        if attachment.file.hash == *hash {
            let file_type = if document {
                "application/pdf"
            } else {
                "image/jpeg"
            };
            return Some((Self::File, Some(file_type)));
        }
        if attachment
            .thumbnail
            .as_ref()
            .is_some_and(|thumbnail| thumbnail.hash == *hash)
        {
            return Some((Self::Thumbnail, Some("image/jpeg")));
        }
        if let Content::Photo {
            metadata, original, ..
        } = &attachment.content
        {
            if let Some(original) = original
                && original.file.hash == *hash
            {
                return Some((Self::Original, Some(original.file_type.mime())));
            }
            if metadata.raw.as_ref().is_some_and(|raw| raw.hash == *hash) {
                return Some((Self::Metadata, None));
            }
        }
        None
    }
}

/// A removed attachment, and who removed it when (if recorded: removals before migration 0020
/// weren't).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RemovedAttachment {
    /// Its ID.
    pub id: AttachmentId,
    /// The attachment as it was.
    pub attachment: Attachment,
    /// Who removed it.
    pub removed_by: Option<UserId>,
    /// When.
    pub removed_at: Option<UnixMillis>,
}

/// One erased file's audit record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ErasedFile {
    /// The file's SHA-256.
    pub hash: FileHash,
    /// Its size, in bytes.
    pub bytes: u64,
    /// Which of the attachment's files it was.
    pub part: FilePart,
    /// The attachment's kind: `photo` or `document`.
    pub kind: String,
    /// The file's type (e.g. `image/jpeg`), when known.
    pub file_type: Option<String>,
    /// The attachment's name.
    pub name: String,
    /// The attachment.
    pub attachment: AttachmentId,
    /// What it was on (`vehicle`, `event`, `squawk`, `document`, `reference`).
    pub owner: String,
    /// That record's ID (the vehicle's, the event's, …).
    pub owner_id: Option<Id<()>>,
    /// Who removed it.
    pub removed_by: Option<UserId>,
    /// When.
    pub removed_at: Option<UnixMillis>,
    /// When the file was erased.
    pub erased_at: UnixMillis,
}

impl ErasedFile {
    /// The record for the file `hash` of `removed`, erased at `erased_at`; `None` if
    /// `removed` doesn't use it.
    #[must_use]
    pub fn of(removed: &RemovedAttachment, hash: &FileHash, erased_at: UnixMillis) -> Option<Self> {
        let attachment = &removed.attachment;
        let (part, file_type) = FilePart::of(attachment, hash)?;
        let bytes = attachment
            .files()
            .into_iter()
            .find(|file| file.hash == *hash)
            .map_or(0, |file| file.bytes);
        let owner_id = match attachment.owner {
            AttachmentOwner::Vehicle => attachment.asset.map(Id::cast),
            AttachmentOwner::Event { event, .. } => Some(event.cast()),
            AttachmentOwner::Squawk { squawk } => Some(squawk.cast()),
            AttachmentOwner::Document { document } => Some(document.cast()),
            AttachmentOwner::Reference { reference } => Some(reference.cast()),
        };
        Some(Self {
            hash: hash.clone(),
            bytes,
            part,
            kind: match attachment.content {
                Content::Photo { .. } => "photo".to_owned(),
                Content::Document { .. } => "document".to_owned(),
            },
            file_type: file_type.map(str::to_owned),
            name: attachment.name.as_str().to_owned(),
            attachment: removed.id,
            owner: owner_to_sql(attachment.owner).owner.to_owned(),
            owner_id,
            removed_by: removed.removed_by,
            removed_at: removed.removed_at,
            erased_at,
        })
    }
}

impl Database {
    /// Records who removed attachment `id`, and when (the removal itself is a command).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn record_attachment_removal(
        &self,
        id: AttachmentId,
        by: UserId,
        at: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE attachments SET removed_by = ?, removed_ms = ? WHERE id = ?")
            .bind(id_to_sql(by))
            .bind(at.as_millis())
            .bind(id_to_sql(id))
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// The removed attachments that used the file `hash`.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn removed_attachments_using(
        &self,
        hash: &FileHash,
    ) -> Result<Vec<RemovedAttachment>, StorageError> {
        // The original's and the metadata's hashes are inside `content` (JSON); a hash is 64
        // hex digits, so finding it there is exact enough, and checked below.
        let rows = sqlx::query(
            "SELECT * FROM attachments
             WHERE deleted = 1
               AND (file_sha256 = ?1 OR thumbnail_sha256 = ?1 OR instr(content, ?1) > 0)
             ORDER BY id",
        )
        .bind(hash.as_str())
        .fetch_all(self.pool())
        .await?;
        let mut removed = Vec::new();
        for row in &rows {
            let (id, attachment) = attachment_from_row(row)?;
            if FilePart::of(&attachment, hash).is_none() {
                continue;
            }
            let by: Option<Vec<u8>> = row.try_get("removed_by")?;
            let at: Option<i64> = row.try_get("removed_ms")?;
            removed.push(RemovedAttachment {
                id,
                attachment,
                removed_by: by.map(|by| id_from_sql(&by, "removed_by")).transpose()?,
                removed_at: at.map(UnixMillis::from_millis),
            });
        }
        Ok(removed)
    }

    /// Writes an erased file's audit record.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn record_erased_file(&self, erased: &ErasedFile) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO erased_files
                 (sha256, bytes, part, kind, file_type, name, attachment_id, owner, owner_id,
                  removed_by, removed_ms, erased_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(erased.hash.as_str())
        .bind(i64::try_from(erased.bytes).unwrap_or(i64::MAX).max(1))
        .bind(erased.part.as_str())
        .bind(&erased.kind)
        .bind(erased.file_type.as_deref())
        .bind(&erased.name)
        .bind(id_to_sql(erased.attachment))
        .bind(&erased.owner)
        .bind(erased.owner_id.map(id_to_sql))
        .bind(erased.removed_by.map(id_to_sql))
        .bind(erased.removed_at.map(UnixMillis::as_millis))
        .bind(erased.erased_at.as_millis())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Every erased file's audit record, oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn erased_files(&self) -> Result<Vec<ErasedFile>, StorageError> {
        sqlx::query("SELECT * FROM erased_files ORDER BY id")
            .fetch_all(self.pool())
            .await?
            .iter()
            .map(|row| {
                let blob = |column: &'static str| -> Result<Option<Vec<u8>>, StorageError> {
                    Ok(row.try_get(column)?)
                };
                let hash: String = row.try_get("sha256")?;
                let part: String = row.try_get("part")?;
                let bytes: i64 = row.try_get("bytes")?;
                let attachment: Vec<u8> = row.try_get("attachment_id")?;
                let removed_ms: Option<i64> = row.try_get("removed_ms")?;
                Ok(ErasedFile {
                    hash: FileHash::parse(&hash).map_err(invalid("sha256"))?,
                    bytes: u64::try_from(bytes).map_err(invalid("bytes"))?,
                    part: FilePart::parse(&part)
                        .ok_or(StorageError::InvalidValue { column: "part" })?,
                    kind: row.try_get("kind")?,
                    file_type: row.try_get("file_type")?,
                    name: row.try_get("name")?,
                    attachment: id_from_sql(&attachment, "attachment_id")?,
                    owner: row.try_get("owner")?,
                    owner_id: blob("owner_id")?
                        .map(|id| id_from_sql(&id, "owner_id"))
                        .transpose()?,
                    removed_by: blob("removed_by")?
                        .map(|id| id_from_sql(&id, "removed_by"))
                        .transpose()?,
                    removed_at: removed_ms.map(UnixMillis::from_millis),
                    erased_at: UnixMillis::from_millis(row.try_get("erased_ms")?),
                })
            })
            .collect()
    }
}
