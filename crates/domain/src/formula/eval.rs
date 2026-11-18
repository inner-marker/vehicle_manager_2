//! Working a checked formula out on an event: its values, since the last event, and the
//! past events an Over picks.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use super::syntax::{Expr, Function, Op, Over};
use super::units::{Dimension, Quantity, Unit, from_units, in_units};
use crate::dates::utc_to_local;
use crate::i18n::{MessageId, messages};
use crate::location::TimeZone;
use crate::preferences::Preferences;
use crate::time::UnixMillis;

/// Milliseconds in a day.
const DAY_MILLIS: i64 = 86_400_000;

/// One event's values, as formulas see them.
#[derive(Debug, Clone, PartialEq)]
pub struct Scope {
    /// When it happened.
    pub at: UnixMillis,
    /// Its values by key: its fields, its readings, and what it adds (a fuel-up's `volume`
    /// and `cost`). A value left out has no value here.
    pub values: BTreeMap<String, Quantity>,
    /// How far each meter moved since the last event of this kind, by the meter's key.
    pub since_last: BTreeMap<String, Quantity>,
    /// How long since the last event of this kind; `None` for the first.
    pub time_since_last: Option<Quantity>,
}

impl Default for Scope {
    /// No values, at the epoch.
    fn default() -> Self {
        Self {
            at: UnixMillis::from_millis(0),
            values: BTreeMap::new(),
            since_last: BTreeMap::new(),
            time_since_last: None,
        }
    }
}

/// Where a formula is worked out.
#[derive(Debug, Clone, Copy)]
pub struct Context<'a> {
    /// The event itself.
    pub current: &'a Scope,
    /// The vehicle's earlier events of the same kind, oldest first (for an Over).
    pub earlier: &'a [Scope],
    /// The key of the vehicle's main odometer (for `last 1000 mi`), if it has one.
    pub main_distance: Option<&'a str>,
    /// The household's time zone (for `last 3 months`).
    pub zone: &'a TimeZone,
    /// The person's units (`round` rounds as shown).
    pub units: &'a Preferences,
}

/// Why a formula has no value on an event (shown as "—", with this).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum NoValue {
    /// A value it uses was left out.
    Missing {
        /// Its key.
        key: String,
    },
    /// It uses "since the last", and there's no earlier event of this kind.
    NoEarlier,
    /// None of the past events it works over has a value.
    NothingInOver,
    /// A division by zero.
    DivideByZero,
    /// Money in different currencies.
    MixedCurrencies,
    /// `geometric_mean` or `harmonic_mean` of a value that isn't above zero.
    NotPositive,
    /// `stddev` of fewer than two values.
    TooFew,
    /// `last … mi` on a vehicle without a main odometer.
    NoOdometer,
    /// The result is too big to show.
    NotFinite,
    /// Its values no longer fit it (checked when it was saved, so it shouldn't happen).
    Mismatch,
}

impl NoValue {
    /// The message saying why (`Missing` takes `key`).
    #[must_use]
    pub const fn message(&self) -> MessageId {
        match self {
            Self::Missing { .. } => messages::FORMULA_NO_VALUE_MISSING,
            Self::NoEarlier => messages::FORMULA_NO_VALUE_NO_EARLIER,
            Self::NothingInOver => messages::FORMULA_NO_VALUE_NOTHING_IN_OVER,
            Self::DivideByZero => messages::FORMULA_NO_VALUE_DIVIDE_BY_ZERO,
            Self::MixedCurrencies => messages::FORMULA_NO_VALUE_MIXED_CURRENCIES,
            Self::NotPositive => messages::FORMULA_NO_VALUE_NOT_POSITIVE,
            Self::TooFew => messages::FORMULA_NO_VALUE_TOO_FEW,
            Self::NoOdometer => messages::FORMULA_NO_VALUE_NO_ODOMETER,
            Self::NotFinite => messages::FORMULA_NO_VALUE_NOT_FINITE,
            Self::Mismatch => messages::FORMULA_NO_VALUE_MISMATCH,
        }
    }
}

