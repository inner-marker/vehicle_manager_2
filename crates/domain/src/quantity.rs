//! Quantities in each person's units (decisions #007, #029): typed values become the stored
//! whole numbers, and stored numbers are shown back with display rounding.
//!
//! **Stored** (plan §3; as custom fields already define them): distance in **meters**,
//! duration in **seconds**, volume in **milliliters**, fuel economy in **meters per liter**,
//! money in the currency's **minor unit** (cents). Conversions go through `uom`, so the factors
//! (a mile, a US gallon, an imperial gallon) come from one well-tested place.
//!
//! **Typed:** a plain decimal number in the person's unit ("12.3", "1,234.5"; commas are
//! ignored), never negative. Durations are typed as "H:MM" or "H:MM:SS". Meter readings are typed
//! in the meter's own unit, with at most its decimal places.
//!
//! # Examples
//!
//! ```
//! use domain::preferences::{DistanceUnit, VolumeUnit};
//! use domain::quantity::{parse_distance, parse_volume, show_distance, show_volume};
//!
//! assert_eq!(parse_distance("12.5", DistanceUnit::Miles), Ok(20_117));
//! assert_eq!(show_distance(20_117, DistanceUnit::Kilometers), "20.1 km");
//! let gallons = parse_volume("10", VolumeUnit::UsGallons).unwrap();
//! assert_eq!(gallons, 37_854);
//! assert_eq!(show_volume(gallons, VolumeUnit::Liters), "37.85 L");
//! ```

use serde::{Deserialize, Serialize};
use uom::si::f64::{Length, Time, Volume};
use uom::si::length::{kilometer, meter, mile};
use uom::si::time::{hour, second};
use uom::si::volume::{gallon, gallon_imperial, liter, milliliter};

use crate::i18n::{MessageId, Translator, messages};
use crate::meter::MeterUnit;
use crate::preferences::{CurrencyCode, DistanceUnit, EconomyUnit, VolumeUnit};

/// Why a typed quantity was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum QuantityError {
    /// Not a number.
    NotANumber,
    /// Below zero.
    Negative,
    /// Too large to store.
    TooLarge,
    /// More decimal places than allowed (e.g. a meter that shows tenths).
    TooPrecise,
    /// Not a duration ("1:30" or "1:30:15").
    NotADuration,
}

impl QuantityError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::NotANumber => messages::QUANTITY_NOT_A_NUMBER,
            Self::Negative => messages::QUANTITY_NEGATIVE,
            Self::TooLarge => messages::QUANTITY_TOO_LARGE,
            Self::TooPrecise => messages::QUANTITY_TOO_PRECISE,
            Self::NotADuration => messages::QUANTITY_NOT_A_DURATION,
        }
    }
}

/// The largest stored value accepted (well inside both `i64` and `f64`'s exact integers).
const MAX_STORED: f64 = 1e15;

/// A typed decimal: commas and spaces ignored, never negative. Also its number of decimals.
fn number(text: &str) -> Result<(f64, usize), QuantityError> {
    let cleaned: String = text
        .chars()
        .filter(|c| *c != ',' && !c.is_whitespace())
        .collect();
    if cleaned.is_empty()
        || !cleaned
            .chars()
            .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
    {
        return Err(QuantityError::NotANumber);
    }
    let value: f64 = cleaned.parse().map_err(|_| QuantityError::NotANumber)?;
    if !value.is_finite() {
        return Err(QuantityError::NotANumber);
    }
    if value < 0.0 {
        return Err(QuantityError::Negative);
    }
    let decimals = cleaned
        .split_once('.')
        .map_or(0, |(_, fraction)| fraction.len());
    Ok((value, decimals))
}

/// `value` rounded to the nearest whole number, for storing.
fn whole(value: f64) -> Result<i64, QuantityError> {
    let rounded = value.round();
    if !rounded.is_finite() || rounded.abs() > MAX_STORED {
        return Err(QuantityError::TooLarge);
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "checked above: finite, whole, and within ±1e15, which fits an i64 exactly"
    )]
    Ok(rounded as i64)
}

