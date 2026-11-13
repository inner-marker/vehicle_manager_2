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
        stuart: SignedIn,
        sam: SignedIn,
        laptop: SignedIn,
        _copy_temp: tempfile::TempDir,
        copy: Database,
        truck: AssetId,
        odometer: MeterId,
        clock: ManualClock,
    }

    async fn scene() -> Scene {
        let (temp, server, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&server, &stuart, &clock).await;
        let laptop = desktop(&server, "sam").await;
        let (copy_temp, copy) = copy().await;
        pull_all(&server, &copy, &laptop).await;
        Scene {
            _temp: temp,
            server,
            stuart,
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
            logged_by: Some(scene.stuart.user),
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
}
