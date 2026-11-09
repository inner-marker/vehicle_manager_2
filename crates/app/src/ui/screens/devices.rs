//! Devices (phase-3.md 2e, decision #073; `docs/ui-sketches/p3m2-devices-options.svg`): the
//! Settings → **Devices** tab (option 2B), where someone sees where they're signed in, renames
//! this device, and signs others out; and the household Admin's **Devices** dialog for a
//! person (option 3A, from the Users screen).

use dioxus::prelude::*;
use domain::api::{DeviceView, DevicesOutcome, UserSummary};
use domain::device::{DeviceKind, DeviceNameError};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::time::Clock;

use crate::api;
use crate::ui::icons::{Icon, IconView};
use crate::ui::{tr, tr_with};

/// A device's title: its name, or a label by kind.
#[must_use]
pub(crate) fn device_title(device: &DeviceView) -> String {
    if let Some(name) = &device.name {
        return name.as_str().to_owned();
    }
    tr(match (device.kind, device.paired) {
        (DeviceKind::Browser, _) => messages::DEVICES_BROWSER,
        (DeviceKind::Desktop, true) => messages::DEVICES_DESKTOP,
        (DeviceKind::Desktop, false) => messages::DEVICES_DESKTOP_OLDER,
    })
}

/// A device's kind, for its second line.
const fn kind_label(device: &DeviceView) -> MessageId {
    match (device.kind, device.paired) {
        (DeviceKind::Browser, _) => messages::DEVICES_KIND_BROWSER,
        (DeviceKind::Desktop, true) => messages::DEVICES_KIND_DESKTOP,
        (DeviceKind::Desktop, false) => messages::DEVICES_KIND_DESKTOP_OLDER,
    }
}

/// When a device was last used, from `now_ms` and `last_ms`: "Active now" (within 5 minutes),
/// minutes, hours, "Yesterday", or days.
#[must_use]
pub(crate) fn seen_label(now_ms: i64, last_ms: i64) -> String {
    const MINUTE: i64 = 60 * 1000;
    const HOUR: i64 = 60 * MINUTE;
    const DAY: i64 = 24 * HOUR;
    let ago = (now_ms - last_ms).max(0);
    if ago < 5 * MINUTE {
        tr(messages::DEVICES_SEEN_NOW)
    } else if ago < HOUR {
        tr_with(
            messages::DEVICES_SEEN_MINUTES,
            &[("minutes", MessageArg::from(ago / MINUTE))],
        )
    } else if ago < DAY {
        tr_with(
            messages::DEVICES_SEEN_HOURS,
            &[("hours", MessageArg::from(ago / HOUR))],
        )
    } else if ago < 2 * DAY {
        tr(messages::DEVICES_SEEN_YESTERDAY)
    } else {
        tr_with(
            messages::DEVICES_SEEN_DAYS,
            &[("days", MessageArg::from(ago / DAY))],
        )
    }
}

/// The problem to show for an answer other than devices or done.
fn problem(outcome: &DevicesOutcome) -> Option<String> {
    let id = match outcome {
        DevicesOutcome::Devices { .. } | DevicesOutcome::Done => return None,
        DevicesOutcome::InvalidName { reason } => match reason {
            DeviceNameError::Empty => messages::DEVICES_NAME_EMPTY,
            DeviceNameError::TooLong => messages::DEVICES_NAME_TOO_LONG,
            DeviceNameError::ControlCharacter => messages::DEVICES_NAME_CONTROL,
        },
        DevicesOutcome::NotSignedIn => messages::API_ERROR_MISSING_CREDENTIAL,
        DevicesOutcome::NotAllowed => messages::USERS_NOT_ALLOWED,
        DevicesOutcome::NotFound | DevicesOutcome::Unavailable => messages::DEVICES_UNAVAILABLE,
    };
    Some(tr(id))
}

fn now_ms() -> i64 {
    crate::logging::SystemClock.now().as_millis()
}

/// One device's row: icon, title (with "This device"), kind and last use, and its button.
#[component]
fn DeviceRow(device: DeviceView, button: Element) -> Element {
    let icon = match device.kind {
        DeviceKind::Desktop => Icon::System,
        DeviceKind::Browser => Icon::Browser,
    };
    let detail = format!(
        "{} · {}",
        tr(kind_label(&device)),
        seen_label(now_ms(), device.last_seen.as_millis())
    );
    rsx! {
        div { class: "device-row",
            IconView { icon }
            div { class: "device-text",
                span { class: "device-title",
                    {device_title(&device)}
                    if device.this_device {
                        span { class: "device-chip", {tr(messages::DEVICES_THIS_DEVICE)} }
                    }
                }
                span { class: "device-detail", {detail} }
            }
            {button}
        }
    }
}

