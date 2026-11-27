use super::*;
use crate::access::{
    ActiveMode, AssetScope, CustomGrant, GrantTarget, Role, RoleGrant, available_modes,
    permissions_on, permits_anywhere, permits_on_asset,
};
use crate::test_support::test_id;

const MODES: [ActiveMode; 3] = [
    ActiveMode::Driver,
    ActiveMode::Maintainer,
    ActiveMode::Admin,
];

fn truck() -> AssetScope {
    AssetScope {
        id: test_id(10),
        group: test_id(20),
    }
}

fn car() -> AssetScope {
    AssetScope {
        id: test_id(11),
        group: test_id(21),
    }
}

fn role(name: &str, actions: &[AssetAction]) -> CustomRole {
    CustomRole {
        name: Label::parse(name).unwrap(),
        permissions: Permissions::of(actions),
    }
}

fn grant(target: GrantTarget, kept: &CustomRole) -> RoleGrant {
    RoleGrant {
        user: test_id(1),
        target,
        role: Role::Viewer,
        custom: Some(CustomGrant {
            role: test_id(50),
            permissions: kept.permissions,
        }),
    }
}

fn chauffeur() -> CustomRole {
    role(
        "Chauffeur",
        &[AssetAction::LogFuel, AssetAction::LogReading],
    )
}

fn apprentice() -> CustomRole {
    role(
        "Apprentice",
        &[
            AssetAction::LogFuel,
            AssetAction::LogReading,
            AssetAction::OpenSquawk,
            AssetAction::LogMaintenance,
        ],
    )
}

/// The check as it was before custom roles (Phase 2): the highest role on the asset, capped by
/// the mode except for viewing.
fn as_before(granted: Role, mode: ActiveMode, action: AssetAction) -> bool {
    let required = action.required_role();
    let allowed = if required == Role::Viewer {
        granted
    } else {
        granted.min(mode.cap())
    };
    allowed >= required
}

#[test]
fn every_built_in_role_allows_what_it_did_in_every_mode() {
    for granted in [Role::Viewer, Role::Driver, Role::Maintainer, Role::Admin] {
        let grants = [RoleGrant {
            user: test_id(1),
            target: GrantTarget::AllAssets,
            role: granted,
            custom: None,
        }];
        for mode in MODES {
            for action in AssetAction::ALL {
                assert_eq!(
                    permits_on_asset(&grants, mode, action, truck()),
                    as_before(granted, mode, action),
                    "{granted:?} in {mode:?}: {action:?}"
                );
            }
        }
    }
}

#[test]
fn a_chauffeur_logs_fuel_and_readings_and_nothing_else() {
    let grants = [grant(GrantTarget::AllAssets, &chauffeur())];
    for mode in MODES {
        for action in AssetAction::ALL {
            let expected = matches!(
                action,
                AssetAction::View | AssetAction::LogFuel | AssetAction::LogReading
            );
            assert_eq!(
                permits_on_asset(&grants, mode, action, truck()),
                expected,
                "{mode:?}: {action:?}"
            );
        }
    }
    assert_eq!(available_modes(&grants), [ActiveMode::Driver]);
}

#[test]
fn an_apprentice_logs_maintenance_only_in_maintainer_mode_and_never_signs_off() {
    let grants = [grant(GrantTarget::AllAssets, &apprentice())];
    let may = |mode, action| permits_on_asset(&grants, mode, action, truck());
    assert!(!may(ActiveMode::Driver, AssetAction::LogMaintenance));
    assert!(may(ActiveMode::Maintainer, AssetAction::LogMaintenance));
    assert!(may(ActiveMode::Driver, AssetAction::OpenSquawk));
    for mode in MODES {
        assert!(!may(mode, AssetAction::DispositionSquawk));
        assert!(!may(mode, AssetAction::ConfigureMaintenance));
    }
    assert_eq!(
        available_modes(&grants),
        [ActiveMode::Driver, ActiveMode::Maintainer]
    );
    assert!(!permits_anywhere(
        &grants,
        ActiveMode::Maintainer,
        AssetAction::ConfigureMaintenance
    ));
}

#[test]
fn a_custom_role_on_a_group_or_one_vehicle_covers_only_that() {
    let on_group = [grant(GrantTarget::Group(truck().group), &chauffeur())];
    let on_car = [grant(GrantTarget::Asset(car().id), &chauffeur())];
    let fuel = |grants: &[RoleGrant], scope| {
        permits_on_asset(grants, ActiveMode::Driver, AssetAction::LogFuel, scope)
    };
    assert!(fuel(&on_group, truck()));
    assert!(!fuel(&on_group, car()));
    assert!(fuel(&on_car, car()));
    assert!(!fuel(&on_car, truck()));
    assert_eq!(permissions_on(&on_car, truck()), Permissions::NONE);
    assert!(permissions_on(&on_car, car()).contains(AssetAction::View));
}

#[test]
fn roles_add_up_with_built_in_ones() {
    let grants = [
        grant(GrantTarget::AllAssets, &chauffeur()),
        RoleGrant {
            user: test_id(1),
            target: GrantTarget::Asset(truck().id),
            role: Role::Maintainer,
            custom: None,
        },
    ];
    let sign_off = |scope| {
        permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::DispositionSquawk,
            scope,
        )
    };
    assert!(sign_off(truck()));
    assert!(!sign_off(car()));
}

#[test]
fn custom_roles_are_checked() {
    assert_eq!(check_custom_role(&chauffeur(), None, []), Ok(()));
    let kept = chauffeur();
    let others = [(test_id(5), &kept)];
    assert_eq!(check_custom_role(&kept, Some(test_id(5)), others), Ok(()));
    assert_eq!(
        check_custom_role(&role("CHAUFFEUR", &[AssetAction::LogFuel]), None, others),
        Err(RoleProblem::NameTaken)
    );
    assert_eq!(
        check_custom_role(&role("driver", &[AssetAction::LogFuel]), None, []),
        Err(RoleProblem::NameTaken),
        "a built-in role's name"
    );
    assert_eq!(
        check_custom_role(&role("Looker", &[AssetAction::View]), None, []),
        Err(RoleProblem::NoPermissions)
    );
    assert_eq!(
        check_custom_role(&role("Boss", &[AssetAction::ManageAsset]), None, []),
        Err(RoleProblem::AdminOnly)
    );
}

#[test]
fn permissions_are_stored_as_names_and_survive_the_trip() {
    let json = serde_json::to_string(&apprentice().permissions).unwrap();
    assert_eq!(
        json,
        r#"["log-fuel","log-reading","open-squawk","log-maintenance"]"#
    );
    assert_eq!(
        serde_json::from_str::<Permissions>(&json).unwrap(),
        apprentice().permissions
    );
    let offered = offered_permissions();
    assert_eq!(offered[0], AssetAction::LogFuel);
    assert!(!offered.contains(&AssetAction::ManageAsset));
    assert!(!offered.contains(&AssetAction::View));
    assert_eq!(offered.len(), 10);
}
