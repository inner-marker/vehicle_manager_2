//! What people type to add or change vehicles, meters, custom fields, and groups, and the
//! checks that turn it into records (decisions #030, #052).
//!
//! The app sends these forms as typed; the checks live here so the server (and later the
//! devices) apply exactly the same rules. A refused form names its [`Problem`].
//!
//! # Examples
//!
//! ```
//! use domain::asset::AssetError;
//! use domain::asset_form::{AssetForm, Problem};
//! use domain::id::Id;
//! use domain::time::UnixMillis;
//!
//! let garage = Id::generate(UnixMillis::from_millis(1_700_000_000_000), [1; 10]).unwrap();
//! let mut form = AssetForm::new("truck", garage);
//! form.name = "  ".into();
//! assert_eq!(form.parse(), Err(Problem::Asset(AssetError::NameEmpty)));
//! form.name = "Truck".into();
//! assert_eq!(form.parse().unwrap().name.as_str(), "Truck");
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::{
    Asset, AssetError, AssetKind, AssetName, Identifier, IdentifierKind, LifecycleStatus,
    check_identifiers, check_year, optional_text,
};
use crate::custom_field::{CustomFieldError, FieldKind, FieldRange, check_field};
use crate::group::{Group, GroupIcon, GroupId, GroupName, GroupNameError};
use crate::i18n::{MessageId, messages};
use crate::label::{Label, LabelError};
use crate::meter::{EventKind, MeterDimension, MeterError, MeterUnit, check_meter};
use crate::notes::{Notes, NotesError};

/// Why a form or request was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", content = "reason", rename_all = "kebab-case")]
pub enum Problem {
    /// A vehicle's details.
    Asset(AssetError),
    /// A group's name.
    GroupName(GroupNameError),
    /// A meter's or field's name.
    Label(LabelError),
    /// A meter's settings.
    Meter(MeterError),
    /// A custom field's settings.
    Field(CustomFieldError),
    /// The description.
    Notes(NotesError),
    /// There's no profile for that kind.
    UnknownKind,
    /// There's no such group.
    UnknownGroup,
    /// A group with vehicles in it can't be removed.
    GroupNotEmpty,
    /// The last group can't be removed: every vehicle needs one.
    LastGroup,
    /// The typed name doesn't match (deleting needs it typed exactly).
    NameMismatch,
}

impl Problem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Asset(error) => error.message(),
            Self::GroupName(error) => error.message(),
            Self::Label(error) => error.message(),
            Self::Meter(error) => error.message(),
            Self::Field(error) => error.message(),
            Self::Notes(error) => error.message(),
            Self::UnknownKind | Self::UnknownGroup => messages::SOMETHING_WENT_WRONG,
            Self::GroupNotEmpty => messages::GROUP_NOT_EMPTY,
            Self::LastGroup => messages::GROUP_LAST,
            Self::NameMismatch => messages::NAME_MISMATCH,
        }
    }
}

/// A vehicle's details as typed. Its kind is chosen when it's added and never changes; its
/// group changes only by moving it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetForm {
    /// The kind (a profile ID).
    pub kind: String,
    /// The name.
    pub name: String,
    /// Make (empty: none).
    pub make: String,
    /// Model (empty: none).
    pub model: String,
    /// Model year.
    pub year: Option<u16>,
    /// Colour (empty: none).
    pub colour: String,
    /// Description (Markdown subset).
    pub description: String,
    /// The group it's added to.
    pub group: GroupId,
    /// Where it is in its life.
    pub status: LifecycleStatus,
    /// Identifiers as `(kind, value)`; an empty value is left out.
    pub identifiers: Vec<(String, String)>,
}

impl AssetForm {
    /// An empty form for a new vehicle of `kind` in `group`.
    #[must_use]
    pub fn new(kind: &str, group: GroupId) -> Self {
        Self {
            kind: kind.to_owned(),
            name: String::new(),
            make: String::new(),
            model: String::new(),
            year: None,
            colour: String::new(),
            description: String::new(),
            group,
            status: LifecycleStatus::InService,
            identifiers: Vec::new(),
        }
    }

