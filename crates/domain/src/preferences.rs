//! Display preferences: units, formats, currency, and language (decisions #029, #048).
//!
//! Display only: stored values are always metric integers and currency codes. Each value
//! here is chosen by the household (an Admin) and can be overridden by each user:
//! **the user's choice → the household's → the built-in default** ([`Preferences::resolve`]).
//!
//! One record holds the household's choices and one each user's, with one field per setting,
//! so two devices changing different settings never overwrite each other (#043).
//!
//! # Examples
//!
//! ```
//! use domain::preferences::{DistanceUnit, PreferenceChoices, Preferences};
//!
//! let household = PreferenceChoices::default();
//! let mine = PreferenceChoices { distance: Some(DistanceUnit::Kilometers), ..PreferenceChoices::default() };
//!
//! let effective = Preferences::resolve(&mine, &household);
//! assert_eq!(effective.distance, DistanceUnit::Kilometers);
//! assert_eq!(effective, Preferences { distance: DistanceUnit::Kilometers, ..Preferences::default() });
//! ```

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::command::{Patch, Record};
use crate::id::Id;

/// How distances are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DistanceUnit {
    /// Miles (the default).
    #[default]
    Miles,
    /// Kilometers.
    Kilometers,
}

/// How fuel volumes are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum VolumeUnit {
    /// US gallons (the default).
    #[default]
    UsGallons,
    /// Imperial gallons.
    ImperialGallons,
    /// Liters.
    Liters,
}

/// How fuel economy is shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum EconomyUnit {
    /// Miles per US gallon (the default).
    #[default]
    MpgUs,
    /// Miles per imperial gallon.
    MpgUk,
    /// Liters per 100 kilometers.
    #[serde(rename = "liters-per-100-km")]
    LitersPer100Km,
    /// Kilometers per liter.
    KmPerLiter,
}

/// How pressures (e.g. tires) are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum PressureUnit {
    /// Pounds per square inch (the default).
    #[default]
    Psi,
    /// Kilopascals.
    Kpa,
    /// Bar.
    Bar,
}

/// How dates are shown.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DateFormat {
    /// `30 Oct 2026` (the default).
    #[default]
    DayMonthNameYear,
    /// `10/30/2026`.
    MonthDayYear,
    /// `30/10/2026`.
    DayMonthYear,
    /// `2026-10-30`.
    YearMonthDay,
}

/// The app's language. English (US) only so far (decision #022).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Language {
    /// English (United States).
    #[default]
    #[serde(rename = "en-US")]
    EnglishUs,
}

/// How long the app may sit idle before it signs out by itself (decision #049).
///
/// Only real input counts as activity (see [`crate::idle`]). The server's own limit, 30 idle
/// days, still applies whatever this says.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum IdleSignOut {
    /// After 1 hour.
    OneHour,
    /// After 4 hours.
    FourHours,
    /// After 8 hours (the default).
    #[default]
    EightHours,
    /// After 24 hours.
    TwentyFourHours,
    /// Never: only the server's limit applies.
    Never,
}

impl IdleSignOut {
    /// Milliseconds in one hour.
    const HOUR_MILLIS: i64 = 60 * 60 * 1000;

    /// How long idle before signing out, in milliseconds; `None` for [`IdleSignOut::Never`].
    #[must_use]
    pub const fn after_millis(self) -> Option<i64> {
        match self {
            Self::OneHour => Some(Self::HOUR_MILLIS),
            Self::FourHours => Some(4 * Self::HOUR_MILLIS),
            Self::EightHours => Some(8 * Self::HOUR_MILLIS),
            Self::TwentyFourHours => Some(24 * Self::HOUR_MILLIS),
            Self::Never => None,
        }
    }
}

/// An ISO 4217 currency code: three capital letters, e.g. `USD`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct CurrencyCode([u8; 3]);

impl CurrencyCode {
    /// US dollar, the built-in default.
    pub const USD: Self = Self(*b"USD");

    /// The common currencies listed first in the choice list.
    pub const COMMON: [Self; 6] = [
        Self::USD,
        Self(*b"CAD"),
        Self(*b"EUR"),
        Self(*b"GBP"),
        Self(*b"AUD"),
        Self(*b"MXN"),
    ];

