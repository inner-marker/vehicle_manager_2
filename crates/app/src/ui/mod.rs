//! The user interface: one adaptive layout for web, desktop, and mobile (decision #011).
//!
//! - `routes`: every screen's address.
//! - `shell`: the layout around every screen (sidebar, top bar, bottom tabs).
//! - `nav`: which screens appear in the sidebar and bottom tabs.
//! - `theme`: Light / Dark / System, default Dark (decision #012).
//! - `auth`: who is signed in, and the gate showing setup, sign-in, or the app.
//! - `account`: the mode chip, account chip, Admin dialog, and Admin banner (in the shell).
//! - `idle`: leaving Admin mode and signing out when idle, and their notices (#049).
//! - `inbox`: the person's messages, the unread badge, and the urgent banner (#050, #053).
//! - `vehicles`: the groups and vehicles the person can see, and the sidebar's group links
//!   (#052, #056).
//! - `screens`: one component per screen; placeholders until their phase.
//!
//! Every visible string comes from `domain::i18n` (through `tr` and `tr_with`, which call
//! squawks by the household's word, #064); components never contain business rules or touch
//! the database.

mod account;
pub(crate) mod auth;
mod icons;
pub(crate) mod idle;
pub(crate) mod inbox;
pub(crate) mod location;
pub(crate) mod nav;
pub(crate) mod pdfs;
pub(crate) mod photos;
pub(crate) mod routes;
mod screens;
mod shell;
pub(crate) mod theme;
pub(crate) mod units;
pub(crate) mod vehicles;

use std::sync::atomic::{AtomicU8, Ordering};

use dioxus::prelude::*;
use domain::i18n::{MessageArg, MessageId, Translator};
use domain::preferences::SquawkWord;

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
    idle::provide_idle_context();

    rsx! {
        Stylesheet { href: MAIN_CSS }
        div { class: "app", "data-theme": theme().attribute(),
            auth::AuthGate { Router::<Route> {} }
        }
    }
}

/// What the household calls squawks (#064), as its place in [`SquawkWord::ALL`]: read by
/// every [`tr`] (outside a component too), so a plain atomic rather than a signal.
static SQUAWK_WORD_INDEX: AtomicU8 = AtomicU8::new(0);

/// The same word, for the shell to re-render when it changes.
pub(crate) static SQUAWK_WORD: GlobalSignal<SquawkWord> = Signal::global(SquawkWord::default);

/// The household's word for squawks, as last loaded.
fn squawk_word() -> SquawkWord {
    SquawkWord::ALL
        .get(usize::from(SQUAWK_WORD_INDEX.load(Ordering::Relaxed)))
        .copied()
        .unwrap_or_default()
}

/// Uses `word` for squawks from now on (from the preferences the server sends).
pub(crate) fn set_squawk_word(word: SquawkWord) {
    let index = SquawkWord::ALL
        .iter()
        .position(|each| *each == word)
        .and_then(|index| u8::try_from(index).ok())
        .unwrap_or(0);
    SQUAWK_WORD_INDEX.store(index, Ordering::Relaxed);
    if *SQUAWK_WORD.peek() != word {
        *SQUAWK_WORD.write() = word;
    }
}

/// The person's currency (the household's unless they chose their own), as last loaded: amounts
/// in it show a plain "$", others "CA$" and so on (`CurrencyCode::symbol`).
static HOME_CURRENCY: std::sync::Mutex<domain::preferences::CurrencyCode> =
    std::sync::Mutex::new(domain::preferences::CurrencyCode::USD);

/// Uses `currency` as the person's own from now on (from the preferences the server sends).
pub(crate) fn set_home_currency(currency: domain::preferences::CurrencyCode) {
    *HOME_CURRENCY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = currency;
}

/// The person's currency, as last loaded.
pub(crate) fn home_currency() -> domain::preferences::CurrencyCode {
    *HOME_CURRENCY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// An amount with its currency's symbol, as the person sees it: "$45.67", "CA$45.67".
pub(crate) fn money(minor: i64, currency: domain::preferences::CurrencyCode) -> String {
    domain::quantity::show_money(minor, currency, home_currency())
}

/// The English text of a message (the only language so far; decision #022).
fn tr(id: MessageId) -> String {
    Translator::english().tr_for(squawk_word(), id)
}

/// The English text of a message with arguments filled in.
fn tr_with(id: MessageId, args: &[(&str, MessageArg)]) -> String {
    Translator::english().tr_with_for(squawk_word(), id, args)
}

#[cfg(test)]
mod stylesheet {
    /// Every `{` in the stylesheet is closed, so no rule lands inside a phone-only block by
    /// mistake (a merge once dropped one, and the Documents tab was styled only on phones).
    #[test]
    fn the_stylesheets_braces_balance() {
        let css = include_str!("../../assets/main.css");
        let mut depth = 0_i32;
        for (line, text) in css.lines().enumerate() {
            for character in text.chars() {
                match character {
                    '{' => depth += 1,
                    '}' => depth -= 1,
                    _other => {}
                }
                assert!(depth >= 0, "an extra closing brace on line {}", line + 1);
            }
        }
        assert_eq!(depth, 0, "a block is left open");
    }
}
