//! A vehicle's life as events (milestone 9; plan §3 Asset lifecycle & components; decision
//! #067): when and how it was **acquired**, when it was **sent out** for service and
//! **returned**, and when and how it was **disposed of**.
//!
//! Each is an [`Event`](crate::event::Event) of its own kind, with its details in
//! [`LifecycleDetails`]; its date, readings, notes, and attachments are the event's own, and it
//! follows the event rules (#034): a draft until finalized, corrected after.
//!
//! What they mean for the vehicle is worked out from them, never stored:
//! - its **status** ([`status`]): Disposed after a disposal, Out for service while a send-out
//!   has no return, otherwise what Setup says;
//! - whether it **takes new records** ([`takes_new_records`]): not once its disposal is
//!   finalized, though corrections still are;
//! - whether a return is **overdue** ([`overdue_return`]): the day after the expected return
//!   date, with no return.
//!
//! The checks a lifecycle event passes are [`check`]'s.

use serde::{Deserialize, Serialize};

use crate::asset::LifecycleStatus;
use crate::dates::CalendarDate;
use crate::event::{EventId, EventProblem, EventStatus, PERFORMER_MAX_CHARS};
use crate::i18n::{MessageId, messages};
use crate::meter::EventKind;
use crate::preferences::CurrencyCode;
use crate::time::UnixMillis;

/// The most characters a work order number, carrier, or tracking number may have.
pub const REFERENCE_MAX_CHARS: usize = 100;

/// Someone on the other side: a seller, a shop, a buyer.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Party {
    /// Their name, e.g. "Joe's Auto" (1–100 characters).
    pub name: String,
    /// How to reach them, if wanted (up to 100 characters).
    pub contact: Option<String>,
}

/// An amount of money, in its currency's minor unit.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Money {
    /// How much (zero or more).
    pub minor: i64,
    /// In what.
    pub currency: CurrencyCode,
}

/// How the vehicle was acquired.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Acquisition {
    /// From whom (a seller, dealer, or maker), if recorded.
    pub from: Option<Party>,
    /// What it cost, if recorded.
    pub price: Option<Money>,
}

/// Where the vehicle went, and when it should be back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SendOut {
    /// The shop or vendor.
    pub shop: Party,
    /// The shop's work order number.
    pub work_order: Option<String>,
    /// When it should be back.
    pub expected_back: Option<CalendarDate>,
    /// Who's shipping it, if it's shipped.
    pub carrier: Option<String>,
    /// The tracking number.
    pub tracking: Option<String>,
    /// What the shop expects it to cost.
    pub estimate: Option<Money>,
}

/// The vehicle came back.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Return {
    /// The send-out it ends.
    pub sent_out: EventId,
    /// What it cost in the end.
    pub cost: Option<Money>,
}

/// How a vehicle left the household.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DisposalMethod {
    /// Sold.
    Sold,
    /// Traded in for another.
    TradedIn,
    /// Scrapped.
    Scrapped,
    /// Given away.
    GivenAway,
    /// Returned to the vendor (e.g. the end of a lease).
    ReturnedToVendor,
    /// Lost or stolen.
    LostOrStolen,
    /// Something else (the notes say what).
    Other,
}

impl DisposalMethod {
    /// Every method, in the order the form offers them.
    pub const ALL: [Self; 7] = [
        Self::Sold,
        Self::TradedIn,
        Self::Scrapped,
        Self::GivenAway,
        Self::ReturnedToVendor,
        Self::LostOrStolen,
        Self::Other,
    ];

    /// The method's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Sold => messages::DISPOSAL_SOLD,
            Self::TradedIn => messages::DISPOSAL_TRADED_IN,
            Self::Scrapped => messages::DISPOSAL_SCRAPPED,
            Self::GivenAway => messages::DISPOSAL_GIVEN_AWAY,
            Self::ReturnedToVendor => messages::DISPOSAL_RETURNED_TO_VENDOR,
            Self::LostOrStolen => messages::DISPOSAL_LOST_OR_STOLEN,
            Self::Other => messages::DISPOSAL_OTHER,
        }
    }
}

