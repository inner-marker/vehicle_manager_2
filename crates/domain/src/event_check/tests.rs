//! The event checks, test-first: a truck whose odometer is required on fuel-ups, with a
//! required "Trip time" field and an optional "Fuel brand" choice.

use proptest::prelude::*;

use super::*;
use crate::custom_field::{FieldKey, FieldRange};
use crate::dates::{EventDates, local_to_utc};
use crate::event::{
    ClashSide, Event, EventLocation, EventSource, FieldData, FieldValue, Flag, FuelDetails,
    Reading, ReadingClash, Reason, Skipped,
};
use crate::label::Label;
use crate::location::{LocationSource, Place, TimeZone};
use crate::meter::{MeterDimension, MeterUnit};
use crate::notes::Notes;
use crate::preferences::CurrencyCode;
use crate::test_support::test_id;

const TRUCK: u32 = 1;
const TRAILER: u32 = 2;
const ODOMETER: u32 = 10;
const HOURS: u32 = 11;
const TRIP_TIME: u32 = 20;
const BRAND: u32 = 21;
const OFF_FIELD: u32 = 22;

fn utc() -> TimeZone {
    TimeZone::parse("UTC").unwrap()
}

fn on(date: &str) -> UnixMillis {
    local_to_utc(date, "12:00", &utc()).unwrap()
}

fn meters() -> Vec<(MeterId, Meter)> {
    let meter = |name: &str, dimension, unit, required_for| Meter {
        asset: test_id(TRUCK),
        name: Label::parse(name).unwrap(),
        dimension,
        unit,
        precision: 0,
        description: None,
        required_for,
    };
    vec![
        (
            test_id(ODOMETER),
            meter(
                "Odometer",
                MeterDimension::Distance,
                MeterUnit::Miles,
                vec![EventKind::Fuel],
            ),
        ),
        (
            test_id(HOURS),
            meter(
                "Engine hours",
                MeterDimension::Duration,
                MeterUnit::Hours,
                vec![],
            ),
        ),
    ]
}

fn fields() -> Vec<(CustomFieldId, CustomField)> {
    let field = |key: &str, label: &str, kind, enabled, range| CustomField {
        asset: test_id(TRUCK),
        shown_on: vec![EventKind::Fuel],
        key: FieldKey::parse(key).unwrap(),
        label: Label::parse(label).unwrap(),
        kind,
        enabled,
        // Trip time and the old field are required; the old one is off, so it never is.
        required_for: if key == "brand" {
            vec![]
        } else {
            vec![EventKind::Fuel]
        },
        range,
    };
    let brands = vec![
        Label::parse("Shell").unwrap(),
        Label::parse("Costco").unwrap(),
    ];
    vec![
        (
            test_id(TRIP_TIME),
            field(
                "trip-time",
                "Trip time",
                FieldKind::Duration,
                true,
                FieldRange::new(Some(60), Some(36_000)).ok(),
            ),
        ),
        (
            test_id(BRAND),
            field(
                "brand",
                "Fuel brand",
                FieldKind::Choice { options: brands },
                true,
                None,
            ),
        ),
        (
            test_id(OFF_FIELD),
            field("old", "Old field", FieldKind::Text, false, None),
        ),
    ]
}

/// Odometer readings already logged: 50,000 m on 1 January, 52,000 m on 1 March.
fn history() -> Vec<(MeterId, UnixMillis, i64)> {
    vec![
        (test_id(ODOMETER), on("2026-01-01"), 50_000),
        (test_id(ODOMETER), on("2026-03-01"), 52_000),
    ]
}

