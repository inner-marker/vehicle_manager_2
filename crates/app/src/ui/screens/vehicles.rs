//! All vehicles and a group's own page (decisions #052, #056).
//!
//! `docs/ui-sketches/p2m4-garage-chosen.svg`: cards in a grid, a section per group; a Sort
//! menu (Newest activity, Name, Custom with Arrange: ▲▼ on each card until Done); Show
//! archived; **+ Add vehicle** (the household's Admin, in Admin mode) as a two-step dialog,
//! kind then details. On phones, a menu at the top picks All vehicles or a group.
//!
//! **Pinned vehicles** (#090; `docs/ui-sketches/p6m3-vehicles-schedules-options.svg` part 1A):
//! each person's own, first in their group whatever the sort, with 📌 before the name. Pin and
//! Unpin are in each card's ⋯ menu, and on the card in Arrange mode.

use dioxus::prelude::*;
use domain::api::{AssetSummary, AssetsOutcome, GroupSummary};
use domain::asset::{AssetId, LifecycleStatus};
use domain::asset_form::AssetForm;
use domain::garage::{Direction, move_within, pinned_first, sort_assets, with_pin};
use domain::group::GroupId;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::preferences::{Choice, GarageSort, PreferencesPatch};
use domain::profile::{Profile, profile, profiles};
use domain::vehicle_type::VehicleTypeId;

use crate::ui::icons::{Icon, IconView};
use crate::ui::routes::Route;
use crate::ui::screens::{Field, FormError, problem_from_error};
use crate::ui::vehicles::{VehiclesContext, group_icon, refresh_garage, use_vehicles};
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// The sort choices, in menu order, with their labels.
const SORTS: [(GarageSort, &str, MessageId); 4] = [
    (
        GarageSort::MostUrgent,
        "most-urgent",
        messages::SORT_MOST_URGENT,
    ),
    (
        GarageSort::NewestActivity,
        "newest-activity",
        messages::SORT_NEWEST_ACTIVITY,
    ),
    (GarageSort::Name, "name", messages::SORT_NAME),
    (GarageSort::Custom, "custom", messages::SORT_CUSTOM),
];

/// A lifecycle status's label.
pub(crate) const fn status_label(status: LifecycleStatus) -> MessageId {
    match status {
        LifecycleStatus::InService => messages::STATUS_IN_SERVICE,
        LifecycleStatus::Spare => messages::STATUS_SPARE,
        LifecycleStatus::OutForService => messages::STATUS_OUT_FOR_SERVICE,
        LifecycleStatus::Retired => messages::STATUS_RETIRED,
        LifecycleStatus::Disposed => messages::STATUS_DISPOSED,
    }
}

/// A kind's name, from its profile (or its ID, for a kind this build doesn't know).
pub(crate) fn kind_name(kind: &domain::asset::AssetKind) -> String {
    profile(kind).map_or_else(
        || kind.as_str().to_owned(),
        |profile| Translator::english().tr_key(&profile.name),
    )
}

/// Every group's vehicles.
#[component]
pub(crate) fn AllVehicles() -> Element {
    rsx! { VehiclesScreen { group: None } }
}

/// One group's vehicles.
#[component]
pub(crate) fn GroupVehicles(group_id: String) -> Element {
    match group_id.parse::<GroupId>() {
        Ok(group) => rsx! { VehiclesScreen { group: Some(group) } },
        Err(_) => rsx! { p { class: "placeholder", {tr(messages::VEHICLES_EMPTY_GROUP)} } },
    }
}

/// How the person sorts the Garage: their sort, custom order, and pinned vehicles.
type Sorting<'a> = (GarageSort, &'a [AssetId], &'a [AssetId]);

/// The groups to show, each with its vehicles in the person's sort, pinned ones first.
fn sections(
    groups: &[GroupSummary],
    assets: &[AssetSummary],
    only: Option<GroupId>,
    show_archived: bool,
    (sort, order, pinned): Sorting<'_>,
) -> Vec<(GroupSummary, Vec<AssetSummary>)> {
    groups
        .iter()
        .filter(|summary| only.is_none_or(|group| group == summary.id))
        .map(|summary| {
            // This group's vehicles that are shown, in the person's sort.
            let mut in_group: Vec<AssetSummary> = assets
                .iter()
                .filter(|item| item.asset.group == summary.id)
                .filter(|item| show_archived || item.standing.status.shows_in_garage())
                .cloned()
                .collect();
            sort_assets(&mut in_group, sort, order);
            pinned_first(&mut in_group, pinned);
            (summary.clone(), in_group)
        })
        .collect()
}

