use proptest::prelude::*;

use super::*;
use crate::location::TimeZone;
use crate::preferences::{CurrencyCode, EconomyUnit, Preferences};
use crate::time::UnixMillis;

const MILE: f64 = 1_609.344;
const HOUR: f64 = 3_600.0;
const DAY: i64 = 86_400_000;

fn close(a: f64, b: f64) -> bool {
    (a - b).abs() <= 1e-9 * a.abs().max(b.abs()).max(1.0)
}

const fn info(dimension: Dimension, meter: bool) -> KeyInfo {
    KeyInfo {
        dimension,
        percent: false,
        meter,
    }
}

/// A fuel-up's keys: cost, volume, trip-time, the odometer and engine hours, and oil life.
fn keys(key: &str) -> Option<KeyInfo> {
    match key {
        "cost" => Some(info(Dimension::MONEY, false)),
        "volume" => Some(info(Dimension::VOLUME, false)),
        "trip-time" => Some(info(Dimension::TIME, false)),
        "trip-distance" => Some(info(Dimension::DISTANCE, false)),
        "odometer" => Some(info(Dimension::DISTANCE, true)),
        "engine-hours" => Some(info(Dimension::TIME, true)),
        "oil-life" => Some(KeyInfo {
            dimension: Dimension::NONE,
            percent: true,
            meter: true,
        }),
        _ => None,
    }
}

fn parse(text: &str) -> Formula {
    Formula::parse(text).unwrap()
}

fn refused(text: &str) -> FormulaError {
    Formula::parse(text)
        .and_then(|formula| formula.check(&keys).map(|_| ()))
        .unwrap_err()
}

fn utc() -> TimeZone {
    TimeZone::parse("UTC").unwrap()
}

/// `text` worked out on `scope` with no earlier events, in the default units.
fn value_on(text: &str, scope: &Scope) -> Result<Quantity, NoValue> {
    let units = Preferences::default();
    let zone = utc();
    let formula = parse(text);
    formula.check(&keys).unwrap();
    formula.evaluate(&Context {
        current: scope,
        earlier: &[],
        main_distance: Some("odometer"),
        zone: &zone,
        units: &units,
    })
}

fn number_of(text: &str) -> f64 {
    value_on(text, &Scope::default()).unwrap().value
}

/// A fuel-up on day `day` with `trip-time` minutes (if any) and the odometer at `miles`.
fn fuel_up(day: i64, minutes: Option<f64>, miles: f64) -> Scope {
    let mut scope = Scope {
        at: UnixMillis::from_millis(1_800_000_000_000 + day * DAY),
        ..Scope::default()
    };
    if let Some(minutes) = minutes {
        scope.values.insert(
            "trip-time".into(),
            Quantity::of(minutes * 60.0, Dimension::TIME),
        );
    }
    scope.values.insert(
        "odometer".into(),
        Quantity::of(miles * MILE, Dimension::DISTANCE),
    );
    scope
}

/// `text` over fuel-ups (the last is the event itself), in minutes.
fn minutes_over(text: &str, fuel_ups: &[Scope]) -> Result<f64, NoValue> {
    let units = Preferences::default();
    let zone = utc();
    let (current, earlier) = fuel_ups.split_last().unwrap();
    let formula = parse(text);
    formula.check(&keys).unwrap();
    formula
        .evaluate(&Context {
            current,
            earlier,
            main_distance: Some("odometer"),
            zone: &zone,
            units: &units,
        })
        .map(|quantity| quantity.value / 60.0)
}

// --- Reading ---------------------------------------------------------------------------

#[test]
fn arithmetic_follows_the_usual_order() {
    assert!(close(number_of("1 + 2 × 3"), 7.0));
    assert!(close(number_of("(1 + 2) * 3"), 9.0));
    assert!(close(number_of("10 − 4 - 3"), 3.0));
    assert!(close(number_of("12 / 3 ÷ 2"), 2.0));
    assert!(close(number_of("−2 × 3"), -6.0));
    assert!(close(number_of("--2"), 2.0));
    assert!(close(number_of("2.5 * 4"), 10.0));
}

#[test]
fn keys_may_contain_hyphens_and_a_spaced_minus_is_a_minus() {
    assert_eq!(parse("trip-time").expr(), &Expr::Key("trip-time".into()));
    assert!(matches!(
        parse("trip-time - trip-time").expr(),
        Expr::Binary { op: Op::Minus, .. }
    ));
    assert!(matches!(
        parse("odometer -5").expr(),
        Expr::Binary { op: Op::Minus, .. }
    ));
}

