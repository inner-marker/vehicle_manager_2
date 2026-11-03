//! The **+ Log** chooser (decision #040; option A of
//! `docs/ui-sketches/p2m6-log-chooser-options.svg`).
//!
//! One dialog (a sheet on phones): the vehicle at the top, with last time's chosen (or the
//! vehicle whose page it's opened from), then a tile for each kind of event the person may log
//! on it, by their role and mode. Squawks and maintenance join in milestones 7 and 8.
//!
//! **Last time's vehicle** is remembered on this device (the browser's local storage), so it
//! survives a reload.

use dioxus::prelude::*;
use domain::access::{ActiveMode, AssetAction};
use domain::api::AssetSummary;
use domain::asset::AssetId;
use domain::i18n::{MessageArg, messages};
use domain::log_area;
use domain::meter::EventKind;
use domain::profile::event_kinds;

use super::vehicle_setup::event_value;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::routes::Route;
use crate::ui::vehicles::use_vehicles;
use crate::ui::{tr, tr_with};

/// Where the last vehicle is kept on this device.
const LAST_VEHICLE_KEY: &str = "vehiclemanager.lastLogVehicle";

/// The chooser's state, in context (provided by the shell).
#[derive(Debug, Clone, Copy)]
pub(crate) struct LogChooser {
    /// Whether it's open.
    open: Signal<bool>,
    /// The vehicle to start on (`None`: last time's).
    start: Signal<Option<AssetId>>,
    /// The last vehicle something was logged for, on this device.
    last: Signal<Option<AssetId>>,
}

impl LogChooser {
    /// Opens the chooser, on `vehicle` if given (otherwise last time's).
    pub(crate) fn open(mut self, vehicle: Option<AssetId>) {
        self.start.set(vehicle);
        self.open.set(true);
    }

    fn close(mut self) {
        self.open.set(false);
    }

    /// Remembers `vehicle` as the last one logged for.
    pub(crate) fn remember(mut self, vehicle: AssetId) {
        self.last.set(Some(vehicle));
        let script = format!(
            "localStorage.setItem({}, {});",
            serde_json::to_string(LAST_VEHICLE_KEY).unwrap_or_default(),
            serde_json::to_string(&vehicle.to_string()).unwrap_or_default()
        );
        let _ = document::eval(&script);
    }
}

/// Puts a [`LogChooser`] in context and reads last time's vehicle; call once, in the shell.
pub(crate) fn provide_log_chooser() -> LogChooser {
    let chooser = use_context_provider(|| LogChooser {
        open: Signal::new(false),
        start: Signal::new(None),
        last: Signal::new(None),
    });
    let mut last = chooser.last;
    // After mounting only (never during server rendering).
    use_effect(move || {
        spawn(async move {
            let script = format!(
                "return localStorage.getItem({});",
                serde_json::to_string(LAST_VEHICLE_KEY).unwrap_or_default()
            );
            if let Ok(Some(text)) = document::eval(&script).join::<Option<String>>().await
                && let Ok(id) = text.parse::<AssetId>()
            {
                last.set(Some(id));
            }
        });
    });
    chooser
}

/// The [`LogChooser`], from context.
pub(crate) fn use_log_chooser() -> LogChooser {
    use_context::<LogChooser>()
}

/// The kinds of events the person may log on `vehicle` in `mode`: its kind's (#030), limited by
/// their role there and the mode (plan §6). Fuel-ups and readings only, until milestone 7.
pub(crate) fn loggable_kinds(vehicle: &AssetSummary, mode: ActiveMode) -> Vec<EventKind> {
    let kinds = event_kinds(&vehicle.asset.kind);
    let allowed = vehicle.role.min(mode.cap());
    [
        (EventKind::Fuel, AssetAction::LogFuel),
        (EventKind::Reading, AssetAction::LogReading),
    ]
    .into_iter()
    .filter(|(kind, action)| kinds.contains(kind) && allowed >= action.required_role())
    .map(|(kind, _)| kind)
    .collect()
}

/// What a kind's tile says under its name.
const fn kind_detail(kind: EventKind) -> domain::i18n::MessageId {
    match kind {
        EventKind::Fuel => messages::LOG_FUEL_DETAIL,
        EventKind::Reading => messages::LOG_READING_DETAIL,
        EventKind::Maintenance | EventKind::Repair => messages::LOG_LATER,
    }
}

/// The chooser dialog, while it's open (rendered by the shell).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn LogChooserDialog() -> Element {
    let chooser = use_log_chooser();
    if !(chooser.open)() {
        return rsx! {};
    }
    let start = (chooser.start)();
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| chooser.close(),
            div {
                class: "dialog log-chooser",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::LOG_TITLE)} }
                ChooserBody { start, on_done: move |()| chooser.close() }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| chooser.close(), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

