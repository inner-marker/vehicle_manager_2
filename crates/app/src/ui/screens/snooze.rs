//! Snoozing a due message (Phase 5 milestone 3; decision #083; sketch
//! `docs/ui-sketches/p5m3-messages-options.svg`, 1A and 4A).
//!
//! **Snooze ▾** opens quick choices: 1 day, 1 week, 1 month, an amount on the item's meter
//! fitted to it (500 mi, 500 km, 10 h), the two together ("1 week or 500 mi"), and
//! **Custom…** (days and/or an amount, whichever comes first). An item without a meter that
//! counts up snoozes by time only. The server works the snooze out from the meter's latest
//! reading, so the amount here is how much more.

use dioxus::prelude::*;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::inbox::{DueNotice, Snooze, SnoozeRequest};
use domain::meter::{Meter, MeterDimension};
use domain::quantity::{parse_reading, show_reading};
use domain::time_utils::format_utc_minute;

use crate::ui::{tr, tr_with};

/// The item's snooze meter, if its message has one.
fn snooze_meter(notice: &DueNotice) -> Option<&Meter> {
    let id = notice.snooze_meter?;
    notice
        .meters
        .iter()
        .find_map(|(meter, found)| (*meter == id).then_some(found))
}

/// An amount on `meter`, in its stored unit, shown with its unit ("500 mi").
fn amount_text(amount: i64, meter: &Meter) -> String {
    format!(
        "{} {}",
        show_reading(amount, meter.unit, meter.precision),
        tr(meter.unit.abbr())
    )
}

/// The quick amount for `meter`: 500 of a distance's unit, or 10 hours; none for a
/// percentage.
fn quick_amount(meter: &Meter) -> Option<i64> {
    let typed = match meter.dimension {
        MeterDimension::Distance => "500",
        MeterDimension::Duration => "10",
        MeterDimension::Percentage => return None,
    };
    parse_reading(typed, meter.unit, meter.precision).ok()
}

/// The quick choices for an item with `meter` (its snooze meter, if any), each with its label.
pub(super) fn quick_choices(meter: Option<&Meter>) -> Vec<(String, SnoozeRequest)> {
    let days = |days| SnoozeRequest {
        days: Some(days),
        amount: None,
    };
    let mut choices = vec![
        (tr(messages::SNOOZE_DAY), days(1)),
        (tr(messages::SNOOZE_WEEK), days(7)),
        (tr(messages::SNOOZE_MONTH), days(30)),
    ];
    if let Some((meter, amount)) = meter.and_then(|meter| Some((meter, quick_amount(meter)?))) {
        let shown = amount_text(amount, meter);
        choices.push((
            shown.clone(),
            SnoozeRequest {
                days: None,
                amount: Some(amount),
            },
        ));
        choices.push((
            tr_with(
                messages::SNOOZE_EITHER,
                &[
                    ("time", MessageArg::from(tr(messages::SNOOZE_WEEK).as_str())),
                    ("amount", MessageArg::from(shown.as_str())),
                ],
            ),
            SnoozeRequest {
                days: Some(7),
                amount: Some(amount),
            },
        ));
    }
    choices
}

/// A custom snooze from typed `days` and `amount` (on `meter`, in its unit); either may be
/// empty, not both.
///
/// # Errors
///
/// [`messages::MESSAGES_BAD_SNOOZE`] if neither is given, or either isn't a usable number.
pub(super) fn custom_request(
    days: &str,
    amount: &str,
    meter: Option<&Meter>,
) -> Result<SnoozeRequest, MessageId> {
    let bad = messages::MESSAGES_BAD_SNOOZE;
    let days = match days.trim() {
        "" => None,
        typed => Some(
            typed
                .parse::<u16>()
                .ok()
                .filter(|days| (1..=domain::inbox::MAX_SNOOZE_DAYS).contains(days))
                .ok_or(bad)?,
        ),
    };
    let amount = match (amount.trim(), meter) {
        ("", _) => None,
        (typed, Some(meter)) => Some(
            parse_reading(typed, meter.unit, meter.precision)
                .ok()
                .filter(|amount| *amount > 0)
                .ok_or(bad)?,
        ),
        (_, None) => return Err(bad),
    };
    if days.is_none() && amount.is_none() {
        return Err(bad);
    }
    Ok(SnoozeRequest { days, amount })
}

