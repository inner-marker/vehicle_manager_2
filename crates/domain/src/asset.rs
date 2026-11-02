//! Assets: the tracked things, called "vehicles" in the UI (plan §4 Asset, decisions #030, #052).
//!
//! An [`Asset`] holds what doesn't change over time: its kind (which selects its profile), a
//! short **name** for cards and lists, make, model, year, colour, a description, its **group**
//! (exactly one), an optional parent (where a component is installed; components are
//! post-MVP), its lifecycle status, and typed identifiers. Meter readings are never asset
//! fields: they're recorded on events.
//!
//! **Retired and disposed** assets are archived: hidden from the Garage, still viewable.
//! Deleting is only for mistakes (an Admin-only soft delete).
//!
//! # Examples
//!
//! ```
//! use domain::asset::{AssetName, Identifier, IdentifierKind, LifecycleStatus};
//!
//! assert_eq!(AssetName::parse(" Truck ").unwrap().as_str(), "Truck");
//! let plate = Identifier::new(IdentifierKind::parse("plate").unwrap(), " abc 123 ").unwrap();
//! assert_eq!(plate.value, "abc 123");
//! assert!(LifecycleStatus::InService.shows_in_garage());
//! assert!(!LifecycleStatus::Disposed.shows_in_garage());
//! ```

use serde::{Deserialize, Serialize};

use crate::command::{Clearable, Patch, Record};
use crate::group::GroupId;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::notes::Notes;

/// The ID of an asset.
pub type AssetId = Id<Asset>;

/// The most characters an asset's name may have.
pub const ASSET_NAME_MAX_CHARS: usize = 40;
/// The most characters in make, model, colour, or an identifier's value.
pub const SHORT_TEXT_MAX_CHARS: usize = 64;
/// The most identifiers an asset may have.
pub const MAX_IDENTIFIERS: usize = 10;
/// The earliest model year accepted (old equipment included).
pub const EARLIEST_MODEL_YEAR: u16 = 1800;
/// The latest model year accepted (next year's models, with room to spare).
pub const LATEST_MODEL_YEAR: u16 = 2200;

/// A tracked thing.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Asset {
    /// Its kind, which selects its profile (e.g. `car`, `generator`).
    pub kind: AssetKind,
    /// A short name for cards and lists, e.g. "Truck".
    pub name: AssetName,
    /// Make, e.g. "Ford".
    pub make: Option<String>,
    /// Model, e.g. "F-150".
    pub model: Option<String>,
    /// Model year.
    pub year: Option<u16>,
    /// Colour.
    pub colour: Option<String>,
    /// A description (Markdown subset, #055).
    pub description: Notes,
    /// Where it lives (exactly one group, #052).
    pub group: GroupId,
    /// Where it's installed, for a component (post-MVP).
    pub parent: Option<AssetId>,
    /// Where it is in its life.
    pub status: LifecycleStatus,
    /// VIN, plate, serial number, and so on.
    pub identifiers: Vec<Identifier>,
}

/// An asset kind: the ID of its profile, e.g. `car` (lowercase letters, digits, `-`).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetKind(String);

impl AssetKind {
    /// A kind from its profile ID.
    ///
    /// # Errors
    ///
    /// [`AssetError::InvalidKind`] unless it's 1–32 lowercase letters, digits, or `-`.
    pub fn parse(input: &str) -> Result<Self, AssetError> {
        slug(input).map(Self).ok_or(AssetError::InvalidKind)
    }

    /// The profile ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetKind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<AssetKind> for String {
    fn from(kind: AssetKind) -> Self {
        kind.0
    }
}

/// An asset's name: trimmed, 1–40 characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct AssetName(String);

