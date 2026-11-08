//! Lifecycle events, reference documents, and requirements' compliance against a real database
//! (milestone 9), and migration 0015's rebuild of `events` and `attachments` on a filled one.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod lifecycle {
    use std::collections::BTreeMap;
    use std::str::FromStr;

    use domain::asset::{Asset, AssetKind, AssetName, AssetPatch, LifecycleStatus};
    use domain::attachment::{Attachment, AttachmentOwner, AttachmentPatch, Content, FileName};
    use domain::command::{Clearable, Patch};
    use domain::dates::{CalendarDate, EventDates};
    use domain::event::{
        Event, EventLocation, EventPatch, EventSource, EventStatus, Finalized, Finalizer,
        FuelDetails, Reading, ReadingSet, Reason, Task, WorkDetails,
    };
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::label::Label;
    use domain::lifecycle::{LifecycleDetails, Money, Party, Return, SendOut};
    use domain::location::{LocationSource, TimeZone};
    use domain::meter::{EventKind, Meter, MeterDimension, MeterPatch, MeterUnit};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::reference::{
        RECALL, ReferenceDocument, ReferenceDocumentPatch, ReferenceNumber, ReferenceType,
    };
    use domain::requirement::{Applicability, ComplianceMode, Requirement, RequirementPatch};
    use domain::squawk::{
        Disposition, Severity, Squawk, SquawkDisposition, SquawkPatch, SquawkText,
    };
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{Connection, Row, SqliteConnection};
    use storage::assets::{AssetWriter, GroupWriter, MeterWriter};
    use storage::attachments::AttachmentWriter;
    use storage::commands::RecordWriter;
    use storage::database::MIGRATOR;
    use storage::events::EventWriter;
    use storage::references::ReferenceDocumentWriter;
    use storage::requirements::RequirementWriter;
    use storage::squawks::SquawkWriter;
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const DAY: i64 = 24 * 60 * 60 * 1000;
    const STUART: u32 = 1;
    const TRUCK: u32 = 10;
    const ODOMETER: u32 = 20;
    const OIL: u32 = 40;
    const RECALL_ID: u32 = 45;
    const GARAGE: u32 = 50;
    const RATTLE: u32 = 60;
    const JOB: u32 = 100;
    const FIX: u32 = 101;
    const FUEL: u32 = 102;
    const GONE: u32 = 103;
    const SENT: u32 = 110;
    const BACK: u32 = 111;
    const AXLE: u32 = 70;

    fn at(days: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + days * DAY)
    }

    /// A person, a group, a truck with an odometer, and a squawk, written on `connection`.
    async fn add_household(connection: &mut SqliteConnection) {
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
        let asset = Asset {
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
            identifiers: vec![],
        };
        AssetWriter
            .create(connection, test_id(TRUCK), &AssetPatch::all(asset), 3)
            .await
            .unwrap();
        let meter = Meter {
            asset: test_id(TRUCK),
            name: Label::parse("Odometer").unwrap(),
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            precision: 0,
            description: None,
            required_for: vec![EventKind::Maintenance],
        };
        MeterWriter
            .create(connection, test_id(ODOMETER), &MeterPatch::all(meter), 4)
            .await
            .unwrap();
        let squawk = Squawk {
            asset: test_id(TRUCK),
            description: SquawkText::parse("Rattle").unwrap(),
            notes: Notes::default(),
            severity: Severity::Low,
            opened_at: NOW,
            opened_by: test_id(STUART),
            found_on: None,
            received_at: NOW,
        };
        SquawkWriter
            .create(connection, test_id(RATTLE), &SquawkPatch::all(squawk), 5)
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
        add_household(&mut connection).await;
        drop(connection);
        (temp, database)
    }

    fn reason(text: &str) -> Reason {
        Reason::parse(text).unwrap()
    }

    /// An event of `kind` on the truck `days` after `NOW`, finalized, with nothing else.
    fn event(kind: EventKind, days: i64) -> Event {
        Event {
            asset: test_id(TRUCK),
            kind,
            dates: EventDates {
                start: at(days),
                end: None,
                zone: TimeZone::parse("America/New_York").unwrap(),
            },
            location: EventLocation {
                place: None,
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: Some(reason("In the driveway")),
            },
            readings: vec![Reading {
                meter: test_id(ODOMETER),
                value: 80_000_000 + days,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::parse("Some **notes**").unwrap(),
            source: EventSource::Typed,
            logged_by: test_id(STUART),
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: None,
            reading_override: None,
            status: EventStatus::Finalized,
            last_edited_at: at(days),
            received_at: at(days),
            finalized: Some(Finalized {
                at: at(days),
                by: Finalizer::Server,
            }),
            corrects: None,
            correction_reason: None,
        }
    }

    /// A finished oil change with a task for the oil requirement and the rattle deferred.
    fn oil_change() -> Event {
        Event {
            work: Some(WorkDetails {
                tasks: vec![Task {
                    title: Label::parse("Engine oil & filter").unwrap(),
                    requirement: Some(test_id(OIL)),
                    notes: Notes::default(),
                    parts: vec![domain::event::Part {
                        description: "Oil filter".into(),
                        part_number: Some("FL-500S".into()),
                        quantity_milli: 1_000,
                        unit_price_minor: Some(1_299),
                    }],
                    labor_seconds: Some(1_800),
                    labor_cost_minor: None,
                    resets_from: ReadingSet::Start,
                    next_due: None,
                }],
                end_readings: vec![],
                dispositions: vec![SquawkDisposition {
                    squawk: test_id(RATTLE),
                    action: Disposition::Deferred {
                        reason: reason("Parts on order"),
                    },
                    severity: None,
                }],
                currency: CurrencyCode::USD,
                complaint: None,
                cause: None,
            }),
            ..event(EventKind::Maintenance, 0)
        }
    }

    fn fuel_up() -> Event {
        Event {
            fuel: Some(FuelDetails {
                volume_ml: 68_137,
                cost_minor: Some(24_517),
                currency: CurrencyCode::USD,
                filled_full: true,
                missed_previous: false,
                highway_pct: 70,
                towing_pct: 0,
                hauling_pct: 0,
                towed: vec![],
                reported_economy: None,
            }),
            ..event(EventKind::Fuel, 2)
        }
    }

    async fn create_event(connection: &mut SqliteConnection, n: u32, event: &Event) {
        EventWriter
            .create(connection, test_id(n), &EventPatch::all(event.clone()), 10)
            .await
            .unwrap();
    }

    /// Every table's rows, each as its columns' quoted values, for the columns it has now.
    async fn dump(
        connection: &mut SqliteConnection,
        columns_of: &BTreeMap<String, Vec<String>>,
    ) -> BTreeMap<String, Vec<String>> {
        let mut rows = BTreeMap::new();
        for (table, columns) in columns_of {
            let values = columns
                .iter()
                .map(|column| format!("quote(\"{column}\")"))
                .collect::<Vec<_>>()
                .join(" || '|' || ");
            let sql = format!("SELECT {values} AS line FROM \"{table}\" ORDER BY line");
            let lines: Vec<String> = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_all(&mut *connection)
                .await
                .unwrap();
            rows.insert(table.clone(), lines);
        }
        rows
    }

    /// Every table and its columns.
    async fn columns(connection: &mut SqliteConnection) -> BTreeMap<String, Vec<String>> {
        let tables: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_schema WHERE type = 'table' AND name NOT LIKE 'sqlite_%'
             ORDER BY name",
        )
        .fetch_all(&mut *connection)
        .await
        .unwrap();
        let mut out = BTreeMap::new();
        for table in tables {
            let columns: Vec<String> =
                sqlx::query_scalar("SELECT name FROM pragma_table_info(?) ORDER BY cid")
                    .bind(&table)
                    .fetch_all(&mut *connection)
                    .await
                    .unwrap();
            out.insert(table, columns);
        }
        out
    }

    /// A truck with a finished job and its correction, a fuel-up, a deleted draft, a
    /// requirement, a squawk, and three attachments (one removed), at schema 14.
    async fn fill_before_0015(connection: &mut SqliteConnection) {
        add_household(connection).await;
        sqlx::query(
            "INSERT INTO requirements (id, asset_id, name, notes, enabled, rules, sequence)
             VALUES (?, ?, 'Engine oil & filter', '', 1, '[]', 6)",
        )
        .bind(test_id::<Requirement>(OIL).to_bytes().to_vec())
        .bind(test_id::<Asset>(TRUCK).to_bytes().to_vec())
        .execute(&mut *connection)
        .await
        .unwrap();
        create_event(connection, JOB, &oil_change()).await;
        let mut fix = oil_change();
        fix.corrects = Some(test_id(JOB));
        fix.correction_reason = Some(reason("Wrong filter"));
        create_event(connection, FIX, &fix).await;
        create_event(connection, FUEL, &fuel_up()).await;
        let mut gone = event(EventKind::Reading, 3);
        gone.status = EventStatus::Draft;
        gone.finalized = None;
        create_event(connection, GONE, &gone).await;
        sqlx::query("UPDATE events SET deleted = 1 WHERE id = ?")
            .bind(test_id::<Event>(GONE).to_bytes().to_vec())
            .execute(&mut *connection)
            .await
            .unwrap();
        for (n, owner, event, deleted) in [
            (200_u32, "vehicle", None, 0_i64),
            (201, "event", Some(JOB), 0),
            (202, "event", Some(FIX), 1),
        ] {
            sqlx::query(
                "INSERT INTO attachments (id, asset_id, owner, event_id, kind, file_sha256,
                     file_bytes, name, content, added_ms, added_by, received_ms, sequence,
                     deleted)
                 VALUES (?, ?, ?, ?, 'document', ?, 1000, 'scan.pdf', '{\"kind\":\"document\",\"pages\":2}',
                         0, ?, 0, 20, ?)",
            )
            .bind(test_id::<Attachment>(n).to_bytes().to_vec())
            .bind(test_id::<Asset>(TRUCK).to_bytes().to_vec())
            .bind(owner)
            .bind(event.map(|n| test_id::<Event>(n).to_bytes().to_vec()))
            .bind("a".repeat(64))
            .bind(test_id::<User>(STUART).to_bytes().to_vec())
            .bind(deleted)
            .execute(&mut *connection)
            .await
            .unwrap();
        }
    }

    /// Foreign keys back on, none broken, the database sound, and every index back.
    async fn assert_sound_after_0015(connection: &mut SqliteConnection) {
        let foreign_keys_on: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(foreign_keys_on, 1, "turned back on");
        let broken: i64 = sqlx::query_scalar("SELECT count(*) FROM pragma_foreign_key_check")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(broken, 0);
        let integrity: String = sqlx::query_scalar("PRAGMA integrity_check")
            .fetch_one(&mut *connection)
            .await
            .unwrap();
        assert_eq!(integrity, "ok");
        let indexes: Vec<String> = sqlx::query_scalar(
            "SELECT name FROM sqlite_schema WHERE type = 'index' AND tbl_name IN ('events',
             'attachments') AND name NOT LIKE 'sqlite_%' ORDER BY name",
        )
        .fetch_all(&mut *connection)
        .await
        .unwrap();
        assert_eq!(
            indexes,
            [
                "attachments_by_asset",
                "attachments_by_document",
                "attachments_by_event",
                "attachments_by_reference",
                "attachments_by_squawk",
                "events_by_asset",
                "events_by_correction",
                "events_drafts",
            ]
        );
    }

    #[tokio::test]
    async fn migration_0015_rebuilds_events_and_attachments_keeping_every_row() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(true);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        for migration in MIGRATOR.iter().filter(|migration| migration.version < 15) {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&mut connection)
                .await
                .unwrap();
        }
        fill_before_0015(&mut connection).await;
        let before_columns = columns(&mut connection).await;
        let before = dump(&mut connection, &before_columns).await;
        assert_eq!(before["events"].len(), 4);
        assert_eq!(before["event_tasks"].len(), 2);

        let fifteen = MIGRATOR
            .iter()
            .find(|migration| migration.version == 15)
            .unwrap();
        assert!(fifteen.no_tx, "it must run outside sqlx's transaction");
        sqlx::raw_sql(fifteen.sql.clone())
            .execute(&mut connection)
            .await
            .unwrap();

        assert_eq!(
            dump(&mut connection, &before_columns).await,
            before,
            "every row of every table, with the same values"
        );
        let after_columns = columns(&mut connection).await;
        for table in ["lifecycle_details", "reference_documents"] {
            assert!(after_columns.contains_key(table), "{table}");
        }
        assert!(!after_columns.contains_key("events_new"));
        assert!(!after_columns.contains_key("attachments_new"));
        assert_sound_after_0015(&mut connection).await;
        let requirement = sqlx::query("SELECT mode, applicability, reference_id FROM requirements")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(requirement.get::<String, _>("mode"), "recurring");
        assert_eq!(
            requirement.get::<String, _>("applicability"),
            r#"{"applies":"applies"}"#
        );
        assert_eq!(requirement.get::<Option<Vec<u8>>, _>("reference_id"), None);
    }

    #[tokio::test]
    async fn migration_0015_refuses_a_database_with_a_broken_foreign_key() {
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(false);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        for migration in MIGRATOR.iter().filter(|migration| migration.version < 15) {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&mut connection)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO event_readings (event_id, meter_id, value) VALUES (?, ?, 5)")
            .bind(vec![1_u8; 16])
            .bind(vec![2_u8; 16])
            .execute(&mut connection)
            .await
            .unwrap();
        let fifteen = MIGRATOR
            .iter()
            .find(|migration| migration.version == 15)
            .unwrap();
        assert!(
            sqlx::raw_sql(fifteen.sql.clone())
                .execute(&mut connection)
                .await
                .is_err()
        );
    }

    fn party(name: &str) -> Party {
        Party {
            name: name.into(),
            contact: Some("555-0100".into()),
        }
    }

    #[tokio::test]
    async fn a_send_out_and_its_return_round_trip() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let sent = Event {
            lifecycle: Some(LifecycleDetails::SentOut(SendOut {
                shop: party("Joe's Auto"),
                work_order: Some("WO-1182".into()),
                expected_back: Some(CalendarDate::parse("2026-11-10").unwrap()),
                carrier: None,
                tracking: None,
                estimate: Some(Money {
                    minor: 45_000,
                    currency: CurrencyCode::USD,
                }),
            })),
            ..event(EventKind::SentOut, 0)
        };
        create_event(&mut connection, SENT, &sent).await;
        let mut back = Event {
            lifecycle: Some(LifecycleDetails::Returned(Return {
                sent_out: test_id(SENT),
                cost: None,
            })),
            ..event(EventKind::Returned, 5)
        };
        back.status = EventStatus::Draft;
        back.finalized = None;
        create_event(&mut connection, BACK, &back).await;
        assert_eq!(
            database.event(test_id(SENT)).await.unwrap(),
            Some(sent.clone())
        );
        assert_eq!(
            database.event(test_id(BACK)).await.unwrap(),
            Some(back.clone())
        );
        let sent_out: Option<Vec<u8>> =
            sqlx::query_scalar("SELECT sent_out_id FROM lifecycle_details WHERE event_id = ?")
                .bind(test_id::<Event>(BACK).to_bytes().to_vec())
                .fetch_one(&mut *connection)
                .await
                .unwrap();
        assert_eq!(sent_out, Some(test_id::<Event>(SENT).to_bytes().to_vec()));

        // Editing the draft return rewrites its details.
        let cost = LifecycleDetails::Returned(Return {
            sent_out: test_id(SENT),
            cost: Some(Money {
                minor: 51_250,
                currency: CurrencyCode::USD,
            }),
        });
        EventWriter
            .update(
                &mut connection,
                test_id(BACK),
                &EventPatch {
                    lifecycle: Some(Clearable::Set(cost.clone())),
                    ..EventPatch::default()
                },
                11,
            )
            .await
            .unwrap();
        back.lifecycle = Some(cost);
        let events = database.events_of(test_id(TRUCK)).await.unwrap();
        assert!(events.contains(&(test_id(BACK), back)));
        assert!(events.contains(&(test_id(SENT), sent)));
    }

    fn recall() -> ReferenceDocument {
        ReferenceDocument {
            doc_type: ReferenceType::listed(RECALL).unwrap(),
            authority: Some("NHTSA".into()),
            numbers: vec![ReferenceNumber {
                scheme: Some(Label::parse("NHTSA campaign").unwrap()),
                number: "24V-123".into(),
            }],
            title: "Rear axle nut may loosen".into(),
            description: Notes::parse("The nut may not be torqued.").unwrap(),
            remedy: Notes::parse("Dealers will retorque it.").unwrap(),
            issued_on: Some(CalendarDate::parse("2024-03-01").unwrap()),
            effective_on: None,
            revision: Some("B".into()),
            supersedes: None,
            applicability: Notes::parse("2022–2024 Tacoma").unwrap(),
            url: Some("https://www.nhtsa.gov/recalls".into()),
            added_at: NOW,
            added_by: test_id(STUART),
            received_at: NOW,
        }
    }

    #[tokio::test]
    async fn a_reference_document_round_trips_and_its_edits_apply() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let document = recall();
        ReferenceDocumentWriter
            .create(
                &mut connection,
                test_id(AXLE),
                &ReferenceDocumentPatch::all(document.clone()),
                7,
            )
            .await
            .unwrap();
        assert_eq!(
            database.reference_document(test_id(AXLE)).await.unwrap(),
            Some(document.clone())
        );
        let patch = ReferenceDocumentPatch {
            revision: Some(Clearable::Clear),
            title: Some("Rear axle nut may loosen (revised)".into()),
            ..ReferenceDocumentPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["title", "revision"]);
        ReferenceDocumentWriter
            .update(&mut connection, test_id(AXLE), &patch, 8)
            .await
            .unwrap();
        let mut expected = document;
        expected.revision = None;
        expected.title = "Rear axle nut may loosen (revised)".into();
        assert_eq!(
            database.reference_documents().await.unwrap(),
            vec![(test_id(AXLE), expected)]
        );
        ReferenceDocumentWriter
            .delete(&mut connection, test_id(AXLE), 9)
            .await
            .unwrap();
        assert_eq!(
            database.reference_document(test_id(AXLE)).await.unwrap(),
            None
        );
    }

    #[tokio::test]
    async fn a_requirement_keeps_its_link_mode_and_applicability() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        ReferenceDocumentWriter
            .create(
                &mut connection,
                test_id(AXLE),
                &ReferenceDocumentPatch::all(recall()),
                7,
            )
            .await
            .unwrap();
        let requirement = Requirement {
            asset: test_id(TRUCK),
            name: Label::parse("Rear axle nut recall").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![],
            reference: Some(test_id(AXLE)),
            mode: ComplianceMode::OneTime,
            applicability: Applicability::UnderReview,
        };
        RequirementWriter
            .create(
                &mut connection,
                test_id(RECALL_ID),
                &RequirementPatch::all(requirement.clone()),
                8,
            )
            .await
            .unwrap();
        assert_eq!(
            database.requirement(test_id(RECALL_ID)).await.unwrap(),
            Some(requirement.clone())
        );
        assert_eq!(
            database.requirements_linking(test_id(AXLE)).await.unwrap(),
            vec![(test_id(RECALL_ID), requirement.clone())]
        );
        let ruled_out = Applicability::NotApplicable {
            reason: reason("Built after the VIN range"),
        };
        RequirementWriter
            .update(
                &mut connection,
                test_id(RECALL_ID),
                &RequirementPatch {
                    applicability: Some(ruled_out.clone()),
                    reference: Some(Clearable::Clear),
                    ..RequirementPatch::default()
                },
                9,
            )
            .await
            .unwrap();
        let expected = Requirement {
            applicability: ruled_out,
            reference: None,
            ..requirement
        };
        assert_eq!(
            database.requirement(test_id(RECALL_ID)).await.unwrap(),
            Some(expected)
        );
        assert!(
            database
                .requirements_linking(test_id(AXLE))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_reference_documents_attachment_is_on_no_vehicle() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        ReferenceDocumentWriter
            .create(
                &mut connection,
                test_id(AXLE),
                &ReferenceDocumentPatch::all(recall()),
                7,
            )
            .await
            .unwrap();
        let attachment = Attachment {
            asset: None,
            owner: AttachmentOwner::Reference {
                reference: test_id(AXLE),
            },
            file: domain::attachment::StoredFile {
                hash: domain::attachment::FileHash::parse(&"b".repeat(64)).unwrap(),
                bytes: 2_000,
            },
            thumbnail: None,
            name: FileName::from_upload("24V123.pdf").unwrap(),
            content: Content::Document { pages: 3 },
            added_at: NOW,
            added_by: test_id(STUART),
            received_at: NOW,
        };
        AttachmentWriter
            .create(
                &mut connection,
                test_id(300),
                &AttachmentPatch::all(attachment.clone()),
                8,
            )
            .await
            .unwrap();
        assert_eq!(
            database.attachment(test_id(300)).await.unwrap(),
            Some(attachment.clone())
        );
        assert_eq!(
            database
                .attachments_of_reference(test_id(AXLE))
                .await
                .unwrap(),
            vec![(test_id(300), attachment)]
        );
        assert!(
            database
                .attachments_of(test_id(TRUCK))
                .await
                .unwrap()
                .is_empty()
        );
    }
}