/// How the vehicle was disposed of.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Disposal {
    /// How.
    pub method: DisposalMethod,
    /// To whom, if recorded.
    pub to: Option<Party>,
    /// What it brought in, if anything.
    pub price: Option<Money>,
    /// Set only on a correction: it wasn't disposed of after all, so the vehicle is back
    /// (decision #067).
    pub withdrawn: bool,
}

/// What a lifecycle event adds.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum LifecycleDetails {
    /// It was acquired.
    Acquired(Acquisition),
    /// It went out for service.
    SentOut(SendOut),
    /// It came back.
    Returned(Return),
    /// It was disposed of.
    Disposed(Disposal),
}

impl LifecycleDetails {
    /// The kind of event these details belong on.
    #[must_use]
    pub const fn kind(&self) -> EventKind {
        match self {
            Self::Acquired(_) => EventKind::Acquired,
            Self::SentOut(_) => EventKind::SentOut,
            Self::Returned(_) => EventKind::Returned,
            Self::Disposed(_) => EventKind::Disposed,
        }
    }
}

/// One of the vehicle's lifecycle events that counts (not one that has been corrected), as the
/// rules here need it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LifecycleRecord {
    /// The event.
    pub id: EventId,
    /// When it happened.
    pub at: UnixMillis,
    /// Draft or finalized.
    pub status: EventStatus,
    /// What it says.
    pub details: LifecycleDetails,
}

/// What a vehicle's lifecycle events say about it now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Standing {
    /// The status it shows ([`status`]).
    pub status: LifecycleStatus,
    /// Whether it takes new records ([`takes_new_records`]).
    pub takes_new_records: bool,
    /// The send-out it's on now, if it's out for service ([`open_send_out`]).
    pub sent_out: Option<EventId>,
    /// Its acquisition, if one is recorded ([`acquisition`]).
    #[serde(default)]
    pub acquired: Option<EventId>,
    /// Its disposal, if one is recorded, a draft too ([`disposal`]).
    #[serde(default)]
    pub disposal: Option<EventId>,
}

impl Standing {
    /// The lifecycle kinds that fit now (#067): none once it takes no new records; Sent out
    /// while it isn't out, Returned while it is; Acquired until one is recorded; Disposed until
    /// one is. Who may log each is a separate check (`AssetAction::to_log`).
    #[must_use]
    pub fn lifecycle_kinds(&self) -> Vec<EventKind> {
        if !self.takes_new_records {
            return Vec::new();
        }
        let disposed = self.disposal.is_some();
        [
            (EventKind::SentOut, self.sent_out.is_none() && !disposed),
            (EventKind::Returned, self.sent_out.is_some()),
            (EventKind::Acquired, self.acquired.is_none()),
            (EventKind::Disposed, !disposed),
        ]
        .into_iter()
        .filter_map(|(kind, fits)| fits.then_some(kind))
        .collect()
    }
}

impl Default for Standing {
    /// In service, taking new records, not out.
    fn default() -> Self {
        Self {
            status: LifecycleStatus::InService,
            takes_new_records: true,
            sent_out: None,
            acquired: None,
            disposal: None,
        }
    }
}

/// What `records` say about a vehicle whose Setup status is `stored`.
#[must_use]
pub fn standing(stored: LifecycleStatus, records: &[LifecycleRecord]) -> Standing {
    Standing {
        status: status(stored, records),
        takes_new_records: takes_new_records(records),
        sent_out: open_send_out(records).map(|(record, _)| record.id),
        acquired: acquisition(records).map(|record| record.id),
        disposal: disposal(records).map(|record| record.id),
    }
}

