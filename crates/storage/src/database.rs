//! Opening the SQLite database: create or upgrade it safely, then hand out a pool.
//!
//! On every open (plan §6 "Schema & versioning", "Master database: SQLite"):
//! 1. Open or create `<data dir>/vehiclemanager.sqlite` with WAL, a busy timeout, and
//!    foreign keys on.
//! 2. **Refuse a newer schema** than this build knows: never risk corrupting data
//!    written by a newer version.
//! 3. If an existing database needs migrations, **back it up first** (`VACUUM INTO`
//!    `<data dir>/backups/`).
//! 4. Apply the missing migrations in order, each in its own transaction.
//! 5. On a new database, record its ID and the server's device ID.

use std::path::{Path, PathBuf};
use std::time::Duration;

use domain::id::{DatabaseId, DeviceId};
use domain::identity::APP_ID;
use domain::log_area;
use domain::time::UnixMillis;
use domain::time_utils::format_utc_file_stamp;
use sqlx::Row;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePool, SqliteSynchronous};

use crate::StorageError;
use crate::convert::{id_from_sql, id_to_sql};

/// Every migration, embedded in the binary. Released migrations are never edited.
pub static MIGRATOR: Migrator = sqlx::migrate!("./migrations");

/// The folder, inside the data directory, for backups made before migrating.
pub const BACKUP_DIR_NAME: &str = "backups";

/// How long a write waits for another to finish before failing: 5 seconds.
const BUSY_TIMEOUT: Duration = Duration::from_secs(5);

/// The database file name, from the app ID (never the display name): `vehiclemanager.sqlite`.
#[must_use]
pub fn database_file_name() -> String {
    format!("{APP_ID}.sqlite")
}

/// The IDs a brand-new database is given. Made by the caller, since `storage` doesn't
/// read the clock or a random source; ignored when the database already exists.
#[derive(Debug, Clone, Copy)]
pub struct NewDatabaseIds {
    /// The new database's ID.
    pub database_id: DatabaseId,
    /// The server's own device ID, for the changes it makes itself.
    pub server_device_id: DeviceId,
}

/// What happened while opening.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OpenReport {
    /// The database didn't exist and was created.
    pub created: bool,
    /// The schema version before opening (0 for a new or empty database).
    pub schema_before: i64,
    /// The schema version now.
    pub schema_now: i64,
    /// The backup made before migrating, if any.
    pub backup: Option<PathBuf>,
    /// This database's ID.
    pub database_id: DatabaseId,
    /// The server's device ID.
    pub server_device_id: DeviceId,
}

/// An open database.
#[derive(Debug, Clone)]
pub struct Database {
    pool: SqlitePool,
}

impl Database {
    /// Opens (or creates) the database in `data_dir` and brings its schema up to date.
    /// `now` names the backup file; `new_ids` is used only for a new database.
    ///
    /// # Errors
    ///
    /// - [`StorageError::NewerSchema`] if the database was written by a newer version.
    /// - [`StorageError::Io`] if the data or backup folder can't be created.
    /// - [`StorageError::Sqlx`] / [`StorageError::Migrate`] if SQLite or a migration fails;
    ///   a failed migration rolls back and leaves the database as it was.
    pub async fn open(
        data_dir: &Path,
        now: UnixMillis,
        new_ids: NewDatabaseIds,
    ) -> Result<(Self, OpenReport), StorageError> {
        std::fs::create_dir_all(data_dir)?;
        let path = data_dir.join(database_file_name());
        let created = !path.exists();
        let pool = SqlitePool::connect_with(connect_options(&path)).await?;

        let schema_before = applied_schema_version(&pool).await?;
        let schema_latest = latest_schema_version();
        if schema_before > schema_latest {
            pool.close().await;
            return Err(StorageError::NewerSchema {
                database: schema_before,
                app: schema_latest,
            });
        }

        let backup = if !created && schema_before < schema_latest {
            Some(back_up_before_migrating(&pool, data_dir, now, schema_before).await?)
        } else {
            None
        };

        MIGRATOR.run(&pool).await?;
        let (database_id, server_device_id) = ensure_meta(&pool, now, new_ids).await?;

        let report = OpenReport {
            created,
            schema_before,
            schema_now: latest_schema_version(),
            backup,
            database_id,
            server_device_id,
        };
        tracing::info!(
            target: log_area::DATABASE,
            path = %path.display(),
            created,
            schema_before,
            schema_now = report.schema_now,
            %database_id,
            "database open"
        );
        Ok((Self { pool }, report))
    }

