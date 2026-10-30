//! Every saved release fixture migrates to the latest schema (plan §6 "Testing").
//!
//! From `v0.2.0` on, a copy of a database made by each release is saved in
//! `tests/fixtures/` (see `tests/fixtures/README.md`). This test copies each one to a
//! temporary folder, opens it, and checks it reaches the latest schema with its ID intact.

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
        database.close().await;
    }
}
