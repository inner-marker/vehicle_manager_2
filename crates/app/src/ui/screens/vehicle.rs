//! One vehicle's page (decisions #040, #051, #057).
//!
//! Option A of `docs/ui-sketches/p2m4-vehicle-options.svg`: a header (name, kind and model,
//! identifiers, group, status) and tabs across the top. **Overview** starts the per-vehicle
//! dashboard: its panels say when they fill, and the meters are listed. **Setup** holds the
//! details, meters, custom fields, required readings, the group, and the status (see
//! `vehicle_setup`). **Squawks** (milestone 7) lists them all, and the Overview's card the
//! outstanding ones (see `squawks`). **Maintenance** holds the schedule, the jobs, and setting
//! the schedule up (see `maintenance`). Documents says which milestone fills it. On phones the
//! tabs scroll sideways.

use dioxus::prelude::*;
use domain::api::{AssetView, AssetsOutcome};
use domain::asset::{Asset, AssetId, Identifier};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;
use domain::meter::{EventKind, Meter, MeterDimension};
use domain::profile::profile;

use super::attachments::AttachmentStrip;
use super::documents::DocumentsTab;
use super::maintenance::{MaintenanceTab, MaintenanceView};
use super::squawks::{SquawksCard, SquawksTab, VehicleSquawks, use_vehicle_squawks};
use super::timeline::{OverviewPanels, Shown, Timeline};
use super::vehicle_setup::Setup;
use super::vehicles::{kind_name, status_label};
use crate::api;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::icons::IconView;
use crate::ui::routes::Route;
use crate::ui::screens::problem_from_error;
use crate::ui::vehicles::{group_icon, use_vehicles};
use crate::ui::{tr, tr_with};
use domain::attachment::{AttachmentOwner, PhotoSize};

/// The page's state, in context for the Setup sections.
#[derive(Debug, Clone, Copy)]
pub(super) struct VehiclePage {
    /// The vehicle, once loaded.
    pub view: Signal<Option<AssetView>>,
    /// Why it couldn't be shown.
    pub problem: Signal<Option<MessageId>>,
    /// Bumped to load it again (after a change).
    reloads: Signal<u32>,
}

impl VehiclePage {
    /// Loads the vehicle again.
    pub(super) fn reload(mut self) {
        self.reloads += 1;
    }
}

/// The page's state, from context.
pub(super) fn use_vehicle_page() -> VehiclePage {
    use_context::<VehiclePage>()
}

/// The tabs, in order (#040).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Tab {
    Overview,
    Squawks,
    Timeline,
    Maintenance,
    Fuel,
    Documents,
    Setup,
}

impl Tab {
    /// Every tab, in order.
    const ALL: [Self; 7] = [
        Self::Overview,
        Self::Squawks,
        Self::Timeline,
        Self::Maintenance,
        Self::Fuel,
        Self::Documents,
        Self::Setup,
    ];

    /// Its name in an address (`/vehicles/:id/tab/fuel`).
    pub(super) const fn slug(self) -> &'static str {
        match self {
            Self::Overview => "overview",
            Self::Squawks => "squawks",
            Self::Timeline => "timeline",
            Self::Maintenance => "maintenance",
            Self::Fuel => "fuel",
            Self::Documents => "documents",
            Self::Setup => "setup",
        }
    }

    /// The tab an address names, and the Maintenance tab's view (`jobs` and `set-up` are
    /// the Maintenance tab too).
    fn from_slug(slug: &str) -> Option<(Self, MaintenanceView)> {
        if let Some(view) = MaintenanceView::from_slug(slug) {
            return Some((Self::Maintenance, view));
        }
        Self::ALL
            .into_iter()
            .find(|tab| tab.slug() == slug)
            .map(|tab| (tab, MaintenanceView::Schedule))
    }

    pub(super) const fn label(self) -> MessageId {
        match self {
            Self::Overview => messages::TAB_OVERVIEW,
            Self::Squawks => messages::TAB_SQUAWKS,
            Self::Timeline => messages::TAB_TIMELINE,
            Self::Maintenance => messages::TAB_MAINTENANCE,
            Self::Fuel => messages::TAB_FUEL,
            Self::Documents => messages::TAB_DOCUMENTS,
            Self::Setup => messages::TAB_SETUP,
        }
    }
}

/// Whether the vehicle's kind has fuel-ups (trailers don't).
pub(super) fn has_fuel(asset: &Asset) -> bool {
    profile(&asset.kind).is_none_or(|profile| profile.event_kinds.contains(&EventKind::Fuel))
}

