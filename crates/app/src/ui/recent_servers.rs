//! The desktop's recent servers (phase-3.md 2d, #073; `docs/ui-sketches/p3m2-devices-options.svg`
//! option 1A): every server it has signed in to, as cards above the sign-in form, the current
//! one checked, and "+ Another server…" to type a new address.
//!
//! Dioxus sets the server address once per run (#027), so choosing another server saves it and
//! says "Quit and reopen to switch". A server is remembered (with its instance tag and database
//! ID, for milestone 4's local data) when this desktop signs in to it, and again when it starts
//! already signed in. The web app talks to the server it came from: nothing here shows there.

use dioxus::prelude::*;
#[cfg(any(all(feature = "desktop", not(target_arch = "wasm32")), test))]
use domain::i18n::{MessageArg, messages};

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
use crate::settings::{self, KnownServer};
#[cfg(any(all(feature = "desktop", not(target_arch = "wasm32")), test))]
use crate::ui::{tr, tr_with};

/// "Last used today", "yesterday", or "N days ago", from `now_ms` and `last_ms`.
#[cfg(any(all(feature = "desktop", not(target_arch = "wasm32")), test))]
#[must_use]
pub(crate) fn last_used_label(now_ms: i64, last_ms: i64) -> String {
    const DAY: i64 = 24 * 60 * 60 * 1000;
    match (now_ms - last_ms).max(0) / DAY {
        0 => tr(messages::SERVER_LAST_USED_TODAY),
        1 => tr(messages::SERVER_LAST_USED_YESTERDAY),
        days => tr_with(
            messages::SERVER_LAST_USED_DAYS,
            &[("days", MessageArg::from(days))],
        ),
    }
}

/// Remembers the server this desktop is using (its instance and database ID), once per run
/// after it's signed in.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) async fn remember_once(instance: Option<String>) {
    use std::sync::atomic::{AtomicBool, Ordering};
    static REMEMBERED: AtomicBool = AtomicBool::new(false);
    if !REMEMBERED.swap(true, Ordering::Relaxed) {
        remember(instance).await;
    }
}

/// Nothing to remember on the web (it talks to the server it came from).
#[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
pub(crate) fn remember_once(_instance: Option<String>) -> std::future::Ready<()> {
    std::future::ready(())
}

/// Remembers the server this desktop is using now: its address, `instance`, and database ID
/// (from the handshake), as last used now.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) async fn remember(instance: Option<String>) {
    use domain::time::Clock;

    let address = dioxus_fullstack::get_server_url().to_owned();
    let database_id = crate::api::handshake()
        .await
        .ok()
        .and_then(|handshake| handshake.database_id);
    let Some(data_folder) = crate::data_folder::platform_data_folder() else {
        return;
    };
    let server = KnownServer {
        address,
        instance,
        database_id,
        last_used_ms: crate::logging::SystemClock.now().as_millis(),
    };
    let saved = settings::update(&data_folder, |saved| {
        saved.servers = settings::with_known_server(std::mem::take(&mut saved.servers), server);
    });
    if let Err(error) = saved {
        tracing::warn!(target: domain::log_area::APP, %error, "couldn't remember this server");
    }
    // A first pairing's copy waits for the database ID just remembered (phase-3.md 4c).
    crate::local_copy::open_if_waiting().await;
}

