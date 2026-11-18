//! Dimensions, the units a formula's numbers can carry, what a result shows as, and values in
//! a person's units.

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, Translator, messages};
use crate::preferences::{CurrencyCode, DistanceUnit, EconomyUnit, Preferences, VolumeUnit};

/// Meters in a mile.
const METERS_PER_MILE: f64 = 1_609.344;
/// Liters in a US gallon.
const LITERS_PER_US_GALLON: f64 = 3.785_411_784;
/// Liters in an imperial gallon.
const LITERS_PER_IMPERIAL_GALLON: f64 = 4.546_09;
/// Liters in a US liquid quart.
const LITERS_PER_QUART: f64 = 0.946_352_946;
/// Seconds in an hour.
const SECONDS_PER_HOUR: f64 = 3_600.0;
/// Seconds in a day.
const SECONDS_PER_DAY: f64 = 86_400.0;

/// What a value measures: powers of distance, volume, time, and money. All zero is a plain
/// number (or a percentage). Fuel economy is distance ÷ volume; cost per mile, money ÷
/// distance.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Dimension {
    /// The power of distance (meters).
    pub distance: i8,
    /// The power of volume (liters).
    pub volume: i8,
    /// The power of time (seconds).
    pub time: i8,
    /// The power of money (in a currency's whole units).
    pub money: i8,
}

impl Dimension {
    /// A plain number.
    pub const NONE: Self = Self::of(0, 0, 0, 0);
    /// A distance.
    pub const DISTANCE: Self = Self::of(1, 0, 0, 0);
    /// A volume.
    pub const VOLUME: Self = Self::of(0, 1, 0, 0);
    /// A time.
    pub const TIME: Self = Self::of(0, 0, 1, 0);
    /// Money.
    pub const MONEY: Self = Self::of(0, 0, 0, 1);
    /// Fuel economy: distance ÷ volume.
    pub const ECONOMY: Self = Self::of(1, -1, 0, 0);
    /// Money per distance.
    pub const MONEY_PER_DISTANCE: Self = Self::of(-1, 0, 0, 1);
    /// Money per volume.
    pub const MONEY_PER_VOLUME: Self = Self::of(0, -1, 0, 1);
    /// A speed: distance ÷ time.
    pub const SPEED: Self = Self::of(1, 0, -1, 0);

    const fn of(distance: i8, volume: i8, time: i8, money: i8) -> Self {
        Self {
            distance,
            volume,
            time,
            money,
        }
    }

    /// This times `other`.
    #[must_use]
    pub const fn times(self, other: Self) -> Self {
        Self::of(
            self.distance.saturating_add(other.distance),
            self.volume.saturating_add(other.volume),
            self.time.saturating_add(other.time),
            self.money.saturating_add(other.money),
        )
    }

    /// This divided by `other`.
    #[must_use]
    pub const fn per(self, other: Self) -> Self {
        Self::of(
            self.distance.saturating_sub(other.distance),
            self.volume.saturating_sub(other.volume),
            self.time.saturating_sub(other.time),
            self.money.saturating_sub(other.money),
        )
    }

    /// Its name for messages: "a distance", "fuel economy", "money per distance", or, for an
    /// unusual one, its parts ("distance² ÷ time").
    #[must_use]
    pub fn describe(self, translator: &Translator) -> String {
        if let Some(id) = self.name() {
            return translator.tr(id);
        }
        let parts = [
            (self.distance, messages::FORMULA_WORD_DISTANCE),
            (self.volume, messages::FORMULA_WORD_VOLUME),
            (self.time, messages::FORMULA_WORD_TIME),
            (self.money, messages::FORMULA_WORD_MONEY),
        ];
        let side = |positive: bool| -> String {
            parts
                .iter()
                .filter(|(power, _)| if positive { *power > 0 } else { *power < 0 })
                .map(|(power, word)| {
                    let word = translator.tr(*word);
                    match power.unsigned_abs() {
                        1 => word,
                        2 => format!("{word}²"),
                        3 => format!("{word}³"),
                        n => format!("{word}^{n}"),
                    }
                })
                .collect::<Vec<_>>()
                .join(" × ")
        };
        let (top, bottom) = (side(true), side(false));
        let unit = match (top.is_empty(), bottom.is_empty()) {
            (_, true) => top,
            (true, false) => format!("1 ÷ {bottom}"),
            (false, false) => format!("{top} ÷ {bottom}"),
        };
        translator.tr_with(messages::FORMULA_DIM_OTHER, &[("unit", unit.into())])
    }

