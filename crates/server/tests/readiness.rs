//! Readiness on the server (Phase 5 milestone 6; decision #086): a vehicle's default usage,
//! usages added and removed, readiness with a missing document and an open High issue, the
//! Garage's badge, and copying a schedule with its usages.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod readiness {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{
        AssetsOutcome, ReadinessOutcome, ScheduleOutcome, SquawkInput, SquawksOutcome,
    };
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::document::DocumentType;
    use domain::label::Label;
    use domain::location::TimeZone;
    use domain::notes::Notes;
    use domain::readiness::{ItemReason, Mark, Readiness};
    use domain::requirement::{Applicability, ComplianceMode, Requirement};
    use domain::squawk::{Severity, SquawkText};
    use domain::test_support::{ManualClock, test_id};
    use domain::usage::{Usage, UsageId, UsageProblem};
    use server::assets::{AssetsContext, add as add_vehicle, garage};
    use server::health::Health;
    use server::sessions::SignedIn;
    use server::usages;
    use storage::Database;

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

    async fn vehicle(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
        name: &str,
    ) -> AssetId {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(jordan, ActiveMode::Admin);
        let form = AssetForm {
            name: name.into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id, .. } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("{name} wasn't added");
        };
        id
    }

    async fn readiness_of(viewer: &AssetsContext<'_>, asset: AssetId) -> Vec<Readiness> {
        let ReadinessOutcome::Readiness { usages } =
            server::readiness::view(viewer, asset, &utc()).await
        else {
            panic!("no readiness");
        };
        usages
    }

    fn requirement(asset: AssetId, name: &str, usages: Option<Vec<UsageId>>) -> Requirement {
        Requirement {
            asset,
            name: Label::parse(name).unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs: vec![],
            usages,
        }
    }

    #[tokio::test]
    async fn a_new_truck_is_drivable_once_its_documents_are_on_file() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &jordan, &clock, "Truck").await;
        let sams = context(&database, &sam, &clock);

        // Its default usage, from the truck profile; nothing more to make at startup.
        let usages = database.usages_of(truck).await.unwrap();
        assert_eq!(usages.len(), 1);
        let (_, drivable) = &usages[0];
        assert_eq!(
            (drivable.name.as_str(), drivable.default),
            ("Drivable", true)
        );
        assert_eq!(drivable.documents.len(), 2);
        assert_eq!(
            usages::ensure_default_usages(&database, test_id(901), Health::Healthy, &clock).await,
            Ok(0)
        );

        // No registration or insurance on file: not drivable, two reasons.
        let ready = readiness_of(&sams, truck).await;
        assert_eq!(ready[0].mark, Mark::NotReady);
        assert_eq!(
            ready[0]
                .items
                .iter()
                .filter(|item| matches!(item.reason, ItemReason::MissingDocument { .. }))
                .count(),
            2
        );

        // An open High issue (Sam opens it) blocks too; the Garage's chip says so.
        let squawk = SquawkInput {
            asset: truck,
            description: SquawkText::parse("Brakes grinding").unwrap(),
            notes: Notes::default(),
            severity: Severity::High,
            opened_at: NOW,
            found_on: None,
        };
        assert!(matches!(
            server::squawks::open(&sams, squawk).await,
            SquawksOutcome::Saved { .. }
        ));
        let ready = readiness_of(&sams, truck).await;
        assert!(ready[0].items.iter().any(|item| matches!(
            &item.reason,
            ItemReason::HighIssue {
                deferred: false,
                ..
            }
        )));
        let AssetsOutcome::Garage { view } = garage(&sams, &utc()).await else {
            panic!("no garage");
        };
        assert_eq!(
            view.assets[0].ready,
            Some((Label::parse("Drivable").unwrap(), Mark::NotReady))
        );
        assert_eq!(
            server::readiness::view(&sams, test_id(777), &utc()).await,
            ReadinessOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn usages_are_added_kept_apart_and_removed() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &jordan, &clock, "Truck").await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let sams = context(&database, &sam, &clock);
        let usage = |name: &str| Usage {
            asset: truck,
            name: Label::parse(name).unwrap(),
            default: true, // ignored: never a second default
            documents: vec![],
        };
        assert_eq!(
            usages::add_usage(&sams, usage("Towing")).await,
            ScheduleOutcome::NotAllowed
        );
        let ScheduleOutcome::SavedUsage { id: towing } =
            usages::add_usage(&shop, usage("Towing")).await
        else {
            panic!("no Towing");
        };
        assert!(!database.usage(towing).await.unwrap().unwrap().default);
        assert_eq!(
            usages::add_usage(&shop, usage("towing")).await,
            ScheduleOutcome::InvalidUsage {
                problem: UsageProblem::NameTaken
            }
        );

        // The hitch is needed for Towing alone; the oil change (never chosen) for Drivable.
        let hitch = requirement(truck, "Hitch inspection", Some(vec![towing]));
        let ScheduleOutcome::SavedRequirement { id: hitch_id } =
            server::schedule::add_requirement(&shop, hitch).await
        else {
            panic!("no hitch");
        };
        server::schedule::add_requirement(&shop, requirement(truck, "Oil", None)).await;
        assert_eq!(
            server::schedule::add_requirement(
                &shop,
                requirement(truck, "Wipers", Some(vec![test_id(999)]))
            )
            .await,
            ScheduleOutcome::Invalid {
                problem: domain::requirement::RequirementProblem::BadUsages
            }
        );

        // The default can't go; Towing can, and the hitch is then needed for none.
        let drivable = database.usages_of(truck).await.unwrap()[0].0;
        assert_eq!(
            usages::delete_usage(&shop, drivable).await,
            ScheduleOutcome::InvalidUsage {
                problem: UsageProblem::DefaultStays
            }
        );
        assert_eq!(
            usages::delete_usage(&shop, towing).await,
            ScheduleOutcome::Done
        );
        let hitch = database.requirement(hitch_id).await.unwrap().unwrap();
        assert_eq!(
            hitch.usages,
            Some(vec![]),
            "needed for none, not the default"
        );
    }

    #[tokio::test]
    async fn copying_a_schedule_brings_its_usages_by_name() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let truck = vehicle(&database, &jordan, &clock, "Truck").await;
        let van = vehicle(&database, &jordan, &clock, "Van").await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let towing = Usage {
            asset: truck,
            name: Label::parse("Towing").unwrap(),
            default: false,
            documents: vec![DocumentType::listed("document-registration").unwrap()],
        };
        let ScheduleOutcome::SavedUsage { id: towing } = usages::add_usage(&shop, towing).await
        else {
            panic!("no Towing");
        };
        server::schedule::add_requirement(
            &shop,
            requirement(truck, "Hitch inspection", Some(vec![towing])),
        )
        .await;
        assert!(matches!(
            server::schedule::copy(&shop, truck, van).await,
            ScheduleOutcome::Copied { .. }
        ));
        let vans = database.usages_of(van).await.unwrap();
        let names: Vec<&str> = vans.iter().map(|(_, usage)| usage.name.as_str()).collect();
        assert_eq!(names, ["Drivable", "Towing"], "made on the van");
        let vans_towing = vans[1].0;
        let copied = &database.requirements_of(van).await.unwrap()[0].1;
        assert_eq!(copied.usages, Some(vec![vans_towing]));
    }
}
