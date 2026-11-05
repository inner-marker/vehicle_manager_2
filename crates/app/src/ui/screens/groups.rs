//! Groups (the household's Admin, in Admin mode; decision #058, option B of
//! `docs/ui-sketches/p2m4-groups-roles-options.svg`).
//!
//! Laid out like Users: the groups on the left (icon, name, count; + Add group), the chosen
//! group on the right: its name and icon (saved when changed), its vehicles, who sees it and
//! why, and Delete (only for an empty group that isn't the last). On phones: the list, then
//! a page per group. Moving a vehicle stays on its own Setup tab (#057).

use dioxus::prelude::*;
use domain::access::{ActiveMode, GrantTarget, Role};
use domain::api::{AssetSummary, AssetsOutcome, GroupSummary, UserAdminOutcome, UserSummary};
use domain::asset_form::GroupForm;
use domain::group::{GroupIcon, GroupId};
use domain::i18n::{MessageArg, MessageId, messages};

use super::users::role_label;
use crate::api;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::icons::IconView;
use crate::ui::routes::Route;
use crate::ui::screens::problem_from_error;
use crate::ui::vehicles::{VehiclesContext, group_icon, refresh_garage, use_vehicles};
use crate::ui::{tr, tr_with};

/// A group icon's name.
const fn icon_label(icon: GroupIcon) -> MessageId {
    match icon {
        GroupIcon::Garage => messages::GROUP_ICON_GARAGE,
        GroupIcon::Hangar => messages::GROUP_ICON_HANGAR,
        GroupIcon::Boathouse => messages::GROUP_ICON_BOATHOUSE,
        GroupIcon::Storage => messages::GROUP_ICON_STORAGE,
        GroupIcon::Warehouse => messages::GROUP_ICON_WAREHOUSE,
        GroupIcon::Shop => messages::GROUP_ICON_SHOP,
        GroupIcon::Farm => messages::GROUP_ICON_FARM,
        GroupIcon::House => messages::GROUP_ICON_HOUSE,
    }
}

/// What went wrong, if the answer isn't Done.
fn failure(result: &Result<AssetsOutcome, ServerFnError>) -> Option<String> {
    match result {
        Ok(AssetsOutcome::Done) => None,
        Ok(AssetsOutcome::Invalid { problem }) => Some(tr(problem.message())),
        Ok(AssetsOutcome::NotAllowed) => Some(tr(messages::GROUPS_NOT_ALLOWED)),
        Ok(_) => Some(tr(messages::SOMETHING_WENT_WRONG)),
        Err(error) => Some(tr(problem_from_error(error).message())),
    }
}

/// One person's access to a group, in words, or `None` for no access: each role that
/// reaches it ("Viewer · all vehicles, Driver · Truck only").
fn access_to(user: &UserSummary, group: GroupId, vehicles: &[AssetSummary]) -> Option<String> {
    let role = |role: Role| MessageArg::from(tr(role_label(role)));
    let parts: Vec<String> = user
        .roles
        .iter()
        .filter_map(|(target, granted)| match target {
            GrantTarget::AllAssets => Some(tr_with(
                messages::GROUPS_FROM_ALL,
                &[("role", role(*granted))],
            )),
            GrantTarget::Group(id) if *id == group => Some(tr_with(
                messages::GROUPS_FROM_GROUP,
                &[("role", role(*granted))],
            )),
            GrantTarget::Asset(id) => vehicles.iter().find(|item| item.id == *id).map(|item| {
                tr_with(
                    messages::GROUPS_FROM_VEHICLE,
                    &[
                        ("role", role(*granted)),
                        ("vehicle", MessageArg::from(item.asset.name.as_str())),
                    ],
                )
            }),
            GrantTarget::Group(_) => None,
        })
        .collect();
    (!parts.is_empty()).then(|| parts.join(", "))
}

