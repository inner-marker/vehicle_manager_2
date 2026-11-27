//! A household's custom roles (#27; sub-plan 05; decision #091).
//!
//! A custom role is a **name and a set of vehicle permissions** ([`Permissions`]), each at the
//! Driver or Maintainer level; never the Admin's. It's granted like a built-in role, on all
//! vehicles, a group, or one vehicle ([`crate::access::RoleGrant::custom`]). The mode picker
//! stays Driver / Maintainer / Admin: in a mode, a custom role gives its permissions at or
//! below it ([`crate::access::RoleGrant::allows`]). The built-in roles stay as they are, and
//! Admin always has every permission.
//!
//! # Examples
//!
//! ```
//! use domain::access::{AssetAction, Permissions};
//! use domain::custom_role::{CustomRole, check_custom_role};
//! use domain::label::Label;
//!
//! let chauffeur = CustomRole {
//!     name: Label::parse("Chauffeur").unwrap(),
//!     permissions: Permissions::of(&[AssetAction::LogFuel, AssetAction::LogReading]),
//! };
//! assert!(check_custom_role(&chauffeur, None, []).is_ok());
//! ```

use serde::{Deserialize, Serialize};

use crate::access::{AssetAction, Permissions};
use crate::command::{Record, record_patch};
use crate::i18n::{MessageId, Translator, messages};
use crate::id::Id;
use crate::label::Label;

/// A household's custom role.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomRole {
    /// Its name, e.g. "Chauffeur" (unique among roles, ignoring case).
    pub name: Label,
    /// What it allows (viewing always comes with it).
    pub permissions: Permissions,
}

/// The ID of a custom role.
pub type CustomRoleId = Id<CustomRole>;

record_patch! {
    /// The changeable fields of a [`CustomRole`]; `Some` means "set to this".
    CustomRolePatch
    {
        /// See [`CustomRole::name`].
        name: Label,
        /// See [`CustomRole::permissions`] (the whole set).
        permissions: Permissions,
    }
}

impl CustomRolePatch {
    /// Every field of `role`, for adding it.
    #[must_use]
    pub fn all(role: CustomRole) -> Self {
        Self {
            name: Some(role.name),
            permissions: Some(role.permissions),
        }
    }

    /// The fields that differ from `from` to `to`, for changing it.
    #[must_use]
    pub fn changes(from: &CustomRole, to: CustomRole) -> Self {
        Self {
            name: (from.name != to.name).then_some(to.name),
            permissions: (from.permissions != to.permissions).then_some(to.permissions),
        }
    }
}

impl Record for CustomRole {
    const ENTITY: &'static str = "custom_role";
    type Patch = CustomRolePatch;
}

/// Why a custom role was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RoleProblem {
    /// Another role (or a built-in one) has that name.
    NameTaken,
    /// It allows nothing beyond viewing.
    NoPermissions,
    /// It holds an Admin-only permission.
    AdminOnly,
}

impl RoleProblem {
    /// The message explaining it.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NameTaken => messages::ROLE_NAME_TAKEN,
            Self::NoPermissions => messages::ROLE_NO_PERMISSIONS,
            Self::AdminOnly => messages::HOUSEHOLD_ROLES_ADMIN_ONLY,
        }
    }
}

/// The built-in roles' names, as shown (a custom role can't take one).
fn built_in_names() -> [String; 4] {
    let english = Translator::english();
    [
        messages::ROLE_VIEWER,
        messages::ROLE_DRIVER,
        messages::ROLE_MAINTAINER,
        messages::ROLE_ADMIN,
    ]
    .map(|id| english.tr(id).to_lowercase())
}

/// Checks `role` (as `id`, if it's one already kept) against the household's `others`: a name
/// no other role has, at least one permission beyond viewing, and none of the Admin's.
///
/// # Errors
///
/// The first [`RoleProblem`] found.
pub fn check_custom_role<'a>(
    role: &CustomRole,
    id: Option<CustomRoleId>,
    others: impl IntoIterator<Item = (CustomRoleId, &'a CustomRole)>,
) -> Result<(), RoleProblem> {
    if role
        .permissions
        .actions()
        .iter()
        .any(|action| !action.customisable())
    {
        return Err(RoleProblem::AdminOnly);
    }
    if role.permissions.highest_level().is_none() {
        return Err(RoleProblem::NoPermissions);
    }
    let name = role.name.as_str().to_lowercase();
    if built_in_names().contains(&name)
        || others
            .into_iter()
            .any(|(other, kept)| Some(other) != id && kept.name.as_str().to_lowercase() == name)
    {
        return Err(RoleProblem::NameTaken);
    }
    Ok(())
}

/// The permissions a custom role may hold, Driver level first.
#[must_use]
pub fn offered_permissions() -> Vec<AssetAction> {
    let mut offered: Vec<AssetAction> = AssetAction::ALL
        .into_iter()
        .filter(|action| action.customisable() && *action != AssetAction::View)
        .collect();
    offered.sort_by_key(|action| action.required_role());
    offered
}

#[cfg(test)]
mod tests;