/// A good fuel-up on 1 February: the odometer between its neighbours, trip time given.
fn fuel_up() -> Event {
    Event {
        asset: test_id(TRUCK),
        kind: EventKind::Fuel,
        dates: EventDates::new(on("2026-02-01"), None, utc()).unwrap(),
        location: EventLocation {
            place: Place::parse("Asheville", "North Carolina", "US").ok(),
            coordinates: None,
            accuracy_m: None,
            fix_time: None,
            source: LocationSource::Manual,
            place_name: None,
            saved_place: None,
            geocoder_version: None,
            missing_reason: None,
        },
        readings: vec![Reading {
            meter: test_id(ODOMETER),
            value: 51_000,
        }],
        skipped: vec![],
        fields: vec![FieldValue {
            field: test_id(TRIP_TIME),
            data: FieldData::Duration(5_400),
        }],
        notes: Notes::default(),
        source: EventSource::Typed,
        logged_by: test_id(100),
        performed_by: None,
        fuel: Some(FuelDetails {
            volume_ml: 75_000,
            cost_minor: Some(25_000),
            currency: CurrencyCode::USD,
            filled_full: true,
            missed_previous: false,
            highway_pct: 75,
            towing_pct: 0,
            hauling_pct: 0,
            towed: vec![test_id(TRAILER)],
            reported_economy: Some(8_500),
        }),
        work: None,
        reading_override: None,
        status: EventStatus::Draft,
        last_edited_at: on("2026-02-01"),
        received_at: on("2026-02-01"),
        finalized: None,
        corrects: None,
        correction_reason: None,
    }
}

/// Checks `event` as a Driver (`admin` false) or the Admin in Admin mode.
fn checked(event: &Event, admin: bool) -> Result<Vec<Warning>, EventProblem> {
    let (meters, fields, history) = (meters(), fields(), history());
    check(
        event,
        &CheckContext {
            kinds: &EventKind::ALL,
            requirements: &[],
            outstanding_squawks: &[],
            meters: &meters,
            fields: &fields,
            history: &history,
            location_rule: LocationRule::Required,
            may_override_readings: admin,
        },
    )
}

fn reason(text: &str) -> Reason {
    Reason::parse(text).unwrap()
}

#[test]
fn a_good_fuel_up_passes_with_nothing_flagged() {
    assert_eq!(checked(&fuel_up(), false), Ok(vec![]));
    assert!(fuel_up().flags().is_empty());
}

#[test]
fn a_place_is_required_unless_theres_a_reason() {
    let mut event = fuel_up();
    event.location.place = None;
    assert_eq!(checked(&event, false), Err(EventProblem::LocationRequired));
    event.location.missing_reason = Some(reason("No signal, and the gauge was dark"));
    assert_eq!(checked(&event, false), Ok(vec![]));
    assert_eq!(event.flags(), vec![Flag::MissingLocation]);
}

#[test]
fn required_readings_and_fields_are_given_or_skipped_with_a_reason() {
    let mut event = fuel_up();
    event.readings.clear();
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::Required(Requirement::Meter(test_id(
            ODOMETER
        ))))
    );
    event.skipped.push(Skipped {
        item: Requirement::Meter(test_id(ODOMETER)),
        reason: reason("Dashboard won't light"),
    });
    assert_eq!(checked(&event, false), Ok(vec![]));
    assert_eq!(event.flags(), vec![Flag::Skipped]);

    let mut event = fuel_up();
    event.fields.clear();
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::Required(Requirement::Field(test_id(
            TRIP_TIME
        ))))
    );
    event.skipped.push(Skipped {
        item: Requirement::Field(test_id(TRIP_TIME)),
        reason: reason("Forgot to reset the trip"),
    });
    assert_eq!(
        checked(&event, false),
        Ok(vec![]),
        "the old field is required too, but it's off, so it isn't asked for"
    );

    // A required field on another kind of event isn't asked for on a fuel-up.
    let (meters, mut fields, history) = (meters(), fields(), history());
    fields[1].1.shown_on = vec![EventKind::Fuel, EventKind::Reading];
    fields[1].1.required_for = vec![EventKind::Reading];
    let mut event = fuel_up();
    event.fields.retain(|value| value.field != test_id(BRAND));
    let context = CheckContext {
        kinds: &EventKind::ALL,
        requirements: &[],
        outstanding_squawks: &[],
        meters: &meters,
        fields: &fields,
        history: &history,
        location_rule: LocationRule::Required,
        may_override_readings: false,
    };
    assert_eq!(check(&event, &context), Ok(vec![]));
}

