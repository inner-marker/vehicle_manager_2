//! The file store, attachments, official documents, and license plates against a real database
//! (milestone 8).

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod attachments {
    use std::str::FromStr;

    use domain::asset::{
        Asset, AssetKind, AssetName, AssetPatch, Identifier, IdentifierKind, LifecycleStatus,
    };
    use domain::attachment::{
        Attachment, AttachmentOwner, AttachmentPatch, Content, FileName, PhotoMetadata, PhotoSize,
    };
    use domain::command::Clearable;
    use domain::dates::CalendarDate;
    use domain::document::{AssetDocument, AssetDocumentPatch, DocumentType, REGISTRATION};
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::hlc::{Hlc, Stamp};
    use domain::label::Label;
    use domain::notes::Notes;
    use domain::plate::{Authority, LicensePlate, LicensePlatePatch, PlateNumber};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{Connection, Row, SqliteConnection};
    use storage::assets::{AssetWriter, GroupWriter};
    use storage::attachments::AttachmentWriter;
    use storage::commands::{RecordWriter, insert_tombstone};
    use storage::convert::id_from_sql;
    use storage::database::MIGRATOR;
    use storage::documents::{AssetDocumentWriter, LicensePlateWriter};
    use storage::files::{FileCheck, FileStore, hash_of};
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const STUART: u32 = 1;
    const GARAGE: u32 = 50;
    const TRUCK: u32 = 10;
    const EVENT: u32 = 30;
    const PLATE: u32 = 70;
    const REGISTERED: u32 = 80;
    const PHOTO: u32 = 90;

    fn date(text: &str) -> CalendarDate {
        CalendarDate::parse(text).unwrap()
    }

    fn truck(identifiers: Vec<Identifier>) -> Asset {
        Asset {
            kind: AssetKind::parse("truck").unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: None,
            model: None,
            year: None,
            colour: None,
            description: Notes::default(),
            group: test_id(GARAGE),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers,
        }
    }

    /// A person, a group, and a truck, written on `connection`.
    async fn add_household(connection: &mut SqliteConnection, identifiers: Vec<Identifier>) {
        let user = User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: false,
        };
        UserWriter
            .create(connection, test_id(STUART), &UserPatch::all(user), 1)
            .await
            .unwrap();
        let group = Group {
            name: GroupName::parse("Garage").unwrap(),
            icon: GroupIcon::Garage,
        };
        GroupWriter
            .create(connection, test_id(GARAGE), &GroupPatch::all(group), 2)
            .await
            .unwrap();
        AssetWriter
            .create(
                connection,
                test_id(TRUCK),
                &AssetPatch::all(truck(identifiers)),
                3,
            )
            .await
            .unwrap();
    }

    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        add_household(&mut connection, vec![]).await;
        drop(connection);
        (temp, database)
    }

    fn photo(store: &FileStore, owner: AttachmentOwner) -> Attachment {
        Attachment {
            asset: Some(test_id(TRUCK)),
            owner,
            file: store.put(b"\xFF\xD8\xFF a photo").unwrap(),
            thumbnail: Some(store.put(b"\xFF\xD8\xFF its thumbnail").unwrap()),
            name: FileName::from_upload("IMG_0412.jpg").unwrap(),
            content: Content::Photo {
                size: PhotoSize::Small,
                width: 1280,
                height: 960,
                metadata: PhotoMetadata {
                    camera: Some("Apple iPhone 15 Pro".into()),
                    ..PhotoMetadata::default()
                },
                original: None,
            },
            added_at: NOW,
            added_by: test_id(STUART),
            received_at: NOW,
        }
    }

    fn nc_plate() -> LicensePlate {
        LicensePlate {
            asset: test_id(TRUCK),
            authority: Some(Authority::region("US", "NC").unwrap()),
            number: PlateNumber::parse("NCX 4471").unwrap(),
            issued_on: Some(date("2024-06-01")),
            expires_on: None,
            retired_on: None,
            notes: Notes::default(),
            added_at: NOW,
            added_by: test_id(STUART),
            received_at: NOW,
        }
    }

    fn registration() -> AssetDocument {
        AssetDocument {
            asset: test_id(TRUCK),
            doc_type: DocumentType::listed(REGISTRATION).unwrap(),
            number: Some("R-55120".into()),
            issuer: Some("NC DMV".into()),
            issued_on: Some(date("2025-11-01")),
            effective_on: None,
            expires_on: Some(date("2026-10-31")),
            supersedes: None,
            plate: Some(test_id(PLATE)),
            notes: Notes::default(),
            added_at: NOW,
            added_by: test_id(STUART),
            received_at: NOW,
        }
    }

    #[test]
    fn the_store_keeps_each_file_once_and_finds_damage() {
        let temp = tempfile::tempdir().unwrap();
        let store = FileStore::open(temp.path()).unwrap();
        let first = store.put(b"receipt").unwrap();
        let again = store.put(b"receipt").unwrap();
        assert_eq!(first, again);
        assert_eq!(first.hash, hash_of(b"receipt"));
        assert_eq!(first.bytes, 7);
        let path = store.path(&first.hash);
        assert!(path.starts_with(temp.path().join("files").join(&first.hash.as_str()[..2])));
        assert_eq!(store.get(&first.hash).unwrap(), b"receipt");
        assert_eq!(store.check(&first.hash).unwrap(), FileCheck::Good);

        std::fs::write(&path, b"receipT").unwrap();
        assert_eq!(store.check(&first.hash).unwrap(), FileCheck::Damaged);
        store.erase(&first.hash).unwrap();
        assert_eq!(store.check(&first.hash).unwrap(), FileCheck::Missing);
        // Erasing what isn't there is fine.
        store.erase(&first.hash).unwrap();
        // Nothing is left half-written in the incoming folder.
        let incoming = std::fs::read_dir(temp.path().join("files/incoming")).unwrap();
        assert_eq!(incoming.count(), 0);
    }

    /// A photo on the truck, its files stored and listed.
    async fn stored_photo(
        database: &Database,
        store: &FileStore,
        connection: &mut SqliteConnection,
    ) -> Attachment {
        let attachment = photo(store, AttachmentOwner::Vehicle);
        for file in attachment.files() {
            database.record_stored_file(file, NOW).await.unwrap();
            assert!(database.is_stored(&file.hash).await.unwrap());
        }
        AttachmentWriter
            .create(
                connection,
                test_id(PHOTO),
                &AttachmentPatch::all(attachment.clone()),
                10,
            )
            .await
            .unwrap();
        attachment
    }

    #[tokio::test]
    async fn attachments_are_stored_and_found() {
        let (temp, database) = database().await;
        let store = FileStore::open(temp.path()).unwrap();
        let on_task = AttachmentOwner::Event {
            event: test_id(EVENT),
            task: Some(1),
        };
        let mut connection = database.pool().acquire().await.unwrap();
        let attachment = stored_photo(&database, &store, &mut connection).await;
        assert_eq!(
            database.attachment(test_id(PHOTO)).await.unwrap(),
            Some(attachment.clone())
        );
        assert_eq!(
            database
                .attachments_on(test_id(TRUCK), AttachmentOwner::Vehicle)
                .await
                .unwrap()
                .len(),
            1
        );
        assert!(
            database
                .attachments_on(test_id(TRUCK), on_task)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_removed_attachments_files_wait_for_the_snapshots() {
        let (temp, database) = database().await;
        let store = FileStore::open(temp.path()).unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        let attachment = stored_photo(&database, &store, &mut connection).await;
        // Files in use stay. Once it's removed, its files stay while a snapshot from before
        // the removal may need them, then they're unused.
        let removed_at = UnixMillis::from_millis(NOW.as_millis() + 10);
        let later = UnixMillis::from_millis(NOW.as_millis() + 11);
        assert!(database.unused_files(later).await.unwrap().is_empty());
        AttachmentWriter
            .delete(&mut connection, test_id(PHOTO), 11)
            .await
            .unwrap();
        assert_eq!(database.attachment(test_id(PHOTO)).await.unwrap(), None);
        assert!(
            database.unused_files(later).await.unwrap().is_empty(),
            "a removal with no time recorded counts as recent"
        );
        let stamp = Stamp {
            hlc: Hlc::new(removed_at, 0).unwrap(),
            device: test_id(901),
        };
        insert_tombstone(
            &mut connection,
            "attachment",
            test_id::<Attachment>(PHOTO).to_bytes(),
            stamp,
            11,
            removed_at,
        )
        .await
        .unwrap();
        assert!(
            database.unused_files(removed_at).await.unwrap().is_empty(),
            "removed too recently"
        );
        let mut unused = database.unused_files(later).await.unwrap();
        unused.sort();
        let mut expected: Vec<_> = attachment
            .files()
            .into_iter()
            .map(|f| f.hash.clone())
            .collect();
        expected.sort();
        assert_eq!(unused, expected);
        for hash in &unused {
            store.erase(hash).unwrap();
            database.forget_file(hash).await.unwrap();
        }
        assert!(database.stored_files().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn an_attachment_moves_between_tasks_of_an_event() {
        let (temp, database) = database().await;
        let store = FileStore::open(temp.path()).unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        // Foreign keys off for this connection, so the attachment needs no real event.
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await
            .unwrap();
        let on = |task| AttachmentOwner::Event {
            event: test_id(EVENT),
            task,
        };
        AttachmentWriter
            .create(
                &mut connection,
                test_id(PHOTO),
                &AttachmentPatch::all(photo(&store, on(Some(2)))),
                10,
            )
            .await
            .unwrap();
        let moved = AttachmentPatch {
            owner: Some(on(Some(1))),
            ..AttachmentPatch::default()
        };
        AttachmentWriter
            .update(&mut connection, test_id(PHOTO), &moved, 11)
            .await
            .unwrap();
        let found = database
            .attachments_on(test_id(TRUCK), on(None))
            .await
            .unwrap();
        assert_eq!(found.len(), 1, "an event's attachments include its tasks'");
        assert_eq!(found[0].1.owner, on(Some(1)));
    }

    #[tokio::test]
    async fn plates_and_documents_round_trip_and_change() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let plate = nc_plate();
        LicensePlateWriter
            .create(
                &mut connection,
                test_id(PLATE),
                &LicensePlatePatch::all(plate.clone()),
                10,
            )
            .await
            .unwrap();
        assert_eq!(
            database.plate(test_id(PLATE)).await.unwrap(),
            Some(plate.clone())
        );

        let registration = registration();
        AssetDocumentWriter
            .create(
                &mut connection,
                test_id(REGISTERED),
                &AssetDocumentPatch::all(registration.clone()),
                11,
            )
            .await
            .unwrap();
        assert_eq!(
            database.documents_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(REGISTERED), registration.clone())]
        );

        // The registration's expiry follows to the plate; then the plate retires.
        let follows = LicensePlatePatch {
            expires_on: Some(Clearable::Set(date("2026-10-31"))),
            retired_on: Some(Clearable::Set(date("2026-11-04"))),
            ..LicensePlatePatch::default()
        };
        LicensePlateWriter
            .update(&mut connection, test_id(PLATE), &follows, 12)
            .await
            .unwrap();
        let changed = database.plate(test_id(PLATE)).await.unwrap().unwrap();
        assert_eq!(changed.expires_on, Some(date("2026-10-31")));
        assert_eq!(changed.retired_on, Some(date("2026-11-04")));
        assert_eq!(changed.authority, plate.authority);

        // A document becomes a household-named one, loses its number, and keeps the rest.
        let renamed = AssetDocumentPatch {
            doc_type: Some(DocumentType::Custom {
                name: Label::parse("Parking permit").unwrap(),
            }),
            number: Some(Clearable::Clear),
            plate: Some(Clearable::Clear),
            ..AssetDocumentPatch::default()
        };
        AssetDocumentWriter
            .update(&mut connection, test_id(REGISTERED), &renamed, 13)
            .await
            .unwrap();
        let document = database
            .document(test_id(REGISTERED))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(document.number, None);
        assert_eq!(document.plate, None);
        assert_eq!(document.issuer, registration.issuer);
        assert!(matches!(document.doc_type, DocumentType::Custom { .. }));

        AssetDocumentWriter
            .delete(&mut connection, test_id(REGISTERED), 14)
            .await
            .unwrap();
        assert!(
            database
                .documents_of(test_id(TRUCK))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn plate_identifiers_become_plate_records() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(true);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        for migration in MIGRATOR.iter().filter(|migration| migration.version < 14) {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&mut connection)
                .await
                .unwrap();
        }
        sqlx::query(
            "INSERT INTO database_meta (id, database_id, server_device_id, created_at_ms,
                                        last_sequence)
             VALUES (1, ?, ?, 0, 5)",
        )
        .bind(vec![1_u8; 16])
        .bind(vec![2_u8; 16])
        .execute(&mut connection)
        .await
        .unwrap();
        let kind = |text: &str| IdentifierKind::parse(text).unwrap();
        add_household(
            &mut connection,
            vec![
                Identifier::new(kind("vin"), "1FTFW1E50PFA00001").unwrap(),
                Identifier::new(kind("plate"), "ncx 4471").unwrap(),
                Identifier::new(kind("plate"), &"9".repeat(20)).unwrap(),
            ],
        )
        .await;

        let fourteen = MIGRATOR
            .iter()
            .find(|migration| migration.version == 14)
            .unwrap();
        sqlx::raw_sql(fourteen.sql.clone())
            .execute(&mut connection)
            .await
            .unwrap();

        let plate = sqlx::query("SELECT * FROM license_plates")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(plate.get::<String, _>("number"), "NCX 4471");
        assert_eq!(plate.get::<Option<String>, _>("authority"), None);
        assert_eq!(plate.get::<i64, _>("sequence"), 6);
        let id: Vec<u8> = plate.get("id");
        id_from_sql::<LicensePlate>(&id, "id").expect("a UUIDv7");
        assert_eq!(
            plate.get::<Vec<u8>, _>("added_by"),
            test_id::<User>(STUART).to_bytes()
        );

        let asset = sqlx::query("SELECT identifiers, sequence FROM assets")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        let left: Vec<Identifier> =
            serde_json::from_str(&asset.get::<String, _>("identifiers")).unwrap();
        assert_eq!(left.len(), 2, "the VIN, and the plate too long to move");
        assert_eq!(left[0].kind.as_str(), "vin");
        assert_eq!(left[1].kind.as_str(), "plate");
        assert_eq!(asset.get::<i64, _>("sequence"), 7);
        let last: i64 = sqlx::query_scalar("SELECT last_sequence FROM database_meta")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(last, 7);
    }
}
