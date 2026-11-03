//! The signed-in person's units and formats (decision #029), for screens that show or take
//! quantities.

use dioxus::prelude::*;
use domain::api::PreferencesOutcome;
use domain::custom_field::FieldKind;
use domain::i18n::messages;
use domain::preferences::{DistanceUnit, EconomyUnit, Preferences, VolumeUnit};

use crate::api;
use crate::ui::tr;

/// The person's effective preferences: the built-in defaults until they're loaded (after
/// mounting, never during server rendering).
pub(crate) fn use_units() -> Signal<Preferences> {
    let mut units = use_signal(Preferences::default);
    use_effect(move || {
        spawn(async move {
            if let Ok(PreferencesOutcome::Preferences { view }) = api::get_preferences().await {
                units.set(view.effective);
            }
        });
    });
    units
}

/// The short name of the person's distance unit: "mi" or "km".
pub(crate) fn distance_abbr(unit: DistanceUnit) -> String {
    tr(match unit {
        DistanceUnit::Miles => messages::UNIT_ABBR_MI,
        DistanceUnit::Kilometers => messages::UNIT_ABBR_KM,
    })
}

/// The short name of the person's volume unit: "gal", "gal (UK)", or "L".
pub(crate) fn volume_abbr(unit: VolumeUnit) -> String {
    tr(match unit {
        VolumeUnit::UsGallons => messages::UNIT_ABBR_GAL,
        VolumeUnit::ImperialGallons => messages::UNIT_ABBR_GAL_UK,
        VolumeUnit::Liters => messages::UNIT_ABBR_L,
    })
}

/// The short name of the person's fuel economy unit: "mpg", "L/100 km", ….
pub(crate) fn economy_abbr(unit: EconomyUnit) -> String {
    tr(match unit {
        EconomyUnit::MpgUs => messages::UNIT_ABBR_MPG,
        EconomyUnit::MpgUk => messages::UNIT_ABBR_MPG_UK,
        EconomyUnit::KmPerLiter => messages::UNIT_ABBR_KM_PER_L,
        EconomyUnit::LitersPer100Km => messages::UNIT_ABBR_L_PER_100_KM,
    })
}

/// The unit a custom field of `kind` is typed in, for its label ("mi", "H:MM"); `None` for
/// kinds without one.
pub(crate) fn field_unit(kind: &FieldKind, units: Preferences) -> Option<String> {
    match kind {
        FieldKind::Distance => Some(distance_abbr(units.distance)),
        FieldKind::Duration => Some("HH:MM".to_owned()),
        FieldKind::Volume => Some(volume_abbr(units.volume)),
        FieldKind::FuelEconomy => Some(economy_abbr(units.economy)),
        FieldKind::Number { .. }
        | FieldKind::Text
        | FieldKind::Date
        | FieldKind::YesNo
        | FieldKind::Choice { .. } => None,
    }
}
