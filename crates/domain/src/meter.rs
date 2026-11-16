//! Meters and the kinds of events (plan §4 Meters, Required readings; decision #030).
//!
//! An asset tracks usage on any number of **meters**: an odometer, run hours, engine hours,
//! and so on. A meter measures a **distance**, a **duration**, or a **percentage** (Phase 5,
//! decision #080), shown in its own unit with a number of decimal places. It **counts up** (an
//! odometer) or **down** (a car's Oil life, 100% toward 0%), and may be **resettable** to a
//! value (Oil life back to 100%); a reading can then be marked as a reset (`event::Reading`). Each meter says which kinds of events **require** a reading of
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

/// A kind of event: the everyday ones a profile chooses among, and the lifecycle ones every
/// vehicle has (milestone 9).
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
    /// The vehicle was acquired (milestone 9, #067).
    Acquired,
    /// It went out for service.
    SentOut,
    /// It came back from service.
    Returned,
    /// It was disposed of.
    Disposed,
}

impl EventKind {
    /// Every kind, in order.
    pub const ALL: [Self; 8] = [
        Self::Fuel,
        Self::Reading,
        Self::Maintenance,
        Self::Repair,
        Self::Acquired,
        Self::SentOut,
        Self::Returned,
        Self::Disposed,
    ];

    /// The everyday kinds, the ones a profile chooses among (a trailer has no fuel-ups).
    pub const EVERYDAY: [Self; 4] = [Self::Fuel, Self::Reading, Self::Maintenance, Self::Repair];

    /// The lifecycle kinds (#067), which every vehicle has.
    pub const LIFECYCLE: [Self; 4] = [
        Self::Acquired,
        Self::SentOut,
        Self::Returned,
        Self::Disposed,
    ];

    /// Whether it's a lifecycle event (acquired, sent out, returned, disposed).
    #[must_use]
    pub const fn is_lifecycle(self) -> bool {
        matches!(
            self,
            Self::Acquired | Self::SentOut | Self::Returned | Self::Disposed
        )
    }

    /// Whether it finalizes by itself, 24 hours after its last edit (fuel-ups, readings, and
    /// lifecycle events); maintenance and repairs lock only when finished (#034).
    #[must_use]
    pub const fn finalizes_by_itself(self) -> bool {
        !matches!(self, Self::Maintenance | Self::Repair)
    }

    /// The kind's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Fuel => messages::EVENT_KIND_FUEL,
            Self::Reading => messages::EVENT_KIND_READING,
            Self::Maintenance => messages::EVENT_KIND_MAINTENANCE,
            Self::Repair => messages::EVENT_KIND_REPAIR,
            Self::Acquired => messages::EVENT_KIND_ACQUIRED,
            Self::SentOut => messages::EVENT_KIND_SENT_OUT,
            Self::Returned => messages::EVENT_KIND_RETURNED,
            Self::Disposed => messages::EVENT_KIND_DISPOSED,
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
    /// A percentage (stored in hundredths of a percent: 100% is 10,000).
    Percentage,
}

/// Which way a meter's readings go between resets (decision #080).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeterDirection {
    /// Up, like an odometer (every meter made before Phase 5).
    #[default]
    Up,
    /// Down, like a car's Oil life.
    Down,
}

impl MeterDirection {
    /// Both directions, in the order the form offers them.
    pub const ALL: [Self; 2] = [Self::Up, Self::Down];

    /// Whether `next` may follow `earlier` (equal always may: Oil life can sit at 0%).
    #[must_use]
    pub const fn allows(self, earlier: i64, next: i64) -> bool {
        match self {
            Self::Up => next >= earlier,
            Self::Down => next <= earlier,
        }
    }

    /// The direction's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Up => messages::METER_COUNTS_UP,
            Self::Down => messages::METER_COUNTS_DOWN,
        }
    }
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
    /// Percent.
    Percent,
}

impl MeterUnit {
    /// Every unit, in order.
    pub const ALL: [Self; 4] = [Self::Miles, Self::Kilometers, Self::Hours, Self::Percent];

    /// What the unit measures.
    #[must_use]
    pub const fn dimension(self) -> MeterDimension {
        match self {
            Self::Miles | Self::Kilometers => MeterDimension::Distance,
            Self::Hours => MeterDimension::Duration,
            Self::Percent => MeterDimension::Percentage,
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
            Self::Percent => messages::UNIT_PERCENT,
        }
    }