/// Settings → Devices: your devices and browsers.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn DevicesTab() -> Element {
    let mut devices = use_signal(|| None::<Vec<DeviceView>>);
    let mut error = use_signal(|| None::<String>);
    let mut confirming = use_signal(|| false);
    let mut renaming = use_signal(|| None::<String>);
    let mut reload = use_signal(|| 0_u32);

    use_effect(move || {
        let _ = reload();
        spawn(async move {
            match api::devices().await {
                Ok(DevicesOutcome::Devices { devices: list }) => devices.set(Some(list)),
                Ok(other) => error.set(problem(&other)),
                Err(failure) => error
                    .set(Some(tr(
                        crate::ui::screens::problem_from_error(&failure).message()
                    ))),
            }
        });
    });
    let mut done = move |result: Result<DevicesOutcome, ServerFnError>| match result {
        Ok(DevicesOutcome::Done) => {
            error.set(None);
            *reload.write() += 1;
        }
        Ok(other) => error.set(problem(&other)),
        Err(failure) => error.set(Some(tr(
            crate::ui::screens::problem_from_error(&failure).message()
        ))),
    };

    // This device first, then the others as the server sent them (most recently used first).
    let mut list = devices().unwrap_or_default();
    list.sort_by_key(|device| !device.this_device);
    let others = list.iter().filter(|device| !device.this_device).count();
    rsx! {
        p { class: "muted-note", {tr(messages::DEVICES_HELP)} }
        if let Some(message) = error() {
            p { class: "form-error", role: "alert", {message} }
        }
        div { class: "device-list",
            for device in list {
                {
                    let id = device.id;
                    let this = device.this_device;
                    let current = device.name.as_ref().map(|name| name.as_str().to_owned()).unwrap_or_default();
                    let button = if this {
                        rsx! {
                            button { class: "secondary-button", onclick: move |_| renaming.set(Some(current.clone())),
                                {tr(messages::DEVICES_RENAME)}
                            }
                        }
                    } else {
                        rsx! {
                            button {
                                class: "secondary-button",
                                onclick: move |_| { spawn(async move { done(api::sign_out_device(id).await); }); },
                                {tr(messages::DEVICES_SIGN_OUT)}
                            }
                        }
                    };
                    rsx! {
                        DeviceRow { key: "{id}", device, button }
                        if this && let Some(typed) = renaming() {
                            div { class: "device-rename",
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::DEVICES_NAME)} }
                                    input {
                                        class: "field-input",
                                        value: typed,
                                        maxlength: "64",
                                        oninput: move |event| renaming.set(Some(event.value())),
                                    }
                                }
                                div { class: "form-buttons",
                                    button { class: "secondary-button", onclick: move |_| renaming.set(None), {tr(messages::ACTION_CANCEL)} }
                                    button {
                                        class: "primary-button",
                                        onclick: move |_| {
                                            let name = renaming().unwrap_or_default();
                                            spawn(async move {
                                                let result = api::rename_device(id, name).await;
                                                if matches!(result, Ok(DevicesOutcome::Done)) {
                                                    renaming.set(None);
                                                }
                                                done(result);
                                            });
                                        },
                                        {tr(messages::ACTION_SAVE)}
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        if devices().is_some() && others == 0 {
            p { class: "muted-note", {tr(messages::DEVICES_ONLY_THIS)} }
        }
        if others > 0 {
            if confirming() {
                div { class: "device-confirm",
                    p { class: "dialog-text", {tr(messages::DEVICES_SIGN_OUT_OTHERS_CONFIRM)} }
                    div { class: "form-buttons",
                        button { class: "secondary-button", onclick: move |_| confirming.set(false), {tr(messages::ACTION_CANCEL)} }
                        button {
                            class: "danger-button",
                            onclick: move |_| {
                                confirming.set(false);
                                spawn(async move { done(api::sign_out_other_devices().await); });
                            },
                            {tr(messages::DEVICES_SIGN_OUT_OTHERS)}
                        }
                    }
                }
            } else {
                button { class: "secondary-button device-sign-out-others", onclick: move |_| confirming.set(true),
                    {tr(messages::DEVICES_SIGN_OUT_OTHERS)}
                }
            }
        }
    }
}

/// The household Admin's view of one person's devices (a dialog's content; Admin mode).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn UserDevices(
    user: UserSummary,
    on_close: EventHandler<()>,
    on_changed: EventHandler<()>,
) -> Element {
    let mut devices = use_signal(|| None::<Vec<DeviceView>>);
    let mut error = use_signal(|| None::<String>);
    let mut reload = use_signal(|| 0_u32);
    let id = user.id;

    use_effect(move || {
        let _ = reload();
        spawn(async move {
            match api::user_devices(id).await {
                Ok(DevicesOutcome::Devices { devices: list }) => devices.set(Some(list)),
                Ok(other) => error.set(problem(&other)),
                Err(failure) => error
                    .set(Some(tr(
                        crate::ui::screens::problem_from_error(&failure).message()
                    ))),
            }
        });
    });
    let sign_out = move |device: Option<domain::id::DeviceId>| {
        spawn(async move {
            match api::sign_out_user_devices(id, device).await {
                Ok(DevicesOutcome::Done) => {
                    error.set(None);
                    *reload.write() += 1;
                    on_changed.call(());
                }
                Ok(other) => error.set(problem(&other)),
                Err(failure) => error
                    .set(Some(tr(
                        crate::ui::screens::problem_from_error(&failure).message()
                    ))),
            }
        });
    };
    let name = MessageArg::from(user.display_name.as_str());
    let list = devices().unwrap_or_default();
    let any = list.iter().any(|device| !device.this_device);
    rsx! {
        h2 { class: "dialog-title", {tr_with(messages::USERS_DEVICES_TITLE, &[("name", name.clone())])} }
        if let Some(message) = error() {
            p { class: "form-error", role: "alert", {message} }
        }
        div { class: "device-list",
            for device in list {
                {
                    let device_id = device.id;
                    // Not the device the Admin is using (the account menu's Sign out does that).
                    let button = if device.this_device {
                        rsx! {}
                    } else {
                        rsx! {
                            button { class: "secondary-button", onclick: move |_| sign_out(Some(device_id)), {tr(messages::DEVICES_SIGN_OUT)} }
                        }
                    };
                    rsx! { DeviceRow { key: "{device_id}", device, button } }
                }
            }
        }
        if devices().is_some() && !any {
            p { class: "muted-note", {tr(messages::USERS_DEVICES_NONE)} }
        }
        div { class: "form-buttons",
            if any {
                button { class: "danger-button", onclick: move |_| sign_out(None),
                    {tr_with(messages::USERS_DEVICES_SIGN_OUT_ALL, &[("name", name)])}
                }
            }
            button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CLOSE)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::test_support::test_id;
    use domain::time::UnixMillis;

    const MINUTE: i64 = 60 * 1000;

    #[test]
    fn last_use_reads_naturally() {
        let now = 1_762_700_000_000;
        assert_eq!(seen_label(now, now - MINUTE), "Active now");
        assert_eq!(seen_label(now, now - 20 * MINUTE), "20 min ago");
        assert_eq!(seen_label(now, now - 60 * MINUTE), "1 hour ago");
        assert_eq!(seen_label(now, now - 5 * 60 * MINUTE), "5 hours ago");
        assert_eq!(seen_label(now, now - 30 * 60 * MINUTE), "Yesterday");
        assert_eq!(seen_label(now, now - 4 * 24 * 60 * MINUTE), "4 days ago");
    }

    #[test]
    fn a_device_without_a_name_is_labelled_by_kind() {
        let device = |kind, paired| DeviceView {
            id: test_id(1),
            kind,
            name: None,
            paired,
            added_at: UnixMillis::EPOCH,
            last_seen: UnixMillis::EPOCH,
            this_device: false,
        };
        assert_eq!(device_title(&device(DeviceKind::Browser, false)), "Browser");
        assert_eq!(
            device_title(&device(DeviceKind::Desktop, true)),
            "Desktop app"
        );
        assert_eq!(
            device_title(&device(DeviceKind::Desktop, false)),
            "Desktop app (older version)"
        );
        let named = DeviceView {
            name: Some(domain::device::DeviceName::parse("Safari on iPhone").unwrap()),
            ..device(DeviceKind::Browser, false)
        };
        assert_eq!(device_title(&named), "Safari on iPhone");
    }
}
