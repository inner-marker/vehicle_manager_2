//! Messages: the message center (decisions #050, #053).
//!
//! `docs/ui-sketches/p2m3-messages-chosen.svg`: the list on a darker panel, a divider, and the
//! chosen message beside it; on phones, the list, then a page per message (**← Messages**).
//! Opening a message marks it read; **Dismiss** puts it away and opens the next one.

use dioxus::prelude::*;
use domain::api::MarkMessage;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::inbox::{InboxItem, MessageAction, MessageKey, MessageKind, Severity};

use crate::ui::inbox::{mark, severity_and_time, severity_class, use_inbox};
use crate::ui::routes::Route;
use crate::ui::{tr, tr_with};

/// The severity filters, in order: `None` is "All".
const FILTERS: [(Option<Severity>, MessageId); 4] = [
    (None, messages::MESSAGES_FILTER_ALL),
    (Some(Severity::Urgent), messages::MESSAGES_FILTER_URGENT),
    (Some(Severity::Warning), messages::MESSAGES_FILTER_WARNING),
    (Some(Severity::Info), messages::MESSAGES_FILTER_INFO),
];

/// The Messages screen.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Messages() -> Element {
    let context = use_inbox();
    let mut selected = context.selected;
    let mut filter = use_signal(|| None::<Severity>);
    let mut error = use_signal(|| None::<MessageId>);

    let all: Vec<InboxItem> = context
        .inbox
        .read()
        .as_ref()
        .map(|inbox| inbox.items.clone())
        .unwrap_or_default();
    let unread = all.iter().filter(|item| !item.read).count();
    let shown: Vec<InboxItem> = all
        .iter()
        .filter(|item| filter().is_none_or(|severity| item.message.severity == severity))
        .cloned()
        .collect();
    let chosen = selected
        .read()
        .as_ref()
        .and_then(|key| all.iter().find(|item| &item.message.key == key))
        .cloned();

    // Opening a message marks it read (once: the server's answer then says it's read).
    let open = move |item: InboxItem| {
        selected.set(Some(item.message.key.clone()));
        if !item.read {
            spawn(async move {
                error.set(mark(context, item.message.key, MarkMessage::Read).await);
            });
        }
    };
    // Dismiss: put it away, then open the next one in the list (which marks it read), or
    // nothing if it was the last.
    let dismiss = move |key: MessageKey| {
        // `open` only holds signals, so this copy opens the same way.
        let mut open = open;
        spawn(async move {
            let problem = mark(context, key.clone(), MarkMessage::Dismiss).await;
            error.set(problem);
            if problem.is_none() {
                let next = context
                    .inbox
                    .read()
                    .as_ref()
                    .and_then(|inbox| inbox.items.first().cloned());
                match next {
                    Some(item) => open(item),
                    None => selected.set(None),
                }
            }
        });
    };

    let layout = if chosen.is_some() {
        "messages has-selection"
    } else {
        "messages"
    };
    rsx! {
        div { class: layout,
            div { class: "messages-list",
                // This computer's own (refused changes, waiting too long): phase-3.md 5e, 2A.
                super::not_sent::NotSent {}
                div { class: "filter-chips", role: "group",
                    for (severity, label) in FILTERS {
                        button {
                            key: "{label.as_str()}",
                            class: if filter() == severity { "chip-filter on" } else { "chip-filter" },
                            "aria-pressed": filter() == severity,
                            onclick: move |_| filter.set(severity),
                            {tr(label)}
                        }
                    }
                }
                for item in shown {
                    MessageRow {
                        key: "{item.message.key}",
                        selected: chosen.as_ref().is_some_and(|open| open.message.key == item.message.key),
                        on_open: open,
                        item,
                    }
                }
                p { class: "muted-note",
                    {tr_with(messages::MESSAGES_UNREAD, &[("count", MessageArg::from(i64::try_from(unread).unwrap_or(i64::MAX)))])}
                }
            }
            div { class: "messages-detail",
                if let Some(problem) = error() {
                    p { class: "form-error", role: "alert", {tr(problem)} }
                }
                if let Some(item) = chosen {
                    MessageDetail { item, on_back: move |()| selected.set(None), on_dismiss: dismiss }
                } else if all.is_empty() {
                    p { class: "placeholder", {tr(messages::MESSAGES_EMPTY)} }
                } else {
                    p { class: "placeholder", {tr(messages::MESSAGES_SELECT_HINT)} }
                }
            }
        }
    }
}