/// The server cards above the sign-in form (desktop only).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ServerList() -> Element {
    use domain::i18n::MessageId;
    use domain::time::Clock;

    let current = dioxus_fullstack::get_server_url().to_owned();
    let data_folder = crate::data_folder::platform_data_folder();
    let saved = data_folder
        .as_deref()
        .map(settings::load)
        .unwrap_or_default();
    let has_address = saved.server_address.is_some();
    let mut servers = saved.servers;
    // The server this run uses, even if it hasn't been signed in to yet.
    if has_address && !servers.iter().any(|known| known.address == current) {
        servers.insert(
            0,
            KnownServer {
                address: current.clone(),
                instance: None,
                database_id: None,
                last_used_ms: 0,
            },
        );
    }
    // No address yet (a new computer): the address field is open from the start.
    let mut adding = use_signal(move || !has_address);
    let mut typed = use_signal(String::new);
    let mut chosen = use_signal(|| None::<String>);
    let mut problem = use_signal(|| None::<MessageId>);
    let now = crate::logging::SystemClock.now().as_millis();

    let mut choose = move |address: String| {
        let result = settings::save_server_address(
            crate::data_folder::platform_data_folder().as_deref(),
            &address,
        );
        match result {
            Ok(saved) => {
                problem.set(None);
                adding.set(false);
                chosen.set(Some(saved.as_str().to_owned()));
            }
            Err(settings::SaveAddressError::Invalid(error)) => problem.set(Some(error.message())),
            Err(settings::SaveAddressError::NoDataFolder | settings::SaveAddressError::Io(_)) => {
                problem.set(Some(messages::CONNECTION_ADDRESS_SAVE_FAILED));
            }
        }
    };

    rsx! {
        div { class: "server-list",
            span { class: "field-label", {tr(messages::SIGN_IN_SERVER)} }
            for server in servers {
                {
                    let on = chosen().map_or(server.address == current, |picked| picked == server.address);
                    let address = server.address.clone();
                    rsx! {
                        button {
                            key: "{server.address}",
                            class: if on { "server-card on" } else { "server-card" },
                            r#type: "button",
                            role: "radio",
                            "aria-checked": on,
                            onclick: move |_| choose(address.clone()),
                            span { class: "server-dot" }
                            span { class: "server-text",
                                span { class: "server-address",
                                    {server.address.clone()}
                                    if let Some(instance) = server.instance.clone() {
                                        span { class: "instance-tag",
                                            {tr_with(messages::INSTANCE_TAG, &[("name", MessageArg::from(instance.as_str()))])}
                                        }
                                    }
                                }
                                if server.last_used_ms > 0 {
                                    span { class: "server-detail", {last_used_label(now, server.last_used_ms)} }
                                }
                            }
                        }
                    }
                }
            }
            if adding() {
                label { class: "field",
                    span { class: "field-label", {tr(messages::SIGN_IN_SERVER_ADDRESS)} }
                    input {
                        class: "field-input",
                        r#type: "url",
                        autocomplete: "off",
                        spellcheck: "false",
                        placeholder: "https://nas.example.ts.net",
                        value: typed(),
                        oninput: move |event| typed.set(event.value()),
                    }
                }
                button {
                    class: "secondary-button",
                    r#type: "button",
                    onclick: move |_| choose(typed()),
                    {tr(messages::SIGN_IN_USE_SERVER)}
                }
            } else {
                button { class: "link-button", r#type: "button", onclick: move |_| adding.set(true),
                    {tr(messages::SIGN_IN_ANOTHER_SERVER)}
                }
            }
            if let Some(message) = problem() {
                p { class: "check-problem", role: "alert", {tr(message)} }
            }
            if let Some(address) = chosen().filter(|address| *address != current) {
                p { class: "check-detail", role: "status",
                    {tr_with(messages::SIGN_IN_REOPEN_TO_SWITCH, &[("address", MessageArg::from(address.as_str()))])}
                }
            }
        }
    }
}

/// Nothing on the web: the web app always talks to the server it came from.
#[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
#[component]
pub(crate) fn ServerList() -> Element {
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    const DAY: i64 = 24 * 60 * 60 * 1000;

    #[test]
    fn last_used_says_today_yesterday_or_how_many_days() {
        let now = 1_762_700_000_000;
        assert_eq!(last_used_label(now, now - 1_000), "Last used today");
        assert_eq!(last_used_label(now, now - DAY - 1), "Last used yesterday");
        assert_eq!(last_used_label(now, now - 5 * DAY), "Last used 5 days ago");
        assert_eq!(
            last_used_label(now, now + DAY),
            "Last used today",
            "a clock ahead"
        );
    }
}