/// A stored whole number as `f64` (exact: stored values stay within ±1e15).
fn float(stored: i64) -> f64 {
    #[expect(
        clippy::cast_precision_loss,
        reason = "stored values stay within ±1e15, below 2^53, so the conversion is exact"
    )]
    let value = stored as f64;
    value
}

/// A number with `decimals` places.
fn fixed(value: f64, decimals: usize) -> String {
    format!("{value:.decimals$}")
}

// --- Distance --------------------------------------------------------------------------------

/// A typed distance in `unit`, as meters.
///
/// # Errors
///
/// [`QuantityError`] if it isn't a non-negative number, or is too large.
pub fn parse_distance(text: &str, unit: DistanceUnit) -> Result<i64, QuantityError> {
    let (value, _) = number(text)?;
    let length = match unit {
        DistanceUnit::Miles => Length::new::<mile>(value),
        DistanceUnit::Kilometers => Length::new::<kilometer>(value),
    };
    whole(length.get::<meter>())
}

/// A distance in `unit` with one decimal place: "12.4 mi", "20.1 km".
#[must_use]
pub fn show_distance(meters: i64, unit: DistanceUnit) -> String {
    let length = Length::new::<meter>(float(meters));
    let tr = Translator::english();
    match unit {
        DistanceUnit::Miles => format!(
            "{} {}",
            fixed(length.get::<mile>(), 1),
            tr.tr(messages::UNIT_ABBR_MI)
        ),
        DistanceUnit::Kilometers => format!(
            "{} {}",
            fixed(length.get::<kilometer>(), 1),
            tr.tr(messages::UNIT_ABBR_KM)
        ),
    }
}

/// A distance in `unit` as a plain number for a form (one decimal place).
#[must_use]
pub fn distance_value(meters: i64, unit: DistanceUnit) -> String {
    let length = Length::new::<meter>(float(meters));
    fixed(
        match unit {
            DistanceUnit::Miles => length.get::<mile>(),
            DistanceUnit::Kilometers => length.get::<kilometer>(),
        },
        1,
    )
}

// --- Volume ----------------------------------------------------------------------------------

/// A typed volume in `unit`, as milliliters.
///
/// # Errors
///
/// [`QuantityError`] if it isn't a non-negative number, or is too large.
pub fn parse_volume(text: &str, unit: VolumeUnit) -> Result<i64, QuantityError> {
    let (value, _) = number(text)?;
    let volume = match unit {
        VolumeUnit::UsGallons => Volume::new::<gallon>(value),
        VolumeUnit::ImperialGallons => Volume::new::<gallon_imperial>(value),
        VolumeUnit::Liters => Volume::new::<liter>(value),
    };
    whole(volume.get::<milliliter>())
}

/// A volume in `unit` with the decimals a pump shows: "10.000 gal", "37.85 L".
#[must_use]
pub fn show_volume(milliliters: i64, unit: VolumeUnit) -> String {
    let tr = Translator::english();
    let (value, label) = volume_parts(milliliters, unit);
    format!("{value} {}", tr.tr(label))
}

/// A volume in `unit` as a plain number for a form.
#[must_use]
pub fn volume_value(milliliters: i64, unit: VolumeUnit) -> String {
    volume_parts(milliliters, unit).0
}

fn volume_parts(milliliters: i64, unit: VolumeUnit) -> (String, MessageId) {
    let volume = Volume::new::<milliliter>(float(milliliters));
    match unit {
        // US pumps show thousandths of a gallon; liters to hundredths.
        VolumeUnit::UsGallons => (fixed(volume.get::<gallon>(), 3), messages::UNIT_ABBR_GAL),
        VolumeUnit::ImperialGallons => (
            fixed(volume.get::<gallon_imperial>(), 3),
            messages::UNIT_ABBR_GAL_UK,
        ),
        VolumeUnit::Liters => (fixed(volume.get::<liter>(), 2), messages::UNIT_ABBR_L),
    }
}

// --- Fuel economy ----------------------------------------------------------------------------

/// Meters per liter in one mile per `volume` (a US or imperial gallon).
fn meters_per_liter_per_mpg(per: Volume) -> f64 {
    Length::new::<mile>(1.0).get::<meter>() / per.get::<liter>()
}