impl AssetName {
    /// A name from what was typed.
    ///
    /// # Errors
    ///
    /// [`AssetError::NameEmpty`] or [`AssetError::NameTooLong`].
    pub fn parse(input: &str) -> Result<Self, AssetError> {
        let name = input.trim();
        match name.chars().count() {
            0 => Err(AssetError::NameEmpty),
            length if length > ASSET_NAME_MAX_CHARS => Err(AssetError::NameTooLong),
            _ => Ok(Self(name.to_owned())),
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for AssetName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<AssetName> for String {
    fn from(name: AssetName) -> Self {
        name.0
    }
}

/// Where an asset is in its life (plan §4 Asset lifecycle & components).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LifecycleStatus {
    /// In use (for a component: installed).
    #[default]
    InService,
    /// Owned, not in use.
    Spare,
    /// At a vendor or shop.
    OutForService,
    /// Kept, no longer used (archived).
    Retired,
    /// No longer owned; records kept, read-only (archived).
    Disposed,
}

impl LifecycleStatus {
    /// Every status, in order.
    pub const ALL: [Self; 5] = [
        Self::InService,
        Self::Spare,
        Self::OutForService,
        Self::Retired,
        Self::Disposed,
    ];

    /// Whether the Garage shows it; retired and disposed assets are archived.
    #[must_use]
    pub const fn shows_in_garage(self) -> bool {
        matches!(self, Self::InService | Self::Spare | Self::OutForService)
    }
}

/// What kind of identifier, from the asset's profile, e.g. `vin`, `serial`, `plate`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct IdentifierKind(String);

impl IdentifierKind {
    /// A kind from its ID.
    ///
    /// # Errors
    ///
    /// [`AssetError::InvalidIdentifierKind`] unless it's 1–32 lowercase letters, digits, or `-`.
    pub fn parse(input: &str) -> Result<Self, AssetError> {
        slug(input)
            .map(Self)
            .ok_or(AssetError::InvalidIdentifierKind)
    }

    /// The kind's ID.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for IdentifierKind {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<IdentifierKind> for String {
    fn from(kind: IdentifierKind) -> Self {
        kind.0
    }
}

/// One identifier, e.g. a plate `ABC 123`. The value is kept as typed (trimmed); checks
/// specific to a kind (a VIN's 17 characters) come from profiles later, never from code.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Identifier {
    /// What kind.
    pub kind: IdentifierKind,
    /// Its value.
    pub value: String,
}

impl Identifier {
    /// An identifier from what was typed.
    ///
    /// # Errors
    ///
    /// [`AssetError::IdentifierEmpty`] or [`AssetError::TextTooLong`].
    pub fn new(kind: IdentifierKind, value: &str) -> Result<Self, AssetError> {
        let value = value.trim();
        if value.is_empty() {
            return Err(AssetError::IdentifierEmpty);
        }
        if value.chars().count() > SHORT_TEXT_MAX_CHARS {
            return Err(AssetError::TextTooLong);
        }
        Ok(Self {
            kind,
            value: value.to_owned(),
        })
    }
}

/// Why asset details were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AssetError {
    /// The name is empty.
    NameEmpty,
    /// The name is longer than [`ASSET_NAME_MAX_CHARS`].
    NameTooLong,
    /// Make, model, colour, or an identifier is longer than [`SHORT_TEXT_MAX_CHARS`].
    TextTooLong,
    /// The year is outside [`EARLIEST_MODEL_YEAR`]–[`LATEST_MODEL_YEAR`].
    YearOutOfRange,
    /// The kind isn't a valid profile ID.
    InvalidKind,
    /// The identifier kind isn't a valid ID.
    InvalidIdentifierKind,
    /// An identifier has no value.
    IdentifierEmpty,
    /// More than [`MAX_IDENTIFIERS`] identifiers.
    TooManyIdentifiers,
}

impl AssetError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NameEmpty => messages::ASSET_NAME_EMPTY,
            Self::NameTooLong => messages::ASSET_NAME_TOO_LONG,
            Self::TextTooLong => messages::ASSET_TEXT_TOO_LONG,
            Self::YearOutOfRange => messages::ASSET_YEAR_OUT_OF_RANGE,
            Self::InvalidKind | Self::InvalidIdentifierKind => messages::SOMETHING_WENT_WRONG,
            Self::IdentifierEmpty => messages::ASSET_IDENTIFIER_EMPTY,
            Self::TooManyIdentifiers => messages::ASSET_TOO_MANY_IDENTIFIERS,
        }
    }
}

/// Optional short text (make, model, colour): trimmed; empty means none.
///
/// # Errors
///
/// [`AssetError::TextTooLong`] past [`SHORT_TEXT_MAX_CHARS`].
pub fn optional_text(input: &str) -> Result<Option<String>, AssetError> {
    let text = input.trim();
    if text.chars().count() > SHORT_TEXT_MAX_CHARS {
        return Err(AssetError::TextTooLong);
    }
    Ok((!text.is_empty()).then(|| text.to_owned()))
}

