//! The checks an event passes before it's saved (milestone 6; plan §3 Events, Meters,
//! Event location; decisions #030, #033, #034). Pure functions: the server gathers what they
//! need (the vehicle's meters and fields, other events' readings) and runs them on every save;
//! the forms run them too.
//!
//! - **Location:** required, under a [`LocationRule`] whose only value now is
//!   [`LocationRule::Required`] (groundwork for an Admin setting later). Without a place, a
//!   reason is needed, and the event is flagged.
//! - **Required readings and fields:** each must be given, or skipped with a reason.
//! - **Readings fit their neighbours in time:** not lower than the meter's reading before this
//!   event's date, nor higher than the one after it, so a back-dated entry that fits is fine
//!   for anyone. One that doesn't fit is refused, except that the Admin in Admin mode may keep
//!   it with a reason (the event is flagged).
//! - **Custom fields:** each value matches its field's type; a value outside the field's range
//!   is a warning, never a refusal.
//! - **Fuel-ups:** more than zero, sensible amounts and percentages, and never towing itself.
//! - **Finalized events never change;** a correction needs a finalized original and a reason.

use serde::{Deserialize, Serialize};

use crate::command::Clearable;
use crate::custom_field::{CustomField, CustomFieldId, FieldKind, RangeCheck};
use crate::event::{
    Event, EventId, EventPatch, EventProblem, EventStatus, FieldData, FieldValue, Finalized,
    Finalizer, FuelDetails, PERFORMER_MAX_CHARS, Performer, Requirement,
};
use crate::meter::{EventKind, Meter, MeterId};
use crate::time::UnixMillis;

/// Whether events need a place (groundwork: only "required" exists now, milestone 6 choice 1).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LocationRule {
    /// A place, or a reason why there's none.
    #[default]
    Required,
}

/// What the checks need beyond the event itself.
#[derive(Debug, Clone, Copy)]
pub struct CheckContext<'a> {
    /// Whether the vehicle's kind has fuel-ups (trailers don't, #030).
    pub has_fuel: bool,
    /// The vehicle's meters.
    pub meters: &'a [(MeterId, Meter)],
    /// The vehicle's custom fields.
    pub fields: &'a [(CustomFieldId, CustomField)],
    /// Every other event's readings that count (not this event, nor an original that has been
    /// corrected): meter, when, value.
    pub history: &'a [(MeterId, UnixMillis, i64)],
    /// Whether events need a place.
    pub location_rule: LocationRule,
    /// Whether the person may keep a reading that doesn't fit (the Admin, in Admin mode).
    pub may_override_readings: bool,
}

/// Something worth telling the person that doesn't stop the save.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "warning", rename_all = "kebab-case")]
pub enum Warning {
    /// A field's value is outside its sensible range.
    OutOfRange {
        /// Which field.
        field: CustomFieldId,
        /// Above or below.
        check: RangeCheck,
    },
    /// A reading that doesn't fit its neighbours was kept by the Admin.
    ReadingKept {
        /// Which meter.
        meter: MeterId,
    },
}

/// The readings either side of `at` for `meter`: the latest at or before it, and the earliest
/// after it.
#[must_use]
pub fn neighbours(
    history: &[(MeterId, UnixMillis, i64)],
    meter: MeterId,
    at: UnixMillis,
) -> (Option<i64>, Option<i64>) {
    let of_meter = history.iter().filter(|(id, _, _)| *id == meter);
    let before = of_meter
        .clone()
        .filter(|(_, when, _)| *when <= at)
        .max_by_key(|(_, when, _)| *when)
        .map(|(_, _, value)| *value);
    let after = of_meter
        .filter(|(_, when, _)| *when > at)
        .min_by_key(|(_, when, _)| *when)
        .map(|(_, _, value)| *value);
    (before, after)
}

/// Whether `value` fits between its neighbours (either may be missing).
#[must_use]
pub fn reading_fits(value: i64, before: Option<i64>, after: Option<i64>) -> bool {
    before.is_none_or(|before| value >= before) && after.is_none_or(|after| value <= after)
}

/// Checks a whole event. Returns the warnings, or the first problem.
///
/// # Errors
///
/// The first [`EventProblem`] found.
pub fn check(event: &Event, context: &CheckContext<'_>) -> Result<Vec<Warning>, EventProblem> {
    check_kind(event, context)?;
    check_location(event, context.location_rule)?;
    if let Some(performer) = &event.performed_by {
        check_performer(performer)?;
    }
    check_fuel(event)?;
    check_required(event, context)?;
    let mut warnings = check_readings(event, context)?;
    warnings.extend(check_fields(event, context)?);
    Ok(warnings)
}

