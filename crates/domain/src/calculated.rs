//! Calculated fields and derived meters on a vehicle (Phase 5 milestone 7; decisions #057,
//! #087): the values formulas can use, their checks, and what they work out to.
//!
//! - **Keys:** every field and meter on a vehicle has its own key ([`new_key`]), never a
//!   word formulas use themselves ([`is_reserved`]).
//! - **A calculated field** ([`check_calculated`]) uses, on each kind of event it's shown
//!   on, that event's numeric fields (not other calculated ones), its meters (derived ones
//!   too), and on a fuel-up `volume`, `cost`, `highway`, `towing`, `hauling`, and
//!   `reported-economy` ([`FUEL_VALUES`]). Its values come from [`scopes`] and
//!   [`calculate`].
//! - **A derived meter** ([`check_derived`]) uses only the vehicle's typed meters that
//!   aren't reset, numbers, `+ − × ÷`, and functions of listed values. It gets a value at
//!   every reading of one of its inputs, once each has one ([`derived_readings`]), so what's
//!   due counts on it like any other meter ([`with_derived`], [`add_derived_to_done`]).
//! - **Used by a formula:** a field or meter a formula uses can't be removed or turned off
//!   ([`used_by`]).

use std::collections::BTreeMap;

use crate::custom_field::{CustomField, CustomFieldId, FieldKey, FieldKind};
use crate::event::{Event, EventInput, FieldData, FieldValue, FuelDetails, Reading};
use crate::event_check::PastReading;
use crate::formula::{
    Context, Dimension, Expr, Formula, FormulaError, KeyInfo, NoValue, Quantity, Scope, SimpleUnit,
};
use crate::label::Label;
use crate::location::TimeZone;
use crate::meter::{EventKind, Meter, MeterDimension, MeterId};
use crate::preferences::Preferences;
use crate::requirement::DoneTask;
use crate::time::UnixMillis;

/// What a fuel-up adds for formulas, each with what it measures and whether it's a
/// percentage.
pub const FUEL_VALUES: [(&str, Dimension, bool); 6] = [
    ("volume", Dimension::VOLUME, false),
    ("cost", Dimension::MONEY, false),
    ("highway", Dimension::NONE, true),
    ("towing", Dimension::NONE, true),
    ("hauling", Dimension::NONE, true),
    ("reported-economy", Dimension::ECONOMY, false),
];

/// Words formulas use themselves, which no key may be.
const WORDS: [&str; 10] = [
    "last",
    "cumulative",
    "on",
    "event",
    "events",
    "month",
    "months",
    "abs",
    "round",
    "time-since-last",
];

/// Whether `key` is a word formulas use themselves (a function, a unit, an Over's word, or a
/// fuel-up's value), so no field or meter may have it.
#[must_use]
pub fn is_reserved(key: &str) -> bool {
    WORDS.contains(&key)
        || key == "distance-since-last"
        || crate::formula::Function::ALL
            .iter()
            .any(|function| function.name() == key)
        || SimpleUnit::from_word(key).is_some()
        || FUEL_VALUES.iter().any(|(name, ..)| *name == key)
}

/// Every key on a vehicle with these `fields` and `meters`.
fn keys_on(fields: &[(CustomFieldId, CustomField)], meters: &[(MeterId, Meter)]) -> Vec<FieldKey> {
    fields
        .iter()
        .map(|(_, field)| field.key.clone())
        .chain(meters.iter().map(|(_, meter)| meter.key.clone()))
        .collect()
}

/// A new field's or meter's key from its `label`: different from every key on the vehicle
/// and from every reserved word (by adding `-2`, `-3`, …).
#[must_use]
pub fn new_key(
    label: &Label,
    fields: &[(CustomFieldId, CustomField)],
    meters: &[(MeterId, Meter)],
) -> FieldKey {
    new_key_among(label, keys_on(fields, meters))
}

/// A key from `label`, different from each of `taken` and from every reserved word.
#[must_use]
pub fn new_key_among(label: &Label, mut taken: Vec<FieldKey>) -> FieldKey {
    let mut candidate = FieldKey::from_label(label, &taken);
    // Reserved words can't be keys: count each one the label becomes as taken.
    while is_reserved(candidate.as_str()) {
        taken.push(candidate);
        candidate = FieldKey::from_label(label, &taken);
    }
    candidate
}

