//! Specs and schedule templates on the server (Phase 5 milestone 4; decision #084): who may see
//! and change specs, requirements ticking them, removing one, and copying them with a schedule;
//! saving, applying, renaming, and removing templates.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod specs {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, ScheduleOutcome, TemplatesOutcome};
    use domain::asset::AssetId;
    use domain::asset_form::AssetForm;
    use domain::label::Label;
    use domain::location::TimeZone;
    use domain::meter::MeterId;
    use domain::notes::Notes;
    use domain::requirement::{
        Applicability, Bundle, ComplianceMode, IntervalRule, Requirement, RequirementProblem,
    };
    use domain::spec::{Spec, SpecAmount, SpecId, SpecProblem, SpecUnit};
    use domain::test_support::{ManualClock, test_id};
    use server::assets::{AssetsContext, add as add_vehicle, view as vehicle_page};
    use server::health::Health;
    use server::schedule;
    use server::sessions::SignedIn;
    use server::templates;
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

    /// A vehicle named `name` (its profile's odometer first), and the odometer's ID.
    async fn vehicle(
        database: &Database,
        jordan: &SignedIn,
        clock: &ManualClock,
        name: &str,
    ) -> (AssetId, MeterId) {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        let group = database.groups().await.unwrap()[0].0;
        let admin = in_mode(jordan, ActiveMode::Admin);
        let form = AssetForm {
            name: name.into(),
            ..AssetForm::new("truck", group)
        };
        let AssetsOutcome::Added { id } =
            add_vehicle(&context(database, &admin, clock), &form).await
        else {
            panic!("{name} wasn't added");
        };
        let AssetsOutcome::Asset { view } =
            vehicle_page(&context(database, &admin, clock), id).await
        else {
            panic!("no page for {name}");
        };
        (id, view.meters[0].0)
    }

    fn spec(asset: AssetId, name: &str, amount: Option<SpecAmount>, number: Option<&str>) -> Spec {
        Spec {
            asset,
            name: Label::parse(name).unwrap(),
            amount,
            detail: None,
            part_number: number.map(str::to_owned),
            notes: Notes::default(),
        }
    }

    fn oil_change(asset: AssetId, odometer: MeterId, specs: Vec<SpecId>) -> Requirement {
        Requirement {
            asset,
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Meter {
                meter: odometer,
                every: 5_000 * MILE,
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs,
        }
    }

    async fn add_spec(shop: &AssetsContext<'_>, spec: Spec) -> SpecId {
        let ScheduleOutcome::SavedSpec { id } = schedule::add_spec(shop, spec).await else {
            panic!("the spec wasn't added");
        };
        id
    }

    /// The vehicle's specs and requirements, as anyone who sees its schedule gets them.
    async fn schedule_of(
        viewer: &AssetsContext<'_>,
        asset: AssetId,
    ) -> (Vec<(SpecId, Spec)>, Vec<Requirement>) {
        let ScheduleOutcome::Schedule {
            specs,
            requirements,
            ..
        } = schedule::view(viewer, asset, &TimeZone::parse("UTC").unwrap()).await
        else {
            panic!("no schedule");
        };
        (
            specs,
            requirements
                .into_iter()
                .map(|view| view.requirement)
                .collect(),
        )
    }

    #[tokio::test]
    async fn maintainers_keep_specs_that_requirements_tick() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = vehicle(&database, &jordan, &clock, "Truck").await;
        let (van, _) = vehicle(&database, &jordan, &clock, "Van").await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let sams = context(&database, &sam, &clock);
        let quarts = SpecAmount::parse("5.5", SpecUnit::Quarts).unwrap();

        // A Driver can't add one; a Maintainer can; a part number that's too long is refused.
        assert_eq!(
            schedule::add_spec(&sams, spec(truck, "Engine oil", Some(quarts), None)).await,
            ScheduleOutcome::NotAllowed
        );
        let oil = add_spec(&shop, spec(truck, "Engine oil", Some(quarts), None)).await;
        let filter = add_spec(&shop, spec(truck, "Oil filter", None, Some("FL-500S"))).await;
        let vans_filter = add_spec(&shop, spec(van, "Oil filter", None, Some("FL-1A"))).await;
        assert_eq!(
            schedule::add_spec(&shop, spec(truck, "Belt", None, Some(&"x".repeat(41)))).await,
            ScheduleOutcome::InvalidSpec {
                problem: SpecProblem::BadPartNumber
            }
        );

        // A requirement ticks the truck's own specs, not the van's.
        assert_eq!(
            schedule::add_requirement(&shop, oil_change(truck, odometer, vec![vans_filter])).await,
            ScheduleOutcome::Invalid {
                problem: RequirementProblem::BadSpecs
            }
        );
        let saved =
            schedule::add_requirement(&shop, oil_change(truck, odometer, vec![oil, filter])).await;
        assert!(
            matches!(saved, ScheduleOutcome::SavedRequirement { .. }),
            "{saved:?}"
        );

        // Sam sees them, by name.
        let (specs, requirements) = schedule_of(&sams, truck).await;
        let names: Vec<&str> = specs.iter().map(|(_, spec)| spec.name.as_str()).collect();
        assert_eq!(names, ["Engine oil", "Oil filter"]);
        assert_eq!(requirements[0].specs, vec![oil, filter]);

        // After a job: "Update the spec too?" is an edit.
        let mut changed = spec(truck, "Oil filter", None, Some("FL-910S"));
        changed.asset = van; // ignored: a spec stays on its vehicle
        assert_eq!(
            schedule::edit_spec(&shop, filter, changed).await,
            ScheduleOutcome::SavedSpec { id: filter }
        );
        let stored = database.spec(filter).await.unwrap().unwrap();
        assert_eq!(stored.part_number.as_deref(), Some("FL-910S"));
        assert_eq!(stored.asset, truck);

        // Removing one unticks it.
        assert_eq!(
            schedule::delete_spec(&shop, oil).await,
            ScheduleOutcome::Done
        );
        let (specs, requirements) = schedule_of(&sams, truck).await;
        assert_eq!(specs.len(), 1);
        assert_eq!(requirements[0].specs, vec![filter]);
        assert_eq!(
            schedule::delete_spec(&sams, filter).await,
            ScheduleOutcome::NotAllowed
        );
    }

    /// The truck with two specs, the oil change ticking both, and a bundle.
    async fn truck_schedule(shop: &AssetsContext<'_>, truck: AssetId, odometer: MeterId) {
        let quarts = SpecAmount::parse("5.5", SpecUnit::Quarts).unwrap();
        let oil = add_spec(shop, spec(truck, "Engine oil", Some(quarts), None)).await;
        let filter = add_spec(shop, spec(truck, "Oil filter", None, Some("FL-500S"))).await;
        let ScheduleOutcome::SavedRequirement { id } =
            schedule::add_requirement(shop, oil_change(truck, odometer, vec![oil, filter])).await
        else {
            panic!("no oil change");
        };
        let bundle = Bundle {
            asset: truck,
            name: Label::parse("Annual service").unwrap(),
            requirements: vec![id],
        };
        assert!(matches!(
            schedule::add_bundle(shop, bundle).await,
            ScheduleOutcome::SavedBundle { .. }
        ));
    }

    /// `asset`'s requirements tick its own specs, which say what the truck's said.
    async fn assert_copied(viewer: &AssetsContext<'_>, asset: AssetId) {
        let (specs, requirements) = schedule_of(viewer, asset).await;
        assert_eq!(specs.len(), 2);
        assert!(specs.iter().all(|(_, spec)| spec.asset == asset));
        let ticked: Vec<SpecId> = specs.iter().map(|(id, _)| *id).collect();
        assert_eq!(requirements.len(), 1);
        assert_eq!(
            requirements[0].specs, ticked,
            "re-linked to its own (oil, filter)"
        );
    }

    #[tokio::test]
    async fn copying_a_schedule_brings_its_specs() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = vehicle(&database, &jordan, &clock, "Truck").await;
        let (van, _) = vehicle(&database, &jordan, &clock, "Van").await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        truck_schedule(&shop, truck, odometer).await;
        let ScheduleOutcome::Copied {
            requirements,
            bundles,
            specs,
            ..
        } = schedule::copy(&shop, truck, van).await
        else {
            panic!("not copied");
        };
        assert_eq!((requirements, bundles, specs), (1, 1, 2));
        assert_copied(&shop, van).await;
    }

    #[tokio::test]
    async fn a_template_is_saved_applied_renamed_and_removed() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let (truck, odometer) = vehicle(&database, &jordan, &clock, "Truck").await;
        let (van, _) = vehicle(&database, &jordan, &clock, "Van").await;
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let shop = context(&database, &maintainer, &clock);
        let sams = context(&database, &sam, &clock);
        let name = |text: &str| Label::parse(text).unwrap();

        // Nothing to save yet; then the truck's schedule.
        assert_eq!(
            templates::save(&shop, truck, name("Pickup basics")).await,
            TemplatesOutcome::Invalid {
                problem: RequirementProblem::NothingToSave
            }
        );
        truck_schedule(&shop, truck, odometer).await;
        assert_eq!(
            templates::save(&sams, truck, name("Pickup basics")).await,
            TemplatesOutcome::NotAllowed
        );
        let TemplatesOutcome::Saved { id } =
            templates::save(&shop, truck, name("Pickup basics")).await
        else {
            panic!("not saved");
        };

        // Listed, with what it holds; the van can take it, the truck (with a schedule) can't.
        let TemplatesOutcome::Templates { view } = templates::list(&shop).await else {
            panic!("no list");
        };
        assert_eq!(view.templates.len(), 1);
        assert_eq!(
            (
                view.templates[0].counts.requirements,
                view.templates[0].counts.bundles,
                view.templates[0].counts.specs
            ),
            (1, 1, 2)
        );
        assert_eq!(view.apply_to, vec![(van, "Van".to_owned())]);
        assert!(!view.may_manage, "Maintainer mode");
        let TemplatesOutcome::Templates { view } = templates::list(&sams).await else {
            panic!("no list for Sam");
        };
        assert!(view.apply_to.is_empty(), "a Driver changes no schedules");

        // Applied to the van: a copy; not twice.
        let applied = templates::apply(&shop, id, van).await;
        assert!(
            matches!(
                applied,
                TemplatesOutcome::Applied {
                    requirements: 1,
                    bundles: 1,
                    specs: 2,
                    ..
                }
            ),
            "{applied:?}"
        );
        assert_copied(&shop, van).await;
        assert_eq!(
            templates::apply(&shop, id, van).await,
            TemplatesOutcome::Invalid {
                problem: RequirementProblem::ScheduleNotEmpty
            }
        );

        let admin = in_mode(&jordan, ActiveMode::Admin);
        only_the_admin_manages(&shop, &context(&database, &admin, &clock), id, van).await;
    }

    /// Only the Admin, in Admin mode (`office`), renames and removes template `id`; then it
    /// can't be applied, and the vehicle it went on (`van`) keeps its copy.
    async fn only_the_admin_manages(
        shop: &AssetsContext<'_>,
        office: &AssetsContext<'_>,
        id: domain::template::TemplateId,
        van: AssetId,
    ) {
        let name = Label::parse("Truck basics").unwrap();
        assert_eq!(
            templates::rename(shop, id, name.clone()).await,
            TemplatesOutcome::NotAllowed
        );
        assert_eq!(
            templates::rename(office, id, name).await,
            TemplatesOutcome::Saved { id }
        );
        assert_eq!(
            templates::delete(shop, id).await,
            TemplatesOutcome::NotAllowed
        );
        assert_eq!(templates::delete(office, id).await, TemplatesOutcome::Done);
        assert_eq!(
            templates::apply(shop, id, van).await,
            TemplatesOutcome::NotFound
        );
        assert_copied(shop, van).await;
    }
}