#[test]
fn numbers_carry_units() {
    let speed = value_on("33 mi/h", &Scope::default()).unwrap();
    assert_eq!(speed.dimension, Dimension::SPEED);
    assert!(close(speed.value, 33.0 * MILE / HOUR));
    let volume = value_on("5 gal", &Scope::default()).unwrap();
    assert_eq!(volume.dimension, Dimension::VOLUME);
    assert!(close(volume.value, 5.0 * 3.785_411_784));
    assert!(close(number_of("90 days ÷ 1 h"), 90.0 * 24.0));
    let half = value_on("50%", &Scope::default()).unwrap();
    assert!(half.percent && close(half.value, 0.5));
    // `÷` is always a division: miles divided by a value keyed `h`… which isn't a key here.
    assert_eq!(
        refused("5 mi ÷ h"),
        FormulaError::UnknownKey { key: "h".into() }
    );
}

#[test]
fn what_doesnt_read_is_refused_with_where() {
    assert_eq!(Formula::parse("  ").unwrap_err(), FormulaError::Empty);
    assert_eq!(
        Formula::parse(&"1 + ".repeat(200)).unwrap_err(),
        FormulaError::TooLong
    );
    assert_eq!(
        Formula::parse("1 +").unwrap_err(),
        FormulaError::UnexpectedEnd
    );
    assert_eq!(
        Formula::parse("(1 + 2").unwrap_err(),
        FormulaError::Unclosed { at: 0 }
    );
    assert_eq!(
        Formula::parse("1 2").unwrap_err(),
        FormulaError::Unexpected {
            at: 2,
            found: "2".into()
        }
    );
    assert_eq!(
        Formula::parse("cost $").unwrap_err(),
        FormulaError::Unexpected {
            at: 5,
            found: "$".into()
        }
    );
    assert_eq!(
        Formula::parse("frob(1)").unwrap_err(),
        FormulaError::UnknownFunction {
            at: 0,
            name: "frob".into()
        }
    );
    assert_eq!(
        Formula::parse("round(cost, 1.5)").unwrap_err(),
        FormulaError::BadPlaces
    );
    assert_eq!(
        Formula::parse("round(cost, 7)").unwrap_err(),
        FormulaError::BadPlaces
    );
    assert!(Formula::parse("mean(trip-time, last 0)").is_err());
    assert!(Formula::parse("mean(trip-time, last 2.5)").is_err());
    assert!(Formula::parse("mean(trip-time, last 3 parsecs)").is_err());
    assert!(Formula::parse("mean(trip-time, last 3 days on odometer)").is_err());
}

#[test]
fn over_reads_each_span() {
    let over = |text: &str| {
        let Expr::Over { over, .. } = parse(text).expr().clone() else {
            panic!("{text} has no Over");
        };
        over
    };
    assert_eq!(over("mean(trip-time, cumulative)"), Over::Cumulative);
    assert_eq!(over("mean(trip-time, last 5)"), Over::Events(5));
    assert_eq!(over("mean(trip-time, last 5 events)"), Over::Events(5));
    assert_eq!(over("mean(trip-time, last 30 days)"), Over::Days(30));
    assert_eq!(over("mean(trip-time, last 2 weeks)"), Over::Weeks(2));
    assert_eq!(over("mean(trip-time, last 3 months)"), Over::Months(3));
    assert_eq!(
        over("mean(trip-time, last 1000 mi)"),
        Over::Distance {
            amount: 1000.0,
            unit: SimpleUnit::Miles,
            meter: None
        }
    );
    assert_eq!(
        over("mean(trip-time, last 50 h on engine-hours)"),
        Over::Hours {
            amount: 50.0,
            meter: Some("engine-hours".into())
        }
    );
    // Listed values: a comma not followed by `last` or `cumulative`.
    assert!(matches!(
        parse("mean(trip-time, trip-time)").expr(),
        Expr::Call { args, .. } if args.len() == 2
    ));
}

#[test]
fn it_lists_the_keys_it_uses() {
    let formula =
        parse("cost ÷ distance-since-last(odometer) + mean(volume, last 50 h on engine-hours)");
    let keys: Vec<String> = formula.keys().into_iter().collect();
    assert_eq!(keys, ["cost", "engine-hours", "odometer", "volume"]);
    assert!(formula.uses_history());
    assert!(!parse("cost × 2").uses_history());
}

