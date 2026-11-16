//! The desktop's files database (phase-3.md 6a, decision #077).
//!
//! `<folder>/files.sqlite`, beside the copy of the records ([`crate::local`]) and encrypted
//! under the same [`LocalKey`] with SQLCipher: the photos and PDFs the desktop has, each by its
//! SHA-256. Its own small schema (`migrations-files/`); the records' migrations never run here.
//! Kept apart from the records so downloading them again (#076) keeps the files; deleted with
//! the copy's folder ([`crate::local::delete_local`]).
//!
//! What each file is ([`LocalKind`]) says whether it may be dropped: only photos are, oldest
//! opened first, when they pass the limit ([`LocalFiles::drop_photos_over`]).

use std::path::Path;

use domain::attachment::{FileHash, StoredFile};
use domain::log_area;
use domain::time::UnixMillis;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqlitePool;
use sqlx::{ConnectOptions, Row};

use crate::StorageError;
use crate::convert::invalid;
use crate::database::connect_options;
use crate::files::hash_of;
use crate::local::LocalKey;

/// The files database's schema.
static FILES_MIGRATOR: Migrator = sqlx::migrate!("./migrations-files");

/// The files database's name in the copy's folder.
pub const FILES_FILE_NAME: &str = "files.sqlite";

/// What a file on the desktop is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum LocalKind {
    /// A thumbnail: downloaded by itself; never dropped.
    Thumbnail,
    /// A PDF: downloaded by itself; never dropped.
    Pdf,
    /// A photo: downloaded when opened; dropped first when past the limit.
    Photo,
    /// A photo's original: only on request; never dropped.
    Original,
    /// Added on this desktop and not yet uploaded: never dropped.
    Upload,
}

impl LocalKind {
    /// Its stored name.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Thumbnail => "thumbnail",
            Self::Pdf => "pdf",
            Self::Photo => "photo",
            Self::Original => "original",
            Self::Upload => "upload",
        }
    }

    fn parse(text: &str) -> Option<Self> {
        [
            Self::Thumbnail,
            Self::Pdf,
            Self::Photo,
            Self::Original,
            Self::Upload,
        ]
        .into_iter()
        .find(|kind| kind.as_str() == text)
    }

    /// How firmly it's kept: an upload most, a photo least. Storing a file again as another
    /// kind keeps the firmer of the two.
    const fn firmness(self) -> u8 {
        match self {
            Self::Photo => 0,
            Self::Original => 1,
            Self::Thumbnail | Self::Pdf => 2,
            Self::Upload => 3,
        }
    }
}

/// How much the files database holds.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct LocalFilesSummary {
    /// How many files.
    pub files: u64,
    /// Their total size, in bytes.
    pub bytes: u64,
    /// The photos' size (what the limit counts), in bytes.
    pub photo_bytes: u64,
    /// How many uploads wait to be sent.
    pub uploads: u64,
}

/// A photo or PDF added on this desktop and not yet sent.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PendingUpload {
    /// Its row.
    pub id: i64,
    /// What it's for.
    pub purpose: domain::api::UploadPurpose,
    /// The bytes as picked: what the server is sent.
    pub raw: Vec<u8>,
    /// What processing them here made (the files the copy's attachment names).
    pub made: domain::api::UploadedFile,
}

impl PendingUpload {
    /// Every file processing it here made (the file, and its thumbnail if any).
    #[must_use]
    pub fn made_files(&self) -> Vec<&StoredFile> {
        let mut files = vec![&self.made.file];
        files.extend(&self.made.thumbnail);
        files
    }
}

/// The desktop's files database.
#[derive(Debug, Clone)]
pub struct LocalFiles {
    pool: SqlitePool,
}

