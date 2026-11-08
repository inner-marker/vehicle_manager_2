//! Groups: where assets live, e.g. "Garage", "Hangar" (decision #052).
//!
//! Every asset is in exactly one group. Roles can be granted on a group, covering every asset
//! in it, including ones added later (`crate::access`). A default group is made with the first
//! Admin, so every asset always has one.
//!
//! # Examples
//!
//! ```
//! use domain::group::{GroupIcon, GroupName, GroupNameError};
//!
//! assert_eq!(GroupName::parse("  Hangar 3 ").unwrap().as_str(), "Hangar 3");
//! assert_eq!(GroupName::parse("  "), Err(GroupNameError::Empty));
//! assert_eq!(GroupIcon::default(), GroupIcon::Garage);
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;

/// The ID of a group.
pub type GroupId = Id<Group>;

/// The most characters a group's name may have.
pub const GROUP_NAME_MAX_CHARS: usize = 40;

/// A group of assets.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Group {
    /// What it's called, e.g. "Garage".
    pub name: GroupName,
    /// Its picture in the Garage and menus.
    pub icon: GroupIcon,
}

/// The built-in group icons (an uploaded logo can come once images are stored, milestone 8).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupIcon {
    /// A garage (the default group's).
    #[default]
    Garage,
    /// An aircraft hangar.
    Hangar,
    /// A boathouse or marina.
    Boathouse,
    /// A storage unit.
    Storage,
    /// A warehouse.
    Warehouse,
    /// A workshop.
    Shop,
    /// A farm or barn.
    Farm,
    /// A house (e.g. a second home).
    House,
}

impl GroupIcon {
    /// Every icon, in the order the picker shows them.
    pub const ALL: [Self; 8] = [
        Self::Garage,
        Self::Hangar,
        Self::Boathouse,
        Self::Storage,
        Self::Warehouse,
        Self::Shop,
        Self::Farm,
        Self::House,
    ];
}

/// A group's name: trimmed, 1–40 characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct GroupName(String);

/// Why a group name was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum GroupNameError {
    /// Nothing was entered.
    Empty,
    /// More than [`GROUP_NAME_MAX_CHARS`] characters.
    TooLong,
}

impl GroupNameError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Empty => messages::GROUP_NAME_EMPTY,
            Self::TooLong => messages::GROUP_NAME_TOO_LONG,
        }
    }
}

impl GroupName {
    /// A group name from what was typed.
    ///
    /// # Errors
    ///
    /// [`GroupNameError`] if it's empty or too long.
    pub fn parse(input: &str) -> Result<Self, GroupNameError> {
        let name = input.trim();
        match name.chars().count() {
            0 => Err(GroupNameError::Empty),
            length if length > GROUP_NAME_MAX_CHARS => Err(GroupNameError::TooLong),
            _ => Ok(Self(name.to_owned())),
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for GroupName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<GroupName> for String {
    fn from(name: GroupName) -> Self {
        name.0
    }
}

/// The changeable fields of a [`Group`]; `Some` means "set to this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupPatch {
    /// See [`Group::name`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<GroupName>,
    /// See [`Group::icon`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub icon: Option<GroupIcon>,
}

impl GroupPatch {
    /// Every field of `group`, for creating it.
    #[must_use]
    pub fn all(group: Group) -> Self {
        Self {
            name: Some(group.name),
            icon: Some(group.icon),
        }
    }
}

impl Patch for GroupPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [("name", self.name.is_some()), ("icon", self.icon.is_some())]
            .into_iter()
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        if !fields.contains(&"name") {
            self.name = None;
        }
        if !fields.contains(&"icon") {
            self.icon = None;
        }
    }
}

impl Record for Group {
    const ENTITY: &'static str = "group";
    type Patch = GroupPatch;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_are_trimmed_and_limited() {
        assert_eq!(
            GroupName::parse("x".repeat(40).as_str()).map(|n| n.0.len()),
            Ok(40)
        );
        assert_eq!(
            GroupName::parse(&"x".repeat(41)),
            Err(GroupNameError::TooLong)
        );
    }

    #[test]
    fn every_icon_is_listed_once() {
        for (index, icon) in GroupIcon::ALL.iter().enumerate() {
            assert!(!GroupIcon::ALL[index + 1..].contains(icon), "{icon:?}");
        }
    }

    #[test]
    fn a_patch_lists_and_keeps_its_fields() {
        let mut patch = GroupPatch::all(Group {
            name: GroupName::parse("Garage").unwrap(),
            icon: GroupIcon::Garage,
        });
        assert_eq!(patch.changed_fields(), vec!["name", "icon"]);
        patch.keep_only(&["icon"]);
        assert_eq!(patch.changed_fields(), vec!["icon"]);
    }
}
