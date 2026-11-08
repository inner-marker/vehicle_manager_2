//! The log forms' typed values (milestone 6, step 6g): a custom field's value from what was
//! typed, and back again for editing.
//!
//! Each field kind is typed as the person sees it: numbers and dates as text, quantities in the
//! person's units (#007), a yes or no and a choice from a list. An empty entry means "not
//! given" (a required field is then skipped with a reason, or refused by the checks).
//!
//! # Examples
//!
//! ```
//! use domain::custom_field::FieldKind;
//! use domain::event::FieldData;
//! use domain::event_form::{field_text, parse_field};
//! use domain::preferences::Preferences;
//!
//! let units = Preferences::default();
//! let trip = parse_field(&FieldKind::Duration, "3:40", &units).unwrap();
//! assert_eq!(trip, Some(FieldData::Duration(13_200)));
//! assert_eq!(field_text(&FieldKind::Duration, &trip.unwrap(), &units), "3:40");
//! assert_eq!(parse_field(&FieldKind::Duration, " ", &units), Ok(None));
//! ```

use crate::custom_field::FieldKind;
use crate::event::FieldData;
use crate::i18n::{MessageId, messages};
use crate::label::Label;
use crate::preferences::Preferences;
use crate::quantity::{
    distance_value, economy_value, parse_distance, parse_duration, parse_economy, parse_volume,
    show_duration, volume_value,
};

/// What a yes-or-no field's "yes" is typed as (the form's choice value).
pub const YES: &str = "yes";
/// What a yes-or-no field's "no" is typed as.
pub const NO: &str = "no";

/// A number typed with at most `decimals` places, as the number times 10^decimals (it may be
/// negative, e.g. a temperature).
fn parse_scaled(text: &str, decimals: u8) -> Result<i64, MessageId> {
    let cleaned: String = text.chars().filter(|c| *c != ',').collect();
    let (negative, digits) = cleaned
        .strip_prefix('-')
        .map_or((false, cleaned.as_str()), |rest| (true, rest));
    let (whole, fraction) = digits.split_once('.').unwrap_or((digits, ""));
    let places = usize::from(decimals);
    let all_digits = |part: &str| part.chars().all(|c| c.is_ascii_digit());
    if (whole.is_empty() && fraction.is_empty()) || !all_digits(whole) || !all_digits(fraction) {
        return Err(messages::QUANTITY_NOT_A_NUMBER);
    }
    if fraction.len() > places {
        return Err(messages::QUANTITY_TOO_PRECISE);
    }
    let scaled = format!("{whole}{fraction:0<places$}");
    let value: i64 = scaled.parse().map_err(|_| messages::QUANTITY_TOO_LARGE)?;
    Ok(if negative { -value } else { value })
}

/// A scaled number shown with `decimals` places (125 with one decimal: "12.5").
fn show_scaled(value: i64, decimals: u8) -> String {
    if decimals == 0 {
        return value.to_string();
    }
    let places = usize::from(decimals);
    let digits = format!("{:0>width$}", value.unsigned_abs(), width = places + 1);
    let (whole, fraction) = digits.split_at(digits.len() - places);
    let sign = if value < 0 { "-" } else { "" };
    format!("{sign}{whole}.{fraction}")
}

/// A custom field's value from what was typed, in the person's `units`; `None` when nothing
/// was.
///
/// # Errors
///
/// The message for what's wrong with it (not a number, not a date, not one of the choices).
pub fn parse_field(
    kind: &FieldKind,
    text: &str,
    units: &Preferences,
) -> Result<Option<FieldData>, MessageId> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let quantity = |result: Result<i64, crate::quantity::QuantityError>| {
        result.map_err(crate::quantity::QuantityError::message)
    };
    let data = match kind {
        FieldKind::Text => FieldData::Text(text.to_owned()),
        FieldKind::Number { decimals } => FieldData::Number(parse_scaled(text, *decimals)?),
        FieldKind::Date => {
            let date: jiff::civil::Date = text.parse().map_err(|_| messages::DATE_BAD_DATE)?;
            FieldData::Date(date.to_string())
        }
        FieldKind::YesNo => match text {
            YES => FieldData::YesNo(true),
            NO => FieldData::YesNo(false),
            _ => return Err(messages::EVENT_WRONG_VALUE),
        },
        FieldKind::Choice { options } => {
            let label = Label::parse(text).map_err(|_| messages::EVENT_WRONG_VALUE)?;
            if !options.contains(&label) {
                return Err(messages::EVENT_WRONG_VALUE);
            }
            FieldData::Choice(label)
        }
        FieldKind::Distance => FieldData::Distance(quantity(parse_distance(text, units.distance))?),
        FieldKind::Duration => FieldData::Duration(quantity(parse_duration(text))?),
        FieldKind::Volume => FieldData::Volume(quantity(parse_volume(text, units.volume))?),
        FieldKind::FuelEconomy => {
            FieldData::FuelEconomy(quantity(parse_economy(text, units.economy))?)
        }
    };
    Ok(Some(data))
}

