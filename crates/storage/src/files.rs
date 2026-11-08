//! The file store (decision #065): photos and PDFs as files in `<data dir>/files/`, each named
//! by its SHA-256 and never changed once written.
//!
//! A file `ab12…` lives at `files/ab/ab12…`, so no one folder holds them all. Writing goes to
//! `files/incoming/` first, is flushed to disk, then renamed into place, so a crash never
//! leaves half a file under a real name. The same contents written twice are stored once.
//!
//! The database lists every stored file (`stored_files`) and the attachments that use them;
//! [`Database::unused_files`] finds the ones nothing uses any more, to erase.

use std::fs::{self, File};
use std::io::{self, Read, Write};
use std::path::{Path, PathBuf};

use domain::attachment::{FileHash, StoredFile};
use domain::time::UnixMillis;
use sha2::{Digest, Sha256};

use crate::convert::invalid;
use crate::{Database, StorageError};

/// The folder, inside the data directory, that holds the files.
pub const FILES_DIR_NAME: &str = "files";

/// The folder, inside the files folder, where a file is written before it's renamed into place.
const INCOMING_DIR_NAME: &str = "incoming";

/// The files folder.
#[derive(Debug, Clone)]
pub struct FileStore {
    root: PathBuf,
}

/// What checking a stored file found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileCheck {
    /// There, with the right contents.
    Good,
    /// Not there.
    Missing,
    /// There, but its contents no longer match its name.
    Damaged,
}

/// The SHA-256 of `bytes`.
#[must_use]
pub fn hash_of(bytes: &[u8]) -> FileHash {
    FileHash::from_bytes(Sha256::digest(bytes).into())
}

impl FileStore {
    /// The store in `data_dir`, creating its folders if needed.
    ///
    /// # Errors
    ///
    /// The I/O error if a folder can't be made.
    pub fn open(data_dir: &Path) -> io::Result<Self> {
        let root = data_dir.join(FILES_DIR_NAME);
        fs::create_dir_all(root.join(INCOMING_DIR_NAME))?;
        Ok(Self { root })
    }

    /// Where the file with `hash` lives.
    #[must_use]
    pub fn path(&self, hash: &FileHash) -> PathBuf {
        let text = hash.as_str();
        self.root.join(&text[..2]).join(text)
    }

    /// Stores `bytes`, unless a file with the same contents is already there.
    ///
    /// # Errors
    ///
    /// The I/O error if it can't be written.
    pub fn put(&self, bytes: &[u8]) -> io::Result<StoredFile> {
        let hash = hash_of(bytes);
        let path = self.path(&hash);
        if !path.exists() {
            let incoming = self.root.join(INCOMING_DIR_NAME).join(hash.as_str());
            {
                let mut file = File::create(&incoming)?;
                file.write_all(bytes)?;
                file.sync_all()?;
            }
            if let Some(folder) = path.parent() {
                fs::create_dir_all(folder)?;
            }
            fs::rename(&incoming, &path)?;
        }
        Ok(StoredFile {
            hash,
            bytes: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        })
    }

    /// The contents of the file with `hash`.
    ///
    /// # Errors
    ///
    /// The I/O error, e.g. [`io::ErrorKind::NotFound`] if it isn't there.
    pub fn get(&self, hash: &FileHash) -> io::Result<Vec<u8>> {
        fs::read(self.path(hash))
    }

    /// Whether the file with `hash` is there and still matches it.
    ///
    /// # Errors
    ///
    /// The I/O error if it's there but can't be read.
    pub fn check(&self, hash: &FileHash) -> io::Result<FileCheck> {
        let mut file = match File::open(self.path(hash)) {
            Ok(file) => file,
            Err(error) if error.kind() == io::ErrorKind::NotFound => {
                return Ok(FileCheck::Missing);
            }
            Err(error) => return Err(error),
        };
        let mut hasher = Sha256::new();
        let mut buffer = vec![0; 64 * 1024];
        loop {
            let read = file.read(&mut buffer)?;
            if read == 0 {
                break;
            }
            hasher.update(&buffer[..read]);
        }
        let actual = FileHash::from_bytes(hasher.finalize().into());
        Ok(if actual == *hash {
            FileCheck::Good
        } else {
            FileCheck::Damaged
        })
    }

