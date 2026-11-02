//! What the log form typed, as the event the server takes (step 6g). Kept apart from the form's
//! components so it can be tested without a browser.
//!
//! Each problem names the part of the form it belongs to ([`Target`]), so its message shows
//! there: under the meter, the field, the fuel, the date, or the place.

use domain::api::Located;
use domain::asset::AssetId;
use domain::custom_field::{CustomField, CustomFieldId};
use domain::dates::{EventDates, local_to_utc};
use domain::event::{
    EventInput, EventLocation, EventProblem, EventSource, FieldValue, FuelDetails, Performer,
    Reading, Reason, Requirement, Skipped,
};
use domain::event_form::parse_field;
use domain::i18n::{MessageId, messages};
use domain::location::{Coordinates, Fix, LocationSource, Place, TimeZone};
use domain::meter::{EventKind, Meter, MeterId};
use domain::notes::Notes;
use domain::preferences::Preferences;
use domain::quantity::{QuantityError, parse_economy, parse_money, parse_reading, parse_volume};
use domain::time::UnixMillis;

/// Where on the form a problem shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Target {
    /// Under a meter's reading.
    Meter(MeterId),
    /// Under a custom field.
    Field(CustomFieldId),
    /// Under the fuel.
    Fuel,
    /// Under the date and time.
    When,
    /// Under the place.
    Where,
    /// Above Save.
    General,
}

impl Target {
    /// Where a skipped item's problem shows.
    const fn of(item: Requirement) -> Self {
        match item {
            Requirement::Meter(id) => Self::Meter(id),
            Requirement::Field(id) => Self::Field(id),
        }
    }

    /// Where the server's refusal shows.
    pub(super) const fn of_problem(problem: EventProblem) -> Self {
        match problem {
            EventProblem::Required(item) => Self::of(item),
            EventProblem::ReadingOutOfOrder(meter) | EventProblem::UnknownMeter(meter) => {
                Self::Meter(meter)
            }
            EventProblem::UnknownField(field) | EventProblem::WrongValue(field) => {
                Self::Field(field)
            }
            EventProblem::LocationRequired => Self::Where,
            EventProblem::FuelDetails
            | EventProblem::NoVolume
            | EventProblem::BadAmount
            | EventProblem::BadPercent
            | EventProblem::TowsItself => Self::Fuel,
            EventProblem::ReasonEmpty
            | EventProblem::ReasonTooLong
            | EventProblem::BadPerformer
            | EventProblem::WrongKind
            | EventProblem::Finalized
            | EventProblem::NotFinalized
            | EventProblem::AlreadyCorrected => Self::General,
        }
    }
}

/// A problem and where it shows.
pub(super) type Problem = (Target, MessageId);

/// Everything the form holds, as plain values.
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct Typed {
    /// Each meter's reading as typed.
    pub readings: Vec<(MeterId, String)>,
    /// Each custom field's value as typed.
    pub fields: Vec<(CustomFieldId, String)>,
    /// The required items being skipped, with their reasons as typed.
    pub skipped: Vec<(Requirement, String)>,
    /// The fuel's volume and total price, as typed.
    pub volume: String,
    /// See `volume`.
    pub price: String,
    /// Filled to full.
    pub filled_full: bool,
    /// An earlier fill-up wasn't logged.
    pub missed_previous: bool,
    /// The driving percentages.
    pub highway: u8,
    /// See `highway`.
    pub towing: u8,
    /// See `highway`.
    pub hauling: u8,
    /// The vehicles towed.
    pub towed: Vec<AssetId>,
    /// The vehicle's own fuel economy, as typed.
    pub economy: String,
    /// A date and time typed (`None`: now).
    pub when: Option<(String, String)>,
    /// The city, state or province, and country, as typed.
    pub city: String,
    /// See `city`.
    pub region: String,
    /// See `city`.
    pub country: String,
    /// The picker's point, and the device's fix if it came from it.
    pub coordinates: Option<Coordinates>,
    /// See `coordinates`.
    pub fix: Option<Fix>,
    /// What the server knows about the point.
    pub located: Option<Located>,
    /// Why there's no place, if the person says there can't be one.
    pub no_place_reason: Option<String>,
    /// Someone else who did it: name and contact (`None`: the person logging it).
    pub someone: Option<(String, String)>,
    /// Notes.
    pub notes: String,
    /// The Admin's reason for keeping a reading that doesn't fit.
    pub override_reason: Option<String>,
}