/// The Groups screen.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Groups() -> Element {
    let auth = use_auth();
    let context = use_vehicles();
    let in_admin_mode =
        matches!(auth(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin);
    let mut selected = use_signal(|| None::<GroupId>);
    let mut adding = use_signal(|| false);
    let mut users = use_signal(Vec::<UserSummary>::new);

    // Everyone's roles, for "who sees it" (after mounting, and when the mode changes).
    use_effect(move || {
        let admin =
            matches!(auth(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin);
        if admin {
            spawn(async move {
                if let Ok(UserAdminOutcome::Users { users: list }) = api::list_users().await {
                    users.set(list);
                }
            });
        }
    });

    if !in_admin_mode {
        return rsx! { p { class: "placeholder", {tr(messages::GROUPS_NOT_ALLOWED)} } };
    }
    let Some(view) = context.garage.read().clone() else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let chosen =
        selected().and_then(|id| view.groups.iter().find(|summary| summary.id == id).cloned());
    let layout = if chosen.is_some() {
        "users has-selection"
    } else {
        "users"
    };
    let last = view.groups.len() == 1;
    rsx! {
        div { class: layout,
            div { class: "users-list",
                button { class: "primary-button", onclick: move |_| adding.set(true), {tr(messages::GROUPS_ADD)} }
                for summary in view.groups.clone() {
                    GroupRow {
                        key: "{summary.id}",
                        selected: selected() == Some(summary.id),
                        vehicles: view.assets.iter().filter(|item| item.asset.group == summary.id).count(),
                        on_select: move |id| selected.set(Some(id)),
                        summary,
                    }
                }
            }
            div { class: "users-detail",
                if let Some(summary) = chosen {
                    GroupDetail {
                        key: "{summary.id}",
                        vehicles: view.assets.iter().filter(|item| item.asset.group == summary.id).cloned().collect::<Vec<_>>(),
                        users: users(),
                        last,
                        on_back: move |()| selected.set(None),
                        summary,
                    }
                } else {
                    p { class: "placeholder", {tr(messages::GROUPS_SELECT_HINT)} }
                }
            }
        }
        if adding() {
            AddGroupDialog { on_close: move |()| adding.set(false) }
        }
    }
}

/// One row of the list: icon, name, and how many vehicles.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn GroupRow(
    summary: GroupSummary,
    vehicles: usize,
    selected: bool,
    on_select: EventHandler<GroupId>,
) -> Element {
    let id = summary.id;
    let count = i64::try_from(vehicles).unwrap_or(i64::MAX);
    rsx! {
        button { class: if selected { "user-row selected" } else { "user-row" }, onclick: move |_| on_select.call(id),
            span { class: "group-row",
                IconView { icon: group_icon(summary.group.icon) }
                span { class: "user-row-text",
                    strong { {summary.group.name.as_str().to_owned()} }
                    span { class: "user-row-role", {tr_with(messages::VEHICLES_COUNT, &[("count", MessageArg::from(count))])} }
                }
            }
        }
    }
}

/// Saves a group's name and icon, then reloads the groups.
async fn save_group(
    context: VehiclesContext,
    id: GroupId,
    form: GroupForm,
    mut status: Signal<Option<Result<(), String>>>,
) {
    let result = api::edit_vehicle_group(id, form).await;
    match failure(&result) {
        None => {
            status.set(Some(Ok(())));
            let _ = refresh_garage(context).await;
        }
        Some(message) => status.set(Some(Err(message))),
    }
}

/// The chosen group: name and icon, vehicles, who sees it, and Delete.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn GroupDetail(
    summary: GroupSummary,
    vehicles: Vec<AssetSummary>,
    users: Vec<UserSummary>,
    last: bool,
    on_back: EventHandler<()>,
) -> Element {
    let context = use_vehicles();
    let id = summary.id;
    let mut name = use_signal(|| summary.group.name.as_str().to_owned());
    let mut icon = use_signal(|| summary.group.icon);
    let status = use_signal(|| None::<Result<(), String>>);
    let mut deleting = use_signal(|| false);
    let save = move || {
        let form = GroupForm {
            name: name(),
            icon: icon(),
        };
        spawn(save_group(context, id, form, status));
    };
    let can_delete = vehicles.is_empty() && !last;
    rsx! {
        button { class: "link-button users-back", onclick: move |_| on_back.call(()), {tr(messages::GROUPS_BACK)} }
        label { class: "field group-name-field",
            span { class: "field-label", {tr(messages::FIELD_NAME)} }
            input {
                class: "field-input",
                autocomplete: "off",
                value: name(),
                oninput: move |event| name.set(event.value()),
                // Saves when the field is left (or Enter is pressed).
                onchange: move |_| save(),
            }
        }
        IconPicker { chosen: icon(), on_pick: move |picked| { icon.set(picked); save(); } }
        match status() {
            Some(Ok(())) => rsx! { p { class: "muted-note", role: "status", {tr(messages::GROUPS_SAVED)} } },
            Some(Err(message)) => rsx! { p { class: "form-error", role: "alert", {message} } },
            None => rsx! {},
        }
        h3 { class: "section-heading", {tr(messages::GROUPS_VEHICLES).to_uppercase()} }
        if vehicles.is_empty() {
            p { class: "muted-note", {tr(messages::GROUPS_NO_VEHICLES)} }
        }
        for item in vehicles.iter() {
            div { key: "{item.id}", class: "group-line",
                span { {item.asset.name.as_str().to_owned()} }
                Link { class: "link-button", to: Route::VehicleDetail { asset_id: item.id.to_string() }, {tr(messages::GROUPS_OPEN)} }
            }
        }
        h3 { class: "section-heading", {tr(messages::GROUPS_WHO).to_uppercase()} }
        for user in users.iter() {
            div { key: "{user.id}", class: "group-line",
                span { {user.display_name.clone()} }
                match access_to(user, id, &vehicles) {
                    Some(access) => rsx! { span { class: "group-access", {access} } },
                    None => rsx! { span { class: "group-access none", {tr(messages::GROUPS_NO_ACCESS)} } },
                }
            }
        }
        div { class: "account-actions",
            button { class: "danger-button", disabled: !can_delete, onclick: move |_| deleting.set(true), {tr(messages::GROUPS_DELETE)} }
        }
        p { class: "muted-note", {tr(messages::GROUPS_DELETE_NOTE)} }
        p { class: "muted-note", {tr(messages::GROUPS_SAVES_NOTE)} }
        if deleting() {
            DeleteGroupDialog {
                id,
                name: summary.group.name.as_str().to_owned(),
                on_close: move |()| deleting.set(false),
                on_deleted: move |()| on_back.call(()),
            }
        }
    }
}

