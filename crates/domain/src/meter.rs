//! Meters and the kinds of events (plan §4 Meters, Required readings; decision #030).
//!
//! An asset tracks usage on any number of **meters**: an odometer, run hours, engine hours,
//! and so on. A meter measures a **distance** or a **duration**, shown in its own unit with a
//! number of decimal places. Each meter says which kinds of events **require** a reading of
//! it (e.g. a fuel-up requires the odometer). Readings themselves are recorded on events
//! (milestone 6); their values arrive with `uom` then.
//!
//! # Examples
//!
//! ```
//! use domain::meter::{MeterDimension, MeterUnit};
//!
//! assert_eq!(MeterUnit::Kilometers.dimension(), MeterDimension::Distance);
//! assert!(MeterUnit::Hours.fits(MeterDimension::Duration));
//! assert!(!MeterUnit::Miles.fits(MeterDimension::Duration));
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::{AssetId, optional_text};
use crate::command::{Clearable, Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;

/// The ID of a meter.
pub type MeterId = Id<Meter>;

/// The most decimal places a meter shows (e.g. tenths of an hour: 1).
pub const MAX_PRECISION: u8 = 2;

/// A kind of event. Events themselves arrive in milestone 6; profiles, meters, and custom
/// fields refer to these kinds already.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EventKind {
    /// A fuel-up.
    Fuel,
    /// A meter reading on its own.
    Reading,
    /// Scheduled maintenance.
    Maintenance,
    /// A repair.
    Repair,
}

impl EventKind {
    /// Every kind, in order.
    pub const ALL: [Self; 4] = [Self::Fuel, Self::Reading, Self::Maintenance, Self::Repair];

    /// Whether it finalizes by itself, 24 hours after its last edit (fuel-ups and readings);
    /// maintenance and repairs lock only when finished (#034).
    #[must_use]
    pub const fn finalizes_by_itself(self) -> bool {
        matches!(self, Self::Fuel | Self::Reading)
    }

    /// The kind's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Fuel => messages::EVENT_KIND_FUEL,
            Self::Reading => messages::EVENT_KIND_READING,
            Self::Maintenance => messages::EVENT_KIND_MAINTENANCE,
            Self::Repair => messages::EVENT_KIND_REPAIR,
        }
    }
}

/// What a meter measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterDimension {
    /// A distance (stored in meters).
    Distance,
    /// A duration (stored in seconds).
    Duration,
}

/// The unit a meter is shown in, matching its gauge.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterUnit {
    /// Miles.
    Miles,
    /// Kilometers.
    Kilometers,
    /// Hours.
    Hours,
}

impl MeterUnit {
    /// Every unit, in order.
    pub const ALL: [Self; 3] = [Self::Miles, Self::Kilometers, Self::Hours];

    /// What the unit measures.
    #[must_use]
    pub const fn dimension(self) -> MeterDimension {
        match self {
            Self::Miles | Self::Kilometers => MeterDimension::Distance,
            Self::Hours => MeterDimension::Duration,
        }
    }

    /// Whether the unit can show a meter of `dimension`.
    #[must_use]
    pub fn fits(self, dimension: MeterDimension) -> bool {
        self.dimension() == dimension
    }

    /// The unit's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Miles => messages::UNIT_MILES,
            Self::Kilometers => messages::UNIT_KILOMETERS,
            Self::Hours => messages::UNIT_HOURS,
        }
    }
}

/// One meter on one asset.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meter {
    /// Whose meter.
    pub asset: AssetId,
    /// Its name, e.g. "Odometer".
    pub name: Label,
    /// What it measures; fixed once the meter exists (readings depend on it).
    pub dimension: MeterDimension,
    /// The unit it's shown in.
    pub unit: MeterUnit,
    /// Decimal places shown, 0–2.
    pub precision: u8,
    /// What drives it, to tell similar hour meters apart.
    pub description: Option<String>,
    /// The kinds of events that require a reading of it.
    pub required_for: Vec<EventKind>,
}

/// Why a meter's settings were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterError {
    /// The unit doesn't measure the meter's dimension (e.g. hours for a distance).
    WrongUnit,
    /// More than [`MAX_PRECISION`] decimal places.
    TooPrecise,
    /// The description is too long.
    DescriptionTooLong,
}