    /// The message naming a common dimension.
    const fn name(self) -> Option<MessageId> {
        Some(match (self.distance, self.volume, self.time, self.money) {
            (0, 0, 0, 0) => messages::FORMULA_DIM_NUMBER,
            (1, 0, 0, 0) => messages::FORMULA_DIM_DISTANCE,
            (0, 1, 0, 0) => messages::FORMULA_DIM_VOLUME,
            (0, 0, 1, 0) => messages::FORMULA_DIM_TIME,
            (0, 0, 0, 1) => messages::FORMULA_DIM_MONEY,
            (1, -1, 0, 0) => messages::FORMULA_DIM_ECONOMY,
            (-1, 0, 0, 1) => messages::FORMULA_DIM_MONEY_PER_DISTANCE,
            (0, -1, 0, 1) => messages::FORMULA_DIM_MONEY_PER_VOLUME,
            (1, 0, -1, 0) => messages::FORMULA_DIM_SPEED,
            _ => return None,
        })
    }
}

/// A unit a number in a formula can carry (`33 mi/h` is miles over hours).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SimpleUnit {
    /// Miles (`mi`).
    Miles,
    /// Kilometers (`km`).
    Kilometers,
    /// Meters (`m`).
    Meters,
    /// Hours (`h`).
    Hours,
    /// Minutes (`min`).
    Minutes,
    /// Seconds (`s`).
    Seconds,
    /// Days (`days`).
    Days,
    /// Weeks (`weeks`).
    Weeks,
    /// US gallons (`gal`).
    UsGallons,
    /// Imperial gallons (`imp-gal`).
    ImperialGallons,
    /// Liters (`L`).
    Liters,
    /// Milliliters (`mL`).
    Milliliters,
    /// US quarts (`qt`).
    Quarts,
}

impl SimpleUnit {
    /// Every unit.
    pub const ALL: [Self; 13] = [
        Self::Miles,
        Self::Kilometers,
        Self::Meters,
        Self::Hours,
        Self::Minutes,
        Self::Seconds,
        Self::Days,
        Self::Weeks,
        Self::UsGallons,
        Self::ImperialGallons,
        Self::Liters,
        Self::Milliliters,
        Self::Quarts,
    ];

    /// The unit a word names (`mi`, `L`, `days`), if any. Case matters only for nothing: `l`
    /// and `L` are both liters.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        Some(match word.to_ascii_lowercase().as_str() {
            "mi" => Self::Miles,
            "km" => Self::Kilometers,
            "m" => Self::Meters,
            "h" | "hr" | "hrs" => Self::Hours,
            "min" => Self::Minutes,
            "s" | "sec" => Self::Seconds,
            "day" | "days" => Self::Days,
            "week" | "weeks" => Self::Weeks,
            "gal" => Self::UsGallons,
            "imp-gal" => Self::ImperialGallons,
            "l" => Self::Liters,
            "ml" => Self::Milliliters,
            "qt" => Self::Quarts,
            _ => return None,
        })
    }

    /// How it's written.
    #[must_use]
    pub const fn word(self) -> &'static str {
        match self {
            Self::Miles => "mi",
            Self::Kilometers => "km",
            Self::Meters => "m",
            Self::Hours => "h",
            Self::Minutes => "min",
            Self::Seconds => "s",
            Self::Days => "days",
            Self::Weeks => "weeks",
            Self::UsGallons => "gal",
            Self::ImperialGallons => "imp-gal",
            Self::Liters => "L",
            Self::Milliliters => "mL",
            Self::Quarts => "qt",
        }
    }

    /// What it measures.
    #[must_use]
    pub const fn dimension(self) -> Dimension {
        match self {
            Self::Miles | Self::Kilometers | Self::Meters => Dimension::DISTANCE,
            Self::Hours | Self::Minutes | Self::Seconds | Self::Days | Self::Weeks => {
                Dimension::TIME
            }
            Self::UsGallons
            | Self::ImperialGallons
            | Self::Liters
            | Self::Milliliters
            | Self::Quarts => Dimension::VOLUME,
        }
    }

    /// One of it in base units (meters, liters, seconds).
    #[must_use]
    pub const fn factor(self) -> f64 {
        match self {
            Self::Miles => METERS_PER_MILE,
            Self::Kilometers => 1_000.0,
            Self::Meters | Self::Liters | Self::Seconds => 1.0,
            Self::Hours => SECONDS_PER_HOUR,
            Self::Minutes => 60.0,
            Self::Days => SECONDS_PER_DAY,
            Self::Weeks => 7.0 * SECONDS_PER_DAY,
            Self::UsGallons => LITERS_PER_US_GALLON,
            Self::ImperialGallons => LITERS_PER_IMPERIAL_GALLON,
            Self::Milliliters => 0.001,
            Self::Quarts => LITERS_PER_QUART,
        }
    }
}