/// What building the event needs besides what was typed.
#[derive(Debug, Clone, Copy)]
pub(super) struct Context<'a> {
    /// The vehicle.
    pub asset: AssetId,
    /// Fuel-up or reading.
    pub kind: EventKind,
    /// Its meters and fields.
    pub meters: &'a [(MeterId, Meter)],
    /// See `meters`.
    pub fields: &'a [(CustomFieldId, CustomField)],
    /// The person's units and currency.
    pub units: &'a Preferences,
    /// The zone when the place data doesn't give one (the device's).
    pub fallback_zone: &'a TimeZone,
    /// Now, for an event logged as "now".
    pub now: UnixMillis,
}

fn quantity(target: Target) -> impl Fn(QuantityError) -> Problem {
    move |error| (target, error.message())
}

fn reason(text: &str, target: Target) -> Result<Reason, Problem> {
    Reason::parse(text).map_err(|problem| (target, problem.message()))
}

/// The event's time zone: the place data's for the point, otherwise the device's.
pub(super) fn zone(typed: &Typed, fallback: &TimeZone) -> TimeZone {
    typed
        .located
        .as_ref()
        .and_then(|located| located.suggestion.as_ref())
        .and_then(|suggestion| suggestion.time_zone.clone())
        .unwrap_or_else(|| fallback.clone())
}

/// The place, if all three parts were given.
pub(super) fn place(typed: &Typed) -> Result<Option<Place>, Problem> {
    let parts = [&typed.city, &typed.region, &typed.country];
    if parts.iter().all(|part| part.trim().is_empty()) {
        return Ok(None);
    }
    Place::parse(&typed.city, &typed.region, &typed.country)
        .map(Some)
        .map_err(|problem| (Target::Where, problem.message()))
}

fn location(typed: &Typed) -> Result<EventLocation, Problem> {
    let place = place(typed)?;
    let missing_reason = match (&place, &typed.no_place_reason) {
        (Some(_), _) => None,
        (None, Some(text)) => Some(reason(text, Target::Where)?),
        (None, None) => return Err((Target::Where, messages::EVENT_LOCATION_REQUIRED)),
    };
    let saved = typed
        .located
        .as_ref()
        .and_then(|located| located.saved.clone());
    let suggestion = typed
        .located
        .as_ref()
        .and_then(|located| located.suggestion.as_ref());
    // The data version only when the place is the data's own suggestion.
    let geocoder_version = suggestion
        .filter(|suggestion| place.as_ref() == Some(&suggestion.place))
        .map(|suggestion| suggestion.data_version.clone());
    let source = match (&saved, typed.fix) {
        (Some(_), _) => LocationSource::SavedPlace,
        (None, Some(_)) => LocationSource::Gps,
        (None, None) => LocationSource::Manual,
    };
    Ok(EventLocation {
        coordinates: typed.coordinates,
        accuracy_m: typed.fix.and_then(|fix| fix.accuracy_m),
        fix_time: typed.fix.map(|fix| fix.taken_at),
        source,
        place_name: saved.as_ref().map(|(_, saved)| saved.name.clone()),
        saved_place: saved.map(|(id, _)| id),
        geocoder_version,
        missing_reason,
        place,
    })
}

fn readings(typed: &Typed, context: &Context<'_>) -> Result<Vec<Reading>, Problem> {
    let mut readings = Vec::new();
    for (id, text) in &typed.readings {
        let skipping = typed
            .skipped
            .iter()
            .any(|(item, _)| *item == Requirement::Meter(*id));
        let Some((_, meter)) = context.meters.iter().find(|(have, _)| have == id) else {
            continue;
        };
        if skipping || text.trim().is_empty() {
            continue;
        }
        let value = parse_reading(text, meter.unit, meter.precision)
            .map_err(quantity(Target::Meter(*id)))?;
        readings.push(Reading { meter: *id, value });
    }
    Ok(readings)
}

fn fields(typed: &Typed, context: &Context<'_>) -> Result<Vec<FieldValue>, Problem> {
    let mut values = Vec::new();
    for (id, text) in &typed.fields {
        let Some((_, field)) = context.fields.iter().find(|(have, _)| have == id) else {
            continue;
        };
        let skipping = typed
            .skipped
            .iter()
            .any(|(item, _)| *item == Requirement::Field(*id));
        if skipping || !field.enabled || field.applies_to != context.kind {
            continue;
        }
        if let Some(data) =
            parse_field(&field.kind, text, context.units).map_err(|m| (Target::Field(*id), m))?
        {
            values.push(FieldValue { field: *id, data });
        }
    }
    Ok(values)
}

