//! Pull (phase-3.md 4b, decision #075): a paired desktop's copy, page by page, through the
//! command pipeline on the server and `apply_pull_page` on a copy.
//!
//! The copy here is a plain database (an encrypted one needs the desktop's SQLCipher build;
//! `storage`'s `local_copy` tests cover that): what matters is that it ends up with the
//! server's rows, minus what isn't the person's, and that the server's own read code gives the
//! same answers from it.

mod common;

#[cfg(test)]
mod sync {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, EventsOutcome, PlacesOutcome};
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::EventDates;
    use domain::device::DeviceName;
    use domain::device_signature::public_key_of;
    use domain::event::{EventId, EventInput, EventLocation, EventSource, FuelDetails, Reading};
    use domain::location::{LocationSource, Place, SavedPlaceForm, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::sync::{PullOutcome, PullPage};
    use domain::test_support::{ManualClock, test_id};
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::events::{add, delete, edit, view};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::login_limiter::LoginLimiter;
    use server::places::PlacesContext;
    use server::sessions::{SignedIn, SigningInDevice, sign_in_from};
    use server::sync::pull;
    use storage::sync::{PullScope, schema_version};
    use storage::{Database, NewDatabaseIds, StorageError};

    const MILE: i64 = 1_609;

    fn context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> AssetsContext<'a> {
        AssetsContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            clock,
        }
    }

    fn places<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> PlacesContext<'a> {
        PlacesContext {
            database,
            user,
            health: Health::Healthy,
            clock,
            geocoder: None,
        }
    }

    /// `username` signed in on a paired desktop.
    async fn desktop(database: &Database, username: &str) -> SignedIn {
        let (_, signed_in) = sign_in_from(
            database,
            &LoginLimiter::new(),
            username,
            "a long password".into(),
            "laptop",
            NOW,
            SigningInDevice::Desktop {
                public_key: public_key_of(&[5; 32]),
                name: Some(DeviceName::parse("Garage Mac").unwrap()),
            },
        )
        .await
        .unwrap();
        signed_in
    }

    /// An empty copy of the server `test_id(900)`, as a desktop's starts.
    async fn copy() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(900),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        sqlx::query("INSERT INTO pull_state (id, server_database_id) VALUES (1, ?)")
            .bind(test_id::<()>(900).to_bytes().to_vec())
            .execute(database.pool())
            .await
            .unwrap();
        (temp, database)
    }

    /// Pulls until `copy` is up to date; returns how many pages it took.
    async fn pull_all(server: &Database, copy: &Database, desktop: &SignedIn) -> usize {
        let mut pages = 0;
        loop {
            let cursor = copy.pull_state().await.unwrap().unwrap().cursor;
            let PullOutcome::Page { page } =
                pull(server, test_id(900), desktop, cursor, schema_version()).await
            else {
                panic!("no page");
            };
            copy.apply_pull_page(&page, NOW).await.unwrap();
            pages += 1;
            if page.up_to_date() {
                return pages;
            }
        }
    }

    /// A truck, added by the Admin; and its odometer.
    async fn truck(
        database: &Database,
        stuart: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = SignedIn {
            mode: ActiveMode::Admin,
            ..stuart.clone()
        };
        let form = AssetForm {
            name: "Truck".into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("the truck wasn't added");
        };
        let AssetsOutcome::Asset { view } =
            vehicle_page(&context(database, &admin, clock), id).await
        else {
            panic!("no truck page");
        };
        (id, view.meters[0].0)
    }

    fn fuel_up(asset: AssetId, odometer: MeterId, miles: i64) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Fuel,
            dates: EventDates {
                start: NOW,
                end: None,
                zone: TimeZone::parse("America/New_York").unwrap(),
            },
            location: EventLocation {
                place: Some(Place::parse("Asheville", "North Carolina", "US").unwrap()),
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![Reading {
                meter: odometer,
                value: miles * MILE,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: Some(FuelDetails {
                volume_ml: 60_000,
                cost_minor: Some(21_000),
                currency: CurrencyCode::USD,
                filled_full: true,
                missed_previous: false,
                highway_pct: 50,
                towing_pct: 0,
                hauling_pct: 0,
                towed: vec![],
                reported_economy: None,
            }),
            work: None,
            lifecycle: None,
            reading_override: None,
        }
    }

    fn saved(outcome: &EventsOutcome) -> EventId {
        let EventsOutcome::Saved { id, .. } = outcome else {
            panic!("expected a saved event, got {outcome:?}");
        };
        *id
    }

    fn place(name: &str) -> SavedPlaceForm {
        SavedPlaceForm {
            name: name.into(),
            latitude: "35.5".into(),
            longitude: "-82.5".into(),
            radius_m: 150,
            city: "Asheville".into(),
            region: "North Carolina".into(),
            country: "US".into(),
        }
    }

    async fn count(database: &Database, table: &str) -> i64 {
        sqlx::query_scalar(sqlx::AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
            .fetch_one(database.pool())
            .await
            .unwrap()
    }

    #[tokio::test]
    async fn a_desktop_copy_gets_the_households_records_and_reads_the_same() {
        let (_temp, server, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &stuart, &clock).await;
        let id = saved(
            &add(
                &context(&server, &sam, &clock),
                fuel_up(truck, odometer, 50_000),
            )
            .await,
        );
        // Each has a saved place: only Sam's own reaches Sam's copy.
        for (person, name) in [(&stuart, "Stuart's shop"), (&sam, "Sam's house")] {
            let added = server::places::add(&places(&server, person, &clock), &place(name)).await;
            assert!(matches!(added, PlacesOutcome::Added { .. }), "{added:?}");
        }

        let laptop = desktop(&server, "sam").await;
        let (_copy_temp, copy) = copy().await;
        pull_all(&server, &copy, &laptop).await;

        // The server's own read code gives the same answers from the copy.
        let on_server = view(&context(&server, &sam, &clock), id).await;
        let on_copy = view(&context(&copy, &sam, &clock), id).await;
        assert!(
            matches!(on_copy, EventsOutcome::Event { .. }),
            "{on_copy:?}"
        );
        assert_eq!(on_copy, on_server);
        assert_eq!(
            vehicle_page(&context(&copy, &sam, &clock), truck).await,
            vehicle_page(&context(&server, &sam, &clock), truck).await
        );
        let PlacesOutcome::Places { places: mine } =
            server::places::list(&places(&copy, &sam, &clock)).await
        else {
            panic!("no places");
        };
        assert_eq!(mine.len(), 1);
        assert_eq!(mine[0].1.name.as_str(), "Sam's house");
        assert_eq!(count(&copy, "saved_places").await, 1, "not Stuart's");

        // Nothing server-only came.
        for table in [
            "sessions",
            "devices",
            "user_passwords",
            "jobs",
            "applied_commands",
            "stored_files",
        ] {
            assert_eq!(count(&copy, table).await, 0, "{table}");
        }
        let state = copy.pull_state().await.unwrap().unwrap();
        assert_eq!(state.cursor, state.server_sequence);
        assert_eq!(state.last_pull, Some(NOW));
    }

    #[tokio::test]
    async fn an_edit_and_a_delete_reach_the_copy_on_the_next_pull() {
        let (_temp, server, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &stuart, &clock).await;
        let sam_context = context(&server, &sam, &clock);
        let id = saved(&add(&sam_context, fuel_up(truck, odometer, 50_000)).await);
        let laptop = desktop(&server, "sam").await;
        let (_copy_temp, copy) = copy().await;
        pull_all(&server, &copy, &laptop).await;

        saved(&edit(&sam_context, id, fuel_up(truck, odometer, 50_100)).await);
        assert_eq!(
            pull_all(&server, &copy, &laptop).await,
            1,
            "one page of news"
        );
        assert_eq!(
            view(&context(&copy, &sam, &clock), id).await,
            view(&sam_context, id).await,
            "the edit (its reading: event children are rewritten whole)"
        );
        assert_eq!(
            count(&copy, "event_readings").await,
            1,
            "the old reading is gone"
        );

        let deleted = delete(&sam_context, id).await;
        assert!(matches!(deleted, EventsOutcome::Done), "{deleted:?}");
        pull_all(&server, &copy, &laptop).await;
        assert_eq!(
            view(&context(&copy, &sam, &clock), id).await,
            view(&sam_context, id).await
        );
        assert_eq!(
            count(&copy, "tombstones").await,
            1,
            "the tombstone came too"
        );
    }

    #[tokio::test]
    async fn small_pages_end_the_same_as_one_big_one() {
        let (_temp, server, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &stuart, &clock).await;
        let sam_context = context(&server, &sam, &clock);
        for miles in [50_000, 50_300, 50_600] {
            saved(&add(&sam_context, fuel_up(truck, odometer, miles)).await);
        }
        let (_one_temp, one) = copy().await;
        let laptop = desktop(&server, "sam").await;
        assert_eq!(pull_all(&server, &one, &laptop).await, 1);

        // Two change numbers a page, straight from storage.
        let (_small_temp, small) = copy().await;
        let scope = PullScope {
            user: sam.user,
            household: test_id(900),
        };
        let mut pages = 0;
        loop {
            let cursor = small.pull_state().await.unwrap().unwrap().cursor;
            let read = server.read_pull_page(&scope, cursor, 2).await.unwrap();
            let page = PullPage {
                database_id: test_id(900),
                schema: schema_version(),
                cursor,
                next_cursor: read.next_cursor,
                server_sequence: read.server_sequence,
                tables: read.tables,
            };
            small.apply_pull_page(&page, NOW).await.unwrap();
            pages += 1;
            if page.up_to_date() {
                break;
            }
        }
        assert!(pages > 3, "{pages} pages");
        for table in [
            "events",
            "event_readings",
            "fuel_details",
            "assets",
            "meters",
            "users",
            "role_grants",
            "field_clocks",
        ] {
            assert_eq!(
                count(&small, table).await,
                count(&one, table).await,
                "{table}"
            );
        }
        let EventsOutcome::Events { .. } =
            server::events::list(&context(&small, &sam, &clock), truck).await
        else {
            panic!("no events from the small-page copy");
        };
        assert_eq!(
            server::events::list(&context(&small, &sam, &clock), truck).await,
            server::events::list(&sam_context, truck).await
        );
    }

    #[tokio::test]
    async fn only_a_paired_desktop_on_the_same_schema_pulls() {
        let (_temp, server, stuart, _) = setup().await;
        // The common setup signs in without pairing.
        assert_eq!(
            pull(&server, test_id(900), &stuart, 0, schema_version()).await,
            PullOutcome::NotAllowed
        );
        let laptop = desktop(&server, "stuart").await;
        assert_eq!(
            pull(&server, test_id(900), &laptop, 0, schema_version() - 1).await,
            PullOutcome::OtherSchema {
                server: schema_version()
            }
        );
        assert!(matches!(
            pull(&server, test_id(900), &laptop, 0, schema_version()).await,
            PullOutcome::Page { .. }
        ));
    }

    #[tokio::test]
    async fn a_page_from_elsewhere_or_out_of_step_is_refused_and_changes_nothing() {
        let (_temp, server, _, _) = setup().await;
        let laptop = desktop(&server, "stuart").await;
        let PullOutcome::Page { page } =
            pull(&server, test_id(900), &laptop, 0, schema_version()).await
        else {
            panic!("no page");
        };
        let (_copy_temp, copy) = copy().await;
        let other = PullPage {
            database_id: test_id(999),
            ..(*page).clone()
        };
        assert!(matches!(
            copy.apply_pull_page(&other, NOW).await,
            Err(StorageError::OtherServer)
        ));
        let late = PullPage {
            cursor: 1,
            ..(*page).clone()
        };
        assert!(matches!(
            copy.apply_pull_page(&late, NOW).await,
            Err(StorageError::OutOfStep)
        ));
        let mut strange = (*page).clone();
        strange.tables[0].table = "sessions".into();
        assert!(matches!(
            copy.apply_pull_page(&strange, NOW).await,
            Err(StorageError::InvalidValue { .. })
        ));
        assert_eq!(count(&copy, "users").await, 0, "nothing applied");
        assert_eq!(copy.pull_state().await.unwrap().unwrap().cursor, 0);
    }
}
