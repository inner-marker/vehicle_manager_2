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
    async fn the_outbox_and_clock_are_kept_in_the_encrypted_copy() {
        use domain::hlc::Hlc;
        use domain::operation::{Operation, OperationChange};

        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let (database, _) =
            Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
                .await
                .unwrap();
        let operation = Operation::new(
            test_id(10),
            Hlc::new(now(), 0).unwrap(),
            OperationChange::DeleteEvent { id: test_id(11) },
        );
        database.add_to_outbox(&operation, now()).await.unwrap();
        database.advance_device_hlc(operation.hlc).await.unwrap();
        database.close().await;

        let (database, _) =
            Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
                .await
                .unwrap();
        assert_eq!(
            database.outbox_waiting(10).await.unwrap(),
            vec![operation.clone()]
        );
        assert_eq!(database.device_hlc().await.unwrap(), operation.hlc);
        database.close().await;
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

    #[tokio::test]
    async fn a_pulled_page_lands_in_an_encrypted_copy() {
        use domain::sync::PullPage;
        use storage::NewDatabaseIds;
        use storage::sync::{PullScope, schema_version};

        // A server with one user, as the command pipeline would leave it.
        let server_temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(1),
            server_device_id: test_id(2),
        };
        let (server_db, _) = Database::open(server_temp.path(), now(), ids)
            .await
            .unwrap();
        sqlx::query(
            "INSERT INTO users (id, username, display_name, disabled, must_change_password, sequence)
             VALUES (?, 'sam', 'Samantha Quillfeather', 0, 0, 1)",
        )
        .bind(test_id::<()>(10).to_bytes().to_vec())
        .execute(server_db.pool())
        .await
        .unwrap();
        sqlx::query("UPDATE database_meta SET last_sequence = 1")
            .execute(server_db.pool())
            .await
            .unwrap();
        let scope = PullScope {
            user: test_id(10),
            household: test_id(1),
        };
        let read = server_db.read_pull_page(&scope, 0, 500).await.unwrap();
        let page = PullPage {
            database_id: test_id(1),
            schema: schema_version(),
            cursor: 0,
            next_cursor: read.next_cursor,
            server_sequence: read.server_sequence,
            tables: read.tables,
        };

        let temp = tempfile::tempdir().unwrap();
        let folder = temp.path().join("copy");
        let (copy, _) = Database::open_local(&folder, &LocalKey::new(KEY), &server(1, ""), now())
            .await
            .unwrap();
        copy.apply_pull_page(&page, now()).await.unwrap();
        let name: String = sqlx::query_scalar("SELECT display_name FROM users")
            .fetch_one(copy.pool())
            .await
            .unwrap();
        assert_eq!(name, "Samantha Quillfeather");
        assert_eq!(copy.pull_state().await.unwrap().unwrap().cursor, 1);
        copy.close().await;
        let bytes = std::fs::read(folder.join(database_file_name())).unwrap();
        let marker = b"Samantha Quillfeather";
        assert!(
            !bytes.windows(marker.len()).any(|window| window == marker),
            "the name isn't readable on disk"
        );
    }
}
