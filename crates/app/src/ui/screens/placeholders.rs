//! Placeholder screens: each shows "Coming soon" under the top-bar title until its
//! phase builds it (Garage and the vehicle screens in Phase 2, Due in Phase 5, ...).

use dioxus::prelude::*;
use domain::i18n::{MessageArg, messages};

use crate::ui::routes::Route;
use crate::ui::{tr, tr_with};

/// The shared "Coming soon" body.
#[component]
fn ComingSoon() -> Element {
    rsx! {
        p { class: "placeholder", {tr(messages::PLACEHOLDER_COMING_SOON)} }
    }
}

/// Home: one card per vehicle (Phase 2).
#[component]
pub(crate) fn Garage() -> Element {
    rsx! { ComingSoon {} }
}

/// What's due and open squawks (Phase 5).
#[component]
pub(crate) fn Due() -> Element {
    rsx! { ComingSoon {} }
}

/// The message center (Phase 2).
#[component]
pub(crate) fn Messages() -> Element {
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

/// Managing vehicles (Phase 2).
#[component]
pub(crate) fn Vehicles() -> Element {
    rsx! { ComingSoon {} }
}

/// One vehicle's overview (Phase 2). Shows which vehicle was asked for.
#[component]
pub(crate) fn VehicleDetail(asset_id: String) -> Element {
    rsx! {
        p { class: "detail-id", {tr_with(messages::VEHICLE_DETAIL_ID, &[("id", MessageArg::from(asset_id))])} }
        ComingSoon {}
    }
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
        Link { class: "not-found-back", to: Route::Garage {}, {tr(messages::NOT_FOUND_BACK)} }
    }
}
