//! Calculated fields and derived meters, test-first: a truck with an odometer, engine hours,
//! a resettable Oil life, a trip-time field, and fuel-ups.

use super::*;
use crate::dates::EventDates;
use crate::event::{EventLocation, EventSource, EventStatus, FieldValue, FuelDetails};
use crate::formula::{FormulaError, ShowsAs, in_units};
use crate::location::LocationSource;
use crate::meter::{MeterDirection, MeterError, MeterUnit};
use crate::notes::Notes;
use crate::preferences::CurrencyCode;
use crate::test_support::test_id;
use crate::time::UnixMillis;

const TRUCK: u32 = 1;
const ODOMETER: u32 = 10;
const HOURS: u32 = 11;
const OIL_LIFE: u32 = 12;
const SERVICE_MILES: u32 = 13;
const TRIP_TIME: u32 = 20;
const PER_MILE: u32 = 21;
const MILE: f64 = 1_609.344;
const HOUR: i64 = 3_600_000;

fn key(text: &str) -> FieldKey {
    FieldKey::parse(text).unwrap()
}

fn label(text: &str) -> Label {
    Label::parse(text).unwrap()
}

fn meter(name: &str, key_text: &str, dimension: MeterDimension, unit: MeterUnit) -> Meter {
    Meter {
        asset: test_id(TRUCK),
        name: label(name),
        dimension,
        unit,
        precision: 0,
        description: None,
        required_for: vec![],
        direction: MeterDirection::Up,
        reset_to: None,
        key: key(key_text),
        formula: None,
    }
}

/// Service miles: the odometer plus 33 miles for each engine hour.
fn service_miles() -> Meter {
    Meter {
        formula: Some(Formula::parse("odometer + engine-hours × 33 mi/h").unwrap()),
        ..meter(
            "Service miles",
            "service-miles",
            MeterDimension::Distance,
            MeterUnit::Miles,
        )
    }
}

fn meters() -> Vec<(MeterId, Meter)> {
    vec![
        (
            test_id(ODOMETER),
            Meter {
                required_for: vec![EventKind::Fuel],
                ..meter(
                    "Odometer",
                    "odometer",
                    MeterDimension::Distance,
                    MeterUnit::Miles,
                )
            },
        ),
        (
            test_id(HOURS),
            meter(
                "Engine hours",
                "engine-hours",
                MeterDimension::Duration,
                MeterUnit::Hours,
            ),
        ),
        (
            test_id(OIL_LIFE),
            Meter {
                direction: MeterDirection::Down,
                reset_to: Some(10_000),
                ..meter(
                    "Oil life",
                    "oil-life",
                    MeterDimension::Percentage,
                    MeterUnit::Percent,
                )
            },
        ),
        (test_id(SERVICE_MILES), service_miles()),
    ]
}

fn field(name: &str, key_text: &str, kind: FieldKind) -> CustomField {
    CustomField {
        asset: test_id(TRUCK),
        shown_on: vec![EventKind::Fuel],
        key: key(key_text),
        label: label(name),
        kind,
        enabled: true,
        required_for: vec![],
        range: None,
    }
}

fn calculated(name: &str, key_text: &str, formula: &str, shows: ShowsAs) -> CustomField {
    field(
        name,
        key_text,
        FieldKind::Calculated {
            formula: Formula::parse(formula).unwrap(),
            shows,
        },
    )
}

fn fields() -> Vec<(CustomFieldId, CustomField)> {
    vec![
        (
            test_id(TRIP_TIME),
            field("Trip time", "trip-time", FieldKind::Duration),
        ),
        (
            test_id(PER_MILE),
            calculated(
                "Cost per mile",
                "cost-per-mile",
                "round(cost ÷ distance-since-last(odometer), 2)",
                ShowsAs::MoneyPerDistance,
            ),
        ),
    ]
}

fn hours(n: i64) -> UnixMillis {
    UnixMillis::from_millis(1_800_000_000_000 + n * HOUR)
}

fn past(meter: u32, at: i64, value: i64) -> PastReading {
    PastReading {
        meter: test_id(meter),
        at: hours(at),
        value,
        reset: false,
    }
}

fn miles(n: f64) -> i64 {
    crate::quantity::whole((n * MILE).round()).unwrap()
}