/// The tabs shown for `asset`.
fn tabs(asset: &Asset) -> Vec<Tab> {
    [
        Tab::Overview,
        Tab::Squawks,
        Tab::Timeline,
        Tab::Maintenance,
        Tab::Fuel,
        Tab::Documents,
        Tab::Setup,
    ]
    .into_iter()
    .filter(|tab| *tab != Tab::Fuel || has_fuel(asset))
    .collect()
}

/// "Truck · 2021 Ford F-150 · Blue": the kind, then the year, make, and model, then the colour.
pub(super) fn subtitle(asset: &Asset) -> String {
    let model: Vec<String> = [
        asset.year.map(|year| year.to_string()),
        asset.make.clone(),
        asset.model.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let mut parts = vec![kind_name(&asset.kind)];
    if !model.is_empty() {
        parts.push(model.join(" "));
    }
    parts.extend(asset.colour.clone());
    parts.join(" · ")
}

/// An identifier as shown in the header: a plate on its own, others with their name.
fn identifier_text(asset: &Asset, identifier: &Identifier) -> String {
    if identifier.kind.as_str() == "plate" {
        return identifier.value.clone();
    }
    format!(
        "{} {}",
        identifier_name(asset, identifier.kind.as_str()),
        identifier.value
    )
}

/// An identifier kind's name, from the vehicle's profile (or the kind's ID).
pub(super) fn identifier_name(asset: &Asset, kind: &str) -> String {
    profile(&asset.kind)
        .and_then(|profile| {
            profile
                .identifiers
                .iter()
                .find(|offered| offered.kind.as_str() == kind)
        })
        .map_or_else(
            || kind.to_owned(),
            |offered| domain::i18n::Translator::english().tr_key(&offered.name),
        )
}

/// "Distance · miles · whole": what a meter measures, its unit, and its decimals.
pub(super) fn meter_summary(meter: &Meter) -> String {
    let dimension = match meter.dimension {
        MeterDimension::Distance => messages::DIMENSION_DISTANCE,
        MeterDimension::Duration => messages::DIMENSION_DURATION,
    };
    format!(
        "{} · {} · {}",
        tr(dimension),
        tr(meter.unit.label()).to_lowercase(),
        tr(precision_label(meter.precision))
    )
}

/// A number of decimal places, in words.
pub(super) const fn precision_label(precision: u8) -> MessageId {
    match precision {
        0 => messages::PRECISION_0,
        1 => messages::PRECISION_1,
        _ => messages::PRECISION_2,
    }
}

/// "Required for: Fuel-up, Maintenance", or that nothing requires it.
pub(super) fn required_text(meter: &Meter) -> String {
    if meter.required_for.is_empty() {
        return tr(messages::METER_REQUIRED_NONE);
    }
    let kinds: Vec<String> = meter
        .required_for
        .iter()
        .map(|kind| tr(kind.label()))
        .collect();
    tr_with(
        messages::METER_REQUIRED_FOR,
        &[("kinds", MessageArg::from(kinds.join(", ")))],
    )
}

/// The vehicle at `/vehicles/:asset_id`.
#[component]
pub(crate) fn VehicleDetail(asset_id: String) -> Element {
    match asset_id.parse::<AssetId>() {
        Ok(id) => {
            rsx! { VehicleScreen { id, start: Tab::Overview, maintenance_view: MaintenanceView::Schedule } }
        }
        Err(_) => rsx! { p { class: "placeholder", {tr(messages::VEHICLE_NOT_FOUND)} } },
    }
}

/// `/vehicles/:asset_id/tab/:tab`: the vehicle's page, opened on one tab (e.g. back from an
/// event to the Fuel tab).
#[component]
pub(crate) fn VehicleTab(asset_id: String, tab: String) -> Element {
    match (asset_id.parse::<AssetId>(), Tab::from_slug(&tab)) {
        (Ok(id), Some((start, maintenance_view))) => {
            rsx! { VehicleScreen { id, start, maintenance_view } }
        }
        _ => rsx! { p { class: "placeholder", {tr(messages::VEHICLE_NOT_FOUND)} } },
    }
}

/// Loads the vehicle into `page`.
async fn load(mut page: VehiclePage, id: AssetId) {
    let problem = match api::vehicle_page(id).await {
        Ok(AssetsOutcome::Asset { view }) => {
            tracing::debug!(target: log_area::UI, asset = %id, "vehicle loaded");
            page.view.set(Some(*view));
            None
        }
        Ok(AssetsOutcome::NotFound) => Some(messages::VEHICLE_NOT_FOUND),
        Ok(AssetsOutcome::NotAllowed) => Some(messages::VEHICLES_NOT_ALLOWED),
        Ok(other) => {
            tracing::debug!(target: log_area::UI, ?other, "vehicle: unexpected answer");
            Some(messages::SOMETHING_WENT_WRONG)
        }
        Err(error) => Some(problem_from_error(&error).message()),
    };
    if problem.is_some() {
        page.view.set(None);
    }
    page.problem.set(problem);
}

/// The page: header, tabs, and the chosen tab.
#[component]
fn VehicleScreen(id: AssetId, start: Tab, maintenance_view: MaintenanceView) -> Element {
    let auth = use_auth();
    let page = use_context_provider(|| VehiclePage {
        view: Signal::new(None),
        problem: Signal::new(None),
        reloads: Signal::new(0),
    });
    let mut tab = use_signal(|| start);
    let squawks = use_vehicle_squawks(id);
    // What the person may change depends on their mode, so a mode change loads it again.
    let mode = use_memo(move || match auth() {
        AuthState::SignedIn(session) => Some(session.mode),
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => None,
    });
    // Loads after mounting (never during server rendering), and for another vehicle.
    use_effect(use_reactive!(|id| {
        let _ = ((page.reloads)(), mode());
        spawn(load(page, id));
    }));
    // On the asked-for tab (Overview, unless the address names one) for another vehicle.
    use_effect(use_reactive!(|id, start| {
        let _ = id;
        tab.set(start);
    }));

    if let Some(problem) = (page.problem)() {
        return rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } };
    }
    let Some(view) = (page.view)().filter(|view| view.id == id) else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let shown = tabs(&view.asset);
    let current = if shown.contains(&tab()) {
        tab()
    } else {
        Tab::Overview
    };
    rsx! {
        div { class: "vehicle-page",
            Header { view: view.clone() }
            TabBar { shown, current, squawks: squawks().as_ref().map(VehicleSquawks::outstanding), on_pick: move |picked| tab.set(picked) }
            match current {
                Tab::Overview => rsx! {
                    Overview {
                        view,
                        squawks: squawks(),
                        on_timeline: move |()| tab.set(Tab::Timeline),
                        on_squawks: move |()| tab.set(Tab::Squawks),
                    }
                },
                Tab::Squawks => rsx! { SquawksTab { vehicle: view, squawks: squawks() } },
                Tab::Setup => rsx! { Setup { view } },
                Tab::Timeline => rsx! { Timeline { view, only: None } },
                Tab::Fuel => rsx! { Timeline { view, only: Some(Shown::FuelUps) } },
                Tab::Maintenance => rsx! { MaintenanceTab { vehicle: view, start: maintenance_view } },
                Tab::Documents => rsx! { DocumentsTab { vehicle: view } },
            }
        }
    }
}