/// "Snoozed until 2026-11-23 08:12 UTC or 85,210 mi".
pub(super) fn snooze_text(snooze: &Snooze, notice: Option<&DueNotice>) -> String {
    let mut parts = Vec::new();
    if let Some(until) = snooze.until {
        parts.push(tr_with(
            messages::MESSAGES_TIME_UTC,
            &[("time", MessageArg::from(format_utc_minute(until)))],
        ));
    }
    if let Some(wait) = snooze.meter {
        let meter = notice.and_then(|notice| {
            notice
                .meters
                .iter()
                .find_map(|(id, meter)| (*id == wait.meter).then_some(meter))
        });
        parts.push(meter.map_or_else(|| wait.at.to_string(), |meter| amount_text(wait.at, meter)));
    }
    tr_with(
        messages::MESSAGES_SNOOZED_UNTIL,
        &[(
            "when",
            MessageArg::from(parts.join(&format!(" {} ", tr(messages::DUE_OR)))),
        )],
    )
}

/// **Snooze ▾**: the quick choices, and **Custom…** with its fields.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn SnoozeMenu(notice: DueNotice, on_snooze: EventHandler<SnoozeRequest>) -> Element {
    let mut open = use_signal(|| false);
    let mut custom = use_signal(|| false);
    let mut days = use_signal(String::new);
    let mut amount = use_signal(String::new);
    let mut problem = use_signal(|| None::<MessageId>);
    let meter = snooze_meter(&notice).cloned();
    let choices = quick_choices(meter.as_ref());
    let meter_label = meter.as_ref().map(|meter| {
        tr_with(
            messages::SNOOZE_OR_METER,
            &[
                ("meter", MessageArg::from(meter.name.as_str())),
                ("unit", MessageArg::from(tr(meter.unit.abbr()).as_str())),
            ],
        )
    });
    let submit = move |_| match custom_request(&days(), &amount(), meter.as_ref()) {
        Ok(request) => {
            problem.set(None);
            custom.set(false);
            on_snooze.call(request);
        }
        Err(message) => problem.set(Some(message)),
    };
    rsx! {
        div { class: "snooze",
            button {
                class: "secondary-button",
                "aria-expanded": open(),
                "aria-haspopup": "menu",
                onclick: move |_| open.set(!open()),
                {format!("{} ▾", tr(messages::SNOOZE))}
            }
            if open() {
                div { class: "snooze-menu", role: "menu",
                    for (label, request) in choices {
                        button {
                            key: "{label}",
                            class: "snooze-choice",
                            role: "menuitem",
                            onclick: move |_| {
                                open.set(false);
                                on_snooze.call(request);
                            },
                            {label}
                        }
                    }
                    button {
                        class: "snooze-choice link-button",
                        role: "menuitem",
                        onclick: move |_| {
                            open.set(false);
                            custom.set(true);
                        },
                        {tr(messages::SNOOZE_CUSTOM)}
                    }
                }
            }
            if custom() {
                div { class: "snooze-custom",
                    label { class: "field",
                        span { class: "field-label", {tr(messages::SNOOZE_DAYS)} }
                        input {
                            r#type: "text",
                            inputmode: "numeric",
                            value: "{days}",
                            oninput: move |event| days.set(event.value()),
                        }
                    }
                    if let Some(label) = meter_label {
                        label { class: "field",
                            span { class: "field-label", {label} }
                            input {
                                r#type: "text",
                                inputmode: "decimal",
                                value: "{amount}",
                                oninput: move |event| amount.set(event.value()),
                            }
                        }
                    }
                    button { class: "secondary-button", onclick: submit, {tr(messages::SNOOZE)} }
                    p { class: "muted-note", {tr(messages::SNOOZE_HINT)} }
                    if let Some(message) = problem() {
                        p { class: "form-error", role: "alert", {tr(message)} }
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::label::Label;
    use domain::meter::{MeterDirection, MeterUnit};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;

    use super::*;

    const MILE: i64 = 1_609_344;

    fn meter(dimension: MeterDimension, unit: MeterUnit) -> Meter {
        Meter {
            key: domain::custom_field::FieldKey::parse("odometer").unwrap(),
            formula: None,
            asset: test_id(1),
            name: Label::parse("Odometer").unwrap(),
            dimension,
            unit,
            precision: 0,
            description: None,
            required_for: vec![],
            direction: MeterDirection::Up,
            reset_to: None,
        }
    }

    #[test]
    fn quick_choices_fit_the_items_meter() {
        let labels = |meter: Option<&Meter>| -> Vec<String> {
            quick_choices(meter)
                .into_iter()
                .map(|(label, _)| label)
                .collect()
        };
        assert_eq!(labels(None), ["1 day", "1 week", "1 month"], "time only");
        let odometer = meter(MeterDimension::Distance, MeterUnit::Miles);
        assert_eq!(
            labels(Some(&odometer)),
            ["1 day", "1 week", "1 month", "500 mi", "1 week or 500 mi"]
        );
        let either = quick_choices(Some(&odometer)).pop().unwrap().1;
        assert_eq!(either.days, Some(7));
        assert_eq!(either.amount, Some(500 * MILE / 1_000));
        let hours = meter(MeterDimension::Duration, MeterUnit::Hours);
        assert_eq!(labels(Some(&hours))[3], "10 h");
        let percent = meter(MeterDimension::Percentage, MeterUnit::Percent);
        assert_eq!(
            labels(Some(&percent)).len(),
            3,
            "no amount for a percentage"
        );
    }

    #[test]
    fn a_custom_snooze_needs_days_or_an_amount() {
        let odometer = meter(MeterDimension::Distance, MeterUnit::Miles);
        let both = custom_request("10", "800", Some(&odometer)).unwrap();
        assert_eq!(both.days, Some(10));
        assert_eq!(both.amount, Some(800 * MILE / 1_000));
        assert_eq!(
            custom_request(" ", "800", Some(&odometer)).unwrap().days,
            None
        );
        let bad = Err(messages::MESSAGES_BAD_SNOOZE);
        assert_eq!(custom_request("", "", Some(&odometer)), bad);
        assert_eq!(custom_request("0", "", None), bad);
        assert_eq!(custom_request("400", "", None), bad, "over a year");
        assert_eq!(custom_request("", "0", Some(&odometer)), bad);
        assert_eq!(custom_request("", "800", None), bad, "no meter");
        assert_eq!(custom_request("a week", "", None), bad);
    }

    #[test]
    fn a_snooze_says_when_it_wakes() {
        let odometer = meter(MeterDimension::Distance, MeterUnit::Miles);
        let notice = DueNotice {
            asset: test_id(1),
            vehicle: "Outback".into(),
            subject: domain::api::DueSubject::Requirement {
                id: test_id(20),
                name: Label::parse("Tire rotation").unwrap(),
            },
            due: domain::due::Due {
                step: domain::due::DueStep::DueSoon,
                rules: vec![],
                date: None,
                not_done_yet: false,
            },
            meters: vec![(test_id(10), odometer)],
            snooze_meter: Some(test_id(10)),
        };
        let snooze = Snooze {
            until: Some(UnixMillis::from_millis(1_700_000_000_000)),
            meter: Some(domain::inbox::SnoozeMeter {
                meter: test_id(10),
                at: 85_210 * MILE / 1_000,
            }),
        };
        assert_eq!(
            snooze_text(&snooze, Some(&notice)),
            "Snoozed until 2023-11-14 22:13 UTC or 85,210 mi"
        );
    }
}
