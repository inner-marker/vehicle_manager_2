//! Opening, creating, and migrating the database, against real temporary files.

use domain::test_support::test_id;
use domain::time::UnixMillis;
use storage::database::{BACKUP_DIR_NAME, MIGRATOR, database_file_name};
use storage::{Database, NewDatabaseIds, StorageError};

const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

fn new_ids(n: u32) -> NewDatabaseIds {
    NewDatabaseIds {
        database_id: test_id(n),
        server_device_id: test_id(n + 1),
    }
}

fn latest() -> i64 {
    MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap_or_default()
}

#[tokio::test]
async fn new_database_is_created_migrated_and_given_its_ids() {
    let temp = tempfile::tempdir().unwrap();
    let data_dir = temp.path().join("data");

    let (database, report) = Database::open(&data_dir, NOW, new_ids(1)).await.unwrap();

    assert!(report.created);
    assert_eq!(report.schema_before, 0);
    assert_eq!(report.schema_now, latest());
    assert_eq!(report.backup, None, "nothing to back up in a new database");
    assert_eq!(report.database_id, new_ids(1).database_id);
    assert!(data_dir.join(database_file_name()).exists());
    assert_eq!(database_file_name(), "vehiclemanager.sqlite");
    database.close().await;
}

#[tokio::test]
async fn reopening_keeps_the_original_ids_and_makes_no_backup() {
    let temp = tempfile::tempdir().unwrap();
    let (database, _) = Database::open(temp.path(), NOW, new_ids(1)).await.unwrap();
    database.close().await;

    let (database, report) = Database::open(temp.path(), NOW, new_ids(10)).await.unwrap();

    assert!(!report.created);
    assert_eq!(report.schema_before, latest());
    assert_eq!(report.backup, None, "up to date, so nothing to migrate");
    assert_eq!(
        report.database_id,
        new_ids(1).database_id,
        "a reopened database keeps its ID"
    );
    assert_eq!(report.server_device_id, new_ids(1).server_device_id);
    database.close().await;
}

#[tokio::test]
async fn existing_database_is_backed_up_before_migrating() {
    let temp = tempfile::tempdir().unwrap();
    // An existing SQLite file with data but no migrations applied yet.
    let path = temp.path().join(database_file_name());
    let pool = sqlx::SqlitePool::connect_with(
        sqlx::sqlite::SqliteConnectOptions::new()
            .filename(&path)
            .create_if_missing(true),
    )
    .await
    .unwrap();
    sqlx::query("CREATE TABLE keep_me (value TEXT) STRICT")
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("INSERT INTO keep_me VALUES ('precious')")
        .execute(&pool)
        .await
        .unwrap();
    pool.close().await;

    let (database, report) = Database::open(temp.path(), NOW, new_ids(1)).await.unwrap();

    let backup = report
        .backup
        .expect("an existing database is backed up before migrating");
    assert_eq!(backup.parent().unwrap(), temp.path().join(BACKUP_DIR_NAME));
    assert_eq!(
        backup.file_name().unwrap().to_str().unwrap(),
        "pre-migration-20231114T221320Z-v0.sqlite"
    );
    // The backup holds the data as it was before migrating.
    let backup_pool =
        sqlx::SqlitePool::connect_with(sqlx::sqlite::SqliteConnectOptions::new().filename(&backup))
            .await
            .unwrap();
    let kept: String = sqlx::query_scalar("SELECT value FROM keep_me")
        .fetch_one(&backup_pool)
        .await
        .unwrap();
    assert_eq!(kept, "precious");
    backup_pool.close().await;
    database.close().await;
}

#[tokio::test]
async fn database_from_a_newer_app_is_refused_and_left_untouched() {
    let temp = tempfile::tempdir().unwrap();
    let (database, _) = Database::open(temp.path(), NOW, new_ids(1)).await.unwrap();
    // Pretend a newer app applied a migration this build doesn't have.
    sqlx::query(
        "INSERT INTO _sqlx_migrations (version, description, success, checksum, execution_time)
         VALUES (9999, 'from the future', 1, x'00', 0)",
    )
    .execute(database.pool())
    .await
    .unwrap();
    database.close().await;

    let result = Database::open(temp.path(), NOW, new_ids(1)).await;

    match result {
        Err(StorageError::NewerSchema { database, app }) => {
            assert_eq!(database, 9999);
            assert_eq!(app, latest());
        }
        other => panic!("expected NewerSchema, got {other:?}"),
    }
    assert!(
        !temp.path().join(BACKUP_DIR_NAME).exists(),
        "a refused database isn't backed up or migrated"
    );
}

#[tokio::test]
async fn sqlite_settings_are_applied() {
    let temp = tempfile::tempdir().unwrap();
    let (database, _) = Database::open(temp.path(), NOW, new_ids(1)).await.unwrap();

    let journal: String = sqlx::query_scalar("PRAGMA journal_mode")
        .fetch_one(database.pool())
        .await
        .unwrap();
    let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
        .fetch_one(database.pool())
        .await
        .unwrap();
    assert_eq!(journal, "wal");
    assert_eq!(foreign_keys, 1);

    // STRICT tables reject a value of the wrong type.
    let wrong_type = sqlx::query("UPDATE database_meta SET last_sequence = 'many'")
        .execute(database.pool())
        .await;
    assert!(wrong_type.is_err());
    database.close().await;
}