    /// The connection pool, for queries in this crate and the server.
    #[must_use]
    pub const fn pool(&self) -> &SqlitePool {
        &self.pool
    }

    /// Closes every connection (e.g. before shutdown or in tests).
    pub async fn close(&self) {
        self.pool.close().await;
    }
}

/// WAL (readers never block the writer), a busy timeout, foreign keys, and `NORMAL`
/// syncing, which is safe in WAL mode and much faster than `FULL`.
fn connect_options(path: &Path) -> SqliteConnectOptions {
    // Set from the path, not a `sqlite://` URL, which would break on spaces in paths.
    SqliteConnectOptions::new()
        .filename(path)
        .create_if_missing(true)
        .journal_mode(SqliteJournalMode::Wal)
        .synchronous(SqliteSynchronous::Normal)
        .busy_timeout(BUSY_TIMEOUT)
        .foreign_keys(true)
}

/// The newest migration this build contains.
fn latest_schema_version() -> i64 {
    MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or(0)
}

/// The newest migration applied to the database, or 0 if it has none yet.
async fn applied_schema_version(pool: &SqlitePool) -> Result<i64, StorageError> {
    let has_table: bool = sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = '_sqlx_migrations')",
    )
    .fetch_one(pool)
    .await?;
    if !has_table {
        return Ok(0);
    }
    let version: Option<i64> =
        sqlx::query_scalar("SELECT MAX(version) FROM _sqlx_migrations WHERE success = 1")
            .fetch_one(pool)
            .await?;
    Ok(version.unwrap_or(0))
}

/// Copies the database to `backups/pre-migration-<UTC time>-v<schema>.sqlite`.
async fn back_up_before_migrating(
    pool: &SqlitePool,
    data_dir: &Path,
    now: UnixMillis,
    schema_before: i64,
) -> Result<PathBuf, StorageError> {
    let backup_dir = data_dir.join(BACKUP_DIR_NAME);
    std::fs::create_dir_all(&backup_dir)?;
    let backup = backup_dir.join(format!(
        "pre-migration-{}-v{schema_before}.sqlite",
        format_utc_file_stamp(now)
    ));
    sqlx::query("VACUUM INTO ?")
        .bind(backup.display().to_string())
        .execute(pool)
        .await?;
    tracing::info!(target: log_area::DATABASE, backup = %backup.display(), schema_before, "backed up before migrating");
    Ok(backup)
}

/// Records the database's IDs on first open, and returns the stored ones.
async fn ensure_meta(
    pool: &SqlitePool,
    now: UnixMillis,
    new_ids: NewDatabaseIds,
) -> Result<(DatabaseId, DeviceId), StorageError> {
    // Does nothing if the row exists, so an existing database keeps its IDs.
    sqlx::query(
        "INSERT OR IGNORE INTO database_meta (id, database_id, server_device_id, created_at_ms)
         VALUES (1, ?, ?, ?)",
    )
    .bind(id_to_sql(new_ids.database_id))
    .bind(id_to_sql(new_ids.server_device_id))
    .bind(now.as_millis())
    .execute(pool)
    .await?;

    let row = sqlx::query("SELECT database_id, server_device_id FROM database_meta WHERE id = 1")
        .fetch_one(pool)
        .await?;
    let database_id = id_from_sql(row.try_get::<&[u8], _>("database_id")?, "database_id")?;
    let server_device_id = id_from_sql(
        row.try_get::<&[u8], _>("server_device_id")?,
        "server_device_id",
    )?;
    Ok((database_id, server_device_id))
}