/// `expr` worked out on `scope`, within `context`.
pub(super) fn evaluate(
    expr: &Expr,
    scope: &Scope,
    context: &Context<'_>,
) -> Result<Quantity, NoValue> {
    let value = match expr {
        Expr::Number { value, unit } => number(*value, *unit),
        Expr::Key(key) => scope
            .values
            .get(key)
            .copied()
            .ok_or_else(|| NoValue::Missing { key: key.clone() })?,
        Expr::SinceLast(key) => scope
            .since_last
            .get(key)
            .copied()
            .ok_or(NoValue::NoEarlier)?,
        Expr::TimeSinceLast => scope.time_since_last.ok_or(NoValue::NoEarlier)?,
        Expr::Negate(inner) => {
            let inner = evaluate(inner, scope, context)?;
            Quantity {
                value: -inner.value,
                ..inner
            }
        }
        Expr::Abs(inner) => {
            let inner = evaluate(inner, scope, context)?;
            Quantity {
                value: inner.value.abs(),
                ..inner
            }
        }
        Expr::Round { expr, places } => round(evaluate(expr, scope, context)?, *places, context)?,
        Expr::Binary { op, left, right } => binary(
            *op,
            evaluate(left, scope, context)?,
            evaluate(right, scope, context)?,
        )?,
        Expr::Call { function, args } => {
            let values = args
                .iter()
                .map(|arg| evaluate(arg, scope, context))
                .collect::<Result<Vec<_>, _>>()?;
            apply(*function, &values)?
        }
        Expr::Over {
            function,
            expr,
            over,
        } => {
            let values: Vec<Quantity> = picked(over, context)?
                .into_iter()
                .filter_map(|each| evaluate(expr, each, context).ok())
                .collect();
            if values.is_empty() {
                return Err(NoValue::NothingInOver);
            }
            apply(*function, &values)?
        }
    };
    if value.value.is_finite() {
        Ok(value)
    } else {
        Err(NoValue::NotFinite)
    }
}

fn number(value: f64, unit: Option<Unit>) -> Quantity {
    match unit {
        None => Quantity::number(value),
        Some(Unit::Percent) => Quantity::percentage(value * Unit::Percent.factor()),
        Some(unit) => Quantity::of(value * unit.factor(), unit.dimension()),
    }
}

/// The currency of a result that combines `a` and `b`.
fn currency(
    a: &Quantity,
    b: &Quantity,
    dimension: Dimension,
) -> Result<Option<crate::preferences::CurrencyCode>, NoValue> {
    match (a.currency, b.currency) {
        (Some(x), Some(y)) if x != y => Err(NoValue::MixedCurrencies),
        (x, y) => Ok(if dimension.money == 0 { None } else { x.or(y) }),
    }
}

fn binary(op: Op, a: Quantity, b: Quantity) -> Result<Quantity, NoValue> {
    let (value, dimension, percent) = match op {
        Op::Plus | Op::Minus => {
            if a.dimension != b.dimension {
                return Err(NoValue::Mismatch);
            }
            let value = if op == Op::Plus {
                a.value + b.value
            } else {
                a.value - b.value
            };
            (value, a.dimension, a.percent && b.percent)
        }
        Op::Times => (a.value * b.value, a.dimension.times(b.dimension), false),
        Op::Divide => {
            if b.value.abs() < f64::MIN_POSITIVE {
                return Err(NoValue::DivideByZero);
            }
            (a.value / b.value, a.dimension.per(b.dimension), false)
        }
    };
    Ok(Quantity {
        value,
        dimension,
        percent,
        currency: currency(&a, &b, dimension)?,
    })
}

/// `value` rounded to `places` decimals as it's shown in the person's units.
fn round(value: Quantity, places: u8, context: &Context<'_>) -> Result<Quantity, NoValue> {
    let shown = in_units(&value, context.units).ok_or(NoValue::DivideByZero)?;
    let scale = 10_f64.powi(i32::from(places));
    let rounded = (shown * scale).round() / scale;
    let back = from_units(rounded, value.dimension, value.percent, context.units)
        .ok_or(NoValue::DivideByZero)?;
    Ok(Quantity {
        value: back,
        ..value
    })
}

/// How many values, as a number.
fn count(values: &[Quantity]) -> f64 {
    f64::from(u32::try_from(values.len()).unwrap_or(u32::MAX))
}