#[test]
fn a_formula_is_stored_as_its_text() {
    let formula = parse(" cost ÷ volume ");
    assert_eq!(formula.as_str(), "cost ÷ volume");
    let json = serde_json::to_string(&formula).unwrap();
    assert_eq!(json, "\"cost ÷ volume\"");
    assert_eq!(serde_json::from_str::<Formula>(&json).unwrap(), formula);
    assert!(serde_json::from_str::<Formula>("\"1 +\"").is_err());
}

// --- Checking --------------------------------------------------------------------------

#[test]
fn dimensions_combine_and_must_agree() {
    let dimension = |text: &str| parse(text).check(&keys).unwrap().dimension;
    assert_eq!(
        dimension("cost ÷ distance-since-last(odometer)"),
        Dimension::MONEY_PER_DISTANCE
    );
    assert_eq!(dimension("trip-distance ÷ volume"), Dimension::ECONOMY);
    assert_eq!(
        dimension("odometer + engine-hours × 33 mi/h"),
        Dimension::DISTANCE
    );
    assert_eq!(dimension("trip-distance ÷ trip-time"), Dimension::SPEED);
    assert_eq!(dimension("count(trip-time, last 5)"), Dimension::NONE);
    assert_eq!(dimension("time-since-last"), Dimension::TIME);
    let checked = parse("oil-life − 10%").check(&keys).unwrap();
    assert!(checked.percent);
    assert!(!parse("oil-life × 2").check(&keys).unwrap().percent);
    assert_eq!(
        refused("trip-time + volume"),
        FormulaError::Mismatch {
            op: Op::Plus,
            left: "trip-time".into(),
            right: "volume".into(),
            left_dimension: Dimension::TIME,
            right_dimension: Dimension::VOLUME,
        }
    );
    assert_eq!(
        refused("mean(cost, volume)"),
        FormulaError::MixedValues {
            function: "mean".into()
        }
    );
}

#[test]
fn keys_and_meters_must_be_right() {
    assert_eq!(
        refused("cost ÷ tires"),
        FormulaError::UnknownKey {
            key: "tires".into()
        }
    );
    assert_eq!(
        refused("distance-since-last(cost)"),
        FormulaError::NotAMeter { key: "cost".into() }
    );
    assert_eq!(
        refused("mean(cost, last 50 h)"),
        FormulaError::HoursNeedMeter
    );
    assert_eq!(
        refused("mean(cost, last 50 h on odometer)"),
        FormulaError::WrongMeter {
            key: "odometer".into(),
            wanted: Dimension::TIME
        }
    );
    assert_eq!(
        refused("mean(mean(cost, last 2), last 5)"),
        FormulaError::NestedOver
    );
    // A past-events function among listed values is fine.
    assert!(parse("max(cost, mean(cost, last 5))").check(&keys).is_ok());
}

#[test]
fn the_result_must_be_what_the_field_shows() {
    let formula = parse("cost ÷ distance-since-last(odometer)");
    assert!(
        formula
            .check_shows(&keys, ShowsAs::MoneyPerDistance)
            .is_ok()
    );
    assert_eq!(
        formula.check_shows(&keys, ShowsAs::Money).unwrap_err(),
        FormulaError::WrongResult {
            found: Dimension::MONEY_PER_DISTANCE,
            wanted: Dimension::MONEY
        }
    );
    assert!(
        parse("oil-life")
            .check_shows(&keys, ShowsAs::Percent { decimals: 0 })
            .is_ok()
    );
    assert!(
        parse("count(cost, cumulative)")
            .check_shows(&keys, ShowsAs::Number { decimals: 0 })
            .is_ok()
    );
}

