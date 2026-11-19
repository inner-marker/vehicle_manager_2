//! Formulas for calculated fields and derived meters (Phase 5 milestone 7; decisions #057,
//! #087).
//!
//! A small language of our own:
//! - **Values by stable key** (`cost`, `trip-time`, `odometer`), and **numbers with units**
//!   (`33 mi/h`, `1000 mi`, `5 gal`, `90 days`, `50%`). A key may contain `-`, so a minus
//!   between keys needs spaces (`a - b`, or `−`).
//! - **`+ − × ÷`** (also `- * /`) and parentheses.
//! - **Since the last event of its kind:** `distance-since-last(odometer)` (how far a meter
//!   moved) and `time-since-last`.
//! - **Functions:** `mean`, `median`, `geometric_mean`, `harmonic_mean`, `min`, `max`, `sum`,
//!   `count`, and `stddev`, over listed values (`mean(a, b, c)`) or over past events with an
//!   **Over** (`mean(trip-time, last 5)`, `sum(volume, last 3 months)`,
//!   `mean(cost ÷ volume, last 1000 mi)`, `max(trip-time, cumulative)`); `round(x, places)`
//!   (as shown, in the person's units) and `abs(x)`.
//!
//! **Units are checked by dimension** ([`Dimension`]): `+` and `−` need the same one, `×`
//! and `÷` combine them, and a calculated field's formula must give what it shows as
//! ([`ShowsAs`]). [`Formula::check`] refuses anything else, in words ([`FormulaError`]).
//! **Worked out when shown, never stored** ([`Formula::evaluate`]); when something it needs is
//! missing, it has no value, and says why ([`NoValue`]).
//!
//! # Examples
//!
//! ```
//! use domain::formula::{Context, Dimension, Formula, KeyInfo, Quantity, Scope, in_units};
//! use domain::location::TimeZone;
//! use domain::preferences::{CurrencyCode, Preferences};
//!
//! let formula = Formula::parse("round(cost ÷ distance-since-last(odometer), 2)").unwrap();
//! let keys = |key: &str| match key {
//!     "cost" => Some(KeyInfo { dimension: Dimension::MONEY, percent: false, meter: false }),
//!     "odometer" => Some(KeyInfo { dimension: Dimension::DISTANCE, percent: false, meter: true }),
//!     _ => None,
//! };
//! assert_eq!(formula.check(&keys).unwrap().dimension, Dimension::MONEY_PER_DISTANCE);
//!
//! let mut fuel_up = Scope::default();
//! fuel_up.values.insert("cost".into(), Quantity::money(58.20, CurrencyCode::USD));
//! fuel_up.since_last.insert("odometer".into(), Quantity::of(410.3 * 1609.344, Dimension::DISTANCE));
//! let units = Preferences::default();
//! let zone = TimeZone::parse("UTC").unwrap();
//! let context = Context { current: &fuel_up, earlier: &[], main_distance: Some("odometer"), zone: &zone, units: &units };
//! let per_mile = formula.evaluate(&context).unwrap();
//! assert!((in_units(&per_mile, &units).unwrap() - 0.14).abs() < 1e-9);
//! ```

mod check;
mod eval;
mod syntax;
mod units;

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

pub use check::{Checked, KeyInfo};
pub use eval::{Context, NoValue, Scope};
pub use syntax::{Expr, FORMULA_MAX_CHARS, Function, Op, Over, ROUND_MAX_PLACES};
pub use units::{Dimension, Quantity, ShowsAs, SimpleUnit, Unit, in_units};

use crate::i18n::{MessageArg, MessageId, Translator, messages};

/// The full guide to writing formulas (`docs/help/formulas.md`), until the app has help pages
/// of its own (idea 22).
pub const GUIDE_URL: &str =
    "https://github.com/inner-marker/vehicle_manager_2/blob/dev/docs/help/formulas.md";

/// A formula: its text, kept as typed, and what it reads as. Two formulas are the same when
/// their text is (the text decides what it reads as).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Formula {
    text: String,
    expr: Expr,
}

impl PartialEq for Formula {
    fn eq(&self, other: &Self) -> bool {
        self.text == other.text
    }
}

impl Eq for Formula {}

impl core::hash::Hash for Formula {
    fn hash<H: core::hash::Hasher>(&self, state: &mut H) {
        self.text.hash(state);
    }
}

impl Formula {
    /// Reads `text` (surrounding spaces dropped).
    ///
    /// # Errors
    ///
    /// [`FormulaError`] if it's empty, longer than [`FORMULA_MAX_CHARS`], or doesn't read as
    /// a formula (an unknown function or unit, a missing bracket, …).
    pub fn parse(text: &str) -> Result<Self, FormulaError> {
        let text = text.trim();
        if text.is_empty() {
            return Err(FormulaError::Empty);
        }
        if text.chars().count() > FORMULA_MAX_CHARS {
            return Err(FormulaError::TooLong);
        }
        Ok(Self {
            text: text.to_owned(),
            expr: syntax::parse(text)?,
        })
    }