    /// Reads a code; surrounding spaces are ignored and letters uppercased.
    ///
    /// # Errors
    ///
    /// [`InvalidCurrencyCode`] unless it's exactly three letters A–Z.
    pub fn parse(text: &str) -> Result<Self, InvalidCurrencyCode> {
        let upper = text.trim().to_ascii_uppercase();
        let bytes: [u8; 3] = upper
            .as_bytes()
            .try_into()
            .map_err(|_| InvalidCurrencyCode)?;
        if bytes.iter().all(u8::is_ascii_uppercase) {
            Ok(Self(bytes))
        } else {
            Err(InvalidCurrencyCode)
        }
    }

    /// The code, e.g. `USD`.
    #[must_use]
    pub fn as_str(&self) -> &str {
        // Always three ASCII capital letters (checked by `parse`), so always valid UTF-8.
        core::str::from_utf8(&self.0).unwrap_or("???")
    }
}

impl Default for CurrencyCode {
    fn default() -> Self {
        Self::USD
    }
}

impl fmt::Display for CurrencyCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(self.as_str())
    }
}

/// A currency code that isn't three letters A–Z.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct InvalidCurrencyCode;

impl fmt::Display for InvalidCurrencyCode {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a currency code is three letters, e.g. USD")
    }
}

impl TryFrom<String> for CurrencyCode {
    type Error = InvalidCurrencyCode;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value)
    }
}

impl From<CurrencyCode> for String {
    fn from(code: CurrencyCode) -> Self {
        code.as_str().to_owned()
    }
}

/// The preferences in effect for someone: every setting has a value.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Preferences {
    /// Distances.
    pub distance: DistanceUnit,
    /// Fuel volumes.
    pub volume: VolumeUnit,
    /// Fuel economy.
    pub economy: EconomyUnit,
    /// Pressures.
    pub pressure: PressureUnit,
    /// Dates.
    pub date_format: DateFormat,
    /// The default currency for amounts.
    pub currency: CurrencyCode,
    /// The app's language.
    pub language: Language,
}

/// What someone (or the household) chose: `None` means "not chosen" (a user then follows the
/// household; the household then follows the built-in default).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PreferenceChoices {
    /// Distances.
    pub distance: Option<DistanceUnit>,
    /// Fuel volumes.
    pub volume: Option<VolumeUnit>,
    /// Fuel economy.
    pub economy: Option<EconomyUnit>,
    /// Pressures.
    pub pressure: Option<PressureUnit>,
    /// Dates.
    pub date_format: Option<DateFormat>,
    /// The default currency.
    pub currency: Option<CurrencyCode>,
    /// The app's language.
    pub language: Option<Language>,
}

impl Preferences {
    /// The preferences in effect: each setting from `mine` if chosen, else the household's,
    /// else the built-in default.
    #[must_use]
    pub fn resolve(mine: &PreferenceChoices, household: &PreferenceChoices) -> Self {
        let defaults = Self::default();
        Self {
            distance: mine
                .distance
                .or(household.distance)
                .unwrap_or(defaults.distance),
            volume: mine.volume.or(household.volume).unwrap_or(defaults.volume),
            economy: mine
                .economy
                .or(household.economy)
                .unwrap_or(defaults.economy),
            pressure: mine
                .pressure
                .or(household.pressure)
                .unwrap_or(defaults.pressure),
            date_format: mine
                .date_format
                .or(household.date_format)
                .unwrap_or(defaults.date_format),
            currency: mine
                .currency
                .or(household.currency)
                .unwrap_or(defaults.currency),
            language: mine
                .language
                .or(household.language)
                .unwrap_or(defaults.language),
        }
    }
}

/// One setting being changed: to a value, or back to "not chosen".
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "to", content = "value", rename_all = "kebab-case")]
pub enum Choice<T> {
    /// Follow the default again (the household's, or the built-in one).
    Default,
    /// Use this.
    Set(T),
}

impl<T: Copy> Choice<T> {
    /// The stored form: `None` for [`Choice::Default`].
    #[must_use]
    pub const fn value(self) -> Option<T> {
        match self {
            Self::Default => None,
            Self::Set(value) => Some(value),
        }
    }
}

/// A preferences record: the household's (its ID is the database's) or one user's (its ID is
/// the user's).
#[derive(Debug)]
pub enum PreferencesRecord {}

/// The ID of a preferences record.
pub type PreferencesId = Id<PreferencesRecord>;

/// Changed settings; `None` means unchanged.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferencesPatch {
    /// See [`Preferences::distance`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub distance: Option<Choice<DistanceUnit>>,
    /// See [`Preferences::volume`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub volume: Option<Choice<VolumeUnit>>,
    /// See [`Preferences::economy`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub economy: Option<Choice<EconomyUnit>>,
    /// See [`Preferences::pressure`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub pressure: Option<Choice<PressureUnit>>,
    /// See [`Preferences::date_format`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub date_format: Option<Choice<DateFormat>>,
    /// See [`Preferences::currency`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub currency: Option<Choice<CurrencyCode>>,
    /// See [`Preferences::language`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub language: Option<Choice<Language>>,
}