/// All vehicles, or one group's.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn VehiclesScreen(group: Option<GroupId>) -> Element {
    let context = use_vehicles();
    let mut show_archived = use_signal(|| false);
    let arranging = use_signal(|| false);
    let mut adding = use_signal(|| false);
    let mut error = use_signal(|| None::<MessageId>);

    let Some(view) = context.garage.read().clone() else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let archived = view
        .assets
        .iter()
        .filter(|item| group.is_none_or(|id| item.asset.group == id))
        .filter(|item| !item.standing.status.shows_in_garage())
        .count();
    let shown = sections(
        &view.groups,
        &view.assets,
        group,
        show_archived(),
        (view.sort, &view.order, &view.pinned),
    );
    let on_move = move |(asset, direction): (AssetId, Direction)| {
        // Every shown vehicle in custom order (the order a move starts from, pins aside), and
        // the ones beside it as shown: in its group, pinned or not as it is.
        let everything = display_order(context, group, show_archived());
        let shown_here = shown_group_of(context, asset, show_archived());
        spawn(async move {
            let order = move_within(&everything, &shown_here, asset, direction);
            error.set(match crate::data::set_garage_order(order).await {
                Ok(AssetsOutcome::Done) => refresh_garage(context).await,
                Ok(_) => Some(messages::SOMETHING_WENT_WRONG),
                Err(failure) => Some(problem_from_error(&failure).message()),
            });
        });
    };
    let on_pin = move |(asset, pin): (AssetId, bool)| {
        let now = context
            .garage
            .read()
            .as_ref()
            .map(|view| view.pinned.clone())
            .unwrap_or_default();
        let pinned = with_pin(&now, asset, pin);
        spawn(async move {
            error.set(match crate::data::set_garage_pins(pinned).await {
                Ok(AssetsOutcome::Done) => refresh_garage(context).await,
                Ok(_) => Some(messages::SOMETHING_WENT_WRONG),
                Err(failure) => Some(problem_from_error(&failure).message()),
            });
        });
    };
    let start_group = group.or_else(|| view.groups.first().map(|summary| summary.id));
    let custom = view.sort == GarageSort::Custom;

    rsx! {
        div { class: "vehicles",
            GroupMenu { groups: view.groups.clone(), current: group }
            // Household-wide counts, so on All vehicles only (#083).
            if group.is_none() {
                super::due_list::FleetStrip { counts: view.fleet }
            }
            div { class: "vehicles-toolbar",
                if view.can_add {
                    button { class: "primary-button", onclick: move |_| adding.set(true), {tr(messages::VEHICLES_ADD)} }
                }
                SortMenu { sort: view.sort, error }
                if custom {
                    ArrangeButton { arranging }
                }
                if archived > 0 {
                    button {
                        class: "secondary-button archived-toggle",
                        onclick: move |_| show_archived.toggle(),
                        if show_archived() {
                            {tr(messages::VEHICLES_HIDE_ARCHIVED)}
                        } else {
                            {tr_with(messages::VEHICLES_SHOW_ARCHIVED, &[("count", MessageArg::from(i64::try_from(archived).unwrap_or(i64::MAX)))])}
                        }
                    }
                }
            }
            if let Some(problem) = error() {
                p { class: "form-error", role: "alert", {tr(problem)} }
            }
            if view.assets.is_empty() {
                p { class: "placeholder", {tr(messages::VEHICLES_EMPTY)} }
                if view.can_add {
                    p { class: "muted-note", {tr(messages::VEHICLES_EMPTY_ADMIN)} }
                }
            }
            for (summary, items) in shown {
                GroupSection {
                    key: "{summary.id}",
                    summary,
                    items,
                    pinned: view.pinned.clone(),
                    heading: group.is_none(),
                    arranging: custom && arranging(),
                    on_move,
                    on_pin,
                }
            }
        }
        if adding() {
            AddVehicleDialog {
                groups: view.groups,
                start_group,
                on_close: move |()| adding.set(false),
            }
        }
    }
}