/// A field's stored value as it's typed, for editing (the reverse of [`parse_field`]).
#[must_use]
pub fn field_text(kind: &FieldKind, data: &FieldData, units: &Preferences) -> String {
    match (kind, data) {
        (FieldKind::Number { decimals }, FieldData::Number(value)) => {
            show_scaled(*value, *decimals)
        }
        (_, FieldData::Text(text) | FieldData::Date(text)) => text.clone(),
        (_, FieldData::Number(value)) => value.to_string(),
        (_, FieldData::YesNo(yes)) => (if *yes { YES } else { NO }).to_owned(),
        (_, FieldData::Choice(label)) => label.as_str().to_owned(),
        (_, FieldData::Distance(meters)) => distance_value(*meters, units.distance),
        (_, FieldData::Duration(seconds)) => show_duration(*seconds),
        (_, FieldData::Volume(milliliters)) => volume_value(*milliliters, units.volume),
        (_, FieldData::FuelEconomy(value)) => economy_value(*value, units.economy),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::preferences::{DistanceUnit, VolumeUnit};

    fn units() -> Preferences {
        Preferences {
            distance: DistanceUnit::Miles,
            volume: VolumeUnit::UsGallons,
            ..Preferences::default()
        }
    }

    #[test]
    fn every_kind_parses_and_shows_again() {
        let choices = FieldKind::Choice {
            options: vec![
                Label::parse("Shell").unwrap(),
                Label::parse("Costco").unwrap(),
            ],
        };
        let cases = [
            (FieldKind::Text, "Pump 4", FieldData::Text("Pump 4".into())),
            (
                FieldKind::Number { decimals: 1 },
                "-3.5",
                FieldData::Number(-35),
            ),
            (
                FieldKind::Date,
                "2026-11-02",
                FieldData::Date("2026-11-02".into()),
            ),
            (FieldKind::YesNo, "yes", FieldData::YesNo(true)),
            (
                choices,
                "Costco",
                FieldData::Choice(Label::parse("Costco").unwrap()),
            ),
            (FieldKind::Distance, "12.5", FieldData::Distance(20_117)),
            (FieldKind::Duration, "1:30", FieldData::Duration(5_400)),
            (FieldKind::Volume, "10.000", FieldData::Volume(37_854)),
        ];
        for (kind, typed, data) in cases {
            let parsed = parse_field(&kind, typed, &units()).unwrap().unwrap();
            assert_eq!(parsed, data, "{kind:?}");
            assert_eq!(field_text(&kind, &parsed, &units()), typed, "{kind:?}");
        }
    }

    #[test]
    fn wrong_values_say_why() {
        let units = units();
        assert_eq!(
            parse_field(&FieldKind::Number { decimals: 0 }, "1.5", &units),
            Err(messages::QUANTITY_TOO_PRECISE)
        );
        assert_eq!(
            parse_field(&FieldKind::Number { decimals: 2 }, "abc", &units),
            Err(messages::QUANTITY_NOT_A_NUMBER)
        );
        assert_eq!(
            parse_field(&FieldKind::Date, "2026-02-30", &units),
            Err(messages::DATE_BAD_DATE)
        );
        assert_eq!(
            parse_field(&FieldKind::Distance, "-4", &units),
            Err(messages::QUANTITY_NEGATIVE)
        );
        let choices = FieldKind::Choice {
            options: vec![Label::parse("Shell").unwrap()],
        };
        assert_eq!(
            parse_field(&choices, "BP", &units),
            Err(messages::EVENT_WRONG_VALUE)
        );
        assert_eq!(parse_field(&FieldKind::Text, "   ", &units), Ok(None));
    }
}