    /// The form as an asset (with no parent: components are post-MVP).
    ///
    /// # Errors
    ///
    /// The first [`Problem`] found.
    pub fn parse(&self) -> Result<Asset, Problem> {
        let identifiers = self
            .identifiers
            .iter()
            .filter(|(_, value)| !value.trim().is_empty())
            .map(|(kind, value)| Identifier::new(IdentifierKind::parse(kind)?, value))
            .collect::<Result<Vec<_>, _>>()
            .and_then(check_identifiers)
            .map_err(Problem::Asset)?;
        Ok(Asset {
            kind: AssetKind::parse(&self.kind).map_err(Problem::Asset)?,
            name: AssetName::parse(&self.name).map_err(Problem::Asset)?,
            make: optional_text(&self.make).map_err(Problem::Asset)?,
            model: optional_text(&self.model).map_err(Problem::Asset)?,
            year: self
                .year
                .map(check_year)
                .transpose()
                .map_err(Problem::Asset)?,
            colour: optional_text(&self.colour).map_err(Problem::Asset)?,
            description: Notes::parse(&self.description).map_err(Problem::Notes)?,
            group: self.group,
            parent: None,
            status: self.status,
            identifiers,
        })
    }

    /// A form showing `asset` as it is, for editing.
    #[must_use]
    pub fn from_asset(asset: &Asset) -> Self {
        Self {
            kind: asset.kind.as_str().to_owned(),
            name: asset.name.as_str().to_owned(),
            make: asset.make.clone().unwrap_or_default(),
            model: asset.model.clone().unwrap_or_default(),
            year: asset.year,
            colour: asset.colour.clone().unwrap_or_default(),
            description: asset.description.as_str().to_owned(),
            group: asset.group,
            status: asset.status,
            identifiers: asset
                .identifiers
                .iter()
                .map(|identifier| {
                    (
                        identifier.kind.as_str().to_owned(),
                        identifier.value.clone(),
                    )
                })
                .collect(),
        }
    }
}

/// A meter's settings as typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterForm {
    /// Its name.
    pub name: String,
    /// What it measures (fixed once it exists).
    pub dimension: MeterDimension,
    /// Its unit.
    pub unit: MeterUnit,
    /// Decimal places.
    pub precision: u8,
    /// What drives it (empty: none).
    pub description: String,
    /// The kinds of events that require it.
    pub required_for: Vec<EventKind>,
}

/// A meter form, checked.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParsedMeter {
    /// Its name.
    pub name: Label,
    /// What it measures.
    pub dimension: MeterDimension,
    /// Its unit.
    pub unit: MeterUnit,
    /// Decimal places.
    pub precision: u8,
    /// What drives it.
    pub description: Option<String>,
    /// The kinds of events that require it, without repeats, in order.
    pub required_for: Vec<EventKind>,
}

impl MeterForm {
    /// The form, checked.
    ///
    /// # Errors
    ///
    /// The first [`Problem`] found.
    pub fn parse(&self) -> Result<ParsedMeter, Problem> {
        let name = Label::parse(&self.name).map_err(Problem::Label)?;
        let description = check_meter(self.dimension, self.unit, self.precision, &self.description)
            .map_err(Problem::Meter)?;
        let mut required_for = self.required_for.clone();
        required_for.sort_unstable();
        required_for.dedup();
        Ok(ParsedMeter {
            name,
            dimension: self.dimension,
            unit: self.unit,
            precision: self.precision,
            description,
            required_for,
        })
    }
}

/// A custom field's settings as typed (its kind and event kind are fixed once it exists).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FieldForm {
    /// The kind of event it's on.
    pub applies_to: EventKind,
    /// Its label.
    pub label: String,
    /// What it holds.
    pub kind: FieldKind,
    /// Shown on forms.
    pub enabled: bool,
    /// Required on its kind of event.
    pub required: bool,
    /// Its sensible range, for numbers.
    pub range: Option<FieldRange>,
}