/// The ids of every shown vehicle (one group's, or all), in display order without the pins
/// (so a ▲▼ move changes the custom order alone, and unpinning puts a vehicle back in place).
fn display_order(
    context: VehiclesContext,
    only: Option<GroupId>,
    show_archived: bool,
) -> Vec<AssetId> {
    let Some(view) = context.garage.read().clone() else {
        return Vec::new();
    };
    sections(
        &view.groups,
        &view.assets,
        only,
        show_archived,
        (view.sort, &view.order, &[]),
    )
    .into_iter()
    .flat_map(|(_, items)| items.into_iter().map(|item| item.id))
    .collect()
}

/// The ids of the shown vehicles beside `asset` (for a ▲▼ move): in its group, and pinned if
/// it is (or unpinned if it isn't), in display order.
fn shown_group_of(context: VehiclesContext, asset: AssetId, show_archived: bool) -> Vec<AssetId> {
    let Some((group, pinned)) = context.garage.read().as_ref().and_then(|view| {
        view.assets
            .iter()
            .find(|item| item.id == asset)
            .map(|item| (item.asset.group, view.pinned.clone()))
    }) else {
        return Vec::new();
    };
    let is_pinned = pinned.contains(&asset);
    display_order(context, Some(group), show_archived)
        .into_iter()
        .filter(|id| pinned.contains(id) == is_pinned)
        .collect()
}

/// The Sort menu: changing it saves the person's preference and reloads.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SortMenu(sort: GarageSort, error: Signal<Option<MessageId>>) -> Element {
    let context = use_vehicles();
    let mut error = error;
    rsx! {
        select {
            class: "role-select sort-menu",
            "aria-label": tr(messages::VEHICLES_SORT_LABEL),
            onchange: move |event| {
                let Some(chosen) = SORTS.iter().find(|(_, value, _)| *value == event.value()).map(|(sort, _, _)| *sort) else {
                    return;
                };
                spawn(async move {
                    let patch = PreferencesPatch {
                        garage_sort: Some(Choice::Set(chosen)),
                        ..PreferencesPatch::default()
                    };
                    error.set(match crate::data::set_my_preferences(patch).await {
                        Ok(_) => refresh_garage(context).await,
                        Err(failure) => Some(problem_from_error(&failure).message()),
                    });
                });
            },
            for (choice, value, label) in SORTS {
                option { key: "{value}", value, selected: choice == sort, {tr(label)} }
            }
        }
    }
}

/// Arrange / Done, for the Custom sort.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ArrangeButton(arranging: Signal<bool>) -> Element {
    let mut arranging = arranging;
    rsx! {
        button {
            class: if arranging() { "primary-button" } else { "secondary-button" },
            onclick: move |_| arranging.toggle(),
            if arranging() { {tr(messages::VEHICLES_ARRANGE_DONE)} } else { {tr(messages::VEHICLES_ARRANGE)} }
        }
    }
}

/// On phones: pick All vehicles or a group (the sidebar does this on wider screens).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn GroupMenu(groups: Vec<GroupSummary>, current: Option<GroupId>) -> Element {
    let navigator = use_navigator();
    if groups.len() < 2 {
        return rsx! {};
    }
    rsx! {
        select {
            class: "role-select group-menu",
            "aria-label": tr(messages::VEHICLES_GROUP_MENU),
            onchange: move |event| {
                let value = event.value();
                if value.is_empty() {
                    navigator.push(Route::AllVehicles {});
                } else {
                    navigator.push(Route::GroupVehicles { group_id: value });
                }
            },
            option { value: "", selected: current.is_none(), {tr(messages::NAV_ALL_VEHICLES)} }
            for summary in groups {
                option {
                    key: "{summary.id}",
                    value: "{summary.id}",
                    selected: current == Some(summary.id),
                    {summary.group.name.as_str().to_owned()}
                }
            }
        }
    }
}

