//! Settings → HOUSEHOLD → Labor rates (phase-6.md 2g; sketch part 4 of
//! `docs/ui-sketches/p6m2-faster-logging-options.svg`; decision #089): the household's labor
//! rate, then its shops, each with a contact and its own rate if it differs.
//!
//! Shown in Maintainer mode and above. They're read from the desktop's copy offline (so a job
//! logged offline is priced from them), but changed on the server only (phase-6.md 2f).

use dioxus::prelude::*;
use domain::api::{ShopsOutcome, ShopsView};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;
use domain::log_area;
use domain::preferences::CurrencyCode;
use domain::quantity::{money_value, parse_money};
use domain::shop::{LaborRate, Shop, ShopId};

use super::log_form::{MoneyInput, per_hour};
use super::{FormError, problem_from_error};
use crate::api;
use crate::ui::offline::needs_server_note;
use crate::ui::{tr, tr_with};

/// A rate as typed ("120.00"), as a rate: nothing typed is none.
///
/// # Errors
///
/// What's wrong with it: not an amount, or a rate that's negative or far too high.
fn parse_rate(text: &str, currency: CurrencyCode) -> Result<Option<LaborRate>, MessageId> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    let minor = parse_money(text, currency).map_err(domain::quantity::QuantityError::message)?;
    LaborRate::new(minor)
        .map(Some)
        .map_err(domain::shop::ShopProblem::message)
}

/// A rate as typed again ("120.00"); none is nothing.
fn rate_text(rate: Option<LaborRate>, currency: CurrencyCode) -> String {
    rate.map(|rate| money_value(rate.minor_per_hour(), currency))
        .unwrap_or_default()
}

/// The shop as typed in its dialog: a contact of only spaces is none.
///
/// # Errors
///
/// What's wrong with its name or rate (the server checks the contact and the name's clashes).
fn shop_from(
    name: &str,
    contact: &str,
    rate: &str,
    currency: CurrencyCode,
) -> Result<Shop, MessageId> {
    let contact = contact.trim();
    Ok(Shop {
        name: Label::parse(name).map_err(domain::label::LabelError::message)?,
        contact: (!contact.is_empty()).then(|| contact.to_owned()),
        labor_rate: parse_rate(rate, currency)?,
    })
}

/// What went wrong with a change, if anything.
fn failure(result: &Result<ShopsOutcome, ServerFnError>) -> Option<String> {
    match result {
        Ok(ShopsOutcome::Shops { .. } | ShopsOutcome::Saved { .. } | ShopsOutcome::Done) => None,
        Ok(ShopsOutcome::Invalid { problem }) => Some(tr(problem.message())),
        Ok(ShopsOutcome::NotFound) => Some(tr(messages::SHOP_NOT_FOUND)),
        Ok(ShopsOutcome::NotAllowed) => Some(tr(messages::VEHICLES_NOT_ALLOWED)),
        Ok(ShopsOutcome::Unavailable) => Some(tr(messages::SOMETHING_WENT_WRONG)),
        Err(error) => Some(tr(problem_from_error(error).message())),
    }
}

/// A dialog over the section.
#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    Add,
    Edit(ShopId, Shop),
    Remove(ShopId, String),
}