#[test]
fn refusals_read_as_words() {
    let english = Translator::english();
    assert_eq!(
        refused("trip-time + volume").describe(english),
        "trip-time + volume: can't add a time to a volume."
    );
    assert_eq!(
        refused("trip-time - volume").describe(english),
        "trip-time − volume: can't take a volume from a time."
    );
    assert_eq!(
        refused("cost ÷ tires").describe(english),
        "There's no value called \"tires\" here."
    );
    let odd = Dimension::DISTANCE
        .times(Dimension::DISTANCE)
        .per(Dimension::TIME);
    assert_eq!(odd.describe(english), "an unusual unit (distance² ÷ time)");
    // Every refusal has words.
    let all = [
        FormulaError::Empty,
        FormulaError::TooLong,
        FormulaError::UnexpectedEnd,
        refused("1 2"),
        refused("(1"),
        refused("frob(1)"),
        FormulaError::BadPlaces,
        refused("cost ÷ tires"),
        refused("distance-since-last(cost)"),
        refused("mean(cost, last 50 h on odometer)"),
        FormulaError::HoursNeedMeter,
        refused("trip-time + volume"),
        refused("mean(cost, volume)"),
        FormulaError::NestedOver,
        parse("cost")
            .check_shows(&keys, ShowsAs::Volume)
            .unwrap_err(),
    ];
    for error in all {
        let words = error.describe(english);
        assert!(
            !words.is_empty() && !words.contains('{'),
            "{error:?}: {words}"
        );
    }
}

// --- Working out -------------------------------------------------------------------------

#[test]
fn cost_per_mile_on_a_fuel_up() {
    let mut fuel_up = Scope::default();
    fuel_up
        .values
        .insert("cost".into(), Quantity::money(58.20, CurrencyCode::USD));
    fuel_up.since_last.insert(
        "odometer".into(),
        Quantity::of(410.3 * MILE, Dimension::DISTANCE),
    );
    let units = Preferences::default();
    let per_mile = value_on("cost ÷ distance-since-last(odometer)", &fuel_up).unwrap();
    assert_eq!(per_mile.currency, Some(CurrencyCode::USD));
    assert!(close(in_units(&per_mile, &units).unwrap(), 58.20 / 410.3));
    let rounded = value_on("round(cost ÷ distance-since-last(odometer), 2)", &fuel_up).unwrap();
    assert!(close(in_units(&rounded, &units).unwrap(), 0.14));
}

#[test]
fn missing_values_give_no_value_and_why() {
    let first = Scope::default();
    assert_eq!(
        value_on("cost × 2", &first),
        Err(NoValue::Missing { key: "cost".into() })
    );
    assert_eq!(
        value_on("distance-since-last(odometer)", &first),
        Err(NoValue::NoEarlier)
    );
    assert_eq!(value_on("time-since-last", &first), Err(NoValue::NoEarlier));
    assert_eq!(value_on("1 ÷ 0", &first), Err(NoValue::DivideByZero));
    let mut mixed = Scope::default();
    mixed
        .values
        .insert("cost".into(), Quantity::money(10.0, CurrencyCode::USD));
    mixed.values.insert(
        "volume".into(),
        Quantity::money(10.0, CurrencyCode::parse("EUR").unwrap()),
    );
    let units = Preferences::default();
    let zone = utc();
    let formula = parse("cost + volume");
    let result = formula.evaluate(&Context {
        current: &mixed,
        earlier: &[],
        main_distance: None,
        zone: &zone,
        units: &units,
    });
    assert_eq!(result, Err(NoValue::MixedCurrencies));
    for why in [
        NoValue::Missing { key: "cost".into() },
        NoValue::NoEarlier,
        NoValue::NothingInOver,
        NoValue::DivideByZero,
        NoValue::MixedCurrencies,
        NoValue::NotPositive,
        NoValue::TooFew,
        NoValue::NoOdometer,
        NoValue::NotFinite,
        NoValue::Mismatch,
    ] {
        let words = Translator::english().tr_with(why.message(), &[("key", "cost".into())]);
        assert!(!words.is_empty() && !words.contains('{'), "{why:?}");
    }
}

#[test]
fn functions_over_listed_values() {
    assert!(close(number_of("mean(1, 2, 3)"), 2.0));
    assert!(close(number_of("median(3, 1, 2)"), 2.0));
    assert!(close(number_of("median(4, 1, 3, 2)"), 2.5));
    assert!(close(number_of("geometric_mean(2, 8)"), 4.0));
    assert!(close(number_of("harmonic_mean(1, 4)"), 1.6));
    assert!(close(number_of("min(3, -1, 2)"), -1.0));
    assert!(close(number_of("max(3, -1, 2)"), 3.0));
    assert!(close(number_of("sum(3, -1, 2)"), 4.0));
    assert!(close(number_of("count(3, -1, 2)"), 3.0));
    assert!(close(
        number_of("stddev(2, 4, 4, 4, 5, 5, 7, 9)"),
        2.138_089_935_299_395
    ));
    assert!(close(number_of("abs(−3)"), 3.0));
    assert!(close(number_of("round(2.345, 2)"), 2.35));
    assert!(close(number_of("round(2.5, 0)"), 3.0));
    let empty = Scope::default();
    assert_eq!(
        value_on("geometric_mean(0, 1)", &empty),
        Err(NoValue::NotPositive)
    );
    assert_eq!(
        value_on("harmonic_mean(-1, 1)", &empty),
        Err(NoValue::NotPositive)
    );
    assert_eq!(value_on("stddev(1)", &empty), Err(NoValue::TooFew));
    // The geometric mean of distances is a distance.
    let mean = value_on("geometric_mean(1 mi, 4 mi)", &empty).unwrap();
    assert_eq!(mean.dimension, Dimension::DISTANCE);
    assert!(close(mean.value, 2.0 * MILE));
}

