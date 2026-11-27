//! Manage → Roles (#091; sub-plan 05; option A of part 1 of
//! `docs/ui-sketches/p6m4-people-security-options.svg`): the built-in roles, read only, then
//! the household's own, each a name and ticked permissions by level. Laid out like Groups and
//! Vehicle types: the list, then the chosen role. Everyone sees them; the household's Admin, in
//! Admin mode, changes them, and a change reaches everyone holding the role at once.

use dioxus::prelude::*;
use domain::access::{AssetAction, Permissions, Role};
use domain::api::{RoleSummary, RolesOutcome, RolesView};
use domain::custom_role::{CustomRole, CustomRoleId, offered_permissions};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;

use super::problem_from_error;
use super::users::role_label;
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// A permission's name, or `None` for the Admin's own (never in a role's list).
pub(crate) const fn permission_label(action: AssetAction) -> Option<MessageId> {
    Some(match action {
        AssetAction::View => messages::PERMISSION_VIEW,
        AssetAction::LogFuel => messages::PERMISSION_LOG_FUEL,
        AssetAction::LogReading => messages::PERMISSION_LOG_READING,
        AssetAction::OpenSquawk => messages::PERMISSION_OPEN_SQUAWK,
        AssetAction::LogMaintenance => messages::PERMISSION_LOG_MAINTENANCE,
        AssetAction::DispositionSquawk => messages::PERMISSION_DISPOSITION_SQUAWK,
        AssetAction::ConfigureMaintenance => messages::PERMISSION_CONFIGURE,
        AssetAction::ManageEvents => messages::PERMISSION_MANAGE_EVENTS,
        AssetAction::KeepOriginals => messages::PERMISSION_KEEP_ORIGINALS,
        AssetAction::ManageDocuments => messages::PERMISSION_MANAGE_DOCUMENTS,
        AssetAction::SendOut => messages::PERMISSION_SEND_OUT,
        AssetAction::ManageAsset | AssetAction::OverrideReadings | AssetAction::RecordOwnership => {
            return None;
        }
    })
}

/// The permissions a role's page lists at `level` (Driver or Maintainer).
fn at_level(level: Role) -> Vec<AssetAction> {
    offered_permissions()
        .into_iter()
        .filter(|action| action.required_role() == level)
        .collect()
}

/// The modes a role's holders can pick, in words: "Driver", "Driver or Maintainer".
fn modes_text(permissions: Permissions) -> Option<String> {
    let highest = permissions.highest_level()?;
    let modes: Vec<String> = [Role::Driver, Role::Maintainer]
        .into_iter()
        .filter(|level| *level <= highest)
        .map(|level| tr(role_label(level)))
        .collect();
    Some(tr_with(
        messages::HOUSEHOLD_ROLES_MODES,
        &[("modes", MessageArg::from(modes.join(" or ")))],
    ))
}

/// "2 people".
fn holders_text(holders: usize) -> String {
    tr_with(
        messages::HOUSEHOLD_ROLES_HOLDERS,
        &[(
            "count",
            MessageArg::from(i64::try_from(holders).unwrap_or(i64::MAX)),
        )],
    )
}

