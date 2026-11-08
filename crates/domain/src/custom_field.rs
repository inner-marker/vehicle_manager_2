//! Custom field definitions (plan §3 Custom fields, decision #030).
//!
//! A custom field is **defined first** (its label, type, and which kind of event it's on),
//! then filled in on events (milestone 6). Profiles ship some (e.g. trip time on fuel-ups),
//! which each asset gets a copy of and can turn off or extend.
//!
//! **Values are whole numbers in a fixed storage unit**, like every quantity (plan §3):
//! meters for distance, seconds for duration, milliliters for volume, meters per liter for
//! fuel economy, and for a plain number, the number times 10^decimals. An optional
//! **sensible range** in the same units makes forms (and later voice) warn about unlikely
//! values; it never refuses them.
//!
//! # Examples
//!
//! ```
//! use domain::custom_field::{FieldRange, RangeCheck};
//!
//! let tank = FieldRange::new(Some(0), Some(100_000)).unwrap(); // up to 100 L, in mL
//! assert_eq!(tank.check(60_000), RangeCheck::Within);
//! assert_eq!(tank.check(150_000), RangeCheck::Above);
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Clearable, Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;
use crate::meter::EventKind;

/// The ID of a custom field definition.
pub type CustomFieldId = Id<CustomField>;

/// The most decimal places a number field may have.
pub const MAX_DECIMALS: u8 = 4;
/// The most choices a choice list may have.
pub const MAX_CHOICES: usize = 30;

/// What a custom field holds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "kebab-case")]
pub enum FieldKind {
    /// Free text.
    Text,
    /// A number with a fixed number of decimal places.
    Number {
        /// Decimal places, 0–4.
        decimals: u8,
    },
    /// A calendar date.
    Date,
    /// Yes or no.
    YesNo,
    /// One of a list of choices.
    Choice {
        /// The choices, in order.
        options: Vec<Label>,
    },
    /// A distance (stored in meters, shown in the person's distance unit).
    Distance,
    /// A duration (stored in seconds, entered and shown as `HH:MM:SS`).
    Duration,
    /// A volume (stored in milliliters, shown in the person's volume unit).
    Volume,
    /// Fuel economy (stored in meters per liter, shown in the person's economy unit).
    FuelEconomy,
}

impl FieldKind {
    /// Whether values are numbers, so a range can apply.
    #[must_use]
    pub const fn is_numeric(&self) -> bool {
        matches!(
            self,
            Self::Number { .. }
                | Self::Distance
                | Self::Duration
                | Self::Volume
                | Self::FuelEconomy
        )
    }
}

/// An optional sensible range, in the field's storage unit; either end may be open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct FieldRange {
    /// The lowest likely value.
    pub min: Option<i64>,
    /// The highest likely value.
    pub max: Option<i64>,
}

/// Where a value falls against a [`FieldRange`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RangeCheck {
    /// Inside the range (or no limit on that side).
    Within,
    /// Below the lowest likely value: warn.
    Below,
    /// Above the highest likely value: warn.
    Above,
}

impl FieldRange {
    /// A range, if its ends are in order.
    ///
    /// # Errors
    ///
    /// [`CustomFieldError::RangeReversed`] if `min` is above `max`.
    pub fn new(min: Option<i64>, max: Option<i64>) -> Result<Self, CustomFieldError> {
        match (min, max) {
            (Some(low), Some(high)) if low > high => Err(CustomFieldError::RangeReversed),
            _ => Ok(Self { min, max }),
        }
    }

    /// Where `value` falls.
    #[must_use]
    pub fn check(self, value: i64) -> RangeCheck {
        if self.min.is_some_and(|min| value < min) {
            RangeCheck::Below
        } else if self.max.is_some_and(|max| value > max) {
            RangeCheck::Above
        } else {
            RangeCheck::Within
        }
    }
}

/// A field's stable key, e.g. `trip-ev-distance`: 1–40 lowercase letters, digits, and `-`
/// (#057). Formulas, reports, and voice refer to fields by key, so renaming a label never
/// breaks them. Built-in fields get theirs from data; others from their label.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FieldKey(String);

/// The most characters a field key may have.
pub const FIELD_KEY_MAX_CHARS: usize = 40;

impl FieldKey {
    /// A key as written in data.
    ///
    /// # Errors
    ///
    /// [`CustomFieldError::BadKey`] unless it's 1–40 lowercase letters, digits, or `-`, not
    /// starting or ending with `-`.
    pub fn parse(input: &str) -> Result<Self, CustomFieldError> {
        let valid = (1..=FIELD_KEY_MAX_CHARS).contains(&input.len())
            && !input.starts_with('-')
            && !input.ends_with('-')
            && input
                .chars()
                .all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
        valid
            .then(|| Self(input.to_owned()))
            .ok_or(CustomFieldError::BadKey)
    }

