//! The signed-in user's controls in the shell (option A of
//! `docs/ui-sketches/p2m2-mode-options.svg`, decision #040):
//!
//! - the **mode chip** (`Driver ▾`): switch between the modes the user's roles allow;
//!   Admin asks for the password in a dialog first;
//! - the **account chip** (`Stuart ▾`): change password, sign out;
//! - the **Admin banner**: an orange strip with **Leave** while in Admin mode.
//!
//! The server decides; after every change the app asks it again (`auth::refresh`).

use dioxus::prelude::*;
use domain::access::ActiveMode;
use domain::api::SwitchModeOutcome;
use domain::i18n::{MessageId, messages};
use domain::log_area;

use super::auth::{AuthState, refresh, use_auth};
use super::screens::{ChangePasswordForm, problem_from_error};
use super::tr;
use crate::{api, client};

/// The label of a mode.
const fn mode_label(mode: ActiveMode) -> MessageId {
    match mode {
        ActiveMode::Driver => messages::MODE_DRIVER,
        ActiveMode::Maintainer => messages::MODE_MAINTAINER,
        ActiveMode::Admin => messages::MODE_ADMIN,
    }
}

/// Asks the server to switch mode, then refreshes the session. Returns the error to show,
/// if any (only Admin's password can be wrong).
async fn switch(mode: ActiveMode, password: Option<String>) -> Option<String> {
    match api::switch_mode(mode, password).await {
        Ok(SwitchModeOutcome::Switched { mode }) => {
            tracing::info!(target: log_area::AUTH, ?mode, "mode switched");
            None
        }
        Ok(SwitchModeOutcome::WrongPassword) => Some(tr(messages::WRONG_PASSWORD)),
        Ok(SwitchModeOutcome::NotAllowed | SwitchModeOutcome::Unavailable) => {
            Some(tr(messages::SOMETHING_WENT_WRONG))
        }
        Ok(SwitchModeOutcome::NotSignedIn) => Some(tr(messages::API_ERROR_MISSING_CREDENTIAL)),
        Err(error) => Some(tr(problem_from_error(&error).message())),
    }
}

/// The mode chip and its menu, plus the Admin password dialog.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ModeChip() -> Element {
    let state = use_auth();
    let mut menu_open = use_signal(|| false);
    let mut admin_dialog = use_signal(|| false);
    let AuthState::SignedIn(session) = state() else {
        return rsx! {};
    };
    if session.available_modes.is_empty() {
        // A Viewer-only user works read-only: nothing to switch.
        return rsx! {
            span { class: "chip chip-static", {tr(messages::MODE_READ_ONLY)} }
        };
    }
    let current = session.mode;
    let chip_class = if current == ActiveMode::Admin {
        "chip chip-admin"
    } else {
        "chip"
    };

    rsx! {
        div { class: "chip-anchor",
            button {
                class: chip_class,
                "aria-label": tr(messages::MODE_MENU_LABEL),
                "aria-expanded": menu_open(),
                onclick: move |_| menu_open.toggle(),
                {tr(mode_label(current))}
                span { class: "chip-caret", "▾" }
            }
            if menu_open() {
                div { class: "menu-scrim", onclick: move |_| menu_open.set(false) }
                div { class: "chip-menu", role: "menu",
                    for mode in session.available_modes {
                        button {
                            key: "{mode:?}",
                            class: if mode == current { "menu-item checked" } else { "menu-item" },
                            role: "menuitemradio",
                            "aria-checked": mode == current,
                            onclick: move |_| {
                                menu_open.set(false);
                                if mode == current {
                                    return;
                                }
                                if mode == ActiveMode::Admin {
                                    admin_dialog.set(true);
                                } else {
                                    spawn(async move {
                                        let _ = switch(mode, None).await;
                                        refresh(state).await;
                                    });
                                }
                            },
                            span { class: "menu-check", if mode == current { "✓" } }
                            span { {tr(mode_label(mode))} }
                            if mode == ActiveMode::Admin && current != ActiveMode::Admin {
                                span { class: "menu-hint", "🔒 " {tr(messages::MODE_NEEDS_PASSWORD)} }
                            }
                        }
                    }
                }
            }
            if admin_dialog() {
                AdminDialog { on_close: move |()| admin_dialog.set(false) }
            }
        }
    }
}