/// What a meter's readings measure, as formulas see them.
const fn meter_dimension(dimension: MeterDimension) -> (Dimension, bool) {
    match dimension {
        MeterDimension::Distance => (Dimension::DISTANCE, false),
        MeterDimension::Duration => (Dimension::TIME, false),
        MeterDimension::Percentage => (Dimension::NONE, true),
    }
}

/// What a field's values measure, if they're numbers formulas can use.
const fn field_dimension(kind: &FieldKind) -> Option<Dimension> {
    match kind {
        FieldKind::Number { .. } | FieldKind::YesNo => Some(Dimension::NONE),
        FieldKind::Distance => Some(Dimension::DISTANCE),
        FieldKind::Duration => Some(Dimension::TIME),
        FieldKind::Volume => Some(Dimension::VOLUME),
        FieldKind::FuelEconomy => Some(Dimension::ECONOMY),
        FieldKind::Text
        | FieldKind::Date
        | FieldKind::Choice { .. }
        | FieldKind::Calculated { .. } => None,
    }
}

/// A reading (in its meter's stored unit) as formulas see it.
fn reading_quantity(meter: &Meter, value: i64) -> Quantity {
    let value = crate::quantity::float(value);
    match meter.dimension {
        MeterDimension::Distance => Quantity::of(value, Dimension::DISTANCE),
        MeterDimension::Duration => Quantity::of(value, Dimension::TIME),
        MeterDimension::Percentage => Quantity::percentage(value / 10_000.0),
    }
}

/// A worked-out value as a reading of `meter`, in its stored unit.
fn stored(meter: &Meter, quantity: &Quantity) -> Option<i64> {
    let value = match meter.dimension {
        MeterDimension::Distance | MeterDimension::Duration => quantity.value,
        MeterDimension::Percentage => quantity.value * 10_000.0,
    };
    crate::quantity::whole(value.round()).ok()
}

/// A field's value on an event, as formulas see it.
fn field_quantity(kind: &FieldKind, data: &FieldData) -> Option<Quantity> {
    let float = crate::quantity::float;
    Some(match (kind, data) {
        (FieldKind::Number { decimals }, FieldData::Number(n)) => {
            Quantity::number(float(*n) / 10_f64.powi(i32::from(*decimals)))
        }
        (FieldKind::YesNo, FieldData::YesNo(yes)) => Quantity::number(if *yes { 1.0 } else { 0.0 }),
        (FieldKind::Distance, FieldData::Distance(n)) => {
            Quantity::of(float(*n), Dimension::DISTANCE)
        }
        (FieldKind::Duration, FieldData::Duration(n)) => Quantity::of(float(*n), Dimension::TIME),
        (FieldKind::Volume, FieldData::Volume(n)) => {
            Quantity::of(float(*n) / 1_000.0, Dimension::VOLUME)
        }
        (FieldKind::FuelEconomy, FieldData::FuelEconomy(n)) => {
            Quantity::of(float(*n), Dimension::ECONOMY)
        }
        _ => return None,
    })
}

/// What each key gives a calculated field on events of `kind`, and whether it's a calculated
/// field itself (which formulas can't use).
fn field_key_info(
    key: &str,
    kind: EventKind,
    fields: &[(CustomFieldId, CustomField)],
    meters: &[(MeterId, Meter)],
) -> Option<KeyInfo> {
    if let Some((_, meter)) = meters.iter().find(|(_, meter)| meter.key.as_str() == key) {
        let (dimension, percent) = meter_dimension(meter.dimension);
        return Some(KeyInfo {
            dimension,
            percent,
            meter: true,
        });
    }
    if let Some((_, field)) = fields
        .iter()
        .find(|(_, field)| field.key.as_str() == key && field.shown(kind))
    {
        return field_dimension(&field.kind).map(|dimension| KeyInfo {
            dimension,
            percent: false,
            meter: false,
        });
    }
    if kind == EventKind::Fuel {
        return FUEL_VALUES.iter().find(|(name, ..)| *name == key).map(
            |(_, dimension, percent)| KeyInfo {
                dimension: *dimension,
                percent: *percent,
                meter: false,
            },
        );
    }
    None
}

