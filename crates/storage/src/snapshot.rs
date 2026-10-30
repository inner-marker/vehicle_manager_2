//! Nightly snapshots: a consistent copy of the live database (plan §6, decision #035).
//!
//! `VACUUM INTO` writes a compacted copy while the server keeps running; the live file is
//! never copied directly. Each snapshot is then opened on its own and integrity-checked,
//! and only the newest few are kept.

use std::path::{Path, PathBuf};

use domain::time::UnixMillis;
use domain::time_utils::format_utc_file_stamp;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};

use crate::{Database, StorageError};

/// Snapshot file names start with this, then the UTC time: `snapshot-20231114T221320Z.sqlite`.
const SNAPSHOT_PREFIX: &str = "snapshot-";

/// The file name for a snapshot taken at `now`. Names sort in time order.
#[must_use]
pub fn snapshot_file_name(now: UnixMillis) -> String {
    format!("{SNAPSHOT_PREFIX}{}.sqlite", format_utc_file_stamp(now))
}

impl Database {
    /// Writes a snapshot to `dir` (which must exist) and returns its path.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if SQLite can't write it (e.g. the folder isn't writable or
    /// the disk is full). A partly written file is removed.
    pub async fn snapshot_into(
        &self,
        dir: &Path,
        now: UnixMillis,
    ) -> Result<PathBuf, StorageError> {
        let path = dir.join(snapshot_file_name(now));
        let result = sqlx::query("VACUUM INTO ?")
            .bind(path.display().to_string())
            .execute(self.pool())
            .await;
        if let Err(error) = result {
            // Never leave a half-written snapshot that looks like a good one.
            let _ = std::fs::remove_file(&path);
            return Err(error.into());
        }
        Ok(path)
    }
}

/// Opens a snapshot read-only and runs the integrity check on it. Empty means healthy.
///
/// # Errors
///
/// [`StorageError::Sqlx`] if the file can't be opened or checked.
pub async fn verify_snapshot(path: &Path) -> Result<Vec<String>, StorageError> {
    let pool = SqlitePool::connect_with(SqliteConnectOptions::new().filename(path).read_only(true))
        .await?;
    let rows: Vec<String> = sqlx::query_scalar("PRAGMA integrity_check(20)")
        .fetch_all(&pool)
        .await?;
    pool.close().await;
    Ok(rows.into_iter().filter(|row| row != "ok").collect())
}

/// Deletes the oldest snapshots in `dir` beyond the newest `keep`, and returns the ones
/// removed. Only files this module named are touched.
///
/// # Errors
///
/// [`StorageError::Io`] if the folder can't be read or a file can't be removed.
pub fn rotate_snapshots(dir: &Path, keep: usize) -> Result<Vec<PathBuf>, StorageError> {
    let mut snapshots: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| is_snapshot_file(path))
        .collect();
    // Names sort in time order, so the oldest come first.
    snapshots.sort();
    let excess = snapshots.len().saturating_sub(keep);
    let removed: Vec<PathBuf> = snapshots.into_iter().take(excess).collect();
    for path in &removed {
        std::fs::remove_file(path)?;
    }
    Ok(removed)
}

fn is_snapshot_file(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.starts_with(SNAPSHOT_PREFIX) && name.ends_with(".sqlite"))
}