    /// Erases the file with `hash`, if it's there.
    ///
    /// # Errors
    ///
    /// The I/O error if it's there but can't be removed.
    pub fn erase(&self, hash: &FileHash) -> io::Result<()> {
        match fs::remove_file(self.path(hash)) {
            Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
            Ok(()) | Err(_) => Ok(()),
        }
    }
}

impl Database {
    /// Lists a file just put in the store (again listing one already there does nothing).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn record_stored_file(
        &self,
        file: &StoredFile,
        now: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT OR IGNORE INTO stored_files (sha256, bytes, stored_ms) VALUES (?, ?, ?)",
        )
        .bind(file.hash.as_str())
        .bind(i64::try_from(file.bytes).map_err(invalid("stored_files.bytes"))?)
        .bind(now.as_millis())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Whether the store lists `hash` (an attachment may only point at a listed file).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn is_stored(&self, hash: &FileHash) -> Result<bool, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM stored_files WHERE sha256 = ?)")
                .bind(hash.as_str())
                .fetch_one(self.pool())
                .await?,
        )
    }

    /// One listed file, with its size, if the store lists it.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or the stored size is invalid.
    pub async fn stored_file(&self, hash: &FileHash) -> Result<Option<StoredFile>, StorageError> {
        let bytes: Option<i64> =
            sqlx::query_scalar("SELECT bytes FROM stored_files WHERE sha256 = ?")
                .bind(hash.as_str())
                .fetch_optional(self.pool())
                .await?;
        bytes
            .map(|bytes| {
                Ok(StoredFile {
                    hash: hash.clone(),
                    bytes: u64::try_from(bytes).map_err(invalid("stored_files.bytes"))?,
                })
            })
            .transpose()
    }

    /// Every listed file, for the integrity check.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored hash is invalid.
    pub async fn stored_files(&self) -> Result<Vec<StoredFile>, StorageError> {
        let rows: Vec<(String, i64)> =
            sqlx::query_as("SELECT sha256, bytes FROM stored_files ORDER BY sha256")
                .fetch_all(self.pool())
                .await?;
        rows.into_iter()
            .map(|(hash, bytes)| {
                Ok(StoredFile {
                    hash: FileHash::parse(&hash).map_err(invalid("stored_files.sha256"))?,
                    bytes: u64::try_from(bytes).map_err(invalid("stored_files.bytes"))?,
                })
            })
            .collect()
    }

    /// The listed files nothing needs any more, as of `before`: stored before it (so a file
    /// uploaded a moment ago, whose attachment is still on its way, is left alone), and used by
    /// no attachment, except ones removed before it. The server sets `before` far enough back
    /// that every kept snapshot still finds its files after a restore.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn unused_files(&self, before: UnixMillis) -> Result<Vec<FileHash>, StorageError> {
        let mut needed = self.all_attachments().await?;
        needed.extend(self.attachments_removed_since(before).await?);
        let used: std::collections::HashSet<FileHash> = needed
            .iter()
            .flat_map(|(_, attachment)| {
                attachment
                    .files()
                    .into_iter()
                    .map(|file| file.hash.clone())
                    .collect::<Vec<_>>()
            })
            .collect();
        let old: Vec<String> = sqlx::query_scalar(
            "SELECT sha256 FROM stored_files WHERE stored_ms < ? ORDER BY sha256",
        )
        .bind(before.as_millis())
        .fetch_all(self.pool())
        .await?;
        old.into_iter()
            .map(|hash| FileHash::parse(&hash).map_err(invalid("stored_files.sha256")))
            .filter(|hash| hash.as_ref().map_or(true, |hash| !used.contains(hash)))
            .collect()
    }

    /// Stops listing a file (once it's erased from the store).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails (e.g. an attachment still points at it).
    pub async fn forget_file(&self, hash: &FileHash) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM stored_files WHERE sha256 = ?")
            .bind(hash.as_str())
            .execute(self.pool())
            .await?;
        Ok(())
    }
}
