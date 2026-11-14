//! Every saved release fixture migrates to the latest schema (plan §6 "Testing").
//!
//! From `v0.2.0` on, a copy of a database made by each release is saved in
//! `tests/fixtures/` (see `tests/fixtures/README.md`). This test copies each one to a
//! temporary folder, opens it, and checks it reaches the latest schema with its ID intact, its
//! vehicles, events, and other records still readable, and the database sound. From `v0.3.0`,
//! Phase 3's records too: the erased files' audit record.

use std::path::Path;

use domain::test_support::test_id;
use domain::time::UnixMillis;
use storage::database::{MIGRATOR, database_file_name};
use storage::{Database, NewDatabaseIds};

#[tokio::test]
async fn every_release_fixture_migrates_to_the_latest_schema() {
    let fixtures = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures");
    let latest = MIGRATOR
        .iter()
        .map(|migration| migration.version)
        .max()
        .unwrap();
    let new_ids = NewDatabaseIds {
        database_id: test_id(1),
        server_device_id: test_id(2),
    };

    for entry in std::fs::read_dir(&fixtures).unwrap() {
        let fixture = entry.unwrap().path();
        if fixture.extension().and_then(|extension| extension.to_str()) != Some("sqlite") {
            continue;
        }
        let temp = tempfile::tempdir().unwrap();
        std::fs::copy(&fixture, temp.path().join(database_file_name())).unwrap();

        let (database, report) = Database::open(temp.path(), UnixMillis::EPOCH, new_ids)
            .await
            .unwrap_or_else(|error| panic!("{} failed to open: {error}", fixture.display()));

        assert_eq!(report.schema_now, latest, "{}", fixture.display());
        assert_ne!(
            report.database_id,
            new_ids.database_id,
            "{} lost its database ID",
            fixture.display()
        );
        // Its records came through: every fixture has users, a vehicle, and events
        // (from v0.2.0, the first with a database), readable by this build.
        assert!(
            !database.assets().await.unwrap().is_empty(),
            "{}",
            fixture.display()
        );
        for (id, _) in database.assets().await.unwrap() {
            assert!(
                !database.events_of(id).await.unwrap().is_empty(),
                "{} lost its events",
                fixture.display()
            );
            database.requirements_of(id).await.unwrap();
            database.attachments_of(id).await.unwrap();
            database.documents_of(id).await.unwrap();
        }
        database.reference_documents().await.unwrap();
        erased_files_come_through(&database, &fixture).await;
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(integrity, "ok", "{}", fixture.display());
        let broken_links: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check")
            .fetch_one(database.pool())
            .await
            .unwrap();
        assert_eq!(
            broken_links,
            0,
            "{} has broken foreign keys",
            fixture.display()
        );
        // Every session kept has its device (migration 0016), readable by this build.
        let session_devices: Vec<Vec<u8>> = sqlx::query_scalar("SELECT device_id FROM sessions")
            .fetch_all(database.pool())
            .await
            .unwrap();
        for bytes in session_devices {
            let id = storage::convert::id_from_sql(&bytes, "device_id").unwrap();
            assert!(
                database.device(id).await.unwrap().is_some(),
                "{}: a session lost its device",
                fixture.display()
            );
        }
        database.close().await;
    }
}

/// The erased files' audit record (migration 0020; from `v0.3.0`) is readable: each says who
/// removed its attachment, which is no longer among the live ones.
#[cfg(test)]
async fn erased_files_come_through(database: &Database, fixture: &Path) {
    let live = database.all_attachments().await.unwrap();
    for record in database.erased_files().await.unwrap() {
        assert!(
            record.removed_by.is_some(),
            "{}: an erased file lost who removed it",
            fixture.display()
        );
        assert!(
            live.iter().all(|(id, _)| *id != record.attachment),
            "{}: an erased file's attachment came back",
            fixture.display()
        );
    }
}
