//! The desktop's encrypted copy (phase-3.md 4a, decision #075).
//!
//! A plain build (the server's) must refuse to open a copy. With the `sqlcipher` feature (the
//! desktop's build; CI's "Desktop database" workflow on macOS, Windows, and Linux), a copy
//! opens with its key only, its file is unreadable without it, and it belongs to one server.

use domain::test_support::test_id;
use domain::time::UnixMillis;
use storage::{CopyOf, Database, LocalKey, StorageError};

fn now() -> UnixMillis {
    UnixMillis::from_millis(1_800_000_000_000)
}

fn server(byte: u8, instance: &str) -> CopyOf {
    CopyOf {
        database_id: test_id(u32::from(byte)),
        instance: instance.to_owned(),
    }
}

#[cfg(not(feature = "sqlcipher"))]
#[tokio::test]
async fn a_build_without_sqlcipher_refuses_and_writes_nothing() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("copy");
    let result =
        Database::open_local(&folder, &LocalKey::new([7; 32]), &server(1, ""), now()).await;
    assert!(
        matches!(result, Err(StorageError::NotEncrypted)),
        "{result:?}"
    );
    assert!(!folder.exists(), "nothing written in the clear");
}

#[tokio::test]
async fn deleting_a_copy_removes_its_folder_and_a_missing_one_is_fine() {
    let temp = tempfile::tempdir().unwrap();
    let folder = temp.path().join("copy");
    std::fs::create_dir_all(&folder).unwrap();
    std::fs::write(folder.join("vehiclemanager.sqlite"), b"x").unwrap();
    storage::local::delete_local(&folder).unwrap();
    assert!(!folder.exists());
    storage::local::delete_local(&folder).unwrap();
}

#[cfg(feature = "sqlcipher")]
mod encrypted {
    use super::*;
    use storage::database::database_file_name;

    const KEY: [u8; 32] = [7; 32];

    #[tokio::test]
    async fn a_copy_opens_with_its_key_and_its_file_is_unreadable_without_it() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let (database, report) =
            Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
                .await
                .unwrap();
        assert!(report.created);
        assert_eq!(report.schema_before, 0);
        sqlx::query("UPDATE pull_state SET cursor = 42")
            .execute(database.pool())
            .await
            .unwrap();
        database.close().await;

        let bytes = std::fs::read(folder.join(database_file_name())).unwrap();
        assert!(
            !bytes.starts_with(b"SQLite format 3"),
            "no plain SQLite header"
        );
        assert!(
            !bytes.windows(10).any(|window| window == b"pull_state"),
            "no readable table names"
        );

        let (database, report) =
            Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
                .await
                .unwrap();
        assert!(!report.created);
        assert_eq!(report.schema_before, report.schema_now);
        let cursor: i64 = sqlx::query_scalar("SELECT cursor FROM pull_state")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(cursor, 42, "the right key reads it back");
        database.close().await;

        let wrong =
            Database::open_local(&folder, &LocalKey::new([8; 32]), &server(1, ""), now()).await;
        assert!(matches!(wrong, Err(StorageError::WrongKey)), "{wrong:?}");
    }

    #[tokio::test]
    async fn a_copy_belongs_to_one_server_and_instance() {
        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let (database, _) =
            Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
                .await
                .unwrap();
        database.close().await;
        for other in [server(2, ""), server(1, "test")] {
            let result = Database::open_local(&folder, &LocalKey::new(KEY), &other, now()).await;
            assert!(
                matches!(result, Err(StorageError::OtherServer)),
                "{other:?}: {result:?}"
            );
        }
    }

    #[tokio::test]
    async fn the_copy_has_the_servers_database_id() {
        let temp = tempfile::tempdir().unwrap();
        let (database, _) = Database::open_local(
            &temp.path().join("copy"),
            &LocalKey::new(KEY),
            &server(3, ""),
            now(),
        )
        .await
        .unwrap();
        let id: Vec<u8> = sqlx::query_scalar("SELECT database_id FROM database_meta")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(id, test_id::<()>(3).to_bytes().to_vec());
    }
}
