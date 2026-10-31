//! Users (Admin mode): the household's people, their roles, and their accounts.
//!
//! Option A of `docs/ui-sketches/p2m2-users-options.svg` (decision #046): a list on the left
//! and the chosen person's page on the right; on phones, the list, then a page per person.
//! The server makes every decision (permissions, the last Admin, temporary passwords); this
//! screen shows its answers.

use dioxus::prelude::*;
use domain::access::{ActiveMode, Role};
use domain::api::{AccountStatus, UserAdminOutcome, UserSummary};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;
use domain::user::UserId;

use crate::api;
use crate::ui::auth::{AuthState, use_auth};
use crate::ui::screens::problem_from_error;
use crate::ui::{tr, tr_with};

/// Every role, from least to most powerful, as the role menus list them.
const ROLES: [Role; 4] = [Role::Viewer, Role::Driver, Role::Maintainer, Role::Admin];

/// A role's label.
pub(crate) const fn role_label(role: Role) -> MessageId {
    match role {
        Role::Viewer => messages::ROLE_VIEWER,
        Role::Driver => messages::ROLE_DRIVER,
        Role::Maintainer => messages::ROLE_MAINTAINER,
        Role::Admin => messages::ROLE_ADMIN,
    }
}

/// A role's `<option>` value, and back.
const fn role_value(role: Option<Role>) -> &'static str {
    match role {
        None => "none",
        Some(Role::Viewer) => "viewer",
        Some(Role::Driver) => "driver",
        Some(Role::Maintainer) => "maintainer",
        Some(Role::Admin) => "admin",
    }
}

fn role_from_value(value: &str) -> Option<Role> {
    ROLES
        .into_iter()
        .find(|role| role_value(Some(*role)) == value)
}

/// The message for an answer that isn't the hoped-for one.
fn outcome_message(outcome: &UserAdminOutcome) -> Option<String> {
    match outcome {
        UserAdminOutcome::Users { .. }
        | UserAdminOutcome::Done
        | UserAdminOutcome::TemporaryPassword { .. } => None,
        UserAdminOutcome::NotAllowed => Some(tr(messages::USERS_NOT_ALLOWED)),
        UserAdminOutcome::LastAdmin => Some(tr(messages::USERS_LAST_ADMIN)),
        UserAdminOutcome::UsernameTaken => Some(tr(messages::USERS_USERNAME_TAKEN)),
        UserAdminOutcome::InvalidUsername { reason } => Some(tr(reason.message())),
        UserAdminOutcome::InvalidDisplayName { reason } => Some(tr(reason.message())),
        UserAdminOutcome::NotFound | UserAdminOutcome::Unavailable => {
            Some(tr(messages::SOMETHING_WENT_WRONG))
        }
    }
}

fn failure_message(error: &ServerFnError) -> String {
    tr(problem_from_error(error).message())
}

/// What's open on top of the screen.
#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    Add,
    ConfirmDisable(UserSummary),
    ConfirmReset(UserSummary),
    TemporaryPassword { name: String, password: String },
}