/// A statistics function over `values` (at least one, all of one dimension).
fn apply(function: Function, values: &[Quantity]) -> Result<Quantity, NoValue> {
    let Some(first) = values.first() else {
        return Err(NoValue::NothingInOver);
    };
    if function == Function::Count {
        return Ok(Quantity::number(count(values)));
    }
    if values
        .iter()
        .any(|value| value.dimension != first.dimension)
    {
        return Err(NoValue::Mismatch);
    }
    if values
        .iter()
        .any(|value| value.currency.is_some() && value.currency != first.currency)
    {
        return Err(NoValue::MixedCurrencies);
    }
    let numbers: Vec<f64> = values.iter().map(|value| value.value).collect();
    let n = count(values);
    let mean = numbers.iter().sum::<f64>() / n;
    let positive = || {
        if numbers.iter().all(|x| *x > 0.0) {
            Ok(())
        } else {
            Err(NoValue::NotPositive)
        }
    };
    let value = match function {
        Function::Mean => mean,
        Function::Sum => numbers.iter().sum(),
        Function::Min => numbers.iter().copied().fold(f64::INFINITY, f64::min),
        Function::Max => numbers.iter().copied().fold(f64::NEG_INFINITY, f64::max),
        Function::Median => median(&numbers),
        Function::GeometricMean => {
            positive()?;
            (numbers.iter().map(|x| x.ln()).sum::<f64>() / n).exp()
        }
        Function::HarmonicMean => {
            positive()?;
            n / numbers.iter().map(|x| 1.0 / x).sum::<f64>()
        }
        Function::Stddev => {
            if numbers.len() < 2 {
                return Err(NoValue::TooFew);
            }
            let squares: f64 = numbers.iter().map(|x| (x - mean).powi(2)).sum();
            (squares / (n - 1.0)).sqrt()
        }
        Function::Count => n,
    };
    Ok(Quantity {
        value,
        dimension: first.dimension,
        percent: values.iter().all(|value| value.percent),
        currency: first.currency,
    })
}

fn median(numbers: &[f64]) -> f64 {
    let mut sorted = numbers.to_vec();
    sorted.sort_by(f64::total_cmp);
    let middle = sorted.len() / 2;
    match (sorted.len() % 2, sorted.get(middle)) {
        (1, Some(x)) => *x,
        (_, Some(x)) => sorted.get(middle - 1).map_or(*x, |y| f64::midpoint(*x, *y)),
        (_, None) => f64::NAN,
    }
}

/// The events an Over picks: the earlier ones and the event itself.
fn picked<'a>(over: &Over, context: &Context<'a>) -> Result<Vec<&'a Scope>, NoValue> {
    let all: Vec<&Scope> = context
        .earlier
        .iter()
        .chain(core::iter::once(context.current))
        .collect();
    let now = context.current.at.as_millis();
    let since = |millis: i64| -> Vec<&'a Scope> {
        let from = now.saturating_sub(millis);
        all.iter()
            .copied()
            .filter(|scope| scope.at.as_millis() >= from)
            .collect()
    };
    Ok(match over {
        Over::Cumulative => all,
        Over::Events(n) => {
            let keep = usize::try_from(*n).unwrap_or(usize::MAX);
            all[all.len().saturating_sub(keep)..].to_vec()
        }
        Over::Days(n) => since(i64::from(*n).saturating_mul(DAY_MILLIS)),
        Over::Weeks(n) => since(i64::from(*n).saturating_mul(7 * DAY_MILLIS)),
        Over::Months(n) => {
            let date = |at: UnixMillis| {
                utc_to_local(at, context.zone)
                    .ok()
                    .and_then(|(date, _)| date.parse::<jiff::civil::Date>().ok())
            };
            let Some(cutoff) = date(context.current.at).and_then(|today| {
                today
                    .checked_sub(jiff::Span::new().months(i64::from(*n)))
                    .ok()
            }) else {
                return Ok(Vec::new());
            };
            all.into_iter()
                .filter(|scope| date(scope.at).is_some_and(|day| day >= cutoff))
                .collect()
        }
        Over::Distance {
            amount,
            unit,
            meter,
        } => {
            let key = meter
                .as_deref()
                .or(context.main_distance)
                .ok_or(NoValue::NoOdometer)?;
            within(&all, context.current, key, amount * unit.factor())?
        }
        Over::Hours { amount, meter } => {
            let key = meter.as_deref().ok_or(NoValue::NoOdometer)?;
            within(&all, context.current, key, amount * 3_600.0)?
        }
    })
}

/// The events whose reading of `key` is within `back` (base units) of the current one's.
fn within<'a>(
    all: &[&'a Scope],
    current: &Scope,
    key: &str,
    back: f64,
) -> Result<Vec<&'a Scope>, NoValue> {
    let now = current.values.get(key).ok_or_else(|| NoValue::Missing {
        key: key.to_owned(),
    })?;
    let from = now.value - back;
    Ok(all
        .iter()
        .copied()
        .filter(|scope| {
            scope
                .values
                .get(key)
                .is_some_and(|reading| reading.value >= from)
        })
        .collect())
}
