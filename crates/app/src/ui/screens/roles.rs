//! A person's roles on the Users screen (decision #058, option A of
//! `docs/ui-sketches/p2m4-groups-roles-options.svg`).
//!
//! A list of the roles they have, on **all vehicles**, **a group**, or **one vehicle**, each
//! with its role menu and ✕; **+ Add role** picks what it's on and the role. Roles add up:
//! on each vehicle, the highest applies. The server makes every decision (the last Admin, a
//! group or vehicle that's gone).

use dioxus::prelude::*;
use domain::access::{GrantTarget, Role};
use domain::api::{GarageView, UserAdminOutcome, UserSummary};
use domain::i18n::{MessageArg, messages};

use super::users::{
    ROLES, failure_message, outcome_message, role_from_value, role_label, role_value,
};
use crate::api;
use crate::ui::icons::{Icon, IconView};
use crate::ui::vehicles::{group_icon, use_vehicles};
use crate::ui::{tr, tr_with};

/// What a role is on, in words: "All vehicles", "Hangar · group", "Truck · vehicle".
pub(crate) fn target_text(target: GrantTarget, garage: Option<&GarageView>) -> String {
    let removed = || tr(messages::ROLES_REMOVED);
    match target {
        GrantTarget::AllAssets => tr(messages::USERS_ALL_VEHICLES),
        GrantTarget::Group(id) => {
            let name = garage
                .and_then(|view| view.groups.iter().find(|summary| summary.id == id))
                .map_or_else(removed, |summary| summary.group.name.as_str().to_owned());
            tr_with(
                messages::ROLES_ON_GROUP,
                &[("name", MessageArg::from(name))],
            )
        }
        GrantTarget::Asset(id) => {
            let name = garage
                .and_then(|view| view.assets.iter().find(|summary| summary.id == id))
                .map_or_else(removed, |summary| summary.asset.name.as_str().to_owned());
            tr_with(
                messages::ROLES_ON_VEHICLE,
                &[("name", MessageArg::from(name))],
            )
        }
    }
}

/// The name alone (for the list's summary): "all vehicles", "Hangar", "Truck".
fn target_name(target: GrantTarget, garage: Option<&GarageView>) -> String {
    match target {
        GrantTarget::AllAssets => Some(tr(messages::USERS_ALL_VEHICLES).to_lowercase()),
        GrantTarget::Group(id) => garage
            .and_then(|view| view.groups.iter().find(|summary| summary.id == id))
            .map(|summary| summary.group.name.as_str().to_owned()),
        GrantTarget::Asset(id) => garage
            .and_then(|view| view.assets.iter().find(|summary| summary.id == id))
            .map(|summary| summary.asset.name.as_str().to_owned()),
    }
    .unwrap_or_else(|| tr(messages::ROLES_REMOVED))
}

/// A role's icon: all vehicles, the group's own, or a vehicle.
fn target_icon(target: GrantTarget, garage: Option<&GarageView>) -> Icon {
    match target {
        GrantTarget::AllAssets => Icon::AllVehicles,
        GrantTarget::Group(id) => garage
            .and_then(|view| view.groups.iter().find(|summary| summary.id == id))
            .map_or(Icon::Groups, |summary| group_icon(summary.group.icon)),
        GrantTarget::Asset(_) => Icon::Vehicles,
    }
}

/// The list's one-line summary: "Admin · all vehicles", "Maintainer · Hangar", "3 roles".
pub(crate) fn roles_summary(user: &UserSummary, garage: Option<&GarageView>) -> String {
    match user.roles.as_slice() {
        [] => tr(messages::ROLE_NONE),
        [(GrantTarget::AllAssets, role)] => tr_with(
            messages::USERS_ROLE_SUMMARY,
            &[("role", MessageArg::from(tr(role_label(*role))))],
        ),
        [(target, role)] => tr_with(
            messages::USERS_ROLE_ON,
            &[
                ("role", MessageArg::from(tr(role_label(*role)))),
                ("target", MessageArg::from(target_name(*target, garage))),
            ],
        ),
        several => tr_with(
            messages::USERS_ROLES_COUNT,
            &[(
                "count",
                MessageArg::from(i64::try_from(several.len()).unwrap_or(i64::MAX)),
            )],
        ),
    }
}

