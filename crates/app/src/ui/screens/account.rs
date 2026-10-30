//! Sign-in, first-Admin setup, and changing a password: the screens before the app, in the
//! chosen card layout (`docs/ui-sketches/p2m2-sign-in-chosen.svg`, decision #040).
//!
//! Every action is a server function called from an event handler, never during server
//! rendering (`crate::api`). The forms submit with Enter as well as the button.

use dioxus::prelude::*;
use domain::api::{
    ChangePasswordOutcome, CheckSetupCodeOutcome, RecoveryOutcome, SetupOutcome, SignInOutcome,
};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;

use crate::api;
use crate::client;
use crate::ui::auth::{AuthState, refresh, use_auth};
use crate::ui::screens::problem_from_error;
use crate::ui::{tr, tr_with};

/// "Too many attempts. Try again in N minutes.", rounding up so it never says 0.
fn too_many_attempts(retry_after_seconds: i64) -> String {
    let minutes = (retry_after_seconds + 59) / 60;
    tr_with(
        messages::TOO_MANY_ATTEMPTS,
        &[("minutes", MessageArg::from(minutes.max(1)))],
    )
}

/// The message for a server function that failed outright (unreachable, refused).
fn transport_error(error: &ServerFnError) -> String {
    tr(problem_from_error(error).message())
}

// --- Shared pieces -----------------------------------------------------------------------

/// A labelled input. `autocomplete` helps password managers (`username`, `current-password`,
/// `new-password`, or `off`).
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn Field(
    label: MessageId,
    value: Signal<String>,
    #[props(default = "text")] kind: &'static str,
    #[props(default = "off")] autocomplete: &'static str,
    #[props(default)] mono: bool,
    #[props(default)] autofocus: bool,
) -> Element {
    let mut value = value;
    rsx! {
        label { class: "field",
            span { class: "field-label", {tr(label)} }
            input {
                class: if mono { "field-input mono" } else { "field-input" },
                r#type: kind,
                autocomplete,
                autofocus,
                spellcheck: "false",
                value: value(),
                oninput: move |event| value.set(event.value()),
            }
        }
    }
}

/// A form error, above the button.
#[component]
fn FormError(message: Option<String>) -> Element {
    rsx! {
        if let Some(message) = message {
            p { class: "form-error", role: "alert", {message} }
        }
    }
}

/// The plain page with one centred card.
#[component]
fn AuthCard(children: Element) -> Element {
    rsx! {
        div { class: "auth-page",
            div { class: "auth-card", {children} }
        }
    }
}

// --- Sign in -----------------------------------------------------------------------------

/// Where the sign-in card is: signing in, asking for recovery, or entering a recovery code.
#[derive(Debug, Clone, PartialEq, Eq)]
enum SignInStage {
    SignIn,
    Forgot,
    Recover { username: String },
}

/// The sign-in screen, and password recovery (`docs/ui-sketches/p2m2-recovery.svg`, #047).
#[component]
pub(crate) fn SignIn() -> Element {
    let mut stage = use_signal(|| SignInStage::SignIn);
    rsx! {
        match stage() {
            SignInStage::SignIn => rsx! { SignInForm { on_forgot: move |()| stage.set(SignInStage::Forgot) } },
            SignInStage::Forgot => rsx! {
                ForgotPassword {
                    on_asked: move |username| stage.set(SignInStage::Recover { username }),
                    on_back: move |()| stage.set(SignInStage::SignIn),
                }
            },
            SignInStage::Recover { username } => rsx! {
                RecoverWithCode { username, on_back: move |()| stage.set(SignInStage::SignIn) }
            },
        }
    }
}

/// "Forgot your password?": the username. Everyone then gets the same answer.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ForgotPassword(on_asked: EventHandler<String>, on_back: EventHandler<()>) -> Element {
    let username = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            // The answer is the same whatever happens, so failures aren't shown either.
            if let Err(error) = api::request_recovery(username()).await {
                tracing::debug!(target: log_area::AUTH, %error, "recovery request failed");
            }
            busy.set(false);
            on_asked.call(username());
        });
    };
    rsx! {
        AuthCard {
            form { class: "auth-form", onsubmit: submit,
                h1 { class: "auth-title", {tr(messages::RECOVERY_TITLE)} }
                p { class: "auth-subtitle", {tr(messages::RECOVERY_SUBTITLE)} }
                Field { label: messages::FIELD_USERNAME, value: username, autocomplete: "username", autofocus: true }
                button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::RECOVERY_CONTINUE)} }
                button { class: "link-button", r#type: "button", onclick: move |_| on_back.call(()), {tr(messages::RECOVERY_BACK)} }
            }
        }
    }
}

