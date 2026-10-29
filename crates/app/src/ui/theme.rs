//! The Light / Dark / System theme setting (decision #012), default Dark.
//!
//! Held in a context signal and applied as `data-theme` on the root element;
//! `main.css` defines the colors for each value, and `system` follows the OS
//! through `prefers-color-scheme`. In memory only for now: remembering the choice
//! per device needs local storage, which comes in a later phase.

use dioxus::prelude::*;
use domain::i18n::{MessageId, messages};
use domain::log_area;

use super::icons::{Icon, IconView};
use super::tr;

/// The user's theme choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) enum ThemeChoice {
    Light,
    #[default]
    Dark,
    System,
}

impl ThemeChoice {
    /// Every choice, in the order the toggle shows them.
    pub(crate) const ALL: [Self; 3] = [Self::Light, Self::Dark, Self::System];

    /// The `data-theme` value `main.css` styles.
    pub(crate) const fn attribute(self) -> &'static str {
        match self {
            Self::Light => "light",
            Self::Dark => "dark",
            Self::System => "system",
        }
    }

    pub(crate) const fn label(self) -> MessageId {
        match self {
            Self::Light => messages::THEME_LIGHT,
            Self::Dark => messages::THEME_DARK,
            Self::System => messages::THEME_SYSTEM,
        }
    }

    const fn icon(self) -> Icon {
        match self {
            Self::Light => Icon::Light,
            Self::Dark => Icon::Dark,
            Self::System => Icon::System,
        }
    }
}

/// A segmented Light / Dark / System toggle. Shown in the top bar on wide screens
/// and in the ☰ menu on narrow ones.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ThemeToggle() -> Element {
    // Provided by `App`, which wraps every screen.
    let mut theme = use_context::<Signal<ThemeChoice>>();

    rsx! {
        div { class: "theme-toggle", role: "group", "aria-label": tr(messages::THEME_LABEL),
            for choice in ThemeChoice::ALL {
                button {
                    key: "{choice.attribute()}",
                    class: "theme-option",
                    title: tr(choice.label()),
                    "aria-pressed": if theme() == choice { "true" } else { "false" },
                    onclick: move |_| {
                        theme.set(choice);
                        tracing::debug!(target: log_area::UI, theme = choice.attribute(), "theme changed");
                    },
                    IconView { icon: choice.icon() }
                    span { class: "theme-option-label", {tr(choice.label())} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn theme_defaults_to_dark() {
        assert_eq!(ThemeChoice::default(), ThemeChoice::Dark);
    }

    #[test]
    fn theme_choice_maps_to_data_theme_attribute() {
        let attributes: Vec<&str> = ThemeChoice::ALL.iter().map(|c| c.attribute()).collect();
        assert_eq!(attributes, ["light", "dark", "system"]);
    }
}
