//! The message center on the server (decisions #040, #050).
//!
//! Messages are derived from the server's state each time they're asked for (its health, each
//! job's latest run, the person's forgotten maintenance jobs, #034, overdue returns, #067,
//! desktops unlocked after wrong tries, #074, and what's coming due on their vehicles, #083,
//! in the person's time zone);
//! only each person's read, dismissed, and snoozed marks are stored, one record per person and
//! message, through the command pipeline. A person can only mark messages they can see, and
//! only due messages snooze.

use domain::access::Actor;
use domain::api::{InboxOutcome, MarkMessage};
use domain::command::Change;
use domain::command::Clearable;
use domain::id::DatabaseId;
use domain::inbox::{
    Message, MessageKey, MessageKind, MessageMarkPatch, ServerProblem, ServerState, inbox,
    server_messages,
};
use domain::location::TimeZone;
use domain::log_area;
use domain::meter::MeterId;
use domain::time::{Clock, UnixMillis};
use domain::timers::Timers;
use storage::Database;
use storage::message_marks::MessageMarkWriter;

use crate::assets::AssetsContext;
use crate::health::{Health, HealthProblem};
use crate::ids::new_id;
use crate::sessions::SignedIn;

/// What the message center needs.
pub struct MessagesContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Its ID (the household preferences' record ID).
    pub database_id: DatabaseId,
    /// Who is asking.
    pub user: &'a SignedIn,
    /// The server's health (marks are changes, refused while unhealthy).
    pub health: Health,
    /// What's wrong and since when, if the server is unhealthy.
    pub problem: Option<(HealthProblem, UnixMillis)>,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
    /// The server's timers (when a job counts as forgotten).
    pub timers: Timers,
}

fn unavailable(error: &dyn std::fmt::Display) -> InboxOutcome {
    tracing::error!(target: log_area::APP, %error, "message center failed");
    InboxOutcome::Unavailable
}

const fn server_problem(problem: HealthProblem) -> ServerProblem {
    match problem {
        HealthProblem::DatabaseUnavailable => ServerProblem::DatabaseUnavailable,
        HealthProblem::IntegrityCheckFailed => ServerProblem::IntegrityCheckFailed,
    }
}

/// Every message the server's state gives the person right now, in their `zone` (before their
/// mode and marks are applied): the server's own, their forgotten jobs, overdue returns, and
/// what's coming due; with their vehicles' meters' latest values, for snoozes.
async fn current_messages(
    context: &MessagesContext<'_>,
    zone: &TimeZone,
) -> Result<(Vec<Message>, Vec<(MeterId, i64)>), InboxOutcome> {
    let jobs = context
        .database
        .job_reports()
        .await
        .map_err(|error| unavailable(&error))?;
    let state = ServerState {
        problem: context
            .problem
            .map(|(problem, since)| (server_problem(problem), since)),
        jobs,
        place_lookup_problem: crate::places::lookup_problem(),
    };
    let assets = AssetsContext {
        database: context.database,
        database_id: context.database_id,
        user: context.user,
        health: context.health,
        clock: context.clock,
        origin: crate::origin::Origin::Online,
    };
    let forgotten = crate::events::forgotten_jobs_for(&assets, context.timers)
        .await
        .map_err(|_| InboxOutcome::Unavailable)?;
    let overdue = crate::lifecycle::overdue_returns_for(&assets)
        .await
        .map_err(|_| InboxOutcome::Unavailable)?;
    let failed_unlocks = crate::devices::failed_unlock_messages(context.database)
        .await
        .map_err(|error| unavailable(&error))?;
    let clock_ahead = crate::devices::clock_ahead_messages(context.database)
        .await
        .map_err(|error| unavailable(&error))?;
    let (due, latest) = crate::due::due_messages(&assets, zone)
        .await
        .map_err(|error| unavailable(&error))?;
    let mut messages = server_messages(&state);
    messages.extend(forgotten);
    messages.extend(overdue);
    messages.extend(failed_unlocks);
    messages.extend(clock_ahead);
    messages.extend(due);
    Ok((messages, latest))
}

