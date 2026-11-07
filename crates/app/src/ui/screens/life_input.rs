//! What a lifecycle event's own section typed (milestone 9; #067), as its
//! [`LifecycleDetails`], and back. Kept apart from the components so it can be tested without a
//! browser.
//!
//! One set of fields serves all four kinds: **the other party** (who it was bought from, the
//! shop, who it went to), **an amount** (the price, the estimate, the final cost), and for a
//! send-out its work order, due-back date, carrier, and tracking; for a disposal, how. A return
//! names the send-out it ends, which the form doesn't ask for.

use domain::dates::CalendarDate;
use domain::event::EventId;
use domain::i18n::messages;
use domain::lifecycle::{
    Acquisition, Disposal, DisposalMethod, LifecycleDetails, Money, Party, Return, SendOut,
};
use domain::meter::EventKind;
use domain::preferences::CurrencyCode;
use domain::quantity::{money_value, parse_money};

use super::log_input::{Problem, Target};

/// A lifecycle event's own fields, as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypedLife {
    /// The other party's name: from whom, the shop, to whom.
    pub party: String,
    /// How to reach them.
    pub contact: String,
    /// The price, the estimate, or the final cost.
    pub amount: String,
    /// A send-out's work order number.
    pub work_order: String,
    /// A send-out's due-back date, `2026-11-20` (as `<input type="date">` gives it).
    pub expected_back: String,
    /// A send-out's carrier.
    pub carrier: String,
    /// A send-out's tracking number.
    pub tracking: String,
    /// A disposal's method.
    pub method: Option<DisposalMethod>,
    /// A correction of a disposal: it wasn't disposed of after all.
    pub withdrawn: bool,
    /// The send-out a return ends.
    pub sent_out: Option<EventId>,
}

/// Trimmed `text`, or `None` if it's empty.
fn optional(text: &str) -> Option<String> {
    Some(text.trim().to_owned()).filter(|text| !text.is_empty())
}

/// The other party, if a name was typed.
fn party(typed: &TypedLife) -> Option<Party> {
    optional(&typed.party).map(|name| Party {
        name,
        contact: optional(&typed.contact),
    })
}

/// The amount, if one was typed, in `currency`.
fn money(typed: &TypedLife, currency: CurrencyCode) -> Result<Option<Money>, Problem> {
    optional(&typed.amount)
        .map(|text| {
            parse_money(&text, currency)
                .map(|minor| Money { minor, currency })
                .map_err(|error| (Target::Life, error.message()))
        })
        .transpose()
}

/// The details `typed` gives an event of `kind` (none for an everyday kind).
///
/// # Errors
///
/// A [`Problem`] under the section: no shop for a send-out, no method for a disposal, no
/// send-out for a return, an amount or date that doesn't parse.
pub(super) fn life_from(
    typed: &TypedLife,
    kind: EventKind,
    currency: CurrencyCode,
) -> Result<Option<LifecycleDetails>, Problem> {
    Ok(Some(match kind {
        EventKind::Fuel | EventKind::Reading | EventKind::Maintenance | EventKind::Repair => {
            return Ok(None);
        }
        EventKind::Acquired => LifecycleDetails::Acquired(Acquisition {
            from: party(typed),
            price: money(typed, currency)?,
        }),
        EventKind::SentOut => LifecycleDetails::SentOut(SendOut {
            shop: party(typed).ok_or((Target::Life, messages::LIFE_SHOP_REQUIRED))?,
            work_order: optional(&typed.work_order),
            expected_back: optional(&typed.expected_back)
                .map(|text| {
                    CalendarDate::parse(&text).map_err(|error| (Target::Life, error.message()))
                })
                .transpose()?,
            carrier: optional(&typed.carrier),
            tracking: optional(&typed.tracking),
            estimate: money(typed, currency)?,
        }),
        EventKind::Returned => LifecycleDetails::Returned(Return {
            sent_out: typed
                .sent_out
                .ok_or((Target::Life, messages::EVENT_NOT_OUT))?,
            cost: money(typed, currency)?,
        }),
        EventKind::Disposed => LifecycleDetails::Disposed(Disposal {
            method: typed
                .method
                .ok_or((Target::Life, messages::LIFE_METHOD_REQUIRED))?,
            to: party(typed),
            price: money(typed, currency)?,
            withdrawn: typed.withdrawn,
        }),
    }))
}

