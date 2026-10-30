//! Who may do what: roles, role grants, active modes, and the permission check
//! (plan §6 "Users, roles & permissions", decisions #019, #045).
//!
//! - A **role** (Viewer < Driver < Maintainer < Admin) includes every lower one.
//! - A **grant** gives a user a role on **one asset** or on **all assets** (#045). A user's
//!   role on an asset is the highest of every grant that covers it.
//! - The **active mode** (Driver, Maintainer, Admin) caps what the user can do right now,
//!   whatever their roles: least privilege by default. Sign-in starts in Driver mode.
//! - **Admin-wide actions** (users, adding assets, server operations) need the Admin role on
//!   *all* assets and Admin mode; being Admin of one vehicle doesn't make someone the
//!   household's Admin.
//! - There is **always at least one Admin** ([`leaves_an_admin`]).
//!
//! The same checks run on devices (instant feedback) and on the server (enforcement).
//!
//! # Examples
//!
//! ```
//! use domain::access::{ActiveMode, AssetAction, GrantTarget, Role, RoleGrant, permits_on_asset};
//! use domain::id::Id;
//! use domain::time::UnixMillis;
//!
//! let at = UnixMillis::from_millis(1_700_000_000_000);
//! let user = Id::generate(at, [1; 10]).unwrap();
//! let truck = Id::generate(at, [2; 10]).unwrap();
//! let grants = [RoleGrant { user, target: GrantTarget::AllAssets, role: Role::Maintainer }];
//!
//! // A Maintainer can log maintenance, but only once they switch to Maintainer mode.
//! assert!(!permits_on_asset(&grants, ActiveMode::Driver, AssetAction::LogMaintenance, truck));
//! assert!(permits_on_asset(&grants, ActiveMode::Maintainer, AssetAction::LogMaintenance, truck));
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Patch, Record};
use crate::id::{AssetId, Id};
use crate::user::UserId;

/// A role, from least to most powerful; each includes every lower one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Role {
    /// Read-only.
    Viewer,
    /// Fuel-ups, readings, and opening squawks.
    Driver,
    /// Maintenance and repairs, squawk sign-off, and maintenance setup.
    Maintainer,
    /// Everything, including users, vehicles, and server operations.
    Admin,
}

/// What a grant covers.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "asset", rename_all = "kebab-case")]
pub enum GrantTarget {
    /// Every asset, including ones added later.
    AllAssets,
    /// One asset.
    Asset(AssetId),
}

impl GrantTarget {
    /// Whether this target covers `asset`.
    #[must_use]
    pub fn covers(self, asset: AssetId) -> bool {
        match self {
            Self::AllAssets => true,
            Self::Asset(target) => target == asset,
        }
    }
}

/// One role for one user on one target. A user can have several; they add up.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RoleGrant {
    /// Who.
    pub user: UserId,
    /// On what.
    pub target: GrantTarget,
    /// Which role.
    pub role: Role,
}

/// The mode a user is working in. It caps their permissions: least privilege by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ActiveMode {
    /// The starting mode: fuel-ups, readings, squawks.
    Driver,
    /// Maintenance work.
    Maintainer,
    /// Administration; needs a confirmation to enter and times out.
    Admin,
}

impl ActiveMode {
    /// The highest role this mode allows.
    #[must_use]
    pub const fn cap(self) -> Role {
        match self {
            Self::Driver => Role::Driver,
            Self::Maintainer => Role::Maintainer,
            Self::Admin => Role::Admin,
        }
    }
}

/// Something done to one asset.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AssetAction {
    /// See the asset and its history.
    View,
    /// Log a fuel-up.
    LogFuel,
    /// Log a meter reading.
    LogReading,
    /// Open a squawk.
    OpenSquawk,
    /// Log a maintenance or repair event.
    LogMaintenance,
    /// Sign off or defer a squawk.
    DispositionSquawk,
    /// Change meters, intervals, service bundles, or required readings.
    ConfigureMaintenance,
    /// Edit the asset's own details, archive or delete it.
    ManageAsset,
}

impl AssetAction {
    /// The lowest role that may do this.
    #[must_use]
    pub const fn required_role(self) -> Role {
        match self {
            Self::View => Role::Viewer,
            Self::LogFuel | Self::LogReading | Self::OpenSquawk => Role::Driver,
            Self::LogMaintenance | Self::DispositionSquawk | Self::ConfigureMaintenance => {
                Role::Maintainer
            }
            Self::ManageAsset => Role::Admin,
        }
    }
}

