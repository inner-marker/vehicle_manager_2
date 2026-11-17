//! A vehicle's specs (Phase 5 milestone 4; decisions #080, #084): the oil it takes and how
//! much, its filter's part number, lug nut torque, tire pressures.
//!
//! - **A spec** has a name, an optional **amount** in a unit of its own kind (volume, torque,
//!   pressure, or length), a detail ("5W-30 full synthetic"), a part number, and a note.
//! - **Kept as entered:** the amount is stored in thousandths of the unit it was typed in, so
//!   "5.5 qt" stays exactly that. The other system's figure ("5.2 L") is worked out to show
//!   beside it, never stored.
//! - **Requirements tick the specs their job uses** ([`crate::requirement::Requirement::specs`]).
//!   A task added for the requirement gets a part for each volume spec, and each spec with a
//!   part number but no amount ([`task_specs`]); torque, pressure, and length specs are
//!   reminders on the task.
//! - **After a job:** the parts that came from specs and were changed (part number or
//!   quantity) are offered back to their specs ([`spec_updates`]).
//!
//! # Examples
//!
//! ```
//! use domain::spec::{SpecAmount, SpecUnit};
//!
//! let oil = SpecAmount::parse("5.5", SpecUnit::Quarts).unwrap();
//! assert_eq!(oil.show(), "5.5 qt");
//! assert_eq!(oil.show_both(), "5.5 qt (5.2 L)");
//! let torque = SpecAmount::parse("150", SpecUnit::FootPounds).unwrap();
//! assert_eq!(torque.show_both(), "150 ft-lb (203 N·m)");
//! ```

use serde::{Deserialize, Serialize};
use uom::si::f64::{Length, Pressure, Torque, Volume};
use uom::si::length::{inch, millimeter};
use uom::si::pressure::{bar, kilopascal, pound_force_per_square_inch};
use uom::si::torque::{newton_meter, pound_force_foot, pound_force_inch};
use uom::si::volume::{fluid_ounce, gallon, gallon_imperial, liter, milliliter, quart_liquid};

use crate::asset::AssetId;
use crate::command::{Clearable, Record, record_patch};
use crate::event::Part;
use crate::i18n::{MessageId, Translator, messages};
use crate::id::Id;
use crate::label::Label;
use crate::notes::Notes;
use crate::quantity::{QuantityError, fixed, float, number, whole};

/// The longest detail ("5W-30 full synthetic, dexos approved").
pub const DETAIL_MAX_CHARS: usize = 100;

/// The longest part number, as on a job's part.
pub const PART_NUMBER_MAX_CHARS: usize = 40;

/// The most decimals an amount may have.
pub const AMOUNT_MAX_DECIMALS: usize = 3;

/// What a spec's amount measures.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecKind {
    /// A fluid's capacity.
    Volume,
    /// A tightening torque.
    Torque,
    /// A tire's or a system's pressure.
    Pressure,
    /// A gap or a minimum thickness.
    Length,
}

impl SpecKind {
    /// Every kind, in the dialog's order.
    pub const ALL: [Self; 4] = [Self::Volume, Self::Torque, Self::Pressure, Self::Length];

    /// The kind's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Volume => messages::SPEC_KIND_VOLUME,
            Self::Torque => messages::SPEC_KIND_TORQUE,
            Self::Pressure => messages::SPEC_KIND_PRESSURE,
            Self::Length => messages::SPEC_KIND_LENGTH,
        }
    }

    /// Its units, the usual first.
    #[must_use]
    pub fn units(self) -> Vec<SpecUnit> {
        SpecUnit::ALL
            .into_iter()
            .filter(|unit| unit.kind() == self)
            .collect()
    }
}

/// The unit a spec's amount was entered in.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecUnit {
    /// US liquid quarts.
    Quarts,
    /// Liters.
    Liters,
    /// US gallons.
    UsGallons,
    /// Imperial gallons.
    ImperialGallons,
    /// US fluid ounces.
    FluidOunces,
    /// Milliliters.
    Milliliters,
    /// Foot-pounds.
    FootPounds,
    /// Inch-pounds.
    InchPounds,
    /// Newton-meters.
    NewtonMeters,
    /// Pounds per square inch.
    Psi,
    /// Kilopascals.
    Kilopascals,
    /// Bar.
    Bar,
    /// Inches.
    Inches,
    /// Millimeters.
    Millimeters,
}

