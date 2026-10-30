//! Server startup: open the database, check it, and set the server's health.
//!
//! [`start`] runs once per process (later calls, e.g. on a development hot reload, just
//! return the same database). It never stops the server: if the database can't be opened
//! or fails its check, the server still serves pages, reports itself unhealthy, and
//! refuses changes (`crate::health`).

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::log_area;
use domain::time::Clock;
use storage::{Database, NewDatabaseIds, StorageError};
use tokio::sync::OnceCell;

use crate::health::{self, Health, HealthProblem};
use crate::ids::{NewIdError, new_id};
use crate::jobs::{self, JobContext, SnapshotDir};
use crate::setup;
use domain::id::DeviceId;

static DATABASE: OnceCell<Option<Database>> = OnceCell::const_new();

/// The server's own device ID (from the database), for the changes it makes itself.
static SERVER_DEVICE: std::sync::OnceLock<DeviceId> = std::sync::OnceLock::new();

/// The server's own device ID, once [`start`] has opened the database.
#[must_use]
pub fn server_device() -> Option<DeviceId> {
    SERVER_DEVICE.get().copied()
}

/// Where the server keeps its files.
#[derive(Debug, Clone)]
pub struct ServerPaths {
    /// The data directory: the database, backups, and (by default) snapshots.
    pub data_dir: PathBuf,
    /// Where nightly snapshots go.
    pub snapshot_dir: SnapshotDir,
}

/// The environment variable naming a snapshot folder other than `<data dir>/snapshots`
/// (e.g. a USB disk mounted into the container; #035), without the app prefix.
pub const SNAPSHOT_DIR_ENV_NAME: &str = "SNAPSHOT_DIR";

/// The default snapshot folder inside the data directory.
pub const DEFAULT_SNAPSHOT_DIR_NAME: &str = "snapshots";

impl ServerPaths {
    /// The paths for `data_dir`, with the snapshot folder from `snapshot_dir_override`
    /// (the `VEHICLEMANAGER_SNAPSHOT_DIR` value, if set) or `<data dir>/snapshots`.
    #[must_use]
    pub fn new(data_dir: PathBuf, snapshot_dir_override: Option<PathBuf>) -> Self {
        let snapshot_dir = snapshot_dir_override.map_or_else(
            || SnapshotDir {
                path: data_dir.join(DEFAULT_SNAPSHOT_DIR_NAME),
                create_if_missing: true,
            },
            |path| SnapshotDir {
                path,
                create_if_missing: false,
            },
        );
        Self {
            data_dir,
            snapshot_dir,
        }
    }
}

/// Opens and checks the database the first time it's called, starts the scheduled jobs,
/// and returns the database (`None` if it couldn't be opened). `clock` is the real clock,
/// from the app. Must run inside the tokio runtime (it spawns the scheduler).
pub async fn start(
    paths: &ServerPaths,
    clock: Arc<dyn Clock + Send + Sync>,
) -> Option<&'static Database> {
    DATABASE
        .get_or_init(|| async {
            let database = open_and_check(&paths.data_dir, clock.as_ref()).await?;
            match database.user_count().await {
                Ok(count) => {
                    setup::open_if_needed(count);
                }
                Err(error) => tracing::error!(target: log_area::AUTH, %error, "can't count users; setup stays closed"),
            }
            jobs::spawn(JobContext {
                database: database.clone(),
                clock,
                snapshot_dir: paths.snapshot_dir.clone(),
            });
            tracing::info!(target: log_area::DATABASE, snapshot_dir = %paths.snapshot_dir.path.display(), "scheduled jobs started");
            Some(database)
        })
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
    let (database, report) = Database::open(data_dir, now, new_ids).await?;
    // Set once per process; a second open (tests) keeps the first.
    let _ = SERVER_DEVICE.set(report.server_device_id);
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

    #[test]
    fn snapshot_dir_env_var_comes_from_app_id() {
        // deploy/compose.yaml and .env.example repeat this name (crates/app/tests/deploy_config.rs).
        assert_eq!(
            domain::identity::env_var_name(SNAPSHOT_DIR_ENV_NAME),
            "VEHICLEMANAGER_SNAPSHOT_DIR"
        );
    }

    #[test]
    fn snapshots_default_to_the_data_dir_and_an_override_is_never_created() {
        let default = ServerPaths::new(PathBuf::from("/data"), None);
        assert_eq!(default.snapshot_dir.path, PathBuf::from("/data/snapshots"));
        assert!(default.snapshot_dir.create_if_missing);

        let usb = ServerPaths::new(
            PathBuf::from("/data"),
            Some(PathBuf::from("/usb/snapshots")),
        );
        assert_eq!(usb.snapshot_dir.path, PathBuf::from("/usb/snapshots"));
        assert!(!usb.snapshot_dir.create_if_missing);
    }

    #[tokio::test]
    async fn an_unusable_data_dir_is_an_error_not_a_panic() {
        let temp = tempfile::tempdir().unwrap();
        let blocked = temp.path().join("not-a-folder");
        std::fs::write(&blocked, "").unwrap();
        assert!(open(&blocked, &ManualClock::new(NOW)).await.is_err());
    }
}
