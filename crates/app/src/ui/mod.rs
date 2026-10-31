//! The user interface: one adaptive layout for web, desktop, and mobile (decision #011).
//!
//! - `routes`: every screen's address.
//! - `shell`: the layout around every screen (sidebar, top bar, bottom tabs).
//! - `nav`: which screens appear in the sidebar and bottom tabs.
//! - `theme`: Light / Dark / System, default Dark (decision #012).
//! - `auth`: who is signed in, and the gate showing setup, sign-in, or the app.
//! - `account`: the mode chip, account chip, Admin dialog, and Admin banner (in the shell).
//! - `screens`: one component per screen; placeholders until their phase.
//!
//! Every visible string comes from `domain::i18n`; components never contain
//! business rules or touch the database.

mod account;
pub(crate) mod auth;
mod icons;
pub(crate) mod nav;
pub(crate) mod routes;
mod screens;
mod shell;
pub(crate) mod theme;

use dioxus::prelude::*;
use domain::i18n::{MessageArg, MessageId, Translator};

use routes::Route;
use theme::ThemeChoice;

/// The app's single stylesheet: theme colors and the three layouts.
const MAIN_CSS: Asset = asset!("/assets/main.css");

/// The root component: provides the theme, loads the stylesheet, and starts the router.
///
/// Expects the debug log's `LogHandles` in context (`LaunchBuilder::with_context`);
/// without them the log viewer says logging isn't running.
#[component]
pub fn App() -> Element {
    let theme = use_context_provider(|| Signal::new(ThemeChoice::default()));
    use_context_provider(|| Signal::new(auth::AuthState::Loading));

    rsx! {
        Stylesheet { href: MAIN_CSS }
        div { class: "app", "data-theme": theme().attribute(),
            auth::AuthGate { Router::<Route> {} }
        }
    }
}

/// The English text of a message (the only language so far; decision #022).
fn tr(id: MessageId) -> String {
    Translator::english().tr(id)
}

/// The English text of a message with arguments filled in.
fn tr_with(id: MessageId, args: &[(&str, MessageArg)]) -> String {
    Translator::english().tr_with(id, args)
}