impl Patch for PreferencesPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("distance", self.distance.is_some()),
            ("volume", self.volume.is_some()),
            ("economy", self.economy.is_some()),
            ("pressure", self.pressure.is_some()),
            ("date_format", self.date_format.is_some()),
            ("currency", self.currency.is_some()),
            ("language", self.language.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, set)| set.then_some(field))
        .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |name: &str| fields.contains(&name);
        if !keep("distance") {
            self.distance = None;
        }
        if !keep("volume") {
            self.volume = None;
        }
        if !keep("economy") {
            self.economy = None;
        }
        if !keep("pressure") {
            self.pressure = None;
        }
        if !keep("date_format") {
            self.date_format = None;
        }
        if !keep("currency") {
            self.currency = None;
        }
        if !keep("language") {
            self.language = None;
        }
    }
}

impl Record for PreferencesRecord {
    const ENTITY: &'static str = "preferences";
    type Patch = PreferencesPatch;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_built_in_defaults_are_the_agreed_ones() {
        let defaults = Preferences::default();
        assert_eq!(defaults.distance, DistanceUnit::Miles);
        assert_eq!(defaults.volume, VolumeUnit::UsGallons);
        assert_eq!(defaults.economy, EconomyUnit::MpgUs);
        assert_eq!(defaults.pressure, PressureUnit::Psi);
        assert_eq!(defaults.date_format, DateFormat::DayMonthNameYear);
        assert_eq!(defaults.currency.as_str(), "USD");
        assert_eq!(defaults.language, Language::EnglishUs);
    }

    #[test]
    fn mine_beats_the_household_which_beats_the_default() {
        let household = PreferenceChoices {
            distance: Some(DistanceUnit::Kilometers),
            volume: Some(VolumeUnit::Liters),
            ..PreferenceChoices::default()
        };
        let mine = PreferenceChoices {
            distance: Some(DistanceUnit::Miles),
            ..PreferenceChoices::default()
        };

        let effective = Preferences::resolve(&mine, &household);

        assert_eq!(effective.distance, DistanceUnit::Miles, "mine");
        assert_eq!(effective.volume, VolumeUnit::Liters, "the household's");
        assert_eq!(
            effective.pressure,
            PressureUnit::Psi,
            "the built-in default"
        );
    }

    #[test]
    fn idle_sign_out_defaults_to_eight_hours_and_never_has_no_limit() {
        assert_eq!(IdleSignOut::default(), IdleSignOut::EightHours);
        assert_eq!(
            IdleSignOut::EightHours.after_millis(),
            Some(8 * 60 * 60 * 1000)
        );
        assert_eq!(IdleSignOut::OneHour.after_millis(), Some(60 * 60 * 1000));
        assert_eq!(IdleSignOut::Never.after_millis(), None);
        assert_eq!(
            serde_json::to_string(&IdleSignOut::TwentyFourHours).unwrap(),
            r#""twenty-four-hours""#
        );
    }

    #[test]
    fn currency_codes_are_three_letters() {
        assert_eq!(CurrencyCode::parse(" cad ").unwrap().as_str(), "CAD");
        assert_eq!(CurrencyCode::parse("US"), Err(InvalidCurrencyCode));
        assert_eq!(CurrencyCode::parse("US1"), Err(InvalidCurrencyCode));
        assert_eq!(CurrencyCode::parse("USDX"), Err(InvalidCurrencyCode));
        assert!(serde_json::from_str::<CurrencyCode>("\"EUR\"").is_ok());
        assert!(serde_json::from_str::<CurrencyCode>("\"E1R\"").is_err());
    }

    #[test]
    fn a_patch_can_set_a_setting_or_put_it_back_to_default() {
        let patch = PreferencesPatch {
            distance: Some(Choice::Set(DistanceUnit::Kilometers)),
            pressure: Some(Choice::Default),
            ..PreferencesPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["distance", "pressure"]);
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(
            json,
            r#"{"distance":{"to":"set","value":"kilometers"},"pressure":{"to":"default"}}"#
        );
        assert_eq!(
            serde_json::from_str::<PreferencesPatch>(&json).unwrap(),
            patch
        );
        assert_eq!(Choice::<PressureUnit>::Default.value(), None);
    }
}