/// A derived reading: the odometer's stored reading (`odometer` miles) plus `extra` miles.
fn plus(odometer: f64, extra: f64) -> i64 {
    let odometer = crate::quantity::float(miles(odometer));
    crate::quantity::whole((odometer + extra * MILE).round()).unwrap()
}

/// A fuel-up at hour `at`: the odometer at `odometer` miles, `cost` dollars, 10 gallons.
fn fuel_up(at: i64, odometer: f64, cost: Option<i64>) -> Event {
    Event {
        asset: test_id(TRUCK),
        kind: EventKind::Fuel,
        dates: EventDates::new(hours(at), None, TimeZone::parse("UTC").unwrap()).unwrap(),
        location: EventLocation {
            place: None,
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
            value: miles(odometer),
            reset: false,
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
            volume_ml: 37_854,
            cost_minor: cost.map(|dollars| dollars * 100),
            currency: CurrencyCode::USD,
            filled_full: true,
            missed_previous: false,
            highway_pct: 75,
            towing_pct: 0,
            hauling_pct: 0,
            towed: vec![],
            reported_economy: None,
        }),
        work: None,
        lifecycle: None,
        reading_override: None,
        status: EventStatus::Draft,
        last_edited_at: hours(at),
        received_at: hours(at),
        finalized: None,
        corrects: None,
        correction_reason: None,
    }
}

// --- Keys --------------------------------------------------------------------------------

#[test]
fn a_new_key_is_its_own_and_never_a_word_formulas_use() {
    let (fields, meters) = (fields(), meters());
    assert_eq!(
        new_key(&label("Tire pressure"), &fields, &meters),
        key("tire-pressure")
    );
    assert_eq!(
        new_key(&label("Odometer"), &fields, &meters),
        key("odometer-2")
    );
    assert_eq!(
        new_key(&label("Trip time"), &fields, &meters),
        key("trip-time-2")
    );
    assert_eq!(new_key(&label("Cost"), &fields, &meters), key("cost-2"));
    assert_eq!(new_key(&label("Mean"), &[], &[]), key("mean-2"));
    assert_eq!(new_key(&label("Last"), &[], &[]), key("last-2"));
    for word in [
        "mean",
        "geometric_mean",
        "round",
        "abs",
        "last",
        "cumulative",
        "on",
        "mi",
        "gal",
        "days",
        "volume",
        "reported-economy",
        "time-since-last",
    ] {
        assert!(is_reserved(word), "{word}");
    }
    assert!(!is_reserved("odometer"));
}

// --- Checks ------------------------------------------------------------------------------

#[test]
fn a_calculated_field_is_checked_on_each_kind_it_shows_on() {
    let (fields, meters) = (fields(), meters());
    let per_mile = &fields[1].1;
    assert_eq!(check_calculated(per_mile, &fields, &meters), Ok(()));
    // The trip-time field and fuel-up values aren't on readings.
    let on_readings = CustomField {
        shown_on: vec![EventKind::Fuel, EventKind::Reading],
        ..per_mile.clone()
    };
    assert_eq!(
        check_calculated(&on_readings, &fields, &meters),
        Err(FormulaError::UnknownKey { key: "cost".into() })
    );
    let wrong = calculated("Spend", "spend", "cost ÷ volume", ShowsAs::Money);
    assert!(matches!(
        check_calculated(&wrong, &fields, &meters),
        Err(FormulaError::WrongResult { .. })
    ));
    let nested = calculated(
        "Double",
        "double",
        "cost-per-mile × 2",
        ShowsAs::MoneyPerDistance,
    );
    assert_eq!(
        check_calculated(&nested, &fields, &meters),
        Err(FormulaError::Calculated {
            key: "cost-per-mile".into()
        })
    );
    let with_meters = calculated(
        "Service miles per hour",
        "service-rate",
        "distance-since-last(service-miles) ÷ time-since-last",
        ShowsAs::Speed,
    );
    assert_eq!(check_calculated(&with_meters, &fields, &meters), Ok(()));
    let oil = calculated("Oil", "oil", "oil-life", ShowsAs::Percent { decimals: 0 });
    assert_eq!(check_calculated(&oil, &fields, &meters), Ok(()));
}