fn fuel(typed: &Typed, context: &Context<'_>) -> Result<Option<FuelDetails>, Problem> {
    if context.kind != EventKind::Fuel {
        return Ok(None);
    }
    if typed.volume.trim().is_empty() {
        return Err((Target::Fuel, messages::EVENT_NO_VOLUME));
    }
    let units = context.units;
    let volume_ml = parse_volume(&typed.volume, units.volume).map_err(quantity(Target::Fuel))?;
    let optional = |text: &str, parse: &dyn Fn(&str) -> Result<i64, QuantityError>| {
        let text = text.trim();
        (!text.is_empty())
            .then(|| parse(text).map_err(quantity(Target::Fuel)))
            .transpose()
    };
    Ok(Some(FuelDetails {
        volume_ml,
        cost_minor: optional(&typed.price, &|text| parse_money(text, units.currency))?,
        currency: units.currency,
        filled_full: typed.filled_full,
        missed_previous: typed.missed_previous,
        highway_pct: typed.highway,
        towing_pct: typed.towing,
        hauling_pct: typed.hauling,
        towed: if typed.towing > 0 {
            typed.towed.clone()
        } else {
            Vec::new()
        },
        reported_economy: optional(&typed.economy, &|text| parse_economy(text, units.economy))?,
    }))
}

/// The event to send.
///
/// # Errors
///
/// The first [`Problem`] found, with where it shows.
pub(super) fn build(typed: &Typed, context: &Context<'_>) -> Result<EventInput, Problem> {
    let zone = zone(typed, context.fallback_zone);
    let start = match &typed.when {
        None => context.now,
        Some((date, time)) => {
            local_to_utc(date, time, &zone).map_err(|error| (Target::When, error.message()))?
        }
    };
    let dates =
        EventDates::new(start, None, zone).map_err(|error| (Target::When, error.message()))?;
    let skipped = typed
        .skipped
        .iter()
        .map(|(item, text)| {
            Ok(Skipped {
                item: *item,
                reason: reason(text, Target::of(*item))?,
            })
        })
        .collect::<Result<Vec<_>, Problem>>()?;
    let performed_by = typed
        .someone
        .as_ref()
        .map(|(name, contact)| Performer::Named {
            name: name.trim().to_owned(),
            contact: Some(contact.trim().to_owned()).filter(|contact| !contact.is_empty()),
        });
    Ok(EventInput {
        asset: context.asset,
        kind: context.kind,
        dates,
        location: location(typed)?,
        readings: readings(typed, context)?,
        skipped,
        fields: fields(typed, context)?,
        notes: Notes::parse(&typed.notes).map_err(|error| (Target::General, error.message()))?,
        source: EventSource::Typed,
        performed_by,
        fuel: fuel(typed, context)?,
        reading_override: typed
            .override_reason
            .as_deref()
            .map(|text| reason(text, Target::General))
            .transpose()?,
    })
}

/// The meters and fields that are required on `kind`, in the form's order.
pub(super) fn required_items(
    kind: EventKind,
    meters: &[(MeterId, Meter)],
    fields: &[(CustomFieldId, CustomField)],
) -> Vec<Requirement> {
    meters
        .iter()
        .filter(|(_, meter)| meter.required_for.contains(&kind))
        .map(|(id, _)| Requirement::Meter(*id))
        .chain(
            fields
                .iter()
                .filter(|(_, field)| field.required_on(kind))
                .map(|(id, _)| Requirement::Field(*id)),
        )
        .collect()
}

#[cfg(test)]
mod tests {
    use domain::custom_field::{FieldKey, FieldKind};
    use domain::label::Label;
    use domain::meter::{MeterDimension, MeterUnit};
    use domain::preferences::{DistanceUnit, VolumeUnit};
    use domain::test_support::test_id;

    use super::*;

    const ODOMETER: u32 = 20;
    const TRIP: u32 = 30;

    fn meters() -> Vec<(MeterId, Meter)> {
        vec![(
            test_id(ODOMETER),
            Meter {
                asset: test_id(1),
                name: Label::parse("Odometer").unwrap(),
                dimension: MeterDimension::Distance,
                unit: MeterUnit::Miles,
                precision: 0,
                description: None,
                required_for: vec![EventKind::Fuel],
            },
        )]
    }

    fn fields() -> Vec<(CustomFieldId, CustomField)> {
        vec![(
            test_id(TRIP),
            CustomField {
                asset: test_id(1),
                applies_to: EventKind::Fuel,
                key: FieldKey::parse("trip-time").unwrap(),
                label: Label::parse("Trip time").unwrap(),
                kind: FieldKind::Duration,
                enabled: true,
                required: false,
                range: None,
            },
        )]
    }

