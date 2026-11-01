//! Asset profiles: what each kind of asset starts with, as data (plan §4 Asset profiles,
//! decision #030).
//!
//! The six Phase 2 profiles are RON files in `crates/domain/profiles/`, compiled in (see the
//! README there). Adding a kind means adding a profile, never `if kind == …` in code. A new
//! asset gets **a copy** of its profile's meters and custom fields
//! ([`Profile::new_meters`], [`Profile::new_fields`]), which it can then change.
//!
//! # Examples
//!
//! ```
//! use domain::asset::AssetKind;
//! use domain::profile::profile;
//!
//! let generator = profile(&AssetKind::parse("generator").unwrap()).unwrap();
//! assert_eq!(generator.meters.len(), 1);
//! assert!(profile(&AssetKind::parse("spaceship").unwrap()).is_none());
//! ```

use std::sync::LazyLock;

use serde::Deserialize;

use crate::asset::{AssetId, AssetKind, IdentifierKind};
use crate::custom_field::{CustomField, FieldKey, FieldKind, FieldRange};
use crate::i18n::Translator;
use crate::label::Label;
use crate::meter::{EventKind, Meter, MeterDimension, MeterUnit};
use crate::preferences::DistanceUnit;

/// The profile files, by name, in the order kinds are offered.
const SOURCES: [(&str, &str); 6] = [
    ("car.ron", include_str!("../profiles/car.ron")),
    ("truck.ron", include_str!("../profiles/truck.ron")),
    ("motorcycle.ron", include_str!("../profiles/motorcycle.ron")),
    ("trailer.ron", include_str!("../profiles/trailer.ron")),
    ("generator.ron", include_str!("../profiles/generator.ron")),
    (
        "other-equipment.ron",
        include_str!("../profiles/other-equipment.ron"),
    ),
];

/// One kind's starting setup.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Profile {
    /// The kind's stable ID, stored on every asset.
    pub id: AssetKind,
    /// Bumped whenever the profile changes.
    pub version: u32,
    /// The kind's name (a message key).
    pub name: String,
    /// The kinds of events it has.
    pub event_kinds: Vec<EventKind>,
    /// Extra sections shown for it.
    pub sections: Vec<Section>,
    /// The identifiers it offers.
    pub identifiers: Vec<ProfileIdentifier>,
    /// The official documents expected (message keys; documents arrive in milestone 8).
    pub documents: Vec<String>,
    /// The meters a new asset starts with.
    pub meters: Vec<ProfileMeter>,
    /// The custom fields a new asset starts with.
    pub fields: Vec<ProfileField>,
}

/// An extra section of screens and forms for some kinds.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Section {
    /// Fuel economy.
    FuelEconomy,
    /// Towing and hauling.
    TowingHauling,
}

/// An identifier a profile offers, e.g. a VIN.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProfileIdentifier {
    /// Its stable ID.
    pub kind: IdentifierKind,
    /// Its name (a message key).
    pub name: String,
}

/// A meter a new asset starts with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProfileMeter {
    /// Its name (a message key).
    pub name: String,
    /// What it measures.
    pub dimension: MeterDimension,
    /// Its unit; `None` means the person's distance preference.
    pub unit: Option<MeterUnit>,
    /// Decimal places shown.
    pub precision: u8,
    /// The kinds of events that require it.
    pub required_for: Vec<EventKind>,
}

/// A custom field a new asset starts with.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct ProfileField {
    /// Its stable key (#057), e.g. `trip-time`; never changed once shipped.
    pub key: FieldKey,
    /// Its label (a message key).
    pub label: String,
    /// The kind of event it's on.
    pub applies_to: EventKind,
    /// What it holds.
    pub kind: FieldKind,
    /// Whether it starts turned on.
    pub enabled: bool,
    /// Its sensible range, if any.
    pub range: Option<FieldRange>,
}

/// A profile file that couldn't be read (a build with a broken profile; the tests catch it).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProfileError {
    /// The file.
    pub file: &'static str,
    /// What's wrong.
    pub message: String,
}

impl core::fmt::Display for ProfileError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "profile {}: {}", self.file, self.message)
    }
}

/// Every profile, read once.
static PROFILES: LazyLock<Result<Vec<Profile>, ProfileError>> = LazyLock::new(|| {
    SOURCES
        .iter()
        .map(|(file, source)| {
            ron::from_str::<Profile>(source).map_err(|error| ProfileError {
                file,
                message: error.to_string(),
            })
        })
        .collect()
});

/// Every profile, in the order kinds are offered.
///
/// # Errors
///
/// [`ProfileError`] if a profile file can't be read (never in a tested build).
pub fn profiles() -> Result<&'static [Profile], ProfileError> {
    PROFILES.as_deref().map_err(Clone::clone)
}

/// The profile for `kind`, if there is one (and the profiles could be read).
#[must_use]
pub fn profile(kind: &AssetKind) -> Option<&'static Profile> {
    profiles().ok()?.iter().find(|profile| &profile.id == kind)
}

impl Profile {
    /// The meters a new asset of this kind starts with. A distance meter without its own unit
    /// uses the person's distance preference; names are translated with `translator`.
    #[must_use]
    pub fn new_meters(
        &self,
        asset: AssetId,
        distance: DistanceUnit,
        translator: &Translator,
    ) -> Vec<Meter> {
        let person_unit = match distance {
            DistanceUnit::Miles => MeterUnit::Miles,
            DistanceUnit::Kilometers => MeterUnit::Kilometers,
        };
        // Each profile meter, named in the person's language; one whose name doesn't fit a
        // label is skipped (the tests make sure none are).
        self.meters
            .iter()
            .filter_map(|meter| {
                Some(Meter {
                    asset,
                    name: Label::parse(&translator.tr_key(&meter.name)).ok()?,
                    dimension: meter.dimension,
                    unit: meter.unit.unwrap_or(person_unit),
                    precision: meter.precision,
                    description: None,
                    required_for: meter.required_for.clone(),
                })
            })
            .collect()
    }