/// A model year, if it's plausible.
///
/// # Errors
///
/// [`AssetError::YearOutOfRange`] outside [`EARLIEST_MODEL_YEAR`]–[`LATEST_MODEL_YEAR`].
pub fn check_year(year: u16) -> Result<u16, AssetError> {
    (EARLIEST_MODEL_YEAR..=LATEST_MODEL_YEAR)
        .contains(&year)
        .then_some(year)
        .ok_or(AssetError::YearOutOfRange)
}

/// Checks the identifier list's size.
///
/// # Errors
///
/// [`AssetError::TooManyIdentifiers`] past [`MAX_IDENTIFIERS`].
pub fn check_identifiers(identifiers: Vec<Identifier>) -> Result<Vec<Identifier>, AssetError> {
    if identifiers.len() > MAX_IDENTIFIERS {
        Err(AssetError::TooManyIdentifiers)
    } else {
        Ok(identifiers)
    }
}

/// `input` as a stable ID: 1–32 lowercase ASCII letters, digits, or `-`.
fn slug(input: &str) -> Option<String> {
    let valid = (1..=32).contains(&input.len())
        && input
            .chars()
            .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    valid.then(|| input.to_owned())
}

/// The changeable fields of an [`Asset`]; `Some` means "change this". Optional fields
/// are cleared with [`Clearable::Clear`].
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetPatch {
    /// See [`Asset::kind`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<AssetKind>,
    /// See [`Asset::name`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<AssetName>,
    /// See [`Asset::make`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub make: Option<Clearable<String>>,
    /// See [`Asset::model`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub model: Option<Clearable<String>>,
    /// See [`Asset::year`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub year: Option<Clearable<u16>>,
    /// See [`Asset::colour`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub colour: Option<Clearable<String>>,
    /// See [`Asset::description`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Notes>,
    /// See [`Asset::group`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub group: Option<GroupId>,
    /// See [`Asset::parent`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub parent: Option<Clearable<AssetId>>,
    /// See [`Asset::status`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status: Option<LifecycleStatus>,
    /// See [`Asset::identifiers`] (the whole list is replaced).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub identifiers: Option<Vec<Identifier>>,
}

impl AssetPatch {
    /// Every field of `asset`, for creating it.
    #[must_use]
    pub fn all(asset: Asset) -> Self {
        Self {
            kind: Some(asset.kind),
            name: Some(asset.name),
            make: Some(Clearable::from_option(asset.make)),
            model: Some(Clearable::from_option(asset.model)),
            year: Some(Clearable::from_option(asset.year)),
            colour: Some(Clearable::from_option(asset.colour)),
            description: Some(asset.description),
            group: Some(asset.group),
            parent: Some(Clearable::from_option(asset.parent)),
            status: Some(asset.status),
            identifiers: Some(asset.identifiers),
        }
    }
}

/// Every field name, in a fixed order (as stored in the field clocks).
const ASSET_FIELDS: [&str; 11] = [
    "kind",
    "name",
    "make",
    "model",
    "year",
    "colour",
    "description",
    "group",
    "parent",
    "status",
    "identifiers",
];

impl Patch for AssetPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        let set = [
            self.kind.is_some(),
            self.name.is_some(),
            self.make.is_some(),
            self.model.is_some(),
            self.year.is_some(),
            self.colour.is_some(),
            self.description.is_some(),
            self.group.is_some(),
            self.parent.is_some(),
            self.status.is_some(),
            self.identifiers.is_some(),
        ];
        // Each field name paired with whether this patch sets it, keeping the set ones.
        ASSET_FIELDS
            .into_iter()
            .zip(set)
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |name: &str| fields.contains(&name);
        if !keep("kind") {
            self.kind = None;
        }
        if !keep("name") {
            self.name = None;
        }
        if !keep("make") {
            self.make = None;
        }
        if !keep("model") {
            self.model = None;
        }
        if !keep("year") {
            self.year = None;
        }
        if !keep("colour") {
            self.colour = None;
        }
        if !keep("description") {
            self.description = None;
        }
        if !keep("group") {
            self.group = None;
        }
        if !keep("parent") {
            self.parent = None;
        }
        if !keep("status") {
            self.status = None;
        }
        if !keep("identifiers") {
            self.identifiers = None;
        }
    }
}