/// Checks calculated field `field`'s formula on each kind of event it's shown on, against the
/// vehicle's other `fields` and its `meters`. Fine for a field that isn't calculated.
///
/// # Errors
///
/// [`FormulaError`]: as [`Formula::check_shows`], or [`FormulaError::Calculated`] for another
/// calculated field.
pub fn check_calculated(
    field: &CustomField,
    fields: &[(CustomFieldId, CustomField)],
    meters: &[(MeterId, Meter)],
) -> Result<(), FormulaError> {
    let FieldKind::Calculated { formula, shows } = &field.kind else {
        return Ok(());
    };
    check_calculated_formula(formula, *shows, &field.shown_on, fields, meters)
}

/// [`check_calculated`] for a calculated field's parts: its `formula`, what it `shows` as,
/// and the kinds of events it's `shown_on`.
///
/// # Errors
///
/// As [`check_calculated`].
pub fn check_calculated_formula(
    formula: &Formula,
    shows: crate::formula::ShowsAs,
    shown_on: &[EventKind],
    fields: &[(CustomFieldId, CustomField)],
    meters: &[(MeterId, Meter)],
) -> Result<(), FormulaError> {
    for key in formula.keys() {
        if fields
            .iter()
            .any(|(_, other)| other.key.as_str() == key && other.calculated())
        {
            return Err(FormulaError::Calculated { key });
        }
    }
    for kind in shown_on {
        let keys = |key: &str| field_key_info(key, *kind, fields, meters);
        formula.check_shows(&keys, shows)?;
    }
    Ok(())
}

/// Why a derived meter was refused.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DerivedError {
    /// Its settings: resettable, or required on events.
    Meter(crate::meter::MeterError),
    /// Its formula.
    Formula(FormulaError),
}

/// Whether `expr` has anything a derived meter can't use.
fn derived_only(expr: &Expr) -> bool {
    let mut fine = true;
    expr.walk(&mut |each| {
        fine &= !matches!(
            each,
            Expr::Round { .. } | Expr::Over { .. } | Expr::SinceLast(_) | Expr::TimeSinceLast
        );
    });
    fine
}

/// The meters a derived meter can use: the vehicle's typed ones that aren't reset (not `id`
/// itself).
fn inputs(
    id: Option<MeterId>,
    meters: &[(MeterId, Meter)],
) -> impl Iterator<Item = &(MeterId, Meter)> {
    meters
        .iter()
        .filter(move |(each, meter)| Some(*each) != id && !meter.derived() && !meter.resettable())
}

/// Checks derived meter `meter` (`id`, if it exists already) against the vehicle's `meters`.
/// Fine for a typed meter.
///
/// # Errors
///
/// [`DerivedError`]: resettable or required on events; a formula that uses anything but its
/// inputs, numbers, `+ − × ÷`, and functions of listed values; or one that doesn't give what
/// the meter measures.
pub fn check_derived(
    id: Option<MeterId>,
    meter: &Meter,
    meters: &[(MeterId, Meter)],
) -> Result<(), DerivedError> {
    let Some(formula) = &meter.formula else {
        return Ok(());
    };
    if meter.resettable() || !meter.required_for.is_empty() {
        return Err(DerivedError::Meter(
            crate::meter::MeterError::DerivedResettable,
        ));
    }
    check_derived_formula(id, formula, meter.dimension, meters).map_err(DerivedError::Formula)
}

/// [`check_derived`]'s check of a derived meter's `formula` alone, for a meter (`id`, if it
/// exists already) measuring `dimension`.
///
/// # Errors
///
/// [`FormulaError`]: anything but its inputs, numbers, `+ − × ÷`, and functions of listed
/// values, or not what the meter measures.
pub fn check_derived_formula(
    id: Option<MeterId>,
    formula: &Formula,
    dimension: MeterDimension,
    meters: &[(MeterId, Meter)],
) -> Result<(), FormulaError> {
    if !derived_only(formula.expr()) {
        return Err(FormulaError::NotInDerived);
    }
    for key in formula.keys() {
        let is_input = inputs(id, meters).any(|(_, input)| input.key.as_str() == key);
        let is_meter = meters.iter().any(|(_, other)| other.key.as_str() == key);
        if is_meter && !is_input {
            return Err(FormulaError::NotAnInput { key });
        }
    }
    let keys = |key: &str| {
        inputs(id, meters)
            .find(|(_, input)| input.key.as_str() == key)
            .map(|(_, input)| {
                let (dimension, percent) = meter_dimension(input.dimension);
                KeyInfo {
                    dimension,
                    percent,
                    meter: true,
                }
            })
    };
    let checked = formula.check(&keys)?;
    let (wanted, _) = meter_dimension(dimension);
    if checked.dimension == wanted {
        Ok(())
    } else {
        Err(FormulaError::WrongResult {
            found: checked.dimension,
            wanted,
        })
    }
}

