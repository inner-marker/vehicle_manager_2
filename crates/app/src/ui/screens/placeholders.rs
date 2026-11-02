//! Placeholder screens: each shows "Coming soon" under the top-bar title until its
//! phase builds it (Garage and the vehicle screens in Phase 2, Due in Phase 5, ...).

use dioxus::prelude::*;
use domain::i18n::messages;

use crate::ui::routes::Route;
use crate::ui::tr;

/// The shared "Coming soon" body.
#[component]
fn ComingSoon() -> Element {
    rsx! {
        p { class: "placeholder", {tr(messages::PLACEHOLDER_COMING_SOON)} }
    }
}

/// What's due and open squawks (Phase 5).
#[component]
pub(crate) fn Due() -> Element {
    rsx! { ComingSoon {} }
}

/// Every event, newest first (Phase 2).
#[component]
pub(crate) fn Timeline() -> Element {
    rsx! { ComingSoon {} }
}

/// History reports and exports (post-MVP).
#[component]
pub(crate) fn Reports() -> Element {
    rsx! { ComingSoon {} }
}

/// Logging a fuel-up, reading, squawk, or maintenance (Phase 2).
#[component]
pub(crate) fn LogEvent() -> Element {
    rsx! { ComingSoon {} }
}

/// No screen at this address.
#[component]
pub(crate) fn NotFound(segments: Vec<String>) -> Element {
    tracing::debug!(target: domain::log_area::UI, path = segments.join("/"), "no route for path");
    rsx! {
        Link { class: "not-found-back", to: Route::AllVehicles {}, {tr(messages::NOT_FOUND_BACK)} }
    }
}