#[test]
fn a_derived_meter_uses_only_typed_meters_that_arent_reset() {
    let meters = meters();
    assert_eq!(
        check_derived(Some(test_id(SERVICE_MILES)), &service_miles(), &meters),
        Ok(())
    );
    let typed = meter("Trip", "trip", MeterDimension::Distance, MeterUnit::Miles);
    assert_eq!(check_derived(None, &typed, &meters), Ok(()));
    let with = |formula: &str| Meter {
        formula: Some(Formula::parse(formula).unwrap()),
        ..service_miles()
    };
    let refused = |formula: &str| check_derived(None, &with(formula), &meters).unwrap_err();
    assert_eq!(
        refused("odometer × oil-life"),
        DerivedError::Formula(FormulaError::NotAnInput {
            key: "oil-life".into()
        })
    );
    assert_eq!(
        refused("service-miles + 1 mi"),
        DerivedError::Formula(FormulaError::NotAnInput {
            key: "service-miles".into()
        })
    );
    assert_eq!(
        refused("round(odometer, 0)"),
        DerivedError::Formula(FormulaError::NotInDerived)
    );
    assert_eq!(
        refused("distance-since-last(odometer)"),
        DerivedError::Formula(FormulaError::NotInDerived)
    );
    assert_eq!(
        refused("engine-hours"),
        DerivedError::Formula(FormulaError::WrongResult {
            found: Dimension::TIME,
            wanted: Dimension::DISTANCE
        })
    );
    assert_eq!(
        refused("tires"),
        DerivedError::Formula(FormulaError::UnknownKey {
            key: "tires".into()
        })
    );
    assert!(
        check_derived(
            None,
            &with("max(odometer, engine-hours × 33 mi/h)"),
            &meters
        )
        .is_ok()
    );
    let resettable = Meter {
        reset_to: Some(0),
        ..service_miles()
    };
    assert_eq!(
        check_derived(None, &resettable, &meters),
        Err(DerivedError::Meter(MeterError::DerivedResettable))
    );
    let required = Meter {
        required_for: vec![EventKind::Fuel],
        ..service_miles()
    };
    assert_eq!(
        check_derived(None, &required, &meters),
        Err(DerivedError::Meter(MeterError::DerivedResettable))
    );
}

#[test]
fn a_formula_in_use_keeps_its_fields_and_meters() {
    let (fields, meters) = (fields(), meters());
    assert_eq!(
        used_by(&key("odometer"), &fields, &meters),
        Some(&label("Cost per mile"))
    );
    assert_eq!(
        used_by(&key("engine-hours"), &fields, &meters),
        Some(&label("Service miles"))
    );
    assert_eq!(used_by(&key("trip-time"), &fields, &meters), None);
    assert_eq!(main_odometer(&meters), Some("odometer"));
}

// --- Derived readings ------------------------------------------------------------------------

#[test]
fn a_derived_meter_gets_a_value_at_every_reading_of_an_input() {
    let meters = meters();
    let readings = [
        past(ODOMETER, 1, miles(1_000.0)),
        past(HOURS, 2, 10 * 3_600),
        past(ODOMETER, 3, miles(1_100.0)),
        past(OIL_LIFE, 4, 5_000),
        past(HOURS, 5, 12 * 3_600),
    ];
    let derived = derived_readings(&meters, &readings);
    let values: Vec<(UnixMillis, i64)> = derived
        .iter()
        .map(|reading| (reading.at, reading.value))
        .collect();
    // Nothing until both inputs are read; none at Oil life's reading.
    assert_eq!(
        values,
        [
            (hours(2), plus(1_000.0, 330.0)),
            (hours(3), plus(1_100.0, 330.0)),
            (hours(5), plus(1_100.0, 396.0)),
        ]
    );
    assert!(
        derived
            .iter()
            .all(|reading| reading.meter == test_id(SERVICE_MILES))
    );
    let all = with_derived(&meters, &readings);
    assert_eq!(all.len(), readings.len() + 3);
    assert!(all.windows(2).all(|pair| pair[0].at <= pair[1].at));
}