#[test]
fn a_back_dated_reading_that_fits_is_fine_for_anyone() {
    // 1 February, between 50,000 (January) and 52,000 (March).
    assert_eq!(checked(&fuel_up(), false), Ok(vec![]));
    // The same value as a neighbour fits too (the meter didn't move).
    let mut event = fuel_up();
    event.readings[0].value = 52_000;
    assert_eq!(checked(&event, false), Ok(vec![]));
}

#[test]
fn a_reading_that_doesnt_fit_is_refused_except_the_admin_may_keep_it() {
    for (value, side, at, other) in [
        (49_000, ClashSide::BelowEarlier, on("2026-01-01"), 50_000),
        (53_000, ClashSide::AboveLater, on("2026-03-01"), 52_000),
    ] {
        let mut event = fuel_up();
        event.readings[0].value = value;
        let refused = Err(EventProblem::ReadingOutOfOrder(ReadingClash {
            meter: test_id(ODOMETER),
            side,
            at,
            value: other,
        }));
        assert_eq!(checked(&event, false), refused, "a Driver, {value}");
        assert_eq!(
            checked(&event, true),
            refused,
            "the Admin, without a reason"
        );
        event.reading_override = Some(reason("Odometer replaced in February"));
        assert_eq!(checked(&event, false), refused, "a Driver can't override");
        assert_eq!(
            checked(&event, true),
            Ok(vec![Warning::ReadingKept {
                meter: test_id(ODOMETER)
            }])
        );
        assert!(event.flags().contains(&Flag::ReadingOverride));
    }
}

#[test]
fn readings_need_a_known_meter_once() {
    let mut event = fuel_up();
    event.readings.push(Reading {
        meter: test_id(99),
        value: 1,
    });
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::UnknownMeter(test_id(99)))
    );
    let mut event = fuel_up();
    event.readings.push(event.readings[0]);
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::UnknownMeter(test_id(ODOMETER)))
    );
}

#[test]
fn field_values_match_their_field_and_ranges_only_warn() {
    let mut event = fuel_up();
    event.fields[0].data = FieldData::Number(5);
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::WrongValue(test_id(TRIP_TIME)))
    );

    let mut event = fuel_up();
    event.fields.push(FieldValue {
        field: test_id(BRAND),
        data: FieldData::Choice(Label::parse("Exxon").unwrap()),
    });
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::WrongValue(test_id(BRAND)))
    );

    let mut event = fuel_up();
    event.fields.push(FieldValue {
        field: test_id(OFF_FIELD),
        data: FieldData::Text("x".into()),
    });
    assert_eq!(
        checked(&event, false),
        Err(EventProblem::UnknownField(test_id(OFF_FIELD)))
    );

    // Trip time of 12 hours is above its range: a warning, not a refusal.
    let mut event = fuel_up();
    event.fields[0].data = FieldData::Duration(43_200);
    assert_eq!(
        checked(&event, false),
        Ok(vec![Warning::OutOfRange {
            field: test_id(TRIP_TIME),
            check: RangeCheck::Above
        }])
    );
}

#[test]
fn fuel_ups_need_sensible_details() {
    let with = |change: fn(&mut FuelDetails)| {
        let mut event = fuel_up();
        change(event.fuel.as_mut().unwrap());
        checked(&event, false)
    };
    assert_eq!(with(|fuel| fuel.volume_ml = 0), Err(EventProblem::NoVolume));
    assert_eq!(
        with(|fuel| fuel.cost_minor = Some(-1)),
        Err(EventProblem::BadAmount)
    );
    assert_eq!(
        with(|fuel| fuel.reported_economy = Some(0)),
        Err(EventProblem::BadAmount)
    );
    assert_eq!(
        with(|fuel| fuel.highway_pct = 101),
        Err(EventProblem::BadPercent)
    );
    assert_eq!(
        with(|fuel| fuel.towed = vec![test_id(TRUCK)]),
        Err(EventProblem::TowsItself)
    );

    let mut no_details = fuel_up();
    no_details.fuel = None;
    assert_eq!(checked(&no_details, false), Err(EventProblem::FuelDetails));
}

