//! Maintenance and repair work, squawks, requirements, and bundles against a real database
//! (milestone 7).

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod maintenance {
    use domain::asset::{Asset, AssetKind, AssetName, AssetPatch, LifecycleStatus};
    use domain::command::{Clearable, Patch};
    use domain::dates::EventDates;
    use domain::event::{
        Event, EventLocation, EventPatch, EventSource, EventStatus, Finalizer, Part, Reading,
        ReadingSet, Reason, Task, WorkDetails,
    };
    use domain::event_check::finalize;
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::label::Label;
    use domain::location::{LocationSource, TimeZone};
    use domain::meter::{EventKind, Meter, MeterDimension, MeterPatch, MeterUnit};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::requirement::{
        Bundle, BundlePatch, DoneTask, IntervalRule, Requirement, RequirementPatch,
    };
    use domain::squawk::{
        Disposition, Severity, Squawk, SquawkDisposition, SquawkPatch, SquawkStatus, SquawkText,
    };
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use std::str::FromStr;

    use sqlx::sqlite::SqliteConnectOptions;
    use sqlx::{Connection, Row, SqliteConnection};
    use storage::assets::{AssetWriter, GroupWriter, MeterWriter};
    use storage::commands::RecordWriter;
    use storage::database::MIGRATOR;
    use storage::events::{CountedReading, EventWriter};
    use storage::requirements::{BundleWriter, RequirementWriter};
    use storage::squawks::SquawkWriter;
    use storage::users::UserWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const DAY: i64 = 24 * 60 * 60 * 1000;
    const STUART: u32 = 1;
    const GARAGE: u32 = 50;
    const TRUCK: u32 = 10;
    const ODOMETER: u32 = 20;
    const OIL: u32 = 40;
    const TIRES: u32 = 41;
    const SERVICE: u32 = 45;
    const BRAKE_LIGHT: u32 = 60;
    const RATTLE: u32 = 61;

    fn at(days: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + days * DAY)
    }

    /// A database with a person and a truck with an odometer.
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
            .create(&mut connection, test_id(TRUCK), &AssetPatch::all(asset), 3)
            .await
            .unwrap();
        add_schedule_and_squawks(&mut connection).await;
        drop(connection);
        (temp, database)
    }

    /// The truck's odometer, two requirements (oil by distance or a year; tires when needed),
    /// and two squawks.
    async fn add_schedule_and_squawks(connection: &mut SqliteConnection) {
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
        for (n, name, rules) in [
            (
                OIL,
                "Engine oil & filter",
                vec![
                    IntervalRule::Meter {
                        meter: test_id(ODOMETER),
                        every: 8_046_720,
                    },
                    IntervalRule::Calendar {
                        every: 12,
                        unit: domain::requirement::CalendarUnit::Months,
                        end_of_month: false,
                    },
                ],
            ),
            (TIRES, "Rotate tires", vec![]),
        ] {
            let requirement = Requirement {
                asset: test_id(TRUCK),
                name: Label::parse(name).unwrap(),
                notes: Notes::default(),
                enabled: true,
                rules,
            };
            RequirementWriter
                .create(
                    connection,
                    test_id(n),
                    &RequirementPatch::all(requirement),
                    5,
                )
                .await
                .unwrap();
        }
        for (n, description) in [(BRAKE_LIGHT, "Brake light out"), (RATTLE, "Rattle")] {
            SquawkWriter
                .create(
                    connection,
                    test_id(n),
                    &SquawkPatch::all(squawk(description)),
                    6,
                )
                .await
                .unwrap();
        }
    }

    fn squawk(description: &str) -> Squawk {
        Squawk {
            asset: test_id(TRUCK),
            description: SquawkText::parse(description).unwrap(),
            notes: Notes::default(),
            severity: Severity::Low,
            opened_at: NOW,
            opened_by: test_id(STUART),
            found_on: None,
            received_at: NOW,
        }
    }

    fn reading(value: i64) -> Reading {
        Reading {
            meter: test_id(ODOMETER),
            value,
        }
    }

    fn reason(text: &str) -> Reason {
        Reason::parse(text).unwrap()
    }

    fn task(title: &str, requirement: Option<u32>) -> Task {
        Task {
            title: Label::parse(title).unwrap(),
            requirement: requirement.map(test_id),
            notes: Notes::default(),
            parts: vec![],
            labor_seconds: None,
            labor_cost_minor: None,
            resets_from: ReadingSet::Start,
            next_due: None,
        }
    }

    /// An oil change `days` after `NOW`, at `odometer` meters, with every part of its work
    /// filled in: two tasks (one with parts, reset from the end readings), and both squawks
    /// dispositioned.
    fn oil_change(days: i64, odometer: i64) -> Event {
        let mut oil = task("Engine oil & filter", Some(OIL));
        oil.notes = Notes::parse("5W-30 **synthetic**").unwrap();
        oil.parts = vec![
            Part {
                description: "Oil filter".into(),
                part_number: Some("FL-500S".into()),
                quantity_milli: 1_000,
                unit_price_minor: Some(1_299),
            },
            Part {
                description: "Engine oil".into(),
                part_number: None,
                quantity_milli: 5_500,
                unit_price_minor: None,
            },
        ];
        oil.labor_seconds = Some(1_800);
        oil.labor_cost_minor = Some(4_500);
        oil.resets_from = ReadingSet::End;
        Event {
            asset: test_id(TRUCK),
            kind: EventKind::Maintenance,
            dates: EventDates {
                start: at(days),
                end: Some(at(days + 1)),
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
            readings: vec![reading(odometer)],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            logged_by: test_id(STUART),
            performed_by: None,
            fuel: None,
            work: Some(WorkDetails {
                tasks: vec![oil, task("Wipers", None)],
                end_readings: vec![reading(odometer + 16_000)],
                dispositions: vec![
                    SquawkDisposition {
                        squawk: test_id(BRAKE_LIGHT),
                        action: Disposition::SignedOff {
                            notes: Notes::parse("New bulb").unwrap(),
                            task: Some(1),
                        },
                        severity: None,
                    },
                    SquawkDisposition {
                        squawk: test_id(RATTLE),
                        action: Disposition::Deferred {
                            reason: reason("Parts on order"),
                        },
                        severity: Some(Severity::Medium),
                    },
                ],
                currency: CurrencyCode::USD,
                complaint: None,
                cause: None,
            }),
            lifecycle: None,
            reading_override: None,
            status: EventStatus::Draft,
            last_edited_at: at(days),
            received_at: at(days),
            finalized: None,
            corrects: None,
            correction_reason: None,
        }
    }

    async fn create(connection: &mut SqliteConnection, n: u32, event: &Event) {
        EventWriter
            .create(connection, test_id(n), &EventPatch::all(event.clone()), 10)
            .await
            .unwrap();
    }

    async fn finish(connection: &mut SqliteConnection, n: u32, when: UnixMillis) {
        EventWriter
            .update(
                connection,
                test_id(n),
                &finalize(Finalizer::User(test_id(STUART)), when),
                20,
            )
            .await
            .unwrap();
    }

    #[tokio::test]
    async fn a_jobs_work_round_trips_and_is_rewritten_whole() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let event = oil_change(0, 80_000_000);
        create(&mut connection, 100, &event).await;
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(event.clone())
        );
        assert_eq!(
            database.events_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(100), event.clone())]
        );

        // Changing only the start readings keeps the end ones.
        let readings = EventPatch {
            readings: Some(vec![reading(80_000_100)]),
            ..EventPatch::default()
        };
        EventWriter
            .update(&mut connection, test_id(100), &readings, 11)
            .await
            .unwrap();
        let mut expected = event.clone();
        expected.readings = vec![reading(80_000_100)];
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(expected.clone())
        );

        // Less work: one task, a complaint and cause, no end readings.
        let mut repair = expected.work.clone().unwrap();
        repair.tasks.truncate(1);
        repair.tasks[0].parts.truncate(1);
        repair.end_readings.clear();
        repair.dispositions.truncate(1);
        repair.dispositions[0].action = Disposition::SignedOff {
            notes: Notes::default(),
            task: None,
        };
        repair.complaint = Some(Notes::parse("Squeal when braking").unwrap());
        repair.cause = Some(Notes::parse("Worn pads").unwrap());
        let work = EventPatch {
            work: Some(Clearable::Set(repair.clone())),
            ..EventPatch::default()
        };
        EventWriter
            .update(&mut connection, test_id(100), &work, 12)
            .await
            .unwrap();
        expected.work = Some(repair);
        assert_eq!(
            database.event(test_id(100)).await.unwrap(),
            Some(expected.clone())
        );

        // Clearing the work removes all of it.
        let clear = EventPatch {
            work: Some(Clearable::Clear),
            ..EventPatch::default()
        };
        EventWriter
            .update(&mut connection, test_id(100), &clear, 13)
            .await
            .unwrap();
        expected.work = None;
        assert_eq!(database.event(test_id(100)).await.unwrap(), Some(expected));
        for table in [
            "work_details",
            "event_tasks",
            "task_parts",
            "squawk_dispositions",
        ] {
            let sql = format!("SELECT count(*) FROM {table}");
            let count: i64 = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
                .fetch_one(&mut *connection)
                .await
                .unwrap();
            assert_eq!(count, 0, "{table}");
        }
    }

    #[tokio::test]
    async fn end_readings_count_at_the_jobs_end() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        create(&mut connection, 100, &oil_change(0, 80_000_000)).await;
        let mut one_day = oil_change(5, 80_100_000);
        one_day.dates.end = None;
        create(&mut connection, 101, &one_day).await;
        let counted = |event: u32, days: i64, value: i64| CountedReading {
            meter: test_id(ODOMETER),
            event: test_id(event),
            at: at(days),
            value,
        };
        assert_eq!(
            database.readings_of(test_id(TRUCK)).await.unwrap(),
            vec![
                counted(100, 0, 80_000_000),
                counted(100, 1, 80_016_000),
                counted(101, 5, 80_100_000),
                counted(101, 5, 80_116_000),
            ]
        );
        assert_eq!(
            database.latest_readings(test_id(TRUCK)).await.unwrap(),
            vec![counted(101, 5, 80_116_000)]
        );
    }

    #[tokio::test]
    async fn squawks_stand_where_their_counted_dispositions_put_them() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let both = vec![test_id(BRAKE_LIGHT), test_id(RATTLE)];
        assert_eq!(
            database
                .outstanding_squawks_of(test_id(TRUCK))
                .await
                .unwrap(),
            both
        );

        // A draft's dispositions don't count yet.
        create(&mut connection, 100, &oil_change(0, 80_000_000)).await;
        assert_eq!(
            database
                .outstanding_squawks_of(test_id(TRUCK))
                .await
                .unwrap(),
            both
        );

        // Finished: the brake light is resolved, the rattle deferred and now Medium.
        finish(&mut connection, 100, at(1)).await;
        let states = database.squawk_states_of(test_id(TRUCK)).await.unwrap();
        let state = |n: u32| states.iter().find(|state| state.id == test_id(n)).unwrap();
        assert_eq!(state(BRAKE_LIGHT).status(), SquawkStatus::Resolved);
        assert_eq!(state(RATTLE).status(), SquawkStatus::Deferred);
        assert_eq!(state(RATTLE).severity(), Severity::Medium);
        assert_eq!(state(RATTLE).history[0].event, test_id(100));
        assert_eq!(
            database
                .outstanding_squawks_of(test_id(TRUCK))
                .await
                .unwrap(),
            vec![test_id(RATTLE)]
        );

        // A finished correction that signs off the rattle counts instead.
        let mut correction = oil_change(0, 80_000_000);
        let work = correction.work.as_mut().unwrap();
        work.dispositions[1].action = Disposition::SignedOff {
            notes: Notes::default(),
            task: None,
        };
        correction.corrects = Some(test_id(100));
        correction.correction_reason = Some(reason("Fixed it after all"));
        create(&mut connection, 101, &correction).await;
        assert_eq!(
            database
                .outstanding_squawks_of(test_id(TRUCK))
                .await
                .unwrap(),
            vec![test_id(RATTLE)],
            "not until the correction is finished"
        );
        finish(&mut connection, 101, at(2)).await;
        assert_eq!(
            database
                .outstanding_squawks_of(test_id(TRUCK))
                .await
                .unwrap(),
            vec![]
        );
        let history = database
            .counted_dispositions_of(test_id(TRUCK))
            .await
            .unwrap();
        assert!(history.iter().all(|counted| counted.event == test_id(101)));
    }

    #[tokio::test]
    async fn a_squawk_round_trips_and_its_edits_apply() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        create(&mut connection, 100, &oil_change(0, 80_000_000)).await;
        let mut found = squawk("Wiper streaks");
        found.notes = Notes::parse("Passenger side").unwrap();
        found.found_on = Some(test_id(100));
        SquawkWriter
            .create(
                &mut connection,
                test_id(62),
                &SquawkPatch::all(found.clone()),
                7,
            )
            .await
            .unwrap();
        assert_eq!(
            database.squawk(test_id(62)).await.unwrap(),
            Some(found.clone())
        );

        let patch = SquawkPatch {
            description: Some(SquawkText::parse("Wipers streak").unwrap()),
            severity: Some(Severity::High),
            found_on: Some(Clearable::Clear),
            // Fixed once opened: ignored.
            opened_by: Some(test_id(999)),
            ..SquawkPatch::default()
        };
        assert_eq!(
            patch.changed_fields(),
            vec!["description", "severity", "opened_by", "found_on"]
        );
        SquawkWriter
            .update(&mut connection, test_id(62), &patch, 8)
            .await
            .unwrap();
        found.description = SquawkText::parse("Wipers streak").unwrap();
        found.severity = Severity::High;
        found.found_on = None;
        assert_eq!(database.squawk(test_id(62)).await.unwrap(), Some(found));
        assert_eq!(database.squawks_of(test_id(TRUCK)).await.unwrap().len(), 3);

        SquawkWriter
            .delete(&mut connection, test_id(62), 9)
            .await
            .unwrap();
        assert_eq!(database.squawk(test_id(62)).await.unwrap(), None);
        assert_eq!(database.squawks_of(test_id(TRUCK)).await.unwrap().len(), 2);
    }

    #[tokio::test]
    async fn requirements_and_bundles_round_trip() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let names: Vec<String> = database
            .requirements_of(test_id(TRUCK))
            .await
            .unwrap()
            .iter()
            .map(|(_, requirement)| requirement.name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["Engine oil & filter", "Rotate tires"]);

        let patch = RequirementPatch {
            notes: Some(Notes::parse("Every other oil change").unwrap()),
            enabled: Some(false),
            rules: Some(vec![IntervalRule::FixedDate {
                first: Some("2026-11-30".into()),
            }]),
            ..RequirementPatch::default()
        };
        RequirementWriter
            .update(&mut connection, test_id(TIRES), &patch, 7)
            .await
            .unwrap();
        let tires = database.requirement(test_id(TIRES)).await.unwrap().unwrap();
        assert_eq!(tires.name.as_str(), "Rotate tires");
        assert!(!tires.enabled);
        assert_eq!(tires.notes.as_str(), "Every other oil change");
        assert_eq!(patch.rules.unwrap(), tires.rules);

        let bundle = Bundle {
            asset: test_id(TRUCK),
            name: Label::parse("5,000-mile service").unwrap(),
            requirements: vec![test_id(OIL), test_id(TIRES)],
        };
        BundleWriter
            .create(
                &mut connection,
                test_id(SERVICE),
                &BundlePatch::all(bundle.clone()),
                8,
            )
            .await
            .unwrap();
        assert_eq!(
            database.bundles_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(SERVICE), bundle.clone())]
        );
        let reordered = BundlePatch {
            requirements: Some(vec![test_id(TIRES), test_id(OIL)]),
            ..BundlePatch::default()
        };
        BundleWriter
            .update(&mut connection, test_id(SERVICE), &reordered, 9)
            .await
            .unwrap();
        assert_eq!(
            database
                .bundle(test_id(SERVICE))
                .await
                .unwrap()
                .unwrap()
                .requirements,
            vec![test_id(TIRES), test_id(OIL)]
        );

        BundleWriter
            .delete(&mut connection, test_id(SERVICE), 10)
            .await
            .unwrap();
        RequirementWriter
            .delete(&mut connection, test_id(TIRES), 11)
            .await
            .unwrap();
        assert_eq!(database.bundles_of(test_id(TRUCK)).await.unwrap(), vec![]);
        assert_eq!(database.requirement(test_id(TIRES)).await.unwrap(), None);
        assert_eq!(
            database
                .requirements_of(test_id(TRUCK))
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn done_tasks_come_from_finished_jobs() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        create(&mut connection, 100, &oil_change(0, 80_000_000)).await;
        assert_eq!(
            database.done_tasks_of(test_id(TRUCK)).await.unwrap(),
            vec![]
        );
        finish(&mut connection, 100, at(1)).await;
        // Only the task with a requirement, reset from the end readings.
        assert_eq!(
            database.done_tasks_of(test_id(TRUCK)).await.unwrap(),
            vec![DoneTask {
                requirement: test_id(OIL),
                at: at(0),
                event: test_id(100),
                readings: vec![reading(80_016_000)],
                next_due: None,
            }]
        );
    }

    #[tokio::test]
    async fn forgotten_jobs_are_maintenance_and_repair_drafts() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        create(&mut connection, 100, &oil_change(0, 80_000_000)).await;
        create(&mut connection, 101, &oil_change(20, 80_100_000)).await;
        create(&mut connection, 102, &oil_change(1, 80_200_000)).await;
        finish(&mut connection, 102, at(2)).await;
        let cutoff = at(14);
        assert_eq!(
            database.forgotten_jobs(cutoff).await.unwrap(),
            vec![test_id(100)]
        );
        assert_eq!(
            database.drafts_edited_before(cutoff).await.unwrap(),
            vec![],
            "the server never finalizes a job by itself"
        );
    }

    #[tokio::test]
    async fn existing_readings_become_start_readings() {
        // Foreign keys off, so a reading needs no event.
        let options = SqliteConnectOptions::from_str("sqlite::memory:")
            .unwrap()
            .foreign_keys(false);
        let mut connection = SqliteConnection::connect_with(&options).await.unwrap();
        for migration in MIGRATOR.iter().filter(|migration| migration.version < 12) {
            sqlx::raw_sql(migration.sql.clone())
                .execute(&mut connection)
                .await
                .unwrap();
        }
        sqlx::query("INSERT INTO event_readings (event_id, meter_id, value) VALUES (?, ?, ?)")
            .bind(vec![1_u8; 16])
            .bind(vec![2_u8; 16])
            .bind(80_000_000_i64)
            .execute(&mut connection)
            .await
            .unwrap();
        let twelve = MIGRATOR
            .iter()
            .find(|migration| migration.version == 12)
            .unwrap();
        sqlx::raw_sql(twelve.sql.clone())
            .execute(&mut connection)
            .await
            .unwrap();
        let row = sqlx::query("SELECT reading_set, value FROM event_readings")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        assert_eq!(row.get::<String, _>("reading_set"), "start");
        assert_eq!(row.get::<i64, _>("value"), 80_000_000);
    }
}