    /// A key made from `label`, different from every key in `taken` (by adding `-2`, `-3`, …).
    #[must_use]
    pub fn from_label(label: &Label, taken: &[Self]) -> Self {
        // Letters and digits kept (lowercased); anything else becomes one `-`.
        let mut slug = String::new();
        for c in label.as_str().chars() {
            if c.is_ascii_alphanumeric() {
                slug.push(c.to_ascii_lowercase());
            } else if !slug.ends_with('-') {
                slug.push('-');
            }
        }
        let slug = match slug.trim_matches('-') {
            "" => "field",
            trimmed => trimmed,
        };
        // One more candidate than there are taken keys, so one is always free.
        (1..=taken.len() + 1)
            .map(|n| {
                let suffix = if n == 1 {
                    String::new()
                } else {
                    format!("-{n}")
                };
                // ASCII only, so characters are bytes.
                let room = FIELD_KEY_MAX_CHARS - suffix.len();
                let base = slug[..slug.len().min(room)].trim_end_matches('-');
                Self(format!("{base}{suffix}"))
            })
            .find(|candidate| !taken.contains(candidate))
            .unwrap_or_else(|| Self(slug.chars().take(FIELD_KEY_MAX_CHARS).collect()))
    }

    /// The key.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FieldKey {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<FieldKey> for String {
    fn from(key: FieldKey) -> Self {
        key.0
    }
}

/// One custom field on one asset's events.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomField {
    /// Whose field.
    pub asset: AssetId,
    /// The kinds of events it's shown on (at least one), like a meter's.
    pub shown_on: Vec<EventKind>,
    /// Its stable key (#057); fixed once the field exists.
    pub key: FieldKey,
    /// Its label on forms, e.g. "Trip time".
    pub label: Label,
    /// What it holds; fixed once the field exists (values depend on it).
    pub kind: FieldKind,
    /// Shown on forms (a profile's field can be turned off).
    pub enabled: bool,
    /// The kinds of events that require it (among those it's shown on): given, or skipped with
    /// a reason (choice 5). A field that's turned off is never required.
    pub required_for: Vec<EventKind>,
    /// The sensible range, for numeric kinds.
    pub range: Option<FieldRange>,
}

impl CustomField {
    /// Whether an event of `kind` must give this field (or skip it with a reason).
    #[must_use]
    pub fn required_on(&self, kind: EventKind) -> bool {
        self.shown(kind) && self.required_for.contains(&kind)
    }

    /// Whether events of `kind` show this field (it's on, and shown on that kind).
    #[must_use]
    pub fn shown(&self, kind: EventKind) -> bool {
        self.enabled && self.shown_on.contains(&kind)
    }
}

/// Why a custom field's settings were refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CustomFieldError {
    /// More than [`MAX_DECIMALS`] decimal places.
    TooManyDecimals,
    /// A choice list with no choices, or more than [`MAX_CHOICES`].
    BadChoices,
    /// A range on a field that doesn't hold numbers.
    RangeNotNumeric,
    /// The range's lowest value is above its highest.
    RangeReversed,
    /// A key that isn't 1–40 lowercase letters, digits, or `-`.
    BadKey,
    /// Shown on no kind of event.
    NoKinds,
}

impl CustomFieldError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::TooManyDecimals => messages::FIELD_TOO_MANY_DECIMALS,
            Self::BadChoices => messages::FIELD_BAD_CHOICES,
            Self::RangeNotNumeric | Self::RangeReversed => messages::FIELD_BAD_RANGE,
            Self::BadKey => messages::SOMETHING_WENT_WRONG,
            Self::NoKinds => messages::FIELD_NO_KINDS,
        }
    }
}

/// Checks a field's kind and range together.
///
/// # Errors
///
/// [`CustomFieldError`] for the first problem found.
pub fn check_field(kind: &FieldKind, range: Option<FieldRange>) -> Result<(), CustomFieldError> {
    if let FieldKind::Number { decimals } = kind
        && *decimals > MAX_DECIMALS
    {
        return Err(CustomFieldError::TooManyDecimals);
    }
    if let FieldKind::Choice { options } = kind
        && (options.is_empty() || options.len() > MAX_CHOICES)
    {
        return Err(CustomFieldError::BadChoices);
    }
    match range {
        Some(_) if !kind.is_numeric() => Err(CustomFieldError::RangeNotNumeric),
        Some(range) => FieldRange::new(range.min, range.max).map(|_| ()),
        None => Ok(()),
    }
}

/// The changeable fields of a [`CustomField`]; `Some` means "change this".
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CustomFieldPatch {
    /// See [`CustomField::asset`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub asset: Option<AssetId>,
    /// See [`CustomField::shown_on`] (the whole list).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shown_on: Option<Vec<EventKind>>,
    /// See [`CustomField::key`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<FieldKey>,
    /// See [`CustomField::label`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub label: Option<Label>,
    /// See [`CustomField::kind`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub kind: Option<FieldKind>,
    /// See [`CustomField::enabled`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub enabled: Option<bool>,
    /// See [`CustomField::required_for`] (the whole list).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub required_for: Option<Vec<EventKind>>,
    /// See [`CustomField::range`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub range: Option<Clearable<FieldRange>>,
}

