//! The groups and vehicles the signed-in person can see, shared by the shell's sidebar and the
//! vehicle screens (decisions #052, #056).
//!
//! The shell loads them again on every navigation and whenever the mode changes. The sidebar
//! lists **All vehicles** (when there's more than one group) and **each group**, with its icon;
//! a one-group household sees just that group, which opens All vehicles.

use dioxus::prelude::*;
use domain::api::{AssetsOutcome, GarageView};
use domain::group::{GroupIcon, GroupId};
use domain::i18n::{MessageId, messages};
use domain::log_area;

use super::icons::{Icon, IconView};
use super::routes::Route;
use super::tr;
use crate::data;

/// The vehicles state, in context (provided by the shell).
#[derive(Debug, Clone, Copy)]
pub(crate) struct VehiclesContext {
    /// What the person can see, once loaded.
    pub garage: Signal<Option<GarageView>>,
}

/// Puts a fresh [`VehiclesContext`] in context; call once, in the shell.
pub(crate) fn provide_vehicles_context() -> VehiclesContext {
    use_context_provider(|| VehiclesContext {
        garage: Signal::new(None),
    })
}

/// The [`VehiclesContext`], from context.
pub(crate) fn use_vehicles() -> VehiclesContext {
    use_context::<VehiclesContext>()
}

/// Keeps the server's answer, if it's the Garage. Returns what went wrong otherwise.
pub(crate) fn apply(
    mut context: VehiclesContext,
    result: Result<AssetsOutcome, ServerFnError>,
) -> Option<MessageId> {
    match result {
        Ok(AssetsOutcome::Garage { view }) => {
            tracing::debug!(target: log_area::UI, vehicles = view.assets.len(), groups = view.groups.len(), "garage loaded");
            context.garage.set(Some(view));
            None
        }
        Ok(AssetsOutcome::Invalid { problem }) => Some(problem.message()),
        Ok(AssetsOutcome::NotAllowed) => Some(messages::VEHICLES_NOT_ALLOWED),
        Ok(other) => {
            tracing::debug!(target: log_area::UI, ?other, "garage: unexpected answer");
            Some(messages::SOMETHING_WENT_WRONG)
        }
        Err(error) => {
            tracing::debug!(target: log_area::UI, %error, "garage request failed");
            Some(super::screens::problem_from_error(&error).message())
        }
    }
}

/// Asks the server for the Garage again.
pub(crate) async fn refresh_garage(context: VehiclesContext) -> Option<MessageId> {
    apply(context, data::get_garage().await)
}

/// A group's icon.
pub(crate) const fn group_icon(icon: GroupIcon) -> Icon {
    match icon {
        GroupIcon::Garage => Icon::Garage,
        GroupIcon::Hangar => Icon::Hangar,
        GroupIcon::Boathouse => Icon::Boathouse,
        GroupIcon::Storage => Icon::Storage,
        GroupIcon::Warehouse => Icon::Warehouse,
        GroupIcon::Shop => Icon::Shop,
        GroupIcon::Farm => Icon::Farm,
        GroupIcon::House => Icon::House,
    }
}

/// A group's name, for the top bar on its page.
pub(crate) fn group_name(context: VehiclesContext, group_id: &str) -> Option<String> {
    let id: GroupId = group_id.parse().ok()?;
    context
        .garage
        .read()
        .as_ref()?
        .groups
        .iter()
        .find(|summary| summary.id == id)
        .map(|summary| summary.group.name.as_str().to_owned())
}

/// A vehicle's name, for the top bar on its page.
pub(crate) fn vehicle_name(context: VehiclesContext, asset_id: &str) -> Option<String> {
    let id: domain::asset::AssetId = asset_id.parse().ok()?;
    context
        .garage
        .read()
        .as_ref()?
        .assets
        .iter()
        .find(|summary| summary.id == id)
        .map(|summary| summary.asset.name.as_str().to_owned())
}

/// The group's name, when the person sees exactly one group.
pub(crate) fn only_group_name(context: VehiclesContext) -> Option<String> {
    match context.garage.read().as_ref()?.groups.as_slice() {
        [only] => Some(only.group.name.as_str().to_owned()),
        _ => None,
    }
}

/// The sidebar's vehicle links: All vehicles and each group, or just the one group.
#[component]
pub(crate) fn SidebarVehicles(on_navigate: EventHandler<MouseEvent>) -> Element {
    let context = use_vehicles();
    let groups = context
        .garage
        .read()
        .as_ref()
        .map(|view| view.groups.clone())
        .unwrap_or_default();
    // One group: it is "everything", so it opens All vehicles under its own name and icon.
    if let [only] = groups.as_slice() {
        return rsx! {
            SidebarEntry {
                to: Route::AllVehicles {},
                label: only.group.name.as_str().to_owned(),
                icon: group_icon(only.group.icon),
                on_navigate,
            }
        };
    }
    rsx! {
        SidebarEntry {
            to: Route::AllVehicles {},
            label: tr(messages::NAV_ALL_VEHICLES),
            icon: Icon::AllVehicles,
            on_navigate,
        }
        for summary in groups {
            SidebarEntry {
                key: "{summary.id}",
                to: Route::GroupVehicles { group_id: summary.id.to_string() },
                label: summary.group.name.as_str().to_owned(),
                icon: group_icon(summary.group.icon),
                on_navigate,
            }
        }
    }
}

/// One sidebar link with a label in words (a group's name).
#[component]
fn SidebarEntry(
    to: Route,
    label: String,
    icon: Icon,
    on_navigate: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        Link {
            class: "sidebar-link",
            active_class: "active",
            to,
            title: label.clone(),
            onclick: move |event| on_navigate.call(event),
            IconView { icon }
            span { class: "sidebar-label", {label} }
        }
    }
}