    fn typed() -> Typed {
        Typed {
            readings: vec![(test_id(ODOMETER), "52,644".into())],
            fields: vec![(test_id(TRIP), "3:40".into())],
            volume: "18.004".into(),
            price: "62.27".into(),
            filled_full: true,
            highway: 75,
            towing: 25,
            towed: vec![test_id(9)],
            city: "Asheville".into(),
            region: "North Carolina".into(),
            country: "US".into(),
            ..Typed::default()
        }
    }

    fn built(typed: &Typed) -> Result<EventInput, Problem> {
        let (meters, fields) = (meters(), fields());
        let units = Preferences {
            distance: DistanceUnit::Miles,
            volume: VolumeUnit::UsGallons,
            ..Preferences::default()
        };
        let zone = TimeZone::parse("America/New_York").unwrap();
        build(
            typed,
            &Context {
                asset: test_id(1),
                kind: EventKind::Fuel,
                meters: &meters,
                fields: &fields,
                units: &units,
                fallback_zone: &zone,
                now: UnixMillis::from_millis(1_700_000_000_000),
            },
        )
    }

    #[test]
    fn a_typical_fuel_up_becomes_the_event() {
        let input = built(&typed()).unwrap();
        assert_eq!(input.readings[0].value, 84_722_306, "52,644 mi in meters");
        assert_eq!(
            input.fields[0].data,
            domain::event::FieldData::Duration(13_200)
        );
        let fuel = input.fuel.unwrap();
        assert_eq!((fuel.volume_ml, fuel.cost_minor), (68_153, Some(6_227)));
        assert_eq!(fuel.towed, vec![test_id(9)]);
        assert_eq!(input.location.source, LocationSource::Manual);
        assert_eq!(input.dates.zone.as_str(), "America/New_York");
        assert_eq!(input.dates.start.as_millis(), 1_700_000_000_000, "now");
        assert_eq!(input.performed_by, None, "done by the person logging it");
    }

    #[test]
    fn each_problem_says_where_it_belongs() {
        let mut no_fuel = typed();
        no_fuel.volume.clear();
        assert_eq!(
            built(&no_fuel).err(),
            Some((Target::Fuel, messages::EVENT_NO_VOLUME))
        );
        let mut bad_reading = typed();
        bad_reading.readings[0].1 = "52,644.5".into();
        assert_eq!(
            built(&bad_reading).err(),
            Some((
                Target::Meter(test_id(ODOMETER)),
                messages::QUANTITY_TOO_PRECISE
            ))
        );
        let mut nowhere = typed();
        nowhere.city.clear();
        nowhere.region.clear();
        nowhere.country.clear();
        assert_eq!(
            built(&nowhere).err(),
            Some((Target::Where, messages::EVENT_LOCATION_REQUIRED))
        );
        nowhere.no_place_reason = Some("Out of signal".into());
        assert!(built(&nowhere).unwrap().location.place.is_none());
        let mut bad_date = typed();
        bad_date.when = Some(("2026-02-30".into(), "15:30".into()));
        assert_eq!(
            built(&bad_date).err().map(|(target, _)| target),
            Some(Target::When)
        );
    }

    #[test]
    fn a_skipped_reading_is_left_out_and_towing_at_zero_tows_nothing() {
        let mut skipping = typed();
        skipping.skipped = vec![(Requirement::Meter(test_id(ODOMETER)), "Dark".into())];
        skipping.towing = 0;
        let input = built(&skipping).unwrap();
        assert!(input.readings.is_empty());
        assert_eq!(input.skipped[0].reason.as_str(), "Dark");
        assert!(input.fuel.unwrap().towed.is_empty());
        skipping.skipped[0].1 = " ".into();
        assert_eq!(
            built(&skipping).err(),
            Some((
                Target::Meter(test_id(ODOMETER)),
                messages::EVENT_REASON_EMPTY
            ))
        );
    }

    #[test]
    fn required_items_come_from_the_meters_and_fields() {
        let mut fields = fields();
        assert_eq!(
            required_items(EventKind::Fuel, &meters(), &fields),
            vec![Requirement::Meter(test_id(ODOMETER))]
        );
        fields[0].1.required = true;
        assert_eq!(required_items(EventKind::Fuel, &meters(), &fields).len(), 2);
        assert!(required_items(EventKind::Reading, &meters(), &fields).is_empty());
    }
}
