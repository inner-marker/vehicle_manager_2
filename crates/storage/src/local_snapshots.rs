//! Snapshots of a desktop's copy (phase-3.md 7a, decision #078).
//!
//! The copy is the household's backup (plan §6), so before anything that could cost it records
//! (a big delete from the server, using a changed server, downloading it again) and once a day,
//! the desktop keeps a snapshot: `VACUUM INTO` a file in `<folder>/snapshots/`, which SQLCipher
//! writes **encrypted under the copy's own key** (the 7a spike: no plain header, and it opens
//! with that key and no other). Each is checked, and the newest [`LOCAL_SNAPSHOTS_KEPT`] kept.
//! The files database isn't snapshotted: its files are by hash, and the server has them.
//!
//! [`restore_local_snapshot`] puts one back in the copy's place (the copy closed first).

use std::path::{Path, PathBuf};

use domain::log_area;
use domain::time::UnixMillis;
use sqlx::ConnectOptions;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::database::database_file_name;
use crate::local::LocalKey;
use crate::snapshot::rotate_snapshots;
use crate::{Database, StorageError};

/// The folder, in the copy's folder, for its snapshots.
pub const LOCAL_SNAPSHOTS_DIR: &str = "snapshots";

/// How many snapshots of the copy are kept.
pub const LOCAL_SNAPSHOTS_KEPT: usize = 7;

/// The snapshots folder for the copy in `folder`.
#[must_use]
pub fn snapshots_dir(folder: &Path) -> PathBuf {
    folder.join(LOCAL_SNAPSHOTS_DIR)
}

impl Database {
    /// Snapshots this copy (in `folder`) into its snapshots folder, checks it with `key`, and
    /// keeps the newest [`LOCAL_SNAPSHOTS_KEPT`]. The snapshot's path.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if it can't be written or doesn't check out (it's removed then).
    pub async fn snapshot_local(
        &self,
        folder: &Path,
        key: &LocalKey,
        now: UnixMillis,
    ) -> Result<PathBuf, StorageError> {
        let dir = snapshots_dir(folder);
        std::fs::create_dir_all(&dir)?;
        let path = self.snapshot_into(&dir, now).await?;
        match verify_local_snapshot(&path, key).await {
            Ok(problems) if problems.is_empty() => {}
            Ok(problems) => {
                let _ = std::fs::remove_file(&path);
                tracing::error!(target: log_area::DATABASE, ?problems, "a snapshot of this computer's copy didn't check out");
                return Err(StorageError::InvalidValue { column: "snapshot" });
            }
            Err(error) => {
                let _ = std::fs::remove_file(&path);
                return Err(error);
            }
        }
        rotate_snapshots(&dir, LOCAL_SNAPSHOTS_KEPT)?;
        tracing::info!(target: log_area::DATABASE, snapshot = %path.display(), "this computer's copy was snapshotted");
        Ok(path)
    }
}

/// Opens a snapshot of a copy with `key`, read-only, and runs the integrity check. Empty means
/// healthy.
///
/// # Errors
///
/// [`StorageError::WrongKey`] if `key` doesn't open it; [`StorageError::Sqlx`] otherwise.
pub async fn verify_local_snapshot(
    path: &Path,
    key: &LocalKey,
) -> Result<Vec<String>, StorageError> {
    let options = SqliteConnectOptions::new()
        .filename(path)
        .read_only(true)
        .pragma("key", key.pragma_value())
        .disable_statement_logging();
    let pool = SqlitePool::connect_with(options).await?;
    let rows = sqlx::query_scalar::<_, String>("PRAGMA integrity_check(20)")
        .fetch_all(&pool)
        .await;
    pool.close().await;
    match rows {
        Ok(rows) => Ok(rows.into_iter().filter(|row| row != "ok").collect()),
        Err(error) if crate::local::is_not_a_database(&error) => Err(StorageError::WrongKey),
        Err(error) => Err(error.into()),
    }
}

/// The copy's snapshots in `folder`, newest first.
///
/// # Errors
///
/// [`StorageError::Io`] if the folder exists but can't be read.
pub fn local_snapshots(folder: &Path) -> Result<Vec<PathBuf>, StorageError> {
    let dir = snapshots_dir(folder);
    let mut found: Vec<PathBuf> = match std::fs::read_dir(&dir) {
        Ok(entries) => entries
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| crate::snapshot::is_snapshot_file(path))
            .collect(),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Vec::new(),
        Err(error) => return Err(error.into()),
    };
    // Names sort in time order.
    found.sort();
    found.reverse();
    Ok(found)
}

/// Puts `snapshot` back as the copy in `folder` (the copy must be closed): written beside it,
/// then renamed into place, with the old copy's journal files removed. The snapshot stays.
///
/// # Errors
///
/// [`StorageError::Io`] if it can't be copied or renamed.
pub fn restore_local_snapshot(folder: &Path, snapshot: &Path) -> Result<(), StorageError> {
    let live = folder.join(database_file_name());
    let incoming = folder.join(format!("{}.restoring", database_file_name()));
    std::fs::copy(snapshot, &incoming)?;
    for suffix in ["-wal", "-shm"] {
        let journal = folder.join(format!("{}{suffix}", database_file_name()));
        match std::fs::remove_file(&journal) {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => return Err(error.into()),
            Ok(()) | Err(_) => {}
        }
    }
    std::fs::rename(&incoming, &live)?;
    tracing::info!(target: log_area::DATABASE, snapshot = %snapshot.display(), "this computer's copy was restored from a snapshot");
    Ok(())
}