#[test]
fn kinds_follow_the_vehicle_and_the_milestone() {
    let (meters, fields, history) = (meters(), fields(), history());
    let trailer = CheckContext {
        kinds: &[
            EventKind::Reading,
            EventKind::Maintenance,
            EventKind::Repair,
        ],
        requirements: &[],
        outstanding_squawks: &[],
        meters: &meters,
        fields: &fields,
        history: &history,
        location_rule: LocationRule::Required,
        may_override_readings: false,
    };
    assert_eq!(
        check(&fuel_up(), &trailer),
        Err(EventProblem::WrongKind),
        "a trailer has no fuel-ups"
    );

    let mut maintenance = fuel_up();
    maintenance.kind = EventKind::Maintenance;
    maintenance.fuel = None;
    assert_eq!(
        checked(&maintenance, false),
        Err(EventProblem::WorkDetails),
        "maintenance needs its work details"
    );

    // A reading: the odometer isn't required, and fuel details aren't allowed.
    let mut reading = fuel_up();
    reading.kind = EventKind::Reading;
    reading.fuel = None;
    reading.fields.clear();
    let context = CheckContext {
        kinds: &EventKind::ALL,
        requirements: &[],
        outstanding_squawks: &[],
        meters: &meters,
        fields: &fields,
        history: &history,
        location_rule: LocationRule::Required,
        may_override_readings: false,
    };
    assert_eq!(check(&reading, &context), Ok(vec![]));
    reading.fuel = fuel_up().fuel;
    assert_eq!(check(&reading, &context), Err(EventProblem::FuelDetails));
}

#[test]
fn a_performer_needs_a_name() {
    let named = |name: &str| Performer::Named {
        name: name.into(),
        contact: None,
    };
    assert_eq!(check_performer(&named("Joe's Auto")), Ok(()));
    assert_eq!(
        check_performer(&named("  ")),
        Err(EventProblem::BadPerformer)
    );
    assert_eq!(
        check_performer(&named(&"x".repeat(101))),
        Err(EventProblem::BadPerformer)
    );
    let mut event = fuel_up();
    event.performed_by = Some(named(""));
    assert_eq!(checked(&event, false), Err(EventProblem::BadPerformer));
}

#[test]
fn finalized_events_never_change_and_finalize_after_24_hours() {
    let mut event = fuel_up();
    assert_eq!(may_change(&event), Ok(()));
    let last = event.last_edited_at.as_millis();
    assert!(!event.finalize_due(UnixMillis::from_millis(last + 86_399_999), 86_400_000));
    assert!(event.finalize_due(UnixMillis::from_millis(last + 86_400_000), 86_400_000));
    // Test timers (#054): 2 minutes.
    assert!(event.finalize_due(UnixMillis::from_millis(last + 120_000), 120_000));

    let patch = finalize(
        Finalizer::Server,
        UnixMillis::from_millis(last + 86_400_000),
    );
    assert_eq!(patch.status, Some(EventStatus::Finalized));
    assert_eq!(
        patch.finalized,
        Some(Clearable::Set(Finalized {
            at: UnixMillis::from_millis(last + 86_400_000),
            by: Finalizer::Server
        }))
    );
    event.status = EventStatus::Finalized;
    assert_eq!(may_change(&event), Err(EventProblem::Finalized));
    assert!(
        !event.finalize_due(UnixMillis::from_millis(last + 999_999_999), 1),
        "already final"
    );
}

