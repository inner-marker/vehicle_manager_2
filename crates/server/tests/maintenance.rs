//! Squawks, maintenance and repairs, and schedules on the server, against a real database
//! through the command pipeline (milestone 7; choices 1, 2, 4, 8, 10).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod maintenance {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, EventsOutcome, InboxOutcome, ScheduleOutcome, SquawkInput, SquawkView,
        SquawksOutcome,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::EventDates;
    use domain::event::{
        EventId, EventInput, EventLocation, EventProblem, EventSource, Reading, ReadingSet, Reason,
        Task, WorkDetails,
    };
    use domain::inbox::MessageKind;
    use domain::label::Label;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::CurrencyCode;
    use domain::requirement::{
        Bundle, BundleId, IntervalRule, Requirement, RequirementId, RequirementProblem,
    };
    use domain::squawk::{
        Disposition, Severity, SquawkDisposition, SquawkId, SquawkProblem, SquawkStatus, SquawkText,
    };
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::timers::Timers;
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::events::{add, correct, edit, finalize_now, finish};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::messages::{MessagesContext, view as inbox_view};
    use server::schedule;
    use server::sessions::SignedIn;
    use server::squawks;
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
        }
    }

    fn in_mode(user: &SignedIn, mode: ActiveMode) -> SignedIn {
        SignedIn {
            mode,
            ..user.clone()
        }
    }

    fn at(days: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + days * DAY)
    }

    /// A truck named `name` and its odometer's ID.
    async fn new_truck(
        database: &Database,
        stuart: &SignedIn,
        clock: &ManualClock,
        name: &str,
    ) -> (AssetId, MeterId) {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(stuart, ActiveMode::Admin);
        let form = AssetForm {
            name: name.into(),
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

    fn squawk_input(asset: AssetId, description: &str) -> SquawkInput {
        SquawkInput {
            asset,
            description: SquawkText::parse(description).unwrap(),
            notes: Notes::default(),
            severity: Severity::Low,
            opened_at: NOW,
            found_on: None,
        }
    }

    async fn open_squawk(context: &AssetsContext<'_>, asset: AssetId, text: &str) -> SquawkId {
        let SquawksOutcome::Saved { id } = squawks::open(context, squawk_input(asset, text)).await
        else {
            panic!("the squawk wasn't opened");
        };
        id
    }

    async fn squawk_of(context: &AssetsContext<'_>, id: SquawkId) -> SquawkView {
        let SquawksOutcome::Squawk { view } = squawks::view(context, id).await else {
            panic!("no squawk {id}");
        };
        *view
    }

    /// An oil change `days` after `NOW` at `miles`, doing `requirement` and dispositioning
    /// `dispositions`.
    fn oil_change(
        asset: AssetId,
        odometer: MeterId,
        days: i64,
        miles: i64,
        requirement: Option<RequirementId>,
        dispositions: Vec<SquawkDisposition>,
    ) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Maintenance,
            dates: EventDates {
                start: at(days),
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
            fuel: None,
            work: Some(WorkDetails {
                tasks: vec![Task {
                    title: Label::parse("Engine oil & filter").unwrap(),
                    requirement,
                    notes: Notes::default(),
                    parts: vec![],
                    labor_seconds: None,
                    labor_cost_minor: None,
                    resets_from: ReadingSet::Start,
                    next_due: None,
                }],
                end_readings: vec![],
                dispositions,
                currency: CurrencyCode::USD,
                complaint: None,
                cause: None,
            }),
            reading_override: None,
        }
    }

    fn deferred(squawk: SquawkId) -> SquawkDisposition {
        SquawkDisposition {
            squawk,
            action: Disposition::Deferred {
                reason: reason("Parts on order"),
            },
            severity: Some(Severity::Medium),
        }
    }

    fn signed_off(squawk: SquawkId) -> SquawkDisposition {
        SquawkDisposition {
            squawk,
            action: Disposition::SignedOff {
                notes: Notes::default(),
                task: Some(0),
            },
            severity: None,
        }
    }

    fn saved(outcome: &EventsOutcome) -> EventId {
        let EventsOutcome::Saved { id, .. } = outcome else {
            panic!("expected a saved event, got {outcome:?}");
        };
        *id
    }

    fn refused(problem: EventProblem) -> EventsOutcome {
        EventsOutcome::Invalid { problem }
    }

    fn oil_requirement(asset: AssetId, odometer: MeterId) -> Requirement {
        Requirement {
            asset,
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![
                IntervalRule::Meter {
                    meter: odometer,
                    every: 5_000 * MILE,
                },
                IntervalRule::Calendar {
                    every: 6,
                    unit: domain::requirement::CalendarUnit::Months,
                    end_of_month: false,
                },
            ],
        }
    }

    async fn add_requirement(
        context: &AssetsContext<'_>,
        requirement: Requirement,
    ) -> RequirementId {
        let ScheduleOutcome::SavedRequirement { id } =
            schedule::add_requirement(context, requirement).await
        else {
            panic!("the requirement wasn't added");
        };
        id
    }

    #[tokio::test]
    async fn a_driver_opens_squawks_and_may_change_them_until_a_job_deals_with_them() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock, "Truck").await;
        let driver = context(&database, &sam, &clock);
        let brake = open_squawk(&driver, truck, "Brake light out").await;

        let SquawksOutcome::Squawks {
            squawks: list,
            may_open,
        } = squawks::list(&driver, truck).await
        else {
            panic!("no squawks");
        };
        assert!(may_open);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].status, SquawkStatus::Open);
        assert_eq!(list[0].severity, Severity::Low, "new squawks start at Low");
        assert_eq!(list[0].opened_by_name, "sam");
        assert!(list[0].may_change, "its opener may change it");

        let mut worse = squawk_input(truck, "Both brake lights out");
        worse.severity = Severity::High;
        assert_eq!(
            squawks::edit(&driver, brake, worse.clone()).await,
            SquawksOutcome::Saved { id: brake }
        );
        let view = squawk_of(&driver, brake).await;
        assert_eq!(view.severity, Severity::High);
        assert_eq!(view.squawk.description.as_str(), "Both brake lights out");

        // A Driver doesn't log maintenance; nor can a squawk be found on another vehicle's
        // event.
        let job = oil_change(truck, odometer, 1, 80_000, None, vec![deferred(brake)]);
        assert_eq!(add(&driver, job.clone()).await, EventsOutcome::NotAllowed);
        let (other, _) = new_truck(&database, &stuart, &clock, "Van").await;
        let mut elsewhere = squawk_input(other, "Rattle");
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let job_id = saved(&add(&shop, job).await);
        elsewhere.found_on = Some(job_id);
        assert_eq!(
            squawks::open(&driver, elsewhere).await,
            SquawksOutcome::NotFound
        );

        // A draft job naming it is enough to freeze it.
        assert!(!squawk_of(&driver, brake).await.may_change);
        let frozen = SquawksOutcome::Invalid {
            problem: SquawkProblem::Dispositioned,
        };
        assert_eq!(squawks::edit(&driver, brake, worse).await, frozen);
        assert_eq!(squawks::delete(&shop, brake).await, frozen);

        // One no job named can go.
        let typo = open_squawk(&driver, truck, "Typo").await;
        assert_eq!(squawks::delete(&driver, typo).await, SquawksOutcome::Done);
        assert_eq!(squawks::view(&driver, typo).await, SquawksOutcome::NotFound);
    }

    #[tokio::test]
    async fn finish_needs_every_outstanding_squawk_and_deferrals_dont_carry_over() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock, "Truck").await;
        let driver = context(&database, &sam, &clock);
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let brake = open_squawk(&driver, truck, "Brake light out").await;
        let oil = add_requirement(&shop, oil_requirement(truck, odometer)).await;

        // Saved without the disposition, but Finish refuses it, and Finalize now is for
        // fuel-ups and readings.
        let job = saved(
            &add(
                &shop,
                oil_change(truck, odometer, 1, 80_000, Some(oil), vec![]),
            )
            .await,
        );
        assert_eq!(
            finish(&shop, job).await,
            refused(EventProblem::Squawk(SquawkProblem::Undispositioned(brake)))
        );
        assert_eq!(
            finalize_now(&shop, job).await,
            refused(EventProblem::NotAJob)
        );

        // Deferred: finished, and the squawk is Medium and still outstanding.
        let input = oil_change(truck, odometer, 1, 80_000, Some(oil), vec![deferred(brake)]);
        saved(&edit(&shop, job, input).await);
        assert_eq!(finish(&shop, job).await, EventsOutcome::Done);
        let view = squawk_of(&driver, brake).await;
        assert_eq!(view.status, SquawkStatus::Deferred);
        assert_eq!(view.severity, Severity::Medium);

        // The next job must deal with it again; it signs it off.
        let next = oil_change(truck, odometer, 30, 85_000, Some(oil), vec![]);
        let next = saved(&add(&shop, next).await);
        assert_eq!(
            finish(&shop, next).await,
            refused(EventProblem::Squawk(SquawkProblem::Undispositioned(brake)))
        );
        let input = oil_change(
            truck,
            odometer,
            30,
            85_000,
            Some(oil),
            vec![signed_off(brake)],
        );
        saved(&edit(&shop, next, input).await);
        assert_eq!(finish(&shop, next).await, EventsOutcome::Done);
        assert_eq!(
            squawk_of(&driver, brake).await.status,
            SquawkStatus::Resolved
        );

        // A signed-off squawk can't be dispositioned again, except by correcting the job that
        // signed it off.
        let again = oil_change(truck, odometer, 40, 86_000, None, vec![deferred(brake)]);
        assert_eq!(
            add(&shop, again).await,
            refused(EventProblem::Squawk(SquawkProblem::NotOutstanding(brake)))
        );
        let corrected = oil_change(
            truck,
            odometer,
            30,
            85_000,
            Some(oil),
            vec![deferred(brake)],
        );
        saved(&correct(&shop, next, corrected, "Not fixed after all").await);

        // Last done is the latest finished task.
        let ScheduleOutcome::Schedule { requirements, .. } = schedule::view(&driver, truck).await
        else {
            panic!("no schedule");
        };
        let last = requirements[0].last_done.as_ref().unwrap();
        assert_eq!((last.event, last.at), (next, at(30)));
    }

    /// Oil (by distance or six months) and tires (when needed) on `truck`, and a bundle of
    /// both: their IDs.
    async fn oil_tires_and_service(
        shop: &AssetsContext<'_>,
        truck: AssetId,
        odometer: MeterId,
    ) -> (RequirementId, RequirementId, BundleId) {
        let oil = add_requirement(shop, oil_requirement(truck, odometer)).await;
        let mut tires = oil_requirement(truck, odometer);
        tires.name = Label::parse("Rotate tires").unwrap();
        tires.rules = vec![];
        let tires = add_requirement(shop, tires).await;
        let ScheduleOutcome::SavedBundle { id: service } = schedule::add_bundle(
            shop,
            Bundle {
                asset: truck,
                name: Label::parse("5,000-mile service").unwrap(),
                requirements: vec![oil, tires],
            },
        )
        .await
        else {
            panic!("the bundle wasn't added");
        };
        (oil, tires, service)
    }

    #[tokio::test]
    async fn a_maintainer_sets_up_a_schedule() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock, "Truck").await;
        let driver = context(&database, &sam, &clock);
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);

        assert_eq!(
            schedule::add_requirement(&driver, oil_requirement(truck, odometer)).await,
            ScheduleOutcome::NotAllowed
        );
        let mut elsewhere = oil_requirement(truck, odometer);
        elsewhere.rules = vec![IntervalRule::Meter {
            meter: test_id(7777),
            every: MILE,
        }];
        assert_eq!(
            schedule::add_requirement(&shop, elsewhere).await,
            ScheduleOutcome::Invalid {
                problem: RequirementProblem::UnknownMeter
            }
        );
        let (oil, tires, service) = oil_tires_and_service(&shop, truck, odometer).await;

        // A requirement a task did stays (turn it off instead); one no task did goes, and
        // out of its bundle with it.
        saved(
            &add(
                &shop,
                oil_change(truck, odometer, 1, 80_000, Some(oil), vec![]),
            )
            .await,
        );
        assert_eq!(
            schedule::delete_requirement(&shop, oil).await,
            ScheduleOutcome::Invalid {
                problem: RequirementProblem::InUse
            }
        );
        assert_eq!(
            schedule::delete_requirement(&shop, tires).await,
            ScheduleOutcome::Done
        );
        let ScheduleOutcome::Schedule { bundles, .. } = schedule::view(&shop, truck).await else {
            panic!("no schedule");
        };
        assert_eq!(
            bundles,
            vec![(
                service,
                Bundle {
                    asset: truck,
                    name: Label::parse("5,000-mile service").unwrap(),
                    requirements: vec![oil],
                }
            )]
        );
    }

    #[tokio::test]
    async fn a_schedule_is_copied_onto_a_vehicle_without_one() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock, "Truck").await;
        let (van, _) = new_truck(&database, &stuart, &clock, "Van").await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let (oil, _, _) = oil_tires_and_service(&shop, truck, odometer).await;

        // Copied onto the van, its odometer matched; never twice.
        let ScheduleOutcome::Copied {
            requirements,
            bundles,
            left_out,
        } = schedule::copy(&shop, truck, van).await
        else {
            panic!("the schedule wasn't copied");
        };
        assert_eq!((requirements, bundles, left_out.len()), (2, 1, 0));
        assert_eq!(
            schedule::copy(&shop, truck, van).await,
            ScheduleOutcome::Invalid {
                problem: RequirementProblem::ScheduleNotEmpty
            }
        );
        let ScheduleOutcome::Schedule {
            requirements: copied,
            bundles,
            may_configure,
        } = schedule::view(&shop, van).await
        else {
            panic!("no van schedule");
        };
        assert!(may_configure);
        let copied_ids: Vec<RequirementId> = copied.iter().map(|view| view.id).collect();
        assert!(!copied_ids.contains(&oil), "copies, never links");
        assert_eq!(bundles[0].1.requirements.len(), 2);
        assert!(
            bundles[0]
                .1
                .requirements
                .iter()
                .all(|id| copied_ids.contains(id))
        );
    }

    /// What's in `user`'s inbox at `clock`'s time.
    async fn messages(
        database: &Database,
        user: &SignedIn,
        clock: &ManualClock,
        timers: Timers,
    ) -> Vec<MessageKind> {
        let context = MessagesContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            problem: None,
            clock,
            timers,
        };
        let InboxOutcome::Inbox { inbox } = inbox_view(&context).await else {
            panic!("no inbox");
        };
        inbox
            .items
            .into_iter()
            .map(|item| item.message.kind)
            .collect()
    }

    #[tokio::test]
    async fn a_forgotten_job_reminds_its_logger_and_the_maintainers() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = new_truck(&database, &stuart, &clock, "Truck").await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let job = saved(
            &add(
                &context(&database, &maintainer, &clock),
                oil_change(truck, odometer, 0, 80_000, None, vec![]),
            )
            .await,
        );
        let forgotten = vec![MessageKind::ForgottenJob {
            event: job,
            vehicle: "Truck".into(),
        }];

        clock.set(at(13));
        assert_eq!(
            messages(&database, &maintainer, &clock, Timers::NORMAL).await,
            vec![]
        );
        clock.set(at(14));
        assert_eq!(
            messages(&database, &maintainer, &clock, Timers::NORMAL).await,
            forgotten
        );
        assert_eq!(
            messages(&database, &sam, &clock, Timers::NORMAL).await,
            vec![],
            "a Driver who didn't log it isn't reminded"
        );
        // With test timers, minutes instead of days.
        let quick = ManualClock::new(UnixMillis::from_millis(NOW.as_millis() + 2 * 60_000));
        assert_eq!(
            messages(&database, &maintainer, &quick, Timers::test(2).unwrap()).await,
            forgotten
        );
    }
}