#[test]
fn rounding_is_as_shown_in_the_persons_units() {
    let units = Preferences::default();
    let ten_km = value_on("round(10 km, 1)", &Scope::default()).unwrap();
    assert!(close(in_units(&ten_km, &units).unwrap(), 6.2));
    let litres_per_100 = Preferences {
        economy: EconomyUnit::LitersPer100Km,
        ..Preferences::default()
    };
    // 30 mpg (US) is about 7.84 L/100 km: shown as L/100 km, the inverse.
    let economy = Quantity::of(30.0 * MILE / 3.785_411_784, Dimension::ECONOMY);
    assert!(close(
        in_units(&economy, &litres_per_100).unwrap(),
        100_000.0 / (30.0 * MILE / 3.785_411_784)
    ));
    assert!(close(in_units(&economy, &units).unwrap(), 30.0));
    let oil = Quantity::percentage(0.8);
    assert!(close(in_units(&oil, &units).unwrap(), 80.0));
}

#[test]
fn over_picks_past_events_up_to_and_including_this_one() {
    let fuel_ups: Vec<Scope> = [10.0, 20.0, 30.0, 40.0, 50.0, 60.0]
        .iter()
        .enumerate()
        .map(|(day, minutes)| {
            let day = i64::try_from(day).unwrap();
            fuel_up(
                day * 10,
                Some(*minutes),
                1_000.0 + 300.0 * f64::from(u8::try_from(day).unwrap()),
            )
        })
        .collect();
    assert!(close(
        minutes_over("mean(trip-time, last 5)", &fuel_ups).unwrap(),
        40.0
    ));
    assert!(close(
        minutes_over("mean(trip-time, last 2 events)", &fuel_ups).unwrap(),
        55.0
    ));
    assert!(close(
        minutes_over("mean(trip-time, cumulative)", &fuel_ups).unwrap(),
        35.0
    ));
    assert!(close(
        minutes_over("max(trip-time, last 100)", &fuel_ups).unwrap(),
        60.0
    ));
    // Days 50 and 40 are within 10 days of day 50; 30 isn't.
    assert!(close(
        minutes_over("sum(trip-time, last 10 days)", &fuel_ups).unwrap(),
        110.0
    ));
    assert!(close(
        minutes_over("sum(trip-time, last 3 weeks)", &fuel_ups).unwrap(),
        150.0
    ));
    // 2,500 mi now; 600 mi back reaches 1,900 (40 and 50 and 60 minutes).
    assert!(close(
        minutes_over("mean(trip-time, last 600 mi)", &fuel_ups).unwrap(),
        50.0
    ));
    assert!(close(
        minutes_over("mean(trip-time, last 966 km on odometer)", &fuel_ups).unwrap(),
        50.0
    ));
    let count = minutes_over("count(trip-time, last 5)", &fuel_ups).unwrap() * 60.0;
    assert!(close(count, 5.0));
}

#[test]
fn over_leaves_out_events_without_a_value() {
    let fuel_ups = [
        fuel_up(0, Some(10.0), 100.0),
        fuel_up(1, None, 200.0),
        fuel_up(2, Some(30.0), 300.0),
    ];
    assert!(close(
        minutes_over("mean(trip-time, cumulative)", &fuel_ups).unwrap(),
        20.0
    ));
    assert!(close(
        minutes_over("count(trip-time, cumulative)", &fuel_ups).unwrap() * 60.0,
        2.0
    ));
    let none = [fuel_up(0, None, 100.0), fuel_up(1, None, 200.0)];
    assert_eq!(
        minutes_over("mean(trip-time, cumulative)", &none),
        Err(NoValue::NothingInOver)
    );
}