/// One row of the list: severity stripe and word, title, time, and an unread dot.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MessageRow(item: InboxItem, selected: bool, on_open: EventHandler<InboxItem>) -> Element {
    let severity = severity_class(item.message.severity);
    let class = match (selected, item.read) {
        (true, _) => format!("message-row selected {severity}"),
        (false, false) => format!("message-row unread {severity}"),
        (false, true) => format!("message-row {severity}"),
    };
    let meta = severity_and_time(&item);
    let title = tr(item.message.kind.title());
    rsx! {
        button { class, onclick: move |_| on_open.call(item.clone()),
            span { class: "message-row-text",
                span { class: "message-row-title", {title} }
                span { class: "message-row-meta", {meta} }
            }
        }
    }
}

/// A message's text, with the vehicle's name for a forgotten job or an overdue return (and its
/// date, as the person writes dates).
fn body_text(kind: &MessageKind, units: &domain::preferences::Preferences) -> String {
    match kind {
        MessageKind::ForgottenJob { vehicle, .. } => tr_with(
            kind.body(),
            &[("vehicle", MessageArg::from(vehicle.as_str()))],
        ),
        MessageKind::OverdueReturn {
            vehicle, expected, ..
        } => tr_with(
            kind.body(),
            &[
                ("vehicle", MessageArg::from(vehicle.as_str())),
                (
                    "date",
                    MessageArg::from(super::documents::date_text(expected, units)),
                ),
            ],
        ),
        MessageKind::FailedUnlocks {
            device_name,
            person,
            count,
            ..
        } => tr_with(
            kind.body(),
            &[
                (
                    "device",
                    MessageArg::from(desktop_name(device_name).as_str()),
                ),
                ("person", MessageArg::from(person.as_str())),
                ("count", MessageArg::from(i64::from(*count))),
            ],
        ),
        MessageKind::ClockAhead {
            device_name,
            person,
            minutes,
            ..
        } => tr_with(
            kind.body(),
            &[
                (
                    "device",
                    MessageArg::from(desktop_name(device_name).as_str()),
                ),
                ("person", MessageArg::from(person.as_str())),
                ("minutes", MessageArg::from(*minutes)),
            ],
        ),
        MessageKind::JobFailed { .. }
        | MessageKind::JobSkipped { .. }
        | MessageKind::ServerUnhealthy { .. }
        | MessageKind::PlaceLookupUnavailable => tr(kind.body()),
    }
}

/// The chosen message: its text, the Admin's details, its action, and Dismiss.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MessageDetail(
    item: InboxItem,
    on_back: EventHandler<()>,
    on_dismiss: EventHandler<MessageKey>,
) -> Element {
    let navigator = use_navigator();
    let units = crate::ui::units::use_units();
    let message = item.message.clone();
    let key = message.key.clone();
    rsx! {
        button { class: "link-button messages-back", onclick: move |_| on_back.call(()), {tr(messages::MESSAGES_BACK)} }
        p { class: "message-meta {severity_class(message.severity)}", {severity_and_time(&item)} }
        h2 { class: "message-title", {tr(message.kind.title())} }
        p { class: "message-body", {body_text(&message.kind, &units())} }
        if !message.detail.is_empty() {
            h3 { class: "section-heading", {tr(messages::MESSAGES_DETAILS)} }
            pre { class: "message-detail", {message.detail.clone()} }
        }
        div { class: "form-buttons message-buttons",
            if let Some(action) = message.action {
                button {
                    class: "primary-button",
                    onclick: move |_| match action {
                        MessageAction::OpenDiagnostics => {
                            navigator.push(Route::Diagnostics {});
                        }
                        MessageAction::OpenEvent { event } | MessageAction::OpenSendOut { event } => {
                            navigator.push(Route::EventDetail { event_id: event.to_string() });
                        }
                    },
                    {tr(action.label())}
                }
            }
            button { class: "secondary-button", onclick: move |_| on_dismiss.call(key.clone()),
                {tr(messages::MESSAGES_DISMISS)}
            }
        }
    }
}

/// A desktop's name for a message; an unnamed one is "Desktop app".
fn desktop_name(name: &str) -> String {
    if name.is_empty() {
        tr(messages::DEVICES_DESKTOP)
    } else {
        name.to_owned()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_failed_unlocks_message_names_the_computer_and_its_person() {
        let units = domain::preferences::Preferences::default();
        let kind = |name: &str, count| MessageKind::FailedUnlocks {
            device: domain::test_support::test_id(1),
            device_name: name.to_owned(),
            person: "Sam".into(),
            count,
        };
        let text = body_text(&kind("Garage Mac", 3), &units);
        assert!(
            text.starts_with("Someone typed 3 wrong PINs or passwords on Garage Mac (Sam)"),
            "{text}"
        );
        assert!(text.contains("sign the computer out"), "{text}");
        let one = body_text(&kind("", 1), &units);
        assert!(
            one.contains("1 wrong PIN or password on Desktop app (Sam)"),
            "{one}"
        );
    }
}