/// The Users screen.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Users() -> Element {
    let auth = use_auth();
    let in_admin_mode =
        matches!(auth(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin);
    let mut users = use_signal(Vec::<UserSummary>::new);
    let mut selected = use_signal(|| None::<UserId>);
    let mut error = use_signal(|| None::<String>);
    let mut dialog = use_signal(|| None::<Dialog>);
    let mut reloads = use_signal(|| 0_u32);

    // Loads after mounting (never during server rendering), again when asked, and when the
    // mode changes.
    use_effect(move || {
        let _ = reloads();
        let admin =
            matches!(auth(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin);
        if !admin {
            return;
        }
        spawn(async move {
            match api::list_users().await {
                Ok(UserAdminOutcome::Users { users: list }) => {
                    tracing::debug!(target: log_area::UI, count = list.len(), "users loaded");
                    users.set(list);
                }
                Ok(other) => error.set(outcome_message(&other)),
                Err(failure) => error.set(Some(failure_message(&failure))),
            }
        });
    });

    if !in_admin_mode {
        return rsx! { p { class: "placeholder", {tr(messages::USERS_NOT_ALLOWED)} } };
    }

    let list = users();
    let chosen = selected().and_then(|id| list.iter().find(|user| user.id == id).cloned());
    let layout = if chosen.is_some() {
        "users has-selection"
    } else {
        "users"
    };

    rsx! {
        div { class: layout,
            div { class: "users-list",
                button { class: "primary-button", onclick: move |_| dialog.set(Some(Dialog::Add)),
                    {tr(messages::USERS_ADD)}
                }
                for user in list {
                    UserRow {
                        key: "{user.id}",
                        selected: selected() == Some(user.id),
                        on_select: move |id| selected.set(Some(id)),
                        user,
                    }
                }
            }
            div { class: "users-detail",
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", {message} }
                }
                if let Some(user) = chosen {
                    UserDetail {
                        user,
                        on_back: move |()| selected.set(None),
                        on_changed: move |()| reloads += 1,
                        on_error: move |message| error.set(Some(message)),
                        on_dialog: move |next| dialog.set(Some(next)),
                    }
                } else {
                    p { class: "placeholder", {tr(messages::USERS_SELECT_HINT)} }
                }
            }
        }
        if let Some(open) = dialog() {
            UsersDialog {
                dialog: open,
                on_close: move |()| dialog.set(None),
                on_next: move |next| dialog.set(Some(next)),
                on_changed: move |()| reloads += 1,
            }
        }
    }
}

/// One row of the list.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn UserRow(user: UserSummary, selected: bool, on_select: EventHandler<UserId>) -> Element {
    let role = user.role_on_all.map_or_else(
        || tr(messages::ROLE_NONE),
        |role| {
            tr_with(
                messages::USERS_ROLE_SUMMARY,
                &[("role", MessageArg::from(tr(role_label(role))))],
            )
        },
    );
    let class = match (selected, user.status == AccountStatus::Disabled) {
        (true, _) => "user-row selected",
        (false, true) => "user-row disabled",
        (false, false) => "user-row",
    };
    rsx! {
        button { class, onclick: move |_| on_select.call(user.id),
            span { class: "user-row-text",
                strong { {user.display_name.clone()} }
                span { class: "user-row-role", {role} }
            }
            StatusBadge { user: user.clone() }
        }
    }
}

/// "You", "Disabled", or "Temporary password".
#[component]
fn StatusBadge(user: UserSummary) -> Element {
    let label = match user.status {
        AccountStatus::Disabled => Some(messages::USERS_DISABLED),
        AccountStatus::TemporaryPassword => Some(messages::USERS_TEMPORARY),
        AccountStatus::Active if user.is_you => Some(messages::USERS_YOU),
        AccountStatus::Active => None,
    };
    rsx! {
        if let Some(label) = label {
            span { class: "badge", {tr(label)} }
        }
    }
}