/// The vehicle and the tiles; `on_done` after a tile is chosen.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ChooserBody(start: Option<AssetId>, on_done: EventHandler<()>) -> Element {
    let chooser = use_log_chooser();
    let vehicles = use_vehicles();
    let auth = use_auth();
    let navigator = use_navigator();
    let mode = match auth() {
        AuthState::SignedIn(session) => session.mode,
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => ActiveMode::Driver,
    };
    let last = (chooser.last)();
    // The vehicles the person may log for, last time's first.
    let mut choices: Vec<AssetSummary> = (vehicles.garage)()
        .map(|garage| garage.assets)
        .unwrap_or_default()
        .into_iter()
        .filter(|vehicle| !loggable_kinds(vehicle, mode).is_empty())
        .collect();
    choices.sort_by_key(|vehicle| Some(vehicle.id) != last);
    let first = start
        .filter(|id| choices.iter().any(|vehicle| vehicle.id == *id))
        .or_else(|| choices.first().map(|vehicle| vehicle.id));
    let mut picked = use_signal(|| None::<AssetId>);
    let chosen = picked().or(first);
    if choices.is_empty() {
        return rsx! { p { class: "muted-note", {tr(messages::LOG_NO_VEHICLES)} } };
    }
    let kinds = choices
        .iter()
        .find(|vehicle| Some(vehicle.id) == chosen)
        .map(|vehicle| loggable_kinds(vehicle, mode))
        .unwrap_or_default();
    rsx! {
        label { class: "field",
            span { class: "field-label", {tr(messages::LOG_VEHICLE)} }
            select {
                class: "field-input",
                onchange: move |event| picked.set(event.value().parse().ok()),
                for vehicle in choices.clone() {
                    option {
                        key: "{vehicle.id}",
                        value: "{vehicle.id}",
                        selected: Some(vehicle.id) == chosen,
                        if Some(vehicle.id) == last {
                            {tr_with(messages::LOG_LAST_TIME, &[("name", MessageArg::from(vehicle.asset.name.as_str()))])}
                        } else {
                            {vehicle.asset.name.as_str().to_owned()}
                        }
                    }
                }
            }
        }
        div { class: "log-tiles",
            for kind in kinds {
                button {
                    key: "{event_value(kind)}",
                    class: "log-tile",
                    r#type: "button",
                    onclick: move |_| {
                        if let Some(asset) = chosen {
                            tracing::debug!(target: log_area::UI, %asset, kind = event_value(kind), "log form opened");
                            navigator.push(Route::LogForm {
                                asset_id: asset.to_string(),
                                kind: event_value(kind).to_owned(),
                            });
                            on_done.call(());
                        }
                    },
                    span { class: "log-tile-icon", if kind == EventKind::Fuel { "⛽" } else { "⏱" } }
                    span { class: "log-tile-text",
                        span { class: "log-tile-name", {tr(kind.label())} }
                        span { class: "log-tile-detail", {tr(kind_detail(kind))} }
                    }
                }
            }
        }
    }
}

/// `/log`: the chooser as a page (for a link straight to it).
#[component]
pub(crate) fn LogEvent() -> Element {
    rsx! {
        div { class: "log-chooser-page",
            ChooserBody { start: None, on_done: move |()| {} }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::access::Role;
    use domain::asset::{Asset, AssetKind, AssetName, LifecycleStatus};
    use domain::notes::Notes;
    use domain::test_support::test_id;

    use super::*;

    fn vehicle(kind: &str, role: Role) -> AssetSummary {
        AssetSummary {
            id: test_id(1),
            asset: Asset {
                kind: AssetKind::parse(kind).unwrap(),
                name: AssetName::parse("Thing").unwrap(),
                make: None,
                model: None,
                year: None,
                colour: None,
                description: Notes::default(),
                group: test_id(2),
                parent: None,
                status: LifecycleStatus::InService,
                identifiers: vec![],
            },
            role,
        }
    }

    #[test]
    fn what_can_be_logged_follows_the_kind_the_role_and_the_mode() {
        let both = vec![EventKind::Fuel, EventKind::Reading];
        assert_eq!(
            loggable_kinds(&vehicle("truck", Role::Driver), ActiveMode::Driver),
            both
        );
        assert_eq!(
            loggable_kinds(&vehicle("truck", Role::Admin), ActiveMode::Driver),
            both
        );
        assert!(loggable_kinds(&vehicle("truck", Role::Viewer), ActiveMode::Admin).is_empty());
        assert_eq!(
            loggable_kinds(&vehicle("trailer", Role::Driver), ActiveMode::Driver),
            vec![EventKind::Reading],
            "trailers have no fuel-ups"
        );
    }
}
