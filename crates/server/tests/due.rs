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
        ScheduleOutcome,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::dates::{CalendarDate, EventDates};
    use domain::document::{DocumentProblem, DocumentType};
    use domain::due::{DaySteps, DueDefaults, DueStep, IntervalSteps, LastDoneBefore};
    use domain::event::{EventInput, EventLocation, EventSource, Reading};
    use domain::label::Label;
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::{EventKind, MeterId};
    use domain::notes::Notes;
    use domain::preferences::GarageSort;
    use domain::requirement::{
        Applicability, ComplianceMode, IntervalRule, Requirement, RequirementProblem,
    };
    use domain::test_support::{ManualClock, test_id};
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

    /// A reading of `miles` on the odometer, now.
    fn reading(asset: AssetId, odometer: MeterId, miles: i64) -> EventInput {
        EventInput {
            asset,
            kind: EventKind::Reading,
            dates: EventDates {
                start: NOW,
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
}
