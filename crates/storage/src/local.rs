//! The desktop's encrypted copy of a server's database (phase-3.md 4a, decision #075).
//!
//! [`Database::open_local`] opens (or creates) `<folder>/vehiclemanager.sqlite` encrypted with
//! SQLCipher under a random 32-byte [`LocalKey`] (the app keeps the key sealed with its sign-in,
//! #074). The same migrations run as on the server; tables only the server uses stay empty.
//!
//! - **Only with SQLCipher:** a build without the `sqlcipher` feature refuses
//!   ([`StorageError::NotEncrypted`]) rather than write the household's records in the clear.
//! - **The key is the raw key** (`x'…'`), so SQLCipher skips its own key derivation: the key is
//!   already random. It's never logged: statement logging is off for the copy.
//! - **No backup before migrating:** the copy can always be downloaded again.
//! - **One server per folder:** the copy records the server's database ID and instance
//!   (`pull_state`), and refuses to open as another server's ([`StorageError::OtherServer`]).

use std::path::Path;

use domain::id::DatabaseId;
use domain::log_area;
use domain::time::UnixMillis;
use sqlx::sqlite::{SqliteConnectOptions, SqlitePool};
use sqlx::{ConnectOptions, Row};

use crate::StorageError;
use crate::convert::{id_from_sql, id_to_sql};
use crate::database::{
    Database, NewDatabaseIds, applied_schema_version, connect_options, database_file_name,
    ensure_meta, latest_schema_version,
};

/// The key a desktop's copy is encrypted with: 32 random bytes. `Debug` never shows it.
#[derive(Clone, PartialEq, Eq)]
pub struct LocalKey([u8; 32]);

impl LocalKey {
    /// A key from its 32 bytes.
    #[must_use]
    pub const fn new(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// Its bytes, to seal it.
    #[must_use]
    pub const fn bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// The `PRAGMA key` value: the raw key in SQLCipher's `"x'…'"` form.
    pub(crate) fn pragma_value(&self) -> String {
        use std::fmt::Write;
        let hex = self.0.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02X}");
            text
        });
        format!("\"x'{hex}'\"")
    }
}

impl std::fmt::Debug for LocalKey {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("LocalKey([redacted])")
    }
}

/// Which server a copy is of.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyOf {
    /// The server's database ID.
    pub database_id: DatabaseId,
    /// The server's instance name (`""` for the household's main server, `"test"`, …).
    pub instance: String,
}

/// What happened while opening a copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalOpenReport {
    /// The copy didn't exist and was created (so it needs a full download).
    pub created: bool,
    /// The schema version before opening (0 for a new copy).
    pub schema_before: i64,
    /// The schema version now.
    pub schema_now: i64,
}

/// Whether this build's SQLite has SQLCipher (asked of a database in memory).
pub(crate) async fn has_sqlcipher() -> Result<bool, StorageError> {
    let options = SqliteConnectOptions::new()
        .in_memory(true)
        .disable_statement_logging();
    let mut connection = options.connect().await?;
    let version: Option<String> = sqlx::query_scalar("PRAGMA cipher_version")
        .fetch_optional(&mut connection)
        .await?;
    Ok(version.is_some_and(|version| !version.is_empty()))
}