/// The chosen person's page: roles and account actions.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn UserDetail(
    user: UserSummary,
    on_back: EventHandler<()>,
    on_changed: EventHandler<()>,
    on_error: EventHandler<String>,
    on_dialog: EventHandler<Dialog>,
) -> Element {
    let id = user.id;
    let disabled = user.status == AccountStatus::Disabled;
    let set_role = move |role: Option<Role>| {
        spawn(async move {
            match api::set_user_role(id, role).await {
                Ok(UserAdminOutcome::Done) => on_changed.call(()),
                Ok(other) => on_error.call(outcome_message(&other).unwrap_or_default()),
                Err(failure) => on_error.call(failure_message(&failure)),
            }
        });
    };
    let enable = move |_| {
        spawn(async move {
            match api::set_user_disabled(id, false).await {
                Ok(UserAdminOutcome::Done) => on_changed.call(()),
                Ok(other) => on_error.call(outcome_message(&other).unwrap_or_default()),
                Err(failure) => on_error.call(failure_message(&failure)),
            }
        });
    };
    let for_disable = user.clone();
    let for_reset = user.clone();

    rsx! {
        button { class: "link-button users-back", onclick: move |_| on_back.call(()), {tr(messages::USERS_BACK)} }
        h2 { class: "user-name", {user.display_name.clone()} }
        p { class: "user-username", {user.username.as_str().to_owned()} }

        h3 { class: "section-heading", {tr(messages::USERS_ROLES_HEADING)} }
        div { class: "role-row",
            span { {tr(messages::USERS_ALL_VEHICLES)} }
            // Each option says whether it's selected: a `value` on the <select> itself can
            // arrive before its options exist, and the browser then shows the first one.
            select {
                class: "role-select",
                disabled: user.is_last_admin,
                onchange: move |event| set_role(role_from_value(&event.value())),
                option { value: "none", selected: user.role_on_all.is_none(), {tr(messages::ROLE_NONE)} }
                for role in ROLES {
                    option {
                        key: "{role:?}",
                        value: role_value(Some(role)),
                        selected: user.role_on_all == Some(role),
                        {tr(role_label(role))}
                    }
                }
            }
        }
        p { class: "muted-note", {tr(messages::USERS_ONE_VEHICLE_LATER)} }

        h3 { class: "section-heading", {tr(messages::USERS_ACCOUNT_HEADING)} }
        div { class: "account-actions",
            button { class: "secondary-button", onclick: move |_| on_dialog.call(Dialog::ConfirmReset(for_reset.clone())),
                {tr(messages::USERS_RESET_PASSWORD)}
            }
            if disabled {
                button { class: "secondary-button", onclick: enable, {tr(messages::USERS_ENABLE)} }
            } else if !user.is_you {
                button {
                    class: "danger-button",
                    disabled: user.is_last_admin,
                    onclick: move |_| on_dialog.call(Dialog::ConfirmDisable(for_disable.clone())),
                    {tr(messages::USERS_DISABLE)}
                }
            }
        }
        if user.is_last_admin {
            p { class: "muted-note", {tr(messages::USERS_LAST_ADMIN)} }
        } else if !disabled {
            p { class: "muted-note", {tr(messages::USERS_DISABLE_NOTE)} }
        }
    }
}

/// The add-user, confirm, and temporary-password dialogs.
#[component]
fn UsersDialog(
    dialog: Dialog,
    on_close: EventHandler<()>,
    on_next: EventHandler<Dialog>,
    on_changed: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "dialog", "aria-modal": "true",
                match dialog {
                    Dialog::Add => rsx! { AddUserForm { close: on_close, next: on_next, changed: on_changed } },
                    Dialog::ConfirmDisable(user) => rsx! { ConfirmDisable { user, on_close, on_changed } },
                    Dialog::ConfirmReset(user) => rsx! { ConfirmReset { user, on_close, on_next, on_changed } },
                    Dialog::TemporaryPassword { name, password } => rsx! { TemporaryPassword { name, password, on_close } },
                }
            }
        }
    }
}

/// "Add a user": username, name, and role on all vehicles.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn AddUserForm(
    close: EventHandler<()>,
    next: EventHandler<Dialog>,
    changed: EventHandler<()>,
) -> Element {
    let mut username = use_signal(String::new);
    let mut name = use_signal(String::new);
    let mut role = use_signal(|| Role::Driver);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            let result = api::add_user(username(), name(), role()).await;
            busy.set(false);
            match result {
                Ok(UserAdminOutcome::TemporaryPassword { password }) => {
                    changed.call(());
                    next.call(Dialog::TemporaryPassword {
                        name: name().trim().to_owned(),
                        password,
                    });
                }
                Ok(other) => error.set(outcome_message(&other)),
                Err(failure) => error.set(Some(failure_message(&failure))),
            }
        });
    };

    rsx! {
        form { class: "auth-form", onsubmit: submit,
            h2 { class: "dialog-title", {tr(messages::USERS_ADD_TITLE)} }
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_USERNAME)} }
                input { class: "field-input", autocomplete: "off", spellcheck: "false", autofocus: true, value: username(), oninput: move |event| username.set(event.value()) }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::FIELD_THEIR_NAME)} }
                input { class: "field-input", autocomplete: "off", value: name(), oninput: move |event| name.set(event.value()) }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::USERS_ROLE_ON_ALL)} }
                select {
                    class: "field-input",
                    onchange: move |event| {
                        if let Some(chosen) = role_from_value(&event.value()) {
                            role.set(chosen);
                        }
                    },
                    for each in ROLES {
                        option {
                            key: "{each:?}",
                            value: role_value(Some(each)),
                            selected: role() == each,
                            {tr(role_label(each))}
                        }
                    }
                }
            }
            p { class: "dialog-text", {tr(messages::USERS_ADD_NOTE)} }
            if let Some(message) = error() {
                p { class: "form-error", role: "alert", {message} }
            }
            div { class: "form-buttons",
                button { class: "secondary-button", r#type: "button", onclick: move |_| close.call(()), {tr(messages::ACTION_CANCEL)} }
                button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::USERS_ADD_BUTTON)} }
            }
        }
    }
}