/// "Enter Admin mode": the password again.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn AdminDialog(on_close: EventHandler<()>) -> Element {
    let state = use_auth();
    let mut password = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            let problem = switch(ActiveMode::Admin, Some(password())).await;
            busy.set(false);
            password.set(String::new());
            match problem {
                None => {
                    refresh(state).await;
                    on_close.call(());
                }
                Some(message) => error.set(Some(message)),
            }
        });
    };

    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(messages::ADMIN_DIALOG_TITLE)} }
                p { class: "dialog-text", {tr(messages::ADMIN_DIALOG_TEXT)} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::FIELD_PASSWORD)} }
                    input {
                        class: "field-input",
                        r#type: "password",
                        autocomplete: "current-password",
                        autofocus: true,
                        value: password(),
                        oninput: move |event| password.set(event.value()),
                    }
                }
                if let Some(message) = error() {
                    p { class: "form-error", role: "alert", {message} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()),
                        {tr(messages::ACTION_CANCEL)}
                    }
                    button { class: "primary-button admin-button", r#type: "submit", disabled: busy(),
                        {tr(messages::ADMIN_DIALOG_CONFIRM)}
                    }
                }
            }
        }
    }
}

/// The account chip: the user's name; change password; sign out.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn AccountChip() -> Element {
    let state = use_auth();
    let mut menu_open = use_signal(|| false);
    let mut password_dialog = use_signal(|| false);
    let AuthState::SignedIn(session) = state() else {
        return rsx! {};
    };
    // The full name on wide screens; just its first letter on phones (CSS chooses).
    let initial: String = session.display_name.chars().take(1).collect();

    let sign_out = move |_| {
        menu_open.set(false);
        spawn(async move {
            if let Err(error) = api::sign_out().await {
                tracing::warn!(target: log_area::AUTH, %error, "sign-out request failed; forgetting the session anyway");
            }
            client::set_session_token(None);
            refresh(state).await;
        });
    };

    rsx! {
        div { class: "chip-anchor",
            button {
                class: "chip",
                "aria-label": tr(messages::ACCOUNT_MENU_LABEL),
                "aria-expanded": menu_open(),
                onclick: move |_| menu_open.toggle(),
                span { class: "chip-name-full", {session.display_name.clone()} }
                span { class: "chip-name-short", {initial} }
                span { class: "chip-caret", "▾" }
            }
            if menu_open() {
                div { class: "menu-scrim", onclick: move |_| menu_open.set(false) }
                div { class: "chip-menu", role: "menu",
                    p { class: "menu-heading", "{session.display_name} ({session.username.as_str()})" }
                    button {
                        class: "menu-item",
                        role: "menuitem",
                        onclick: move |_| {
                            menu_open.set(false);
                            password_dialog.set(true);
                        },
                        span { class: "menu-check" }
                        span { {tr(messages::ACCOUNT_CHANGE_PASSWORD)} }
                    }
                    button { class: "menu-item", role: "menuitem", onclick: sign_out,
                        span { class: "menu-check" }
                        span { {tr(messages::ACCOUNT_SIGN_OUT)} }
                    }
                }
            }
            if password_dialog() {
                div { class: "dialog-scrim",
                    div { class: "dialog", role: "dialog", "aria-modal": "true",
                        h2 { class: "dialog-title", {tr(messages::PASSWORD_DIALOG_TITLE)} }
                        ChangePasswordForm {
                            on_done: move |()| {
                                password_dialog.set(false);
                                spawn(refresh(state));
                            },
                            on_cancel: move |()| password_dialog.set(false),
                        }
                    }
                }
            }
        }
    }
}

/// The orange strip under the top bar in Admin mode, with **Leave**.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn AdminBanner() -> Element {
    let state = use_auth();
    let in_admin =
        matches!(state(), AuthState::SignedIn(session) if session.mode == ActiveMode::Admin);
    if !in_admin {
        return rsx! {};
    }
    rsx! {
        div { class: "admin-banner", role: "status",
            strong { class: "admin-banner-label", {tr(messages::ADMIN_BANNER_LABEL)} }
            span { class: "admin-banner-text", {tr(messages::ADMIN_BANNER_TEXT)} }
            button {
                class: "admin-banner-leave",
                onclick: move |_| {
                    spawn(async move {
                        let _ = switch(ActiveMode::Driver, None).await;
                        refresh(state).await;
                    });
                },
                {tr(messages::ADMIN_BANNER_LEAVE)}
            }
        }
    }
}

/// Whether the signed-in user is in Admin mode (the shell tints its sidebar edge).
pub(crate) fn in_admin_mode(state: &AuthState) -> bool {
    matches!(state, AuthState::SignedIn(session) if session.mode == ActiveMode::Admin)
}