/// Works out a derived meter's formula from its inputs' values.
fn derive(meter: &Meter, formula: &Formula, values: BTreeMap<String, Quantity>) -> Option<i64> {
    let scope = Scope {
        values,
        ..Scope::default()
    };
    // Derived formulas use neither the zone nor the units (no `round`, no Over).
    let zone = TimeZone::parse("UTC").ok()?;
    let units = Preferences::default();
    let value = formula
        .evaluate(&Context {
            current: &scope,
            earlier: &[],
            main_distance: None,
            zone: &zone,
            units: &units,
        })
        .ok()?;
    stored(meter, &value)
}

/// Each derived meter's readings from the typed `readings` (in time order): one at every
/// moment one of its inputs was read, once each input has a reading, from each one's latest.
#[must_use]
pub fn derived_readings(meters: &[(MeterId, Meter)], readings: &[PastReading]) -> Vec<PastReading> {
    let mut found = Vec::new();
    for (id, meter) in meters {
        let Some(formula) = &meter.formula else {
            continue;
        };
        let used = formula.keys();
        let inputs: Vec<&(MeterId, Meter)> = meters
            .iter()
            .filter(|(_, input)| used.contains(input.key.as_str()) && !input.derived())
            .collect();
        let mut latest: BTreeMap<String, Quantity> = BTreeMap::new();
        let mut of_inputs: Vec<&PastReading> = readings
            .iter()
            .filter(|reading| inputs.iter().any(|(input, _)| *input == reading.meter))
            .collect();
        of_inputs.sort_by_key(|reading| (reading.at, reading.reset));
        for (index, reading) in of_inputs.iter().enumerate() {
            if let Some((_, input)) = inputs.iter().find(|(input, _)| *input == reading.meter) {
                latest.insert(
                    input.key.as_str().to_owned(),
                    reading_quantity(input, reading.value),
                );
            }
            // Once per moment: after its last reading.
            let last_at_moment = of_inputs
                .get(index + 1)
                .is_none_or(|next| next.at != reading.at);
            if last_at_moment
                && latest.len() == inputs.len()
                && let Some(value) = derive(meter, formula, latest.clone())
            {
                found.push(PastReading {
                    meter: *id,
                    at: reading.at,
                    value,
                    reset: false,
                });
            }
        }
    }
    found
}

/// `readings` (in time order) with every derived meter's added, still in time order: what
/// what's due counts from.
#[must_use]
pub fn with_derived(meters: &[(MeterId, Meter)], readings: &[PastReading]) -> Vec<PastReading> {
    let mut all = readings.to_vec();
    all.extend(derived_readings(meters, readings));
    all.sort_by_key(|reading| (reading.at, reading.reset));
    all
}

/// Derived meter `meter`'s value at `at`: from `own` readings (an event's) where they have its
/// inputs, else each input's latest reading at or before `at`.
fn derived_at(
    meter: &Meter,
    meters: &[(MeterId, Meter)],
    readings: &[PastReading],
    own: &[Reading],
    at: UnixMillis,
) -> Option<i64> {
    let formula = meter.formula.as_ref()?;
    let used = formula.keys();
    let mut values = BTreeMap::new();
    for (input_id, input) in meters
        .iter()
        .filter(|(_, input)| used.contains(input.key.as_str()) && !input.derived())
    {
        let value = own
            .iter()
            .find(|reading| reading.meter == *input_id)
            .map(|reading| reading.value)
            .or_else(|| {
                readings
                    .iter()
                    .filter(|reading| reading.meter == *input_id && reading.at <= at)
                    .max_by_key(|reading| (reading.at, reading.reset))
                    .map(|reading| reading.value)
            })?;
        values.insert(
            input.key.as_str().to_owned(),
            reading_quantity(input, value),
        );
    }
    derive(meter, formula, values)
}

