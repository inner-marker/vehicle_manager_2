//! A lifecycle event's own section of the log form (milestone 9; option A of
//! `docs/ui-sketches/p2m9-lifecycle-options.svg`), shown first: who it was bought from and the
//! price; the shop, work order, due-back date, estimate, and shipping; what the return ends
//! and its final cost; or how it was disposed of, to whom, and the price. What's typed lives in
//! a [`TypedLife`] (built in `life_input`).
//!
//! Saving a disposal asks first ([`DisposeDialog`]): once it's finalized, nothing new can be
//! added to the vehicle (#067). Correcting one offers "It wasn't disposed of after all".

use dioxus::prelude::*;
use domain::api::{AssetView, EventsOutcome, PreferencesOutcome};
use domain::dates::{CalendarDate, utc_to_local};
use domain::event::{Event, EventId};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::lifecycle::{DisposalMethod, LifecycleDetails, Money, Party};
use domain::meter::EventKind;
use domain::preferences::{CurrencyCode, Preferences};

use super::FormError;
use super::documents::date_text;
use super::life_input::TypedLife;
use super::log_form::MoneyInput;
use crate::data;
use crate::ui::{tr, tr_with};

/// A disposal method's `<option>` value.
const fn method_value(method: DisposalMethod) -> &'static str {
    match method {
        DisposalMethod::Sold => "sold",
        DisposalMethod::TradedIn => "traded-in",
        DisposalMethod::Scrapped => "scrapped",
        DisposalMethod::GivenAway => "given-away",
        DisposalMethod::ReturnedToVendor => "returned-to-vendor",
        DisposalMethod::LostOrStolen => "lost-or-stolen",
        DisposalMethod::Other => "other",
    }
}

/// The section's heading, and what the other party and the amount are called, for `kind`.
const fn labels(kind: EventKind) -> (MessageId, MessageId, MessageId) {
    match kind {
        EventKind::SentOut => (
            messages::LIFE_SENT_OUT_HEADING,
            messages::LIFE_SHOP,
            messages::LIFE_ESTIMATE,
        ),
        EventKind::Returned => (
            messages::LIFE_RETURNED_HEADING,
            messages::LIFE_SHOP,
            messages::LIFE_FINAL_COST,
        ),
        EventKind::Disposed => (
            messages::LIFE_DISPOSED_HEADING,
            messages::LIFE_TO_WHOM,
            messages::LIFE_PRICE,
        ),
        EventKind::Acquired
        | EventKind::Fuel
        | EventKind::Reading
        | EventKind::Maintenance
        | EventKind::Repair => (
            messages::LIFE_ACQUIRED_HEADING,
            messages::LIFE_FROM,
            messages::LIFE_PRICE,
        ),
    }
}