impl MeterError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::WrongUnit => messages::METER_WRONG_UNIT,
            Self::TooPrecise => messages::METER_TOO_PRECISE,
            Self::DescriptionTooLong => messages::ASSET_TEXT_TOO_LONG,
        }
    }
}

/// Checks a meter's unit, precision, and description together.
///
/// # Errors
///
/// [`MeterError`] for the first problem found.
pub fn check_meter(
    dimension: MeterDimension,
    unit: MeterUnit,
    precision: u8,
    description: &str,
) -> Result<Option<String>, MeterError> {
    if !unit.fits(dimension) {
        return Err(MeterError::WrongUnit);
    }
    if precision > MAX_PRECISION {
        return Err(MeterError::TooPrecise);
    }
    optional_text(description).map_err(|_| MeterError::DescriptionTooLong)
}

/// The changeable fields of a [`Meter`]; `Some` means "change this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MeterPatch {
    /// See [`Meter::asset`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<AssetId>,
    /// See [`Meter::name`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub name: Option<Label>,
    /// See [`Meter::dimension`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dimension: Option<MeterDimension>,
    /// See [`Meter::unit`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<MeterUnit>,
    /// See [`Meter::precision`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub precision: Option<u8>,
    /// See [`Meter::description`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<Clearable<String>>,
    /// See [`Meter::required_for`] (the whole list is replaced).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_for: Option<Vec<EventKind>>,
}

impl MeterPatch {
    /// Every field of `meter`, for creating it.
    #[must_use]
    pub fn all(meter: Meter) -> Self {
        Self {
            asset: Some(meter.asset),
            name: Some(meter.name),
            dimension: Some(meter.dimension),
            unit: Some(meter.unit),
            precision: Some(meter.precision),
            description: Some(Clearable::from_option(meter.description)),
            required_for: Some(meter.required_for),
        }
    }
}

/// Every field name, in a fixed order.
const METER_FIELDS: [&str; 7] = [
    "asset",
    "name",
    "dimension",
    "unit",
    "precision",
    "description",
    "required_for",
];

impl Patch for MeterPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        let set = [
            self.asset.is_some(),
            self.name.is_some(),
            self.dimension.is_some(),
            self.unit.is_some(),
            self.precision.is_some(),
            self.description.is_some(),
            self.required_for.is_some(),
        ];
        // Each field name paired with whether this patch sets it, keeping the set ones.
        METER_FIELDS
            .into_iter()
            .zip(set)
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |name: &str| fields.contains(&name);
        if !keep("asset") {
            self.asset = None;
        }
        if !keep("name") {
            self.name = None;
        }
        if !keep("dimension") {
            self.dimension = None;
        }
        if !keep("unit") {
            self.unit = None;
        }
        if !keep("precision") {
            self.precision = None;
        }
        if !keep("description") {
            self.description = None;
        }
        if !keep("required_for") {
            self.required_for = None;
        }
    }
}

impl Record for Meter {
    const ENTITY: &'static str = "meter";
    type Patch = MeterPatch;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    #[test]
    fn units_match_their_dimension() {
        for unit in MeterUnit::ALL {
            let other = match unit.dimension() {
                MeterDimension::Distance => MeterDimension::Duration,
                MeterDimension::Duration => MeterDimension::Distance,
            };
            assert!(unit.fits(unit.dimension()) && !unit.fits(other), "{unit:?}");
        }
    }

    #[test]
    fn meter_settings_are_checked() {
        let check = |unit, precision| check_meter(MeterDimension::Duration, unit, precision, "");
        assert_eq!(check(MeterUnit::Hours, 1), Ok(None));
        assert_eq!(check(MeterUnit::Miles, 0), Err(MeterError::WrongUnit));
        assert_eq!(check(MeterUnit::Hours, 3), Err(MeterError::TooPrecise));
        assert_eq!(
            check_meter(
                MeterDimension::Distance,
                MeterUnit::Miles,
                0,
                " Dash gauge "
            ),
            Ok(Some("Dash gauge".into()))
        );
    }

    #[test]
    fn a_create_sets_every_field_and_survives_the_trip() {
        let patch = MeterPatch::all(Meter {
            asset: test_id(1),
            name: Label::parse("Odometer").unwrap(),
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            precision: 0,
            description: None,
            required_for: vec![EventKind::Fuel],
        });
        assert_eq!(patch.changed_fields(), METER_FIELDS.to_vec());
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(serde_json::from_str::<MeterPatch>(&json).unwrap(), patch);
    }
}
