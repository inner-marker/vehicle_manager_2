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
//! **Offline** (a desktop that can't reach its server but has a complete copy; phase-3.md
//! 4e), the app opens as the person the desktop is sealed for, in Driver mode, reading the
//! copy; the copy's puller asks again once the server answers.
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

/// The server's instance name (`None`: the household's main server; decision #062), from the
/// last session check. Shown as the **TEST SERVER** tag.
pub(crate) static SERVER_INSTANCE: GlobalSignal<Option<String>> = Signal::global(|| None);

/// Measures the server's clock (the public, unsigned handshake), so a paired desktop's
/// signatures use the server's time (#073).
async fn measure_server_time() {
    match api::handshake().await {
        Ok(handshake) => client::set_server_time(handshake.server_time_ms),
        Err(error) => {
            tracing::debug!(target: log_area::AUTH, %error, "can't measure the server's clock");
        }
    }
    client::apply_request_headers();
}

/// Asks the server who is signed in. A paired desktop first measures the server's clock (once
/// per run); if the server then says nobody is signed in, it measures again and asks once
/// more, so a computer clock that jumped never costs a pairing.
async fn ask_session() -> Result<domain::api::SessionState, ServerFnError> {
    if client::has_device_key() && !client::server_time_known() {
        measure_server_time().await;
    }
    client::apply_request_headers();
    let answer = api::session().await;
    let signed_out =
        matches!(&answer, Ok(session) if session.signed_in.is_none() && !session.setup_needed);
    if client::has_device_key() && signed_out {
        measure_server_time().await;
        return api::session().await;
    }
    answer
}

/// The state the server's answer means.
fn state_from(answer: Result<domain::api::SessionState, ServerFnError>) -> AuthState {
    match answer {
        Ok(session) if session.setup_needed => AuthState::NeedsSetup,
        Ok(session) => session
            .signed_in
            .map_or(AuthState::SignedOut, AuthState::SignedIn),
        Err(error) => {
            let problem = crate::ui::screens::problem_from_error(&error);
            tracing::debug!(target: log_area::AUTH, ?problem, "session check failed");
            AuthState::Unreachable(problem.message())
        }
    }
}

/// Asks the server who is signed in, and updates `state` (see [`ask_session`]).
pub(crate) async fn refresh(mut state: Signal<AuthState>) {
    let answer = client::within(client::SESSION_WAIT, ask_session()).await;
    if let Ok(session) = &answer
        && *SERVER_INSTANCE.peek() != session.instance
    {
        SERVER_INSTANCE.write().clone_from(&session.instance);
    }
    #[cfg_attr(
        not(all(feature = "desktop", not(target_arch = "wasm32"))),
        expect(unused_mut, reason = "only the desktop opens offline")
    )]
    let mut next = state_from(answer);
    #[cfg_attr(
        not(all(feature = "desktop", not(target_arch = "wasm32"))),
        expect(unused_mut, reason = "only the desktop opens offline")
    )]
    let mut offline = false;
    // No connection: a desktop with a complete copy opens it, in Driver mode (phase-3.md 4e),
    // with the server's instance (its TEST SERVER tag) as last remembered.
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if matches!(next, AuthState::Unreachable(_))
        && let Some(session) = crate::local_copy::offline_session().await
    {
        next = AuthState::SignedIn(session);
        offline = true;
        let instance = crate::local_copy::known_instance();
        if *SERVER_INSTANCE.peek() != instance {
            *SERVER_INSTANCE.write() = instance;
        }
    }
    // A token the server no longer accepts is useless; forget it (and a desktop's pairing:
    // it was signed out, perhaps from Settings -> Devices).
    if matches!(next, AuthState::SignedOut) {
        if client::has_device_key() {
            tracing::info!(target: log_area::AUTH, "the server no longer accepts this device's sign-in; forgetting it");
        }
        crate::pairing::forget();
    }
    let signed_in = matches!(next, AuthState::SignedIn(_));
    if *state.peek() != next {
        tracing::debug!(target: log_area::AUTH, signed_in, "session state changed");
        state.set(next);
    }
    // The desktop remembers this server (its recent servers; phase-3.md 2d), once per run;
    // not offline, when the handshake can't say what it is.
    if signed_in && !offline {
        crate::ui::recent_servers::remember_once(SERVER_INSTANCE.peek().clone()).await;
    }
}

/// The **TEST SERVER** tag on a server that isn't the household's main one (the tab's title
/// says so too: `ui::page_title`); nothing on the main server.
#[component]
pub(crate) fn InstanceTag() -> Element {
    let Some(name) = SERVER_INSTANCE() else {
        return rsx! {};
    };
    let label = crate::ui::tr_with(
        domain::i18n::messages::INSTANCE_TAG,
        &[("name", domain::i18n::MessageArg::from(name.as_str()))],
    );
    rsx! {
        span { class: "instance-tag", title: tr(domain::i18n::messages::INSTANCE_TAG_NOTE),
            // The full "TEST SERVER", or just "TEST" in a phone's crowded top bar.
            span { class: "instance-tag-full", {label} }
            span { class: "instance-tag-short", {name} }
        }
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