    /// The text, as typed.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.text
    }

    /// What it reads as.
    #[must_use]
    pub const fn expr(&self) -> &Expr {
        &self.expr
    }

    /// Every key it uses (values, `distance-since-last`'s meters, and Over's meters).
    #[must_use]
    pub fn keys(&self) -> BTreeSet<String> {
        let mut keys = BTreeSet::new();
        self.expr.walk(&mut |expr| {
            let key = match expr {
                Expr::Key(key)
                | Expr::SinceLast(key)
                | Expr::Over {
                    over:
                        Over::Distance {
                            meter: Some(key), ..
                        }
                        | Over::Hours {
                            meter: Some(key), ..
                        },
                    ..
                } => Some(key),
                Expr::Number { .. }
                | Expr::TimeSinceLast
                | Expr::Negate(_)
                | Expr::Binary { .. }
                | Expr::Call { .. }
                | Expr::Over { .. }
                | Expr::Round { .. }
                | Expr::Abs(_) => None,
            };
            if let Some(key) = key {
                keys.insert(key.clone());
            }
        });
        keys
    }

    /// Whether it uses a past-events function or "since the last".
    #[must_use]
    pub fn uses_history(&self) -> bool {
        let mut uses = false;
        self.expr.walk(&mut |expr| {
            uses |= matches!(
                expr,
                Expr::Over { .. } | Expr::SinceLast(_) | Expr::TimeSinceLast
            );
        });
        uses
    }

    /// Checks it against the values it may use (`keys` gives what each key is, `None` if
    /// there's no such key), and says what it gives.
    ///
    /// # Errors
    ///
    /// [`FormulaError`] for an unknown key, dimensions that don't agree, a meter needed where
    /// it isn't one, or a past-events function inside another.
    pub fn check(&self, keys: &dyn Fn(&str) -> Option<KeyInfo>) -> Result<Checked, FormulaError> {
        check::check(&self.expr, keys, false)
    }

    /// [`Self::check`], and that it gives what `shows` needs.
    ///
    /// # Errors
    ///
    /// As [`Self::check`], or [`FormulaError::WrongResult`].
    pub fn check_shows(
        &self,
        keys: &dyn Fn(&str) -> Option<KeyInfo>,
        shows: ShowsAs,
    ) -> Result<Checked, FormulaError> {
        let checked = self.check(keys)?;
        if checked.dimension == shows.dimension() {
            Ok(checked)
        } else {
            Err(FormulaError::WrongResult {
                found: checked.dimension,
                wanted: shows.dimension(),
            })
        }
    }

    /// Works it out on `context`'s event.
    ///
    /// # Errors
    ///
    /// [`NoValue`], saying why, if something it needs is missing.
    pub fn evaluate(&self, context: &Context<'_>) -> Result<Quantity, NoValue> {
        eval::evaluate(&self.expr, context.current, context)
    }
}

impl TryFrom<String> for Formula {
    type Error = FormulaError;

    fn try_from(text: String) -> Result<Self, Self::Error> {
        Self::parse(&text)
    }
}

impl From<Formula> for String {
    fn from(formula: Formula) -> Self {
        formula.text
    }
}

impl core::fmt::Display for FormulaError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.write_str(&self.describe(Translator::english()))
    }
}

/// Why a formula was refused.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum FormulaError {
    /// Nothing typed.
    Empty,
    /// Longer than [`FORMULA_MAX_CHARS`].
    TooLong,
    /// It stops where more was needed ("1 +").
    UnexpectedEnd,
    /// Something out of place.
    Unexpected {
        /// Where (a character position).
        at: usize,
        /// What.
        found: String,
    },
    /// A `(` never closed.
    Unclosed {
        /// Where it opened.
        at: usize,
    },
    /// `name(…)`, but there's no such function.
    UnknownFunction {
        /// Where.
        at: usize,
        /// The name.
        name: String,
    },
    /// `round`'s places aren't a whole number from 0 to [`ROUND_MAX_PLACES`].
    BadPlaces,
    /// No value has this key.
    UnknownKey {
        /// The key.
        key: String,
    },
    /// A meter is needed here, and this isn't one.
    NotAMeter {
        /// The key.
        key: String,
    },
    /// An Over's meter measures the wrong thing (`last 50 h on odometer`).
    WrongMeter {
        /// The key.
        key: String,
        /// What it needed to measure.
        wanted: Dimension,
    },
    /// `last … h` without `on` an hour meter.
    HoursNeedMeter,
    /// `+` or `−` of two different dimensions.
    Mismatch {
        /// `+` or `−`.
        op: Op,
        /// The left side, as written.
        left: String,
        /// The right side, as written.
        right: String,
        /// What the left side measures.
        left_dimension: Dimension,
        /// What the right side measures.
        right_dimension: Dimension,
    },
    /// A function's values measure different things.
    MixedValues {
        /// The function's name.
        function: String,
    },
    /// A past-events function inside another one's expression.
    NestedOver,
    /// A value a derived meter can't use: anything but a typed meter that isn't reset.
    NotAnInput {
        /// The key.
        key: String,
    },
    /// A calculated field used in a formula (it's worked out itself).
    Calculated {
        /// The key.
        key: String,
    },
    /// In a derived meter: `round`, past events, or "since the last".
    NotInDerived,
    /// It doesn't give what the field shows.
    WrongResult {
        /// What it gives.
        found: Dimension,
        /// What the field shows.
        wanted: Dimension,
    },
}