/// A target's `<option>` value, and back.
fn target_value(target: GrantTarget) -> String {
    match target {
        GrantTarget::AllAssets => "all".to_owned(),
        GrantTarget::Group(id) => format!("group:{id}"),
        GrantTarget::Asset(id) => format!("asset:{id}"),
    }
}

fn target_from_value(value: &str) -> Option<GrantTarget> {
    if value == "all" {
        return Some(GrantTarget::AllAssets);
    }
    let (kind, id) = value.split_once(':')?;
    match kind {
        "group" => id.parse().ok().map(GrantTarget::Group),
        "asset" => id.parse().ok().map(GrantTarget::Asset),
        _ => None,
    }
}

/// Sets (or with `None`, removes) a role, then reports back.
fn save(
    user: domain::user::UserId,
    target: GrantTarget,
    role: Option<Role>,
    on_done: EventHandler<()>,
    on_error: EventHandler<String>,
) {
    spawn(async move {
        match api::set_user_role(user, target, role).await {
            Ok(UserAdminOutcome::Done) => on_done.call(()),
            Ok(other) => on_error.call(outcome_message(&other).unwrap_or_default()),
            Err(failure) => on_error.call(failure_message(&failure)),
        }
    });
}

/// The chosen person's roles, with + Add role.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn RolesSection(
    user: UserSummary,
    on_changed: EventHandler<()>,
    on_error: EventHandler<String>,
) -> Element {
    let context = use_vehicles();
    let mut adding = use_signal(|| false);
    let garage = context.garage.read().clone();
    let id = user.id;
    rsx! {
        div { class: "roles-heading",
            h3 { class: "section-heading", {tr(messages::USERS_ROLES_HEADING)} }
            button { class: "link-button", onclick: move |_| adding.set(true), {tr(messages::ROLES_ADD)} }
        }
        if user.roles.is_empty() {
            p { class: "muted-note", {tr(messages::ROLES_NONE)} }
        }
        for (target, role) in user.roles.clone() {
            div { key: "{target_value(target)}", class: "role-row",
                span { class: "role-target",
                    IconView { icon: target_icon(target, garage.as_ref()) }
                    {target_text(target, garage.as_ref())}
                }
                // The last Admin keeps the Admin role on all vehicles (the server refuses too).
                {
                    let locked = user.is_last_admin && target == GrantTarget::AllAssets;
                    rsx! {
                        select {
                            class: "role-select",
                            disabled: locked,
                            onchange: move |event| {
                                if let Some(chosen) = role_from_value(&event.value()) {
                                    save(id, target, Some(chosen), on_changed, on_error);
                                }
                            },
                            for each in ROLES {
                                option { key: "{each:?}", value: role_value(Some(each)), selected: role == each, {tr(role_label(each))} }
                            }
                        }
                        button {
                            class: "icon-button role-remove",
                            disabled: locked,
                            "aria-label": tr(messages::ROLES_REMOVE),
                            title: tr(messages::ROLES_REMOVE),
                            onclick: move |_| save(id, target, None, on_changed, on_error),
                            IconView { icon: Icon::Close }
                        }
                    }
                }
            }
        }
        p { class: "muted-note", {tr(messages::ROLES_NOTE)} }
        if adding() {
            AddRoleDialog { user, garage, on_close: move |()| adding.set(false), on_changed }
        }
    }
}