impl FieldForm {
    /// The label, after checking the whole form.
    ///
    /// # Errors
    ///
    /// The first [`Problem`] found.
    pub fn parse(&self) -> Result<Label, Problem> {
        let label = Label::parse(&self.label).map_err(Problem::Label)?;
        check_field(&self.kind, self.range).map_err(Problem::Field)?;
        Ok(label)
    }
}

/// A group's name and icon as typed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupForm {
    /// Its name.
    pub name: String,
    /// Its icon.
    pub icon: GroupIcon,
}

impl GroupForm {
    /// The form as a group.
    ///
    /// # Errors
    ///
    /// [`Problem::GroupName`].
    pub fn parse(&self) -> Result<Group, Problem> {
        Ok(Group {
            name: GroupName::parse(&self.name).map_err(Problem::GroupName)?,
            icon: self.icon,
        })
    }
}

/// Whether `typed` is the vehicle's name, as typed to confirm deleting it (case and
/// surrounding spaces ignored).
#[must_use]
pub fn confirms_name(typed: &str, name: &AssetName) -> bool {
    typed.trim().to_lowercase() == name.as_str().to_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    fn truck_form() -> AssetForm {
        AssetForm {
            name: " Truck ".into(),
            make: "Ford".into(),
            year: Some(2021),
            identifiers: vec![
                ("plate".into(), "ABC 123".into()),
                ("vin".into(), "   ".into()),
            ],
            ..AssetForm::new("truck", test_id(1))
        }
    }

    #[test]
    fn a_vehicle_form_becomes_an_asset_and_back() {
        let asset = truck_form().parse().unwrap();
        assert_eq!(asset.name.as_str(), "Truck");
        assert_eq!(asset.make.as_deref(), Some("Ford"));
        assert_eq!(asset.model, None, "empty means none");
        assert_eq!(
            asset.identifiers.len(),
            1,
            "an empty identifier is left out"
        );
        let again = AssetForm::from_asset(&asset).parse().unwrap();
        assert_eq!(again, asset);
    }

    #[test]
    fn each_part_of_a_vehicle_form_is_checked() {
        let bad_year = AssetForm {
            year: Some(1700),
            ..truck_form()
        };
        assert_eq!(
            bad_year.parse(),
            Err(Problem::Asset(AssetError::YearOutOfRange))
        );
        let bad_kind = AssetForm {
            kind: "Truck".into(),
            ..truck_form()
        };
        assert_eq!(
            bad_kind.parse(),
            Err(Problem::Asset(AssetError::InvalidKind))
        );
        let long_notes = AssetForm {
            description: "x".repeat(10_001),
            ..truck_form()
        };
        assert_eq!(long_notes.parse(), Err(Problem::Notes(NotesError::TooLong)));
    }

    #[test]
    fn a_meter_form_is_checked_and_tidied() {
        let form = MeterForm {
            name: "Engine hours".into(),
            dimension: MeterDimension::Duration,
            unit: MeterUnit::Hours,
            precision: 1,
            description: String::new(),
            required_for: vec![EventKind::Maintenance, EventKind::Fuel, EventKind::Fuel],
        };
        let meter = form.parse().unwrap();
        assert_eq!(
            meter.required_for,
            vec![EventKind::Fuel, EventKind::Maintenance]
        );
        let wrong = MeterForm {
            unit: MeterUnit::Miles,
            ..form
        };
        assert_eq!(wrong.parse(), Err(Problem::Meter(MeterError::WrongUnit)));
    }

    #[test]
    fn deleting_needs_the_name_typed() {
        let name = AssetName::parse("Truck").unwrap();
        assert!(confirms_name(" truck ", &name));
        assert!(!confirms_name("Truc", &name));
    }

    #[test]
    fn problems_survive_the_trip_to_the_app() {
        for problem in [
            Problem::Asset(AssetError::NameEmpty),
            Problem::GroupNotEmpty,
            Problem::Field(CustomFieldError::BadChoices),
        ] {
            let json = serde_json::to_string(&problem).unwrap();
            assert_eq!(serde_json::from_str::<Problem>(&json).unwrap(), problem);
        }
    }
}