impl Database {
    /// Opens (or creates) the encrypted copy in `folder`, of the server `copy_of`, and brings
    /// its schema up to date.
    ///
    /// # Errors
    ///
    /// - [`StorageError::NotEncrypted`] in a build without SQLCipher (nothing is written).
    /// - [`StorageError::WrongKey`] if `key` doesn't open it.
    /// - [`StorageError::OtherServer`] if it's a copy of another server.
    /// - [`StorageError::NewerSchema`] if a newer app wrote it.
    /// - [`StorageError::Io`], [`StorageError::Sqlx`], [`StorageError::Migrate`] otherwise.
    pub async fn open_local(
        folder: &Path,
        key: &LocalKey,
        copy_of: &CopyOf,
        now: UnixMillis,
    ) -> Result<(Self, LocalOpenReport), StorageError> {
        if !has_sqlcipher().await? {
            return Err(StorageError::NotEncrypted);
        }
        std::fs::create_dir_all(folder)?;
        let path = folder.join(database_file_name());
        let created = !path.exists();
        // sqlx sends `key` before anything else on every new connection.
        let options = connect_options(&path)
            .pragma("key", key.pragma_value())
            .disable_statement_logging();
        let pool = match SqlitePool::connect_with(options).await {
            Ok(pool) => pool,
            Err(error) if is_not_a_database(&error) => return Err(StorageError::WrongKey),
            Err(error) => return Err(error.into()),
        };

        let schema_before = match applied_schema_version(&pool).await {
            Ok(version) => version,
            Err(StorageError::Sqlx(error)) if is_not_a_database(&error) => {
                pool.close().await;
                return Err(StorageError::WrongKey);
            }
            Err(error) => {
                pool.close().await;
                return Err(error);
            }
        };
        let schema_latest = latest_schema_version();
        if schema_before > schema_latest {
            pool.close().await;
            return Err(StorageError::NewerSchema {
                database: schema_before,
                app: schema_latest,
            });
        }
        crate::database::MIGRATOR.run(&pool).await?;

        // The copy's own IDs are the server's: its records (the household preferences, whose ID
        // is the database ID) must read the same as there. `server_device_id` isn't used on a
        // copy, so the database ID's bytes stand in.
        let ids = NewDatabaseIds {
            database_id: copy_of.database_id,
            server_device_id: copy_of.database_id.cast(),
        };
        let (database_id, _) = ensure_meta(&pool, now, ids).await?;
        sqlx::query(
            "INSERT OR IGNORE INTO pull_state (id, server_database_id, server_instance)
             VALUES (1, ?, ?)",
        )
        .bind(id_to_sql(copy_of.database_id))
        .bind(&copy_of.instance)
        .execute(&pool)
        .await?;
        let row = sqlx::query("SELECT server_database_id, server_instance FROM pull_state")
            .fetch_one(&pool)
            .await?;
        let stored = CopyOf {
            database_id: id_from_sql(
                row.try_get::<&[u8], _>("server_database_id")?,
                "server_database_id",
            )?,
            instance: row.try_get("server_instance")?,
        };
        if stored != *copy_of || database_id != copy_of.database_id {
            pool.close().await;
            return Err(StorageError::OtherServer);
        }

        let report = LocalOpenReport {
            created,
            schema_before,
            schema_now: schema_latest,
        };
        tracing::info!(
            target: log_area::DATABASE,
            folder = %folder.display(),
            created,
            schema_before,
            schema_now = schema_latest,
            "this computer's copy is open"
        );
        Ok((Self::from_pool(pool), report))
    }
}

/// SQLite's "file is not a database" (code 26): what a wrong key looks like.
pub(crate) fn is_not_a_database(error: &sqlx::Error) -> bool {
    error
        .as_database_error()
        .and_then(sqlx::error::DatabaseError::code)
        .is_some_and(|code| code == "26")
}

/// Deletes the copy in `folder` (signing out, or too many wrong tries; #075). Nothing if it
/// isn't there.
///
/// # Errors
///
/// The I/O error if the folder can't be removed.
pub fn delete_local(folder: &Path) -> Result<(), StorageError> {
    match std::fs::remove_dir_all(folder) {
        Ok(()) => {
            tracing::info!(target: log_area::DATABASE, folder = %folder.display(), "this computer's copy is deleted");
            Ok(())
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(error.into()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_key_never_prints_and_its_pragma_is_the_raw_form() {
        let key = LocalKey::new([0xAB; 32]);
        assert_eq!(format!("{key:?}"), "LocalKey([redacted])");
        let pragma = key.pragma_value();
        assert!(pragma.starts_with("\"x'ABAB") && pragma.ends_with("'\""));
        assert_eq!(pragma.len(), 64 + 5);
    }
}