/// Something done to the household as a whole: needs Admin on all assets, in Admin mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum AdminAction {
    /// Add, change, disable users, and grant or remove roles.
    ManageUsers,
    /// Add a new asset.
    AddAsset,
    /// Change server-wide settings and defaults.
    ManageServerSettings,
    /// Server operations (e.g. rebuild from a device).
    ServerOperations,
}

/// The user's role on `asset`: the highest of the grants that cover it, or `None`.
/// `grants` are this user's own grants.
#[must_use]
pub fn role_on(grants: &[RoleGrant], asset: AssetId) -> Option<Role> {
    grants
        .iter()
        .filter(|grant| grant.target.covers(asset))
        .map(|grant| grant.role)
        .max()
}

/// The highest role the user holds anywhere.
#[must_use]
pub fn highest_role(grants: &[RoleGrant]) -> Option<Role> {
    grants.iter().map(|grant| grant.role).max()
}

/// Whether the user is the household's Admin: Admin on all assets.
#[must_use]
pub fn is_household_admin(grants: &[RoleGrant]) -> bool {
    grants
        .iter()
        .any(|grant| grant.role == Role::Admin && grant.target == GrantTarget::AllAssets)
}

/// The modes the user may switch to: Driver for anyone above Viewer, Maintainer for anyone
/// who is a Maintainer somewhere, Admin only for the household's Admin. Empty for a
/// Viewer-only user, who works read-only.
#[must_use]
pub fn available_modes(grants: &[RoleGrant]) -> Vec<ActiveMode> {
    let highest = highest_role(grants);
    [
        (ActiveMode::Driver, highest >= Some(Role::Driver)),
        (ActiveMode::Maintainer, highest >= Some(Role::Maintainer)),
        (ActiveMode::Admin, is_household_admin(grants)),
    ]
    .into_iter()
    .filter_map(|(mode, allowed)| allowed.then_some(mode))
    .collect()
}

/// Whether the user, in `mode`, may do `action` on `asset`. Viewing needs only a grant; every
/// other action needs both the role on that asset and a mode that allows it.
#[must_use]
pub fn permits_on_asset(
    grants: &[RoleGrant],
    mode: ActiveMode,
    action: AssetAction,
    asset: AssetId,
) -> bool {
    let Some(role) = role_on(grants, asset) else {
        return false;
    };
    let required = action.required_role();
    // Viewing isn't limited by mode: every mode can see what the user's grants cover.
    let allowed = if required == Role::Viewer {
        role
    } else {
        role.min(mode.cap())
    };
    allowed >= required
}

/// Whether the user, in `mode`, may do a household-wide `action`.
#[must_use]
pub fn permits_admin(grants: &[RoleGrant], mode: ActiveMode, _action: AdminAction) -> bool {
    mode == ActiveMode::Admin && is_household_admin(grants)
}

/// Whether at least one enabled user would still be the household's Admin after a change.
/// `grants` are everyone's grants after the change; `is_enabled` says which users can sign in.
/// The server refuses any change (removing a grant, demoting, disabling, deleting a user)
/// for which this is false.
#[must_use]
pub fn leaves_an_admin(grants: &[RoleGrant], is_enabled: impl Fn(UserId) -> bool) -> bool {
    grants.iter().any(|grant| {
        grant.role == Role::Admin
            && grant.target == GrantTarget::AllAssets
            && is_enabled(grant.user)
    })
}

/// The changeable fields of a [`RoleGrant`]. Grants are created and deleted, never edited:
/// changing someone's role is a delete plus a create, so it's one clear entry in the history.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RoleGrantPatch {
    /// See [`RoleGrant::user`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserId>,
    /// See [`RoleGrant::target`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub target: Option<GrantTarget>,
    /// See [`RoleGrant::role`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<Role>,
}

impl RoleGrantPatch {
    /// Every field of `grant`, for creating it.
    #[must_use]
    pub const fn all(grant: RoleGrant) -> Self {
        Self {
            user: Some(grant.user),
            target: Some(grant.target),
            role: Some(grant.role),
        }
    }
}

impl Patch for RoleGrantPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("user", self.user.is_some()),
            ("target", self.target.is_some()),
            ("role", self.role.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, set)| set.then_some(field))
        .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        if !fields.contains(&"user") {
            self.user = None;
        }
        if !fields.contains(&"target") {
            self.target = None;
        }
        if !fields.contains(&"role") {
            self.role = None;
        }
    }
}

