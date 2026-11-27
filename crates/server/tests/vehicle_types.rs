//! Household vehicle types on the server (phase-6.md 3e; decision #090): who may change them,
//! their checks, a vehicle added as a type, a type's change and removal, and a vehicle's type
//! changed on Setup.

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod vehicle_types {
    use crate::common::{NOW, setup};
    use domain::access::ActiveMode;
    use domain::api::{AssetsOutcome, ScheduleOutcome, TemplatesOutcome, TypesOutcome};
    use domain::asset::{AssetId, AssetKind};
    use domain::asset_form::{AssetForm, Problem};
    use domain::catalogue::{CatalogueChoice, TireAxle};
    use domain::document::DocumentType;
    use domain::group::GroupId;
    use domain::label::Label;
    use domain::notes::Notes;
    use domain::requirement::{Applicability, ComplianceMode, IntervalRule, Requirement};
    use domain::template::TemplateId;
    use domain::test_support::{ManualClock, test_id};
    use domain::vehicle_type::{TypeIcon, TypeProblem, VehicleType, VehicleTypeId};
    use server::assets::{AssetsContext, add, edit, view};
    use server::health::Health;
    use server::sessions::SignedIn;
    use server::vehicle_types;
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

    fn label(text: &str) -> Label {
        Label::parse(text).unwrap()
    }

    async fn group(database: &Database, clock: &ManualClock) -> GroupId {
        server::groups::ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        database.groups().await.unwrap()[0].0
    }

    /// Adds a vehicle from `form`; its ID and any rules its type's template left out.
    async fn added(office: &AssetsContext<'_>, form: &AssetForm) -> (AssetId, Vec<(Label, Label)>) {
        let outcome = add(office, form).await;
        let AssetsOutcome::Added { id, left_out } = outcome else {
            panic!("not added: {outcome:?}");
        };
        (id, left_out)
    }

    /// A truck with an oil change every 5,000 mi on its odometer, saved as a template.
    async fn pickup_template(office: &AssetsContext<'_>, group: GroupId) -> TemplateId {
        let form = AssetForm {
            name: "Truck".into(),
            ..AssetForm::new("truck", group)
        };
        let (truck, _) = added(office, &form).await;
        let AssetsOutcome::Asset { view } = view(office, truck).await else {
            panic!("no page");
        };
        let oil = Requirement {
            asset: truck,
            name: label("Engine oil"),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Meter {
                meter: view.meters[0].0,
                every: 5_000 * 1_609,
                first: None,
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs: vec![],
            usages: None,
        };
        assert!(matches!(
            server::schedule::add_requirement(office, oil).await,
            ScheduleOutcome::SavedRequirement { .. }
        ));
        let TemplatesOutcome::Saved { id } =
            server::templates::save(office, truck, label("Pickup basics")).await
        else {
            panic!("no template");
        };
        id
    }

    /// "Track car": a car needing only insurance, its trip time off, four tires' fields.
    fn track_car() -> VehicleType {
        VehicleType {
            icon: TypeIcon::Flag,
            documents: vec![DocumentType::listed("document-insurance").unwrap()],
            fields_on: vec![],
            catalogue: vec![CatalogueChoice::Tires {
                axles: vec![TireAxle::Single, TireAxle::Single],
            }],
            ..VehicleType::from_kind(label("Track car"), AssetKind::parse("car").unwrap())
        }
    }

    async fn saved(office: &AssetsContext<'_>, kept: VehicleType) -> VehicleTypeId {
        let outcome = vehicle_types::add(office, kept).await;
        let TypesOutcome::Saved { id } = outcome else {
            panic!("not saved: {outcome:?}");
        };
        id
    }

    #[tokio::test]
    async fn only_the_admin_in_admin_mode_changes_types_and_everyone_sees_them() {
        let (_temp, database, jordan, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let maintainer = in_mode(&jordan, ActiveMode::Maintainer);
        let office = context(&database, &admin, &clock);
        for person in [&maintainer, &sam] {
            assert_eq!(
                vehicle_types::add(&context(&database, person, &clock), track_car()).await,
                TypesOutcome::NotAllowed
            );
        }
        let id = saved(&office, track_car()).await;
        let sams = context(&database, &sam, &clock);
        let TypesOutcome::Types { view } = vehicle_types::list(&sams).await else {
            panic!("Sam sees no types");
        };
        assert_eq!(view.types, vec![(id, track_car())]);
        assert!(!view.may_manage);
        assert_eq!(
            vehicle_types::edit(&sams, id, track_car()).await,
            TypesOutcome::NotAllowed
        );
        assert_eq!(
            vehicle_types::delete(&sams, id).await,
            TypesOutcome::NotAllowed
        );
        let TypesOutcome::Types { view } = vehicle_types::list(&office).await else {
            panic!("no list");
        };
        assert!(view.may_manage);
    }

    #[tokio::test]
    async fn types_are_checked() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let office = context(&database, &admin, &clock);
        let id = saved(&office, track_car()).await;
        let again = VehicleType {
            name: label("track CAR"),
            ..track_car()
        };
        assert_eq!(
            vehicle_types::add(&office, again).await,
            TypesOutcome::Invalid {
                problem: TypeProblem::NameTaken
            }
        );
        let spaceship = VehicleType {
            name: label("Rocket"),
            kind: AssetKind::parse("spaceship").unwrap(),
            ..track_car()
        };
        assert_eq!(
            vehicle_types::add(&office, spaceship).await,
            TypesOutcome::Invalid {
                problem: TypeProblem::UnknownKind
            }
        );
        let missing = VehicleType {
            template: Some(test_id(77)),
            ..track_car()
        };
        assert_eq!(
            vehicle_types::edit(&office, id, missing).await,
            TypesOutcome::Invalid {
                problem: TypeProblem::TemplateGone
            }
        );
        assert_eq!(
            vehicle_types::edit(&office, test_id(78), track_car()).await,
            TypesOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn a_vehicle_added_as_a_type_starts_as_it_says() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let office = context(&database, &admin, &clock);
        let group = group(&database, &clock).await;
        let template = pickup_template(&office, group).await;

        // A track car: its kind from the type, whatever the form says.
        let track = saved(&office, track_car()).await;
        let form = AssetForm {
            name: "Racer".into(),
            vehicle_type: Some(track),
            ..AssetForm::new("truck", group)
        };
        let (racer, left_out) = added(&office, &form).await;
        assert!(left_out.is_empty());
        let AssetsOutcome::Asset { view: page } = view(&office, racer).await else {
            panic!("no page");
        };
        assert_eq!(page.asset.kind.as_str(), "car");
        assert_eq!(page.asset.vehicle_type, Some(track));
        assert_eq!(page.fields.len(), 2 + 8);
        let trip_time = page
            .fields
            .iter()
            .find(|(_, field)| field.key.as_str() == "trip-time")
            .unwrap();
        assert!(!trip_time.1.enabled, "turned off by the type");
        let usages = database.usages_of(racer).await.unwrap();
        assert_eq!(usages.len(), 1);
        assert_eq!(usages[0].1.documents, track_car().documents);

        // A backup generator with the pickup's template: its odometer rule is left out.
        let backup = VehicleType {
            template: Some(template),
            ..VehicleType::from_kind(label("Backup"), AssetKind::parse("generator").unwrap())
        };
        let backup = saved(&office, backup).await;
        let form = AssetForm {
            name: "Shed generator".into(),
            vehicle_type: Some(backup),
            ..AssetForm::new("generator", group)
        };
        let (_, left_out) = added(&office, &form).await;
        assert_eq!(left_out, vec![(label("Engine oil"), label("Odometer"))]);

        // A work truck with it: copied whole.
        let work = VehicleType {
            template: Some(template),
            ..VehicleType::from_kind(label("Work truck"), AssetKind::parse("truck").unwrap())
        };
        let work = saved(&office, work).await;
        let form = AssetForm {
            name: "Truck 2".into(),
            vehicle_type: Some(work),
            ..AssetForm::new("truck", group)
        };
        let (pickup, left_out) = added(&office, &form).await;
        assert!(left_out.is_empty());
        assert_eq!(database.requirements_of(pickup).await.unwrap().len(), 1);

        let gone = AssetForm {
            name: "Nothing".into(),
            vehicle_type: Some(test_id(79)),
            ..AssetForm::new("car", group)
        };
        assert_eq!(
            add(&office, &gone).await,
            AssetsOutcome::Invalid {
                problem: Problem::Type(TypeProblem::UnknownType)
            }
        );
    }

    #[tokio::test]
    async fn a_types_change_or_removal_leaves_its_vehicles_be() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let office = context(&database, &admin, &clock);
        let group = group(&database, &clock).await;
        let track = saved(&office, track_car()).await;
        let form = AssetForm {
            name: "Racer".into(),
            vehicle_type: Some(track),
            ..AssetForm::new("car", group)
        };
        let (racer, _) = added(&office, &form).await;
        let fewer = VehicleType {
            catalogue: vec![],
            documents: vec![],
            ..track_car()
        };
        assert_eq!(
            vehicle_types::edit(&office, track, fewer).await,
            TypesOutcome::Saved { id: track }
        );
        assert_eq!(database.custom_fields_of(racer).await.unwrap().len(), 10);
        assert_eq!(
            database.usages_of(racer).await.unwrap()[0]
                .1
                .documents
                .len(),
            1,
            "still the insurance"
        );
        assert_eq!(
            vehicle_types::delete(&office, track).await,
            TypesOutcome::Done
        );
        let racer_now = database.asset(racer).await.unwrap().unwrap();
        assert_eq!(
            racer_now.vehicle_type,
            Some(track),
            "kept: it shows its kind"
        );
        assert_eq!(racer_now.kind.as_str(), "car");
        assert!(database.vehicle_type(track).await.unwrap().is_none());
    }

    #[tokio::test]
    async fn a_vehicles_type_changes_only_to_one_of_its_kind() {
        let (_temp, database, jordan, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let admin = in_mode(&jordan, ActiveMode::Admin);
        let office = context(&database, &admin, &clock);
        let group = group(&database, &clock).await;
        let form = AssetForm {
            name: "Daily".into(),
            ..AssetForm::new("car", group)
        };
        let (daily, _) = added(&office, &form).await;
        let track = saved(&office, track_car()).await;
        let work = saved(
            &office,
            VehicleType::from_kind(label("Work truck"), AssetKind::parse("truck").unwrap()),
        )
        .await;
        let mut edited = AssetForm::from_asset(&database.asset(daily).await.unwrap().unwrap());
        edited.vehicle_type = Some(work);
        assert_eq!(
            edit(&office, daily, &edited).await,
            AssetsOutcome::Invalid {
                problem: Problem::Type(TypeProblem::WrongKind)
            }
        );
        edited.vehicle_type = Some(track);
        assert_eq!(edit(&office, daily, &edited).await, AssetsOutcome::Done);
        let fields = database.custom_fields_of(daily).await.unwrap().len();
        assert_eq!(fields, 2, "only the label and icon change, not its fields");
        assert_eq!(
            database.asset(daily).await.unwrap().unwrap().vehicle_type,
            Some(track)
        );
        edited.vehicle_type = None;
        assert_eq!(edit(&office, daily, &edited).await, AssetsOutcome::Done);
        assert_eq!(
            database.asset(daily).await.unwrap().unwrap().vehicle_type,
            None
        );
    }
}
