//! Lifecycle events and reference documents on the server, against a real database through the
//! command pipeline (milestone 9; decisions #067, #068).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod lifecycle {
    use crate::common::{NOW, setup};
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant};
    use domain::api::{
        AssetView, AssetsOutcome, DocumentInput, DocumentsOutcome, EventsOutcome, InboxOutcome,
        ReferenceInput, ReferenceLink, ReferencesOutcome, ScheduleOutcome, SquawkInput,
        SquawksOutcome,
    };
    use domain::asset::{AssetId, LifecycleStatus};
    use domain::asset_form::AssetForm;
    use domain::dates::{CalendarDate, EventDates};
    use domain::document::{DocumentType, REGISTRATION};
    use domain::event::{EventId, EventInput, EventLocation, EventProblem, EventSource};
    use domain::inbox::MessageKind;
    use domain::label::Label;
    use domain::lifecycle::{Disposal, DisposalMethod, LifecycleDetails, Party, Return, SendOut};
    use domain::location::{LocationSource, Place, TimeZone};
    use domain::meter::EventKind;
    use domain::notes::Notes;
    use domain::reference::{RECALL, ReferenceType};
    use domain::requirement::{Applicability, ComplianceMode, Requirement};
    use domain::squawk::{Severity, SquawkText};
    use domain::test_support::{ManualClock, test_id};
    use domain::time::UnixMillis;
    use domain::timers::Timers;
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::events::{add, correct, finalize_now};
    use server::groups::ensure_default_group;
    use server::health::Health;
    use server::messages::{MessagesContext, view as inbox_view};
    use server::sessions::SignedIn;
    use storage::Database;

    const DAY: i64 = 24 * 60 * 60 * 1000;

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

    /// A vehicle named `name`, added by Stuart.
    async fn vehicle(
        database: &Database,
        stuart: &SignedIn,
        clock: &ManualClock,
        name: &str,
    ) -> AssetId {
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
            panic!("{name} wasn't added");
        };
        id
    }

    async fn page(context: &AssetsContext<'_>, id: AssetId) -> AssetView {
        let AssetsOutcome::Asset { view } = vehicle_page(context, id).await else {
            panic!("no page");
        };
        *view
    }

    /// A lifecycle event on `asset`, `days` after `NOW`.
    fn input(asset: AssetId, days: i64, details: LifecycleDetails) -> EventInput {
        EventInput {
            asset,
            kind: details.kind(),
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
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: Some(details),
            reading_override: None,
        }
    }

    /// Sent to Joe's Auto, expected back on `expected` (NOW is 14 November 2023).
    fn send_out(expected: &str) -> LifecycleDetails {
        LifecycleDetails::SentOut(SendOut {
            shop: Party {
                name: "Joe's Auto".into(),
                contact: None,
            },
            work_order: Some("WO-1182".into()),
            expected_back: Some(CalendarDate::parse(expected).unwrap()),
            carrier: None,
            tracking: None,
            estimate: None,
        })
    }

    fn sold(withdrawn: bool) -> LifecycleDetails {
        LifecycleDetails::Disposed(Disposal {
            method: DisposalMethod::Sold,
            to: None,
            price: None,
            withdrawn,
        })
    }

    fn saved(outcome: &EventsOutcome) -> EventId {
        let EventsOutcome::Saved { id, .. } = outcome else {
            panic!("not saved: {outcome:?}");
        };
        *id
    }

    #[tokio::test]
    async fn a_send_out_and_its_return_set_the_status_for_maintainers_only() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &stuart, &clock, "Truck").await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let maintain = context(&database, &maintainer, &clock);
        let driver = context(&database, &sam, &clock);

        assert_eq!(
            add(&driver, input(truck, 0, send_out("2023-11-20"))).await,
            EventsOutcome::NotAllowed,
            "a Driver doesn't send vehicles out"
        );
        let sent = saved(&add(&maintain, input(truck, 0, send_out("2023-11-20"))).await);
        let view = page(&maintain, truck).await;
        assert_eq!(view.standing.status, LifecycleStatus::OutForService);
        assert_eq!(
            view.asset.status,
            LifecycleStatus::InService,
            "Setup's is kept"
        );
        assert_eq!(view.standing.sent_out, Some(sent));
        assert_eq!(
            add(&maintain, input(truck, 1, send_out("2023-11-21"))).await,
            EventsOutcome::Invalid {
                problem: EventProblem::AlreadyOut
            }
        );

        let back = LifecycleDetails::Returned(Return {
            sent_out: sent,
            cost: None,
        });
        assert_eq!(
            add(&maintain, input(truck, -1, back.clone())).await,
            EventsOutcome::Invalid {
                problem: EventProblem::ReturnBeforeSendOut
            }
        );
        saved(&add(&maintain, input(truck, 3, back)).await);
        let view = page(&maintain, truck).await;
        assert_eq!(view.standing.status, LifecycleStatus::InService);
        assert_eq!(view.standing.sent_out, None);
    }

    fn messages_context<'a>(
        database: &'a Database,
        user: &'a SignedIn,
        clock: &'a ManualClock,
    ) -> MessagesContext<'a> {
        MessagesContext {
            database,
            database_id: test_id(900),
            user,
            health: Health::Healthy,
            problem: None,
            clock,
            timers: Timers::NORMAL,
        }
    }

    async fn overdue(database: &Database, user: &SignedIn, clock: &ManualClock) -> usize {
        let InboxOutcome::Inbox { inbox } =
            inbox_view(&messages_context(database, user, clock)).await
        else {
            panic!("no inbox");
        };
        inbox
            .items
            .iter()
            .filter(|item| matches!(item.message.kind, MessageKind::OverdueReturn { .. }))
            .count()
    }

    #[tokio::test]
    async fn an_overdue_return_is_a_message_from_the_day_after_until_it_is_back() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &stuart, &clock, "Truck").await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let maintain = context(&database, &maintainer, &clock);
        let sent = saved(&add(&maintain, input(truck, 0, send_out("2023-11-20"))).await);

        // 20 November, noon in Asheville: due today, not overdue yet.
        clock.set(UnixMillis::from_millis(1_700_499_600_000));
        assert_eq!(overdue(&database, &maintainer, &clock).await, 0);
        // 21 November: overdue, for Stuart (he sent it), not for Sam (a Driver).
        clock.set(UnixMillis::from_millis(1_700_586_000_000));
        assert_eq!(overdue(&database, &maintainer, &clock).await, 1);
        assert_eq!(overdue(&database, &sam, &clock).await, 0);

        let back = LifecycleDetails::Returned(Return {
            sent_out: sent,
            cost: None,
        });
        saved(&add(&maintain, input(truck, 8, back)).await);
        assert_eq!(overdue(&database, &maintainer, &clock).await, 0);
    }

    #[tokio::test]
    async fn a_finalized_disposal_takes_corrections_but_nothing_new() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &stuart, &clock, "Truck").await;
        let admin = in_mode(&stuart, ActiveMode::Admin);
        let own = context(&database, &admin, &clock);
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        assert_eq!(
            add(
                &context(&database, &maintainer, &clock),
                input(truck, 0, sold(false))
            )
            .await,
            EventsOutcome::NotAllowed,
            "disposing of a vehicle is the Admin's"
        );
        assert!(matches!(
            add(&own, input(truck, 0, sold(true))).await,
            EventsOutcome::Invalid {
                problem: EventProblem::WithdrawnNotCorrection
            }
        ));
        let disposal = saved(&add(&own, input(truck, 0, sold(false))).await);
        let view = page(&own, truck).await;
        assert_eq!(view.standing.status, LifecycleStatus::Disposed);
        assert!(
            view.standing.takes_new_records,
            "a draft can still be deleted"
        );

        assert_eq!(finalize_now(&own, disposal).await, EventsOutcome::Done);
        let view = page(&own, truck).await;
        assert!(!view.standing.takes_new_records);
        let driver = context(&database, &sam, &clock);
        let squawk = SquawkInput {
            asset: truck,
            description: SquawkText::parse("Rattle").unwrap(),
            notes: Notes::default(),
            severity: Severity::Low,
            opened_at: NOW,
            found_on: None,
        };
        assert_eq!(
            server::squawks::open(&driver, squawk).await,
            SquawksOutcome::NotAllowed
        );
        let document = DocumentInput {
            asset: truck,
            doc_type: DocumentType::listed(REGISTRATION).unwrap(),
            number: None,
            issuer: None,
            issued_on: None,
            effective_on: None,
            expires_on: None,
            plate: None,
            notes: Notes::default(),
        };
        assert_eq!(
            server::documents::add_document(&context(&database, &maintainer, &clock), document)
                .await,
            DocumentsOutcome::NotAllowed
        );
        assert_eq!(
            add(&own, input(truck, 1, send_out("2023-11-20"))).await,
            EventsOutcome::Invalid {
                problem: EventProblem::VehicleDisposed
            }
        );

        // A correction still goes through, and withdrawing the disposal brings it back.
        saved(
            &correct(
                &own,
                disposal,
                input(truck, 0, sold(true)),
                "Sale fell through",
            )
            .await,
        );
        let view = page(&own, truck).await;
        assert_eq!(view.standing.status, LifecycleStatus::InService);
        assert!(view.standing.takes_new_records);
    }

    fn recall() -> ReferenceInput {
        ReferenceInput {
            doc_type: ReferenceType::listed(RECALL).unwrap(),
            authority: Some("NHTSA".into()),
            numbers: vec![],
            title: "Rear axle nut may loosen".into(),
            description: Notes::default(),
            remedy: Notes::default(),
            issued_on: None,
            effective_on: None,
            revision: None,
            supersedes: None,
            applicability: Notes::default(),
            url: Some("https://www.nhtsa.gov/recalls".into()),
        }
    }

    fn link(asset: AssetId) -> ReferenceLink {
        ReferenceLink {
            asset,
            name: Label::parse("Axle nut recall").unwrap(),
            mode: ComplianceMode::OneTime,
            applicability: Applicability::Applies,
        }
    }

    /// Links reference document `id` to `asset` through its schedule (ruled out there).
    async fn link_through_schedule(
        context: &AssetsContext<'_>,
        asset: AssetId,
        id: domain::reference::ReferenceDocumentId,
    ) {
        let requirement = Requirement {
            asset,
            name: Label::parse("Axle nut recall").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![],
            reference: Some(id),
            mode: ComplianceMode::OneTime,
            applicability: Applicability::NotApplicable {
                reason: domain::event::Reason::parse("Built after the VIN range").unwrap(),
            },
        };
        assert!(matches!(
            server::schedule::add_requirement(context, requirement).await,
            ScheduleOutcome::SavedRequirement { .. }
        ));
    }

    #[tokio::test]
    async fn a_recall_is_shared_and_changed_only_by_a_maintainer_of_every_vehicle_linking_it() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &stuart, &clock, "Truck").await;
        let van = vehicle(&database, &stuart, &clock, "Van").await;
        // Pat maintains the truck and only views the van.
        let grant = |asset, role| RoleGrant {
            user: sam.user,
            target: GrantTarget::Asset(asset),
            role,
        };
        let pat = SignedIn {
            grants: vec![grant(truck, Role::Maintainer), grant(van, Role::Viewer)],
            mode: ActiveMode::Maintainer,
            ..sam.clone()
        };
        let as_pat = context(&database, &pat, &clock);
        assert_eq!(
            server::references::add(&as_pat, recall(), link(van)).await,
            ReferencesOutcome::NotAllowed,
            "only from a vehicle they maintain"
        );
        let ReferencesOutcome::Saved {
            id,
            requirement: Some(_),
        } = server::references::add(&as_pat, recall(), link(truck)).await
        else {
            panic!("the recall wasn't added");
        };
        let ReferencesOutcome::Reference { view } = server::references::view(&as_pat, id).await
        else {
            panic!("no recall");
        };
        assert!(view.may_change, "only the truck links it");

        // Stuart links the van too, through its schedule.
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let maintain = context(&database, &maintainer, &clock);
        link_through_schedule(&maintain, van, id).await;
        let ReferencesOutcome::Reference { view } = server::references::view(&as_pat, id).await
        else {
            panic!("no recall");
        };
        assert!(!view.may_change, "Pat only views the van");
        assert_eq!(view.vehicles.len(), 2);
        let changed = ReferenceInput {
            revision: Some("B".into()),
            ..recall()
        };
        assert_eq!(
            server::references::edit(&as_pat, id, changed.clone()).await,
            ReferencesOutcome::NotAllowed
        );
        assert!(matches!(
            server::references::edit(&maintain, id, changed).await,
            ReferencesOutcome::Saved { .. }
        ));

        // Someone who sees neither vehicle doesn't find it, nor can link it.
        let stranger = SignedIn {
            grants: vec![],
            ..sam.clone()
        };
        assert_eq!(
            server::references::view(&context(&database, &stranger, &clock), id).await,
            ReferencesOutcome::NotFound
        );
        let ReferencesOutcome::References { references } =
            server::references::list(&context(&database, &stranger, &clock)).await
        else {
            panic!("no list");
        };
        assert!(references.is_empty());
        assert_eq!(
            server::references::remove(&maintain, id).await,
            ReferencesOutcome::InUse
        );
    }

    #[tokio::test]
    async fn a_finished_job_with_a_task_for_a_recall_closes_it() {
        use domain::event::{ReadingSet, Task, WorkDetails};
        use domain::requirement::RequirementStatus;

        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &stuart, &clock, "Truck").await;
        let maintainer = in_mode(&stuart, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let ReferencesOutcome::Saved {
            requirement: Some(recall),
            ..
        } = server::references::add(&shop, recall(), link(truck)).await
        else {
            panic!("the recall wasn't added");
        };
        let odometer = page(&shop, truck).await.meters[0].0;
        let mut job = input(truck, 1, send_out("2023-11-20"));
        job.kind = EventKind::Maintenance;
        job.lifecycle = None;
        job.readings = vec![domain::event::Reading {
            meter: odometer,
            value: 80_000_000,
        }];
        job.work = Some(WorkDetails {
            tasks: vec![Task {
                title: Label::parse("Axle nut recall").unwrap(),
                requirement: Some(recall),
                notes: Notes::default(),
                parts: vec![],
                labor_seconds: None,
                labor_cost_minor: None,
                resets_from: ReadingSet::Start,
                next_due: None,
            }],
            end_readings: vec![],
            dispositions: vec![],
            currency: domain::preferences::CurrencyCode::USD,
            complaint: None,
            cause: None,
        });
        let job = saved(&add(&shop, job).await);
        assert_eq!(
            server::events::finish(&shop, job).await,
            EventsOutcome::Done
        );
        let ScheduleOutcome::Schedule { requirements, .. } =
            server::schedule::view(&shop, truck).await
        else {
            panic!("no schedule");
        };
        let view = requirements.iter().find(|view| view.id == recall).unwrap();
        assert_eq!(view.last_done.as_ref().map(|done| done.event), Some(job));
        assert!(
            matches!(view.status, RequirementStatus::Complied { event, .. } if event == job),
            "{:?}",
            view.status
        );
    }
}