/// A number's unit: one unit, one over another (`mi/h`), or a percentage (`%`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Unit {
    /// One unit.
    Simple(SimpleUnit),
    /// One unit over another.
    Per(SimpleUnit, SimpleUnit),
    /// A percentage: `50%` is a half.
    Percent,
}

impl Unit {
    /// What it measures.
    #[must_use]
    pub const fn dimension(self) -> Dimension {
        match self {
            Self::Simple(unit) => unit.dimension(),
            Self::Per(top, bottom) => top.dimension().per(bottom.dimension()),
            Self::Percent => Dimension::NONE,
        }
    }

    /// One of it in base units.
    #[must_use]
    pub fn factor(self) -> f64 {
        match self {
            Self::Simple(unit) => unit.factor(),
            Self::Per(top, bottom) => top.factor() / bottom.factor(),
            Self::Percent => 0.01,
        }
    }
}

impl core::fmt::Display for Unit {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Simple(unit) => f.write_str(unit.word()),
            Self::Per(top, bottom) => write!(f, "{}/{}", top.word(), bottom.word()),
            Self::Percent => f.write_str("%"),
        }
    }
}

/// A worked-out value, in base units: meters, liters, seconds, and a currency's whole units
/// (a percentage as a fraction: 80% is 0.8).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Quantity {
    /// The number, in base units.
    pub value: f64,
    /// What it measures.
    pub dimension: Dimension,
    /// It's a percentage (from a percentage meter or field, or a `%` number), shown ×100.
    pub percent: bool,
    /// The currency of the money in it, if any.
    pub currency: Option<CurrencyCode>,
}

impl Quantity {
    /// A plain number.
    #[must_use]
    pub const fn number(value: f64) -> Self {
        Self::of(value, Dimension::NONE)
    }

    /// A value of `dimension`, in base units.
    #[must_use]
    pub const fn of(value: f64, dimension: Dimension) -> Self {
        Self {
            value,
            dimension,
            percent: false,
            currency: None,
        }
    }

    /// An amount of money in `currency`'s whole units.
    #[must_use]
    pub const fn money(value: f64, currency: CurrencyCode) -> Self {
        Self {
            value,
            dimension: Dimension::MONEY,
            percent: false,
            currency: Some(currency),
        }
    }

    /// A percentage, given as a fraction.
    #[must_use]
    pub const fn percentage(fraction: f64) -> Self {
        Self {
            value: fraction,
            dimension: Dimension::NONE,
            percent: true,
            currency: None,
        }
    }
}

/// What a calculated field shows its value as. Its dimension must be the formula's.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum ShowsAs {
    /// A number, with this many decimals.
    Number {
        /// Decimal places, 0–4.
        decimals: u8,
    },
    /// A percentage, with this many decimals.
    Percent {
        /// Decimal places, 0–4.
        decimals: u8,
    },
    /// A distance, in the person's unit.
    Distance,
    /// A duration, as `HH:MM:SS`.
    Duration,
    /// A volume, in the person's unit.
    Volume,
    /// Fuel economy, in the person's unit.
    FuelEconomy,
    /// Money.
    Money,
    /// Money per distance ("$0.14 per mi").
    MoneyPerDistance,
    /// Money per volume ("$3.45 per gal").
    MoneyPerVolume,
    /// A speed ("42 mph").
    Speed,
}