impl LocalFiles {
    /// Opens (or creates) the files database in `folder` with `key`, and brings its schema up
    /// to date.
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotEncrypted`] in a build without SQLCipher (nothing is written).
    /// - [`StorageError::WrongKey`] if `key` doesn't open it.
    /// - [`StorageError::Io`], [`StorageError::Sqlx`], [`StorageError::Migrate`] otherwise.
    pub async fn open(folder: &Path, key: &LocalKey) -> Result<Self, StorageError> {
        if !crate::local::has_sqlcipher().await? {
            return Err(StorageError::NotEncrypted);
        }
        std::fs::create_dir_all(folder)?;
        let options = connect_options(&folder.join(FILES_FILE_NAME))
            .pragma("key", key.pragma_value())
            .disable_statement_logging();
        let pool = match SqlitePool::connect_with(options).await {
            Ok(pool) => pool,
            Err(error) if crate::local::is_not_a_database(&error) => {
                return Err(StorageError::WrongKey);
            }
            Err(error) => return Err(error.into()),
        };
        if let Err(error) = FILES_MIGRATOR.run(&pool).await {
            pool.close().await;
            if let sqlx::migrate::MigrateError::Execute(inner) = &error
                && crate::local::is_not_a_database(inner)
            {
                return Err(StorageError::WrongKey);
            }
            return Err(error.into());
        }
        tracing::info!(target: log_area::DATABASE, folder = %folder.display(), "this computer's files are open");
        Ok(Self { pool })
    }

    /// Closes it (the desktop locked or signed out).
    pub async fn close(&self) {
        self.pool.close().await;
    }

    /// Stores `content` as `kind`, opened `now`; a file already here keeps the firmer kind.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails; [`StorageError::InvalidValue`] for an empty
    /// file.
    pub async fn put(
        &self,
        content: &[u8],
        kind: LocalKind,
        now: UnixMillis,
    ) -> Result<StoredFile, StorageError> {
        let bytes = i64::try_from(content.len())
            .ok()
            .filter(|bytes| *bytes > 0)
            .ok_or(StorageError::InvalidValue { column: "content" })?;
        let hash = hash_of(content);
        let existing = self.kind_of(&hash).await?;
        let kind = existing
            .filter(|existing| existing.firmness() >= kind.firmness())
            .unwrap_or(kind);
        sqlx::query(
            "INSERT INTO files (sha256, content, bytes, kind, added_ms, opened_ms)
             VALUES (?, ?, ?, ?, ?, ?)
             ON CONFLICT (sha256) DO UPDATE SET kind = excluded.kind, opened_ms = excluded.opened_ms",
        )
        .bind(hash.as_str())
        .bind(content)
        .bind(bytes)
        .bind(kind.as_str())
        .bind(now.as_millis())
        .bind(now.as_millis())
        .execute(&self.pool)
        .await?;
        Ok(StoredFile {
            hash,
            bytes: u64::try_from(bytes).unwrap_or_default(),
        })
    }

    /// The file with `hash`, if it's here.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn get(&self, hash: &FileHash) -> Result<Option<Vec<u8>>, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT content FROM files WHERE sha256 = ?")
                .bind(hash.as_str())
                .fetch_optional(&self.pool)
                .await?,
        )
    }

    /// What the file with `hash` is, if it's here.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored kind is unknown.
    pub async fn kind_of(&self, hash: &FileHash) -> Result<Option<LocalKind>, StorageError> {
        let kind: Option<String> = sqlx::query_scalar("SELECT kind FROM files WHERE sha256 = ?")
            .bind(hash.as_str())
            .fetch_optional(&self.pool)
            .await?;
        kind.map(|kind| {
            LocalKind::parse(&kind).ok_or(StorageError::InvalidValue { column: "kind" })
        })
        .transpose()
    }

    /// The size of the file with `hash`, if it's here.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored size is invalid.
    pub async fn bytes_of(&self, hash: &FileHash) -> Result<Option<u64>, StorageError> {
        let bytes: Option<i64> = sqlx::query_scalar("SELECT bytes FROM files WHERE sha256 = ?")
            .bind(hash.as_str())
            .fetch_optional(&self.pool)
            .await?;
        bytes
            .map(|bytes| u64::try_from(bytes).map_err(invalid("bytes")))
            .transpose()
    }

    /// Of `hashes`, the ones not here (to download).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails.
    pub async fn missing(&self, hashes: &[FileHash]) -> Result<Vec<FileHash>, StorageError> {
        let mut missing = Vec::new();
        for hash in hashes {
            let here: Option<i64> = sqlx::query_scalar("SELECT 1 FROM files WHERE sha256 = ?")
                .bind(hash.as_str())
                .fetch_optional(&self.pool)
                .await?;
            if here.is_none() {
                missing.push(hash.clone());
            }
        }
        Ok(missing)
    }

    /// Notes the file with `hash` was shown `now` (photos opened longest ago go first).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn mark_opened(&self, hash: &FileHash, now: UnixMillis) -> Result<(), StorageError> {
        sqlx::query("UPDATE files SET opened_ms = ? WHERE sha256 = ?")
            .bind(now.as_millis())
            .bind(hash.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Makes the file with `hash` `kind` (an upload, once sent, becomes the photo or thumbnail
    /// it is).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn set_kind(&self, hash: &FileHash, kind: LocalKind) -> Result<(), StorageError> {
        sqlx::query("UPDATE files SET kind = ? WHERE sha256 = ?")
            .bind(kind.as_str())
            .bind(hash.as_str())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// Removes the file with `hash`. Whether it was here.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn remove(&self, hash: &FileHash) -> Result<bool, StorageError> {
        Ok(sqlx::query("DELETE FROM files WHERE sha256 = ?")
            .bind(hash.as_str())
            .execute(&self.pool)
            .await?
            .rows_affected()
            > 0)
    }

    /// Keeps an upload waiting to be sent: what it's for, the bytes as picked, and what
    /// processing them here made.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a value can't be stored.
    pub async fn add_upload(
        &self,
        purpose: &domain::api::UploadPurpose,
        raw: &[u8],
        made: &domain::api::UploadedFile,
        now: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query("INSERT INTO uploads (purpose, raw, made, added_ms) VALUES (?, ?, ?, ?)")
            .bind(crate::convert::json_to_sql(purpose, "purpose")?)
            .bind(raw)
            .bind(crate::convert::json_to_sql(made, "made")?)
            .bind(now.as_millis())
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// The upload waiting whose processing made the file `hash`, if there is one.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn upload_making(
        &self,
        hash: &FileHash,
    ) -> Result<Option<PendingUpload>, StorageError> {
        // `made` is JSON naming its files' hashes (64 hex digits): found there, then checked.
        let rows = sqlx::query(
            "SELECT id, purpose, raw, made FROM uploads WHERE instr(made, ?) > 0 ORDER BY id",
        )
        .bind(hash.as_str())
        .fetch_all(&self.pool)
        .await?;
        for row in rows {
            let purpose: String = row.try_get("purpose")?;
            let made: String = row.try_get("made")?;
            let upload = PendingUpload {
                id: row.try_get("id")?,
                purpose: crate::convert::json_from_sql(&purpose, "purpose")?,
                raw: row.try_get("raw")?,
                made: crate::convert::json_from_sql(&made, "made")?,
            };
            if upload.made_files().iter().any(|file| file.hash == *hash) {
                return Ok(Some(upload));
            }
        }
        Ok(None)
    }

    /// Forgets an upload once the server has it.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn remove_upload(&self, id: i64) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM uploads WHERE id = ?")
            .bind(id)
            .execute(&self.pool)
            .await?;
        Ok(())
    }

    /// How much it holds.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn summary(&self) -> Result<LocalFilesSummary, StorageError> {
        let row = sqlx::query(
            "SELECT count(*) AS files,
                    coalesce(sum(bytes), 0) AS bytes,
                    coalesce(sum(bytes) FILTER (WHERE kind = 'photo'), 0) AS photo_bytes,
                    count(*) FILTER (WHERE kind = 'upload') AS uploads
             FROM files",
        )
        .fetch_one(&self.pool)
        .await?;
        let count = |column: &'static str| -> Result<u64, StorageError> {
            let value: i64 = row.try_get(column)?;
            u64::try_from(value).map_err(invalid(column))
        };
        Ok(LocalFilesSummary {
            files: count("files")?,
            bytes: count("bytes")?,
            photo_bytes: count("photo_bytes")?,
            uploads: count("uploads")?,
        })
    }

    /// Drops photos, opened longest ago first, until they use at most `limit` bytes. Never a
    /// thumbnail, PDF, original, or upload. How many were dropped.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails.
    pub async fn drop_photos_over(&self, limit: u64) -> Result<u64, StorageError> {
        let mut used = self.summary().await?.photo_bytes;
        if used <= limit {
            return Ok(0);
        }
        let oldest: Vec<(String, i64)> = sqlx::query_as(
            "SELECT sha256, bytes FROM files WHERE kind = 'photo' ORDER BY opened_ms, sha256",
        )
        .fetch_all(&self.pool)
        .await?;
        let mut dropped = 0;
        for (hash, bytes) in oldest {
            if used <= limit {
                break;
            }
            sqlx::query("DELETE FROM files WHERE sha256 = ?")
                .bind(&hash)
                .execute(&self.pool)
                .await?;
            used = used.saturating_sub(u64::try_from(bytes).unwrap_or_default());
            dropped += 1;
        }
        if dropped > 0 {
            tracing::info!(target: log_area::DATABASE, dropped, limit, "dropped the photos opened longest ago");
        }
        Ok(dropped)
    }
}