/// The signed-in person's inbox, in their `zone`: what they can see in their mode, with their
/// marks applied (snoozed ones set aside until they wake).
pub async fn view(context: &MessagesContext<'_>, zone: &TimeZone) -> InboxOutcome {
    let (messages, latest) = match current_messages(context, zone).await {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    let marks = match context.database.message_marks_of(context.user.user).await {
        Ok(marks) => marks.into_iter().map(|(_, mark)| mark).collect::<Vec<_>>(),
        Err(error) => return unavailable(&error),
    };
    let inbox = inbox(
        &messages,
        &marks,
        context.user.mode,
        context.clock.now(),
        &latest,
    );
    tracing::debug!(target: log_area::APP, user = %context.user.user, items = inbox.items.len(), unread = inbox.unread, "inbox");
    InboxOutcome::Inbox { inbox }
}

/// Marks one of the person's messages read, dismisses it, snoozes it (a due message only), or
/// wakes it; `zone` is theirs, as for [`view`].
pub async fn mark(
    context: &MessagesContext<'_>,
    zone: &TimeZone,
    key: MessageKey,
    action: MarkMessage,
) -> InboxOutcome {
    let (messages, _) = match current_messages(context, zone).await {
        Ok(found) => found,
        Err(outcome) => return outcome,
    };
    // Only messages this person can see now (marks don't matter: a dismissed one stays theirs).
    let visible = inbox(&messages, &[], context.user.mode, context.clock.now(), &[]);
    let Some(message) = visible
        .items
        .into_iter()
        .map(|item| item.message)
        .find(|message| message.key == key)
    else {
        tracing::debug!(target: log_area::APP, user = %context.user.user, %key, "mark refused: no such message for this person");
        return InboxOutcome::NotFound;
    };
    let patch = match patch_for(context, &message, action).await {
        Ok(patch) => patch,
        Err(outcome) => return outcome,
    };
    let existing = match context.database.message_marks_of(context.user.user).await {
        Ok(marks) => marks
            .into_iter()
            .find_map(|(id, mark)| (mark.key == key).then_some(id)),
        Err(error) => return unavailable(&error),
    };
    let change = match existing {
        Some(id) => Change::Update { id, patch },
        None => match new_id(context.clock.now()) {
            Ok(id) => Change::Create {
                id,
                values: MessageMarkPatch {
                    user: Some(context.user.user),
                    key: Some(key.clone()),
                    ..patch
                },
            },
            Err(error) => return unavailable(&error),
        },
    };
    if let Err(outcome) = save(context, change).await {
        return outcome;
    }
    tracing::debug!(target: log_area::APP, user = %context.user.user, %key, ?action, "message marked");
    view(context, zone).await
}

/// The change `action` makes to the person's mark on `message`. A snooze is worked out now,
/// from the item's meter's latest value; only due messages snooze (#083).
async fn patch_for(
    context: &MessagesContext<'_>,
    message: &Message,
    action: MarkMessage,
) -> Result<MessageMarkPatch, InboxOutcome> {
    let read = MessageMarkPatch {
        read: Some(true),
        ..MessageMarkPatch::default()
    };
    Ok(match action {
        MarkMessage::Read => read,
        MarkMessage::Dismiss => MessageMarkPatch {
            dismissed: Some(true),
            ..read
        },
        MarkMessage::Wake => MessageMarkPatch {
            snooze: Some(Clearable::Clear),
            ..MessageMarkPatch::default()
        },
        MarkMessage::Snooze { request } => {
            let MessageKind::Due(notice) = &message.kind else {
                return Err(InboxOutcome::BadSnooze);
            };
            let meter =
                crate::due::snooze_meter_of(context.database, notice.asset, &notice.subject)
                    .await
                    .map_err(|error| unavailable(&error))?;
            let Some(snooze) = request.snooze(context.clock.now(), meter) else {
                tracing::debug!(target: log_area::APP, user = %context.user.user, key = %message.key, ?request, "snooze refused");
                return Err(InboxOutcome::BadSnooze);
            };
            MessageMarkPatch {
                snooze: Some(Clearable::Set(snooze)),
                ..read
            }
        }
    })
}

/// Saves a mark through the command pipeline, as the person.
async fn save(
    context: &MessagesContext<'_>,
    change: Change<domain::inbox::MessageMarkRecord>,
) -> Result<(), InboxOutcome> {
    let actor = Actor {
        user: context.user.user,
        mode: context.user.mode,
    };
    crate::commands::issue(
        context.database,
        &MessageMarkWriter,
        change,
        context.user.device,
        Some(actor),
        context.health,
        context.clock,
    )
    .await
    .map(|_| ())
    .map_err(|error| unavailable(&error))
}