/// One group: its heading (on All vehicles) and its cards.
#[component]
fn GroupSection(
    summary: GroupSummary,
    items: Vec<AssetSummary>,
    pinned: Vec<AssetId>,
    heading: bool,
    arranging: bool,
    on_move: EventHandler<(AssetId, Direction)>,
    on_pin: EventHandler<(AssetId, bool)>,
) -> Element {
    // What's shown (archived vehicles count only while they're shown).
    let count = i64::try_from(items.len()).unwrap_or(i64::MAX);
    rsx! {
        section { class: "vehicle-group",
            if heading {
                h2 { class: "vehicle-group-heading",
                    IconView { icon: group_icon(summary.group.icon) }
                    span { class: "vehicle-group-name", {summary.group.name.as_str().to_owned()} }
                    span { class: "vehicle-group-count", {tr_with(messages::VEHICLES_COUNT, &[("count", MessageArg::from(count))])} }
                }
            }
            if items.is_empty() {
                p { class: "muted-note", {tr(messages::VEHICLES_EMPTY_GROUP)} }
            }
            div { class: "vehicle-cards",
                for item in items {
                    VehicleCard {
                        key: "{item.id}",
                        pinned: pinned.contains(&item.id),
                        item,
                        arranging,
                        on_move,
                        on_pin,
                    }
                }
            }
        }
    }
}

/// One vehicle's card: name (📌 first when pinned), kind and model, an identifier; its ⋯ menu;
/// ▲▼ and Pin while arranging.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn VehicleCard(
    item: AssetSummary,
    pinned: bool,
    arranging: bool,
    on_move: EventHandler<(AssetId, Direction)>,
    on_pin: EventHandler<(AssetId, bool)>,
) -> Element {
    let asset = &item.asset;
    let id = item.id;
    let name = asset.name.as_str().to_owned();
    // "Truck · 2021 Ford F-150": the kind, then whichever of year, make, model there are.
    let model: Vec<String> = [
        asset.year.map(|year| year.to_string()),
        asset.make.clone(),
        asset.model.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    // A vehicle of a type says the type's name instead of its kind's (#090).
    let kind = item.vehicle_type.as_ref().map_or_else(
        || kind_name(&asset.kind),
        |label| label.name.as_str().to_owned(),
    );
    let subtitle = if model.is_empty() {
        kind
    } else {
        format!("{kind} · {}", model.join(" "))
    };
    // Its current plate if it has one, otherwise its first identifier.
    let identifier = item.plate.clone().or_else(|| {
        asset
            .identifiers
            .first()
            .map(|identifier| identifier.value.clone())
    });
    // The status it shows comes from its lifecycle events where they say (#067).
    let status = item.standing.status;
    let archived = !status.shows_in_garage();
    rsx! {
        div { class: if archived { "vehicle-card archived" } else { "vehicle-card" },
            Link { class: "vehicle-card-link", to: Route::VehicleDetail { asset_id: id.to_string() },
                // Its main photo at the left, or its kind's icon (option A, #24).
                super::attachments::VehiclePhoto { photo: asset.main_photo, icon: crate::ui::vehicles::vehicle_icon(&asset.kind, item.vehicle_type.as_ref()), class: "vehicle-photo-card" }
                span { class: "vehicle-card-name",
                    if pinned {
                        span { class: "vehicle-card-pin", role: "img", "aria-label": tr(messages::VEHICLES_PINNED), title: tr(messages::VEHICLES_PINNED), "📌" }
                    }
                    {name.clone()}
                }
                span { class: "vehicle-card-subtitle", {subtitle} }
                if let Some(value) = identifier {
                    span { class: "vehicle-card-identifier", {value} }
                }
                if archived {
                    span { class: "badge", {tr(status_label(status))} }
                } else if status == LifecycleStatus::OutForService {
                    span { class: "badge warn", {tr(status_label(status))} }
                }
                // Its most urgent step (3A, #082); nothing when all is OK.
                if let Some(step) = item.due.filter(|step| *step > domain::due::DueStep::Ok) {
                    super::due_view::DueChip { step }
                }
                // Its default usage's answer (2A, #086).
                if let Some((name, mark)) = item.ready.clone().filter(|_| !archived) {
                    super::readiness_view::ReadyChip { name, mark }
                }
            }
            if arranging {
                div { class: "vehicle-card-arrange",
                    button { class: "icon-button", "aria-label": tr(messages::VEHICLES_MOVE_UP), onclick: move |_| on_move.call((id, Direction::Up)),
                        IconView { icon: Icon::Up }
                    }
                    button { class: "icon-button", "aria-label": tr(messages::VEHICLES_MOVE_DOWN), onclick: move |_| on_move.call((id, Direction::Down)),
                        IconView { icon: Icon::Down }
                    }
                    button {
                        class: if pinned { "icon-button pin-button on" } else { "icon-button pin-button" },
                        "aria-label": tr(if pinned { messages::VEHICLES_UNPIN } else { messages::VEHICLES_PIN }),
                        "aria-pressed": pinned,
                        title: tr(if pinned { messages::VEHICLES_UNPIN } else { messages::VEHICLES_PIN }),
                        onclick: move |_| on_pin.call((id, !pinned)),
                        "📌"
                    }
                }
            } else {
                CardMenu { id, name, pinned, on_pin }
            }
        }
    }
}

/// A Garage card's ⋯ menu: Open, and Pin or Unpin (#090).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn CardMenu(
    id: AssetId,
    name: String,
    pinned: bool,
    on_pin: EventHandler<(AssetId, bool)>,
) -> Element {
    let mut open = use_signal(|| false);
    let navigator = use_navigator();
    rsx! {
        div { class: "chip-anchor vehicle-card-more",
            button {
                class: "vehicle-card-more-button",
                "aria-label": tr_with(messages::VEHICLES_CARD_MENU, &[("name", MessageArg::from(name.as_str()))]),
                "aria-haspopup": "menu",
                "aria-expanded": open(),
                onclick: move |_| open.toggle(),
                "⋯"
            }
            if open() {
                div { class: "menu-scrim", onclick: move |_| open.set(false) }
                div { class: "chip-menu", role: "menu",
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        onclick: move |_| {
                            open.set(false);
                            navigator.push(Route::VehicleDetail { asset_id: id.to_string() });
                        },
                        span { class: "menu-check" }
                        span { {tr(messages::VEHICLES_OPEN)} }
                    }
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        onclick: move |_| {
                            open.set(false);
                            on_pin.call((id, !pinned));
                        },
                        span { class: "menu-check", "📌" }
                        span { {tr(if pinned { messages::VEHICLES_UNPIN } else { messages::VEHICLES_PIN })} }
                    }
                }
            }
        }
    }
}