impl SpecUnit {
    /// Every unit, grouped by kind.
    pub const ALL: [Self; 14] = [
        Self::Quarts,
        Self::Liters,
        Self::UsGallons,
        Self::ImperialGallons,
        Self::FluidOunces,
        Self::Milliliters,
        Self::FootPounds,
        Self::InchPounds,
        Self::NewtonMeters,
        Self::Psi,
        Self::Kilopascals,
        Self::Bar,
        Self::Inches,
        Self::Millimeters,
    ];

    /// What it measures.
    #[must_use]
    pub const fn kind(self) -> SpecKind {
        match self {
            Self::Quarts
            | Self::Liters
            | Self::UsGallons
            | Self::ImperialGallons
            | Self::FluidOunces
            | Self::Milliliters => SpecKind::Volume,
            Self::FootPounds | Self::InchPounds | Self::NewtonMeters => SpecKind::Torque,
            Self::Psi | Self::Kilopascals | Self::Bar => SpecKind::Pressure,
            Self::Inches | Self::Millimeters => SpecKind::Length,
        }
    }

    /// Its short name ("qt", "N·m").
    #[must_use]
    pub const fn abbr(self) -> MessageId {
        match self {
            Self::Quarts => messages::UNIT_ABBR_QT,
            Self::Liters => messages::UNIT_ABBR_L,
            Self::UsGallons => messages::UNIT_ABBR_GAL,
            Self::ImperialGallons => messages::UNIT_ABBR_GAL_UK,
            Self::FluidOunces => messages::UNIT_ABBR_FL_OZ,
            Self::Milliliters => messages::UNIT_ABBR_ML,
            Self::FootPounds => messages::UNIT_ABBR_FT_LB,
            Self::InchPounds => messages::UNIT_ABBR_IN_LB,
            Self::NewtonMeters => messages::UNIT_ABBR_N_M,
            Self::Psi => messages::UNIT_ABBR_PSI,
            Self::Kilopascals => messages::UNIT_ABBR_KPA,
            Self::Bar => messages::UNIT_ABBR_BAR,
            Self::Inches => messages::UNIT_ABBR_IN,
            Self::Millimeters => messages::UNIT_ABBR_MM,
        }
    }

    /// The other system's unit, shown beside an amount, with the decimals that suit it: US
    /// and imperial units to metric, metric to US.
    #[must_use]
    pub const fn other(self) -> (Self, usize) {
        match self {
            Self::Quarts | Self::UsGallons | Self::ImperialGallons => (Self::Liters, 1),
            Self::Liters => (Self::Quarts, 1),
            Self::FluidOunces => (Self::Milliliters, 0),
            Self::Milliliters => (Self::FluidOunces, 1),
            Self::FootPounds | Self::InchPounds => (Self::NewtonMeters, 0),
            Self::NewtonMeters => (Self::FootPounds, 0),
            Self::Psi => (Self::Kilopascals, 0),
            Self::Kilopascals | Self::Bar => (Self::Psi, 0),
            Self::Inches => (Self::Millimeters, 2),
            Self::Millimeters => (Self::Inches, 3),
        }
    }

    /// `value` of this unit in the kind's base unit (liters, newton-meters, kilopascals,
    /// millimeters).
    fn to_base(self, value: f64) -> f64 {
        match self {
            // The base units themselves.
            Self::Liters | Self::NewtonMeters | Self::Kilopascals | Self::Millimeters => value,
            Self::Quarts => Volume::new::<quart_liquid>(value).get::<liter>(),
            Self::UsGallons => Volume::new::<gallon>(value).get::<liter>(),
            Self::ImperialGallons => Volume::new::<gallon_imperial>(value).get::<liter>(),
            Self::FluidOunces => Volume::new::<fluid_ounce>(value).get::<liter>(),
            Self::Milliliters => Volume::new::<milliliter>(value).get::<liter>(),
            Self::FootPounds => Torque::new::<pound_force_foot>(value).get::<newton_meter>(),
            Self::InchPounds => Torque::new::<pound_force_inch>(value).get::<newton_meter>(),
            Self::Psi => Pressure::new::<pound_force_per_square_inch>(value).get::<kilopascal>(),
            Self::Bar => Pressure::new::<bar>(value).get::<kilopascal>(),
            Self::Inches => Length::new::<inch>(value).get::<millimeter>(),
        }
    }