/// A typed fuel economy in `unit`, as meters per liter. L/100 km is the inverse of the others
/// (more liters is worse), and 0 L/100 km isn't an economy.
///
/// # Errors
///
/// [`QuantityError`] if it isn't a positive number (for L/100 km), or is too large.
pub fn parse_economy(text: &str, unit: EconomyUnit) -> Result<i64, QuantityError> {
    let (value, _) = number(text)?;
    let meters_per_liter = match unit {
        EconomyUnit::MpgUs => value * meters_per_liter_per_mpg(Volume::new::<gallon>(1.0)),
        EconomyUnit::MpgUk => value * meters_per_liter_per_mpg(Volume::new::<gallon_imperial>(1.0)),
        EconomyUnit::KmPerLiter => value * 1_000.0,
        EconomyUnit::LitersPer100Km => {
            if value == 0.0 {
                return Err(QuantityError::TooLarge);
            }
            100_000.0 / value
        }
    };
    whole(meters_per_liter)
}

/// A fuel economy in `unit` with one decimal place: "25.3 mpg", "9.3 L/100 km".
#[must_use]
pub fn show_economy(meters_per_liter: i64, unit: EconomyUnit) -> String {
    let tr = Translator::english();
    let label = match unit {
        EconomyUnit::MpgUs => messages::UNIT_ABBR_MPG,
        EconomyUnit::MpgUk => messages::UNIT_ABBR_MPG_UK,
        EconomyUnit::KmPerLiter => messages::UNIT_ABBR_KM_PER_L,
        EconomyUnit::LitersPer100Km => messages::UNIT_ABBR_L_PER_100_KM,
    };
    format!("{} {}", economy_value(meters_per_liter, unit), tr.tr(label))
}

/// A fuel economy in `unit` as a plain number for a form (one decimal place; empty for none).
#[must_use]
pub fn economy_value(meters_per_liter: i64, unit: EconomyUnit) -> String {
    let value = float(meters_per_liter);
    let shown = match unit {
        EconomyUnit::MpgUs => value / meters_per_liter_per_mpg(Volume::new::<gallon>(1.0)),
        EconomyUnit::MpgUk => value / meters_per_liter_per_mpg(Volume::new::<gallon_imperial>(1.0)),
        EconomyUnit::KmPerLiter => value / 1_000.0,
        EconomyUnit::LitersPer100Km if value > 0.0 => 100_000.0 / value,
        EconomyUnit::LitersPer100Km => return String::new(),
    };
    fixed(shown, 1)
}

/// Whether bigger numbers are worse in `unit` (L/100 km), so a range's ends swap when stored.
#[must_use]
pub const fn economy_is_inverse(unit: EconomyUnit) -> bool {
    matches!(unit, EconomyUnit::LitersPer100Km)
}

// --- Duration --------------------------------------------------------------------------------

/// A typed duration, "H:MM" or "H:MM:SS", as seconds.
///
/// # Errors
///
/// [`QuantityError::NotADuration`] unless it's hours, then minutes (and seconds) under 60.
pub fn parse_duration(text: &str) -> Result<i64, QuantityError> {
    let parts: Vec<&str> = text.trim().split(':').collect();
    let field = |part: &str| -> Result<i64, QuantityError> {
        if part.is_empty() || !part.chars().all(|c| c.is_ascii_digit()) || part.len() > 9 {
            return Err(QuantityError::NotADuration);
        }
        part.parse().map_err(|_| QuantityError::NotADuration)
    };
    let (hours, minutes, seconds) = match parts.as_slice() {
        [h, m] => (field(h)?, field(m)?, 0),
        [h, m, s] => (field(h)?, field(m)?, field(s)?),
        _ => return Err(QuantityError::NotADuration),
    };
    if minutes >= 60 || seconds >= 60 {
        return Err(QuantityError::NotADuration);
    }
    Ok(hours * 3_600 + minutes * 60 + seconds)
}

/// A duration as "1:30" (or "1:30:15" when it has seconds).
#[must_use]
pub fn show_duration(seconds: i64) -> String {
    let (hours, rest) = (seconds / 3_600, seconds % 3_600);
    let (minutes, secs) = (rest / 60, rest % 60);
    if secs == 0 {
        format!("{hours}:{minutes:02}")
    } else {
        format!("{hours}:{minutes:02}:{secs:02}")
    }
}

