//! Server startup: open the database, check it, and set the server's health.
//!
//! [`start`] runs once per process (later calls, e.g. on a development hot reload, just
//! return the same database). It never stops the server: if the database can't be opened
//! or fails its check, the server still serves pages, reports itself unhealthy, and
//! refuses changes (`crate::health`).

use std::path::Path;

use domain::log_area;
use domain::time::Clock;
use storage::{Database, NewDatabaseIds, StorageError};
use tokio::sync::OnceCell;

use crate::health::{self, Health, HealthProblem};
use crate::ids::{NewIdError, new_id};

static DATABASE: OnceCell<Option<Database>> = OnceCell::const_new();

/// Opens and checks the database in `data_dir` the first time it's called, and returns
/// it (`None` if it couldn't be opened). `clock` is the real clock, from the app.
pub async fn start(data_dir: &Path, clock: &(dyn Clock + Sync)) -> Option<&'static Database> {
    DATABASE
        .get_or_init(|| open_and_check(data_dir, clock))
        .await
        .as_ref()
}

/// The database, once [`start`] has opened it.
#[must_use]
pub fn database() -> Option<&'static Database> {
    DATABASE.get().and_then(Option::as_ref)
}

async fn open_and_check(data_dir: &Path, clock: &(dyn Clock + Sync)) -> Option<Database> {
    let database = match open(data_dir, clock).await {
        Ok(database) => database,
        Err(error) => {
            tracing::error!(target: log_area::DATABASE, data_dir = %data_dir.display(), %error, "can't open the database; refusing all changes");
            health::set(Health::Unhealthy(HealthProblem::DatabaseUnavailable));
            return None;
        }
    };
    health::set(check(&database).await);
    Some(database)
}

async fn open(data_dir: &Path, clock: &(dyn Clock + Sync)) -> Result<Database, StartupError> {
    let now = clock.now();
    // Used only if the database is new; an existing one keeps its IDs.
    let new_ids = NewDatabaseIds {
        database_id: new_id(now)?,
        server_device_id: new_id(now)?,
    };
    let (database, _report) = Database::open(data_dir, now, new_ids).await?;
    Ok(database)
}

/// The startup integrity check: healthy only if SQLite finds nothing wrong.
pub async fn check(database: &Database) -> Health {
    match database.integrity_check().await {
        Ok(problems) if problems.is_empty() => {
            tracing::info!(target: log_area::DATABASE, "database integrity check passed");
            Health::Healthy
        }
        Ok(problems) => {
            for problem in &problems {
                tracing::error!(target: log_area::DATABASE, problem, "database integrity problem");
            }
            Health::Unhealthy(HealthProblem::IntegrityCheckFailed)
        }
        Err(error) => {
            tracing::error!(target: log_area::DATABASE, %error, "database integrity check couldn't run");
            Health::Unhealthy(HealthProblem::IntegrityCheckFailed)
        }
    }
}

/// Why startup couldn't open the database.
#[derive(Debug)]
enum StartupError {
    NewId(NewIdError),
    Storage(StorageError),
}

impl core::fmt::Display for StartupError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::NewId(error) => error.fmt(formatter),
            Self::Storage(error) => error.fmt(formatter),
        }
    }
}

impl From<NewIdError> for StartupError {
    fn from(error: NewIdError) -> Self {
        Self::NewId(error)
    }
}

impl From<StorageError> for StartupError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

#[cfg(test)]
mod tests {
    use domain::test_support::ManualClock;
    use domain::time::UnixMillis;

    use super::*;

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    #[tokio::test]
    async fn a_new_database_opens_and_is_healthy() {
        let temp = tempfile::tempdir().unwrap();
        let database = open(temp.path(), &ManualClock::new(NOW)).await.unwrap();
        assert_eq!(check(&database).await, Health::Healthy);
        database.close().await;
    }

    #[tokio::test]
    async fn an_unusable_data_dir_is_an_error_not_a_panic() {
        let temp = tempfile::tempdir().unwrap();
        let blocked = temp.path().join("not-a-folder");
        std::fs::write(&blocked, "").unwrap();
        assert!(open(&blocked, &ManualClock::new(NOW)).await.is_err());
    }
}