/// The section: the household's rate, then the shops.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn LaborRates() -> Element {
    let mut view = use_signal(|| None::<ShopsView>);
    let mut error = use_signal(|| None::<String>);
    let mut dialog = use_signal(|| None::<Dialog>);
    let mut reloads = use_signal(|| 0_u32);
    // Loads after mounting (never during server rendering), and again after each change.
    use_effect(move || {
        let _ = reloads();
        spawn(async move {
            match crate::data::list_shops().await {
                Ok(ShopsOutcome::Shops { view: loaded }) => {
                    tracing::debug!(target: log_area::UI, shops = loaded.shops.len(), "labor rates loaded");
                    view.set(Some(loaded));
                }
                other => {
                    error.set(failure(&other).or_else(|| Some(tr(messages::SOMETHING_WENT_WRONG))));
                }
            }
        });
    });
    let Some(current) = view() else {
        return rsx! {
            FormError { message: error() }
            p { class: "auth-loading", {tr(messages::LOADING)} }
        };
    };
    let ShopsView {
        household_rate,
        currency,
        shops,
        may_manage,
    } = current;
    let offline = needs_server_note();
    let may = may_manage && offline.is_none();
    let changed = move |()| {
        dialog.set(None);
        reloads += 1;
    };
    rsx! {
        p { class: "muted-note",
            {tr_with(messages::SHOP_RATES_INTRO, &[("currency", MessageArg::from(currency.as_str()))])}
        }
        if let Some(note) = offline {
            p { class: "muted-note", {note} }
        }
        HouseholdRate { rate: household_rate, currency, may, on_saved: move |()| reloads += 1 }
        h3 { class: "section-heading", {tr(messages::SHOPS_HEADING)} }
        FormError { message: error() }
        if shops.is_empty() {
            p { class: "placeholder", {tr(messages::SHOPS_EMPTY)} }
        }
        for (id, shop) in shops {
            div { key: "{id}", class: "place-row shop-row",
                div { class: "place-row-text",
                    strong { {shop.name.as_str().to_owned()} }
                    span { class: "muted-note", {shop.contact.clone().unwrap_or_default()} }
                }
                span { class: "place-radius",
                    {shop.labor_rate.map_or_else(|| tr(messages::SHOP_RATE_HOUSEHOLD), |rate| per_hour(rate, currency))}
                }
                if may {
                    div { class: "place-row-buttons",
                        button {
                            class: "secondary-button",
                            onclick: {
                                let shop = shop.clone();
                                move |_| dialog.set(Some(Dialog::Edit(id, shop.clone())))
                            },
                            {tr(messages::ACTION_EDIT)}
                        }
                        button {
                            class: "secondary-button",
                            onclick: {
                                let name = shop.name.as_str().to_owned();
                                move |_| dialog.set(Some(Dialog::Remove(id, name.clone())))
                            },
                            {tr(messages::ACTION_REMOVE)}
                        }
                    }
                }
            }
        }
        if may {
            button { class: "link-button", onclick: move |_| dialog.set(Some(Dialog::Add)), {tr(messages::SHOP_ADD)} }
        }
        match dialog() {
            None => rsx! {},
            Some(Dialog::Add) => rsx! { ShopDialog { current: None, currency, on_close: move |()| dialog.set(None), on_saved: changed } },
            Some(Dialog::Edit(id, shop)) => rsx! { ShopDialog { current: Some((id, shop)), currency, on_close: move |()| dialog.set(None), on_saved: changed } },
            Some(Dialog::Remove(id, name)) => rsx! { ConfirmRemove { id, name, on_close: move |()| dialog.set(None), on_removed: changed } },
        }
    }
}

/// The household's rate: typed, then **Save** (empty for none).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn HouseholdRate(
    rate: Option<LaborRate>,
    currency: CurrencyCode,
    may: bool,
    on_saved: EventHandler<()>,
) -> Element {
    let mut typed = use_signal(|| rate_text(rate, currency));
    // Saved (and loaded again): the box shows the rate kept.
    use_effect(use_reactive!(
        |rate, currency| typed.set(rate_text(rate, currency))
    ));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let unchanged = parse_rate(&typed(), currency) == Ok(rate);
    let save = move |event: FormEvent| {
        event.prevent_default();
        let chosen = match parse_rate(&typed(), currency) {
            Ok(chosen) => chosen,
            Err(problem) => {
                error.set(Some(tr(problem)));
                return;
            }
        };
        busy.set(true);
        spawn(async move {
            let result = api::set_labor_rate(chosen).await;
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                on_saved.call(());
            }
        });
    };
    rsx! {
        form { class: "setting-row household-rate", onsubmit: save,
            label { class: "field",
                span { class: "field-label", {tr(messages::SHOP_HOUSEHOLD_RATE)} }
                if may {
                    span { class: "rate-input",
                        MoneyInput {
                            currency,
                            value: typed(),
                            label: tr(messages::SHOP_HOUSEHOLD_RATE),
                            on_input: move |text: String| {
                                typed.set(text);
                                error.set(None);
                            },
                        }
                        span { class: "muted-note", {tr(messages::SHOP_PER_HOUR)} }
                        if !unchanged {
                            button { class: "primary-button", r#type: "submit", disabled: busy(),
                                if busy() { {tr(messages::ACTION_SAVING)} } else { {tr(messages::ACTION_SAVE)} }
                            }
                        }
                    }
                } else {
                    strong {
                        {rate.map_or_else(|| tr(messages::SHOP_RATE_NONE), |rate| per_hour(rate, currency))}
                    }
                }
            }
            p { class: "muted-note", {tr(messages::SHOP_HOUSEHOLD_RATE_HELP)} }
            FormError { message: error() }
        }
    }
}

