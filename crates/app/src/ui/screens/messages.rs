//! Messages: the message center (decisions #050, #053).
//!
//! `docs/ui-sketches/p2m3-messages-chosen.svg`: the list on a darker panel, a divider, and the
//! chosen message beside it; on phones, the list, then a page per message (**← Messages**).
//! Opening a message marks it read; **Dismiss** puts it away and opens the next one.
//!
//! **Due messages** (#083; `docs/ui-sketches/p5m3-messages-options.svg`, 1A and 4A) are titled
//! by vehicle and item ("Outback: Engine oil & filter"), with the step's chip and what's left,
//! and offer **Snooze ▾**. Snoozed ones wait under the **Snoozed** filter, saying when they
//! wake, with **Wake now**.

use dioxus::prelude::*;
use domain::api::MarkMessage;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::inbox::{DueNotice, InboxItem, MessageAction, MessageKey, MessageKind, Severity};

use super::due_view::{DueChip, left_text, step_class};
use super::snooze::{SnoozeMenu, snooze_text};

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
    // The Snoozed filter, instead of a severity.
    let mut asleep_shown = use_signal(|| false);
    let mut error = use_signal(|| None::<MessageId>);

    let (all, asleep): (Vec<InboxItem>, Vec<InboxItem>) = context
        .inbox
        .read()
        .as_ref()
        .map(|inbox| (inbox.items.clone(), inbox.snoozed.clone()))
        .unwrap_or_default();
    let unread = all.iter().filter(|item| !item.read).count();
    let shown: Vec<InboxItem> = if asleep_shown() {
        asleep.clone()
    } else {
        all.iter()
            .filter(|item| filter().is_none_or(|severity| item.message.severity == severity))
            .cloned()
            .collect()
    };
    let chosen = selected
        .read()
        .as_ref()
        .and_then(|key| {
            all.iter()
                .chain(&asleep)
                .find(|item| &item.message.key == key)
        })
        .cloned();
    // Snooze and Wake now: the server's answer shows where the message went.
    let act = move |(key, action): (MessageKey, MarkMessage)| {
        spawn(async move {
            error.set(mark(context, key, action).await);
        });
    };

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
                            class: if !asleep_shown() && filter() == severity { "chip-filter on" } else { "chip-filter" },
                            "aria-pressed": !asleep_shown() && filter() == severity,
                            onclick: move |_| {
                                asleep_shown.set(false);
                                filter.set(severity);
                            },
                            {tr(label)}
                        }
                    }
                    if !asleep.is_empty() || asleep_shown() {
                        button {
                            class: if asleep_shown() { "chip-filter on" } else { "chip-filter" },
                            "aria-pressed": asleep_shown(),
                            onclick: move |_| asleep_shown.set(true),
                            {tr_with(messages::MESSAGES_FILTER_SNOOZED, &[("count", MessageArg::from(i64::try_from(asleep.len()).unwrap_or(i64::MAX)))])}
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
                    MessageDetail { item, on_back: move |()| selected.set(None), on_dismiss: dismiss, on_mark: act }
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
    let meta = item.snooze.as_ref().map_or_else(
        || severity_and_time(&item),
        |snooze| snooze_text(snooze, notice_of(&item.message.kind)),
    );
    let title = title_text(&item.message.kind);
    rsx! {
        button { class, onclick: move |_| on_open.call(item.clone()),
            span { class: "message-row-text",
                span { class: "message-row-title", {title} }
                span { class: "message-row-meta", {meta} }
            }
        }
    }
}

/// A due message's notice, if it is one.
const fn notice_of(kind: &MessageKind) -> Option<&DueNotice> {
    if let MessageKind::Due(notice) = kind {
        Some(notice)
    } else {
        None
    }
}

/// A due message's item: a requirement's name, or a document's type.
fn item_name(notice: &DueNotice) -> String {
    match &notice.subject {
        domain::api::DueSubject::Requirement { name, .. } => name.as_str().to_owned(),
        domain::api::DueSubject::Document { doc_type, .. } => super::documents::type_name(doc_type),
    }
}

/// A message's title: its kind's, or for a due message its vehicle and item.
fn title_text(kind: &MessageKind) -> String {
    notice_of(kind).map_or_else(
        || tr(kind.title()),
        |notice| {
            tr_with(
                messages::INBOX_DUE_ROW,
                &[
                    ("vehicle", MessageArg::from(notice.vehicle.as_str())),
                    ("item", MessageArg::from(item_name(notice).as_str())),
                ],
            )
        },
    )
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
        MessageKind::Due(notice) => tr_with(
            kind.body(),
            &[
                ("vehicle", MessageArg::from(notice.vehicle.as_str())),
                ("item", MessageArg::from(item_name(notice).as_str())),
                (
                    "left",
                    MessageArg::from(left_text(&notice.due, &notice.meters, units).as_str()),
                ),
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
    on_mark: EventHandler<(MessageKey, MarkMessage)>,
) -> Element {
    let navigator = use_navigator();
    let units = crate::ui::units::use_units();
    let message = item.message.clone();
    let key = message.key.clone();
    let notice = notice_of(&message.kind).cloned();
    let (snooze_key, wake_key) = (key.clone(), key.clone());
    rsx! {
        button { class: "link-button messages-back", onclick: move |_| on_back.call(()), {tr(messages::MESSAGES_BACK)} }
        p { class: "message-meta {severity_class(message.severity)}", {severity_and_time(&item)} }
        div { class: "message-title-row",
            h2 { class: "message-title", {title_text(&message.kind)} }
            if let Some(notice) = &notice {
                DueChip { step: notice.due.step }
            }
        }
        if let Some(notice) = &notice {
            p { class: "message-body due-left {step_class(notice.due.step)}",
                {left_text(&notice.due, &notice.meters, &units())}
            }
        } else {
            p { class: "message-body", {body_text(&message.kind, &units())} }
        }
        if let Some(snooze) = &item.snooze {
            p { class: "muted-note", {snooze_text(snooze, notice.as_ref())} }
        }
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
                        MessageAction::OpenMaintenance { asset } => {
                            navigator.push(Route::VehicleTab { asset_id: asset.to_string(), tab: "maintenance".into() });
                        }
                        MessageAction::OpenDocuments { asset } => {
                            navigator.push(Route::VehicleTab { asset_id: asset.to_string(), tab: "documents".into() });
                        }
                    },
                    {tr(action.label())}
                }
            }
            if item.snooze.is_some() {
                button { class: "secondary-button", onclick: move |_| on_mark.call((wake_key.clone(), MarkMessage::Wake)),
                    {tr(messages::MESSAGES_WAKE)}
                }
            } else if let Some(notice) = notice {
                SnoozeMenu {
                    notice,
                    on_snooze: move |request| on_mark.call((snooze_key.clone(), MarkMessage::Snooze { request })),
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
    fn a_due_message_names_its_vehicle_and_item_and_says_whats_left() {
        use domain::due::{Due, DueStep, Left, RuleDue};
        use domain::label::Label;
        use domain::meter::{Meter, MeterDimension, MeterDirection, MeterUnit};
        use domain::test_support::test_id;
        let odometer = Meter {
            key: domain::custom_field::FieldKey::parse("odometer").unwrap(),
            formula: None,
            asset: test_id(1),
            name: Label::parse("Odometer").unwrap(),
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            precision: 0,
            description: None,
            required_for: vec![],
            direction: MeterDirection::Up,
            reset_to: None,
        };
        let kind = MessageKind::Due(DueNotice {
            asset: test_id(1),
            vehicle: "Outback".into(),
            subject: domain::api::DueSubject::Requirement {
                id: test_id(20),
                name: Label::parse("Engine oil & filter").unwrap(),
            },
            due: Due {
                step: DueStep::DueSoon,
                rules: vec![RuleDue {
                    step: DueStep::DueSoon,
                    left: Left::Meter {
                        meter: test_id(10),
                        left: 965_606,
                        projected: None,
                    },
                    since: None,
                }],
                date: None,
                not_done_yet: false,
            },
            meters: vec![(test_id(10), odometer)],
            snooze_meter: Some(test_id(10)),
        });
        assert_eq!(title_text(&kind), "Outback: Engine oil & filter");
        let units = domain::preferences::Preferences::default();
        assert_eq!(
            body_text(&kind, &units),
            "Outback: Engine oil & filter, in 600 mi."
        );
    }

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