// --- Meter readings --------------------------------------------------------------------------

/// A reading typed in a meter's `unit`, with at most `precision` decimals, as the stored raw
/// value (meters, or seconds for hours).
///
/// # Errors
///
/// [`QuantityError`] if it isn't a non-negative number, has too many decimals, or is too large.
pub fn parse_reading(text: &str, unit: MeterUnit, precision: u8) -> Result<i64, QuantityError> {
    let (value, decimals) = number(text)?;
    if decimals > usize::from(precision) {
        return Err(QuantityError::TooPrecise);
    }
    whole(match unit {
        MeterUnit::Miles => Length::new::<mile>(value).get::<meter>(),
        MeterUnit::Kilometers => Length::new::<kilometer>(value).get::<meter>(),
        MeterUnit::Hours => Time::new::<hour>(value).get::<second>(),
    })
}

/// A stored reading in the meter's unit, with its decimals, as a plain number: "52310",
/// "1234.5".
#[must_use]
pub fn reading_value(raw: i64, unit: MeterUnit, precision: u8) -> String {
    let value = float(raw);
    let shown = match unit {
        MeterUnit::Miles => Length::new::<meter>(value).get::<mile>(),
        MeterUnit::Kilometers => Length::new::<meter>(value).get::<kilometer>(),
        MeterUnit::Hours => Time::new::<second>(value).get::<hour>(),
    };
    fixed(shown, usize::from(precision))
}

/// A stored reading shown with thousands grouped: "52,310", "1,234.5" (for reading, not
/// typing; [`reading_value`] is the typed form).
#[must_use]
pub fn show_reading(raw: i64, unit: MeterUnit, precision: u8) -> String {
    let plain = reading_value(raw, unit, precision);
    let (whole, fraction) = plain
        .split_once('.')
        .map_or((plain.as_str(), None), |(w, f)| (w, Some(f)));
    let digits: Vec<char> = whole.chars().collect();
    let mut grouped = String::new();
    for (index, digit) in digits.iter().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            grouped.push(',');
        }
        grouped.push(*digit);
    }
    match fraction {
        Some(fraction) => format!("{grouped}.{fraction}"),
        None => grouped,
    }
}

// --- Money -----------------------------------------------------------------------------------

/// The currency's minor-unit decimals (ISO 4217): 0 for e.g. yen, 3 for e.g. Kuwaiti dinar,
/// otherwise 2.
#[must_use]
pub fn currency_decimals(currency: CurrencyCode) -> u8 {
    match currency.as_str() {
        "JPY" | "KRW" | "VND" | "CLP" | "ISK" | "PYG" | "UGX" | "XAF" | "XOF" | "XPF" | "RWF"
        | "KMF" | "GNF" | "DJF" | "BIF" | "VUV" => 0,
        "BHD" | "KWD" | "OMR" | "JOD" | "TND" | "LYD" | "IQD" => 3,
        _ => 2,
    }
}

/// A typed amount in `currency` ("45.67"), as minor units (4567 cents).
///
/// # Errors
///
/// [`QuantityError`] if it isn't a non-negative number, has more decimals than the currency,
/// or is too large.
pub fn parse_money(text: &str, currency: CurrencyCode) -> Result<i64, QuantityError> {
    let (value, decimals) = number(text)?;
    let places = currency_decimals(currency);
    if decimals > usize::from(places) {
        return Err(QuantityError::TooPrecise);
    }
    whole(value * 10_f64.powi(i32::from(places)))
}

/// An amount as "45.67 USD".
#[must_use]
pub fn show_money(minor: i64, currency: CurrencyCode) -> String {
    let places = currency_decimals(currency);
    let value = float(minor) / 10_f64.powi(i32::from(places));
    format!(
        "{} {}",
        fixed(value, usize::from(places)),
        currency.as_str()
    )
}