/// The eight icons, as large buttons; the chosen one is pressed.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn IconPicker(chosen: GroupIcon, on_pick: EventHandler<GroupIcon>) -> Element {
    rsx! {
        div { class: "field",
            span { class: "field-label", {tr(messages::GROUPS_ICON)} }
            div { class: "icon-picker", role: "group", "aria-label": tr(messages::GROUPS_ICON),
                for each in GroupIcon::ALL {
                    button {
                        key: "{each:?}",
                        r#type: "button",
                        class: if each == chosen { "icon-choice chosen" } else { "icon-choice" },
                        "aria-pressed": if each == chosen { "true" } else { "false" },
                        onclick: move |_| on_pick.call(each),
                        IconView { icon: group_icon(each) }
                        span { {tr(icon_label(each))} }
                    }
                }
            }
        }
    }
}

/// + Add group: a name and an icon.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn AddGroupDialog(on_close: EventHandler<()>) -> Element {
    let context = use_vehicles();
    let mut name = use_signal(String::new);
    let mut icon = use_signal(|| GroupIcon::Garage);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        busy.set(true);
        spawn(async move {
            let result = api::add_vehicle_group(GroupForm {
                name: name(),
                icon: icon(),
            })
            .await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                let _ = refresh_garage(context).await;
                on_close.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog wide-dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(messages::GROUPS_ADD_TITLE)} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::FIELD_NAME)} }
                    input { class: "field-input", autocomplete: "off", autofocus: true, value: name(), oninput: move |event| name.set(event.value()) }
                }
                IconPicker { chosen: icon(), on_pick: move |picked| icon.set(picked) }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", {message} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::ROLES_ADD_BUTTON)} }
                }
            }
        }
    }
}

/// "Delete Boathouse?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DeleteGroupDialog(
    id: GroupId,
    name: String,
    on_close: EventHandler<()>,
    on_deleted: EventHandler<()>,
) -> Element {
    let context = use_vehicles();
    let mut error = use_signal(|| None::<String>);
    let confirm = move |_| {
        spawn(async move {
            let result = api::delete_vehicle_group(id).await;
            error.set(failure(&result));
            if error().is_none() {
                let _ = refresh_garage(context).await;
                on_close.call(());
                on_deleted.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "alertdialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::GROUPS_DELETE_TITLE, &[("name", MessageArg::from(name))])} }
                p { class: "dialog-text", {tr(messages::GROUPS_DELETE_TEXT)} }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", {message} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button filled", onclick: confirm, {tr(messages::DELETE_DELETE)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::api::AccountStatus;
    use domain::asset::{Asset, AssetKind, AssetName, LifecycleStatus};
    use domain::notes::Notes;
    use domain::test_support::test_id;
    use domain::user::Username;

    use super::*;

    fn cub() -> AssetSummary {
        AssetSummary {
            id: test_id(1),
            asset: Asset {
                kind: AssetKind::parse("other-equipment").unwrap(),
                name: AssetName::parse("Cub").unwrap(),
                make: None,
                model: None,
                year: None,
                colour: None,
                description: Notes::default(),
                group: test_id(50),
                parent: None,
                status: LifecycleStatus::InService,
                identifiers: vec![],
            },
            role: Role::Admin,
            plate: None,
        }
    }

    fn person(roles: Vec<(GrantTarget, Role)>) -> UserSummary {
        UserSummary {
            id: test_id(2),
            username: Username::parse("alex").unwrap(),
            display_name: "Alex".into(),
            status: AccountStatus::Active,
            role_on_all: None,
            roles,
            is_you: false,
            is_last_admin: false,
        }
    }

    #[test]
    fn who_sees_a_group_says_where_each_role_comes_from() {
        let hangar = test_id(50);
        let vehicles = [cub()];
        let access = |roles| access_to(&person(roles), hangar, &vehicles);
        assert_eq!(access(vec![]), None);
        assert_eq!(
            access(vec![(GrantTarget::Group(test_id(51)), Role::Admin)]),
            None,
            "another group"
        );
        assert_eq!(
            access(vec![(GrantTarget::Asset(test_id(9)), Role::Admin)]),
            None,
            "a vehicle in another group"
        );
        assert_eq!(
            access(vec![
                (GrantTarget::AllAssets, Role::Viewer),
                (GrantTarget::Group(hangar), Role::Maintainer),
                (GrantTarget::Asset(test_id(1)), Role::Driver),
            ])
            .as_deref(),
            Some("Viewer · all vehicles, Maintainer · this group, Driver · Cub only")
        );
    }

    #[test]
    fn every_group_icon_has_a_name() {
        for icon in GroupIcon::ALL {
            assert!(
                domain::i18n::Translator::english().has(icon_label(icon)),
                "{icon:?}"
            );
        }
    }
}