/// Add a shop, or change one: its name, contact, and own rate.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ShopDialog(
    current: Option<(ShopId, Shop)>,
    currency: CurrencyCode,
    on_close: EventHandler<()>,
    on_saved: EventHandler<()>,
) -> Element {
    let editing = current.as_ref().map(|(id, _)| *id);
    let existing = current.map(|(_, shop)| shop);
    let mut name = use_signal(|| {
        existing
            .as_ref()
            .map(|shop| shop.name.as_str().to_owned())
            .unwrap_or_default()
    });
    let mut contact = use_signal(|| {
        existing
            .as_ref()
            .and_then(|shop| shop.contact.clone())
            .unwrap_or_default()
    });
    let mut rate =
        use_signal(|| rate_text(existing.as_ref().and_then(|shop| shop.labor_rate), currency));
    let mut error = use_signal(|| None::<String>);
    let mut busy = use_signal(|| false);
    let submit = move |event: FormEvent| {
        event.prevent_default();
        let shop = match shop_from(&name(), &contact(), &rate(), currency) {
            Ok(shop) => shop,
            Err(problem) => {
                error.set(Some(tr(problem)));
                return;
            }
        };
        busy.set(true);
        spawn(async move {
            let result = match editing {
                Some(id) => api::edit_shop(id, shop).await,
                None => api::add_shop(shop).await,
            };
            busy.set(false);
            error.set(failure(&result));
            if error().is_none() {
                on_saved.call(());
            }
        });
    };
    let title = if editing.is_some() {
        messages::SHOP_EDIT_TITLE
    } else {
        messages::SHOP_ADD_TITLE
    };
    rsx! {
        div { class: "dialog-scrim",
            form { class: "dialog", role: "dialog", "aria-modal": "true", onsubmit: submit,
                h2 { class: "dialog-title", {tr(title)} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SHOP_NAME)} }
                    input { class: "field-input", autocomplete: "off", maxlength: "40", value: name(), oninput: move |event| name.set(event.value()) }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SHOP_CONTACT)} }
                    input { class: "field-input", autocomplete: "off", value: contact(), oninput: move |event| contact.set(event.value()) }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SHOP_OWN_RATE)} }
                    span { class: "rate-input",
                        MoneyInput { currency, value: rate(), on_input: move |text: String| rate.set(text) }
                        span { class: "muted-note", {tr(messages::SHOP_PER_HOUR)} }
                    }
                }
                p { class: "muted-note", {tr(messages::SHOP_OWN_RATE_HELP)} }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy(),
                        if busy() { {tr(messages::ACTION_SAVING)} } else { {tr(messages::ACTION_SAVE)} }
                    }
                }
            }
        }
    }
}

/// "Remove Joe's Auto?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ConfirmRemove(
    id: ShopId,
    name: String,
    on_close: EventHandler<()>,
    on_removed: EventHandler<()>,
) -> Element {
    let mut error = use_signal(|| None::<String>);
    let remove = move |_| {
        spawn(async move {
            let result = api::delete_shop(id).await;
            error.set(failure(&result));
            if error().is_none() {
                on_removed.call(());
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim",
            div { class: "dialog", role: "alertdialog", "aria-modal": "true",
                h2 { class: "dialog-title", {tr_with(messages::SHOP_REMOVE_TITLE, &[("name", MessageArg::from(name))])} }
                p { class: "dialog-text", {tr(messages::SHOP_REMOVE_TEXT)} }
                FormError { message: error() }
                div { class: "form-buttons",
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button filled", onclick: remove, {tr(messages::ACTION_REMOVE)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_rate_is_typed_as_an_amount_or_left_empty() {
        let usd = CurrencyCode::USD;
        assert_eq!(
            parse_rate("120", usd),
            Ok(Some(LaborRate::new(12_000).unwrap()))
        );
        assert_eq!(parse_rate(" ", usd), Ok(None));
        assert_eq!(parse_rate("100000.01", usd), Err(messages::SHOP_BAD_RATE));
        assert!(parse_rate("lots", usd).is_err());
        assert!(parse_rate("-5", usd).is_err());
        assert_eq!(rate_text(LaborRate::new(9_550).ok(), usd), "95.50");
        assert_eq!(rate_text(None, usd), "");
    }

    #[test]
    fn a_shop_is_typed_with_its_contact_tidied() {
        let usd = CurrencyCode::USD;
        let joes = shop_from(" Joe's Auto ", " 541-555-0100 ", "", usd).unwrap();
        assert_eq!(joes.name.as_str(), "Joe's Auto");
        assert_eq!(joes.contact.as_deref(), Some("541-555-0100"));
        assert_eq!(joes.labor_rate, None);
        assert_eq!(shop_from("Barn", "  ", "95", usd).unwrap().contact, None);
        assert!(shop_from("", "", "", usd).is_err());
        assert_eq!(
            shop_from("Barn", "", "x", usd).map(|_| ()),
            parse_rate("x", usd).map(|_| ())
        );
    }
}
