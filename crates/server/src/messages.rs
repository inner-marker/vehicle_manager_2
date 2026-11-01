//! The message center on the server (decisions #040, #050).
//!
//! Messages are derived from the server's state each time they're asked for (its health and
//! each job's latest run); only each person's read and dismissed marks are stored, one record
//! per person and message, through the command pipeline. A person can only mark messages they
//! can see.

use domain::access::Actor;
use domain::api::{InboxOutcome, MarkMessage};
use domain::command::Change;
use domain::inbox::{
    Message, MessageKey, MessageMarkPatch, ServerProblem, ServerState, inbox, server_messages,
};
use domain::log_area;
use domain::time::{Clock, UnixMillis};
use storage::Database;
use storage::message_marks::MessageMarkWriter;

use crate::health::{Health, HealthProblem};
use crate::ids::new_id;
use crate::sessions::SignedIn;

/// What the message center needs.
pub struct MessagesContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Who is asking.
    pub user: &'a SignedIn,
    /// The server's health (marks are changes, refused while unhealthy).
    pub health: Health,
    /// What's wrong and since when, if the server is unhealthy.
    pub problem: Option<(HealthProblem, UnixMillis)>,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
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

/// Every message the server's state gives right now (before choosing whose inbox).
async fn current_messages(context: &MessagesContext<'_>) -> Result<Vec<Message>, InboxOutcome> {
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
    };
    Ok(server_messages(&state))
}

/// The signed-in person's inbox: what they can see in their mode, with their marks applied.
pub async fn view(context: &MessagesContext<'_>) -> InboxOutcome {
    let messages = match current_messages(context).await {
        Ok(messages) => messages,
        Err(outcome) => return outcome,
    };
    let marks = match context.database.message_marks_of(context.user.user).await {
        Ok(marks) => marks.into_iter().map(|(_, mark)| mark).collect::<Vec<_>>(),
        Err(error) => return unavailable(&error),
    };
    let inbox = inbox(&messages, &marks, context.user.mode);
    tracing::debug!(target: log_area::APP, user = %context.user.user, items = inbox.items.len(), unread = inbox.unread, "inbox");
    InboxOutcome::Inbox { inbox }
}

/// Marks one of the person's messages read, or dismisses it.
pub async fn mark(
    context: &MessagesContext<'_>,
    key: MessageKey,
    action: MarkMessage,
) -> InboxOutcome {
    let messages = match current_messages(context).await {
        Ok(messages) => messages,
        Err(outcome) => return outcome,
    };
    // Only messages this person can see now (marks don't matter: a dismissed one stays theirs).
    let visible = inbox(&messages, &[], context.user.mode);
    if !visible.items.iter().any(|item| item.message.key == key) {
        tracing::debug!(target: log_area::APP, user = %context.user.user, %key, "mark refused: no such message for this person");
        return InboxOutcome::NotFound;
    }
    let existing = match context.database.message_marks_of(context.user.user).await {
        Ok(marks) => marks
            .into_iter()
            .find_map(|(id, mark)| (mark.key == key).then_some(id)),
        Err(error) => return unavailable(&error),
    };
    let patch = MessageMarkPatch {
        read: Some(true),
        dismissed: (action == MarkMessage::Dismiss).then_some(true),
        ..MessageMarkPatch::default()
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
    view(context).await
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