/// The message for an answer that isn't the hoped-for one.
fn refusal(outcome: &RolesOutcome) -> MessageId {
    match outcome {
        RolesOutcome::Invalid { problem } => problem.message(),
        RolesOutcome::NotAllowed => messages::HOUSEHOLD_ROLES_READ_ONLY,
        RolesOutcome::NotFound => messages::HOUSEHOLD_ROLES_NOT_FOUND,
        RolesOutcome::Roles { .. }
        | RolesOutcome::Saved { .. }
        | RolesOutcome::Done
        | RolesOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// The built-in roles as the list shows them, most powerful first.
const BUILT_IN: [Role; 4] = [Role::Admin, Role::Maintainer, Role::Driver, Role::Viewer];

/// Which role is open on the right.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Chosen {
    /// A built-in one (read only).
    BuiltIn(Role),
    /// A new one.
    Adding,
    /// One of the household's.
    Kept(CustomRoleId),
}

/// The Roles screen.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Roles() -> Element {
    let mut reloads = use_signal(|| 0_u32);
    let mut chosen = use_signal(|| None::<Chosen>);
    let loaded = use_resource(move || async move {
        let _ = reloads();
        data::list_custom_roles().await
    });
    let view: RolesView = match &*loaded.read() {
        None => {
            return rsx! {
                p { class: "auth-loading", {tr(messages::LOADING)} }
            };
        }
        Some(Ok(RolesOutcome::Roles { view })) => view.clone(),
        Some(Ok(other)) => {
            return rsx! {
                p { class: "form-error", role: "alert", {tr(refusal(other))} }
            };
        }
        Some(Err(error)) => {
            return rsx! {
                p { class: "form-error", role: "alert", {tr(problem_from_error(error).message())} }
            };
        }
    };
    let layout = if chosen().is_some() {
        "users has-selection"
    } else {
        "users"
    };
    let row_class = move |this: Chosen| {
        if chosen() == Some(this) {
            "user-row selected"
        } else {
            "user-row"
        }
    };
    let open = chosen();
    rsx! {
        div { class: layout,
            div { class: "users-list",
                p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_INTRO)} }
                for role in BUILT_IN {
                    button {
                        key: "{role:?}",
                        class: row_class(Chosen::BuiltIn(role)),
                        onclick: move |_| chosen.set(Some(Chosen::BuiltIn(role))),
                        span { class: "user-row-text",
                            strong { {tr(role_label(role))} }
                            span { class: "user-row-role",
                                {
                                    tr(
                                        if role == Role::Admin {
                                            messages::HOUSEHOLD_ROLES_EVERYTHING
                                        } else {
                                            messages::HOUSEHOLD_ROLES_BUILT_IN
                                        },
                                    )
                                }
                            }
                        }
                    }
                }
                for summary in view.roles.clone() {
                    button {
                        key: "{summary.id}",
                        class: row_class(Chosen::Kept(summary.id)),
                        onclick: move |_| chosen.set(Some(Chosen::Kept(summary.id))),
                        span { class: "user-row-text",
                            strong { {summary.role.name.as_str().to_owned()} }
                            span { class: "user-row-role", {holders_text(summary.holders)} }
                        }
                    }
                }
                if view.may_manage {
                    button {
                        class: "primary-button",
                        onclick: move |_| chosen.set(Some(Chosen::Adding)),
                        {tr(messages::HOUSEHOLD_ROLES_ADD)}
                    }
                }
                p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_NOTE)} }
            }
            div { class: "users-detail",
                match open {
                    Some(Chosen::BuiltIn(role)) => rsx! {
                        BuiltInRole { key: "{role:?}", role, on_back: move |()| chosen.set(None) }
                    },
                    Some(Chosen::Adding) => rsx! {
                        RoleEditor {
                            id: None,
                            start_name: tr(messages::HOUSEHOLD_ROLES_NEW),
                            start_permissions: Permissions::NONE,
                            holders: 0,
                            may_manage: view.may_manage,
                            on_saved: move |saved: CustomRoleId| {
                                reloads += 1;
                                chosen.set(Some(Chosen::Kept(saved)));
                            },
                            on_removed: move |()| chosen.set(None),
                            on_back: move |()| chosen.set(None),
                        }
                    },
                    Some(Chosen::Kept(id)) => {
                        match view.roles.iter().find(|summary| summary.id == id).cloned() {
                            Some(RoleSummary { id, role, holders }) => rsx! {
                                RoleEditor {
                                    key: "{id}",
                                    id: Some(id),
                                    start_name: role.name.as_str().to_owned(),
                                    start_permissions: role.permissions,
                                    holders,
                                    may_manage: view.may_manage,
                                    on_saved: move |_| reloads += 1,
                                    on_removed: move |()| {
                                        reloads += 1;
                                        chosen.set(None);
                                    },
                                    on_back: move |()| chosen.set(None),
                                }
                            },
                            None => rsx! {
                                p { class: "placeholder", {tr(messages::HOUSEHOLD_ROLES_NOT_FOUND)} }
                            },
                        }
                    }
                    None => rsx! {
                        p { class: "placeholder", {tr(messages::HOUSEHOLD_ROLES_SELECT_HINT)} }
                    },
                }
            }
        }
    }
}