/// "Ask your Admin for further instructions", and the form for a recovery code.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RecoverWithCode(username: String, on_back: EventHandler<()>) -> Element {
    let mut state = use_auth();
    let code = use_signal(String::new);
    let password = use_signal(String::new);
    let again = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        if password() != again() {
            error.set(Some(tr(messages::PASSWORDS_DIFFER)));
            return;
        }
        busy.set(true);
        let username = username.clone();
        spawn(async move {
            let result =
                api::complete_recovery(username, code(), password(), client::USES_SESSION_TOKEN)
                    .await;
            busy.set(false);
            match result {
                Ok(RecoveryOutcome::SignedIn { session, token }) => {
                    tracing::info!(target: log_area::AUTH, "password recovered; signed in");
                    if token.is_some() {
                        client::set_session_token(token);
                    }
                    state.set(AuthState::SignedIn(session));
                }
                Ok(RecoveryOutcome::WrongCode) => {
                    error.set(Some(tr(messages::RECOVERY_WRONG_CODE)));
                }
                Ok(RecoveryOutcome::TooManyAttempts {
                    retry_after_seconds,
                }) => {
                    error.set(Some(too_many_attempts(retry_after_seconds)));
                }
                Ok(RecoveryOutcome::InvalidPassword { reason }) => {
                    error.set(Some(tr(reason.message())));
                }
                Ok(RecoveryOutcome::Unavailable) => {
                    error.set(Some(tr(messages::SOMETHING_WENT_WRONG)));
                }
                Err(failure) => error.set(Some(transport_error(&failure))),
            }
        });
    };
    rsx! {
        AuthCard {
            form { class: "auth-form", onsubmit: submit,
                h1 { class: "auth-title", {tr(messages::RECOVERY_ASK_ADMIN_TITLE)} }
                p { class: "auth-subtitle", {tr(messages::RECOVERY_ASK_ADMIN_TEXT)} }
                p { class: "auth-step muted", {tr(messages::RECOVERY_HAVE_CODE)} }
                Field { label: messages::FIELD_RECOVERY_CODE, value: code, mono: true }
                Field { label: messages::FIELD_NEW_PASSWORD_LONG, value: password, kind: "password", autocomplete: "new-password" }
                Field { label: messages::FIELD_NEW_PASSWORD_AGAIN, value: again, kind: "password", autocomplete: "new-password" }
                FormError { message: error() }
                button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::RECOVERY_SUBMIT)} }
                button { class: "link-button", r#type: "button", onclick: move |_| on_back.call(()), {tr(messages::RECOVERY_BACK)} }
            }
        }
    }
}

/// The sign-in form, with the "Forgot your password?" link.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SignInForm(on_forgot: EventHandler<()>) -> Element {
    let mut state = use_auth();
    let username = use_signal(String::new);
    let mut password = use_signal(String::new);
    let mut busy = use_signal(|| false);
    let mut error = use_signal(|| match state() {
        AuthState::Unreachable(message) => Some(tr(message)),
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::SignedIn(_) => None,
    });

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            let result = api::sign_in(username(), password(), client::USES_SESSION_TOKEN).await;
            busy.set(false);
            match result {
                Ok(SignInOutcome::SignedIn { session, token }) => {
                    tracing::info!(target: log_area::AUTH, "signed in");
                    if token.is_some() {
                        client::set_session_token(token);
                    }
                    state.set(AuthState::SignedIn(session));
                }
                Ok(SignInOutcome::InvalidCredentials) => {
                    password.set(String::new());
                    error.set(Some(tr(messages::SIGN_IN_WRONG)));
                }
                Ok(SignInOutcome::TooManyAttempts {
                    retry_after_seconds,
                }) => {
                    error.set(Some(too_many_attempts(retry_after_seconds)));
                }
                Ok(SignInOutcome::Unavailable) => {
                    error.set(Some(tr(messages::SIGN_IN_UNAVAILABLE)));
                }
                Err(failure) => error.set(Some(transport_error(&failure))),
            }
        });
    };

    rsx! {
        AuthCard {
            form { class: "auth-form", onsubmit: submit,
                h1 { class: "auth-title", {tr(messages::SIGN_IN_TITLE)} }
                p { class: "auth-subtitle", {tr(messages::SIGN_IN_SUBTITLE)} }
                Field { label: messages::FIELD_USERNAME, value: username, autocomplete: "username", autofocus: true }
                Field { label: messages::FIELD_PASSWORD, value: password, kind: "password", autocomplete: "current-password" }
                FormError { message: error() }
                button { class: "primary-button", r#type: "submit", disabled: busy(),
                    if busy() { {tr(messages::SIGN_IN_BUSY)} } else { {tr(messages::SIGN_IN_BUTTON)} }
                }
                button { class: "link-button", r#type: "button", onclick: move |_| on_forgot.call(()),
                    {tr(messages::SIGN_IN_FORGOT_LINK)}
                }
            }
        }
    }
}