/// The lifecycle section for `kind`, editing `life`; `correcting` offers a disposal's withdrawal.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn LifePart(
    kind: EventKind,
    life: Signal<TypedLife>,
    currency: CurrencyCode,
    units: Preferences,
    correcting: bool,
    problem: Option<String>,
) -> Element {
    let (heading, party_label, amount_label) = labels(kind);
    let typed = life();
    let mut set = move |change: &dyn Fn(&mut TypedLife)| {
        let mut next = life();
        change(&mut next);
        life.set(next);
    };
    let withdrawn = typed.withdrawn;
    rsx! {
        div { class: "log-item life-part",
            h3 { class: "section-heading", {tr(heading).to_uppercase()} }
            if kind == EventKind::Returned {
                ReturnOf { sent_out: typed.sent_out, units }
            }
            if kind == EventKind::Disposed {
                if correcting {
                    label { class: "check-row",
                        input { r#type: "checkbox", checked: withdrawn, onchange: move |event| set(&|life| life.withdrawn = event.checked()) }
                        span { {tr(messages::LIFE_WITHDRAWN)} }
                    }
                }
                if !withdrawn {
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LIFE_HOW)} }
                        select {
                            class: "field-input",
                            onchange: move |event| {
                                let value = event.value();
                                set(&|life| life.method = DisposalMethod::ALL.into_iter().find(|method| method_value(*method) == value));
                            },
                            option { value: "", selected: typed.method.is_none(), disabled: true, {tr(messages::LIFE_CHOOSE)} }
                            for method in DisposalMethod::ALL {
                                option { key: "{method_value(method)}", value: method_value(method), selected: typed.method == Some(method), {tr(method.label())} }
                            }
                        }
                    }
                }
            }
            if kind != EventKind::Returned && !withdrawn {
                div { class: "field-row",
                    label { class: "field",
                        span { class: "field-label", {tr(party_label)} }
                        input { class: "field-input", autocomplete: "off", value: typed.party.clone(), oninput: move |event| set(&|life| life.party = event.value()) }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LIFE_CONTACT)} }
                        input { class: "field-input", autocomplete: "off", value: typed.contact.clone(), oninput: move |event| set(&|life| life.contact = event.value()) }
                    }
                }
            }
            if kind == EventKind::SentOut {
                div { class: "field-row",
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LIFE_WORK_ORDER)} }
                        input { class: "field-input", autocomplete: "off", value: typed.work_order.clone(), oninput: move |event| set(&|life| life.work_order = event.value()) }
                    }
                    label { class: "field",
                        span { class: "field-label", {tr(messages::LIFE_DUE_BACK)} }
                        input { class: "field-input", r#type: "date", value: typed.expected_back.clone(), oninput: move |event| set(&|life| life.expected_back = event.value()) }
                    }
                }
            }
            if !withdrawn {
                label { class: "field",
                    span { class: "field-label", {tr(amount_label)} }
                    MoneyInput { currency, value: typed.amount.clone(), on_input: move |text: String| set(&|life| life.amount.clone_from(&text)) }
                }
            }
            if kind == EventKind::SentOut {
                details { class: "fold", open: !typed.carrier.is_empty() || !typed.tracking.is_empty(),
                    summary { class: "fold-head",
                        span { class: "fold-toggle", "aria-hidden": "true", "▸" }
                        span { class: "fold-text",
                            span { class: "fold-title", {tr(messages::LIFE_SHIPPING)} }
                            span { class: "fold-summary",
                                if typed.carrier.is_empty() && typed.tracking.is_empty() { {tr(messages::LIFE_SHIPPING_NONE)} } else { {format!("{} {}", typed.carrier, typed.tracking).trim().to_owned()} }
                            }
                        }
                    }
                    div { class: "fold-body field-row",
                        label { class: "field",
                            span { class: "field-label", {tr(messages::LIFE_CARRIER)} }
                            input { class: "field-input", autocomplete: "off", value: typed.carrier.clone(), oninput: move |event| set(&|life| life.carrier = event.value()) }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::LIFE_TRACKING)} }
                            input { class: "field-input", autocomplete: "off", value: typed.tracking, oninput: move |event| set(&|life| life.tracking = event.value()) }
                        }
                    }
                }
            }
            FormError { message: problem }
        }
    }
}

/// A lifecycle event in a few words, for its Timeline row: "Joe's Auto · WO-1182 · due back
/// 20 Nov 2026 · $450.00", "Sold · to a neighbour · $8,000.00".
pub(super) fn life_summary(details: &LifecycleDetails, units: &Preferences) -> Vec<String> {
    let amount =
        |money: Option<Money>| money.map(|money| crate::ui::money(money.minor, money.currency));
    let named = |id: MessageId, party: Option<&Party>| {
        party.map(|party| tr_with(id, &[("name", MessageArg::from(party.name.as_str()))]))
    };
    let mut parts = Vec::new();
    match details {
        LifecycleDetails::Acquired(acquired) => {
            parts.extend(named(messages::LIFE_SUMMARY_FROM, acquired.from.as_ref()));
            parts.extend(amount(acquired.price));
        }
        LifecycleDetails::SentOut(sent) => {
            parts.push(sent.shop.name.clone());
            parts.extend(sent.work_order.clone());
            parts.extend(sent.expected_back.as_ref().map(|date| {
                tr_with(
                    messages::LIFE_SUMMARY_DUE_BACK,
                    &[("date", MessageArg::from(date_text(date, units)))],
                )
            }));
            parts.extend(amount(sent.estimate));
        }
        LifecycleDetails::Returned(back) => parts.extend(amount(back.cost)),
        LifecycleDetails::Disposed(disposal) => {
            if disposal.withdrawn {
                parts.push(tr(messages::LIFE_WITHDRAWN));
            } else {
                parts.push(tr(disposal.method.label()));
                parts.extend(named(messages::LIFE_SUMMARY_TO, disposal.to.as_ref()));
                parts.extend(amount(disposal.price));
            }
        }
    }
    parts
}