impl FormulaError {
    /// The message (see [`Self::describe`] for its arguments).
    #[must_use]
    pub const fn message(&self) -> MessageId {
        match self {
            Self::Empty => messages::FORMULA_EMPTY,
            Self::TooLong => messages::FORMULA_TOO_LONG,
            Self::UnexpectedEnd => messages::FORMULA_UNEXPECTED_END,
            Self::Unexpected { .. } => messages::FORMULA_UNEXPECTED,
            Self::Unclosed { .. } => messages::FORMULA_UNCLOSED,
            Self::UnknownFunction { .. } => messages::FORMULA_UNKNOWN_FUNCTION,
            Self::BadPlaces => messages::FORMULA_BAD_PLACES,
            Self::UnknownKey { .. } => messages::FORMULA_UNKNOWN_KEY,
            Self::NotAMeter { .. } => messages::FORMULA_NOT_A_METER,
            Self::WrongMeter { .. } => messages::FORMULA_WRONG_METER,
            Self::HoursNeedMeter => messages::FORMULA_HOURS_NEED_METER,
            Self::Mismatch { op: Op::Minus, .. } => messages::FORMULA_MISMATCH_MINUS,
            Self::Mismatch { .. } => messages::FORMULA_MISMATCH_PLUS,
            Self::MixedValues { .. } => messages::FORMULA_MIXED_VALUES,
            Self::NestedOver => messages::FORMULA_NESTED_OVER,
            Self::NotAnInput { .. } => messages::FORMULA_NOT_AN_INPUT,
            Self::Calculated { .. } => messages::FORMULA_CALCULATED,
            Self::NotInDerived => messages::FORMULA_NOT_IN_DERIVED,
            Self::WrongResult { .. } => messages::FORMULA_WRONG_RESULT,
        }
    }

    /// The reason, in words: "trip-time + volume: can't add a time to a volume."
    #[must_use]
    pub fn describe(&self, translator: &Translator) -> String {
        let text = |value: &str| MessageArg::from(value);
        let position = |at: &usize| MessageArg::from(i64::try_from(*at + 1).unwrap_or(i64::MAX));
        let args: Vec<(&str, MessageArg)> = match self {
            Self::Unexpected { at, found } => {
                vec![("found", text(found)), ("position", position(at))]
            }
            Self::Unclosed { at } => vec![("position", position(at))],
            Self::UnknownFunction { name, .. } => vec![("name", text(name))],
            Self::BadPlaces => vec![("max", MessageArg::from(u32::from(ROUND_MAX_PLACES)))],
            Self::UnknownKey { key }
            | Self::NotAMeter { key }
            | Self::NotAnInput { key }
            | Self::Calculated { key } => vec![("key", text(key))],
            Self::WrongMeter { key, wanted } => vec![
                ("key", text(key)),
                ("wanted", wanted.describe(translator).into()),
            ],
            Self::Mismatch {
                op,
                left,
                right,
                left_dimension,
                right_dimension,
            } => vec![
                (
                    "expression",
                    format!("{left} {} {right}", op_symbol(*op)).into(),
                ),
                ("left", left_dimension.describe(translator).into()),
                ("right", right_dimension.describe(translator).into()),
            ],
            Self::MixedValues { function } => vec![("function", text(function))],
            Self::WrongResult { found, wanted } => vec![
                ("found", found.describe(translator).into()),
                ("wanted", wanted.describe(translator).into()),
            ],
            Self::Empty
            | Self::TooLong
            | Self::UnexpectedEnd
            | Self::HoursNeedMeter
            | Self::NestedOver
            | Self::NotInDerived => vec![(
                "max",
                MessageArg::from(i64::try_from(FORMULA_MAX_CHARS).unwrap_or(i64::MAX)),
            )],
        };
        translator.tr_with(self.message(), &args)
    }
}

const fn op_symbol(op: Op) -> &'static str {
    match op {
        Op::Plus => "+",
        Op::Minus => "−",
        Op::Times => "×",
        Op::Divide => "÷",
    }
}

#[cfg(test)]
mod tests;