// --- First-Admin setup -------------------------------------------------------------------

/// First-Admin setup: step 1 checks the code; step 2 asks for the account.
#[component]
pub(crate) fn Setup() -> Element {
    let step_two = use_signal(|| false);
    let code = use_signal(String::new);
    let error = use_signal(|| None::<String>);
    rsx! {
        AuthCard {
            if step_two() {
                SetupAccount { code, step_two, error }
            } else {
                SetupCode { code, step_two, error }
            }
        }
    }
}

/// Setup step 1: the code.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SetupCode(
    code: Signal<String>,
    step_two: Signal<bool>,
    error: Signal<Option<String>>,
) -> Element {
    let state = use_auth();
    let mut step_two = step_two;
    let mut error = error;
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        busy.set(true);
        spawn(async move {
            let result = api::check_setup_code(code()).await;
            busy.set(false);
            match result {
                Ok(CheckSetupCodeOutcome::Valid) => {
                    error.set(None);
                    step_two.set(true);
                }
                Ok(CheckSetupCodeOutcome::WrongCode) => {
                    error.set(Some(tr(messages::SETUP_WRONG_CODE)));
                }
                Ok(CheckSetupCodeOutcome::TooManyAttempts {
                    retry_after_seconds,
                }) => {
                    error.set(Some(too_many_attempts(retry_after_seconds)));
                }
                // Someone else finished setup: show sign-in.
                Ok(CheckSetupCodeOutcome::AlreadySetUp) => refresh(state).await,
                Err(failure) => error.set(Some(transport_error(&failure))),
            }
        });
    };

    rsx! {
        form { class: "auth-form", onsubmit: submit,
            p { class: "auth-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(1))])} }
            h1 { class: "auth-title", {tr(messages::SETUP_TITLE)} }
            p { class: "auth-subtitle", {tr(messages::SETUP_SUBTITLE)} }
            p { class: "auth-help", {tr(messages::SETUP_CODE_HELP)} }
            Field { label: messages::FIELD_SETUP_CODE, value: code, mono: true, autofocus: true }
            FormError { message: error() }
            button { class: "primary-button", r#type: "submit", disabled: busy(),
                if busy() { {tr(messages::SETUP_CHECKING)} } else { {tr(messages::SETUP_NEXT)} }
            }
        }
    }
}

/// Setup step 2: the Admin's account.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SetupAccount(
    code: Signal<String>,
    step_two: Signal<bool>,
    error: Signal<Option<String>>,
) -> Element {
    let mut state = use_auth();
    let mut step_two = step_two;
    let mut error = error;
    let username = use_signal(String::new);
    let display_name = use_signal(String::new);
    let password = use_signal(String::new);
    let again = use_signal(String::new);
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        if password() != again() {
            error.set(Some(tr(messages::PASSWORDS_DIFFER)));
            return;
        }
        busy.set(true);
        spawn(async move {
            let result = api::setup(
                code(),
                username(),
                display_name(),
                password(),
                client::USES_SESSION_TOKEN,
            )
            .await;
            busy.set(false);
            match result {
                Ok(SetupOutcome::SignedIn { session, token }) => {
                    tracing::info!(target: log_area::AUTH, "first Admin created and signed in");
                    if token.is_some() {
                        client::set_session_token(token);
                    }
                    state.set(AuthState::SignedIn(session));
                }
                Ok(SetupOutcome::WrongCode) => {
                    error.set(Some(tr(messages::SETUP_WRONG_CODE)));
                    step_two.set(false);
                }
                Ok(SetupOutcome::TooManyAttempts {
                    retry_after_seconds,
                }) => {
                    error.set(Some(too_many_attempts(retry_after_seconds)));
                }
                Ok(SetupOutcome::InvalidUsername { reason }) => {
                    error.set(Some(tr(reason.message())));
                }
                Ok(SetupOutcome::InvalidDisplayName { reason }) => {
                    error.set(Some(tr(reason.message())));
                }
                Ok(SetupOutcome::InvalidPassword { reason }) => {
                    error.set(Some(tr(reason.message())));
                }
                Ok(SetupOutcome::AlreadySetUp) => refresh(state).await,
                Ok(SetupOutcome::Unavailable) => {
                    error.set(Some(tr(messages::SOMETHING_WENT_WRONG)));
                }
                Err(failure) => error.set(Some(transport_error(&failure))),
            }
        });
    };

    rsx! {
        form { class: "auth-form", onsubmit: submit,
            p { class: "auth-step", {tr_with(messages::SETUP_STEP, &[("step", MessageArg::from(2))])} }
            h1 { class: "auth-title", {tr(messages::SETUP_ACCOUNT_TITLE)} }
            p { class: "auth-subtitle", {tr(messages::SETUP_CODE_ACCEPTED)} }
            Field { label: messages::FIELD_USERNAME, value: username, autocomplete: "username", autofocus: true }
            Field { label: messages::FIELD_DISPLAY_NAME, value: display_name, autocomplete: "name" }
            Field { label: messages::FIELD_NEW_PASSWORD, value: password, kind: "password", autocomplete: "new-password" }
            Field { label: messages::FIELD_PASSWORD_AGAIN, value: again, kind: "password", autocomplete: "new-password" }
            FormError { message: error() }
            button { class: "primary-button", r#type: "submit", disabled: busy(),
                if busy() { {tr(messages::SETUP_CREATING)} } else { {tr(messages::SETUP_CREATE)} }
            }
            button {
                class: "link-button",
                r#type: "button",
                onclick: move |_| {
                    error.set(None);
                    step_two.set(false);
                },
                {tr(messages::ACTION_BACK)}
            }
        }
    }
}

