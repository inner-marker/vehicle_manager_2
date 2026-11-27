//! What's due on the server (Phase 5 milestone 2; decision #082): the Overview's list, the
//! schedule's due, the Garage's badge and order, and the checks on the new settings.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod due {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, DocumentInput, DocumentsOutcome, DueOutcome, DueSubject, EventsOutcome,
        FleetOutcome, InboxOutcome, MarkMessage, ScheduleOutcome,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::{CalendarDate, EventDates};
    use domain::document::{DocumentProblem, DocumentType};
    use domain::due::{DaySteps, DueDefaults, DueStep, FleetCounts, IntervalSteps, LastDoneBefore};
    use domain::event::{EventInput, EventLocation, EventSource, Reading};
    use domain::inbox::{InboxItem, MessageKey, MessageKind, SnoozeRequest};
    use domain::label::Label;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::GarageSort;
    use domain::requirement::{
        Applicability, ComplianceMode, IntervalRule, Requirement, RequirementProblem,
    };
    use domain::test_support::{ManualClock, test_id};
    use domain::time::{Clock, UnixMillis};
    use server::assets::{AssetsContext, add as add_vehicle, garage, view as vehicle_page};
    use server::health::Health;
    use server::sessions::SignedIn;
    use storage::Database;

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

    fn utc() -> TimeZone {
        TimeZone::parse("UTC").unwrap()
    }

    /// A truck (its profile's odometer) and the odometer's ID.
    async fn truck(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
    ) -> (AssetId, MeterId) {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(jordan, ActiveMode::Admin);
        let form = AssetForm {
            name: "Truck".into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id, .. } =
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

    /// A reading of `miles` on the odometer, now.
    fn reading(asset: AssetId, odometer: MeterId, miles: i64) -> EventInput {
        reading_at(asset, odometer, miles, NOW)
    }

    /// A reading of `miles` on the odometer at `at`.
    fn reading_at(asset: AssetId, odometer: MeterId, miles: i64, at: UnixMillis) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Reading,
            dates: EventDates {
                start: at,
                end: None,
                zone: utc(),
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
                reset: false,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: None,
            reading_override: None,
        }
    }

    /// The oil change, every 7,500 mi, last done before the records at 50,000 mi.
    fn oil_change(asset: AssetId, odometer: MeterId) -> Requirement {
        Requirement {
            asset,
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Meter {
                meter: odometer,
                every: 7_500 * MILE,
                first: None,
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: Some(LastDoneBefore {
                on: "2023-06-01".into(),
                readings: vec![Reading {
                    meter: odometer,
                    value: 50_000 * MILE,
                    reset: false,
                }],
            }),
            warnings: None,
            specs: vec![],
            usages: None,
        }
    }

    fn insurance(asset: AssetId, expires: &str) -> DocumentInput {
        DocumentInput {
            asset,
            doc_type: DocumentType::listed("document-insurance").unwrap(),
            number: None,
            issuer: None,
            issued_on: None,
            effective_on: None,
            expires_on: Some(CalendarDate::parse(expires).unwrap()),
            plate: None,
            notes: Notes::default(),
            warn_days: None,
        }
    }

    #[tokio::test]
    async fn the_truck_shows_what_comes_due_everywhere() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let ScheduleOutcome::SavedRequirement { id: oil } =
            server::schedule::add_requirement(&shop, oil_change(truck, odometer)).await
        else {
            panic!("the oil change wasn't added");
        };
        // 56,900 mi: 600 of 7,500 left (8%), due soon.
        let sams = context(&database, &sam, &clock);
        let saved = server::events::add(&sams, reading(truck, odometer, 56_900)).await;
        assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
        // Insurance expiring in 20 days: due soon too, later.
        let added = server::documents::add_document(&shop, insurance(truck, "2023-12-04")).await;
        assert!(
            matches!(added, DocumentsOutcome::SavedDocument { .. }),
            "{added:?}"
        );

        let DueOutcome::Due { entries } = server::due::list(&sams, truck, &utc()).await else {
            panic!("nothing due");
        };
        assert_eq!(entries.len(), 2);
        // Equally urgent: the insurance's date comes first; the oil change has no projected
        // date yet (one reading), so it comes after.
        assert!(matches!(entries[0].subject, DueSubject::Document { .. }));
        assert_eq!(entries[0].due.step, DueStep::DueSoon);
        assert!(matches!(entries[1].subject, DueSubject::Requirement { id, .. } if id == oil));
        assert_eq!(entries[1].due.step, DueStep::DueSoon);
        assert_eq!(entries[1].due.date, None);

        let ScheduleOutcome::Schedule { requirements, .. } =
            server::schedule::view(&sams, truck, &utc()).await
        else {
            panic!("no schedule");
        };
        assert_eq!(
            requirements[0].due.as_ref().map(|due| due.step),
            Some(DueStep::DueSoon)
        );

        let AssetsOutcome::Garage { view } = garage(&sams, &utc()).await else {
            panic!("no garage");
        };
        assert_eq!(view.assets[0].due, Some(DueStep::DueSoon));
        assert_eq!(view.sort, GarageSort::MostUrgent, "the default");
        // A vehicle Sam can't see isn't found.
        assert_eq!(
            server::due::list(&sams, test_id(777), &utc()).await,
            DueOutcome::NotFound
        );
    }

    /// An inspection: first at 3 years, then every 2 years (#090).
    fn inspection(asset: AssetId) -> Requirement {
        Requirement {
            asset,
            name: Label::parse("Inspection").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Calendar {
                every: 2,
                unit: domain::requirement::CalendarUnit::Years,
                end_of_month: false,
                first: Some(3),
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs: vec![],
            usages: None,
        }
    }

    /// The inspection's due date on the truck's list.
    async fn inspection_due(context: &AssetsContext<'_>, truck: AssetId) -> (String, DueStep) {
        let DueOutcome::Due { entries } = server::due::list(context, truck, &utc()).await else {
            panic!("nothing due");
        };
        let due = &entries[0].due;
        (due.date.clone().unwrap().on, due.step)
    }

    #[tokio::test]
    async fn a_first_interval_counts_from_in_service_since_else_acquired() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW); // 14 November 2023
        let (truck, _) = truck(&database, &jordan, &clock).await;
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let admins = context(&database, &admin, &clock);
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let added = server::schedule::add_requirement(&shop, inspection(truck)).await;
        assert!(
            matches!(added, ScheduleOutcome::SavedRequirement { .. }),
            "{added:?}"
        );
        // Nothing known: from the day it was added.
        assert_eq!(
            inspection_due(&shop, truck).await,
            ("2026-11-14".to_owned(), DueStep::Ok)
        );

        // Acquired on 1 March 2023: from then.
        let mut acquired = reading_at(
            truck,
            test_id(1),
            0,
            UnixMillis::from_millis(NOW.as_millis() - 258 * 24 * 60 * 60 * 1_000),
        );
        acquired.kind = EventKind::Acquired;
        acquired.readings.clear();
        acquired.lifecycle = Some(domain::lifecycle::LifecycleDetails::Acquired(
            domain::lifecycle::Acquisition {
                from: None,
                price: None,
            },
        ));
        let saved = server::events::add(&admins, acquired).await;
        assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
        assert_eq!(inspection_due(&shop, truck).await.0, "2026-03-01");

        // A used truck, backdated to an estimated build date: overdue.
        let AssetsOutcome::Asset { view } = vehicle_page(&admins, truck).await else {
            panic!("no truck page");
        };
        let mut form = AssetForm::from_asset(&view.asset);
        form.in_service = "2019-06-12".into();
        form.in_service_estimated = true;
        assert_eq!(
            server::assets::edit(&admins, truck, &form).await,
            AssetsOutcome::Done
        );
        let saved = database.asset(truck).await.unwrap().unwrap().in_service;
        assert_eq!(
            saved.as_ref().map(|since| since.on.as_str()),
            Some("2019-06-12")
        );
        assert!(saved.is_some_and(|since| since.estimated));
        assert_eq!(
            inspection_due(&shop, truck).await,
            ("2022-06-12".to_owned(), DueStep::Overdue)
        );

        // Later than today anywhere on Earth is refused; cleared, Acquired counts again.
        form.in_service = "2023-11-16".into();
        assert_eq!(
            server::assets::edit(&admins, truck, &form).await,
            AssetsOutcome::Invalid {
                problem: domain::asset_form::Problem::Asset(
                    domain::asset::AssetError::BadInService
                )
            }
        );
        form.in_service = String::new();
        assert_eq!(
            server::assets::edit(&admins, truck, &form).await,
            AssetsOutcome::Done
        );
        assert_eq!(
            database.asset(truck).await.unwrap().unwrap().in_service,
            None
        );
        assert_eq!(inspection_due(&shop, truck).await.0, "2026-03-01");
    }

    #[tokio::test]
    async fn due_settings_that_dont_make_sense_are_refused() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let refused = |problem| ScheduleOutcome::Invalid { problem };

        let mut backwards = oil_change(truck, odometer);
        backwards.warnings = Some(DueDefaults {
            interval: IntervalSteps {
                coming_up: 5,
                due_soon: 10,
                very_soon: 20,
            },
            days: DaySteps::default(),
        });
        assert_eq!(
            server::schedule::add_requirement(&shop, backwards).await,
            refused(RequirementProblem::BadWarnings)
        );
        let mut elsewhere = oil_change(truck, odometer);
        if let Some(before) = elsewhere.before_records.as_mut() {
            before.readings[0].meter = test_id(999);
        }
        assert_eq!(
            server::schedule::add_requirement(&shop, elsewhere).await,
            refused(RequirementProblem::BadBeforeRecords)
        );
        let mut not_a_date = oil_change(truck, odometer);
        if let Some(before) = not_a_date.before_records.as_mut() {
            before.on = "June".into();
        }
        assert_eq!(
            server::schedule::add_requirement(&shop, not_a_date).await,
            refused(RequirementProblem::BadBeforeRecords)
        );

        let mut early = insurance(truck, "2023-12-04");
        early.warn_days = Some(DaySteps {
            coming_up: 10,
            due_soon: 30,
            very_soon: 60,
        });
        assert_eq!(
            server::documents::add_document(&shop, early).await,
            DocumentsOutcome::InvalidDocument {
                problem: DocumentProblem::BadWarnDays
            }
        );
    }

    const DAY: i64 = 24 * 60 * 60 * 1_000;

    fn messages_context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> server::messages::MessagesContext<'a> {
        server::messages::MessagesContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            problem: None,
            clock,
            timers: domain::timers::Timers::NORMAL,
        }
    }

    /// The person's inbox, in UTC.
    async fn inbox_of(
        database: &Database,
        user: &SignedIn,
        clock: &ManualClock,
    ) -> domain::inbox::Inbox {
        let outcome =
            server::messages::view(&messages_context(database, user, clock), &utc()).await;
        let InboxOutcome::Inbox { inbox } = outcome else {
            panic!("no inbox: {outcome:?}");
        };
        inbox
    }

    /// The due messages in `items`: their keys, steps, and whether each is read.
    fn due_items(items: &[InboxItem]) -> Vec<(String, DueStep, bool)> {
        items
            .iter()
            .filter_map(|item| {
                let MessageKind::Due(notice) = &item.message.kind else {
                    return None;
                };
                Some((
                    item.message.key.as_str().to_owned(),
                    notice.due.step,
                    item.read,
                ))
            })
            .collect()
    }

    async fn snooze(
        database: &Database,
        user: &SignedIn,
        clock: &ManualClock,
        key: &str,
        days: Option<u16>,
        miles: Option<i64>,
    ) -> InboxOutcome {
        server::messages::mark(
            &messages_context(database, user, clock),
            &utc(),
            MessageKey::parse(key).unwrap(),
            MarkMessage::Snooze {
                request: SnoozeRequest {
                    days,
                    amount: miles.map(|miles| miles * MILE),
                },
            },
        )
        .await
    }

    #[tokio::test]
    async fn a_due_message_snoozes_by_distance_and_comes_back_overdue() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let added = server::schedule::add_requirement(&shop, oil_change(truck, odometer)).await;
        assert!(matches!(added, ScheduleOutcome::SavedRequirement { .. }));
        let sams = context(&database, &sam, &clock);
        let log = |miles: i64, at: UnixMillis| {
            let sams = &sams;
            async move {
                let saved = server::events::add(sams, reading_at(truck, odometer, miles, at)).await;
                assert!(matches!(saved, EventsOutcome::Saved { .. }), "{saved:?}");
            }
        };
        // 600 mi of 7,500 left: due soon, a message for Sam (a Driver on it).
        log(56_900, NOW).await;
        let inbox = inbox_of(&database, &sam, &clock).await;
        let due = due_items(&inbox.items);
        assert_eq!(due.len(), 1, "{due:?}");
        let (soon_key, step, read) = due[0].clone();
        assert_eq!((step, read), (DueStep::DueSoon, false));
        assert!(soon_key.starts_with("due:requirement:"), "{soon_key}");

        // A snooze needs days or an amount.
        assert_eq!(
            snooze(&database, &sam, &clock, &soon_key, None, None).await,
            InboxOutcome::BadSnooze
        );
        // "1 week or 500 mi": asleep, and not unread.
        let InboxOutcome::Inbox { inbox } =
            snooze(&database, &sam, &clock, &soon_key, Some(7), Some(500)).await
        else {
            panic!("not snoozed");
        };
        assert!(due_items(&inbox.items).is_empty());
        assert_eq!(due_items(&inbox.snoozed).len(), 1);
        // 400 mi later (due very soon): still asleep.
        clock.set(UnixMillis::from_millis(NOW.as_millis() + DAY));
        log(57_300, clock.now()).await;
        assert!(due_items(&inbox_of(&database, &sam, &clock).await.items).is_empty());
        // 550 mi: awake, read as before, and more urgent.
        clock.set(UnixMillis::from_millis(NOW.as_millis() + 2 * DAY));
        log(57_450, clock.now()).await;
        let awake = due_items(&inbox_of(&database, &sam, &clock).await.items);
        assert_eq!(awake, vec![(soon_key.clone(), DueStep::DueVerySoon, true)]);

        // Snoozed for a month, then overdue: a new message, unread.
        let snoozed = snooze(&database, &sam, &clock, &soon_key, Some(30), None).await;
        assert!(matches!(snoozed, InboxOutcome::Inbox { .. }), "{snoozed:?}");
        clock.set(UnixMillis::from_millis(NOW.as_millis() + 3 * DAY));
        log(57_600, clock.now()).await;
        let inbox = inbox_of(&database, &sam, &clock).await;
        let overdue = due_items(&inbox.items);
        assert_eq!(overdue.len(), 1, "{overdue:?}");
        assert_ne!(overdue[0].0, soon_key);
        assert_eq!((overdue[0].1, overdue[0].2), (DueStep::Overdue, false));
        assert!(inbox.unread >= 1);
        // Overdue replaces the first message, so its snooze no longer has anything to hide.
        assert!(inbox.snoozed.is_empty());
        let wake = |key: String| {
            let sam = &sam;
            let (database, clock) = (&database, &clock);
            async move {
                server::messages::mark(
                    &messages_context(database, sam, clock),
                    &utc(),
                    MessageKey::parse(&key).unwrap(),
                    MarkMessage::Wake,
                )
                .await
            }
        };
        assert_eq!(wake(soon_key).await, InboxOutcome::NotFound);
        // The overdue one snoozes and wakes like any other.
        let overdue_key = overdue[0].0.clone();
        snooze(&database, &sam, &clock, &overdue_key, Some(1), None).await;
        let InboxOutcome::Inbox { inbox } = wake(overdue_key).await else {
            panic!("not woken");
        };
        assert!(inbox.snoozed.is_empty());
        assert_eq!(due_items(&inbox.items).len(), 1);
    }

    #[tokio::test]
    async fn due_and_squawks_covers_the_vehicles_the_garage_shows() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = truck(&database, &jordan, &clock).await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        server::schedule::add_requirement(&shop, oil_change(truck, odometer)).await;
        // 100 mi over: overdue. Insurance in 20 days: due soon.
        let sams = context(&database, &sam, &clock);
        server::events::add(&sams, reading(truck, odometer, 57_600)).await;
        server::documents::add_document(&shop, insurance(truck, "2023-12-04")).await;
        // A retired car with expired insurance isn't counted.
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let group = database.groups().await.unwrap()[0].0;
        let form = AssetForm {
            name: "Old car".into(),
            status: domain::asset::LifecycleStatus::Retired,
            ..AssetForm::new("car", group)
        };
        let AssetsOutcome::Added { id: car, .. } =
            add_vehicle(&context(&database, &admin, &clock), &form).await
        else {
            panic!("the car wasn't added");
        };
        let expired = server::documents::add_document(&shop, insurance(car, "2023-01-01")).await;
        assert!(
            matches!(expired, DocumentsOutcome::SavedDocument { .. }),
            "{expired:?}"
        );

        let FleetOutcome::Fleet { view } = server::due::fleet(&sams, &utc()).await else {
            panic!("no list");
        };
        let steps: Vec<(String, DueStep)> = view
            .due
            .iter()
            .map(|item| (item.vehicle.clone(), item.entry.due.step))
            .collect();
        assert_eq!(
            steps,
            vec![
                ("Truck".to_owned(), DueStep::Overdue),
                ("Truck".to_owned(), DueStep::DueSoon)
            ]
        );
        let counts = FleetCounts {
            overdue: 1,
            due_soon: 1,
            open_squawks: 0,
        };
        assert_eq!(view.counts, counts);
        let AssetsOutcome::Garage { view } = garage(&sams, &utc()).await else {
            panic!("no garage");
        };
        assert_eq!(view.fleet, counts, "the strip agrees");
        // The retired car's messages don't come either.
        let inbox = inbox_of(&database, &sam, &clock).await;
        assert_eq!(due_items(&inbox.items).len(), 2);
    }
}