    /// The custom fields a new asset of this kind starts with, labelled with `translator`.
    #[must_use]
    pub fn new_fields(&self, asset: AssetId, translator: &Translator) -> Vec<CustomField> {
        // Each profile field, labelled in the person's language (skipped if it doesn't fit).
        self.fields
            .iter()
            .filter_map(|field| {
                Some(CustomField {
                    asset,
                    applies_to: field.applies_to,
                    key: field.key.clone(),
                    label: Label::parse(&translator.tr_key(&field.label)).ok()?,
                    kind: field.kind.clone(),
                    enabled: field.enabled,
                    range: field.range,
                })
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::custom_field::check_field;
    use crate::meter::check_meter;
    use crate::test_support::test_id;

    fn all() -> &'static [Profile] {
        profiles().unwrap()
    }

    fn named(id: &str) -> &'static Profile {
        profile(&AssetKind::parse(id).unwrap()).unwrap()
    }

    #[test]
    fn the_six_phase_2_kinds_load_in_order() {
        let ids: Vec<&str> = all().iter().map(|profile| profile.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "car",
                "truck",
                "motorcycle",
                "trailer",
                "generator",
                "other-equipment"
            ]
        );
    }

    #[test]
    fn each_file_is_named_after_its_kind() {
        for ((file, _), profile) in SOURCES.iter().zip(all()) {
            assert_eq!(*file, format!("{}.ron", profile.id.as_str()));
            assert!(profile.version >= 1, "{file}");
        }
    }

    #[test]
    fn every_name_is_a_message_in_the_catalog() {
        let english = Translator::english();
        for profile in all() {
            let keys = std::iter::once(&profile.name)
                .chain(
                    profile
                        .identifiers
                        .iter()
                        .map(|identifier| &identifier.name),
                )
                .chain(&profile.documents)
                .chain(profile.meters.iter().map(|meter| &meter.name))
                .chain(profile.fields.iter().map(|field| &field.label));
            for key in keys {
                assert!(english.has_key(key), "{}: {key}", profile.id.as_str());
                assert!(
                    Label::parse(&english.tr_key(key)).is_ok(),
                    "{key} fits a label"
                );
            }
        }
    }

    #[test]
    fn every_meter_and_field_is_valid() {
        for profile in all() {
            for meter in &profile.meters {
                let unit = meter.unit.unwrap_or(MeterUnit::Miles);
                assert!(
                    check_meter(meter.dimension, unit, meter.precision, "").is_ok(),
                    "{}: {}",
                    profile.id.as_str(),
                    meter.name
                );
                for kind in &meter.required_for {
                    assert!(profile.event_kinds.contains(kind), "{}", meter.name);
                }
            }
            let mut keys: Vec<&str> = profile
                .fields
                .iter()
                .map(|field| field.key.as_str())
                .collect();
            keys.sort_unstable();
            keys.dedup();
            assert_eq!(
                keys.len(),
                profile.fields.len(),
                "{}: keys repeat",
                profile.id.as_str()
            );
            for field in &profile.fields {
                assert!(
                    check_field(&field.kind, field.range).is_ok(),
                    "{}",
                    field.label
                );
                assert!(
                    profile.event_kinds.contains(&field.applies_to),
                    "{}",
                    field.label
                );
            }
        }
    }

    #[test]
    fn decision_30_holds() {
        // A fuel-up requires the odometer, or run hours for the generator.
        for id in ["car", "truck", "motorcycle"] {
            let odometer = &named(id).meters[0];
            assert_eq!(odometer.name, "meter-odometer", "{id}");
            assert_eq!(odometer.required_for, vec![EventKind::Fuel], "{id}");
        }
        assert_eq!(named("generator").meters[0].name, "meter-run-hours");
        // Trailers and other equipment have no fuel-ups.
        for id in ["trailer", "other-equipment"] {
            assert!(!named(id).event_kinds.contains(&EventKind::Fuel), "{id}");
        }
        // Trucks add towing and hauling; trip EV distance starts off.
        assert!(named("truck").sections.contains(&Section::TowingHauling));
        assert!(!named("car").fields[1].enabled);
    }

    #[test]
    fn a_new_asset_copies_its_profile_in_the_persons_units() {
        let car = named("car");
        let english = Translator::english();
        let meters = car.new_meters(test_id(1), DistanceUnit::Kilometers, english);
        assert_eq!(meters.len(), 1);
        assert_eq!(meters[0].name.as_str(), "Odometer");
        assert_eq!(meters[0].unit, MeterUnit::Kilometers);
        assert_eq!(meters[0].asset, test_id(1));
        let generator =
            named("generator").new_meters(test_id(2), DistanceUnit::Kilometers, english);
        assert_eq!(
            generator[0].unit,
            MeterUnit::Hours,
            "its own unit, not distance"
        );
        let fields = car.new_fields(test_id(1), english);
        assert_eq!(fields.len(), 2);
        assert_eq!(fields[0].label.as_str(), "Trip time");
        assert_eq!(fields[0].key.as_str(), "trip-time");
        assert_eq!(fields[0].kind, FieldKind::Duration);
    }
}