/// A household vehicle type picked when adding (#090): its ID, name, and what it starts with.
#[derive(Debug, Clone, PartialEq)]
struct PickedType {
    id: VehicleTypeId,
    name: String,
    summary: String,
}

/// + Add vehicle: the kind (or one of the household's types), then the details.
#[component]
fn AddVehicleDialog(
    groups: Vec<GroupSummary>,
    start_group: Option<GroupId>,
    on_close: EventHandler<()>,
) -> Element {
    let mut kind = use_signal(|| None::<(&'static Profile, Option<PickedType>)>);
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog add-vehicle", role: "dialog", "aria-modal": "true",
                match kind() {
                    None => rsx! {
                        KindStep { on_pick: move |picked| kind.set(Some(picked)), on_close }
                    },
                    Some((picked, vehicle_type)) => rsx! {
                        DetailsStep {
                            profile: picked,
                            vehicle_type,
                            groups: groups,
                            start_group,
                            on_back: move |()| kind.set(None),
                            on_close,
                        }
                    },
                }
            }
        }
    }
}

/// Step 1: what kind.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn KindStep(
    on_pick: EventHandler<(&'static Profile, Option<PickedType>)>,
    on_close: EventHandler<()>,
) -> Element {
    let kinds = profiles().unwrap_or_default();
    // The household's types, after the kinds (5A, #090); from the desktop's copy offline.
    let types = use_resource(|| async {
        match data::list_vehicle_types().await {
            Ok(domain::api::TypesOutcome::Types { view }) => view
                .types
                .iter()
                .filter_map(|(id, kept)| {
                    Some((
                        profile(&kept.kind)?,
                        kept.icon,
                        PickedType {
                            id: *id,
                            name: kept.name.as_str().to_owned(),
                            summary: super::vehicle_types::type_summary(kept, &view.templates),
                        },
                    ))
                })
                .collect(),
            _ => Vec::new(),
        }
    });
    let types: Vec<(&'static Profile, domain::vehicle_type::TypeIcon, PickedType)> =
        types.read().clone().unwrap_or_default();
    rsx! {
        p { class: "setup-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(1))])} }
        h2 { class: "dialog-title", {tr(messages::ADD_VEHICLE_TITLE)} }
        p { class: "dialog-text", {tr(messages::ADD_VEHICLE_KIND)} }
        div { class: "kind-tiles",
            for kind in kinds {
                button {
                    key: "{kind.id.as_str()}",
                    class: "kind-tile",
                    onclick: move |_| on_pick.call((kind, None)),
                    {Translator::english().tr_key(&kind.name)}
                }
            }
        }
        if !types.is_empty() {
            p { class: "dialog-text", {tr(messages::ADD_VEHICLE_YOUR_TYPES)} }
            div { class: "kind-tiles",
                for (kind, icon, picked) in types {
                    button {
                        key: "{picked.id}",
                        class: "kind-tile",
                        onclick: move |_| on_pick.call((kind, Some(picked.clone()))),
                        IconView { icon: crate::ui::vehicles::type_icon(icon) }
                        {picked.name.clone()}
                    }
                }
            }
        }
        div { class: "form-buttons",
            button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
        }
    }
}

/// Step 2: the details, for the chosen kind.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DetailsStep(
    profile: &'static Profile,
    vehicle_type: Option<PickedType>,
    groups: Vec<GroupSummary>,
    start_group: Option<GroupId>,
    on_back: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let context = use_vehicles();
    let name = use_signal(String::new);
    let make = use_signal(String::new);
    let model = use_signal(String::new);
    let year = use_signal(String::new);
    let colour = use_signal(String::new);
    let mut identifiers = use_signal(|| vec![String::new(); profile.identifiers.len()]);
    let mut group = use_signal(|| start_group);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    // Its type's template's rules left out (#090), shown before closing.
    let mut left_out = use_signal(|| None::<String>);
    let type_id = vehicle_type.as_ref().map(|picked| picked.id);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        let Some(chosen_group) = group() else {
            return;
        };
        let year_text = year();
        let year_value = if year_text.trim().is_empty() {
            None
        } else if let Ok(value) = year_text.trim().parse::<u16>() {
            Some(value)
        } else {
            error.set(Some(tr(messages::ASSET_YEAR_OUT_OF_RANGE)));
            return;
        };
        let form = AssetForm {
            name: name(),
            make: make(),
            model: model(),
            year: year_value,
            colour: colour(),
            identifiers: profile
                .identifiers
                .iter()
                .zip(identifiers())
                .map(|(kind, value)| (kind.kind.as_str().to_owned(), value))
                .collect(),
            vehicle_type: type_id,
            ..AssetForm::new(profile.id.as_str(), chosen_group)
        };
        busy.set(true);
        spawn(async move {
            let result = api::add_vehicle(form).await;
            busy.set(false);
            match result {
                Ok(AssetsOutcome::Added {
                    left_out: rules, ..
                }) => {
                    let _ = refresh_garage(context).await;
                    if rules.is_empty() {
                        on_close.call(());
                    } else {
                        let list: Vec<String> = rules
                            .iter()
                            .map(|(rule, meter)| format!("{}: {}", rule.as_str(), meter.as_str()))
                            .collect();
                        left_out.set(Some(tr_with(
                            messages::ADD_VEHICLE_LEFT_OUT,
                            &[("list", MessageArg::from(list.join(", ").as_str()))],
                        )));
                    }
                }
                Ok(AssetsOutcome::Invalid { problem }) => error.set(Some(tr(problem.message()))),
                Ok(AssetsOutcome::NotAllowed) => {
                    error.set(Some(tr(messages::VEHICLES_NOT_ALLOWED)));
                }
                Ok(_) => error.set(Some(tr(messages::SOMETHING_WENT_WRONG))),
                Err(failure) => error.set(Some(tr(problem_from_error(&failure).message()))),
            }
        });
    };

    if let Some(text) = left_out() {
        return rsx! {
            p { class: "form-note", role: "status", {text} }
            div { class: "form-buttons",
                button { class: "primary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CLOSE)} }
            }
        };
    }
    rsx! {
        form { class: "auth-form", onsubmit: submit,
            p { class: "setup-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(2))])} }
            h2 { class: "dialog-title",
                {vehicle_type.as_ref().map_or_else(|| Translator::english().tr_key(&profile.name), |picked| picked.name.clone())}
            }
            if let Some(picked) = vehicle_type.as_ref() {
                p { class: "muted-note", {picked.summary.clone()} }
            }
            Field { label: messages::ADD_VEHICLE_NAME, value: name, autofocus: true }
            div { class: "field-row",
                Field { label: messages::FIELD_MAKE, value: make }
                Field { label: messages::FIELD_MODEL, value: model }
            }
            div { class: "field-row",
                Field { label: messages::FIELD_YEAR, value: year, kind: "text" }
                Field { label: messages::FIELD_COLOUR, value: colour }
            }
            div { class: "field-row",
                for (index, kind) in profile.identifiers.iter().enumerate() {
                    label { key: "{kind.kind.as_str()}", class: "field",
                        span { class: "field-label", {Translator::english().tr_key(&kind.name)} }
                        input {
                            class: "field-input mono",
                            autocomplete: "off",
                            spellcheck: "false",
                            value: identifiers().get(index).cloned().unwrap_or_default(),
                            oninput: move |event| {
                                if let Some(slot) = identifiers.write().get_mut(index) {
                                    *slot = event.value();
                                }
                            },
                        }
                    }
                }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_GROUP)} }
                select {
                    class: "field-input",
                    onchange: move |event| group.set(event.value().parse().ok()),
                    for summary in groups {
                        option {
                            key: "{summary.id}",
                            value: "{summary.id}",
                            selected: group() == Some(summary.id),
                            {summary.group.name.as_str().to_owned()}
                        }
                    }
                }
            }
            p { class: "muted-note", {tr(messages::ADD_VEHICLE_NOTE)} }
            FormError { message: error() }
            div { class: "form-buttons",
                button { class: "secondary-button", r#type: "button", onclick: move |_| on_back.call(()), {tr(messages::ACTION_BACK)} }
                button { class: "primary-button", r#type: "submit", disabled: busy(),
                    if busy() { {tr(messages::ADD_VEHICLE_ADDING)} } else { {tr(messages::ADD_VEHICLE_ADD)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::access::Role;
    use domain::asset::{Asset, AssetKind, AssetName};
    use domain::group::{Group, GroupIcon, GroupName};
    use domain::notes::Notes;
    use domain::test_support::test_id;

    use super::*;

    fn group(n: u32, name: &str) -> GroupSummary {
        GroupSummary {
            id: test_id(n),
            group: Group {
                name: GroupName::parse(name).unwrap(),
                icon: GroupIcon::Garage,
            },
            vehicles: 0,
        }
    }

    fn vehicle(n: u32, name: &str, group: u32, status: LifecycleStatus) -> AssetSummary {
        AssetSummary {
            vehicle_type: None,
            due: None,
            ready: None,
            standing: domain::lifecycle::Standing {
                status,
                ..domain::lifecycle::Standing::default()
            },
            id: test_id(n),
            asset: Asset {
                vehicle_type: None,
                kind: AssetKind::parse("car").unwrap(),
                name: AssetName::parse(name).unwrap(),
                make: None,
                model: None,
                year: None,
                colour: None,
                description: Notes::default(),
                group: test_id(group),
                parent: None,
                status,
                identifiers: vec![],
                main_photo: None,
                in_service: None,
            },
            role: Role::Driver,
            plate: None,
        }
    }

    #[test]
    fn sections_follow_the_group_the_sort_and_the_archived_switch() {
        let groups = [group(50, "Garage"), group(51, "Hangar")];
        let assets = [
            vehicle(1, "Civic", 50, LifecycleStatus::InService),
            vehicle(2, "Truck", 50, LifecycleStatus::InService),
            vehicle(3, "Old", 50, LifecycleStatus::Retired),
            vehicle(4, "Cub", 51, LifecycleStatus::InService),
        ];
        let names = |sections: &[(GroupSummary, Vec<AssetSummary>)]| -> Vec<Vec<String>> {
            sections
                .iter()
                .map(|(_, items)| {
                    items
                        .iter()
                        .map(|item| item.asset.name.as_str().to_owned())
                        .collect()
                })
                .collect()
        };
        let all = sections(&groups, &assets, None, false, (GarageSort::Name, &[], &[]));
        assert_eq!(names(&all), vec![vec!["Civic", "Truck"], vec!["Cub"]]);
        let garage_only = sections(
            &groups,
            &assets,
            Some(test_id(50)),
            true,
            (GarageSort::Name, &[], &[]),
        );
        assert_eq!(names(&garage_only), vec![vec!["Civic", "Old", "Truck"]]);
    }

    #[test]
    fn every_status_and_sort_has_a_label() {
        for status in LifecycleStatus::ALL {
            assert!(
                Translator::english().has(status_label(status)),
                "{status:?}"
            );
        }
        assert_eq!(SORTS.len(), 4);
        for (_, _, label) in SORTS {
            assert!(Translator::english().has(label), "{label:?}");
        }
    }
}
