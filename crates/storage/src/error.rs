//! The storage error type.

use core::fmt;

/// Why a storage operation failed.
#[derive(Debug)]
pub enum StorageError {
    /// The database was written by a newer version of the app, so this one won't touch it.
    NewerSchema {
        /// The database's schema version.
        database: i64,
        /// The newest schema this build knows.
        app: i64,
    },
    /// A stored value can't be read as its domain type (e.g. a malformed ID).
    InvalidValue {
        /// Which column held it.
        column: &'static str,
    },
    /// SQLite reported an error.
    Sqlx(sqlx::Error),
    /// A migration failed (it rolled back).
    Migrate(sqlx::migrate::MigrateError),
    /// A file or folder couldn't be created or written.
    Io(std::io::Error),
}

impl fmt::Display for StorageError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NewerSchema { database, app } => write!(
                formatter,
                "the database has schema version {database}, newer than this app's {app}; update the app"
            ),
            Self::InvalidValue { column } => write!(formatter, "invalid value in column {column}"),
            Self::Sqlx(error) => write!(formatter, "database error: {error}"),
            Self::Migrate(error) => write!(formatter, "migration failed: {error}"),
            Self::Io(error) => write!(formatter, "file error: {error}"),
        }
    }
}

impl std::error::Error for StorageError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Sqlx(error) => Some(error),
            Self::Migrate(error) => Some(error),
            Self::Io(error) => Some(error),
            Self::NewerSchema { .. } | Self::InvalidValue { .. } => None,
        }
    }
}

impl From<sqlx::Error> for StorageError {
    fn from(error: sqlx::Error) -> Self {
        Self::Sqlx(error)
    }
}

impl From<sqlx::migrate::MigrateError> for StorageError {
    fn from(error: sqlx::migrate::MigrateError) -> Self {
        Self::Migrate(error)
    }
}

impl From<std::io::Error> for StorageError {
    fn from(error: std::io::Error) -> Self {
        Self::Io(error)
    }
}
