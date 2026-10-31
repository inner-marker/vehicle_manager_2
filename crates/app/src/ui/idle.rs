//! Leaving Admin mode and signing out when nobody is using the app (decision #049).
//!
//! The [`IdleWatcher`] runs in the shell while someone is signed in. A small script records the
//! last real input (pointer down, key down, wheel, scroll, touch) and reports it with the time
//! about every 15 seconds; [`domain::idle::decide`] says what to do. The server stays the
//! authority: its own limits apply even when the app is closed.
//!
//! What happened is shown as a short notice: [`AdminEndedNotice`] in the shell, and
//! [`SignedOutNotice`] on the sign-in card.

use std::cell::Cell;
use std::rc::Rc;

use dioxus::prelude::*;
use domain::access::ActiveMode;
use domain::api::PreferencesOutcome;
use domain::i18n::{MessageArg, messages};
use domain::idle::{IdleAction, IdleState, decide};
use domain::log_area;
use domain::preferences::IdleSignOut;
use domain::time::UnixMillis;

use super::account::{sign_out, switch};
use super::auth::{AuthState, refresh, use_auth};
use super::{tr, tr_with};
use crate::api;

/// Records the last real input and sends `[now, last input]` (Unix milliseconds) every 15
/// seconds, until Rust sends anything back (the watcher is going away).
const WATCH_SCRIPT: &str = r#"
const kinds = ["pointerdown", "keydown", "wheel", "scroll", "touchstart"];
let lastInput = Date.now();
const mark = () => { lastInput = Date.now(); };
for (const kind of kinds) {
    window.addEventListener(kind, mark, { capture: true, passive: true });
}
let stopped = false;
dioxus.recv().then(() => { stopped = true; });
while (!stopped) {
    dioxus.send([Date.now(), lastInput]);
    await new Promise((resolve) => setTimeout(resolve, 15000));
}
for (const kind of kinds) {
    window.removeEventListener(kind, mark, { capture: true });
}
"#;

/// Milliseconds in one hour, for the "signed out after N hours" notice.
const HOUR_MILLIS: i64 = 60 * 60 * 1000;

/// What the app did by itself, shown until dismissed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IdleNotice {
    /// Left Admin mode after 15 idle minutes.
    AdminEnded,
    /// Signed out after this many idle hours.
    SignedOut {
        /// The person's setting, in hours.
        hours: i64,
    },
}

/// The idle watcher's shared state, in context (provided by the app root).
#[derive(Debug, Clone, Copy)]
pub(crate) struct IdleContext {
    /// The sign-out setting in effect for the signed-in person (the Settings screen updates
    /// it when it changes).
    pub sign_out: Signal<IdleSignOut>,
    /// The notice to show, if any.
    pub notice: Signal<Option<IdleNotice>>,
}

/// Puts a fresh [`IdleContext`] in context; call once, in the root.
pub(crate) fn provide_idle_context() {
    use_context_provider(|| IdleContext {
        sign_out: Signal::new(IdleSignOut::default()),
        notice: Signal::new(None),
    });
}

/// The [`IdleContext`], from context.
pub(crate) fn use_idle() -> IdleContext {
    use_context::<IdleContext>()
}

/// Watches for inactivity while someone is signed in; renders nothing.
#[component]
pub(crate) fn IdleWatcher() -> Element {
    let auth = use_auth();
    let idle = use_idle();
    // The running script, so it can be told to stop when the watcher goes away.
    let running = use_hook(|| Rc::new(Cell::new(None::<document::Eval>)));

    {
        let running = Rc::clone(&running);
        // Runs after mounting, never during server rendering.
        use_future(move || {
            let running = Rc::clone(&running);
            async move {
                load_setting(idle).await;
                let eval = document::eval(WATCH_SCRIPT);
                running.set(Some(eval));
                watch(eval, auth, idle).await;
            }
        });
    }
    use_drop(move || {
        if let Some(eval) = running.take() {
            // The script may already have finished; nothing to do then.
            let _ = eval.send(true);
        }
    });
    rsx! {}
}

/// Loads the person's sign-out setting; on failure the default (8 hours) stays.
async fn load_setting(mut idle: IdleContext) {
    match api::get_preferences().await {
        Ok(PreferencesOutcome::Preferences { view }) => {
            idle.sign_out.set(view.effective.idle_sign_out);
        }
        Ok(other) => {
            tracing::debug!(target: log_area::AUTH, ?other, "idle watcher: no preferences; using the default sign-out time");
        }
        Err(error) => {
            tracing::debug!(target: log_area::AUTH, %error, "idle watcher: preferences failed; using the default sign-out time");
        }
    }
}

/// What the watcher remembers between reports.
#[derive(Debug, Default)]
struct Tracker {
    /// The last time the server heard from us for this person (entering Admin mode counts).
    last_keep_alive: Option<UnixMillis>,
    /// The mode at the last report.
    last_mode: Option<ActiveMode>,
}

impl Tracker {
    /// Notes a report; returns the times to decide on, and whether Admin mode was just entered.
    fn observe(
        &mut self,
        mode: ActiveMode,
        now: UnixMillis,
        last_input: UnixMillis,
    ) -> (IdleState, bool) {
        let entered_admin = mode == ActiveMode::Admin && self.last_mode != Some(ActiveMode::Admin);
        if entered_admin {
            self.last_keep_alive = Some(now);
        }
        self.last_mode = Some(mode);
        let state = IdleState {
            now,
            last_input,
            last_keep_alive: self.last_keep_alive.unwrap_or(now),
        };
        (state, entered_admin)
    }
}

