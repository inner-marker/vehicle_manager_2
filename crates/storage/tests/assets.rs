//! Groups, assets, meters, custom fields, and group grants against a real database.

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod assets {
    use domain::access::{AssetScope, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::asset::{
        Asset, AssetKind, AssetName, AssetPatch, Identifier, IdentifierKind, LifecycleStatus,
    };
    use domain::command::Clearable;
    use domain::custom_field::{CustomField, CustomFieldPatch, FieldKey, FieldKind, FieldRange};
    use domain::group::{Group, GroupIcon, GroupName, GroupPatch};
    use domain::label::Label;
    use domain::meter::{EventKind, Meter, MeterDimension, MeterPatch, MeterUnit};
    use domain::notes::Notes;
    use domain::test_support::test_id;
    use domain::time::UnixMillis;
    use domain::user::{User, UserPatch, Username};
    use storage::assets::{AssetWriter, CustomFieldWriter, GroupWriter, MeterWriter};
    use storage::commands::RecordWriter;
    use storage::users::{RoleGrantWriter, UserWriter};
    use storage::{Database, NewDatabaseIds};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);
    const GARAGE: u32 = 50;
    const HANGAR: u32 = 51;
    const TRUCK: u32 = 10;

    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        (temp, database)
    }

    async fn create_group(database: &Database, n: u32, name: &str, icon: GroupIcon) {
        let mut connection = database.pool().acquire().await.unwrap();
        let group = Group {
            name: GroupName::parse(name).unwrap(),
            icon,
        };
        GroupWriter
            .create(&mut connection, test_id(n), &GroupPatch::all(group), 1)
            .await
            .unwrap();
    }

    fn truck() -> Asset {
        Asset {
            kind: AssetKind::parse("truck").unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: Some("Ford".into()),
            model: Some("F-150".into()),
            year: Some(2021),
            colour: Some("Blue".into()),
            description: Notes::parse("**Synthetic** oil only").unwrap(),
            group: test_id(GARAGE),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers: vec![
                Identifier::new(IdentifierKind::parse("vin").unwrap(), "1FTFW1E50MFA00000")
                    .unwrap(),
                Identifier::new(IdentifierKind::parse("plate").unwrap(), "ABC 123").unwrap(),
            ],
        }
    }

    async fn with_truck() -> (tempfile::TempDir, Database) {
        let (temp, database) = database().await;
        create_group(&database, GARAGE, "Garage", GroupIcon::Garage).await;
        let mut connection = database.pool().acquire().await.unwrap();
        AssetWriter
            .create(
                &mut connection,
                test_id(TRUCK),
                &AssetPatch::all(truck()),
                2,
            )
            .await
            .unwrap();
        (temp, database)
    }

    #[tokio::test]
    async fn groups_are_created_listed_in_name_order_and_renamed() {
        let (_temp, database) = database().await;
        create_group(&database, HANGAR, "Hangar", GroupIcon::Hangar).await;
        create_group(&database, GARAGE, "Garage", GroupIcon::Garage).await;
        let names: Vec<String> = database
            .groups()
            .await
            .unwrap()
            .into_iter()
            .map(|(_, group)| group.name.as_str().to_owned())
            .collect();
        assert_eq!(names, vec!["Garage", "Hangar"]);

        let mut connection = database.pool().acquire().await.unwrap();
        let rename = GroupPatch {
            name: Some(GroupName::parse("Airport hangar").unwrap()),
            ..GroupPatch::default()
        };
        GroupWriter
            .update(&mut connection, test_id(HANGAR), &rename, 3)
            .await
            .unwrap();
        let groups = database.groups().await.unwrap();
        assert_eq!(groups[0].1.name.as_str(), "Airport hangar");
        assert_eq!(groups[0].1.icon, GroupIcon::Hangar, "the icon is unchanged");
    }

    #[tokio::test]
    async fn an_asset_round_trips_every_field() {
        let (_temp, database) = with_truck().await;
        assert_eq!(database.asset(test_id(TRUCK)).await.unwrap(), Some(truck()));
        assert_eq!(
            database.asset_scope(test_id(TRUCK)).await.unwrap(),
            Some(AssetScope {
                id: test_id(TRUCK),
                group: test_id(GARAGE)
            })
        );
        assert_eq!(database.assets_in_group(test_id(GARAGE)).await.unwrap(), 1);
    }

    #[tokio::test]
    async fn an_update_changes_clears_and_leaves_fields_as_asked() {
        let (_temp, database) = with_truck().await;
        create_group(&database, HANGAR, "Hangar", GroupIcon::Hangar).await;
        let mut connection = database.pool().acquire().await.unwrap();
        let patch = AssetPatch {
            colour: Some(Clearable::Clear),
            year: Some(Clearable::Set(2022)),
            group: Some(test_id(HANGAR)),
            status: Some(LifecycleStatus::Retired),
            identifiers: Some(vec![]),
            ..AssetPatch::default()
        };
        AssetWriter
            .update(&mut connection, test_id(TRUCK), &patch, 3)
            .await
            .unwrap();

        let updated = database.asset(test_id(TRUCK)).await.unwrap().unwrap();
        assert_eq!(updated.colour, None, "cleared");
        assert_eq!(updated.year, Some(2022));
        assert_eq!(updated.group, test_id(HANGAR));
        assert_eq!(updated.status, LifecycleStatus::Retired);
        assert!(updated.identifiers.is_empty());
        assert_eq!(updated.make, truck().make, "unchanged");
        assert_eq!(updated.description, truck().description, "unchanged");
    }

    #[tokio::test]
    async fn a_deleted_asset_is_gone_from_every_query() {
        let (_temp, database) = with_truck().await;
        let mut connection = database.pool().acquire().await.unwrap();
        AssetWriter
            .delete(&mut connection, test_id(TRUCK), 3)
            .await
            .unwrap();
        assert!(database.assets().await.unwrap().is_empty());
        assert_eq!(database.asset(test_id(TRUCK)).await.unwrap(), None);
        assert_eq!(database.asset_scope(test_id(TRUCK)).await.unwrap(), None);
        assert_eq!(database.assets_in_group(test_id(GARAGE)).await.unwrap(), 0);
    }

    #[tokio::test]
    async fn an_asset_needs_an_existing_group() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let result = AssetWriter
            .create(
                &mut connection,
                test_id(TRUCK),
                &AssetPatch::all(truck()),
                1,
            )
            .await;
        assert!(result.is_err(), "the Garage group doesn't exist");
    }

    #[tokio::test]
    async fn meters_round_trip_and_clear_their_description() {
        let (_temp, database) = with_truck().await;
        let meter = Meter {
            asset: test_id(TRUCK),
            name: Label::parse("Engine hours").unwrap(),
            dimension: MeterDimension::Duration,
            unit: MeterUnit::Hours,
            precision: 1,
            description: Some("Oil pressure switch".into()),
            required_for: vec![EventKind::Maintenance, EventKind::Fuel],
        };
        let mut connection = database.pool().acquire().await.unwrap();
        MeterWriter
            .create(
                &mut connection,
                test_id(20),
                &MeterPatch::all(meter.clone()),
                3,
            )
            .await
            .unwrap();
        assert_eq!(
            database.meters_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(20), meter.clone())]
        );

        let patch = MeterPatch {
            description: Some(Clearable::Clear),
            required_for: Some(vec![]),
            ..MeterPatch::default()
        };
        MeterWriter
            .update(&mut connection, test_id(20), &patch, 4)
            .await
            .unwrap();
        let (_, updated) = database.meters_of(test_id(TRUCK)).await.unwrap().remove(0);
        assert_eq!(updated.description, None);
        assert!(updated.required_for.is_empty());
        assert_eq!(updated.name, meter.name, "unchanged");
    }

    #[tokio::test]
    async fn custom_fields_round_trip_their_kind_and_range() {
        let (_temp, database) = with_truck().await;
        let field = CustomField {
            asset: test_id(TRUCK),
            shown_on: vec![EventKind::Fuel, EventKind::Reading],
            key: FieldKey::parse("gallons-pumped").unwrap(),
            label: Label::parse("Gallons pumped").unwrap(),
            kind: FieldKind::Number { decimals: 3 },
            enabled: true,
            required_for: vec![EventKind::Fuel],
            range: Some(FieldRange::new(Some(0), Some(40_000)).unwrap()),
        };
        let mut connection = database.pool().acquire().await.unwrap();
        CustomFieldWriter
            .create(
                &mut connection,
                test_id(30),
                &CustomFieldPatch::all(field.clone()),
                3,
            )
            .await
            .unwrap();
        assert_eq!(
            database.custom_fields_of(test_id(TRUCK)).await.unwrap(),
            vec![(test_id(30), field)]
        );

        let patch = CustomFieldPatch {
            enabled: Some(false),
            required_for: Some(vec![]),
            shown_on: Some(vec![EventKind::Reading]),
            range: Some(Clearable::Clear),
            ..CustomFieldPatch::default()
        };
        CustomFieldWriter
            .update(&mut connection, test_id(30), &patch, 4)
            .await
            .unwrap();
        let (_, updated) = database
            .custom_fields_of(test_id(TRUCK))
            .await
            .unwrap()
            .remove(0);
        assert!(!updated.enabled);
        assert!(updated.required_for.is_empty());
        assert_eq!(updated.shown_on, vec![EventKind::Reading]);
        assert_eq!(updated.range, None);
        assert_eq!(updated.kind, FieldKind::Number { decimals: 3 }, "unchanged");

        // Removed fields keep their keys, so a new field never reuses one.
        CustomFieldWriter
            .delete(&mut connection, test_id(30), 5)
            .await
            .unwrap();
        assert_eq!(
            database.field_keys_of(test_id(TRUCK)).await.unwrap(),
            vec![FieldKey::parse("gallons-pumped").unwrap()]
        );
    }

    #[tokio::test]
    async fn a_grant_on_a_group_is_stored_and_read_back() {
        let (_temp, database) = with_truck().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let sam = User {
            username: Username::parse("sam").unwrap(),
            display_name: "Sam".into(),
            disabled: false,
            must_change_password: false,
        };
        UserWriter
            .create(&mut connection, test_id(2), &UserPatch::all(sam), 3)
            .await
            .unwrap();
        for (n, target) in [
            (60, GrantTarget::Group(test_id(GARAGE))),
            (61, GrantTarget::Asset(test_id(TRUCK))),
            (62, GrantTarget::AllAssets),
        ] {
            let grant = RoleGrant {
                user: test_id(2),
                target,
                role: Role::Driver,
            };
            RoleGrantWriter
                .create(&mut connection, test_id(n), &RoleGrantPatch::all(grant), 4)
                .await
                .unwrap();
        }
        let targets: Vec<GrantTarget> = database
            .grants_for(test_id(2))
            .await
            .unwrap()
            .into_iter()
            .map(|(_, grant)| grant.target)
            .collect();
        assert_eq!(targets.len(), 3);
        assert!(targets.contains(&GrantTarget::Group(test_id(GARAGE))));
        assert!(targets.contains(&GrantTarget::Asset(test_id(TRUCK))));
    }

    #[tokio::test]
    async fn a_grant_must_name_a_real_group() {
        let (_temp, database) = database().await;
        let mut connection = database.pool().acquire().await.unwrap();
        let grant = RoleGrant {
            user: test_id(2),
            target: GrantTarget::Group(test_id(HANGAR)),
            role: Role::Driver,
        };
        let result = RoleGrantWriter
            .create(&mut connection, test_id(60), &RoleGrantPatch::all(grant), 1)
            .await;
        assert!(result.is_err(), "no such user or group");
    }
}