impl Record for RoleGrant {
    const ENTITY: &'static str = "role_grant";
    type Patch = RoleGrantPatch;
}

/// The ID of a role grant.
pub type RoleGrantId = Id<RoleGrant>;

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::test_support::test_id;

    const TRUCK: u32 = 10;
    const CAR: u32 = 11;

    fn grant(target: GrantTarget, role: Role) -> RoleGrant {
        RoleGrant {
            user: test_id(1),
            target,
            role,
        }
    }

    fn on(asset: u32, role: Role) -> RoleGrant {
        grant(GrantTarget::Asset(test_id(asset)), role)
    }

    fn everywhere(role: Role) -> RoleGrant {
        grant(GrantTarget::AllAssets, role)
    }

    #[test]
    fn the_highest_covering_grant_wins() {
        let grants = [everywhere(Role::Driver), on(TRUCK, Role::Maintainer)];
        assert_eq!(role_on(&grants, test_id(TRUCK)), Some(Role::Maintainer));
        assert_eq!(role_on(&grants, test_id(CAR)), Some(Role::Driver));
        assert_eq!(role_on(&[on(TRUCK, Role::Admin)], test_id(CAR)), None);
    }

    #[test]
    fn all_assets_covers_assets_added_later() {
        let grants = [everywhere(Role::Maintainer)];
        let new_asset = test_id(999);
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::LogMaintenance,
            new_asset
        ));
    }

    #[test]
    fn the_mode_caps_what_a_role_can_do() {
        let grants = [everywhere(Role::Admin)];
        let truck = test_id(TRUCK);
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Driver,
            AssetAction::LogFuel,
            truck
        ));
        assert!(!permits_on_asset(
            &grants,
            ActiveMode::Driver,
            AssetAction::LogMaintenance,
            truck
        ));
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::LogMaintenance,
            truck
        ));
        assert!(!permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::ManageAsset,
            truck
        ));
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Admin,
            AssetAction::ManageAsset,
            truck
        ));
    }

    #[test]
    fn per_vehicle_roles_still_apply_inside_a_mode() {
        // Maintainer on the truck, Driver on the car: in Maintainer mode, the car stays at Driver.
        let grants = [on(TRUCK, Role::Maintainer), on(CAR, Role::Driver)];
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::LogMaintenance,
            test_id(TRUCK)
        ));
        assert!(!permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::LogMaintenance,
            test_id(CAR)
        ));
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Maintainer,
            AssetAction::LogFuel,
            test_id(CAR)
        ));
    }

    #[test]
    fn viewers_can_view_in_any_mode_and_do_nothing_else() {
        let grants = [everywhere(Role::Viewer)];
        let truck = test_id(TRUCK);
        assert!(permits_on_asset(
            &grants,
            ActiveMode::Driver,
            AssetAction::View,
            truck
        ));
        assert!(!permits_on_asset(
            &grants,
            ActiveMode::Driver,
            AssetAction::LogFuel,
            truck
        ));
        assert!(
            available_modes(&grants).is_empty(),
            "Viewer-only users work read-only"
        );
    }

    #[test]
    fn available_modes_follow_the_roles_held() {
        assert_eq!(
            available_modes(&[on(TRUCK, Role::Driver)]),
            vec![ActiveMode::Driver]
        );
        assert_eq!(
            available_modes(&[on(TRUCK, Role::Maintainer)]),
            vec![ActiveMode::Driver, ActiveMode::Maintainer]
        );
        // Admin of one vehicle isn't the household's Admin.
        assert_eq!(
            available_modes(&[on(TRUCK, Role::Admin)]),
            vec![ActiveMode::Driver, ActiveMode::Maintainer]
        );
        assert_eq!(
            available_modes(&[everywhere(Role::Admin)]),
            vec![
                ActiveMode::Driver,
                ActiveMode::Maintainer,
                ActiveMode::Admin
            ]
        );
    }

    #[test]
    fn household_actions_need_admin_on_all_assets_in_admin_mode() {
        let admin = [everywhere(Role::Admin)];
        assert!(permits_admin(
            &admin,
            ActiveMode::Admin,
            AdminAction::ManageUsers
        ));
        assert!(!permits_admin(
            &admin,
            ActiveMode::Maintainer,
            AdminAction::ManageUsers
        ));
        assert!(!permits_admin(
            &[on(TRUCK, Role::Admin)],
            ActiveMode::Admin,
            AdminAction::ManageUsers
        ));
    }

    #[test]
    fn there_is_always_an_enabled_admin() {
        let admin_id = test_id(1);
        let other = test_id(2);
        let admin = RoleGrant {
            user: admin_id,
            target: GrantTarget::AllAssets,
            role: Role::Admin,
        };
        let maintainer = RoleGrant {
            user: other,
            target: GrantTarget::AllAssets,
            role: Role::Maintainer,
        };

        assert!(leaves_an_admin(&[admin, maintainer], |_| true));
        assert!(
            !leaves_an_admin(&[maintainer], |_| true),
            "removing the last Admin's grant"
        );
        assert!(
            !leaves_an_admin(&[admin, maintainer], |user| user != admin_id),
            "disabling the last Admin"
        );
        let one_vehicle_admin = RoleGrant {
            target: GrantTarget::Asset(test_id(TRUCK)),
            ..admin
        };
        assert!(!leaves_an_admin(&[one_vehicle_admin], |_| true));
    }

    #[test]
    fn grants_round_trip_through_json() {
        let grant = on(TRUCK, Role::Maintainer);
        let json = serde_json::to_string(&grant).unwrap();
        assert!(
            json.contains(r#""kind":"asset""#) && json.contains(r#""role":"maintainer""#),
            "{json}"
        );
        assert_eq!(serde_json::from_str::<RoleGrant>(&json).unwrap(), grant);
        let all = serde_json::to_string(&GrantTarget::AllAssets).unwrap();
        assert_eq!(all, r#"{"kind":"all-assets"}"#);
    }

    // --- Properties ------------------------------------------------------------------

    fn role() -> impl Strategy<Value = Role> {
        prop_oneof![
            Just(Role::Viewer),
            Just(Role::Driver),
            Just(Role::Maintainer),
            Just(Role::Admin)
        ]
    }

    fn mode() -> impl Strategy<Value = ActiveMode> {
        prop_oneof![
            Just(ActiveMode::Driver),
            Just(ActiveMode::Maintainer),
            Just(ActiveMode::Admin)
        ]
    }

    fn action() -> impl Strategy<Value = AssetAction> {
        prop_oneof![
            Just(AssetAction::View),
            Just(AssetAction::LogFuel),
            Just(AssetAction::LogReading),
            Just(AssetAction::OpenSquawk),
            Just(AssetAction::LogMaintenance),
            Just(AssetAction::DispositionSquawk),
            Just(AssetAction::ConfigureMaintenance),
            Just(AssetAction::ManageAsset),
        ]
    }

    /// A grant on all assets, or on one of three assets.
    fn any_grant() -> impl Strategy<Value = RoleGrant> {
        (prop::option::of(0..3_u32), role()).prop_map(|(asset, role)| match asset {
            None => everywhere(role),
            Some(asset) => on(TRUCK + asset, role),
        })
    }

    proptest! {
        #[test]
        fn more_grants_never_take_a_permission_away(
            grants in prop::collection::vec(any_grant(), 0..6),
            extra in any_grant(),
            mode in mode(),
            action in action(),
            asset in 0..3_u32,
        ) {
            let asset = test_id(TRUCK + asset);
            let before = permits_on_asset(&grants, mode, action, asset);
            let mut more = grants;
            more.push(extra);
            prop_assert!(!before || permits_on_asset(&more, mode, action, asset));
        }

        #[test]
        fn a_higher_mode_never_takes_a_permission_away(
            grants in prop::collection::vec(any_grant(), 0..6),
            action in action(),
            asset in 0..3_u32,
        ) {
            let asset = test_id(TRUCK + asset);
            let driver = permits_on_asset(&grants, ActiveMode::Driver, action, asset);
            let maintainer = permits_on_asset(&grants, ActiveMode::Maintainer, action, asset);
            let admin = permits_on_asset(&grants, ActiveMode::Admin, action, asset);
            prop_assert!(!driver || maintainer);
            prop_assert!(!maintainer || admin);
        }

        #[test]
        fn driver_mode_never_does_maintainer_work(
            grants in prop::collection::vec(any_grant(), 0..6),
            action in action(),
            asset in 0..3_u32,
        ) {
            if action.required_role() >= Role::Maintainer {
                prop_assert!(!permits_on_asset(&grants, ActiveMode::Driver, action, test_id(TRUCK + asset)));
            }
        }
    }
}
