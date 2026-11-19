//! Checking a formula against the values it may use: every key known, the dimensions
//! agreeing, meters where a meter is needed, and no past-events function inside another.

use super::FormulaError;
use super::syntax::{Expr, Function, Op, Over};
use super::units::Dimension;

/// What a key gives a formula.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct KeyInfo {
    /// What it measures.
    pub dimension: Dimension,
    /// It's a percentage.
    pub percent: bool,
    /// It's one of the vehicle's meters (for `distance-since-last` and Over).
    pub meter: bool,
}

/// What a checked formula gives.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Checked {
    /// What it measures.
    pub dimension: Dimension,
    /// It's a percentage.
    pub percent: bool,
}

impl Checked {
    const fn of(dimension: Dimension) -> Self {
        Self {
            dimension,
            percent: false,
        }
    }
}

/// Checks `expr` with `keys` (what each key gives, `None` if there's no such key);
/// `in_over` is set inside a past-events function.
pub(super) fn check(
    expr: &Expr,
    keys: &dyn Fn(&str) -> Option<KeyInfo>,
    in_over: bool,
) -> Result<Checked, FormulaError> {
    match expr {
        Expr::Number { unit, .. } => {
            Ok(unit.map_or(Checked::of(Dimension::NONE), |unit| Checked {
                dimension: unit.dimension(),
                percent: unit == super::units::Unit::Percent,
            }))
        }
        Expr::Key(key) => keys(key)
            .map(|info| Checked {
                dimension: info.dimension,
                percent: info.percent,
            })
            .ok_or_else(|| FormulaError::UnknownKey { key: key.clone() }),
        Expr::SinceLast(key) => {
            let info = meter(key, keys)?;
            Ok(Checked {
                dimension: info.dimension,
                percent: info.percent,
            })
        }
        Expr::TimeSinceLast => Ok(Checked::of(Dimension::TIME)),
        Expr::Negate(inner) | Expr::Abs(inner) | Expr::Round { expr: inner, .. } => {
            check(inner, keys, in_over)
        }
        Expr::Binary { op, left, right } => binary(*op, left, right, keys, in_over),
        Expr::Call { function, args } => {
            let checked = args
                .iter()
                .map(|arg| check(arg, keys, in_over))
                .collect::<Result<Vec<_>, _>>()?;
            combine(*function, &checked)
        }
        Expr::Over {
            function,
            expr,
            over,
        } => {
            if in_over {
                return Err(FormulaError::NestedOver);
            }
            check_over(over, keys)?;
            let inner = check(expr, keys, true)?;
            combine(*function, &[inner])
        }
    }
}

fn binary(
    op: Op,
    left: &Expr,
    right: &Expr,
    keys: &dyn Fn(&str) -> Option<KeyInfo>,
    in_over: bool,
) -> Result<Checked, FormulaError> {
    let (a, b) = (check(left, keys, in_over)?, check(right, keys, in_over)?);
    match op {
        Op::Plus | Op::Minus => {
            if a.dimension == b.dimension {
                Ok(Checked {
                    dimension: a.dimension,
                    percent: a.percent && b.percent,
                })
            } else {
                Err(FormulaError::Mismatch {
                    op,
                    left: left.to_string(),
                    right: right.to_string(),
                    left_dimension: a.dimension,
                    right_dimension: b.dimension,
                })
            }
        }
        Op::Times => Ok(Checked::of(a.dimension.times(b.dimension))),
        Op::Divide => Ok(Checked::of(a.dimension.per(b.dimension))),
    }
}

/// A statistics function's result from its values' (all of one dimension).
fn combine(function: Function, values: &[Checked]) -> Result<Checked, FormulaError> {
    if function == Function::Count {
        return Ok(Checked::of(Dimension::NONE));
    }
    let Some(first) = values.first() else {
        return Err(FormulaError::UnexpectedEnd);
    };
    if values
        .iter()
        .any(|value| value.dimension != first.dimension)
    {
        return Err(FormulaError::MixedValues {
            function: function.name().to_owned(),
        });
    }
    Ok(Checked {
        dimension: first.dimension,
        percent: values.iter().all(|value| value.percent),
    })
}

fn meter(key: &str, keys: &dyn Fn(&str) -> Option<KeyInfo>) -> Result<KeyInfo, FormulaError> {
    let info = keys(key).ok_or_else(|| FormulaError::UnknownKey {
        key: key.to_owned(),
    })?;
    if info.meter {
        Ok(info)
    } else {
        Err(FormulaError::NotAMeter {
            key: key.to_owned(),
        })
    }
}

fn check_over(over: &Over, keys: &dyn Fn(&str) -> Option<KeyInfo>) -> Result<(), FormulaError> {
    let (meter_key, wanted) = match over {
        Over::Distance {
            meter: Some(key), ..
        } => (key, Dimension::DISTANCE),
        Over::Hours { meter: None, .. } => return Err(FormulaError::HoursNeedMeter),
        Over::Hours {
            meter: Some(key), ..
        } => (key, Dimension::TIME),
        Over::Cumulative
        | Over::Events(_)
        | Over::Days(_)
        | Over::Weeks(_)
        | Over::Months(_)
        | Over::Distance { meter: None, .. } => return Ok(()),
    };
    let info = meter(meter_key, keys)?;
    if info.dimension == wanted {
        Ok(())
    } else {
        Err(FormulaError::WrongMeter {
            key: meter_key.clone(),
            wanted,
        })
    }
}
