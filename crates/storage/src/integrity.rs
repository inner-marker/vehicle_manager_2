//! SQLite's own consistency checks (plan §6 "Server self-checks").
//!
//! - [`Database::integrity_check`]: the full check plus foreign keys. Thorough but slow
//!   on a big database, so it runs at startup.
//! - [`Database::quick_check`]: skips the index cross-checks; cheap enough to run hourly.
//!
//! Either returns the problems SQLite found, or an empty list when all is well.

use sqlx::Row;

use crate::{Database, StorageError};

/// The full check, reporting at most 20 problems (one is already enough to act on).
const INTEGRITY_CHECK: &str = "PRAGMA integrity_check(20)";

/// The quick check, with the same limit.
const QUICK_CHECK: &str = "PRAGMA quick_check(20)";

impl Database {
    /// Runs `PRAGMA integrity_check` and `PRAGMA foreign_key_check`.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the checks can't run at all (e.g. the file is unreadable).
    pub async fn integrity_check(&self) -> Result<Vec<String>, StorageError> {
        let mut problems = run_check(self, INTEGRITY_CHECK).await?;
        problems.extend(foreign_key_problems(self).await?);
        Ok(problems)
    }

    /// Runs `PRAGMA quick_check`.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the check can't run at all.
    pub async fn quick_check(&self) -> Result<Vec<String>, StorageError> {
        run_check(self, QUICK_CHECK).await
    }
}

/// Runs `integrity_check` or `quick_check`; a single `ok` row means no problems.
async fn run_check(database: &Database, sql: &'static str) -> Result<Vec<String>, StorageError> {
    let rows: Vec<String> = sqlx::query_scalar(sql).fetch_all(database.pool()).await?;
    Ok(rows.into_iter().filter(|row| row != "ok").collect())
}

/// One line per row that points at a missing parent row.
async fn foreign_key_problems(database: &Database) -> Result<Vec<String>, StorageError> {
    let rows = sqlx::query("PRAGMA foreign_key_check")
        .fetch_all(database.pool())
        .await?;
    // Each row names the table and rowid with the broken reference, and the table it
    // should point at.
    rows.iter()
        .map(|row| {
            let table: String = row.try_get(0)?;
            let rowid: Option<i64> = row.try_get(1)?;
            let parent: String = row.try_get(2)?;
            Ok(format!(
                "foreign key: {table} row {} refers to a missing {parent} row",
                rowid.map_or_else(|| "?".to_owned(), |rowid| rowid.to_string())
            ))
        })
        .collect()
}