/// "Add a role for Alex": what it's on, and the role.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn AddRoleDialog(
    user: UserSummary,
    garage: Option<GarageView>,
    on_close: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut target = use_signal(|| GrantTarget::AllAssets);
    let mut role = use_signal(|| Role::Driver);
    let mut error = use_signal(|| None::<String>);
    let id = user.id;
    let (groups, assets) = garage
        .map(|view| (view.groups, view.assets))
        .unwrap_or_default();
    let add = move |event: FormEvent| {
        event.prevent_default();
        save(
            id,
            target(),
            Some(role()),
            EventHandler::new(move |()| {
                on_changed.call(());
                on_close.call(());
            }),
            EventHandler::new(move |message| error.set(Some(message))),
        );
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog", role: "dialog", "aria-modal": "true", onsubmit: add,
                h2 { class: "dialog-title", {tr_with(messages::ROLES_ADD_TITLE, &[("name", MessageArg::from(user.display_name.as_str()))])} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::ROLES_ON)} }
                    select {
                        class: "field-input",
                        onchange: move |event| {
                            if let Some(chosen) = target_from_value(&event.value()) {
                                target.set(chosen);
                            }
                        },
                        option { value: "all", selected: target() == GrantTarget::AllAssets, {tr(messages::USERS_ALL_VEHICLES)} }
                        optgroup { label: tr(messages::ROLES_GROUPS),
                            for summary in groups {
                                option {
                                    key: "{summary.id}",
                                    value: target_value(GrantTarget::Group(summary.id)),
                                    selected: target() == GrantTarget::Group(summary.id),
                                    {summary.group.name.as_str().to_owned()}
                                }
                            }
                        }
                        optgroup { label: tr(messages::ROLES_VEHICLES),
                            for summary in assets {
                                option {
                                    key: "{summary.id}",
                                    value: target_value(GrantTarget::Asset(summary.id)),
                                    selected: target() == GrantTarget::Asset(summary.id),
                                    {summary.asset.name.as_str().to_owned()}
                                }
                            }
                        }
                    }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::ROLES_ROLE)} }
                    select {
                        class: "field-input",
                        onchange: move |event| {
                            if let Some(chosen) = role_from_value(&event.value()) {
                                role.set(chosen);
                            }
                        },
                        for each in ROLES {
                            option { key: "{each:?}", value: role_value(Some(each)), selected: role() == each, {tr(role_label(each))} }
                        }
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", {message} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", {tr(messages::ROLES_ADD_BUTTON)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::access::Role;
    use domain::api::{AccountStatus, AssetSummary, GroupSummary};
    use domain::asset::{Asset, AssetKind, AssetName, LifecycleStatus};
    use domain::group::{Group, GroupIcon, GroupName};
    use domain::notes::Notes;
    use domain::preferences::GarageSort;
    use domain::test_support::test_id;
    use domain::user::Username;

    use super::*;

    fn garage() -> GarageView {
        GarageView {
            groups: vec![GroupSummary {
                id: test_id(50),
                group: Group {
                    name: GroupName::parse("Hangar").unwrap(),
                    icon: GroupIcon::Hangar,
                },
                vehicles: 1,
            }],
            assets: vec![AssetSummary {
                status: LifecycleStatus::InService,
                id: test_id(1),
                asset: Asset {
                    kind: AssetKind::parse("truck").unwrap(),
                    name: AssetName::parse("Truck").unwrap(),
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
            }],
            can_add: true,
            can_manage_groups: true,
            sort: GarageSort::NewestActivity,
            order: vec![],
        }
    }

    fn user(roles: Vec<(GrantTarget, Role)>) -> UserSummary {
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
    fn roles_read_like_the_sketch() {
        let view = garage();
        let hangar = GrantTarget::Group(test_id(50));
        let truck = GrantTarget::Asset(test_id(1));
        assert_eq!(
            target_text(GrantTarget::AllAssets, Some(&view)),
            "All vehicles"
        );
        assert_eq!(target_text(hangar, Some(&view)), "Hangar · group");
        assert_eq!(target_text(truck, Some(&view)), "Truck · vehicle");
        assert_eq!(
            target_text(GrantTarget::Asset(test_id(9)), Some(&view)),
            "(removed) · vehicle"
        );

        assert_eq!(roles_summary(&user(vec![]), Some(&view)), "No role");
        assert_eq!(
            roles_summary(
                &user(vec![(GrantTarget::AllAssets, Role::Admin)]),
                Some(&view)
            ),
            "Admin · all vehicles"
        );
        assert_eq!(
            roles_summary(&user(vec![(hangar, Role::Maintainer)]), Some(&view)),
            "Maintainer · Hangar"
        );
        assert_eq!(
            roles_summary(
                &user(vec![(hangar, Role::Maintainer), (truck, Role::Driver)]),
                Some(&view)
            ),
            "2 roles"
        );
    }

    #[test]
    fn targets_round_trip_through_the_menu() {
        for target in [
            GrantTarget::AllAssets,
            GrantTarget::Group(test_id(50)),
            GrantTarget::Asset(test_id(1)),
        ] {
            assert_eq!(target_from_value(&target_value(target)), Some(target));
        }
        assert_eq!(target_from_value("group:nope"), None);
        assert_eq!(target_from_value("other:1"), None);
    }
}