    /// `base` (in the kind's base unit) in this unit.
    fn in_unit(self, base: f64) -> f64 {
        match self {
            Self::Liters | Self::NewtonMeters | Self::Kilopascals | Self::Millimeters => base,
            Self::Quarts => Volume::new::<liter>(base).get::<quart_liquid>(),
            Self::UsGallons => Volume::new::<liter>(base).get::<gallon>(),
            Self::ImperialGallons => Volume::new::<liter>(base).get::<gallon_imperial>(),
            Self::FluidOunces => Volume::new::<liter>(base).get::<fluid_ounce>(),
            Self::Milliliters => Volume::new::<liter>(base).get::<milliliter>(),
            Self::FootPounds => Torque::new::<newton_meter>(base).get::<pound_force_foot>(),
            Self::InchPounds => Torque::new::<newton_meter>(base).get::<pound_force_inch>(),
            Self::Psi => Pressure::new::<kilopascal>(base).get::<pound_force_per_square_inch>(),
            Self::Bar => Pressure::new::<kilopascal>(base).get::<bar>(),
            Self::Inches => Length::new::<millimeter>(base).get::<inch>(),
        }
    }
}

/// An amount as entered: thousandths of `unit` (5.5 qt is 5,500).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SpecAmount {
    /// How much, in thousandths of the unit, more than zero.
    pub milli: i64,
    /// The unit it was entered in.
    pub unit: SpecUnit,
}

impl SpecAmount {
    /// An amount typed in `unit` ("5.5").
    ///
    /// # Errors
    ///
    /// [`QuantityError`] if it isn't a number above zero, has more than three decimals, or is
    /// too large.
    pub fn parse(text: &str, unit: SpecUnit) -> Result<Self, QuantityError> {
        let (value, decimals) = number(text)?;
        if decimals > AMOUNT_MAX_DECIMALS {
            return Err(QuantityError::TooPrecise);
        }
        let milli = whole(value * 1_000.0)?;
        if milli <= 0 {
            return Err(QuantityError::NotANumber);
        }
        Ok(Self { milli, unit })
    }

    /// The number as typed, without trailing zeros: "5.5", "150", "0.044".
    #[must_use]
    pub fn value_text(self) -> String {
        trimmed(float(self.milli) / 1_000.0, AMOUNT_MAX_DECIMALS)
    }

    /// The amount with its unit: "5.5 qt".
    #[must_use]
    pub fn show(self) -> String {
        format!(
            "{} {}",
            self.value_text(),
            Translator::english().tr(self.unit.abbr())
        )
    }

    /// The other system's figure, for show: "5.2 L" for 5.5 qt.
    #[must_use]
    pub fn other_text(self) -> String {
        let (other, decimals) = self.unit.other();
        let value = other.in_unit(self.unit.to_base(float(self.milli) / 1_000.0));
        format!(
            "{} {}",
            trimmed(value, decimals),
            Translator::english().tr(other.abbr())
        )
    }

    /// Both: "5.5 qt (5.2 L)".
    #[must_use]
    pub fn show_both(self) -> String {
        format!("{} ({})", self.show(), self.other_text())
    }
}

/// `value` with at most `decimals` places, trailing zeros dropped.
fn trimmed(value: f64, decimals: usize) -> String {
    let text = fixed(value, decimals);
    if text.contains('.') {
        text.trim_end_matches('0').trim_end_matches('.').to_owned()
    } else {
        text
    }
}

/// One spec on one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Spec {
    /// Which vehicle.
    pub asset: AssetId,
    /// Its name, e.g. "Engine oil".
    pub name: Label,
    /// How much, if it has an amount.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amount: Option<SpecAmount>,
    /// More about it, e.g. "5W-30 full synthetic" (up to 100 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail: Option<String>,
    /// Its part number, if it's a part (up to 40 characters).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub part_number: Option<String>,
    /// Notes.
    #[serde(default)]
    pub notes: Notes,
}

/// The ID of a spec.
pub type SpecId = Id<Spec>;