/// Fuel-ups and readings only, until maintenance and repairs arrive (milestone 7), and fuel-ups
/// only for kinds that have them.
fn check_kind(event: &Event, context: &CheckContext<'_>) -> Result<(), EventProblem> {
    match event.kind {
        EventKind::Fuel if context.has_fuel => Ok(()),
        EventKind::Reading => Ok(()),
        EventKind::Fuel | EventKind::Maintenance | EventKind::Repair => {
            Err(EventProblem::WrongKind)
        }
    }
}

/// A place, or (under the "required" rule) a reason why there's none.
fn check_location(event: &Event, rule: LocationRule) -> Result<(), EventProblem> {
    match rule {
        LocationRule::Required => {
            if event.location.place.is_some() || event.location.missing_reason.is_some() {
                Ok(())
            } else {
                Err(EventProblem::LocationRequired)
            }
        }
    }
}

/// A performer's name (1–100 characters) and contact (up to 100).
///
/// # Errors
///
/// [`EventProblem::BadPerformer`].
pub fn check_performer(performer: &Performer) -> Result<(), EventProblem> {
    match performer {
        Performer::User { .. } => Ok(()),
        Performer::Named { name, contact } => {
            let name_length = name.trim().chars().count();
            let contact_length = contact.as_deref().map_or(0, |c| c.chars().count());
            if name_length == 0
                || name_length > PERFORMER_MAX_CHARS
                || contact_length > PERFORMER_MAX_CHARS
            {
                Err(EventProblem::BadPerformer)
            } else {
                Ok(())
            }
        }
    }
}

/// A fuel-up has sensible details; a reading has none.
fn check_fuel(event: &Event) -> Result<(), EventProblem> {
    match (event.kind, &event.fuel) {
        (EventKind::Fuel, Some(fuel)) => check_fuel_details(event, fuel),
        (EventKind::Fuel, None) | (_, Some(_)) => Err(EventProblem::FuelDetails),
        (_, None) => Ok(()),
    }
}

fn check_fuel_details(event: &Event, fuel: &FuelDetails) -> Result<(), EventProblem> {
    if fuel.volume_ml <= 0 {
        return Err(EventProblem::NoVolume);
    }
    if fuel.cost_minor.is_some_and(|cost| cost < 0)
        || fuel.reported_economy.is_some_and(|economy| economy <= 0)
    {
        return Err(EventProblem::BadAmount);
    }
    if [fuel.highway_pct, fuel.towing_pct, fuel.hauling_pct]
        .iter()
        .any(|pct| *pct > 100)
    {
        return Err(EventProblem::BadPercent);
    }
    if fuel.towed.contains(&event.asset) {
        return Err(EventProblem::TowsItself);
    }
    Ok(())
}

/// Every required reading and field is given, or skipped with a reason.
fn check_required(event: &Event, context: &CheckContext<'_>) -> Result<(), EventProblem> {
    let skipped = |item: Requirement| event.skipped.iter().any(|skip| skip.item == item);
    for (id, meter) in context.meters {
        if meter.required_for.contains(&event.kind)
            && event.reading(*id).is_none()
            && !skipped(Requirement::Meter(*id))
        {
            return Err(EventProblem::Required(Requirement::Meter(*id)));
        }
    }
    for (id, field) in context.fields {
        if field.required_on(event.kind)
            && !event.fields.iter().any(|value| value.field == *id)
            && !skipped(Requirement::Field(*id))
        {
            return Err(EventProblem::Required(Requirement::Field(*id)));
        }
    }
    Ok(())
}

/// Each reading is for one of the vehicle's meters, once, and fits its neighbours in time (or
/// the Admin kept it, with a reason).
fn check_readings(event: &Event, context: &CheckContext<'_>) -> Result<Vec<Warning>, EventProblem> {
    let mut warnings = Vec::new();
    for (index, reading) in event.readings.iter().enumerate() {
        let known = context.meters.iter().any(|(id, _)| *id == reading.meter);
        let repeated = event.readings[..index]
            .iter()
            .any(|earlier| earlier.meter == reading.meter);
        if !known || repeated {
            return Err(EventProblem::UnknownMeter(reading.meter));
        }
        let (before, after) = neighbours(context.history, reading.meter, event.dates.start);
        if reading_fits(reading.value, before, after) {
            continue;
        }
        if context.may_override_readings && event.reading_override.is_some() {
            warnings.push(Warning::ReadingKept {
                meter: reading.meter,
            });
        } else {
            return Err(EventProblem::ReadingOutOfOrder(reading.meter));
        }
    }
    Ok(warnings)
}