/// The price of one `unit` of fuel, shown with one more decimal than the currency (as pumps
/// show it): "3.459". `None` for no fuel.
#[must_use]
pub fn unit_price(
    cost_minor: i64,
    volume_ml: i64,
    currency: CurrencyCode,
    unit: VolumeUnit,
) -> Option<String> {
    let amount = Volume::new::<milliliter>(float(volume_ml));
    let units = match unit {
        VolumeUnit::UsGallons => amount.get::<gallon>(),
        VolumeUnit::ImperialGallons => amount.get::<gallon_imperial>(),
        VolumeUnit::Liters => amount.get::<liter>(),
    };
    let places = currency_decimals(currency);
    let cost = float(cost_minor) / 10_f64.powi(i32::from(places));
    (units > 0.0).then(|| fixed(cost / units, usize::from(places) + 1))
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    #[test]
    fn readings_are_shown_with_thousands_grouped() {
        let miles = parse_reading("52310", MeterUnit::Miles, 0).unwrap();
        assert_eq!(show_reading(miles, MeterUnit::Miles, 0), "52,310");
        let hours = parse_reading("1234.5", MeterUnit::Hours, 1).unwrap();
        assert_eq!(show_reading(hours, MeterUnit::Hours, 1), "1,234.5");
        assert_eq!(show_reading(0, MeterUnit::Hours, 0), "0");
        let short = parse_reading("999", MeterUnit::Kilometers, 0).unwrap();
        assert_eq!(show_reading(short, MeterUnit::Kilometers, 0), "999");
    }

    #[test]
    fn a_unit_price_has_one_more_decimal_than_the_currency() {
        let gallons = parse_volume("18.004", VolumeUnit::UsGallons).unwrap();
        assert_eq!(
            unit_price(6_227, gallons, usd(), VolumeUnit::UsGallons).as_deref(),
            Some("3.459")
        );
        assert_eq!(unit_price(6_227, 0, usd(), VolumeUnit::Liters), None);
    }

    fn usd() -> CurrencyCode {
        CurrencyCode::USD
    }

    #[test]
    fn distances_convert_both_ways() {
        assert_eq!(parse_distance("1", DistanceUnit::Miles), Ok(1_609));
        assert_eq!(
            parse_distance("1,000", DistanceUnit::Kilometers),
            Ok(1_000_000)
        );
        assert_eq!(show_distance(1_609_344, DistanceUnit::Miles), "1000.0 mi");
        assert_eq!(
            show_distance(1_609_344, DistanceUnit::Kilometers),
            "1609.3 km"
        );
        assert_eq!(distance_value(20_117, DistanceUnit::Miles), "12.5");
        assert_eq!(
            parse_distance("-3", DistanceUnit::Miles),
            Err(QuantityError::Negative)
        );
        assert_eq!(
            parse_distance("ten", DistanceUnit::Miles),
            Err(QuantityError::NotANumber)
        );
        assert_eq!(
            parse_distance("", DistanceUnit::Miles),
            Err(QuantityError::NotANumber)
        );
        assert_eq!(
            parse_distance("1e20", DistanceUnit::Miles),
            Err(QuantityError::NotANumber)
        );
        assert_eq!(
            parse_distance("9".repeat(17).as_str(), DistanceUnit::Miles),
            Err(QuantityError::TooLarge)
        );
    }

    #[test]
    fn volumes_use_the_pumps_units() {
        assert_eq!(parse_volume("10", VolumeUnit::UsGallons), Ok(37_854));
        assert_eq!(parse_volume("10", VolumeUnit::ImperialGallons), Ok(45_461));
        assert_eq!(parse_volume("37.85", VolumeUnit::Liters), Ok(37_850));
        assert_eq!(show_volume(37_854, VolumeUnit::UsGallons), "10.000 gal");
        assert_eq!(
            show_volume(45_461, VolumeUnit::ImperialGallons),
            "10.000 gal (UK)"
        );
        assert_eq!(show_volume(37_854, VolumeUnit::Liters), "37.85 L");
    }

    #[test]
    fn fuel_economy_converts_including_the_inverse_unit() {
        // 25 mpg (US) is about 10.63 km/L, or 9.41 L/100 km.
        let stored = parse_economy("25", EconomyUnit::MpgUs).unwrap();
        assert_eq!(economy_value(stored, EconomyUnit::KmPerLiter), "10.6");
        assert_eq!(economy_value(stored, EconomyUnit::LitersPer100Km), "9.4");
        assert_eq!(economy_value(stored, EconomyUnit::MpgUs), "25.0");
        assert_eq!(show_economy(stored, EconomyUnit::MpgUk), "30.0 mpg (UK)");
        assert_eq!(parse_economy("10", EconomyUnit::LitersPer100Km), Ok(10_000));
        assert_eq!(
            parse_economy("0", EconomyUnit::LitersPer100Km),
            Err(QuantityError::TooLarge)
        );
        assert!(economy_is_inverse(EconomyUnit::LitersPer100Km));
        assert!(!economy_is_inverse(EconomyUnit::MpgUs));
    }

    #[test]
    fn durations_are_hours_and_minutes() {
        assert_eq!(parse_duration("1:30"), Ok(5_400));
        assert_eq!(parse_duration(" 0:45:30 "), Ok(2_730));
        assert_eq!(parse_duration("26:05"), Ok(93_900), "over a day is fine");
        for bad in ["90", "1:60", "1:30:60", "a:30", "1:-5", "", "1:30:15:00"] {
            assert_eq!(
                parse_duration(bad),
                Err(QuantityError::NotADuration),
                "{bad}"
            );
        }
        assert_eq!(show_duration(5_400), "1:30");
        assert_eq!(show_duration(2_730), "0:45:30");
    }

    #[test]
    fn readings_keep_to_the_meters_decimals() {
        assert_eq!(parse_reading("52310", MeterUnit::Miles, 0), Ok(84_184_785));
        assert_eq!(parse_reading("1234.5", MeterUnit::Hours, 1), Ok(4_444_200));
        assert_eq!(
            parse_reading("1234.56", MeterUnit::Hours, 1),
            Err(QuantityError::TooPrecise)
        );
        assert_eq!(
            parse_reading("52310.5", MeterUnit::Miles, 0),
            Err(QuantityError::TooPrecise)
        );
        assert_eq!(reading_value(84_184_785, MeterUnit::Miles, 0), "52310");
        assert_eq!(reading_value(4_444_200, MeterUnit::Hours, 1), "1234.5");
    }

    #[test]
    fn money_uses_the_currencys_decimals() {
        assert_eq!(parse_money("45.67", usd()), Ok(4_567));
        assert_eq!(parse_money("45.678", usd()), Err(QuantityError::TooPrecise));
        let yen = CurrencyCode::parse("JPY").unwrap();
        assert_eq!(parse_money("5000", yen), Ok(5_000));
        assert_eq!(parse_money("5000.5", yen), Err(QuantityError::TooPrecise));
        let dinar = CurrencyCode::parse("KWD").unwrap();
        assert_eq!(parse_money("1.250", dinar), Ok(1_250));
        assert_eq!(show_money(4_567, usd()), "45.67 USD");
        assert_eq!(show_money(5_000, yen), "5000 JPY");
    }

    proptest! {
        #[test]
        fn a_reading_shown_and_typed_again_is_the_same(
            raw in 0i64..10_000_000_000,
            unit in prop_oneof![Just(MeterUnit::Miles), Just(MeterUnit::Kilometers), Just(MeterUnit::Hours)],
            precision in 0u8..=2,
        ) {
            // A reading typed in the meter's unit survives being shown and typed again.
            let typed = reading_value(raw, unit, precision);
            let stored = parse_reading(&typed, unit, precision).unwrap();
            prop_assert_eq!(reading_value(stored, unit, precision), typed);
        }

        #[test]
        fn distances_survive_a_round_trip_to_within_a_meter(
            meters in 0i64..10_000_000_000,
            unit in prop_oneof![Just(DistanceUnit::Miles), Just(DistanceUnit::Kilometers)],
        ) {
            let shown = distance_value(meters, unit);
            let back = parse_distance(&shown, unit).unwrap();
            // One decimal of a mile is up to 80 m: the round trip is within half of that.
            prop_assert!((back - meters).abs() <= 81, "{meters} → {shown} → {back}");
        }

        #[test]
        fn money_survives_a_round_trip_exactly(cents in 0i64..1_000_000_000) {
            let text = show_money(cents, CurrencyCode::USD);
            let number = text.trim_end_matches(" USD");
            prop_assert_eq!(parse_money(number, CurrencyCode::USD), Ok(cents));
        }
    }
}