record_patch! {
    /// The changeable fields of a [`Spec`]; `Some` means "set to this".
    SpecPatch
    {
        /// See [`Spec::asset`] (set when created).
        asset: AssetId,
        /// See [`Spec::name`].
        name: Label,
        /// See [`Spec::amount`].
        amount: Clearable<SpecAmount>,
        /// See [`Spec::detail`].
        detail: Clearable<String>,
        /// See [`Spec::part_number`].
        part_number: Clearable<String>,
        /// See [`Spec::notes`].
        notes: Notes,
    }
}

impl SpecPatch {
    /// Every field of `spec`, for creating it.
    #[must_use]
    pub fn all(spec: Spec) -> Self {
        Self {
            asset: Some(spec.asset),
            name: Some(spec.name),
            amount: Some(Clearable::from_option(spec.amount)),
            detail: Some(Clearable::from_option(spec.detail)),
            part_number: Some(Clearable::from_option(spec.part_number)),
            notes: Some(spec.notes),
        }
    }

    /// The changes that turn `from` into `to` (only what differs).
    #[must_use]
    pub fn changes(from: &Spec, to: Spec) -> Self {
        let changed = |differs: bool| differs.then_some(());
        Self {
            asset: None,
            name: changed(from.name != to.name).map(|()| to.name),
            amount: changed(from.amount != to.amount).map(|()| Clearable::from_option(to.amount)),
            detail: changed(from.detail != to.detail).map(|()| Clearable::from_option(to.detail)),
            part_number: changed(from.part_number != to.part_number)
                .map(|()| Clearable::from_option(to.part_number)),
            notes: changed(from.notes != to.notes).map(|()| to.notes),
        }
    }
}

impl Record for Spec {
    const ENTITY: &'static str = "spec";
    type Patch = SpecPatch;
}

/// Why a spec was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SpecProblem {
    /// The detail is empty or longer than [`DETAIL_MAX_CHARS`].
    BadDetail,
    /// The part number is empty or longer than [`PART_NUMBER_MAX_CHARS`].
    BadPartNumber,
    /// The amount isn't above zero.
    BadAmount,
}

impl SpecProblem {
    /// The message explaining it.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::BadDetail => messages::SPEC_BAD_DETAIL,
            Self::BadPartNumber => messages::SPEC_BAD_PART_NUMBER,
            Self::BadAmount => messages::SPEC_BAD_AMOUNT,
        }
    }
}

/// Whether `text` is 1 to `max` characters once trimmed, and already trimmed.
fn fits(text: &str, max: usize) -> bool {
    let trimmed = text.trim();
    trimmed == text && !trimmed.is_empty() && trimmed.chars().count() <= max
}

/// Checks a spec's fields (its name is a [`Label`], checked when parsed).
///
/// # Errors
///
/// The first [`SpecProblem`] found.
pub fn check_spec(spec: &Spec) -> Result<(), SpecProblem> {
    if spec.amount.is_some_and(|amount| amount.milli <= 0) {
        return Err(SpecProblem::BadAmount);
    }
    if spec
        .detail
        .as_deref()
        .is_some_and(|detail| !fits(detail, DETAIL_MAX_CHARS))
    {
        return Err(SpecProblem::BadDetail);
    }
    if spec
        .part_number
        .as_deref()
        .is_some_and(|number| !fits(number, PART_NUMBER_MAX_CHARS))
    {
        return Err(SpecProblem::BadPartNumber);
    }
    Ok(())
}

/// The longest part description, as a job's part allows.
const PART_DESCRIPTION_MAX_CHARS: usize = 100;

/// What a spec puts on a job's task: a part, or a reminder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SpecUse {
    /// A volume, or a part number with no amount.
    Part,
    /// A torque, pressure, or length.
    Reminder,
    /// Nothing (a spec with neither an amount nor a part number).
    Nothing,
}

impl Spec {
    /// What it puts on a task.
    #[must_use]
    pub fn use_on_task(&self) -> SpecUse {
        match (
            self.amount.map(|amount| amount.unit.kind()),
            &self.part_number,
        ) {
            (Some(SpecKind::Volume), _) | (None, Some(_)) => SpecUse::Part,
            (Some(_), _) => SpecUse::Reminder,
            (None, None) => SpecUse::Nothing,
        }
    }