// --- Changing a password -----------------------------------------------------------------

/// The change-password form: current, new, and new again. `on_done` runs after a change;
/// `on_cancel` shows a Cancel button (the dialog has one; the forced screen doesn't).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ChangePasswordForm(
    on_done: EventHandler<()>,
    #[props(default)] on_cancel: Option<EventHandler<()>>,
) -> Element {
    let current = use_signal(String::new);
    let new = use_signal(String::new);
    let again = use_signal(String::new);
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);

    let submit = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        if new() != again() {
            error.set(Some(tr(messages::PASSWORDS_DIFFER)));
            return;
        }
        busy.set(true);
        spawn(async move {
            let result = api::change_password(current(), new()).await;
            busy.set(false);
            match result {
                Ok(ChangePasswordOutcome::Changed) => {
                    tracing::info!(target: log_area::AUTH, "password changed");
                    on_done.call(());
                }
                Ok(ChangePasswordOutcome::WrongPassword) => {
                    error.set(Some(tr(messages::PASSWORD_CURRENT_WRONG)));
                }
                Ok(ChangePasswordOutcome::InvalidNewPassword { reason }) => {
                    error.set(Some(tr(reason.message())));
                }
                Ok(ChangePasswordOutcome::NotSignedIn) => {
                    error.set(Some(tr(messages::API_ERROR_MISSING_CREDENTIAL)));
                }
                Ok(ChangePasswordOutcome::Unavailable) => {
                    error.set(Some(tr(messages::SOMETHING_WENT_WRONG)));
                }
                Err(failure) => error.set(Some(transport_error(&failure))),
            }
        });
    };

    rsx! {
        form { class: "auth-form", onsubmit: submit,
            Field { label: messages::FIELD_CURRENT_PASSWORD, value: current, kind: "password", autocomplete: "current-password", autofocus: true }
            Field { label: messages::FIELD_NEW_PASSWORD_LONG, value: new, kind: "password", autocomplete: "new-password" }
            Field { label: messages::FIELD_NEW_PASSWORD_AGAIN, value: again, kind: "password", autocomplete: "new-password" }
            FormError { message: error() }
            div { class: "form-buttons",
                if let Some(on_cancel) = on_cancel {
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_cancel.call(()),
                        {tr(messages::ACTION_CANCEL)}
                    }
                }
                button { class: "primary-button", r#type: "submit", disabled: busy(),
                    {tr(messages::PASSWORD_SAVE)}
                }
            }
        }
    }
}

/// After an Admin set a temporary password: choose your own before anything else.
#[component]
pub(crate) fn ForcedPasswordChange() -> Element {
    let state = use_auth();
    rsx! {
        AuthCard {
            h1 { class: "auth-title", {tr(messages::PASSWORD_MUST_CHANGE_TITLE)} }
            p { class: "auth-subtitle", {tr(messages::PASSWORD_MUST_CHANGE_TEXT)} }
            ChangePasswordForm { on_done: move |()| { spawn(refresh(state)); } }
        }
    }
}
