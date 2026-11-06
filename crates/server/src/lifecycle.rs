//! A vehicle's lifecycle on the server (milestone 9; decision #067): what its lifecycle events
//! mean for it, worked out each time from the ones that count (`domain::lifecycle`).
//!
//! - **Its status** in the Garage and on its page: Disposed once a disposal is recorded, Out
//!   for service while a send-out has no return, otherwise what Setup says.
//! - **No new records once its disposal is finalized:** new events (checked with the events,
//!   `EventProblem::VehicleDisposed`), squawks, documents, plates, and attachments other than
//!   on an event are refused; corrections, and attachments on a correction, still go through.
//! - **An overdue return** ([`overdue_returns_for`]): a message, from the day after the
//!   expected return date (in the send-out's own time zone) until it's back, for whoever sent
//!   it out and anyone who may record its return.

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::asset::{AssetId, LifecycleStatus};
use domain::dates::{CalendarDate, local_to_utc, today};
use domain::event::EventId;
use domain::inbox::{Message, overdue_return_message};
use domain::lifecycle::{self, LifecycleRecord, Standing};
use domain::requirement::{CalendarUnit, calendar_due};
use storage::{Database, StorageError};

use crate::assets::{AssetsContext, unavailable};

/// The vehicle's lifecycle events that count, leaving out `leave_out` (the event being
/// checked, and the one it corrects).
///
/// # Errors
///
/// The storage error if they can't be read.
pub(crate) async fn records(
    database: &Database,
    asset: AssetId,
    leave_out: &[EventId],
) -> Result<Vec<LifecycleRecord>, StorageError> {
    Ok(database
        .lifecycle_of(asset)
        .await?
        .into_iter()
        .filter(|(record, _)| !leave_out.contains(&record.id))
        .map(|(record, _)| record)
        .collect())
}

/// What the vehicle's lifecycle events say about it now (its Setup status is `stored`).
///
/// # Errors
///
/// The storage error if they can't be read.
pub(crate) async fn standing(
    database: &Database,
    asset: AssetId,
    stored: LifecycleStatus,
) -> Result<Standing, StorageError> {
    Ok(lifecycle::standing(
        stored,
        &records(database, asset, &[]).await?,
    ))
}

/// Whether the vehicle takes new records (not once its disposal is finalized).
///
/// # Errors
///
/// The storage error if its events can't be read.
pub(crate) async fn takes_new_records(
    database: &Database,
    asset: AssetId,
) -> Result<bool, StorageError> {
    Ok(lifecycle::takes_new_records(
        &records(database, asset, &[]).await?,
    ))
}

/// The overdue-return messages (#067) for the signed-in person: each vehicle they can see that
/// was sent out and isn't back by the day after it was expected, if they sent it out or may
/// record its return.
///
/// # Errors
///
/// The vehicle answer to give if the vehicles or their events can't be read.
pub async fn overdue_returns_for(
    context: &AssetsContext<'_>,
) -> Result<Vec<Message>, domain::api::AssetsOutcome> {
    let now = context.clock.now();
    let assets = context
        .database
        .assets()
        .await
        .map_err(|error| unavailable("assets", &error))?;
    let mut messages = Vec::new();
    for (id, asset) in assets {
        let scope = AssetScope {
            id,
            group: asset.group,
        };
        let records = records(context.database, id, &[])
            .await
            .map_err(|error| unavailable("lifecycle", &error))?;
        let Some((send_out, _)) = lifecycle::open_send_out(&records) else {
            continue;
        };
        let Some(event) = context
            .database
            .event(send_out.id)
            .await
            .map_err(|error| unavailable("event", &error))?
        else {
            continue;
        };
        let Some(today) = today(now, &event.dates.zone)
            .ok()
            .and_then(|text| CalendarDate::parse(&text).ok())
        else {
            continue;
        };
        let Some((_, expected)) = lifecycle::overdue_return(&records, &today) else {
            continue;
        };
        let sent_it = event.logged_by == context.user.user
            && domain::access::role_on(&context.user.grants, scope).is_some();
        let may_return = permits_on_asset(
            &context.user.grants,
            context.user.mode,
            AssetAction::SendOut,
            scope,
        );
        if !(sent_it || may_return) {
            continue;
        }
        // Raised at the start of the day after it was expected, where it was sent from.
        let raised_at = calendar_due(expected.as_str(), 1, CalendarUnit::Days, false)
            .ok()
            .and_then(|day| local_to_utc(&day, "00:00", &event.dates.zone).ok())
            .unwrap_or(now);
        messages.push(overdue_return_message(
            send_out.id,
            asset.name.as_str(),
            expected,
            raised_at,
        ));
    }
    Ok(messages)
}