/// "Disable Alex?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmDisable(
    user: UserSummary,
    on_close: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    let id = user.id;
    let confirm = move |_| {
        spawn(async move {
            match api::set_user_disabled(id, true).await {
                Ok(UserAdminOutcome::Done) => {
                    on_changed.call(());
                    on_close.call(());
                }
                Ok(other) => error.set(outcome_message(&other)),
                Err(failure) => error.set(Some(failure_message(&failure))),
            }
        });
    };
    rsx! {
        h2 { class: "dialog-title", {tr_with(messages::USERS_CONFIRM_DISABLE_TITLE, &[("name", MessageArg::from(user.display_name.as_str()))])} }
        p { class: "dialog-text", {tr(messages::USERS_CONFIRM_DISABLE_TEXT)} }
        if let Some(message) = error() {
            p { class: "form-error", role: "alert", {message} }
        }
        div { class: "form-buttons",
            button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
            button { class: "danger-button filled", onclick: confirm, {tr(messages::USERS_CONFIRM_DISABLE)} }
        }
    }
}

/// "Reset Alex's password?", then their new temporary password.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmReset(
    user: UserSummary,
    on_close: EventHandler<()>,
    on_next: EventHandler<Dialog>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    let id = user.id;
    let name = user.display_name.clone();
    let confirm = move |_| {
        let name = name.clone();
        spawn(async move {
            match api::reset_user_password(id).await {
                Ok(UserAdminOutcome::TemporaryPassword { password }) => {
                    on_changed.call(());
                    on_next.call(Dialog::TemporaryPassword { name, password });
                }
                Ok(other) => error.set(outcome_message(&other)),
                Err(failure) => error.set(Some(failure_message(&failure))),
            }
        });
    };
    rsx! {
        h2 { class: "dialog-title", {tr_with(messages::USERS_CONFIRM_RESET_TITLE, &[("name", MessageArg::from(user.display_name.as_str()))])} }
        p { class: "dialog-text", {tr(messages::USERS_CONFIRM_RESET_TEXT)} }
        if let Some(message) = error() {
            p { class: "form-error", role: "alert", {message} }
        }
        div { class: "form-buttons",
            button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
            button { class: "primary-button", onclick: confirm, {tr(messages::USERS_CONFIRM_RESET)} }
        }
    }
}

/// The temporary password, shown once.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn TemporaryPassword(name: String, password: String, on_close: EventHandler<()>) -> Element {
    let mut copied = use_signal(|| false);
    let to_copy = password.clone();
    rsx! {
        h2 { class: "dialog-title", {tr_with(messages::USERS_TEMP_TITLE, &[("name", MessageArg::from(name.as_str()))])} }
        p { class: "dialog-text", {tr(messages::USERS_TEMP_TEXT)} }
        p { class: "temporary-password", {password} }
        div { class: "form-buttons",
            button {
                class: "secondary-button",
                onclick: move |_| {
                    // The password goes to the clipboard as a JSON string literal, so no
                    // character in it can break out of the script.
                    let literal = serde_json::to_string(&to_copy).unwrap_or_default();
                    document::eval(&format!("navigator.clipboard.writeText({literal})"));
                    copied.set(true);
                },
                if copied() { {tr(messages::USERS_TEMP_COPIED)} } else { {tr(messages::USERS_TEMP_COPY)} }
            }
            button { class: "primary-button", onclick: move |_| on_close.call(()), {tr(messages::USERS_TEMP_DONE)} }
        }
        p { class: "muted-note", {tr(messages::USERS_TEMP_NOTE)} }
    }
}