/// Each value is for one of the vehicle's fields that's on, for this kind of event, once, and
/// of the field's type; a value outside the field's range is a warning.
fn check_fields(event: &Event, context: &CheckContext<'_>) -> Result<Vec<Warning>, EventProblem> {
    let mut warnings = Vec::new();
    for (index, value) in event.fields.iter().enumerate() {
        let field = context
            .fields
            .iter()
            .find(|(id, _)| *id == value.field)
            .map(|(_, field)| field)
            .filter(|field| field.enabled && field.applies_to == event.kind);
        let repeated = event.fields[..index]
            .iter()
            .any(|earlier| earlier.field == value.field);
        let Some(field) = field.filter(|_| !repeated) else {
            return Err(EventProblem::UnknownField(value.field));
        };
        let number = matching_number(&field.kind, value)?;
        if let (Some(number), Some(range)) = (number, field.range) {
            let check = range.check(number);
            if check != RangeCheck::Within {
                warnings.push(Warning::OutOfRange {
                    field: value.field,
                    check,
                });
            }
        }
    }
    Ok(warnings)
}

/// Checks `value` matches `kind`, and gives its number for the range check (if numeric).
fn matching_number(kind: &FieldKind, value: &FieldValue) -> Result<Option<i64>, EventProblem> {
    let wrong = Err(EventProblem::WrongValue(value.field));
    match (kind, &value.data) {
        (FieldKind::Number { .. }, FieldData::Number(n))
        | (FieldKind::Distance, FieldData::Distance(n))
        | (FieldKind::Duration, FieldData::Duration(n))
        | (FieldKind::Volume, FieldData::Volume(n))
        | (FieldKind::FuelEconomy, FieldData::FuelEconomy(n)) => Ok(Some(*n)),
        (FieldKind::Text, FieldData::Text(text)) => {
            if text.trim().is_empty() || text.chars().count() > crate::notes::NOTES_MAX_CHARS {
                wrong
            } else {
                Ok(None)
            }
        }
        (FieldKind::Date, FieldData::Date(date)) => {
            if date.parse::<jiff::civil::Date>().is_ok() {
                Ok(None)
            } else {
                wrong
            }
        }
        (FieldKind::YesNo, FieldData::YesNo(_)) => Ok(None),
        (FieldKind::Choice { options }, FieldData::Choice(choice)) => {
            if options.contains(choice) {
                Ok(None)
            } else {
                wrong
            }
        }
        _ => wrong,
    }
}

/// Whether `current` may still be changed: drafts only (#034).
///
/// # Errors
///
/// [`EventProblem::Finalized`].
pub const fn may_change(current: &Event) -> Result<(), EventProblem> {
    match current.status {
        EventStatus::Draft => Ok(()),
        EventStatus::Finalized => Err(EventProblem::Finalized),
    }
}

/// The change that finalizes an event: by `by`, at `at`.
#[must_use]
pub fn finalize(by: Finalizer, at: UnixMillis) -> EventPatch {
    EventPatch {
        status: Some(EventStatus::Finalized),
        finalized: Some(Clearable::Set(Finalized { at, by })),
        ..EventPatch::default()
    }
}

/// Checks `correction` may correct `original` (whose ID is `original_id`): the original is
/// finalized, and the correction is a new event for the same vehicle and kind that names it and
/// gives a reason.
///
/// # Errors
///
/// [`EventProblem::NotFinalized`] for a draft original (edit it instead),
/// [`EventProblem::ReasonEmpty`] without a reason, or [`EventProblem::WrongKind`] for another
/// vehicle or kind.
pub fn check_correction(
    original: &Event,
    original_id: EventId,
    correction: &Event,
) -> Result<(), EventProblem> {
    if original.status != EventStatus::Finalized {
        return Err(EventProblem::NotFinalized);
    }
    if correction.corrects != Some(original_id) || correction.correction_reason.is_none() {
        return Err(EventProblem::ReasonEmpty);
    }
    if correction.asset != original.asset || correction.kind != original.kind {
        return Err(EventProblem::WrongKind);
    }
    Ok(())
}

#[cfg(test)]
mod tests;