    /// The part it pre-fills (spec `id`): its name, detail, and a volume's unit as the
    /// description ("Engine oil 5W-30 full synthetic (qt)"), its part number, and the amount
    /// as the quantity (1 without one). `None` if it isn't a part.
    #[must_use]
    pub fn part(&self, id: SpecId) -> Option<Part> {
        if self.use_on_task() != SpecUse::Part {
            return None;
        }
        let mut description = self.name.as_str().to_owned();
        if let Some(detail) = &self.detail {
            description.push(' ');
            description.push_str(detail);
        }
        if let Some(amount) = self.amount {
            description.push_str(" (");
            description.push_str(&Translator::english().tr(amount.unit.abbr()));
            description.push(')');
        }
        Some(Part {
            description: description
                .chars()
                .take(PART_DESCRIPTION_MAX_CHARS)
                .collect(),
            part_number: self.part_number.clone(),
            quantity_milli: self.amount.map_or(1_000, |amount| amount.milli),
            unit_price_minor: None,
            spec: Some(id),
        })
    }
}

/// What a requirement's ticked specs put on a task: parts, and reminders.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TaskSpecs {
    /// The parts, pre-filled, in the requirement's order.
    pub parts: Vec<Part>,
    /// The torque, pressure, and length specs to remind of.
    pub reminders: Vec<SpecId>,
}

/// The parts and reminders for a task doing a requirement that ticks `ticked`, from the
/// vehicle's `specs` (unknown IDs are skipped).
#[must_use]
pub fn task_specs(ticked: &[SpecId], specs: &[(SpecId, Spec)]) -> TaskSpecs {
    let mut found = TaskSpecs::default();
    for id in ticked {
        let Some((_, spec)) = specs.iter().find(|(spec_id, _)| spec_id == id) else {
            continue;
        };
        match spec.use_on_task() {
            SpecUse::Part => found.parts.extend(spec.part(*id)),
            SpecUse::Reminder => found.reminders.push(*id),
            SpecUse::Nothing => {}
        }
    }
    found
}

/// A spec to bring in line with what a job used.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SpecUpdate {
    /// Which spec.
    pub spec: SpecId,
    /// Its name, for the question.
    pub name: Label,
    /// The part number the job used, if it differs (`Some(None)`: none).
    pub part_number: Option<Option<String>>,
    /// The amount the job used, if it differs (a volume spec's quantity).
    pub amount: Option<SpecAmount>,
}

impl SpecUpdate {
    /// `spec` with this update applied: the job's part number and amount.
    #[must_use]
    pub fn applied(&self, spec: &Spec) -> Spec {
        Spec {
            part_number: self
                .part_number
                .clone()
                .unwrap_or_else(|| spec.part_number.clone()),
            amount: self.amount.or(spec.amount),
            ..spec.clone()
        }
    }

    /// The change to the spec.
    #[must_use]
    pub fn patch(&self) -> SpecPatch {
        SpecPatch {
            part_number: self.part_number.clone().map(Clearable::from_option),
            amount: self.amount.map(Clearable::Set),
            ..SpecPatch::default()
        }
    }
}

/// The parts among `parts` that came from one of `specs` and differ from it now: a changed
/// part number, or for a volume spec a changed quantity. One update per spec.
#[must_use]
pub fn spec_updates(parts: &[Part], specs: &[(SpecId, Spec)]) -> Vec<SpecUpdate> {
    let mut updates: Vec<SpecUpdate> = Vec::new();
    for part in parts {
        let Some(id) = part.spec else {
            continue;
        };
        if updates.iter().any(|update| update.spec == id) {
            continue;
        }
        let Some((_, spec)) = specs.iter().find(|(spec_id, _)| *spec_id == id) else {
            continue;
        };
        let used_number = part
            .part_number
            .as_deref()
            .map(str::trim)
            .filter(|number| !number.is_empty())
            .map(str::to_owned);
        let part_number = (used_number != spec.part_number).then_some(used_number);
        let amount = spec
            .amount
            .filter(|amount| amount.unit.kind() == SpecKind::Volume)
            .filter(|amount| amount.milli != part.quantity_milli && part.quantity_milli > 0)
            .map(|amount| SpecAmount {
                milli: part.quantity_milli,
                unit: amount.unit,
            });
        if part_number.is_some() || amount.is_some() {
            updates.push(SpecUpdate {
                spec: id,
                name: spec.name.clone(),
                part_number,
                amount,
            });
        }
    }
    updates
}

#[cfg(test)]
mod tests;