/// The header: back to the group, name, kind and model, identifiers, group, and status.
#[component]
fn Header(view: AssetView) -> Element {
    let context = use_vehicles();
    let asset = &view.asset;
    let garage = context.garage.read().clone();
    let groups = garage.map(|garage| garage.groups).unwrap_or_default();
    let group = groups.iter().find(|summary| summary.id == asset.group);
    // With one group, All vehicles is that group (#056).
    let back = match (group, groups.len()) {
        (Some(summary), count) if count > 1 => rsx! {
            Link { class: "vehicle-back", to: Route::GroupVehicles { group_id: summary.id.to_string() },
                {tr_with(messages::VEHICLE_BACK_GROUP, &[("group", MessageArg::from(summary.group.name.as_str()))])}
            }
        },
        (Some(summary), _) => rsx! {
            Link { class: "vehicle-back", to: Route::AllVehicles {},
                {tr_with(messages::VEHICLE_BACK_GROUP, &[("group", MessageArg::from(summary.group.name.as_str()))])}
            }
        },
        (None, _) => rsx! {
            Link { class: "vehicle-back", to: Route::AllVehicles {}, {tr(messages::VEHICLE_BACK_ALL)} }
        },
    };
    rsx! {
        header { class: "vehicle-header",
            {back}
            h2 { class: "vehicle-name", {asset.name.as_str().to_owned()} }
            p { class: "vehicle-subtitle", {subtitle(asset)} }
            div { class: "vehicle-chips",
                if let Some(plate) = view.plate.clone() {
                    span { class: "vehicle-card-identifier vehicle-plate", {plate} }
                }
                for identifier in asset.identifiers.iter() {
                    span { key: "{identifier.kind.as_str()}", class: "vehicle-card-identifier", {identifier_text(asset, identifier)} }
                }
                if let Some(summary) = group {
                    span { class: "badge vehicle-group-badge",
                        IconView { icon: group_icon(summary.group.icon) }
                        {summary.group.name.as_str().to_owned()}
                    }
                }
                span { class: "badge", {tr(status_label(asset.status))} }
            }
        }
    }
}