/// What the section starts with to edit or correct an event with `details` (the reverse of
/// [`life_from`]), and the amount's currency.
pub(super) fn typed_life(details: &LifecycleDetails) -> (TypedLife, Option<CurrencyCode>) {
    let with_party = |party: Option<&Party>, amount: Option<Money>| TypedLife {
        party: party.map(|party| party.name.clone()).unwrap_or_default(),
        contact: party
            .and_then(|party| party.contact.clone())
            .unwrap_or_default(),
        amount: amount
            .map(|money| money_value(money.minor, money.currency))
            .unwrap_or_default(),
        ..TypedLife::default()
    };
    let currency_of = |amount: Option<Money>| amount.map(|money| money.currency);
    match details {
        LifecycleDetails::Acquired(acquired) => (
            with_party(acquired.from.as_ref(), acquired.price),
            currency_of(acquired.price),
        ),
        LifecycleDetails::SentOut(sent) => (
            TypedLife {
                work_order: sent.work_order.clone().unwrap_or_default(),
                expected_back: sent
                    .expected_back
                    .as_ref()
                    .map(|date| date.as_str().to_owned())
                    .unwrap_or_default(),
                carrier: sent.carrier.clone().unwrap_or_default(),
                tracking: sent.tracking.clone().unwrap_or_default(),
                ..with_party(Some(&sent.shop), sent.estimate)
            },
            currency_of(sent.estimate),
        ),
        LifecycleDetails::Returned(back) => (
            TypedLife {
                sent_out: Some(back.sent_out),
                ..with_party(None, back.cost)
            },
            currency_of(back.cost),
        ),
        LifecycleDetails::Disposed(disposal) => (
            TypedLife {
                method: Some(disposal.method),
                withdrawn: disposal.withdrawn,
                ..with_party(disposal.to.as_ref(), disposal.price)
            },
            currency_of(disposal.price),
        ),
    }
}

#[cfg(test)]
mod tests {
    use domain::test_support::test_id;

    use super::*;

    const USD: CurrencyCode = CurrencyCode::USD;

    fn send_out() -> TypedLife {
        TypedLife {
            party: " Joe's Auto ".into(),
            contact: "555-0100".into(),
            amount: "450".into(),
            work_order: "WO-1182".into(),
            expected_back: "2026-11-20".into(),
            ..TypedLife::default()
        }
    }

    #[test]
    fn a_send_out_needs_its_shop_and_round_trips() {
        let built = life_from(&send_out(), EventKind::SentOut, USD)
            .unwrap()
            .unwrap();
        let LifecycleDetails::SentOut(sent) = &built else {
            panic!("not a send-out");
        };
        assert_eq!(sent.shop.name, "Joe's Auto");
        assert_eq!(
            sent.estimate,
            Some(Money {
                minor: 45_000,
                currency: USD
            })
        );
        assert_eq!(sent.expected_back.as_ref().unwrap().as_str(), "2026-11-20");
        assert_eq!(sent.carrier, None, "nothing typed is nothing");
        let (typed, currency) = typed_life(&built);
        assert_eq!(currency, Some(USD));
        assert_eq!(
            life_from(&typed, EventKind::SentOut, USD).unwrap(),
            Some(built)
        );
        let no_shop = TypedLife {
            party: "  ".into(),
            ..send_out()
        };
        assert_eq!(
            life_from(&no_shop, EventKind::SentOut, USD),
            Err((Target::Life, messages::LIFE_SHOP_REQUIRED))
        );
    }

    #[test]
    fn a_disposal_needs_how_and_a_return_its_send_out() {
        assert_eq!(
            life_from(&TypedLife::default(), EventKind::Disposed, USD),
            Err((Target::Life, messages::LIFE_METHOD_REQUIRED))
        );
        let sold = TypedLife {
            method: Some(DisposalMethod::Sold),
            amount: "8000".into(),
            ..TypedLife::default()
        };
        let built = life_from(&sold, EventKind::Disposed, USD).unwrap().unwrap();
        assert_eq!(typed_life(&built).0.method, Some(DisposalMethod::Sold));
        assert_eq!(
            life_from(&TypedLife::default(), EventKind::Returned, USD),
            Err((Target::Life, messages::EVENT_NOT_OUT))
        );
        let back = TypedLife {
            sent_out: Some(test_id(7)),
            ..TypedLife::default()
        };
        assert!(life_from(&back, EventKind::Returned, USD).is_ok());
    }

    #[test]
    fn everyday_kinds_have_no_lifecycle_details_and_bad_amounts_are_refused() {
        assert_eq!(life_from(&send_out(), EventKind::Fuel, USD), Ok(None));
        let bad = TypedLife {
            amount: "lots".into(),
            ..TypedLife::default()
        };
        assert!(matches!(
            life_from(&bad, EventKind::Acquired, USD),
            Err((Target::Life, _))
        ));
    }
}