impl CustomFieldPatch {
    /// Every field of `field`, for creating it.
    #[must_use]
    pub fn all(field: CustomField) -> Self {
        Self {
            asset: Some(field.asset),
            shown_on: Some(field.shown_on),
            key: Some(field.key),
            label: Some(field.label),
            kind: Some(field.kind),
            enabled: Some(field.enabled),
            required_for: Some(field.required_for),
            range: Some(Clearable::from_option(field.range)),
        }
    }
}

/// Every field name, in a fixed order.
const CUSTOM_FIELD_FIELDS: [&str; 8] = [
    "asset",
    "shown_on",
    "key",
    "label",
    "kind",
    "enabled",
    "required_for",
    "range",
];

impl Patch for CustomFieldPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        let set = [
            self.asset.is_some(),
            self.shown_on.is_some(),
            self.key.is_some(),
            self.label.is_some(),
            self.kind.is_some(),
            self.enabled.is_some(),
            self.required_for.is_some(),
            self.range.is_some(),
        ];
        // Each field name paired with whether this patch sets it, keeping the set ones.
        CUSTOM_FIELD_FIELDS
            .into_iter()
            .zip(set)
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |name: &str| fields.contains(&name);
        if !keep("asset") {
            self.asset = None;
        }
        if !keep("shown_on") {
            self.shown_on = None;
        }
        if !keep("key") {
            self.key = None;
        }
        if !keep("label") {
            self.label = None;
        }
        if !keep("kind") {
            self.kind = None;
        }
        if !keep("enabled") {
            self.enabled = None;
        }
        if !keep("required_for") {
            self.required_for = None;
        }
        if !keep("range") {
            self.range = None;
        }
    }
}

impl Record for CustomField {
    const ENTITY: &'static str = "custom_field";
    type Patch = CustomFieldPatch;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    #[test]
    fn keys_are_made_from_labels_and_never_repeat() {
        let label = |text| Label::parse(text).unwrap();
        let key = |text| FieldKey::parse(text).unwrap();
        assert_eq!(
            FieldKey::from_label(&label("Trip EV miles"), &[]).as_str(),
            "trip-ev-miles"
        );
        assert_eq!(
            FieldKey::from_label(&label("  Tire (front) PSI! "), &[]).as_str(),
            "tire-front-psi"
        );
        assert_eq!(FieldKey::from_label(&label("Øl"), &[]).as_str(), "l");
        assert_eq!(FieldKey::from_label(&label("???"), &[]).as_str(), "field");
        let taken = [key("trip-time"), key("trip-time-2")];
        assert_eq!(
            FieldKey::from_label(&label("Trip time"), &taken).as_str(),
            "trip-time-3"
        );
        let long = FieldKey::from_label(&label(&"word ".repeat(8)), &[]);
        assert!(FieldKey::parse(long.as_str()).is_ok(), "{long:?}");
    }

    #[test]
    fn keys_from_data_are_checked() {
        assert!(FieldKey::parse("trip-ev-distance").is_ok());
        for bad in ["", "Trip", "trip_time", "-trip", "trip-", &"x".repeat(41)] {
            assert_eq!(
                FieldKey::parse(bad),
                Err(CustomFieldError::BadKey),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn ranges_warn_and_never_refuse() {
        let range = FieldRange::new(Some(10), None).unwrap();
        assert_eq!(range.check(9), RangeCheck::Below);
        assert_eq!(range.check(10), RangeCheck::Within);
        assert_eq!(range.check(i64::MAX), RangeCheck::Within, "no upper limit");
        assert_eq!(
            FieldRange::new(Some(5), Some(4)),
            Err(CustomFieldError::RangeReversed)
        );
    }

    #[test]
    fn kinds_and_ranges_are_checked_together() {
        assert_eq!(
            check_field(&FieldKind::Number { decimals: 4 }, None),
            Ok(())
        );
        assert_eq!(
            check_field(&FieldKind::Number { decimals: 5 }, None),
            Err(CustomFieldError::TooManyDecimals)
        );
        assert_eq!(
            check_field(&FieldKind::Choice { options: vec![] }, None),
            Err(CustomFieldError::BadChoices)
        );
        let range = FieldRange::new(Some(0), Some(1)).ok();
        assert_eq!(check_field(&FieldKind::Volume, range), Ok(()));
        assert_eq!(
            check_field(&FieldKind::Text, range),
            Err(CustomFieldError::RangeNotNumeric)
        );
    }

    #[test]
    fn a_create_sets_every_field_and_survives_the_trip() {
        let patch = CustomFieldPatch::all(CustomField {
            asset: test_id(1),
            shown_on: vec![EventKind::Fuel, EventKind::Reading],
            key: FieldKey::parse("tire-brand").unwrap(),
            label: Label::parse("Tire brand").unwrap(),
            kind: FieldKind::Choice {
                options: vec![Label::parse("Michelin").unwrap()],
            },
            enabled: true,
            required_for: vec![EventKind::Fuel],
            range: None,
        });
        assert_eq!(patch.changed_fields(), CUSTOM_FIELD_FIELDS.to_vec());
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(
            serde_json::from_str::<CustomFieldPatch>(&json).unwrap(),
            patch
        );
    }
}
