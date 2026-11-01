//! Vehicles, groups, meters, and custom fields against a real database, through the command
//! pipeline (#030, #052).

mod common;

// In a `#[cfg(test)]` module so clippy treats the helpers as test code too (unwrap allowed).
#[cfg(test)]
mod assets {
    use crate::common::{NOW, setup};
    use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::api::{AssetView, AssetsOutcome, GarageView};
    use domain::asset::{AssetId, LifecycleStatus};
    use domain::asset_form::{AssetForm, GroupForm, MeterForm, Problem};
    use domain::command::Change;
    use domain::group::{GroupIcon, GroupId};
    use domain::meter::{EventKind, MeterDimension, MeterUnit};
    use domain::preferences::{Choice, DistanceUnit, PreferencesPatch};
    use domain::test_support::{ManualClock, test_id};
    use server::assets::{
        AssetsContext, add, add_meter, delete, edit, edit_meter, garage, move_to, preview_move,
        set_order, view,
    };
    use server::commands::issue;
    use server::groups::{add_group, delete_group, ensure_default_group};
    use server::health::Health;
    use server::preferences::{PreferencesContext, set_mine};
    use server::sessions::SignedIn;
    use storage::Database;
    use storage::users::RoleGrantWriter;

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

    fn in_admin(user: &SignedIn) -> SignedIn {
        SignedIn {
            mode: ActiveMode::Admin,
            ..user.clone()
        }
    }

    fn the_garage(outcome: AssetsOutcome) -> GarageView {
        let AssetsOutcome::Garage { view } = outcome else {
            panic!("expected the garage, got {outcome:?}");
        };
        view
    }

    fn the_asset(outcome: AssetsOutcome) -> AssetView {
        let AssetsOutcome::Asset { view } = outcome else {
            panic!("expected a vehicle, got {outcome:?}");
        };
        *view
    }

    /// The default group's ID (it's made at startup).
    async fn default_group(database: &Database, clock: &ManualClock) -> GroupId {
        ensure_default_group(database, test_id(901), Health::Healthy, clock)
            .await
            .unwrap();
        database.groups().await.unwrap()[0].0
    }

    fn truck(group: GroupId) -> AssetForm {
        AssetForm {
            name: "Truck".into(),
            identifiers: vec![("plate".into(), "ABC 123".into())],
            ..AssetForm::new("truck", group)
        }
    }

    fn added(outcome: &AssetsOutcome) -> AssetId {
        let AssetsOutcome::Added { id } = outcome else {
            panic!("expected a new vehicle, got {outcome:?}");
        };
        *id
    }

    #[tokio::test]
    async fn the_default_group_is_made_once() {
        let (_temp, database, _stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        default_group(&database, &clock).await;
        default_group(&database, &clock).await;
        let groups = database.groups().await.unwrap();
        assert_eq!(groups.len(), 1);
        assert_eq!(groups[0].1.name.as_str(), "Garage");
        assert_eq!(groups[0].1.icon, GroupIcon::Garage);
    }

    #[tokio::test]
    async fn only_the_admin_in_admin_mode_adds_vehicles_with_their_profiles_setup() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;

        for person in [&stuart, &sam] {
            let outcome = add(&context(&database, person, &clock), &truck(garage_id)).await;
            assert_eq!(outcome, AssetsOutcome::NotAllowed, "Driver mode");
        }

        // Stuart prefers kilometers: the new odometer follows.
        let prefs = PreferencesContext {
            database: &database,
            database_id: test_id(900),
            user: &stuart,
            health: Health::Healthy,
            clock: &clock,
        };
        let km = PreferencesPatch {
            distance: Some(Choice::Set(DistanceUnit::Kilometers)),
            ..PreferencesPatch::default()
        };
        set_mine(&prefs, km).await;

        let admin = in_admin(&stuart);
        let id = added(&add(&context(&database, &admin, &clock), &truck(garage_id)).await);
        let page = the_asset(view(&context(&database, &admin, &clock), id).await);
        assert_eq!(page.asset.name.as_str(), "Truck");
        assert_eq!(page.meters.len(), 1);
        assert_eq!(page.meters[0].1.name.as_str(), "Odometer");
        assert_eq!(page.meters[0].1.unit, MeterUnit::Kilometers);
        assert_eq!(page.meters[0].1.required_for, vec![EventKind::Fuel]);
        assert_eq!(page.fields.len(), 2, "trip time and trip EV distance");
        assert!(page.can_manage && page.can_configure && page.can_move);
    }