/// A role's permissions, by level, ticked; `on_toggle` is `None` when they can't be changed.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PermissionList(
    permissions: Permissions,
    on_toggle: Option<EventHandler<(AssetAction, bool)>>,
) -> Element {
    let levels = [
        (Role::Driver, messages::HOUSEHOLD_ROLES_DRIVER_LEVEL),
        (Role::Maintainer, messages::HOUSEHOLD_ROLES_MAINTAINER_LEVEL),
    ];
    rsx! {
        for (level, heading) in levels {
            h3 { key: "{level:?}", class: "section-heading", {tr(heading).to_uppercase()} }
            if level == Role::Driver {
                label { class: "check",
                    input { r#type: "checkbox", disabled: true, checked: true }
                    span { {tr(messages::PERMISSION_VIEW)} }
                }
            }
            for action in at_level(level) {
                label { key: "{action:?}", class: "check",
                    input {
                        r#type: "checkbox",
                        disabled: on_toggle.is_none(),
                        checked: permissions.contains(action),
                        onchange: move |event: FormEvent| {
                            if let Some(handler) = on_toggle {
                                handler.call((action, event.checked()));
                            }
                        },
                    }
                    span { {permission_label(action).map(tr).unwrap_or_default()} }
                }
            }
        }
    }
}

/// A built-in role: what it allows, read only.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn BuiltInRole(role: Role, on_back: EventHandler<()>) -> Element {
    let permissions = Permissions::of_role(role);
    rsx! {
        button {
            class: "link-button users-back",
            onclick: move |_| on_back.call(()),
            {tr(messages::HOUSEHOLD_ROLES_BACK)}
        }
        h2 { class: "user-name", {tr(role_label(role))} }
        if role == Role::Admin {
            p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_ADMIN_ALL)} }
        }
        PermissionList { permissions, on_toggle: None }
        if let Some(modes) = modes_text(permissions).filter(|_| role != Role::Admin) {
            p { class: "muted-note", {modes} }
        }
        p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_FIXED)} }
    }
}

/// One of the household's roles: its name and permissions, Save, and Delete (read only unless
/// the person may manage).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RoleEditor(
    id: Option<CustomRoleId>,
    start_name: String,
    start_permissions: Permissions,
    holders: usize,
    may_manage: bool,
    on_saved: EventHandler<CustomRoleId>,
    on_removed: EventHandler<()>,
    on_back: EventHandler<()>,
) -> Element {
    let mut name = use_signal(|| start_name.clone());
    let mut permissions = use_signal(|| start_permissions);
    let mut status = use_signal(|| None::<Result<(), String>>);
    let mut busy = use_signal(|| false);
    let mut deleting = use_signal(|| false);
    let save = move |event: FormEvent| {
        event.prevent_default();
        let Ok(label) = Label::parse(&name()) else {
            status.set(Some(Err(tr(messages::LABEL_EMPTY))));
            return;
        };
        let form = CustomRole {
            name: label,
            permissions: permissions(),
        };
        busy.set(true);
        spawn(async move {
            let result = match id {
                Some(id) => api::edit_custom_role(id, form).await,
                None => api::add_custom_role(form).await,
            };
            busy.set(false);
            match result {
                Ok(RolesOutcome::Saved { id }) => {
                    status.set(Some(Ok(())));
                    on_saved.call(id);
                }
                Ok(other) => status.set(Some(Err(tr(refusal(&other))))),
                Err(error) => status.set(Some(Err(tr(problem_from_error(&error).message())))),
            }
        });
    };
    let toggle = move |(action, on): (AssetAction, bool)| {
        let mut chosen = permissions().actions();
        chosen.retain(|each| *each != action);
        if on {
            chosen.push(action);
        }
        permissions.set(Permissions::of(&chosen));
        status.set(None);
    };
    rsx! {
        button {
            class: "link-button users-back",
            onclick: move |_| on_back.call(()),
            {tr(messages::HOUSEHOLD_ROLES_BACK)}
        }
        form { class: "type-editor", onsubmit: save,
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_NAME)} }
                input {
                    class: "field-input",
                    autocomplete: "off",
                    disabled: !may_manage,
                    value: name(),
                    oninput: move |event| name.set(event.value()),
                }
            }
            PermissionList {
                permissions: permissions(),
                on_toggle: may_manage.then_some(EventHandler::new(toggle)),
            }
            p { class: "muted-note",
                {
                    tr_with(
                        messages::HOUSEHOLD_ROLES_HELD_BY,
                        &[("people", MessageArg::from(holders_text(holders)))],
                    )
                }
                if let Some(modes) = modes_text(permissions()) {
                    " "
                    {modes}
                }
            }
            match status() {
                Some(Ok(())) => rsx! {
                    p { class: "muted-note", role: "status", {tr(messages::HOUSEHOLD_ROLES_SAVED)} }
                },
                Some(Err(message)) => rsx! {
                    p { class: "form-error", role: "alert", {message} }
                },
                None => rsx! {},
            }
            if may_manage {
                div { class: "form-buttons",
                    button {
                        class: "primary-button",
                        r#type: "submit",
                        disabled: busy(),
                        if busy() {
                            {tr(messages::ACTION_SAVING)}
                        } else {
                            {tr(messages::ACTION_SAVE)}
                        }
                    }
                }
                if id.is_some() {
                    div { class: "account-actions",
                        button {
                            class: "danger-button",
                            r#type: "button",
                            onclick: move |_| deleting.set(true),
                            {tr(messages::HOUSEHOLD_ROLES_DELETE)}
                        }
                    }
                    p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_DELETE_NOTE)} }
                }
            } else {
                p { class: "muted-note", {tr(messages::HOUSEHOLD_ROLES_READ_ONLY)} }
            }
        }
        if let (true, Some(id)) = (deleting(), id) {
            DeleteRoleDialog {
                id,
                name: start_name,
                on_removed,
                on_close: move |()| deleting.set(false),
            }
        }
    }
}

