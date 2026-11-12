//! Offline on the desktop (phase-3.md 4e, decision #075; `docs/ui-sketches/p3m4-offline-options.svg`
//! options 1A and 3A).
//!
//! While the copy's last pull couldn't reach the server (`crate::local_copy::is_offline`):
//! - [`OfflineTag`] shows **OFFLINE · copy 10:42** in the top bar (1A), with what it means as
//!   its tooltip;
//! - the app's root carries `data-offline="true"`, so the stylesheet turns saving off (Save,
//!   Delete, Finish; 3A) and writes [`saving_note`] beside each form's buttons;
//! - the buttons that only start a change (the Log buttons, **+ Squawk**, a squawk's Edit and
//!   Remove) are disabled, with [`needs_server_note`] as their tooltip.
//!
//! Saving needs the server until milestone 5.
//!
//! Nothing on the web: it has no copy and is never offline this way.

use dioxus::prelude::*;

/// Whether the desktop is offline now (reactive: a component reading it re-renders when it
/// changes).
#[must_use]
pub(crate) fn is_offline_now() -> bool {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        matches!(
            crate::local_copy::STATUS(),
            crate::local_copy::CopyStatus::Offline { .. }
        )
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    false
}

/// The tooltip for a button that starts a change, which is off while offline (the Log
/// buttons, **+ Squawk**, a squawk's Edit and Remove): why it's off; nothing while online.
#[must_use]
pub(crate) fn needs_server_note() -> Option<String> {
    is_offline_now().then(|| crate::ui::tr(domain::i18n::messages::OFFLINE_NEEDS_SERVER))
}

/// The root's `style`: the saving note, as a CSS string the stylesheet shows (so it's in the
/// person's language).
#[must_use]
pub(crate) fn saving_note() -> String {
    let note = crate::ui::tr(domain::i18n::messages::OFFLINE_SAVING_NOTE)
        .replace('\\', "\\\\")
        .replace('"', "\\\"");
    format!("--offline-note: \"{note}\";")
}

/// The OFFLINE tag (1A); nothing while online, and nothing on the web.
#[component]
pub(crate) fn OfflineTag() -> Element {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        use domain::i18n::{MessageArg, Translator, messages};

        use crate::ui::{tr, tr_with};

        let zone = crate::ui::screens::use_device_zone();
        let crate::local_copy::CopyStatus::Offline { since } = crate::local_copy::STATUS() else {
            return rsx! {};
        };
        let time = since
            .and_then(|at| domain::dates::format_time(at, &zone(), Translator::english()).ok());
        let (label, note) = match time {
            Some(time) => (
                tr_with(
                    messages::OFFLINE_TAG,
                    &[("time", MessageArg::from(time.as_str()))],
                ),
                tr_with(
                    messages::OFFLINE_TAG_NOTE,
                    &[("time", MessageArg::from(time.as_str()))],
                ),
            ),
            None => (
                tr(messages::OFFLINE_TAG_NO_TIME),
                tr(messages::COPY_OFFLINE),
            ),
        };
        return rsx! {
            span { class: "offline-tag", role: "status", title: note, {label} }
        };
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    rsx! {}
}