/// Derived meter `meter`'s value now: from each input's latest reading in `readings`.
#[must_use]
pub fn derived_now(
    meter: &Meter,
    meters: &[(MeterId, Meter)],
    readings: &[PastReading],
) -> Option<i64> {
    derived_at(
        meter,
        meters,
        readings,
        &[],
        UnixMillis::from_millis(i64::MAX),
    )
}

/// Adds each derived meter's value when each task was done to its readings, so a rule on a
/// derived meter counts from the job.
pub fn add_derived_to_done(
    done: &mut [DoneTask],
    meters: &[(MeterId, Meter)],
    readings: &[PastReading],
) {
    for task in done {
        for (id, meter) in meters.iter().filter(|(_, meter)| meter.derived()) {
            if task.readings.iter().any(|reading| reading.meter == *id) {
                continue;
            }
            if let Some(value) = derived_at(meter, meters, readings, &task.readings, task.at) {
                task.readings.push(Reading {
                    meter: *id,
                    value,
                    reset: false,
                });
            }
        }
    }
}

/// A fuel-up's own values (`FUEL_VALUES`).
fn fuel_values(fuel: Option<&FuelDetails>, values: &mut BTreeMap<String, Quantity>) {
    let Some(fuel) = fuel else {
        return;
    };
    let float = crate::quantity::float;
    let percent = |share: u8| Quantity::percentage(f64::from(share) / 100.0);
    values.insert(
        "volume".into(),
        Quantity::of(float(fuel.volume_ml) / 1_000.0, Dimension::VOLUME),
    );
    if let Some(cost) = fuel.cost_minor {
        let places = crate::quantity::currency_decimals(fuel.currency);
        values.insert(
            "cost".into(),
            Quantity::money(float(cost) / 10_f64.powi(i32::from(places)), fuel.currency),
        );
    }
    values.insert("highway".into(), percent(fuel.highway_pct));
    values.insert("towing".into(), percent(fuel.towing_pct));
    values.insert("hauling".into(), percent(fuel.hauling_pct));
    if let Some(economy) = fuel.reported_economy {
        values.insert(
            "reported-economy".into(),
            Quantity::of(float(economy), Dimension::ECONOMY),
        );
    }
}

/// A vehicle's fields and meters, and its typed readings (in time order), for working out
/// events' values.
#[derive(Debug, Clone, Copy)]
pub struct Vehicle<'a> {
    /// Its custom fields.
    pub fields: &'a [(CustomFieldId, CustomField)],
    /// Its meters.
    pub meters: &'a [(MeterId, Meter)],
    /// Its typed readings that count, in time order.
    pub readings: &'a [PastReading],
}

/// What an event (or one being typed) gives formulas: when, its readings, its field values,
/// and a fuel-up's details.
#[derive(Debug, Clone, Copy)]
pub struct Logged<'a> {
    /// When it started.
    pub at: UnixMillis,
    /// Its readings.
    pub readings: &'a [Reading],
    /// Its field values.
    pub fields: &'a [FieldValue],
    /// A fuel-up's details.
    pub fuel: Option<&'a FuelDetails>,
}

impl<'a> From<&'a Event> for Logged<'a> {
    fn from(event: &'a Event) -> Self {
        Self {
            at: event.dates.start,
            readings: &event.readings,
            fields: &event.fields,
            fuel: event.fuel.as_ref(),
        }
    }
}

impl<'a> From<&'a EventInput> for Logged<'a> {
    fn from(input: &'a EventInput) -> Self {
        Self {
            at: input.dates.start,
            readings: &input.readings,
            fields: &input.fields,
            fuel: input.fuel.as_ref(),
        }
    }
}

/// One event's own values: its fields, readings (derived meters' too), and a fuel-up's.
fn own_values(event: Logged<'_>, vehicle: &Vehicle<'_>) -> BTreeMap<String, Quantity> {
    let mut values = BTreeMap::new();
    for value in event.fields {
        let Some((_, field)) = vehicle.fields.iter().find(|(id, _)| *id == value.field) else {
            continue;
        };
        if let Some(quantity) = field_quantity(&field.kind, &value.data) {
            values.insert(field.key.as_str().to_owned(), quantity);
        }
    }
    for (id, meter) in vehicle.meters {
        let value = if meter.derived() {
            derived_at(
                meter,
                vehicle.meters,
                vehicle.readings,
                event.readings,
                event.at,
            )
        } else {
            event
                .readings
                .iter()
                .find(|reading| reading.meter == *id)
                .map(|reading| reading.value)
        };
        if let Some(value) = value {
            values.insert(
                meter.key.as_str().to_owned(),
                reading_quantity(meter, value),
            );
        }
    }
    fuel_values(event.fuel, &mut values);
    values
}