#[test]
fn over_months_counts_calendar_months_in_the_zone() {
    // Day 0 is 2027-01-15 (UTC); 2 months back from day 70 (2027-03-26) is 2027-01-26.
    let fuel_ups = [
        fuel_up(0, Some(10.0), 100.0),
        fuel_up(20, Some(20.0), 200.0),
        fuel_up(70, Some(30.0), 300.0),
    ];
    assert!(close(
        minutes_over("sum(trip-time, last 2 months)", &fuel_ups).unwrap(),
        50.0
    ));
    assert!(close(
        minutes_over("sum(trip-time, last 3 months)", &fuel_ups).unwrap(),
        60.0
    ));
}

#[test]
fn over_distance_needs_an_odometer() {
    let units = Preferences::default();
    let zone = utc();
    let scope = fuel_up(0, Some(10.0), 100.0);
    let formula = parse("mean(trip-time, last 100 mi)");
    let context = Context {
        current: &scope,
        earlier: &[],
        main_distance: None,
        zone: &zone,
        units: &units,
    };
    assert_eq!(formula.evaluate(&context), Err(NoValue::NoOdometer));
    let without_reading = Scope::default();
    let context = Context {
        current: &without_reading,
        main_distance: Some("odometer"),
        ..context
    };
    assert_eq!(
        formula.evaluate(&context),
        Err(NoValue::Missing {
            key: "odometer".into()
        })
    );
}

// --- Properties ----------------------------------------------------------------------------

fn dimension() -> impl Strategy<Value = Dimension> {
    (-3_i8..=3, -3_i8..=3, -3_i8..=3, -3_i8..=3).prop_map(|(distance, volume, time, money)| {
        Dimension {
            distance,
            volume,
            time,
            money,
        }
    })
}

fn expr() -> impl Strategy<Value = Expr> {
    let key = prop::sample::select(vec!["cost", "volume", "trip-time", "odometer"]);
    let unit = prop::option::of(prop::sample::select(vec![
        Unit::Simple(SimpleUnit::Miles),
        Unit::Simple(SimpleUnit::Liters),
        Unit::Per(SimpleUnit::Miles, SimpleUnit::Hours),
        Unit::Percent,
    ]));
    let leaf = prop_oneof![
        ((0_u32..100_000), unit).prop_map(|(hundredths, unit)| Expr::Number {
            value: f64::from(hundredths) / 100.0,
            unit,
        }),
        key.prop_map(|key| Expr::Key(key.to_owned())),
        Just(Expr::SinceLast("odometer".into())),
        Just(Expr::TimeSinceLast),
    ];
    leaf.prop_recursive(4, 24, 3, |inner| {
        let op = prop::sample::select(vec![Op::Plus, Op::Minus, Op::Times, Op::Divide]);
        let function = prop::sample::select(Function::ALL.to_vec());
        prop_oneof![
            (op, inner.clone(), inner.clone()).prop_map(|(op, left, right)| Expr::Binary {
                op,
                left: Box::new(left),
                right: Box::new(right),
            }),
            inner
                .clone()
                .prop_map(|inner| Expr::Negate(Box::new(inner))),
            inner.clone().prop_map(|inner| Expr::Abs(Box::new(inner))),
            (inner.clone(), 0_u8..=6).prop_map(|(inner, places)| Expr::Round {
                expr: Box::new(inner),
                places,
            }),
            (function.clone(), prop::collection::vec(inner.clone(), 1..3))
                .prop_map(|(function, args)| Expr::Call { function, args }),
            (function, inner, 1_u32..20).prop_map(|(function, inner, n)| Expr::Over {
                function,
                expr: Box::new(inner),
                over: Over::Events(n),
            }),
        ]
    })
}

proptest! {
    #[test]
    fn dividing_undoes_multiplying(a in dimension(), b in dimension()) {
        prop_assert_eq!(a.times(b).per(b), a);
        prop_assert_eq!(a.per(a), Dimension::NONE);
        prop_assert_eq!(a.times(b), b.times(a));
    }

    #[test]
    fn a_formula_printed_reads_back_the_same(expr in expr()) {
        let printed = expr.to_string();
        prop_assume!(printed.chars().count() <= FORMULA_MAX_CHARS);
        let read = Formula::parse(&printed);
        prop_assert!(read.is_ok(), "{} didn't read: {:?}", printed, read);
        let read = read.unwrap();
        prop_assert_eq!(read.expr(), &expr, "{}", printed);
    }
}
