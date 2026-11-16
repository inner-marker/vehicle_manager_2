//! Fuel-ups and readings on the server, against a real database through the command
//! pipeline (milestone 6; choices 2, 3, 9, 10).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod events {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, EventView, EventsOutcome, LatestReading};
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::EventDates;
    use domain::event::{
        ClashSide, EventId, EventInput, EventLocation, EventProblem, EventSource, EventStatus,
        Finalizer, Flag, FuelDetails, Reading, ReadingClash, Reason, Requirement, Skipped,
    };
    use domain::event_check::Warning;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::timers::Timers;
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::events::{
        add, correct, delete, edit, finalize_due, finalize_now, list, log_context, view,
    };
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;

    const DAY: i64 = 24 * 60 * 60 * 1000;
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

    fn in_mode(user: &SignedIn, mode: ActiveMode) -> SignedIn {
        SignedIn {
            mode,
            ..user.clone()
        }
    }

    /// A truck (its profile's odometer, required on fuel-ups) and the odometer's ID.
    async fn truck(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(jordan, ActiveMode::Admin);
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

    fn reason(text: &str) -> Reason {
        Reason::parse(text).unwrap()
    }

    /// A fuel-up `days` after `NOW`, at `miles` on the odometer.
    fn fuel_up(asset: AssetId, odometer: MeterId, days: i64, miles: i64) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Fuel,
            dates: EventDates {
                start: UnixMillis::from_millis(NOW.as_millis() + days * DAY),
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
                saved_place: Some(test_id(4444)),
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

    fn saved(outcome: &EventsOutcome) -> (EventId, Vec<Warning>) {
        let EventsOutcome::Saved { id, warnings } = outcome else {
            panic!("expected a saved event, got {outcome:?}");
        };
        (*id, warnings.clone())
    }

    async fn the_event(context: &AssetsContext<'_>, id: EventId) -> EventView {
        let EventsOutcome::Event { view } = view(context, id).await else {
            panic!("no event {id}");
        };
        *view
    }

    fn refused(problem: EventProblem) -> EventsOutcome {
        EventsOutcome::Invalid { problem }
    }

    #[tokio::test]
    async fn a_driver_logs_a_draft_and_its_tidied() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let sam_context = context(&database, &sam, &clock);

        // An override reason with nothing to override, and a skip of something given, are
        // dropped; the saved place isn't Sam's (it doesn't exist), so it's dropped too.
        let mut input = fuel_up(truck, odometer, 0, 50_000);
        input.reading_override = Some(reason("Just in case"));
        input.skipped = vec![Skipped {
            item: Requirement::Meter(odometer),
            reason: reason("Not needed"),
        }];
        let (id, warnings) = saved(&add(&sam_context, input).await);
        assert!(warnings.is_empty());
        let seen = the_event(&sam_context, id).await;
        assert_eq!(seen.event.status, EventStatus::Draft);
        assert_eq!(seen.event.logged_by, sam.user);
        assert_eq!(seen.event.received_at, NOW);
        assert_eq!(seen.event.reading_override, None);
        assert!(seen.event.skipped.is_empty());
        assert_eq!(seen.event.location.saved_place, None);
        assert!(seen.event.flags().is_empty());
        assert!(seen.may_change && !seen.may_correct);
        assert_eq!(seen.logged_by_name, "sam");
        assert_eq!(seen.finalized_by_name, None);

        // A required reading, given or skipped with a reason; a place, or a reason.
        let mut no_odometer = fuel_up(truck, odometer, 1, 50_300);
        no_odometer.readings.clear();
        assert_eq!(
            add(&sam_context, no_odometer.clone()).await,
            refused(EventProblem::Required(Requirement::Meter(odometer)))
        );
        no_odometer.skipped = vec![Skipped {
            item: Requirement::Meter(odometer),
            reason: reason("Cluster is dark"),
        }];
        no_odometer.location.place = None;
        assert_eq!(
            add(&sam_context, no_odometer.clone()).await,
            refused(EventProblem::LocationRequired)
        );
        no_odometer.location.missing_reason = Some(reason("No signal"));
        let (id, _) = saved(&add(&sam_context, no_odometer).await);
        assert_eq!(
            the_event(&sam_context, id).await.event.flags(),
            vec![Flag::MissingLocation, Flag::Skipped]
        );

        let EventsOutcome::Events { events } = list(&sam_context, truck).await else {
            panic!("no list");
        };
        assert_eq!(events.len(), 2, "newest first");
        assert_eq!(
            log_context(&sam_context, truck).await,
            EventsOutcome::LogContext {
                latest: vec![LatestReading {
                    meter: odometer,
                    at: NOW,
                    value: 50_000 * MILE
                }],
                may_override_readings: false,
            }
        );
        assert_eq!(
            list(&sam_context, test_id(777)).await,
            EventsOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn only_the_admin_in_admin_mode_keeps_a_reading_that_doesnt_fit() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let sam_context = context(&database, &sam, &clock);
        saved(&add(&sam_context, fuel_up(truck, odometer, 0, 50_000)).await);
        saved(&add(&sam_context, fuel_up(truck, odometer, 10, 51_000)).await);

        // Back-dated between them: fine if it fits, for anyone.
        saved(&add(&sam_context, fuel_up(truck, odometer, 5, 50_500)).await);
        // Above the later one: refused, even with a reason, except for the Admin in Admin mode.
        let mut wrong = fuel_up(truck, odometer, 6, 52_000);
        wrong.reading_override = Some(reason("Odometer was replaced"));
        let out_of_order = refused(EventProblem::ReadingOutOfOrder(ReadingClash {
            meter: odometer,
            side: ClashSide::AboveLater,
            at: UnixMillis::from_millis(NOW.as_millis() + 10 * DAY),
            value: 51_000 * MILE,
        }));
        assert_eq!(add(&sam_context, wrong.clone()).await, out_of_order);
        let jordan_driver = context(&database, &jordan, &clock);
        assert_eq!(add(&jordan_driver, wrong.clone()).await, out_of_order);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let admin_context = context(&database, &admin, &clock);
        let mut no_reason = wrong.clone();
        no_reason.reading_override = None;
        assert_eq!(add(&admin_context, no_reason).await, out_of_order);

        let (id, warnings) = saved(&add(&admin_context, wrong).await);
        assert_eq!(warnings, vec![Warning::ReadingKept { meter: odometer }]);
        assert_eq!(
            the_event(&admin_context, id).await.event.flags(),
            vec![Flag::ReadingOverride]
        );
        let EventsOutcome::LogContext {
            may_override_readings,
            ..
        } = log_context(&admin_context, truck).await
        else {
            panic!("no log context");
        };
        assert!(may_override_readings);
    }

    #[tokio::test]
    async fn drafts_change_until_finalized_then_only_a_correction_does() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let sam_context = context(&database, &sam, &clock);
        let (id, _) = saved(&add(&sam_context, fuel_up(truck, odometer, 0, 50_000)).await);

        // Sam edits his own draft; Jordan can't in Driver mode, but can as a Maintainer.
        clock.advance(60_000);
        let mut changed = fuel_up(truck, odometer, 0, 50_010);
        changed.notes = Notes::parse("Pump 4").unwrap();
        assert_eq!(saved(&edit(&sam_context, id, changed.clone()).await).0, id);
        let seen = the_event(&sam_context, id).await.event;
        assert_eq!(seen.reading(odometer), Some(50_010 * MILE));
        assert_eq!(seen.last_edited_at.as_millis(), NOW.as_millis() + 60_000);
        assert_eq!(seen.received_at, NOW, "unchanged by an edit");
        let jordan_driver = context(&database, &jordan, &clock);
        assert_eq!(
            edit(&jordan_driver, id, changed.clone()).await,
            EventsOutcome::NotAllowed
        );
        assert!(!the_event(&jordan_driver, id).await.may_change);
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let maintainer_context = context(&database, &maintainer, &clock);
        assert!(the_event(&maintainer_context, id).await.may_change);

        // Finalize now: then nothing changes it, and it can't be deleted.
        assert_eq!(finalize_now(&sam_context, id).await, EventsOutcome::Done);
        let finalized = the_event(&sam_context, id).await;
        assert_eq!(
            finalized.event.finalized.map(|done| done.by),
            Some(Finalizer::User(sam.user))
        );
        assert!(!finalized.may_change && finalized.may_correct);
        assert_eq!(finalized.finalized_by_name.as_deref(), Some("sam"));
        let locked = refused(EventProblem::Finalized);
        assert_eq!(edit(&sam_context, id, changed.clone()).await, locked);
        assert_eq!(finalize_now(&sam_context, id).await, locked);
        assert_eq!(delete(&sam_context, id).await, locked);

        // A correction: a reason, the same kind, and its lower reading counts instead.
        let mut fixed = fuel_up(truck, odometer, 0, 49_990);
        assert_eq!(
            correct(&sam_context, id, fixed.clone(), " ").await,
            refused(EventProblem::ReasonEmpty)
        );
        fixed.kind = EventKind::Reading;
        fixed.fuel = None;
        assert_eq!(
            correct(&sam_context, id, fixed.clone(), "Typo").await,
            refused(EventProblem::WrongKind)
        );
        let fixed = fuel_up(truck, odometer, 0, 49_990);
        let (correction, _) = saved(&correct(&sam_context, id, fixed.clone(), "Typo").await);
        let original = the_event(&sam_context, id).await;
        assert_eq!(original.corrected_by, Some(correction));
        assert!(!original.may_correct);
        let seen = the_event(&sam_context, correction).await;
        assert_eq!(seen.event.corrects, Some(id));
        assert_eq!(seen.event.flags(), vec![Flag::Correction]);
        assert_eq!(
            correct(&sam_context, id, fixed.clone(), "Again").await,
            refused(EventProblem::AlreadyCorrected)
        );
        assert_eq!(
            correct(&sam_context, correction, fixed, "Draft").await,
            refused(EventProblem::NotFinalized)
        );
        let EventsOutcome::LogContext { latest, .. } = log_context(&sam_context, truck).await
        else {
            panic!("no log context");
        };
        assert_eq!(latest[0].value, 49_990 * MILE);

        // A draft can be deleted.
        assert_eq!(delete(&sam_context, correction).await, EventsOutcome::Done);
        assert_eq!(
            view(&sam_context, correction).await,
            EventsOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn the_server_finalizes_drafts_after_24_hours_or_the_test_minutes() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let sam_context = context(&database, &sam, &clock);
        let (first, _) = saved(&add(&sam_context, fuel_up(truck, odometer, 0, 50_000)).await);
        clock.advance(2 * 60_000);
        let (second, _) = saved(&add(&sam_context, fuel_up(truck, odometer, 1, 50_100)).await);
        let finalize =
            |timers| finalize_due(&database, test_id(901), Health::Healthy, &clock, timers);

        // Test timers of 2 minutes: only the first is old enough.
        assert_eq!(finalize(Timers::test(2).unwrap()).await, Ok(1));
        let done = the_event(&sam_context, first).await.event;
        assert_eq!(done.status, EventStatus::Finalized);
        assert_eq!(done.finalized.map(|done| done.by), Some(Finalizer::Server));

        // Normal timers: nothing until 24 hours after the second's last edit.
        clock.advance(DAY - 1);
        assert_eq!(finalize(Timers::NORMAL).await, Ok(0));
        clock.advance(1);
        assert_eq!(finalize(Timers::NORMAL).await, Ok(1));
        assert_eq!(
            the_event(&sam_context, second).await.event.status,
            EventStatus::Finalized
        );
    }
}