/// The send-out with no return yet, if the vehicle is out.
#[must_use]
pub fn open_send_out(records: &[LifecycleRecord]) -> Option<(&LifecycleRecord, &SendOut)> {
    records
        .iter()
        .filter_map(|record| match &record.details {
            LifecycleDetails::SentOut(send_out) => Some((record, send_out)),
            LifecycleDetails::Acquired(_)
            | LifecycleDetails::Returned(_)
            | LifecycleDetails::Disposed(_) => None,
        })
        .filter(|(record, _)| {
            !records.iter().any(|other| {
                matches!(&other.details, LifecycleDetails::Returned(back) if back.sent_out == record.id)
            })
        })
        .max_by_key(|(record, _)| record.at)
}

/// The disposal that counts (a draft too), if the vehicle has been disposed of.
#[must_use]
pub fn disposal(records: &[LifecycleRecord]) -> Option<&LifecycleRecord> {
    records
        .iter()
        .find(|record| matches!(&record.details, LifecycleDetails::Disposed(disposal) if !disposal.withdrawn))
}

/// The acquisition, if one is recorded.
#[must_use]
pub fn acquisition(records: &[LifecycleRecord]) -> Option<&LifecycleRecord> {
    records
        .iter()
        .find(|record| matches!(record.details, LifecycleDetails::Acquired(_)))
}

/// The status the vehicle shows: Disposed once a disposal is recorded (even a draft), Out for
/// service while a send-out has no return, otherwise `stored` (what Setup says).
#[must_use]
pub fn status(stored: LifecycleStatus, records: &[LifecycleRecord]) -> LifecycleStatus {
    if disposal(records).is_some() {
        LifecycleStatus::Disposed
    } else if open_send_out(records).is_some() {
        LifecycleStatus::OutForService
    } else {
        stored
    }
}

/// Whether the vehicle takes new records (events, squawks, documents, plates, attachments):
/// not once its disposal is finalized (#067). Corrections of its finished records are still
/// allowed, the disposal's own included.
#[must_use]
pub fn takes_new_records(records: &[LifecycleRecord]) -> bool {
    disposal(records).is_none_or(|record| record.status == EventStatus::Draft)
}

/// The send-out whose return is overdue on `today` (the day after its expected return date or
/// later, with no return), and the date it was expected.
#[must_use]
pub fn overdue_return<'a>(
    records: &'a [LifecycleRecord],
    today: &CalendarDate,
) -> Option<(&'a LifecycleRecord, &'a CalendarDate)> {
    let (record, send_out) = open_send_out(records)?;
    let expected = send_out.expected_back.as_ref()?;
    (today > expected).then_some((record, expected))
}

/// Checks a lifecycle event's details against its kind and the vehicle's other lifecycle
/// events (`others`: the ones that count, not this one nor the one it corrects). `correction`
/// says whether it corrects another event; `at` is when it happened.
///
/// # Errors
///
/// - [`EventProblem::LifecycleDetails`]: details missing, or for another kind.
/// - [`EventProblem::BadPerformer`]: a party's name or contact empty or too long.
/// - [`EventProblem::TextTooLong`]: a work order, carrier, or tracking number too long.
/// - [`EventProblem::BadAmount`]: a negative amount.
/// - [`EventProblem::AlreadyAcquired`], [`EventProblem::AlreadyOut`],
///   [`EventProblem::NotOut`], [`EventProblem::ReturnBeforeSendOut`],
///   [`EventProblem::AlreadyDisposed`]: it doesn't fit the vehicle's life so far.
/// - [`EventProblem::WithdrawnNotCorrection`]: a withdrawn disposal that isn't a correction.
pub fn check(
    kind: EventKind,
    details: Option<&LifecycleDetails>,
    at: UnixMillis,
    correction: bool,
    others: &[LifecycleRecord],
) -> Result<(), EventProblem> {
    let details = match (kind.is_lifecycle(), details) {
        (true, Some(details)) if details.kind() == kind => details,
        (false, None) => return Ok(()),
        (true | false, _) => return Err(EventProblem::LifecycleDetails),
    };
    match details {
        LifecycleDetails::Acquired(acquired) => {
            check_party(acquired.from.as_ref())?;
            check_money(acquired.price)?;
            if acquisition(others).is_some() {
                return Err(EventProblem::AlreadyAcquired);
            }
        }
        LifecycleDetails::SentOut(send_out) => {
            check_party(Some(&send_out.shop))?;
            check_money(send_out.estimate)?;
            for text in [&send_out.work_order, &send_out.carrier, &send_out.tracking] {
                check_reference(text.as_deref())?;
            }
            if open_send_out(others).is_some() {
                return Err(EventProblem::AlreadyOut);
            }
        }
        LifecycleDetails::Returned(back) => {
            check_money(back.cost)?;
            let Some((sent, _)) =
                open_send_out(others).filter(|(sent, _)| sent.id == back.sent_out)
            else {
                return Err(EventProblem::NotOut);
            };
            if at < sent.at {
                return Err(EventProblem::ReturnBeforeSendOut);
            }
        }
        LifecycleDetails::Disposed(disposal_details) => {
            check_party(disposal_details.to.as_ref())?;
            check_money(disposal_details.price)?;
            if disposal_details.withdrawn && !correction {
                return Err(EventProblem::WithdrawnNotCorrection);
            }
            if disposal(others).is_some() {
                return Err(EventProblem::AlreadyDisposed);
            }
        }
    }
    Ok(())
}