/// A lifecycle event's section on its page: its heading, and each detail as a labelled row.
pub(super) fn life_rows(
    kind: EventKind,
    details: &LifecycleDetails,
    units: &Preferences,
) -> (MessageId, Vec<(String, String)>) {
    let (heading, party_label, amount_label) = labels(kind);
    let mut rows: Vec<(String, String)> = Vec::new();
    let mut push = |label: MessageId, value: Option<String>| {
        if let Some(value) = value.filter(|value| !value.is_empty()) {
            rows.push((tr(label), value));
        }
    };
    let party = |party: Option<&Party>| {
        party.map(|party| match &party.contact {
            Some(contact) => format!("{} · {contact}", party.name),
            None => party.name.clone(),
        })
    };
    let amount =
        |money: Option<Money>| money.map(|money| crate::ui::money(money.minor, money.currency));
    match details {
        LifecycleDetails::Acquired(acquired) => {
            push(party_label, party(acquired.from.as_ref()));
            push(amount_label, amount(acquired.price));
        }
        LifecycleDetails::SentOut(sent) => {
            push(party_label, party(Some(&sent.shop)));
            push(messages::LIFE_WORK_ORDER, sent.work_order.clone());
            push(
                messages::LIFE_DUE_BACK,
                sent.expected_back
                    .as_ref()
                    .map(|date| date_text(date, units)),
            );
            push(amount_label, amount(sent.estimate));
            push(messages::LIFE_CARRIER, sent.carrier.clone());
            push(messages::LIFE_TRACKING, sent.tracking.clone());
        }
        LifecycleDetails::Returned(back) => push(amount_label, amount(back.cost)),
        LifecycleDetails::Disposed(disposal) => {
            if disposal.withdrawn {
                push(messages::LIFE_HOW, Some(tr(messages::LIFE_WITHDRAWN)));
            } else {
                push(messages::LIFE_HOW, Some(tr(disposal.method.label())));
                push(party_label, party(disposal.to.as_ref()));
                push(amount_label, amount(disposal.price));
            }
        }
    }
    (heading, rows)
}

