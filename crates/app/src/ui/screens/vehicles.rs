//! All vehicles and a group's own page (decisions #052, #056).
//!
//! `docs/ui-sketches/p2m4-garage-chosen.svg`: cards in a grid, a section per group; a Sort
//! menu (Newest activity, Name, Custom with Arrange: ▲▼ on each card until Done); Show
//! archived; **+ Add vehicle** (the household's Admin, in Admin mode) as a two-step dialog,
//! kind then details. On phones, a menu at the top picks All vehicles or a group.

use dioxus::prelude::*;
use domain::api::{AssetSummary, AssetsOutcome, GroupSummary};
use domain::asset::{AssetId, LifecycleStatus};
use domain::asset_form::AssetForm;
use domain::garage::{Direction, move_within, sort_assets};
use domain::group::GroupId;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::preferences::{Choice, GarageSort, PreferencesPatch};
use domain::profile::{Profile, profile, profiles};

use crate::api;
use crate::ui::icons::{Icon, IconView};
use crate::ui::routes::Route;
use crate::ui::screens::{Field, FormError, problem_from_error};
use crate::ui::vehicles::{VehiclesContext, group_icon, refresh_garage, use_vehicles};
use crate::ui::{tr, tr_with};

/// The sort choices, in menu order, with their labels.
const SORTS: [(GarageSort, &str, MessageId); 3] = [
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

/// The groups to show, each with its vehicles in the person's sort.
fn sections(
    groups: &[GroupSummary],
    assets: &[AssetSummary],
    only: Option<GroupId>,
    show_archived: bool,
    (sort, order): (GarageSort, &[AssetId]),
) -> Vec<(GroupSummary, Vec<AssetSummary>)> {
    groups
        .iter()
        .filter(|summary| only.is_none_or(|group| group == summary.id))
        .map(|summary| {
            // This group's vehicles that are shown, in the person's sort.
            let mut in_group: Vec<AssetSummary> = assets
                .iter()
                .filter(|item| item.asset.group == summary.id)
                .filter(|item| show_archived || item.asset.status.shows_in_garage())
                .cloned()
                .collect();
            sort_assets(&mut in_group, sort, order);
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
        .filter(|item| !item.asset.status.shows_in_garage())
        .count();
    let shown = sections(
        &view.groups,
        &view.assets,
        group,
        show_archived(),
        (view.sort, &view.order),
    );
    let on_move = move |(asset, direction): (AssetId, Direction)| {
        // Every shown vehicle in display order (the custom order a move starts from), and
        // the ones in the moved vehicle's group.
        let everything = display_order(context, group, show_archived());
        let shown_here = shown_group_of(context, asset, show_archived());
        spawn(async move {
            let order = move_within(&everything, &shown_here, asset, direction);
            error.set(match api::set_garage_order(order).await {
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
                    heading: group.is_none(),
                    arranging: custom && arranging(),
                    on_move,
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

/// The ids of every shown vehicle (one group's, or all), in display order.
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
        (view.sort, &view.order),
    )
    .into_iter()
    .flat_map(|(_, items)| items.into_iter().map(|item| item.id))
    .collect()
}

/// The ids of the shown vehicles in `asset`'s group, in display order (for a ▲▼ move).
fn shown_group_of(context: VehiclesContext, asset: AssetId, show_archived: bool) -> Vec<AssetId> {
    let group = context.garage.read().as_ref().and_then(|view| {
        view.assets
            .iter()
            .find(|item| item.id == asset)
            .map(|item| item.asset.group)
    });
    group.map_or_else(Vec::new, |group| {
        display_order(context, Some(group), show_archived)
    })
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
                    error.set(match api::set_my_preferences(patch).await {
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
    heading: bool,
    arranging: bool,
    on_move: EventHandler<(AssetId, Direction)>,
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
                    VehicleCard { key: "{item.id}", item, arranging, on_move }
                }
            }
        }
    }
}

/// One vehicle's card: name, kind and model, an identifier; ▲▼ while arranging.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn VehicleCard(
    item: AssetSummary,
    arranging: bool,
    on_move: EventHandler<(AssetId, Direction)>,
) -> Element {
    let asset = &item.asset;
    let id = item.id;
    // "Truck · 2021 Ford F-150": the kind, then whichever of year, make, model there are.
    let model: Vec<String> = [
        asset.year.map(|year| year.to_string()),
        asset.make.clone(),
        asset.model.clone(),
    ]
    .into_iter()
    .flatten()
    .collect();
    let subtitle = if model.is_empty() {
        kind_name(&asset.kind)
    } else {
        format!("{} · {}", kind_name(&asset.kind), model.join(" "))
    };
    // A plate if it has one, otherwise its first identifier.
    let identifier = asset
        .identifiers
        .iter()
        .find(|identifier| identifier.kind.as_str() == "plate")
        .or_else(|| asset.identifiers.first())
        .map(|identifier| identifier.value.clone());
    let archived = !asset.status.shows_in_garage();
    rsx! {
        div { class: if archived { "vehicle-card archived" } else { "vehicle-card" },
            Link { class: "vehicle-card-link", to: Route::VehicleDetail { asset_id: id.to_string() },
                span { class: "vehicle-card-name", {asset.name.as_str().to_owned()} }
                span { class: "vehicle-card-subtitle", {subtitle} }
                if let Some(value) = identifier {
                    span { class: "vehicle-card-identifier", {value} }
                }
                if archived {
                    span { class: "badge", {tr(status_label(asset.status))} }
                } else {
                    span { class: "vehicle-card-coming", {tr(messages::VEHICLES_CARD_COMING)} }
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
                }
            }
        }
    }
}

/// + Add vehicle: the kind, then the details.
#[component]
fn AddVehicleDialog(
    groups: Vec<GroupSummary>,
    start_group: Option<GroupId>,
    on_close: EventHandler<()>,
) -> Element {
    let mut kind = use_signal(|| None::<&'static Profile>);
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog add-vehicle", role: "dialog", "aria-modal": "true",
                match kind() {
                    None => rsx! {
                        KindStep { on_pick: move |picked| kind.set(Some(picked)), on_close }
                    },
                    Some(picked) => rsx! {
                        DetailsStep {
                            profile: picked,
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
fn KindStep(on_pick: EventHandler<&'static Profile>, on_close: EventHandler<()>) -> Element {
    let kinds = profiles().unwrap_or_default();
    rsx! {
        p { class: "setup-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(1))])} }
        h2 { class: "dialog-title", {tr(messages::ADD_VEHICLE_TITLE)} }
        p { class: "dialog-text", {tr(messages::ADD_VEHICLE_KIND)} }
        div { class: "kind-tiles",
            for kind in kinds {
                button {
                    key: "{kind.id.as_str()}",
                    class: "kind-tile",
                    onclick: move |_| on_pick.call(kind),
                    {Translator::english().tr_key(&kind.name)}
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
            ..AssetForm::new(profile.id.as_str(), chosen_group)
        };
        busy.set(true);
        spawn(async move {
            let result = api::add_vehicle(form).await;
            busy.set(false);
            match result {
                Ok(AssetsOutcome::Added { .. }) => {
                    let _ = refresh_garage(context).await;
                    on_close.call(());
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

    rsx! {
        form { class: "auth-form", onsubmit: submit,
            p { class: "setup-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(2))])} }
            h2 { class: "dialog-title", {Translator::english().tr_key(&profile.name)} }
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
            id: test_id(n),
            asset: Asset {
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
            },
            role: Role::Driver,
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
        let all = sections(&groups, &assets, None, false, (GarageSort::Name, &[]));
        assert_eq!(names(&all), vec![vec!["Civic", "Truck"], vec!["Cub"]]);
        let garage_only = sections(
            &groups,
            &assets,
            Some(test_id(50)),
            true,
            (GarageSort::Name, &[]),
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
        assert_eq!(SORTS.len(), 3);
    }
}
