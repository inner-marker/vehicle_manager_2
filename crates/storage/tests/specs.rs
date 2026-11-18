//! Specs, requirements' specs, and schedule templates against a real database (Phase 5
//! milestone 4, migration 0025; decision #084).

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod specs {
    use domain::asset::{Asset, AssetKind, AssetName, AssetPatch, LifecycleStatus};
    use domain::command::Clearable;
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::label::Label;
    use domain::notes::Notes;
    use domain::requirement::{Applicability, ComplianceMode, Requirement, RequirementPatch};
    use domain::spec::{Spec, SpecAmount, SpecPatch, SpecUnit};
    use domain::template::{ScheduleTemplate, ScheduleTemplatePatch};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use sqlx::Sqlite;
    use sqlx::pool::PoolConnection;
    use storage::assets::{AssetWriter, GroupWriter};
    use storage::commands::RecordWriter;
    use storage::requirements::RequirementWriter;
    use storage::specs::SpecWriter;
    use storage::templates::TemplateWriter;
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const GARAGE: u32 = 50;
    const TRUCK: u32 = 10;
    const OIL: u32 = 30;
    const FILTER: u32 = 31;
    const OIL_CHANGE: u32 = 40;
    const TEMPLATE: u32 = 60;

    /// A database with a group and a truck in it, and a connection to write with.
    async fn database() -> (tempfile::TempDir, Database, PoolConnection<Sqlite>) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        let mut connection = database.pool().acquire().await.unwrap();
        let group = Group {
            name: GroupName::parse("Garage").unwrap(),
            icon: GroupIcon::Garage,
        };
        GroupWriter
            .create(&mut connection, test_id(GARAGE), &GroupPatch::all(group), 1)
            .await
            .unwrap();
        let truck = Asset {
            kind: AssetKind::parse("truck").unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: None,
            model: None,
            year: None,
            colour: None,
            description: Notes::default(),
            group: test_id(GARAGE),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers: vec![],
        };
        AssetWriter
            .create(&mut connection, test_id(TRUCK), &AssetPatch::all(truck), 2)
            .await
            .unwrap();
        (temp, database, connection)
    }

    fn oil() -> Spec {
        Spec {
            asset: test_id(TRUCK),
            name: Label::parse("Engine oil").unwrap(),
            amount: Some(SpecAmount::parse("5.5", SpecUnit::Quarts).unwrap()),
            detail: Some("5W-30 full synthetic".into()),
            part_number: None,
            notes: Notes::default(),
        }
    }

    fn filter() -> Spec {
        Spec {
            asset: test_id(TRUCK),
            name: Label::parse("Oil filter").unwrap(),
            amount: None,
            detail: None,
            part_number: Some("FL-500S".into()),
            notes: Notes::default(),
        }
    }

    #[tokio::test]
    async fn a_spec_keeps_its_amount_as_entered_and_its_fields_clear() {
        let (_temp, database, mut connection) = database().await;
        SpecWriter
            .create(&mut connection, test_id(OIL), &SpecPatch::all(oil()), 3)
            .await
            .unwrap();
        SpecWriter
            .create(
                &mut connection,
                test_id(FILTER),
                &SpecPatch::all(filter()),
                4,
            )
            .await
            .unwrap();
        let (milli, unit): (i64, String) = sqlx::query_as(
            "SELECT amount_milli, amount_unit FROM specs WHERE amount_unit IS NOT NULL",
        )
        .fetch_one(&mut *connection)
        .await
        .unwrap();
        assert_eq!((milli, unit.as_str()), (5_500, "quarts"));
        let listed = database.specs_of(test_id(TRUCK)).await.unwrap();
        assert_eq!(
            listed,
            vec![(test_id(OIL), oil()), (test_id(FILTER), filter())],
            "by name"
        );

        // 6 liters now; no detail; a part number.
        let patch = SpecPatch {
            amount: Some(Clearable::Set(
                SpecAmount::parse("6", SpecUnit::Liters).unwrap(),
            )),
            detail: Some(Clearable::Clear),
            part_number: Some(Clearable::Set("M1-5W30".into())),
            ..SpecPatch::default()
        };
        SpecWriter
            .update(&mut connection, test_id(OIL), &patch, 5)
            .await
            .unwrap();
        let changed = database.spec(test_id(OIL)).await.unwrap().unwrap();
        assert_eq!(changed.amount.map(SpecAmount::show), Some("6 L".into()));
        assert_eq!(changed.detail, None);
        assert_eq!(changed.part_number.as_deref(), Some("M1-5W30"));
        assert_eq!(changed.name, oil().name, "unchanged");
        // No amount at all.
        let cleared = SpecPatch {
            amount: Some(Clearable::Clear),
            ..SpecPatch::default()
        };
        SpecWriter
            .update(&mut connection, test_id(OIL), &cleared, 6)
            .await
            .unwrap();
        assert_eq!(
            database.spec(test_id(OIL)).await.unwrap().unwrap().amount,
            None
        );
        SpecWriter
            .delete(&mut connection, test_id(FILTER), 7)
            .await
            .unwrap();
        assert_eq!(database.spec(test_id(FILTER)).await.unwrap(), None);
        assert_eq!(database.specs_of(test_id(TRUCK)).await.unwrap().len(), 1);
    }

    fn oil_change() -> Requirement {
        Requirement {
            asset: test_id(TRUCK),
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
            before_records: None,
            warnings: None,
            specs: vec![test_id(OIL), test_id(FILTER)],
            usages: vec![],
        }
    }

    #[tokio::test]
    async fn a_requirement_keeps_its_specs_in_order() {
        let (_temp, database, mut connection) = database().await;
        RequirementWriter
            .create(
                &mut connection,
                test_id(OIL_CHANGE),
                &RequirementPatch::all(oil_change()),
                3,
            )
            .await
            .unwrap();
        let stored = database
            .requirement(test_id(OIL_CHANGE))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.specs, vec![test_id(OIL), test_id(FILTER)]);
        let reordered = RequirementPatch {
            specs: Some(vec![test_id(FILTER)]),
            ..RequirementPatch::default()
        };
        RequirementWriter
            .update(&mut connection, test_id(OIL_CHANGE), &reordered, 4)
            .await
            .unwrap();
        let stored = database
            .requirement(test_id(OIL_CHANGE))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.specs, vec![test_id(FILTER)]);
    }

    #[tokio::test]
    async fn a_template_round_trips_and_goes() {
        let (_temp, database, mut connection) = database().await;
        let template = ScheduleTemplate::from_vehicle(
            Label::parse("Pickup basics").unwrap(),
            &[],
            &[(test_id(OIL_CHANGE), oil_change())],
            &[],
            &[(test_id(OIL), oil()), (test_id(FILTER), filter())],
            &[],
        );
        TemplateWriter
            .create(
                &mut connection,
                test_id(TEMPLATE),
                &ScheduleTemplatePatch::all(template.clone()),
                3,
            )
            .await
            .unwrap();
        assert_eq!(
            database.templates().await.unwrap(),
            vec![(test_id(TEMPLATE), template.clone())]
        );
        let renamed = ScheduleTemplatePatch {
            name: Some(Label::parse("Truck basics").unwrap()),
            ..ScheduleTemplatePatch::default()
        };
        TemplateWriter
            .update(&mut connection, test_id(TEMPLATE), &renamed, 4)
            .await
            .unwrap();
        let stored = database.template(test_id(TEMPLATE)).await.unwrap().unwrap();
        assert_eq!(stored.name.as_str(), "Truck basics");
        assert_eq!(stored.specs, template.specs, "the rest unchanged");
        TemplateWriter
            .delete(&mut connection, test_id(TEMPLATE), 5)
            .await
            .unwrap();
        assert!(database.templates().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn a_vehicles_usages_are_kept_default_first() {
        use domain::document::DocumentType;
        use domain::usage::{Usage, UsagePatch};
        use storage::usages::UsageWriter;
        let (_temp, database, mut connection) = database().await;
        let usage = |name: &str, default: bool, documents: Vec<DocumentType>| Usage {
            asset: test_id(TRUCK),
            name: Label::parse(name).unwrap(),
            default,
            documents,
        };
        let insurance = DocumentType::listed("document-insurance").unwrap();
        let drivable = usage("Drivable", true, vec![insurance.clone()]);
        let towing = usage("Towing", false, vec![]);
        for (n, item, sequence) in [(71, &towing, 3), (70, &drivable, 4)] {
            UsageWriter
                .create(
                    &mut connection,
                    test_id(n),
                    &UsagePatch::all(item.clone()),
                    sequence,
                )
                .await
                .unwrap();
        }
        assert_eq!(
            database.usages_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(70), drivable.clone()), (test_id(71), towing)],
            "the default first"
        );
        // Renamed, and needing nothing more; it stays the default.
        let patch = UsagePatch {
            name: Some(Label::parse("Road-ready").unwrap()),
            documents: Some(vec![]),
            default: Some(false),
            ..UsagePatch::default()
        };
        UsageWriter
            .update(&mut connection, test_id(70), &patch, 5)
            .await
            .unwrap();
        let changed = database.usage(test_id(70)).await.unwrap().unwrap();
        assert_eq!(changed.name.as_str(), "Road-ready");
        assert!(changed.documents.is_empty());
        assert!(changed.default, "fixed once made");
        UsageWriter
            .delete(&mut connection, test_id(71), 6)
            .await
            .unwrap();
        assert_eq!(database.usages_of(test_id(TRUCK)).await.unwrap().len(), 1);
    }

    #[tokio::test]
    async fn requirements_and_templates_keep_their_usages() {
        use domain::usage::Usage;
        let (_temp, database, mut connection) = database().await;
        let changed = Usage {
            asset: test_id(TRUCK),
            name: Label::parse("Road-ready").unwrap(),
            default: true,
            documents: vec![],
        };
        // A requirement keeps the usages it ticks; a template keeps the vehicle's usages.
        let mut oil = oil_change();
        oil.usages = vec![test_id(70)];
        RequirementWriter
            .create(
                &mut connection,
                test_id(OIL_CHANGE),
                &RequirementPatch::all(oil),
                7,
            )
            .await
            .unwrap();
        let stored = database
            .requirement(test_id(OIL_CHANGE))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(stored.usages, vec![test_id(70)]);
        let template = ScheduleTemplate::from_vehicle(
            Label::parse("Pickup basics").unwrap(),
            &[],
            &[(test_id(OIL_CHANGE), stored)],
            &[],
            &[],
            &[(test_id(70), changed)],
        );
        TemplateWriter
            .create(
                &mut connection,
                test_id(TEMPLATE),
                &ScheduleTemplatePatch::all(template.clone()),
                8,
            )
            .await
            .unwrap();
        assert_eq!(
            database
                .template(test_id(TEMPLATE))
                .await
                .unwrap()
                .unwrap()
                .usages,
            template.usages
        );
    }
}