/// The tabs; on phones they scroll sideways.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn TabBar(
    shown: Vec<Tab>,
    current: Tab,
    squawks: Option<usize>,
    on_pick: EventHandler<Tab>,
) -> Element {
    rsx! {
        div { class: "vehicle-tabs", role: "tablist", "aria-label": tr(messages::VEHICLE_TABS),
            for tab in shown {
                button {
                    key: "{tab:?}",
                    class: if tab == current { "vehicle-tab active" } else { "vehicle-tab" },
                    role: "tab",
                    "aria-selected": if tab == current { "true" } else { "false" },
                    onclick: move |_| on_pick.call(tab),
                    match (tab, squawks) {
                        (Tab::Squawks, Some(count)) if count > 0 => tr_with(
                            messages::TAB_SQUAWKS_COUNT,
                            &[("count", MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX)))],
                        ),
                        _ => tr(tab.label()),
                    }
                }
            }
        }
    }
}

/// Overview: the dashboard's panels (#051): the outstanding squawks first, then the ones that
/// fill later (each saying when), the latest events, and the meters.
#[component]
fn Overview(
    view: AssetView,
    squawks: Option<VehicleSquawks>,
    on_timeline: EventHandler<()>,
    on_squawks: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "vehicle-panels",
            SquawksCard { vehicle: view.clone(), squawks, on_all: on_squawks }
            Panel { title: messages::PANEL_READY, note: messages::PANEL_READY_COMING }
            Panel { title: messages::PANEL_DUE, note: messages::PANEL_DUE_COMING }
            section { class: "vehicle-panel vehicle-photos",
                AttachmentStrip {
                    asset: view.id,
                    owner: AttachmentOwner::Vehicle,
                    heading: messages::ATTACHMENT_JOB_PHOTOS,
                    default_size: PhotoSize::Medium,
                }
            }
            OverviewPanels { view, on_timeline }
        }
    }
}

/// One Overview panel that fills later.
#[component]
fn Panel(title: MessageId, note: MessageId) -> Element {
    rsx! {
        section { class: "vehicle-panel",
            h3 { class: "section-heading", {tr(title).to_uppercase()} }
            p { class: "muted-note", {tr(note)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::asset::{AssetKind, AssetName, IdentifierKind, LifecycleStatus};
    use domain::label::Label;
    use domain::meter::MeterUnit;
    use domain::notes::Notes;
    use domain::test_support::test_id;

    use super::*;

    fn asset(kind: &str) -> Asset {
        Asset {
            kind: AssetKind::parse(kind).unwrap(),
            name: AssetName::parse("Truck").unwrap(),
            make: Some("Ford".into()),
            model: Some("F-150".into()),
            year: Some(2021),
            colour: Some("Blue".into()),
            description: Notes::default(),
            group: test_id(50),
            parent: None,
            status: LifecycleStatus::InService,
            identifiers: vec![
                Identifier::new(IdentifierKind::parse("plate").unwrap(), "ABC 123").unwrap(),
                Identifier::new(IdentifierKind::parse("vin").unwrap(), "1FT").unwrap(),
            ],
        }
    }

    #[test]
    fn the_header_reads_like_the_sketch() {
        let truck = asset("truck");
        assert_eq!(subtitle(&truck), "Truck · 2021 Ford F-150 · Blue");
        assert_eq!(identifier_text(&truck, &truck.identifiers[0]), "ABC 123");
        assert_eq!(identifier_text(&truck, &truck.identifiers[1]), "VIN 1FT");
    }

    #[test]
    fn trailers_have_no_fuel_tab() {
        assert!(tabs(&asset("truck")).contains(&Tab::Fuel));
        assert!(!tabs(&asset("trailer")).contains(&Tab::Fuel));
        assert_eq!(tabs(&asset("trailer")).len(), 6);
    }

    #[test]
    fn meters_say_what_they_measure_and_need() {
        let meter = Meter {
            asset: test_id(1),
            name: Label::parse("Engine hours").unwrap(),
            dimension: MeterDimension::Duration,
            unit: MeterUnit::Hours,
            precision: 1,
            description: None,
            required_for: vec![EventKind::Maintenance, EventKind::Repair],
        };
        assert_eq!(meter_summary(&meter), "Duration · hours · tenths");
        assert_eq!(required_text(&meter), "Required for: Maintenance, Repair");
    }
}