#[test]
fn a_job_counts_a_derived_meter_from_when_it_was_done() {
    let meters = meters();
    let readings = [
        past(ODOMETER, 1, miles(1_000.0)),
        past(HOURS, 2, 10 * 3_600),
    ];
    let mut done = vec![DoneTask {
        requirement: test_id(50),
        at: hours(3),
        event: test_id(60),
        readings: vec![Reading {
            meter: test_id(ODOMETER),
            value: miles(1_200.0),
            reset: false,
        }],
        next_due: None,
    }];
    add_derived_to_done(&mut done, &meters, &readings);
    let service = done[0]
        .readings
        .iter()
        .find(|reading| reading.meter == test_id(SERVICE_MILES))
        .unwrap();
    // The job's own odometer, and the latest engine hours before it.
    assert_eq!(service.value, plus(1_200.0, 330.0));
}

// --- Values on events ------------------------------------------------------------------------

#[test]
fn each_event_sees_its_values_and_since_the_last() {
    let (fields, meters) = (fields(), meters());
    let readings = [
        past(ODOMETER, 0, miles(1_000.0)),
        past(HOURS, 1, 10 * 3_600),
        past(ODOMETER, 10, miles(1_400.0)),
    ];
    let vehicle = Vehicle {
        fields: &fields,
        meters: &meters,
        readings: &readings,
    };
    let (first, second) = (
        fuel_up(0, 1_000.0, Some(50)),
        fuel_up(10, 1_400.0, Some(60)),
    );
    let scopes = scopes(&[&first, &second], &vehicle);
    let now = &scopes[1];
    assert_eq!(now.values["cost"], Quantity::money(60.0, CurrencyCode::USD));
    assert!((now.values["volume"].value - 37.854).abs() < 1e-9);
    assert_eq!(now.values["highway"], Quantity::percentage(0.75));
    assert_eq!(
        now.values["trip-time"],
        Quantity::of(5_400.0, Dimension::TIME)
    );
    assert!((now.values["service-miles"].value - (1_730.0 * MILE)).abs() < 1.0);
    assert!((now.since_last["odometer"].value - 400.0 * MILE).abs() < 1.0);
    assert_eq!(
        now.time_since_last,
        Some(Quantity::of(36_000.0, Dimension::TIME))
    );
    // The first has nothing before it; the derived meter has no value before engine hours.
    assert!(scopes[0].since_last.is_empty());
    assert_eq!(scopes[0].time_since_last, None);
    assert!(!scopes[0].values.contains_key("service-miles"));
}

#[test]
fn a_calculated_field_is_worked_out_on_its_event() {
    let (fields, meters) = (fields(), meters());
    let vehicle = Vehicle {
        fields: &fields,
        meters: &meters,
        readings: &[],
    };
    let events = [
        fuel_up(0, 1_000.0, Some(50)),
        fuel_up(10, 1_400.0, Some(60)),
    ];
    let scopes = scopes(&[&events[0], &events[1]], &vehicle);
    let zone = TimeZone::parse("UTC").unwrap();
    let units = Preferences::default();
    let per_mile = &fields[1].1;
    let value = calculate(per_mile, &scopes, 1, &meters, &zone, &units)
        .unwrap()
        .unwrap();
    // $60 over 400 mi, to the cent.
    assert!((in_units(&value, &units).unwrap() - 0.15).abs() < 1e-9);
    assert_eq!(
        calculate(per_mile, &scopes, 0, &meters, &zone, &units),
        Some(Err(NoValue::NoEarlier))
    );
    assert_eq!(
        calculate(&fields[0].1, &scopes, 1, &meters, &zone, &units),
        None
    );
    let average = calculated(
        "Average trip",
        "average-trip",
        "mean(trip-time, cumulative)",
        ShowsAs::Duration,
    );
    let value = calculate(&average, &scopes, 1, &meters, &zone, &units)
        .unwrap()
        .unwrap();
    assert_eq!(value, Quantity::of(5_400.0, Dimension::TIME));
}

#[test]
fn a_calculated_field_is_never_required_or_typed() {
    let mut per_mile = fields()[1].1.clone();
    per_mile.required_for = vec![EventKind::Fuel];
    assert!(per_mile.calculated());
    assert!(!per_mile.required_on(EventKind::Fuel));
    assert!(crate::custom_field::check_field(&per_mile.kind, None).is_ok());
    let too_precise = FieldKind::Calculated {
        formula: Formula::parse("cost").unwrap(),
        shows: ShowsAs::Number { decimals: 5 },
    };
    assert!(crate::custom_field::check_field(&too_precise, None).is_err());
}
