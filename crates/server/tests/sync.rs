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
    use domain::event::EventProblem;
    use domain::event::{EventId, EventInput, EventLocation, EventSource, FuelDetails, Reading};
    use domain::event::{EventPatch, EventStatus};
    use domain::hlc::Hlc;
    use domain::id::DatabaseId;
    use domain::inbox::MessageKind;
    use domain::location::{LocationSource, Place, SavedPlaceForm, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::operation::{Operation, OperationChange, Refusal};
    use domain::preferences::CurrencyCode;
    use domain::sync::{OperationAnswer, PushOutcome, PushStop};
    use domain::sync::{PullOutcome, PullPage};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::user::UserId;
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::events::{add, delete, edit, view};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::login_limiter::LoginLimiter;
    use server::places::PlacesContext;
    use server::sessions::{SignedIn, SigningInDevice, sign_in_from};
    use server::sync::{PushContext, Ran, pull, push, run_operation};
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
            origin: server::origin::Origin::Online,
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
            origin: server::origin::Origin::Online,
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
        jordan: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = SignedIn {
            mode: ActiveMode::Admin,
            ..jordan.clone()
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
        let (_temp, server, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &jordan, &clock).await;
        let id = saved(
            &add(
                &context(&server, &sam, &clock),
                fuel_up(truck, odometer, 50_000),
            )
            .await,
        );
        // Each has a saved place: only Sam's own reaches Sam's copy.
        for (person, name) in [(&jordan, "Jordan's shop"), (&sam, "Sam's house")] {
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
        assert_eq!(count(&copy, "saved_places").await, 1, "not Jordan's");

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
        let (_temp, server, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &jordan, &clock).await;
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
        let (_temp, server, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &jordan, &clock).await;
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
        let (_temp, server, jordan, _) = setup().await;
        // The common setup signs in without pairing.
        assert_eq!(
            pull(&server, test_id(900), &jordan, 0, schema_version()).await,
            PullOutcome::NotAllowed
        );
        let laptop = desktop(&server, "jordan").await;
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
        let laptop = desktop(&server, "jordan").await;
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

    // --- Push (phase-3.md 5c, decision #076) ------------------------------------------------

    const MINUTE: i64 = 60_000;

    fn push_context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> PushContext<'a> {
        PushContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            clock,
            files: None,
        }
    }

    /// An operation made `minutes` after `NOW` on the desktop's clock.
    fn operation(n: u32, minutes: i64, change: OperationChange) -> Operation {
        Operation::new(
            test_id(5_000 + n),
            Hlc::new(
                UnixMillis::from_millis(NOW.as_millis() + minutes * MINUTE),
                0,
            )
            .unwrap(),
            change,
        )
    }

    /// The answers to a push that went to the end.
    fn answers(outcome: PushOutcome) -> Vec<OperationAnswer> {
        let PushOutcome::Answers {
            answers,
            stopped: None,
        } = outcome
        else {
            panic!("the push stopped: {outcome:?}");
        };
        answers
    }

    /// Sam's paired laptop with a pulled copy, and the server's truck.
    struct Scene {
        _temp: tempfile::TempDir,
        server: Database,
        jordan: SignedIn,
        sam: SignedIn,
        laptop: SignedIn,
        _copy_temp: tempfile::TempDir,
        copy: Database,
        truck: AssetId,
        odometer: MeterId,
        clock: ManualClock,
    }

    async fn scene() -> Scene {
        let (temp, server, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &jordan, &clock).await;
        let laptop = desktop(&server, "sam").await;
        let (copy_temp, copy) = copy().await;
        pull_all(&server, &copy, &laptop).await;
        Scene {
            _temp: temp,
            server,
            jordan,
            sam,
            laptop,
            _copy_temp: copy_temp,
            copy,
            truck,
            odometer,
            clock,
        }
    }

    #[tokio::test]
    async fn a_change_made_on_the_copy_reaches_the_server_as_the_same_records_once() {
        let scene = scene().await;
        let logged = operation(
            1,
            1,
            OperationChange::AddEvent {
                input: fuel_up(scene.truck, scene.odometer, 50_000),
            },
        );
        // On the desktop first, with the same server code.
        assert_eq!(
            run_operation(
                &push_context(&scene.copy, &scene.laptop, &scene.clock),
                &logged
            )
            .await,
            Ran::Applied
        );
        let id: EventId = logged.derived_id(0).unwrap();
        assert!(matches!(
            view(&context(&scene.copy, &scene.sam, &scene.clock), id).await,
            EventsOutcome::Event { .. }
        ));

        let pushed = push(
            &push_context(&scene.server, &scene.laptop, &scene.clock),
            schema_version(),
            std::slice::from_ref(&logged),
        )
        .await;
        assert_eq!(
            answers(pushed),
            vec![OperationAnswer::Applied { id: logged.id }]
        );
        // The same event, by the same ID, on the server: the copy and the server agree.
        let on_server = view(&context(&scene.server, &scene.sam, &scene.clock), id).await;
        let EventsOutcome::Event { view: event_view } = &on_server else {
            panic!("not on the server: {on_server:?}");
        };
        assert_eq!(
            event_view.event.last_edited_at,
            logged.hlc.physical(),
            "made when the desktop made it"
        );

        // Sent again (the answer was lost): the same answer, nothing applied twice.
        let again = push(
            &push_context(&scene.server, &scene.laptop, &scene.clock),
            schema_version(),
            std::slice::from_ref(&logged),
        )
        .await;
        assert_eq!(
            answers(again),
            vec![OperationAnswer::Applied { id: logged.id }]
        );
        assert_eq!(count(&scene.server, "events").await, 1);
        assert_eq!(count(&scene.server, "applied_operations").await, 1);
    }

    #[tokio::test]
    async fn a_lower_reading_than_one_logged_meanwhile_is_refused_with_why() {
        let scene = scene().await;
        // On the web, meanwhile: 50,100 miles, a day later than the desktop's.
        let mut later = fuel_up(scene.truck, scene.odometer, 50_100);
        later.dates.start = UnixMillis::from_millis(NOW.as_millis() - 24 * 60 * MINUTE);
        saved(&add(&context(&scene.server, &scene.sam, &scene.clock), later).await);
        let mut earlier_but_higher = fuel_up(scene.truck, scene.odometer, 50_200);
        earlier_but_higher.dates.start =
            UnixMillis::from_millis(NOW.as_millis() - 48 * 60 * MINUTE);
        let logged = operation(
            1,
            1,
            OperationChange::AddEvent {
                input: earlier_but_higher,
            },
        );
        let pushed = answers(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version(),
                std::slice::from_ref(&logged),
            )
            .await,
        );
        let [OperationAnswer::Refused { id, refusal }] = pushed.as_slice() else {
            panic!("expected a refusal: {pushed:?}");
        };
        assert_eq!(*id, logged.id);
        assert!(
            matches!(
                refusal,
                Refusal::Events(EventsOutcome::Invalid {
                    problem: EventProblem::ReadingOutOfOrder(_),
                    ..
                })
            ),
            "{refusal:?}"
        );
        assert_eq!(count(&scene.server, "events").await, 1, "nothing saved");
        // Sent again: refused the same way, from the record.
        let again = answers(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version(),
                std::slice::from_ref(&logged),
            )
            .await,
        );
        assert_eq!(again, pushed);
    }

    #[tokio::test]
    async fn a_change_is_checked_as_the_person_is_now() {
        let scene = scene().await;
        // Sam's role was removed while the change waited.
        let no_role = SignedIn {
            grants: vec![],
            ..scene.laptop.clone()
        };
        let logged = operation(
            1,
            1,
            OperationChange::AddEvent {
                input: fuel_up(scene.truck, scene.odometer, 50_000),
            },
        );
        let pushed = answers(
            push(
                &push_context(&scene.server, &no_role, &scene.clock),
                schema_version(),
                std::slice::from_ref(&logged),
            )
            .await,
        );
        assert!(
            matches!(
                pushed.as_slice(),
                [OperationAnswer::Refused {
                    refusal: Refusal::Events(EventsOutcome::NotFound | EventsOutcome::NotAllowed),
                    ..
                }]
            ),
            "{pushed:?}"
        );
        assert_eq!(count(&scene.server, "events").await, 0);
    }

    #[tokio::test]
    async fn two_devices_changing_different_fields_of_one_event_both_stand() {
        let scene = scene().await;
        let sam_online = context(&scene.server, &scene.sam, &scene.clock);
        let id = saved(&add(&sam_online, fuel_up(scene.truck, scene.odometer, 50_000)).await);
        pull_all(&scene.server, &scene.copy, &scene.laptop).await;

        // Offline on the desktop: only the notes, against its copy.
        let EventsOutcome::Event { view: on_copy } =
            view(&context(&scene.copy, &scene.sam, &scene.clock), id).await
        else {
            panic!("not on the copy");
        };
        let mut typed = fuel_up(scene.truck, scene.odometer, 50_000);
        typed.notes = Notes::parse("Checked the tires").unwrap();
        let changes = typed.changes_from(&on_copy.event);
        let edited = operation(1, 2, OperationChange::EditEvent { id, changes });

        // Meanwhile on the web: the reading.
        scene
            .clock
            .set(UnixMillis::from_millis(NOW.as_millis() + MINUTE));
        saved(
            &edit(
                &sam_online,
                id,
                fuel_up(scene.truck, scene.odometer, 50_050),
            )
            .await,
        );
        scene
            .clock
            .set(UnixMillis::from_millis(NOW.as_millis() + 3 * MINUTE));

        let pushed = answers(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version(),
                std::slice::from_ref(&edited),
            )
            .await,
        );
        assert_eq!(pushed, vec![OperationAnswer::Applied { id: edited.id }]);
        let EventsOutcome::Event { view: now } = view(&sam_online, id).await else {
            panic!("no event");
        };
        assert_eq!(
            now.event.notes.as_str(),
            "Checked the tires",
            "the desktop's"
        );
        assert_eq!(
            now.event.readings[0].value,
            50_050 * MILE,
            "the web's, untouched by the desktop"
        );
    }

    #[tokio::test]
    async fn a_pushed_edit_never_sets_what_the_server_sets() {
        let scene = scene().await;
        let sam_online = context(&scene.server, &scene.sam, &scene.clock);
        let id = saved(&add(&sam_online, fuel_up(scene.truck, scene.odometer, 50_000)).await);
        let changes = EventPatch {
            notes: Some(Notes::parse("Hello").unwrap()),
            status: Some(EventStatus::Finalized),
            logged_by: Some(scene.jordan.user),
            ..EventPatch::default()
        };
        let edited = operation(1, 1, OperationChange::EditEvent { id, changes });
        answers(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version(),
                std::slice::from_ref(&edited),
            )
            .await,
        );
        let EventsOutcome::Event { view: now } = view(&sam_online, id).await else {
            panic!("no event");
        };
        assert_eq!(now.event.notes.as_str(), "Hello");
        assert_eq!(
            now.event.status,
            EventStatus::Draft,
            "not finalized by a patch"
        );
        assert_eq!(now.event.logged_by, scene.sam.user);
    }

    #[tokio::test]
    async fn a_format_the_server_cant_take_stops_the_push_and_keeps_the_rest() {
        let scene = scene().await;
        let log = |n, format| {
            let mut logged = operation(
                n,
                1,
                OperationChange::AddEvent {
                    input: fuel_up(scene.truck, scene.odometer, 50_000 + i64::from(n)),
                },
            );
            logged.format_version = format;
            logged
        };
        let context = push_context(&scene.server, &scene.laptop, &scene.clock);
        assert_eq!(
            push(&context, schema_version(), &[log(1, 0), log(2, 1)]).await,
            PushOutcome::Answers {
                answers: vec![],
                stopped: Some(PushStop::AppTooOld {
                    format: 0,
                    oldest: 1
                }),
            }
        );
        assert_eq!(
            push(&context, schema_version(), &[log(3, 1), log(4, 99)]).await,
            PushOutcome::Answers {
                answers: vec![OperationAnswer::Applied { id: log(3, 1).id }],
                stopped: Some(PushStop::ServerTooOld { format: 99 }),
            }
        );
        assert_eq!(count(&scene.server, "events").await, 1);
    }

    #[tokio::test]
    async fn a_clock_far_ahead_stops_the_push_and_tells_the_admin_until_its_right() {
        let scene = scene().await;
        let context = push_context(&scene.server, &scene.laptop, &scene.clock);
        let ahead = operation(
            1,
            60,
            OperationChange::AddEvent {
                input: fuel_up(scene.truck, scene.odometer, 50_000),
            },
        );
        assert_eq!(
            push(&context, schema_version(), std::slice::from_ref(&ahead)).await,
            PushOutcome::Answers {
                answers: vec![],
                stopped: Some(PushStop::ClockAhead {
                    ahead_millis: 60 * MINUTE
                }),
            }
        );
        let messages = server::devices::clock_ahead_messages(&scene.server)
            .await
            .unwrap();
        assert_eq!(messages.len(), 1);
        assert!(matches!(
            &messages[0].kind,
            MessageKind::ClockAhead { minutes: 60, device_name, .. } if device_name == "Garage Mac"
        ));
        assert_eq!(count(&scene.server, "events").await, 0);

        // Its clock set right: the change goes, and the message with it.
        scene
            .clock
            .set(UnixMillis::from_millis(NOW.as_millis() + 60 * MINUTE));
        answers(push(&context, schema_version(), std::slice::from_ref(&ahead)).await);
        assert!(
            server::devices::clock_ahead_messages(&scene.server)
                .await
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn only_a_paired_desktop_on_the_same_schema_pushes() {
        let scene = scene().await;
        let logged = operation(1, 1, OperationChange::SetGarageOrder { order: vec![] });
        // Sam's browser session (not paired).
        assert_eq!(
            push(
                &push_context(&scene.server, &scene.sam, &scene.clock),
                schema_version(),
                std::slice::from_ref(&logged),
            )
            .await,
            PushOutcome::NotAllowed
        );
        assert_eq!(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version() - 1,
                std::slice::from_ref(&logged),
            )
            .await,
            PushOutcome::OtherSchema {
                server: schema_version()
            }
        );
        assert_eq!(count(&scene.server, "applied_operations").await, 0);
    }

    #[tokio::test]
    async fn a_copy_cleared_after_a_refusal_keeps_its_outbox_and_downloads_again() {
        let scene = scene().await;
        let sam_online = context(&scene.server, &scene.sam, &scene.clock);
        saved(&add(&sam_online, fuel_up(scene.truck, scene.odometer, 50_000)).await);
        pull_all(&scene.server, &scene.copy, &scene.laptop).await;
        let waiting = operation(1, 1, OperationChange::SetGarageOrder { order: vec![] });
        scene.copy.add_to_outbox(&waiting, NOW).await.unwrap();
        assert_eq!(count(&scene.copy, "events").await, 1);

        scene.copy.forget_pulled().await.unwrap();
        for table in [
            "events",
            "assets",
            "users",
            "field_clocks",
            "event_readings",
        ] {
            assert_eq!(count(&scene.copy, table).await, 0, "{table}");
        }
        let state = scene.copy.pull_state().await.unwrap().unwrap();
        assert!(!state.complete(), "screens read from the server meanwhile");
        assert_eq!(state.cursor, 0);
        assert_eq!(
            scene.copy.outbox_waiting(10).await.unwrap(),
            vec![waiting],
            "the outbox is kept"
        );

        pull_all(&scene.server, &scene.copy, &scene.laptop).await;
        assert_eq!(count(&scene.copy, "events").await, 1);
        assert!(scene.copy.pull_state().await.unwrap().unwrap().complete());
    }

    // --- Photos and documents (phase-3.md 6b, decision #077) ----------------------------------

    /// A PNG larger than a thumbnail (so it has its own), as a desktop would upload a photo the
    /// person picked; `red` tells photos apart.
    fn png_of(red: u8) -> Vec<u8> {
        let image = image::ImageBuffer::from_fn(640, 480, |x, _| {
            image::Rgba([red, 30, 30, if x < 320 { 255 } else { 0 }])
        });
        let mut bytes = std::io::Cursor::new(Vec::new());
        image.write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    fn png() -> Vec<u8> {
        png_of(200)
    }

    /// The photo uploaded to the server by `user`, as a push does first.
    async fn uploaded_photo(
        scene: &Scene,
        store: &storage::files::FileStore,
        user: &SignedIn,
    ) -> domain::api::UploadedFile {
        uploaded_photo_of(scene, store, user, png()).await
    }

    /// `bytes` uploaded to the server by `user` as a photo.
    async fn uploaded_photo_of(
        scene: &Scene,
        store: &storage::files::FileStore,
        user: &SignedIn,
        bytes: Vec<u8>,
    ) -> domain::api::UploadedFile {
        let files = storage::files::Files::Folder(store.clone());
        let context = server::attachments::UploadContext {
            database: &scene.server,
            files: &files,
            user,
            health: Health::Healthy,
            now: NOW,
        };
        let domain::api::UploadOutcome::Uploaded { uploaded } = server::attachments::upload(
            &context,
            domain::api::UploadPurpose::Photo {
                size: domain::attachment::PhotoSize::Small,
            },
            bytes,
        )
        .await
        else {
            panic!("no upload");
        };
        uploaded
    }

    /// A fuel-up Sam logged online, to put photos on (a Driver may, on their own events).
    async fn sams_fuel_up(scene: &Scene) -> EventId {
        saved(
            &add(
                &context(&scene.server, &scene.sam, &scene.clock),
                fuel_up(scene.truck, scene.odometer, 50_000),
            )
            .await,
        )
    }

    fn photo_on(
        scene: &Scene,
        event: EventId,
        photo: &domain::api::UploadedFile,
    ) -> domain::api::AttachmentInput {
        domain::api::AttachmentInput {
            asset: scene.truck,
            owner: domain::attachment::AttachmentOwner::Event { event, task: None },
            name: domain::attachment::FileName::from_upload("IMG_0412.png").unwrap(),
            file: photo.file.clone(),
            thumbnail: photo.thumbnail.clone(),
            content: domain::attachment::Content::Photo {
                size: domain::attachment::PhotoSize::Small,
                width: 1,
                height: 1,
                metadata: domain::attachment::PhotoMetadata::default(),
                original: None,
            },
        }
    }

    #[tokio::test]
    async fn a_photo_added_on_a_desktop_is_pushed_after_its_upload_and_saved_once() {
        let scene = scene().await;
        let event = sams_fuel_up(&scene).await;
        let files_temp = tempfile::tempdir().unwrap();
        let store = storage::files::FileStore::open(files_temp.path()).unwrap();
        let files = storage::files::Files::Folder(store.clone());
        let pushing = PushContext {
            files: Some(&files),
            ..push_context(&scene.server, &scene.laptop, &scene.clock)
        };

        // A file never uploaded: refused.
        let mut never = uploaded_photo(&scene, &store, &scene.laptop).await;
        never.file.hash = domain::attachment::FileHash::from_bytes([7; 32]);
        let refused = operation(
            1,
            1,
            OperationChange::AddAttachment {
                input: photo_on(&scene, event, &never),
            },
        );
        assert!(matches!(
            answers(push(&pushing, schema_version(), std::slice::from_ref(&refused)).await)
                .as_slice(),
            [OperationAnswer::Refused {
                refusal: Refusal::Attachments(domain::api::AttachmentsOutcome::Invalid { .. }),
                ..
            }]
        ));

        // Uploaded first, then the operation with the server's files.
        let photo = uploaded_photo(&scene, &store, &scene.laptop).await;
        let added = operation(
            2,
            2,
            OperationChange::AddAttachment {
                input: photo_on(&scene, event, &photo),
            },
        );
        for _ in 0..2 {
            let pushed =
                answers(push(&pushing, schema_version(), std::slice::from_ref(&added)).await);
            assert_eq!(pushed, vec![OperationAnswer::Applied { id: added.id }]);
        }
        let id: domain::attachment::AttachmentId = added.derived_id(0).unwrap();
        let attachment = scene.server.attachment(id).await.unwrap().unwrap();
        assert_eq!(attachment.file, photo.file);
        assert_eq!(
            attachment.added_at,
            added.hlc.physical(),
            "made when the desktop made it"
        );
        assert_eq!(count(&scene.server, "attachments").await, 1, "saved once");

        // Without a file store, adding waits (nothing is refused).
        let elsewhere = operation(
            3,
            3,
            OperationChange::AddAttachment {
                input: photo_on(&scene, event, &photo),
            },
        );
        assert!(matches!(
            push(
                &push_context(&scene.server, &scene.laptop, &scene.clock),
                schema_version(),
                std::slice::from_ref(&elsewhere),
            )
            .await,
            PushOutcome::Answers {
                stopped: Some(PushStop::Unavailable),
                ..
            }
        ));
    }

    #[tokio::test]
    async fn a_photo_removed_on_a_desktop_leaves_an_audit_record_once_its_files_are_erased() {
        let scene = scene().await;
        let event = sams_fuel_up(&scene).await;
        let files_temp = tempfile::tempdir().unwrap();
        let store = storage::files::FileStore::open(files_temp.path()).unwrap();
        let files = storage::files::Files::Folder(store.clone());
        let pushing = PushContext {
            files: Some(&files),
            ..push_context(&scene.server, &scene.laptop, &scene.clock)
        };
        let photo = uploaded_photo(&scene, &store, &scene.laptop).await;
        let added = operation(
            1,
            1,
            OperationChange::AddAttachment {
                input: photo_on(&scene, event, &photo),
            },
        );
        let id: domain::attachment::AttachmentId = added.derived_id(0).unwrap();
        let removed = operation(2, 2, OperationChange::RemoveAttachment { id });
        let pushed = answers(push(&pushing, schema_version(), &[added, removed.clone()]).await);
        assert_eq!(pushed.len(), 2);
        assert_eq!(pushed[1], OperationAnswer::Applied { id: removed.id });
        let using = scene
            .server
            .removed_attachments_using(&photo.file.hash)
            .await
            .unwrap();
        assert_eq!(using.len(), 1);
        assert_eq!(using[0].removed_by, Some(scene.laptop.user));
        assert_eq!(using[0].removed_at, Some(removed.hlc.physical()));

        // The nightly file check, past #065's 8 days: the files go, their record stays.
        let clock = std::sync::Arc::new(ManualClock::new(NOW));
        let jobs = server::jobs::JobContext {
            database: scene.server.clone(),
            clock: std::sync::Arc::clone(&clock)
                as std::sync::Arc<dyn domain::time::Clock + Send + Sync>,
            snapshot_dir: server::jobs::SnapshotDir {
                path: files_temp.path().join("snapshots"),
                create_if_missing: true,
            },
            timers: domain::timers::Timers::NORMAL,
            server_device: test_id(901),
            file_store: Some(store.clone()),
        };
        server::jobs::run_due_jobs(&jobs, NOW).await;
        let later = UnixMillis::from_millis(
            NOW.as_millis()
                + server::jobs::keep_unused_files_for(domain::timers::Timers::NORMAL)
                + 24 * 60 * MINUTE,
        );
        clock.set(later);
        scene
            .server
            .bring_job_forward(server::jobs::Job::FileCheck.name(), later)
            .await
            .unwrap();
        server::jobs::run_due_jobs(&jobs, later).await;
        assert!(!store.path(&photo.file.hash).exists(), "erased");
        let records = scene.server.erased_files().await.unwrap();
        assert_eq!(records.len(), 2, "the photo and its thumbnail: {records:?}");
        let file = records
            .iter()
            .find(|record| record.hash == photo.file.hash)
            .unwrap();
        assert_eq!(file.attachment, id);
        assert_eq!(file.owner, "event");
        assert_eq!(file.owner_id, Some(event.cast()));
        assert_eq!(file.name, "IMG_0412.png");
        assert_eq!(file.removed_by, Some(scene.laptop.user));
        assert_eq!(file.erased_at, later);
    }

    #[tokio::test]
    async fn the_same_photo_processed_in_two_places_makes_the_same_files() {
        // What lets a desktop's processing (the server's own code) name the files the server
        // will make; if it ever didn't, the server's would win (copy_files::send_uploads).
        let scene = scene().await;
        let mut made = Vec::new();
        for _ in 0..2 {
            let temp = tempfile::tempdir().unwrap();
            let store = storage::files::FileStore::open(temp.path()).unwrap();
            made.push(uploaded_photo(&scene, &store, &scene.laptop).await);
        }
        assert_eq!(made[0], made[1]);
    }

    // --- Loss detection (phase-3.md 7a, decision #078) ----------------------------------------

    /// The next page for `scene`'s copy, unapplied.
    async fn next_page(scene: &Scene) -> PullPage {
        let cursor = scene.copy.pull_state().await.unwrap().unwrap().cursor;
        let PullOutcome::Page { page } = pull(
            &scene.server,
            test_id(900),
            &scene.laptop,
            cursor,
            schema_version(),
        )
        .await
        else {
            panic!("no page");
        };
        *page
    }

    #[tokio::test]
    async fn a_page_is_checked_for_another_server_going_back_and_what_it_deletes() {
        use storage::sync::{Deletions, PageCheck};

        let scene = scene().await;
        let sam_online = context(&scene.server, &scene.sam, &scene.clock);
        let id = saved(&add(&sam_online, fuel_up(scene.truck, scene.odometer, 50_000)).await);
        pull_all(&scene.server, &scene.copy, &scene.laptop).await;
        assert_eq!(
            scene
                .copy
                .check_pull_page(&next_page(&scene).await)
                .await
                .unwrap(),
            PageCheck::Fine(Deletions::default()),
            "nothing new"
        );

        // A draft deleted on the server: one record of the truck's.
        assert_eq!(delete(&sam_online, id).await, EventsOutcome::Done);
        let page = next_page(&scene).await;
        assert_eq!(
            scene.copy.check_pull_page(&page).await.unwrap(),
            PageCheck::Fine(Deletions {
                total: 1,
                by_vehicle: vec![(scene.truck, 1)],
            })
        );
        // Applied, it's no longer the copy's to lose.
        scene.copy.apply_pull_page(&page, NOW).await.unwrap();
        assert_eq!(
            scene.copy.check_pull_page(&page).await.unwrap(),
            PageCheck::Fine(Deletions::default())
        );

        // Another server's page; a server gone back in time.
        let mut other = next_page(&scene).await;
        other.database_id = test_id(999);
        assert_eq!(
            scene.copy.check_pull_page(&other).await.unwrap(),
            PageCheck::OtherServer
        );
        let cursor = scene.copy.pull_state().await.unwrap().unwrap().cursor;
        let mut older = next_page(&scene).await;
        older.server_sequence = cursor - 1;
        assert_eq!(
            scene.copy.check_pull_page(&older).await.unwrap(),
            PageCheck::WentBack {
                cursor,
                server_sequence: cursor - 1,
            }
        );
    }

    // --- Rebuild (phase-3.md 7b, decision #078) -----------------------------------------------

    /// Sends `copy`'s records to the rebuild with `token`, in pages, as a desktop does.
    async fn send_pages(
        copy: &Database,
        rebuilt: &Database,
        token: &str,
        from: DatabaseId,
        admin: UserId,
    ) {
        let scope = PullScope {
            user: admin,
            household: from,
        };
        let newest = copy.newest_sequence().await.unwrap();
        let mut cursor = 0;
        loop {
            let read = copy
                .read_page_upto(&scope, cursor, 50, newest)
                .await
                .unwrap();
            let page = PullPage {
                database_id: from,
                schema: schema_version(),
                cursor,
                next_cursor: read.next_cursor,
                server_sequence: read.server_sequence,
                tables: read.tables,
            };
            assert_eq!(
                server::rebuild::page(rebuilt, token, &page).await,
                domain::sync::RebuildOutcome::PageTaken
            );
            if read.next_cursor >= newest {
                return;
            }
            cursor = read.next_cursor;
        }
    }

    /// Two photos on a fuel-up of Sam's, pushed to the server: (their files' store and its
    /// folder, the fuel-up, the photos as uploaded).
    async fn two_photos(
        scene: &Scene,
    ) -> (
        tempfile::TempDir,
        storage::files::FileStore,
        EventId,
        Vec<domain::api::UploadedFile>,
    ) {
        let event = sams_fuel_up(scene).await;
        let files_temp = tempfile::tempdir().unwrap();
        let store = storage::files::FileStore::open(files_temp.path()).unwrap();
        let files = storage::files::Files::Folder(store.clone());
        let pushing = PushContext {
            files: Some(&files),
            ..push_context(&scene.server, &scene.laptop, &scene.clock)
        };
        let mut photos = Vec::new();
        for (n, red) in [(1_u32, 100_u8), (2, 200)] {
            let photo = uploaded_photo_of(scene, &store, &scene.laptop, png_of(red)).await;
            let mut input = photo_on(scene, event, &photo);
            input.name =
                domain::attachment::FileName::from_upload(&format!("photo-{n}.png")).unwrap();
            let added = operation(n, 1, OperationChange::AddAttachment { input });
            answers(push(&pushing, schema_version(), std::slice::from_ref(&added)).await);
            photos.push(photo);
        }
        (files_temp, store, event, photos)
    }

    /// A fresh server (setup open, its default group made), and its setup code.
    async fn fresh_server(scene: &Scene) -> (tempfile::TempDir, Database, String) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(800),
            server_device_id: test_id(801),
        };
        let (rebuilt, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        ensure_default_group(&rebuilt, test_id(801), Health::Healthy, &scene.clock)
            .await
            .unwrap();
        let code = server::setup::open_if_needed(0).unwrap();
        (temp, rebuilt, code)
    }

    /// Rebuilds `rebuilt` from `copy` (Jordan's): begin, pages, finish. Everyone else's new
    /// temporary passwords.
    async fn rebuild_from(
        scene: &Scene,
        copy: &Database,
        rebuilt: &Database,
        code: &str,
        jordan: UserId,
    ) -> Vec<domain::sync::TemporaryPassword> {
        use domain::sync::RebuildOutcome;

        let begin = async |code: &str| {
            server::rebuild::begin(rebuilt, Health::Healthy, &scene.clock, code, test_id(900)).await
        };
        assert_eq!(begin("WRONG-CODE").await, RebuildOutcome::WrongCode);
        let RebuildOutcome::Started { token } = begin(code).await else {
            panic!("not started");
        };
        let stray = PullPage {
            database_id: test_id(999),
            schema: schema_version(),
            cursor: 0,
            next_cursor: 0,
            server_sequence: 0,
            tables: vec![],
        };
        assert_eq!(
            server::rebuild::page(rebuilt, token.expose(), &stray).await,
            RebuildOutcome::NotAllowed,
            "a page from another server"
        );
        send_pages(copy, rebuilt, token.expose(), test_id(900), jordan).await;
        let finish = async |admin: &str| {
            server::rebuild::finish(
                rebuilt,
                test_id(801),
                Health::Healthy,
                &scene.clock,
                token.expose(),
                admin,
                "a new long password".into(),
            )
            .await
        };
        assert_eq!(
            finish("sam").await,
            RebuildOutcome::NotAllowed,
            "only the household's Admin"
        );
        let RebuildOutcome::Finished { passwords } = finish("jordan").await else {
            panic!("not finished");
        };
        passwords
    }

    /// `jordan` (signed in to `rebuilt`) restores the first photo's files from `store`, but not
    /// the second's, and settles: the second is left out.
    async fn restore_first_and_settle(
        scene: &Scene,
        rebuilt: &Database,
        jordan: &SignedIn,
        store: &storage::files::FileStore,
        photos: &[domain::api::UploadedFile],
    ) {
        use domain::sync::RebuildOutcome;

        let restored_temp = tempfile::tempdir().unwrap();
        let restored = storage::files::Files::Folder(
            storage::files::FileStore::open(restored_temp.path()).unwrap(),
        );
        let restoring = server::attachments::UploadContext {
            database: rebuilt,
            files: &restored,
            user: jordan,
            health: Health::Healthy,
            now: NOW,
        };
        let restore = async |bytes: Vec<u8>| {
            server::attachments::upload(&restoring, domain::api::UploadPurpose::Restore, bytes)
                .await
        };
        for file in [&photos[0].file, photos[0].thumbnail.as_ref().unwrap()] {
            assert!(matches!(
                restore(store.get(&file.hash).unwrap()).await,
                domain::api::UploadOutcome::Uploaded { .. }
            ));
        }
        assert_eq!(
            restore(b"anything".to_vec()).await,
            domain::api::UploadOutcome::NotAllowed,
            "only a file the records name"
        );
        let settle = async || server::rebuild::settle(rebuilt, jordan, &scene.clock).await;
        assert_eq!(
            settle().await,
            RebuildOutcome::Settled {
                left_out: vec!["photo-2.png".to_owned()]
            }
        );
        assert_eq!(count(rebuilt, "attachments").await, 1);
        assert_eq!(settle().await, RebuildOutcome::NotAllowed, "once");
    }

    #[tokio::test]
    async fn a_fresh_server_rebuilt_from_the_admins_copy_reads_the_same() {
        use domain::sync::RebuildOutcome;

        let scene = scene().await;
        let (_files_temp, store, event, photos) = two_photos(&scene).await;
        // Jordan's (the Admin's) desktop, its copy complete.
        let jordans = desktop(&scene.server, "jordan").await;
        let (_jordan_temp, copy) = copy().await;
        pull_all(&scene.server, &copy, &jordans).await;
        let (_fresh_temp, rebuilt, code) = fresh_server(&scene).await;
        let passwords = rebuild_from(&scene, &copy, &rebuilt, &code, jordans.user).await;
        assert_eq!(passwords.len(), 1);
        assert_eq!(passwords[0].username, "sam");

        // The rebuilt server reads the same as the old one.
        let old = context(&scene.server, &scene.jordan, &scene.clock);
        let here = AssetsContext {
            database: &rebuilt,
            database_id: test_id(800),
            user: &scene.jordan,
            health: Health::Healthy,
            clock: &scene.clock,
            origin: server::origin::Origin::Online,
        };
        assert_eq!(
            vehicle_page(&here, scene.truck).await,
            vehicle_page(&old, scene.truck).await
        );
        assert_eq!(view(&here, event).await, view(&old, event).await);
        // Sam signs in with the temporary password, to change it.
        let sign_in = async |username: &str, password: domain::secret::Secret| {
            sign_in_from(
                &rebuilt,
                &LoginLimiter::new(),
                username,
                password,
                "laptop",
                NOW,
                SigningInDevice::Browser { name: None },
            )
            .await
            .unwrap()
            .1
        };
        assert!(
            sign_in("sam", passwords[0].password.clone())
                .await
                .must_change_password
        );
        // Its next changes come after everything rebuilt; it isn't fresh any more.
        let newest = rebuilt.newest_sequence().await.unwrap();
        saved(&add(&here, fuel_up(scene.truck, scene.odometer, 50_100)).await);
        assert!(rebuilt.newest_sequence().await.unwrap() > newest);
        assert_eq!(
            server::rebuild::begin(&rebuilt, Health::Healthy, &scene.clock, &code, test_id(900))
                .await,
            RebuildOutcome::WrongCode,
            "setup (and its code) closed once rebuilt"
        );

        // Jordan, signed in, restores the first photo's files; the second's never come.
        let jordan = sign_in("jordan", "a new long password".into()).await;
        restore_first_and_settle(&scene, &rebuilt, &jordan, &store, &photos).await;
    }

    #[tokio::test]
    async fn an_unhealthy_server_takes_no_changes() {
        let scene = scene().await;
        let unwell = PushContext {
            health: Health::Unhealthy(server::health::HealthProblem::IntegrityCheckFailed),
            ..push_context(&scene.server, &scene.laptop, &scene.clock)
        };
        let logged = operation(1, 1, OperationChange::SetGarageOrder { order: vec![] });
        assert_eq!(
            push(&unwell, schema_version(), std::slice::from_ref(&logged)).await,
            PushOutcome::Unhealthy
        );
        assert_eq!(count(&scene.server, "applied_operations").await, 0);
    }

    #[tokio::test]
    async fn a_vehicle_deleted_counts_its_whole_history() {
        use storage::sync::PageCheck;

        let scene = scene().await;
        let sam_online = context(&scene.server, &scene.sam, &scene.clock);
        for miles in [50_000, 50_100, 50_200] {
            let mut logged = fuel_up(scene.truck, scene.odometer, miles);
            logged.dates.start = UnixMillis::from_millis(NOW.as_millis() + miles);
            saved(&add(&sam_online, logged).await);
        }
        pull_all(&scene.server, &scene.copy, &scene.laptop).await;

        let admin = SignedIn {
            mode: ActiveMode::Admin,
            ..scene.jordan.clone()
        };
        assert_eq!(
            server::assets::delete(
                &context(&scene.server, &admin, &scene.clock),
                scene.truck,
                "Truck"
            )
            .await,
            AssetsOutcome::Done
        );
        let PageCheck::Fine(deletions) = scene
            .copy
            .check_pull_page(&next_page(&scene).await)
            .await
            .unwrap()
        else {
            panic!("not this server's page");
        };
        // The truck itself, its three fuel-ups, and its meters (at least the odometer).
        assert!(deletions.total >= 5, "{deletions:?}");
        assert_eq!(deletions.by_vehicle, vec![(scene.truck, deletions.total)]);
    }
}