/// Under the vehicle page's header, on every tab (#067; option A of the sketch): while it's out
/// for service, where and until when, with **Record return** for whoever may; once a disposal
/// is recorded, how it left and what that means for its records.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn LifeBanner(view: AssetView, may_return: bool) -> Element {
    let standing = view.standing;
    let shown = standing.disposal.or(standing.sent_out);
    let mut loaded = use_signal(|| None::<(Event, Preferences)>);
    use_effect(use_reactive!(|shown| {
        spawn(async move {
            let Some(id) = shown else {
                loaded.set(None);
                return;
            };
            let units = match data::get_preferences().await {
                Ok(PreferencesOutcome::Preferences { view }) => view.effective,
                _ => Preferences::default(),
            };
            if let Ok(EventsOutcome::Event { view }) = data::event_view(id).await {
                loaded.set(Some((view.event, units)));
            }
        });
    }));
    let navigator = use_navigator();
    let Some((event, units)) = loaded().filter(|_| shown.is_some()) else {
        return rsx! {};
    };
    let day = |at| {
        utc_to_local(at, &event.dates.zone)
            .ok()
            .and_then(|(date, _)| CalendarDate::parse(&date).ok())
            .map(|date| date_text(&date, &units))
            .unwrap_or_default()
    };
    let since = day(event.dates.start);
    let asset = view.id;
    match &event.lifecycle {
        Some(LifecycleDetails::Disposed(disposal)) => {
            let how = tr(disposal.method.label());
            let title = match &disposal.to {
                Some(to) => tr_with(
                    messages::LIFE_BANNER_DISPOSED_TO,
                    &[
                        ("how", MessageArg::from(how.as_str())),
                        ("date", MessageArg::from(since.as_str())),
                        ("name", MessageArg::from(to.name.as_str())),
                    ],
                ),
                None => tr_with(
                    messages::LIFE_BANNER_DISPOSED,
                    &[
                        ("how", MessageArg::from(how.as_str())),
                        ("date", MessageArg::from(since.as_str())),
                    ],
                ),
            };
            let note = tr(if standing.takes_new_records {
                messages::LIFE_BANNER_DISPOSED_DRAFT
            } else {
                messages::LIFE_BANNER_DISPOSED_NOTE
            });
            rsx! {
                div { class: "life-banner disposed", role: "status",
                    span { class: "life-banner-icon", "aria-hidden": "true", "🏁" }
                    span { class: "life-banner-text",
                        strong { {title} }
                        span { {note} }
                    }
                }
            }
        }
        Some(LifecycleDetails::SentOut(sent)) => {
            let late = sent.expected_back.as_ref().and_then(|expected| {
                let today = super::documents::today(&event.dates.zone)?;
                super::documents::days_between(expected, &today).filter(|days| *days > 0)
            });
            let title = match &sent.expected_back {
                Some(expected) => tr_with(
                    messages::LIFE_BANNER_OUT_DUE,
                    &[
                        ("shop", MessageArg::from(sent.shop.name.as_str())),
                        ("date", MessageArg::from(since.as_str())),
                        ("due", MessageArg::from(date_text(expected, &units))),
                    ],
                ),
                None => tr_with(
                    messages::LIFE_BANNER_OUT,
                    &[
                        ("shop", MessageArg::from(sent.shop.name.as_str())),
                        ("date", MessageArg::from(since.as_str())),
                    ],
                ),
            };
            let mut details: Vec<String> = Vec::new();
            if let Some(days) = late {
                details.push(tr_with(
                    messages::LIFE_BANNER_LATE,
                    &[("days", MessageArg::from(days))],
                ));
            }
            details.extend(sent.work_order.clone());
            details.extend(sent.estimate.map(|money| {
                tr_with(
                    messages::LIFE_BANNER_ESTIMATE,
                    &[(
                        "amount",
                        MessageArg::from(crate::ui::money(money.minor, money.currency)),
                    )],
                )
            }));
            rsx! {
                div { class: if late.is_some() { "life-banner out late" } else { "life-banner out" }, role: "status",
                    span { class: "life-banner-icon", "aria-hidden": "true", "🚚" }
                    span { class: "life-banner-text",
                        strong { {title} }
                        span { {details.join(" · ")} }
                    }
                    if may_return {
                        button {
                            class: "secondary-button",
                            r#type: "button",
                            onclick: move |_| {
                                navigator.push(crate::ui::routes::Route::LogForm {
                                    asset_id: asset.to_string(),
                                    kind: super::vehicle_setup::event_value(EventKind::Returned).to_owned(),
                                });
                            },
                            {tr(messages::LIFE_RECORD_RETURN)}
                        }
                    }
                }
            }
        }
        Some(LifecycleDetails::Acquired(_) | LifecycleDetails::Returned(_)) | None => rsx! {},
    }
}

/// "Back from Joe's Auto, sent out on 14 Nov 2026": the send-out a return ends.
#[component]
fn ReturnOf(sent_out: Option<EventId>, units: Preferences) -> Element {
    let mut text = use_signal(|| None::<String>);
    use_effect(use_reactive!(|sent_out| {
        spawn(async move {
            let Some(id) = sent_out else {
                text.set(None);
                return;
            };
            if let Ok(EventsOutcome::Event { view }) = data::event_view(id).await
                && let Some(LifecycleDetails::SentOut(sent)) = &view.event.lifecycle
            {
                let day = utc_to_local(view.event.dates.start, &view.event.dates.zone)
                    .ok()
                    .and_then(|(date, _)| CalendarDate::parse(&date).ok())
                    .map(|date| date_text(&date, &units))
                    .unwrap_or_default();
                text.set(Some(tr_with(
                    messages::LIFE_RETURN_OF,
                    &[
                        ("shop", MessageArg::from(sent.shop.name.as_str())),
                        ("date", MessageArg::from(day)),
                    ],
                )));
            }
        });
    }));
    rsx! {
        if let Some(text) = text() {
            p { class: "life-return-of", {text} }
        }
    }
}

/// Before saving a disposal: once it's finalized, nothing new can be added to `vehicle`.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn DisposeDialog(
    vehicle: String,
    busy: bool,
    on_save: EventHandler<()>,
    on_close: EventHandler<()>,
) -> Element {
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::LIFE_DISPOSE_TITLE)} }
                p { class: "dialog-text", {tr_with(messages::LIFE_DISPOSE_TEXT, &[("vehicle", MessageArg::from(vehicle.as_str()))])} }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "button", disabled: busy, onclick: move |_| on_save.call(()),
                        {tr(if busy { messages::LOG_SAVING } else { messages::LIFE_DISPOSE_SAVE })}
                    }
                }
            }
        }
    }
}