    #[tokio::test]
    async fn a_bad_form_unknown_kind_or_group_is_refused() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);

        let unnamed = AssetForm {
            name: String::new(),
            ..truck(garage_id)
        };
        assert!(matches!(
            add(&ctx, &unnamed).await,
            AssetsOutcome::Invalid { .. }
        ));
        let spaceship = AssetForm::new("spaceship", garage_id);
        let spaceship = AssetForm {
            name: "Enterprise".into(),
            ..spaceship
        };
        assert_eq!(
            add(&ctx, &spaceship).await,
            AssetsOutcome::Invalid {
                problem: Problem::UnknownKind
            }
        );
        assert_eq!(
            add(&ctx, &truck(test_id(777))).await,
            AssetsOutcome::Invalid {
                problem: Problem::UnknownGroup
            }
        );
        assert!(database.assets().await.unwrap().is_empty());
    }

    #[tokio::test]
    async fn the_garage_shows_what_each_person_can_see() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        added(&add(&ctx, &truck(garage_id)).await);
        add_group(
            &ctx,
            &GroupForm {
                name: "Hangar".into(),
                icon: GroupIcon::Hangar,
            },
        )
        .await;

        let admins = the_garage(garage(&ctx).await);
        assert_eq!(admins.assets.len(), 1);
        assert_eq!(
            admins.groups.len(),
            2,
            "the Admin sees the empty Hangar too"
        );
        assert!(admins.can_add && admins.can_manage_groups);

        let sams = the_garage(garage(&context(&database, &sam, &clock)).await);
        assert_eq!(sams.assets.len(), 1, "Driver on all vehicles");
        assert_eq!(sams.assets[0].role, Role::Driver);
        assert_eq!(
            sams.groups.len(),
            1,
            "only groups with vehicles Sam can see"
        );
        assert!(!sams.can_add && !sams.can_manage_groups);
    }

    #[tokio::test]
    async fn editing_changes_details_but_never_kind_or_group() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        let id = added(&add(&ctx, &truck(garage_id)).await);

        let mut form = truck(garage_id);
        form.kind = "car".into();
        form.group = test_id(777);
        form.colour = "Blue".into();
        form.status = LifecycleStatus::Retired;
        assert_eq!(edit(&ctx, id, &form).await, AssetsOutcome::Done);
        let asset = database.asset(id).await.unwrap().unwrap();
        assert_eq!(asset.colour.as_deref(), Some("Blue"));
        assert_eq!(asset.status, LifecycleStatus::Retired, "archived");
        assert_eq!(asset.kind.as_str(), "truck", "kind unchanged");
        assert_eq!(asset.group, garage_id, "group unchanged");

        let sams = edit(&context(&database, &sam, &clock), id, &form).await;
        assert_eq!(sams, AssetsOutcome::NotAllowed);
    }

    #[tokio::test]
    async fn each_person_keeps_their_own_custom_order() {
        let (_temp, database, stuart, sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        let truck_id = added(&add(&ctx, &truck(garage_id)).await);
        let car = AssetForm {
            name: "Civic".into(),
            ..AssetForm::new("car", garage_id)
        };
        let car_id = added(&add(&ctx, &car).await);

        let sams = context(&database, &sam, &clock);
        assert_eq!(
            set_order(&sams, vec![car_id, truck_id]).await,
            AssetsOutcome::Done
        );
        assert_eq!(
            set_order(&sams, vec![truck_id, car_id]).await,
            AssetsOutcome::Done
        );
        assert_eq!(
            the_garage(garage(&sams).await).order,
            vec![truck_id, car_id]
        );
        assert!(
            the_garage(garage(&ctx).await).order.is_empty(),
            "Stuart's own: none yet"
        );
    }

    #[tokio::test]
    async fn deleting_needs_the_name_typed() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        let id = added(&add(&ctx, &truck(garage_id)).await);
        assert_eq!(
            delete(&ctx, id, "Trunk").await,
            AssetsOutcome::Invalid {
                problem: Problem::NameMismatch
            }
        );
        assert_eq!(delete(&ctx, id, " truck ").await, AssetsOutcome::Done);
        assert_eq!(view(&ctx, id).await, AssetsOutcome::NotFound);
    }

    #[tokio::test]
    async fn a_move_previews_and_changes_who_can_see_the_vehicle() {
        let (_temp, database, stuart, mut sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        let id = added(&add(&ctx, &truck(garage_id)).await);
        add_group(
            &ctx,
            &GroupForm {
                name: "Hangar".into(),
                icon: GroupIcon::Hangar,
            },
        )
        .await;
        let hangar = database
            .groups()
            .await
            .unwrap()
            .into_iter()
            .find(|(_, group)| group.name.as_str() == "Hangar")
            .unwrap()
            .0;

        // Sam: only the Garage group (their all-vehicles grant is replaced for this test).
        let garage_only = RoleGrant {
            user: sam.user,
            target: GrantTarget::Group(garage_id),
            role: Role::Driver,
        };
        issue(
            &database,
            &RoleGrantWriter,
            Change::Delete { id: test_id(102) },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        issue(
            &database,
            &RoleGrantWriter,
            Change::Create {
                id: test_id(103),
                values: RoleGrantPatch::all(garage_only),
            },
            test_id(901),
            None,
            Health::Healthy,
            &clock,
        )
        .await
        .unwrap();
        sam.grants = vec![garage_only];

        let AssetsOutcome::MovePreview { changes } = preview_move(&ctx, id, hangar).await else {
            panic!("expected a preview");
        };
        assert_eq!(changes.len(), 1);
        assert_eq!(changes[0].display_name, "sam");
        assert_eq!(
            (changes[0].before, changes[0].after),
            (Some(Role::Driver), None)
        );

        assert_eq!(move_to(&ctx, id, hangar).await, AssetsOutcome::Done);
        assert_eq!(database.asset(id).await.unwrap().unwrap().group, hangar);
        let sams = the_garage(garage(&context(&database, &sam, &clock)).await);
        assert!(sams.assets.is_empty(), "Sam can't see it in the Hangar");
        assert_eq!(
            view(&context(&database, &sam, &clock), id).await,
            AssetsOutcome::NotFound
        );
    }

    #[tokio::test]
    async fn groups_with_vehicles_and_the_last_group_stay() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let ctx = context(&database, &admin, &clock);
        assert_eq!(
            delete_group(&ctx, garage_id).await,
            AssetsOutcome::Invalid {
                problem: Problem::LastGroup
            }
        );
        added(&add(&ctx, &truck(garage_id)).await);
        add_group(
            &ctx,
            &GroupForm {
                name: "Hangar".into(),
                icon: GroupIcon::Hangar,
            },
        )
        .await;
        assert_eq!(
            delete_group(&ctx, garage_id).await,
            AssetsOutcome::Invalid {
                problem: Problem::GroupNotEmpty
            }
        );
    }

    #[tokio::test]
    async fn meters_need_maintainer_work_and_keep_their_dimension() {
        let (_temp, database, stuart, _sam) = setup().await;
        let clock = ManualClock::new(NOW);
        let garage_id = default_group(&database, &clock).await;
        let admin = in_admin(&stuart);
        let id = added(&add(&context(&database, &admin, &clock), &truck(garage_id)).await);
        let hours = MeterForm {
            name: "Engine hours".into(),
            dimension: MeterDimension::Duration,
            unit: MeterUnit::Hours,
            precision: 1,
            description: String::new(),
            required_for: vec![EventKind::Maintenance],
        };
        // Driver mode can't; Maintainer mode can.
        let driver = context(&database, &stuart, &clock);
        assert_eq!(
            add_meter(&driver, id, &hours).await,
            AssetsOutcome::NotAllowed
        );
        let maintainer = SignedIn {
            mode: ActiveMode::Maintainer,
            ..stuart.clone()
        };
        let ctx = context(&database, &maintainer, &clock);
        assert_eq!(add_meter(&ctx, id, &hours).await, AssetsOutcome::Done);

        let (meter_id, _) = database
            .meters_of(id)
            .await
            .unwrap()
            .into_iter()
            .find(|(_, meter)| meter.name.as_str() == "Engine hours")
            .unwrap();
        // Asking for a distance in miles: the dimension stays a duration, so miles is refused.
        let as_distance = MeterForm {
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            ..hours
        };
        assert!(matches!(
            edit_meter(&ctx, meter_id, &as_distance).await,
            AssetsOutcome::Invalid { .. }
        ));
        assert_eq!(
            database.meter(meter_id).await.unwrap().unwrap().dimension,
            MeterDimension::Duration
        );
    }
}