/// What formulas see on `event`, after `previous` (the one before it of its kind, if any):
/// its values, and since that one.
#[must_use]
pub fn scope_of(event: Logged<'_>, previous: Option<&Scope>, vehicle: &Vehicle<'_>) -> Scope {
    let values = own_values(event, vehicle);
    let (since_last, time_since_last) = match previous {
        Some(previous) => {
            let since: BTreeMap<String, Quantity> = vehicle
                .meters
                .iter()
                .filter_map(|(_, meter)| {
                    let key = meter.key.as_str();
                    let (now, before) = (values.get(key)?, previous.values.get(key)?);
                    Some((
                        key.to_owned(),
                        Quantity {
                            value: now.value - before.value,
                            ..*now
                        },
                    ))
                })
                .collect();
            let seconds = (event.at.as_millis() - previous.at.as_millis()) / 1_000;
            let elapsed = Quantity::of(crate::quantity::float(seconds), Dimension::TIME);
            (since, Some(elapsed))
        }
        None => (BTreeMap::new(), None),
    };
    Scope {
        at: event.at,
        values,
        since_last,
        time_since_last,
    }
}

/// What formulas see on each of `events` (one vehicle's, of one kind, in time order): its
/// values, and since the one before it.
#[must_use]
pub fn scopes(events: &[&Event], vehicle: &Vehicle<'_>) -> Vec<Scope> {
    let mut found: Vec<Scope> = Vec::with_capacity(events.len());
    for event in events {
        let scope = scope_of(Logged::from(*event), found.last(), vehicle);
        found.push(scope);
    }
    found
}

/// The key of the vehicle's main odometer (for `last 1000 mi`): the typed distance meter
/// fuel-ups require, else its first typed distance meter.
#[must_use]
pub fn main_odometer(meters: &[(MeterId, Meter)]) -> Option<&str> {
    let distance = |meter: &&Meter| meter.dimension == MeterDimension::Distance && !meter.derived();
    meters
        .iter()
        .map(|(_, meter)| meter)
        .filter(distance)
        .find(|meter| meter.required_for.contains(&EventKind::Fuel))
        .or_else(|| meters.iter().map(|(_, meter)| meter).find(distance))
        .map(|meter| meter.key.as_str())
}

/// Calculated field `field`'s value on the event at `scopes[index]` (the vehicle's events of
/// its kind, in time order), in the household's `zone`, as `units` show it. `None` if the field
/// isn't calculated.
#[must_use]
pub fn calculate(
    field: &CustomField,
    scopes: &[Scope],
    index: usize,
    meters: &[(MeterId, Meter)],
    zone: &TimeZone,
    units: &Preferences,
) -> Option<Result<Quantity, NoValue>> {
    let FieldKind::Calculated { formula, .. } = &field.kind else {
        return None;
    };
    let current = scopes.get(index)?;
    Some(formula.evaluate(&Context {
        current,
        earlier: &scopes[..index],
        main_distance: main_odometer(meters),
        zone,
        units,
    }))
}

/// The name of a calculated field or derived meter whose formula uses `key`, if any (so the
/// field or meter with that key can't be removed or turned off).
#[must_use]
pub fn used_by<'a>(
    key: &FieldKey,
    fields: &'a [(CustomFieldId, CustomField)],
    meters: &'a [(MeterId, Meter)],
) -> Option<&'a Label> {
    let uses = |formula: &Formula| formula.keys().contains(key.as_str());
    fields
        .iter()
        .find_map(|(_, field)| {
            let FieldKind::Calculated { formula, .. } = &field.kind else {
                return None;
            };
            uses(formula).then_some(&field.label)
        })
        .or_else(|| {
            meters
                .iter()
                .find(|(_, meter)| meter.formula.as_ref().is_some_and(uses))
                .map(|(_, meter)| &meter.name)
        })
}

#[cfg(test)]
mod tests;
