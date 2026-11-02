//! The signed-in person's units and formats (decision #029), for screens that show or take
//! quantities.

use dioxus::prelude::*;
use domain::api::PreferencesOutcome;
use domain::preferences::Preferences;

use crate::api;

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