/// Reads the script's reports and acts on them, until the script stops or the app signs out.
async fn watch(mut eval: document::Eval, auth: Signal<AuthState>, mut idle: IdleContext) {
    tracing::debug!(target: log_area::AUTH, "idle watcher started");
    // Signed in again: the sign-in card's notice has done its job.
    if matches!(*idle.notice.peek(), Some(IdleNotice::SignedOut { .. })) {
        idle.notice.set(None);
    }
    let mut tracker = Tracker::default();
    while let Ok((now, last_input)) = eval.recv::<(i64, i64)>().await {
        let AuthState::SignedIn(session) = auth.peek().clone() else {
            break;
        };
        let now = UnixMillis::from_millis(now);
        let (state, entered_admin) =
            tracker.observe(session.mode, now, UnixMillis::from_millis(last_input));
        if entered_admin && *idle.notice.peek() == Some(IdleNotice::AdminEnded) {
            idle.notice.set(None);
        }
        let action = decide(state, session.mode, *idle.sign_out.peek());
        if action == IdleAction::KeepAlive {
            tracker.last_keep_alive = Some(now);
        }
        if act(action, auth, idle).await {
            break;
        }
    }
    tracing::debug!(target: log_area::AUTH, "idle watcher stopped");
}

/// Does what [`decide`] said; returns `true` once signed out (the watcher then stops).
async fn act(action: IdleAction, auth: Signal<AuthState>, idle: IdleContext) -> bool {
    match action {
        IdleAction::Nothing => false,
        IdleAction::KeepAlive => {
            tracing::debug!(target: log_area::AUTH, "idle watcher: keeping Admin mode alive");
            refresh(auth).await;
            false
        }
        IdleAction::LeaveAdmin => {
            leave_admin(auth, idle).await;
            false
        }
        IdleAction::SignOut => {
            sign_out_idle(auth, idle).await;
            true
        }
    }
}

/// Leaves Admin mode, as **Leave** does, and says so.
async fn leave_admin(auth: Signal<AuthState>, mut idle: IdleContext) {
    tracing::info!(target: log_area::AUTH, "idle watcher: leaving Admin mode after 15 idle minutes");
    if let Some(problem) = switch(ActiveMode::Driver, None).await {
        tracing::warn!(target: log_area::AUTH, problem, "idle watcher: couldn't leave Admin mode");
    }
    idle.notice.set(Some(IdleNotice::AdminEnded));
    refresh(auth).await;
}

/// Signs out, leaving a notice for the sign-in card.
async fn sign_out_idle(auth: Signal<AuthState>, mut idle: IdleContext) {
    let hours = idle.sign_out.peek().after_millis().unwrap_or_default() / HOUR_MILLIS;
    tracing::info!(target: log_area::AUTH, hours, "idle watcher: signing out");
    idle.notice.set(Some(IdleNotice::SignedOut { hours }));
    sign_out(auth).await;
}

#[cfg(test)]
mod tests {
    use super::*;

    const MINUTE: i64 = 60_000;

    fn at(minutes: i64) -> UnixMillis {
        UnixMillis::from_millis(minutes * MINUTE)
    }

    #[test]
    fn entering_admin_mode_counts_as_a_keep_alive() {
        let mut tracker = Tracker::default();
        let (_, entered) = tracker.observe(ActiveMode::Driver, at(0), at(0));
        assert!(!entered);
        let (state, entered) = tracker.observe(ActiveMode::Admin, at(5), at(1));
        assert!(entered);
        assert_eq!(state.last_keep_alive, at(5));
        let (state, entered) = tracker.observe(ActiveMode::Admin, at(6), at(6));
        assert!(!entered, "still in Admin mode");
        assert_eq!(state.last_keep_alive, at(5));
    }

    #[test]
    fn before_any_keep_alive_the_first_report_counts_as_one() {
        let mut tracker = Tracker::default();
        let (state, _) = tracker.observe(ActiveMode::Driver, at(3), at(1));
        assert_eq!(state.last_keep_alive, at(3));
    }
}

/// "Admin mode ended after 15 idle minutes.", under the top bar until dismissed.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn AdminEndedNotice() -> Element {
    let mut idle = use_idle();
    if *idle.notice.read() != Some(IdleNotice::AdminEnded) {
        return rsx! {};
    }
    rsx! {
        div { class: "notice-bar", role: "status",
            span { class: "notice-text", {tr(messages::IDLE_ADMIN_ENDED)} }
            button { class: "notice-dismiss", onclick: move |_| idle.notice.set(None),
                {tr(messages::NOTICE_DISMISS)}
            }
        }
    }
}

/// "You were signed out after N idle hours.", on the sign-in card.
#[component]
pub(crate) fn SignedOutNotice() -> Element {
    let idle = use_idle();
    let Some(IdleNotice::SignedOut { hours }) = *idle.notice.read() else {
        return rsx! {};
    };
    rsx! {
        p { class: "auth-notice", role: "status",
            {tr_with(messages::IDLE_SIGNED_OUT, &[("hours", MessageArg::from(hours))])}
        }
    }
}