    /// The unit's short name, after a number: "mi", "km", "h", "%".
    #[must_use]
    pub const fn abbr(self) -> MessageId {
        match self {
            Self::Miles => messages::UNIT_ABBR_MI,
            Self::Kilometers => messages::UNIT_ABBR_KM,
            Self::Hours => messages::UNIT_ABBR_H,
            Self::Percent => messages::UNIT_ABBR_PERCENT,
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
    /// Which way it counts; fixed once it has a reading (Phase 5, #080).
    #[serde(default)]
    pub direction: MeterDirection,
    /// What a reset sets it to, in the stored unit; `None`: it can't be reset.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_to: Option<i64>,
}

impl Meter {
    /// Whether it can be reset.
    #[must_use]
    pub const fn resettable(&self) -> bool {
        self.reset_to.is_some()
    }
}

/// The reset value a new resettable meter starts with: 100% for a percentage counting down,
/// otherwise zero.
#[must_use]
pub const fn default_reset(dimension: MeterDimension, direction: MeterDirection) -> i64 {
    match (dimension, direction) {
        (MeterDimension::Percentage, MeterDirection::Down) => 10_000,
        _ => 0,
    }
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
    /// A negative reset value.
    NegativeReset,
    /// A distance meter fuel-ups require can't be reset: fuel economy needs its running total.
    ResettableFuelMeter,
    /// The direction can't change once the meter has a reading (checked by the server).
    DirectionFixed,
}

impl MeterError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::WrongUnit => messages::METER_WRONG_UNIT,
            Self::TooPrecise => messages::METER_TOO_PRECISE,
            Self::DescriptionTooLong => messages::ASSET_TEXT_TOO_LONG,
            Self::NegativeReset => messages::METER_NEGATIVE_RESET,
            Self::ResettableFuelMeter => messages::METER_RESETTABLE_FUEL,
            Self::DirectionFixed => messages::METER_DIRECTION_FIXED,
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

/// Checks a meter's reset value against the rest of it: never negative, and never on a
/// distance meter that fuel-ups require (fuel economy needs its running total, #080).
///
/// # Errors
///
/// [`MeterError::NegativeReset`] or [`MeterError::ResettableFuelMeter`].
pub fn check_reset(meter: &Meter) -> Result<(), MeterError> {
    let Some(value) = meter.reset_to else {
        return Ok(());
    };
    if value < 0 {
        return Err(MeterError::NegativeReset);
    }
    if meter.dimension == MeterDimension::Distance && meter.required_for.contains(&EventKind::Fuel)
    {
        return Err(MeterError::ResettableFuelMeter);
    }
    Ok(())
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
    /// See [`Meter::direction`] (fixed once the meter has a reading).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub direction: Option<MeterDirection>,
    /// See [`Meter::reset_to`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reset_to: Option<Clearable<i64>>,
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
            direction: Some(meter.direction),
            reset_to: Some(Clearable::from_option(meter.reset_to)),
        }
    }
}

/// Every field name, in a fixed order.
const METER_FIELDS: [&str; 9] = [
    "asset",
    "name",
    "dimension",
    "unit",
    "precision",
    "description",
    "required_for",
    "direction",
    "reset_to",
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
            self.direction.is_some(),
            self.reset_to.is_some(),
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
        if !keep("direction") {
            self.direction = None;
        }
        if !keep("reset_to") {
            self.reset_to = None;
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
                MeterDimension::Duration | MeterDimension::Percentage => MeterDimension::Distance,
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
            direction: MeterDirection::Up,
            reset_to: None,
        });
        assert_eq!(patch.changed_fields(), METER_FIELDS.to_vec());
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(serde_json::from_str::<MeterPatch>(&json).unwrap(), patch);
    }

    fn oil_life() -> Meter {
        Meter {
            asset: test_id(1),
            name: Label::parse("Oil life").unwrap(),
            dimension: MeterDimension::Percentage,
            unit: MeterUnit::Percent,
            precision: 0,
            description: None,
            required_for: vec![],
            direction: MeterDirection::Down,
            reset_to: Some(10_000),
        }
    }

    #[test]
    fn a_meter_stored_before_phase_5_counts_up_and_cant_be_reset() {
        let mut old = serde_json::to_value(oil_life()).unwrap();
        let fields = old.as_object_mut().unwrap();
        fields.remove("direction");
        fields.remove("reset_to");
        let meter: Meter = serde_json::from_value(old).unwrap();
        assert_eq!(meter.direction, MeterDirection::Up);
        assert!(!meter.resettable());
        let mut plain = meter;
        plain.reset_to = None;
        let json = serde_json::to_string(&plain).unwrap();
        assert!(!json.contains("reset_to"), "{json}");
    }

    #[test]
    fn directions_allow_equal_readings_and_go_their_own_way() {
        assert!(MeterDirection::Up.allows(10, 10) && MeterDirection::Up.allows(10, 11));
        assert!(!MeterDirection::Up.allows(10, 9));
        assert!(MeterDirection::Down.allows(0, 0) && MeterDirection::Down.allows(18, 5));
        assert!(!MeterDirection::Down.allows(18, 62));
    }

    #[test]
    fn resets_are_checked() {
        assert_eq!(check_reset(&oil_life()), Ok(()));
        let mut negative = oil_life();
        negative.reset_to = Some(-1);
        assert_eq!(check_reset(&negative), Err(MeterError::NegativeReset));
        let mut odometer = oil_life();
        odometer.dimension = MeterDimension::Distance;
        odometer.unit = MeterUnit::Miles;
        odometer.required_for = vec![EventKind::Fuel];
        assert_eq!(check_reset(&odometer), Err(MeterError::ResettableFuelMeter));
        odometer.reset_to = None;
        assert_eq!(check_reset(&odometer), Ok(()), "not resettable: fine");
        assert_eq!(
            default_reset(MeterDimension::Percentage, MeterDirection::Down),
            10_000
        );
        assert_eq!(
            default_reset(MeterDimension::Duration, MeterDirection::Up),
            0
        );
    }

    #[test]
    fn a_percentage_meter_uses_percent() {
        assert!(MeterUnit::Percent.fits(MeterDimension::Percentage));
        assert_eq!(
            check_meter(MeterDimension::Percentage, MeterUnit::Hours, 0, ""),
            Err(MeterError::WrongUnit)
        );
    }
}
