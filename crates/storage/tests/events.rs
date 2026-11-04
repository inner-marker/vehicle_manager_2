//! Events, their readings, field values, and fuel details against a real database.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod events {
    use domain::asset::{Asset, AssetKind, AssetName, AssetPatch, LifecycleStatus};
    use domain::command::{Clearable, Patch};
    use domain::custom_field::{CustomField, CustomFieldPatch, FieldKey, FieldKind};
    use domain::dates::EventDates;
    use domain::event::{
        Event, EventLocation, EventPatch, EventSource, EventStatus, FieldData, FieldValue,
        Finalized, Finalizer, FuelDetails, Performer, Reading, Reason, Requirement, Skipped,
    };
    use domain::event_check::finalize;
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::label::Label;
    use domain::location::{Coordinates, LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, Meter, MeterDimension, MeterPatch, MeterUnit};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use sqlx::SqliteConnection;
    use storage::assets::{AssetWriter, CustomFieldWriter, GroupWriter, MeterWriter};
    use storage::commands::RecordWriter;
    use storage::events::{CountedReading, EventWriter};
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds, StorageError};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const DAY: i64 = 24 * 60 * 60 * 1000;
    const STUART: u32 = 1;
    const GARAGE: u32 = 50;
    const TRUCK: u32 = 10;
    const TRAILER: u32 = 11;
    const ODOMETER: u32 = 20;
    const HOURS: u32 = 21;
    const TRIP_TIME: u32 = 30;

    /// A database with a person, a truck (odometer and engine hours, a trip time field), and a
    /// trailer.
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        let user = User {
            username: Username::parse("stuart").unwrap(),
            display_name: "Stuart".into(),
            disabled: false,
            must_change_password: false,
        };
        UserWriter
            .create(&mut connection, test_id(STUART), &UserPatch::all(user), 1)
            .await
            .unwrap();
        let group = Group {
            name: GroupName::parse("Garage").unwrap(),
            icon: GroupIcon::Garage,
        };
        GroupWriter
            .create(&mut connection, test_id(GARAGE), &GroupPatch::all(group), 2)
            .await
            .unwrap();
        for (n, name) in [(TRUCK, "Truck"), (TRAILER, "Trailer")] {
            let asset = Asset {
                kind: AssetKind::parse("truck").unwrap(),
                name: AssetName::parse(name).unwrap(),
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
                .create(&mut connection, test_id(n), &AssetPatch::all(asset), 3)
                .await
                .unwrap();
        }
        add_meters_and_field(&mut connection).await;
        drop(connection);
        (temp, database)
    }

    /// The truck's odometer and engine hours (both required on fuel-ups), and its trip time
    /// field.
    async fn add_meters_and_field(connection: &mut SqliteConnection) {
        for (n, name, dimension, unit) in [
            (
                ODOMETER,
                "Odometer",
                MeterDimension::Distance,
                MeterUnit::Miles,
            ),
            (
                HOURS,
                "Engine hours",
                MeterDimension::Duration,
                MeterUnit::Hours,
            ),
        ] {
            let meter = Meter {
                asset: test_id(TRUCK),
                name: Label::parse(name).unwrap(),
                dimension,
                unit,
                precision: 0,
                description: None,
                required_for: vec![EventKind::Fuel],
            };
            MeterWriter
                .create(connection, test_id(n), &MeterPatch::all(meter), 4)
                .await
                .unwrap();
        }
        let field = CustomField {
            asset: test_id(TRUCK),
            shown_on: vec![EventKind::Fuel],
            key: FieldKey::parse("trip-time").unwrap(),
            label: Label::parse("Trip time").unwrap(),
            kind: FieldKind::Duration,
            enabled: true,
            required_for: vec![EventKind::Fuel],
            range: None,
        };
        CustomFieldWriter
            .create(
                connection,
                test_id(TRIP_TIME),
                &CustomFieldPatch::all(field),
                5,
            )
            .await
            .unwrap();
    }

    fn reason(text: &str) -> Reason {
        Reason::parse(text).unwrap()
    }

    /// A fuel-up with every part filled in, `days` after `NOW`, at `odometer` meters.
    fn fuel_up(days: i64, odometer: i64) -> Event {
        let at = UnixMillis::from_millis(NOW.as_millis() + days * DAY);
        Event {
            asset: test_id(TRUCK),
            kind: EventKind::Fuel,
            dates: EventDates {
                start: at,
                end: None,
                zone: TimeZone::parse("America/New_York").unwrap(),
            },
            location: EventLocation {
                place: Some(Place::parse("Asheville", "North Carolina", "US").unwrap()),
                coordinates: Some(Coordinates::from_degrees(35.5951, -82.5515).unwrap()),
                accuracy_m: Some(12),
                fix_time: Some(at),
                source: LocationSource::Gps,
                place_name: Some(Label::parse("Costco gas").unwrap()),
                saved_place: None,
                geocoder_version: Some("2026-11-02.2".into()),
                missing_reason: None,
            },
            readings: vec![Reading {
                meter: test_id(ODOMETER),
                value: odometer,
            }],
            skipped: vec![Skipped {
                item: Requirement::Meter(test_id(HOURS)),
                reason: reason("Hour meter is broken"),
            }],
            fields: vec![FieldValue {
                field: test_id(TRIP_TIME),
                data: FieldData::Duration(5_400),
            }],
            notes: Notes::parse("Pump 4 was **slow**").unwrap(),
            source: EventSource::Typed,
            logged_by: test_id(STUART),
            performed_by: Some(Performer::Named {
                name: "Joe's Auto".into(),
                contact: Some("555-0100".into()),
            }),
            fuel: Some(FuelDetails {
                volume_ml: 68_137,
                cost_minor: Some(24_517),
                currency: CurrencyCode::USD,
                filled_full: true,
                missed_previous: false,
                highway_pct: 70,
                towing_pct: 20,
                hauling_pct: 0,
                towed: vec![test_id(TRAILER)],
                reported_economy: Some(7_015),
            }),
            work: None,
            reading_override: None,
            status: EventStatus::Draft,
            last_edited_at: at,
            received_at: at,
            finalized: None,
            corrects: None,
            correction_reason: None,
        }
    }

    async fn connection(database: &Database) -> sqlx::pool::PoolConnection<sqlx::Sqlite> {
        database.pool().acquire().await.unwrap()
    }

    async fn create(connection: &mut SqliteConnection, n: u32, event: &Event) {
        EventWriter
            .create(connection, test_id(n), &EventPatch::all(event.clone()), 10)
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn every_part_of_an_event_round_trips() {
        let (_temp, database) = database().await;
        let mut connection = connection(&database).await;
        let event = fuel_up(0, 80_000_000);
        create(&mut connection, 100, &event).await;
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(event.clone())
        );
        assert_eq!(
            database.events_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(100), event.clone())]
        );

        // The other shapes: a reading with nobody recorded, no place (with a reason), and a
        // performer who's a user.
        let mut bare = fuel_up(1, 80_100_000);
        bare.kind = EventKind::Reading;
        bare.fuel = None;
        bare.fields.clear();
        bare.skipped.clear();
        bare.performed_by = None;
        bare.location = EventLocation {
            place: None,
            coordinates: None,
            accuracy_m: None,
            fix_time: None,
            source: LocationSource::Manual,
            place_name: None,
            saved_place: None,
            geocoder_version: None,
            missing_reason: Some(reason("Out on the range, no signal")),
        };
        bare.notes = Notes::default();
        create(&mut connection, 101, &bare).await;
        let mut by_user = fuel_up(2, 80_200_000);
        by_user.performed_by = Some(Performer::User {
            user: test_id(STUART),
        });
        create(&mut connection, 102, &by_user).await;
        assert_eq!(
            database.events_of(test_id(TRUCK)).await.unwrap(),
            vec![
                (test_id(102), by_user),
                (test_id(101), bare),
                (test_id(100), event)
            ],
            "newest first, each with only its own parts"
        );
        assert!(
            database
                .events_of(test_id(TRAILER))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_draft_changes_until_its_finalized_then_never() {
        let (_temp, database) = database().await;
        let mut connection = connection(&database).await;
        create(&mut connection, 100, &fuel_up(0, 80_000_000)).await;

        let later = UnixMillis::from_millis(NOW.as_millis() + 60_000);
        let patch = EventPatch {
            readings: Some(vec![Reading {
                meter: test_id(ODOMETER),
                value: 80_000_500,
            }]),
            notes: Some(Notes::parse("Pump 6").unwrap()),
            fuel: Some(Clearable::Clear),
            performed_by: Some(Clearable::Set(Performer::User {
                user: test_id(STUART),
            })),
            last_edited_at: Some(later),
            // Fixed once made: ignored.
            logged_by: Some(test_id(999)),
            ..EventPatch::default()
        };
        EventWriter
            .update(&mut connection, test_id(100), &patch, 11)
            .await
            .unwrap();
        let mut expected = fuel_up(0, 80_000_500);
        expected.notes = Notes::parse("Pump 6").unwrap();
        expected.fuel = None;
        expected.performed_by = Some(Performer::User {
            user: test_id(STUART),
        });
        expected.last_edited_at = later;
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(expected.clone())
        );

        let finalizing = finalize(Finalizer::User(test_id(STUART)), later);
        assert_eq!(finalizing.changed_fields(), vec!["status", "finalized"]);
        EventWriter
            .update(&mut connection, test_id(100), &finalizing, 12)
            .await
            .unwrap();
        expected.status = EventStatus::Finalized;
        expected.finalized = Some(Finalized {
            at: later,
            by: Finalizer::User(test_id(STUART)),
        });
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(expected.clone())
        );

        // Nothing changes it now, not even finalizing again, and it can't be deleted.
        let notes = EventPatch {
            notes: Some(Notes::parse("Sneaky").unwrap()),
            ..EventPatch::default()
        };
        for patch in [notes, finalize(Finalizer::Server, later)] {
            assert!(matches!(
                EventWriter
                    .update(&mut connection, test_id(100), &patch, 13)
                    .await,
                Err(StorageError::Finalized)
            ));
        }
        assert!(matches!(
            EventWriter.delete(&mut connection, test_id(100), 14).await,
            Err(StorageError::Finalized)
        ));
        assert_eq!(database.event(test_id(100)).await.unwrap(), Some(expected));
    }

    #[tokio::test]
    async fn the_server_can_finalize_and_drafts_can_be_deleted() {
        let (_temp, database) = database().await;
        let mut connection = connection(&database).await;
        create(&mut connection, 100, &fuel_up(0, 80_000_000)).await;
        create(&mut connection, 101, &fuel_up(1, 80_100_000)).await;
        create(&mut connection, 102, &fuel_up(2, 80_200_000)).await;
        let cutoff = UnixMillis::from_millis(NOW.as_millis() + DAY);
        assert_eq!(
            database.drafts_edited_before(cutoff).await.unwrap(),
            vec![test_id(100), test_id(101)],
            "oldest edit first; the cutoff itself counts"
        );

        EventWriter
            .update(
                &mut connection,
                test_id(100),
                &finalize(Finalizer::Server, cutoff),
                11,
            )
            .await
            .unwrap();
        let finalized = database.event(test_id(100)).await.unwrap().unwrap();
        assert_eq!(
            finalized.finalized,
            Some(Finalized {
                at: cutoff,
                by: Finalizer::Server
            })
        );
        EventWriter
            .delete(&mut connection, test_id(101), 12)
            .await
            .unwrap();
        assert_eq!(database.event(test_id(101)).await.unwrap(), None);
        assert_eq!(database.drafts_edited_before(cutoff).await.unwrap(), vec![]);
    }

    #[tokio::test]
    async fn readings_that_count_skip_deleted_and_corrected_events() {
        let (_temp, database) = database().await;
        let mut connection = connection(&database).await;
        create(&mut connection, 100, &fuel_up(0, 80_000_000)).await;
        create(&mut connection, 101, &fuel_up(1, 80_100_000)).await;
        create(&mut connection, 102, &fuel_up(2, 80_900_000)).await;
        let counted = |n: u32, days: i64, value: i64| CountedReading {
            meter: test_id(ODOMETER),
            event: test_id(n),
            at: UnixMillis::from_millis(NOW.as_millis() + days * DAY),
            value,
        };
        assert_eq!(
            database.latest_readings(test_id(TRUCK)).await.unwrap(),
            vec![counted(102, 2, 80_900_000)]
        );

        // 102 was a typo: finalize it, then correct it. The correction's reading counts instead.
        EventWriter
            .update(
                &mut connection,
                test_id(102),
                &finalize(Finalizer::Server, NOW),
                11,
            )
            .await
            .unwrap();
        let mut correction = fuel_up(2, 80_200_000);
        correction.corrects = Some(test_id(102));
        correction.correction_reason = Some(reason("Typed 809,000 for 802,000"));
        create(&mut connection, 103, &correction).await;
        assert_eq!(
            database.correction_of(test_id(102)).await.unwrap(),
            Some(test_id(103))
        );
        assert_eq!(database.correction_of(test_id(101)).await.unwrap(), None);

        // A deleted draft doesn't count either.
        EventWriter
            .delete(&mut connection, test_id(101), 12)
            .await
            .unwrap();
        assert_eq!(
            database.readings_of(test_id(TRUCK)).await.unwrap(),
            vec![counted(100, 0, 80_000_000), counted(103, 2, 80_200_000)]
        );
        assert_eq!(
            database.latest_readings(test_id(TRUCK)).await.unwrap(),
            vec![counted(103, 2, 80_200_000)]
        );
        assert!(
            database
                .readings_of(test_id(TRAILER))
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_create_missing_a_needed_field_is_refused() {
        let (_temp, database) = database().await;
        let mut connection = connection(&database).await;
        let patch = EventPatch {
            dates: None,
            ..EventPatch::all(fuel_up(0, 80_000_000))
        };
        assert!(matches!(
            EventWriter
                .create(&mut connection, test_id(100), &patch, 10)
                .await,
            Err(StorageError::InvalidValue { column: "dates" })
        ));
    }
}