#[test]
fn a_correction_needs_a_finalized_original_and_a_reason() {
    let mut original = fuel_up();
    let original_id = test_id(500);
    let mut correction = fuel_up();
    correction.corrects = Some(original_id);
    correction.correction_reason = Some(reason("Typed 15 gallons, was 13"));
    assert_eq!(
        check_correction(&original, original_id, &correction),
        Err(EventProblem::NotFinalized),
        "edit a draft instead"
    );
    original.status = EventStatus::Finalized;
    assert_eq!(
        check_correction(&original, original_id, &correction),
        Ok(())
    );
    assert!(correction.flags().contains(&Flag::Correction));

    let mut unexplained = correction.clone();
    unexplained.correction_reason = None;
    assert_eq!(
        check_correction(&original, original_id, &unexplained),
        Err(EventProblem::ReasonEmpty)
    );
    let mut elsewhere = correction;
    elsewhere.asset = test_id(TRAILER);
    assert_eq!(
        check_correction(&original, original_id, &elsewhere),
        Err(EventProblem::WrongKind)
    );
}

#[test]
fn reasons_and_the_patch() {
    use crate::command::Patch;
    assert_eq!(Reason::parse("  "), Err(EventProblem::ReasonEmpty));
    assert_eq!(
        Reason::parse(&"x".repeat(501)),
        Err(EventProblem::ReasonTooLong)
    );
    assert_eq!(reason(" Typo ").as_str(), "Typo");
    let mut patch = EventPatch::all(fuel_up());
    assert_eq!(patch.changed_fields().len(), 20, "every field");
    patch.keep_only(&["notes", "readings"]);
    assert_eq!(patch.changed_fields(), vec!["readings", "notes"]);
}

#[test]
fn neighbours_are_the_nearest_readings_either_side() {
    let history = history();
    let odometer = test_id(ODOMETER);
    assert_eq!(
        neighbours(&history, odometer, on("2026-02-01")),
        (
            Some((on("2026-01-01"), 50_000)),
            Some((on("2026-03-01"), 52_000))
        )
    );
    assert_eq!(
        neighbours(&history, odometer, on("2025-12-01")),
        (None, Some((on("2026-01-01"), 50_000)))
    );
    assert_eq!(
        neighbours(&history, odometer, on("2026-04-01")),
        (Some((on("2026-03-01"), 52_000)), None)
    );
    // A reading at the same moment counts as before.
    assert_eq!(
        neighbours(&history, odometer, on("2026-03-01")),
        (Some((on("2026-03-01"), 52_000)), None)
    );
    assert_eq!(
        neighbours(&history, test_id(HOURS), on("2026-02-01")),
        (None, None)
    );
}

proptest! {
    #[test]
    fn a_reading_is_accepted_exactly_when_it_fits_its_neighbours(
        mut values in proptest::collection::vec(0i64..1_000_000, 0..6),
        day in 0i64..400,
        value in 0i64..1_000_000,
    ) {
        // A meter's history, one reading a month and rising, and a new reading on some day.
        values.sort_unstable();
        let start = on("2026-01-01").as_millis();
        let day_ms = 86_400_000;
        let history: Vec<_> = values
            .iter()
            .zip(0..)
            .map(|(v, month)| (test_id::<Meter>(ODOMETER), UnixMillis::from_millis(start + month * 30 * day_ms), *v))
            .collect();
        let at = UnixMillis::from_millis(start + day * day_ms + 1);
        let (before, after) = neighbours(&history, test_id(ODOMETER), at);
        let (before, after) = (before.map(|(_, v)| v), after.map(|(_, v)| v));
        let mut event = fuel_up();
        event.dates = EventDates::new(at, None, utc()).unwrap();
        event.readings[0].value = value;
        let (meters, fields) = (meters(), fields());
        let result = check(&event, &CheckContext {
            kinds: &EventKind::ALL,
            requirements: &[],
            outstanding_squawks: &[],
            meters: &meters,
            fields: &fields,
                history: &history,
            location_rule: LocationRule::Required,
            may_override_readings: false,
        });
        prop_assert_eq!(result.is_ok(), reading_fits(value, before, after));
        // And "fits" means between the neighbours, inclusive.
        prop_assert_eq!(
            reading_fits(value, before, after),
            before.is_none_or(|b| value >= b) && after.is_none_or(|a| value <= a)
        );
    }
}