fn check_party(party: Option<&Party>) -> Result<(), EventProblem> {
    let Some(party) = party else {
        return Ok(());
    };
    let name = party.name.trim().chars().count();
    let contact = party.contact.as_deref().map_or(0, |c| c.chars().count());
    if name == 0 || name > PERFORMER_MAX_CHARS || contact > PERFORMER_MAX_CHARS {
        Err(EventProblem::BadPerformer)
    } else {
        Ok(())
    }
}

fn check_money(money: Option<Money>) -> Result<(), EventProblem> {
    if money.is_some_and(|money| money.minor < 0) {
        Err(EventProblem::BadAmount)
    } else {
        Ok(())
    }
}

fn check_reference(text: Option<&str>) -> Result<(), EventProblem> {
    if text.is_some_and(|text| text.chars().count() > REFERENCE_MAX_CHARS) {
        Err(EventProblem::TextTooLong)
    } else {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    fn id(n: u32) -> EventId {
        test_id(n)
    }

    fn date(text: &str) -> CalendarDate {
        CalendarDate::parse(text).unwrap()
    }

    fn party(name: &str) -> Party {
        Party {
            name: name.to_owned(),
            contact: None,
        }
    }

    fn usd(minor: i64) -> Money {
        Money {
            minor,
            currency: CurrencyCode::USD,
        }
    }

    fn send_out(expected: Option<&str>) -> LifecycleDetails {
        LifecycleDetails::SentOut(SendOut {
            shop: party("Joe's Auto"),
            work_order: Some("WO-1182".to_owned()),
            expected_back: expected.map(date),
            carrier: None,
            tracking: None,
            estimate: Some(usd(45_000)),
        })
    }

    fn back(from: u32) -> LifecycleDetails {
        LifecycleDetails::Returned(Return {
            sent_out: id(from),
            cost: Some(usd(51_250)),
        })
    }

    fn disposed(withdrawn: bool) -> LifecycleDetails {
        LifecycleDetails::Disposed(Disposal {
            method: DisposalMethod::Sold,
            to: Some(party("A neighbour")),
            price: Some(usd(800_000)),
            withdrawn,
        })
    }

    fn acquired() -> LifecycleDetails {
        LifecycleDetails::Acquired(Acquisition {
            from: Some(party("Main St Motors")),
            price: Some(usd(2_400_000)),
        })
    }

    fn record(n: u32, at: i64, status: EventStatus, details: LifecycleDetails) -> LifecycleRecord {
        LifecycleRecord {
            id: id(n),
            at: UnixMillis::from_millis(at),
            status,
            details,
        }
    }

    fn draft(n: u32, at: i64, details: LifecycleDetails) -> LifecycleRecord {
        record(n, at, EventStatus::Draft, details)
    }

    fn done(n: u32, at: i64, details: LifecycleDetails) -> LifecycleRecord {
        record(n, at, EventStatus::Finalized, details)
    }

    #[test]
    fn the_status_follows_send_outs_and_returns() {
        let mut records = vec![done(1, 100, acquired())];
        assert_eq!(
            status(LifecycleStatus::Spare, &records),
            LifecycleStatus::Spare
        );
        records.push(draft(2, 200, send_out(None)));
        assert_eq!(
            status(LifecycleStatus::Spare, &records),
            LifecycleStatus::OutForService
        );
        records.push(draft(3, 300, back(2)));
        assert_eq!(
            status(LifecycleStatus::Spare, &records),
            LifecycleStatus::Spare,
            "back to what Setup says"
        );
    }

    #[test]
    fn a_disposal_counts_from_its_draft_but_locks_only_once_finalized() {
        let mut records = vec![draft(1, 100, disposed(false))];
        assert_eq!(
            status(LifecycleStatus::InService, &records),
            LifecycleStatus::Disposed
        );
        assert!(takes_new_records(&records), "a draft can still be deleted");
        records[0].status = EventStatus::Finalized;
        assert!(!takes_new_records(&records));
    }

    #[test]
    fn a_withdrawn_disposal_brings_the_vehicle_back() {
        // The finalized disposal was corrected, so only the correction counts.
        let records = vec![done(2, 100, disposed(true))];
        assert_eq!(
            status(LifecycleStatus::InService, &records),
            LifecycleStatus::InService
        );
        assert!(takes_new_records(&records));
    }

    #[test]
    fn a_return_is_overdue_from_the_day_after_it_was_expected() {
        let records = vec![done(1, 100, send_out(Some("2026-11-10")))];
        assert!(overdue_return(&records, &date("2026-11-10")).is_none());
        let (record, expected) = overdue_return(&records, &date("2026-11-11")).unwrap();
        assert_eq!((record.id, expected.as_str()), (id(1), "2026-11-10"));
    }

    #[test]
    fn a_return_is_never_overdue_once_back_or_without_a_date() {
        let back_already = vec![
            done(1, 100, send_out(Some("2026-11-10"))),
            draft(2, 200, back(1)),
        ];
        assert!(overdue_return(&back_already, &date("2026-12-01")).is_none());
        let no_date = vec![done(1, 100, send_out(None))];
        assert!(overdue_return(&no_date, &date("2026-12-01")).is_none());
    }

    #[test]
    fn details_must_match_the_kind() {
        let at = UnixMillis::from_millis(500);
        assert_eq!(
            check(EventKind::SentOut, Some(&acquired()), at, false, &[]),
            Err(EventProblem::LifecycleDetails)
        );
        assert_eq!(
            check(EventKind::Disposed, None, at, false, &[]),
            Err(EventProblem::LifecycleDetails)
        );
        assert_eq!(
            check(EventKind::Fuel, Some(&acquired()), at, false, &[]),
            Err(EventProblem::LifecycleDetails)
        );
        assert_eq!(check(EventKind::Fuel, None, at, false, &[]), Ok(()));
        assert_eq!(
            check(EventKind::Acquired, Some(&acquired()), at, false, &[]),
            Ok(())
        );
    }

    #[test]
    fn one_acquisition_one_trip_out_and_one_disposal_at_a_time() {
        let at = UnixMillis::from_millis(500);
        let others = [
            done(1, 100, acquired()),
            done(2, 200, send_out(None)),
            draft(3, 300, disposed(false)),
        ];
        assert_eq!(
            check(EventKind::Acquired, Some(&acquired()), at, false, &others),
            Err(EventProblem::AlreadyAcquired)
        );
        assert_eq!(
            check(
                EventKind::SentOut,
                Some(&send_out(None)),
                at,
                false,
                &others
            ),
            Err(EventProblem::AlreadyOut)
        );
        assert_eq!(
            check(
                EventKind::Disposed,
                Some(&disposed(false)),
                at,
                false,
                &others
            ),
            Err(EventProblem::AlreadyDisposed)
        );
    }

    #[test]
    fn a_return_ends_the_open_send_out_and_not_before_it() {
        let others = [done(2, 200, send_out(None))];
        let returned = |from, at| {
            check(
                EventKind::Returned,
                Some(&back(from)),
                UnixMillis::from_millis(at),
                false,
                &others,
            )
        };
        assert_eq!(returned(2, 300), Ok(()));
        assert_eq!(
            returned(2, 300 - 101),
            Err(EventProblem::ReturnBeforeSendOut)
        );
        assert_eq!(returned(9, 300), Err(EventProblem::NotOut));
        assert_eq!(
            check(
                EventKind::Returned,
                Some(&back(2)),
                UnixMillis::from_millis(300),
                false,
                &[]
            ),
            Err(EventProblem::NotOut),
            "not out at all"
        );
    }

    #[test]
    fn only_a_correction_can_withdraw_a_disposal() {
        let at = UnixMillis::from_millis(500);
        assert_eq!(
            check(EventKind::Disposed, Some(&disposed(true)), at, false, &[]),
            Err(EventProblem::WithdrawnNotCorrection)
        );
        assert_eq!(
            check(EventKind::Disposed, Some(&disposed(true)), at, true, &[]),
            Ok(())
        );
    }

    #[test]
    fn parties_amounts_and_references_are_checked() {
        let at = UnixMillis::from_millis(500);
        let nameless = LifecycleDetails::Acquired(Acquisition {
            from: Some(party("  ")),
            price: None,
        });
        assert_eq!(
            check(EventKind::Acquired, Some(&nameless), at, false, &[]),
            Err(EventProblem::BadPerformer)
        );
        let negative = LifecycleDetails::Returned(Return {
            sent_out: id(2),
            cost: Some(usd(-1)),
        });
        assert_eq!(
            check(
                EventKind::Returned,
                Some(&negative),
                at,
                false,
                &[done(2, 200, send_out(None))]
            ),
            Err(EventProblem::BadAmount)
        );
        let long = SendOut {
            shop: party("Joe's Auto"),
            work_order: None,
            expected_back: None,
            carrier: None,
            tracking: Some("1".repeat(REFERENCE_MAX_CHARS + 1)),
            estimate: None,
        };
        assert_eq!(
            check(
                EventKind::SentOut,
                Some(&LifecycleDetails::SentOut(long)),
                at,
                false,
                &[]
            ),
            Err(EventProblem::TextTooLong)
        );
    }

    #[test]
    fn details_survive_the_trip() {
        for details in [
            acquired(),
            send_out(Some("2026-11-10")),
            back(2),
            disposed(false),
        ] {
            let json = serde_json::to_string(&details).unwrap();
            assert_eq!(
                serde_json::from_str::<LifecycleDetails>(&json).unwrap(),
                details
            );
        }
    }

    #[test]
    fn only_the_lifecycle_kinds_that_fit_now_are_offered() {
        let home = standing(LifecycleStatus::InService, &[]);
        assert_eq!(
            home.lifecycle_kinds(),
            [EventKind::SentOut, EventKind::Acquired, EventKind::Disposed]
        );
        let out = standing(
            LifecycleStatus::InService,
            &[done(1, 100, acquired()), done(2, 200, send_out(None))],
        );
        assert_eq!(
            out.lifecycle_kinds(),
            [EventKind::Returned, EventKind::Disposed]
        );
        assert_eq!(out.sent_out, Some(id(2)));
        assert_eq!(out.acquired, Some(id(1)));
        let selling = standing(
            LifecycleStatus::InService,
            &[draft(3, 300, disposed(false))],
        );
        assert_eq!(selling.lifecycle_kinds(), [EventKind::Acquired]);
        let gone = standing(LifecycleStatus::InService, &[done(3, 300, disposed(false))]);
        assert!(gone.lifecycle_kinds().is_empty());
    }
}