/// "Delete the role Chauffeur?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DeleteRoleDialog(
    id: CustomRoleId,
    name: String,
    on_removed: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    let mut problem = use_signal(|| None::<MessageId>);
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title",
                    {
                        tr_with(
                            messages::HOUSEHOLD_ROLES_DELETE_TITLE,
                            &[("name", MessageArg::from(name.as_str()))],
                        )
                    }
                }
                p { {tr(messages::HOUSEHOLD_ROLES_DELETE_NOTE)} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button {
                        class: "primary-button danger",
                        onclick: move |_| {
                            spawn(async move {
                                match api::delete_custom_role(id).await {
                                    Ok(RolesOutcome::Done) => {
                                        on_close.call(());
                                        on_removed.call(());
                                    }
                                    Ok(other) => problem.set(Some(refusal(&other))),
                                    Err(error) => problem.set(Some(problem_from_error(&error).message())),
                                }
                            });
                        },
                        {tr(messages::HOUSEHOLD_ROLES_DELETE)}
                    }
                    button {
                        class: "secondary-button",
                        onclick: move |_| on_close.call(()),
                        {tr(messages::ACTION_CANCEL)}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::i18n::Translator;

    use super::*;

    #[test]
    fn every_offered_permission_has_a_name_and_a_level() {
        let listed: Vec<AssetAction> = [Role::Driver, Role::Maintainer]
            .into_iter()
            .flat_map(at_level)
            .collect();
        assert_eq!(listed, offered_permissions());
        for action in AssetAction::ALL {
            match permission_label(action) {
                Some(id) => assert!(Translator::english().has(id), "{action:?}"),
                None => assert!(!action.customisable(), "{action:?}"),
            }
        }
        assert_eq!(at_level(Role::Driver).len(), 3);
    }

    #[test]
    fn a_role_says_which_modes_its_holders_pick() {
        let chauffeur = Permissions::of(&[AssetAction::View, AssetAction::LogFuel]);
        assert_eq!(
            modes_text(chauffeur).as_deref(),
            Some("Its holders pick Driver mode.")
        );
        let apprentice = Permissions::of(&[AssetAction::LogMaintenance]);
        assert_eq!(
            modes_text(apprentice).as_deref(),
            Some("Its holders pick Driver or Maintainer mode.")
        );
        assert_eq!(modes_text(Permissions::of(&[AssetAction::View])), None);
        assert_eq!(holders_text(1), "1 person");
        assert_eq!(holders_text(2), "2 people");
    }
}
