//! Who is signed in, for the whole app, and the gate in front of the screens.
//!
//! [`AuthState`] lives in context. The [`AuthGate`] shows, in order:
//! 1. a loading line until the app has asked the server (never during server rendering:
//!    server functions are only called from effects; see `crate::api`);
//! 2. **first-Admin setup** while the server has no users;
//! 3. **sign-in** when nobody is signed in;
//! 4. a **forced password change** after an Admin set a temporary password;
//! 5. otherwise, the app itself.
//!
//! [`refresh`] asks the server again; call it after anything that changes the session
//! (sign-in, sign-out, mode switch, password change) and on navigation, so an Admin mode
//! the server ended after 15 idle minutes stops showing.

use dioxus::prelude::*;
use domain::api::SessionInfo;
use domain::log_area;

use crate::api;
use crate::client;
use crate::ui::screens::{ForcedPasswordChange, Setup, SignIn};
use crate::ui::tr;

/// Where the app stands with the server.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum AuthState {
    /// Not asked yet (also what server rendering shows).
    Loading,
    /// The server has no users: show first-Admin setup.
    NeedsSetup,
    /// Nobody is signed in.
    SignedOut,
    /// Someone is signed in.
    SignedIn(SessionInfo),
    /// The server couldn't be asked (unreachable, or refused the app version); the message
    /// says why, and the sign-in screen shows it.
    Unreachable(domain::i18n::MessageId),
}

/// The signal holding the [`AuthState`], from context.
pub(crate) fn use_auth() -> Signal<AuthState> {
    use_context::<Signal<AuthState>>()
}

/// Asks the server who is signed in, and updates `state`.
pub(crate) async fn refresh(mut state: Signal<AuthState>) {
    client::apply_request_headers();
    let next = match api::session().await {
        Ok(session) if session.setup_needed => AuthState::NeedsSetup,
        Ok(session) => session
            .signed_in
            .map_or(AuthState::SignedOut, AuthState::SignedIn),
        Err(error) => {
            let problem = crate::ui::screens::problem_from_error(&error);
            tracing::debug!(target: log_area::AUTH, ?problem, "session check failed");
            AuthState::Unreachable(problem.message())
        }
    };
    // A token the server no longer accepts is useless; forget it.
    if matches!(next, AuthState::SignedOut) {
        client::set_session_token(None);
    }
    if *state.peek() != next {
        tracing::debug!(target: log_area::AUTH, signed_in = matches!(next, AuthState::SignedIn(_)), "session state changed");
        state.set(next);
    }
}

/// Shows setup, sign-in, a forced password change, or `children` (the app), by [`AuthState`].
#[component]
pub(crate) fn AuthGate(children: Element) -> Element {
    let state = use_auth();
    // Runs in the browser or app after mounting, never during server rendering.
    use_effect(move || {
        spawn(refresh(state));
    });

    match state() {
        AuthState::Loading => rsx! {
            div { class: "auth-page",
                p { class: "auth-loading", {tr(domain::i18n::messages::LOADING)} }
            }
        },
        AuthState::NeedsSetup => rsx! { Setup {} },
        AuthState::SignedOut | AuthState::Unreachable(_) => rsx! { SignIn {} },
        AuthState::SignedIn(session) if session.must_change_password => {
            rsx! { ForcedPasswordChange {} }
        }
        AuthState::SignedIn(_) => children,
    }
}