impl Record for Asset {
    const ENTITY: &'static str = "asset";
    type Patch = AssetPatch;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    fn truck() -> Asset {
        Asset {
            kind: AssetKind::parse("truck").unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: Some("Ford".into()),
            model: Some("F-150".into()),
            year: Some(2021),
            colour: None,
            description: Notes::default(),
            group: test_id(10),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers: vec![
                Identifier::new(IdentifierKind::parse("plate").unwrap(), "ABC 123").unwrap(),
            ],
        }
    }

    #[test]
    fn names_are_trimmed_and_limited() {
        assert_eq!(AssetName::parse("  "), Err(AssetError::NameEmpty));
        assert!(AssetName::parse(&"x".repeat(ASSET_NAME_MAX_CHARS)).is_ok());
        assert_eq!(
            AssetName::parse(&"x".repeat(ASSET_NAME_MAX_CHARS + 1)),
            Err(AssetError::NameTooLong)
        );
    }

    #[test]
    fn kinds_and_identifier_kinds_are_stable_ids() {
        assert!(AssetKind::parse("other-equipment").is_ok());
        for bad in ["", "Car", "car truck", "car_1", &"x".repeat(33)] {
            assert_eq!(
                AssetKind::parse(bad),
                Err(AssetError::InvalidKind),
                "{bad:?}"
            );
        }
        assert_eq!(
            IdentifierKind::parse("VIN"),
            Err(AssetError::InvalidIdentifierKind)
        );
    }

    #[test]
    fn optional_text_is_trimmed_and_empty_means_none() {
        assert_eq!(optional_text("  Ford "), Ok(Some("Ford".into())));
        assert_eq!(optional_text("   "), Ok(None));
        assert_eq!(
            optional_text(&"x".repeat(SHORT_TEXT_MAX_CHARS + 1)),
            Err(AssetError::TextTooLong)
        );
    }

    #[test]
    fn years_and_identifier_counts_are_checked() {
        assert_eq!(check_year(2021), Ok(2021));
        assert_eq!(check_year(1799), Err(AssetError::YearOutOfRange));
        assert_eq!(check_year(2201), Err(AssetError::YearOutOfRange));
        let plate = truck().identifiers[0].clone();
        assert!(check_identifiers(vec![plate.clone(); MAX_IDENTIFIERS]).is_ok());
        assert_eq!(
            check_identifiers(vec![plate; MAX_IDENTIFIERS + 1]),
            Err(AssetError::TooManyIdentifiers)
        );
        assert_eq!(
            Identifier::new(IdentifierKind::parse("vin").unwrap(), "  "),
            Err(AssetError::IdentifierEmpty)
        );
    }

    #[test]
    fn only_assets_in_use_show_in_the_garage() {
        let shown: Vec<LifecycleStatus> = LifecycleStatus::ALL
            .into_iter()
            .filter(|status| status.shows_in_garage())
            .collect();
        assert_eq!(
            shown,
            vec![
                LifecycleStatus::InService,
                LifecycleStatus::Spare,
                LifecycleStatus::OutForService
            ]
        );
    }

    #[test]
    fn a_patch_can_clear_an_optional_field() {
        let patch = AssetPatch {
            colour: Some(Clearable::Clear),
            ..AssetPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["colour"]);
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(json, r#"{"colour":{"to":"clear"}}"#);
        assert_eq!(serde_json::from_str::<AssetPatch>(&json).unwrap(), patch);
    }

    #[test]
    fn a_create_sets_every_field_and_survives_the_trip() {
        let patch = AssetPatch::all(truck());
        assert_eq!(patch.changed_fields(), ASSET_FIELDS.to_vec());
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(serde_json::from_str::<AssetPatch>(&json).unwrap(), patch);
    }

    #[test]
    fn keep_only_drops_the_fields_that_lost() {
        let mut patch = AssetPatch::all(truck());
        patch.keep_only(&["name", "group"]);
        assert_eq!(patch.changed_fields(), vec!["name", "group"]);
    }
}