impl ShowsAs {
    /// The most decimals a number or percentage may show.
    pub const MAX_DECIMALS: u8 = 4;

    /// What a formula for it must measure.
    #[must_use]
    pub const fn dimension(self) -> Dimension {
        match self {
            Self::Number { .. } | Self::Percent { .. } => Dimension::NONE,
            Self::Distance => Dimension::DISTANCE,
            Self::Duration => Dimension::TIME,
            Self::Volume => Dimension::VOLUME,
            Self::FuelEconomy => Dimension::ECONOMY,
            Self::Money => Dimension::MONEY,
            Self::MoneyPerDistance => Dimension::MONEY_PER_DISTANCE,
            Self::MoneyPerVolume => Dimension::MONEY_PER_VOLUME,
            Self::Speed => Dimension::SPEED,
        }
    }
}

/// How a value of `dimension` is shown in `units`: shown = `scale` × value, or `scale` ÷
/// value when `inverse` (litres per 100 km).
#[derive(Debug, Clone, Copy, PartialEq)]
struct Shown {
    scale: f64,
    inverse: bool,
}

fn shown(dimension: Dimension, percent: bool, units: &Preferences) -> Shown {
    let mile = match units.distance {
        DistanceUnit::Miles => METERS_PER_MILE,
        DistanceUnit::Kilometers => 1_000.0,
    };
    let gallon = match units.volume {
        VolumeUnit::UsGallons => LITERS_PER_US_GALLON,
        VolumeUnit::ImperialGallons => LITERS_PER_IMPERIAL_GALLON,
        VolumeUnit::Liters => 1.0,
    };
    let scale = |scale: f64| Shown {
        scale,
        inverse: false,
    };
    match dimension {
        Dimension::NONE if percent => scale(100.0),
        Dimension::DISTANCE => scale(1.0 / mile),
        Dimension::VOLUME => scale(1.0 / gallon),
        Dimension::TIME => scale(1.0 / SECONDS_PER_HOUR),
        Dimension::MONEY_PER_DISTANCE => scale(mile),
        Dimension::MONEY_PER_VOLUME => scale(gallon),
        Dimension::SPEED => scale(SECONDS_PER_HOUR / mile),
        Dimension::ECONOMY => match units.economy {
            EconomyUnit::MpgUs => scale(LITERS_PER_US_GALLON / METERS_PER_MILE),
            EconomyUnit::MpgUk => scale(LITERS_PER_IMPERIAL_GALLON / METERS_PER_MILE),
            EconomyUnit::KmPerLiter => scale(0.001),
            // L/100 km = 100 ÷ (km/L) = 100,000 ÷ (m/L).
            EconomyUnit::LitersPer100Km => Shown {
                scale: 100_000.0,
                inverse: true,
            },
        },
        _ => scale(1.0),
    }
}

/// `quantity` in `units`, as it's shown: miles, gallons, mpg (or L/100 km), hours, money per
/// mile, a percentage ×100. `None` for a zero fuel economy shown as L/100 km.
#[must_use]
pub fn in_units(quantity: &Quantity, units: &Preferences) -> Option<f64> {
    let how = shown(quantity.dimension, quantity.percent, units);
    if how.inverse {
        (quantity.value.abs() >= f64::MIN_POSITIVE).then(|| how.scale / quantity.value)
    } else {
        Some(quantity.value * how.scale)
    }
}

/// The base value that shows as `shown` in `units` (the reverse of [`in_units`]).
pub(super) fn from_units(
    shown_value: f64,
    dimension: Dimension,
    percent: bool,
    units: &Preferences,
) -> Option<f64> {
    let how = shown(dimension, percent, units);
    if how.inverse {
        (shown_value.abs() >= f64::MIN_POSITIVE).then(|| how.scale / shown_value)
    } else {
        Some(shown_value / how.scale)
    }
}
