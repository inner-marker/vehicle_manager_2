//! Pull (phase-3.md 4b, decision #075): a paired desktop asks for the changes after its cursor,
//! one page at a time, to keep its encrypted copy (`storage::local`).
//!
//! Only a **paired desktop** pulls (a browser never keeps a copy, #071), and only when its
//! schema version is the server's: otherwise it reads from the server while online and is
//! told to update. The page is the person's: other people's private rows never leave
//! (`storage::sync::PullScope`); everything else of the household's comes, and the desktop's
//! screens hide what the person's role can't see with the server's own read code (#069).
//!
//! **Push** (phase-3.md 5c, decision #076): a paired desktop on the same schema sends the
//! operations in its outbox, oldest first. Each runs through the **same server function as an
//! online save** (`crate::origin`), as the person is now (their role and mode), so every check
//! runs again; its answer is kept (`applied_operations`), so a resent operation gets the same
//! one and is never applied twice.

use domain::api::{
    AssetsOutcome, AttachmentsOutcome, EventsOutcome, PlacesOutcome, PreferencesOutcome,
    SquawksOutcome,
};
use domain::device::DeviceKind;
use domain::hlc_utils::{MAX_AHEAD_MILLIS, check_not_too_far_ahead};
use domain::id::DatabaseId;
use domain::log_area;
use domain::operation::{
    OPERATION_FORMAT_VERSION, Operation, OperationChange, OperationId, Refusal, accepted_formats,
};
use domain::sync::{
    OperationAnswer, PULL_WINDOW, PUSH_LIMIT, PullOutcome, PullPage, PushOutcome, PushStop,
};
use domain::time::Clock;
use storage::Database;
use storage::files::Files;
use storage::outbox::AppliedOperation;
use storage::sync::{PullScope, ReadPage, schema_version};

use crate::assets::AssetsContext;
use crate::health::Health;
use crate::origin::Origin;
use crate::places::PlacesContext;
use crate::preferences::PreferencesContext;
use crate::sessions::SignedIn;
use crate::{assets, attachments, events, places, preferences, squawks};

/// Whether `signed_in` is on a paired desktop (`None` if the device can't be read).
async fn is_paired_desktop(database: &Database, signed_in: &SignedIn) -> Option<bool> {
    match database.device(signed_in.device).await {
        Ok(device) => Some(device.is_some_and(|device| {
            device.kind == DeviceKind::Desktop && device.public_key.is_some()
        })),
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "sync: can't read the device");
            None
        }
    }
}

/// `Ok` if `signed_in` is on a paired desktop; otherwise the refusal.
async fn paired_desktop(database: &Database, signed_in: &SignedIn) -> Result<(), PullOutcome> {
    match is_paired_desktop(database, signed_in).await {
        Some(true) => Ok(()),
        Some(false) => Err(PullOutcome::NotAllowed),
        None => Err(PullOutcome::Unavailable),
    }
}

/// The page after `cursor` for `signed_in`'s desktop, whose schema version is `schema`.
pub async fn pull(
    database: &Database,
    database_id: DatabaseId,
    signed_in: &SignedIn,
    cursor: u64,
    schema: i64,
) -> PullOutcome {
    if let Err(refused) = paired_desktop(database, signed_in).await {
        return refused;
    }
    let server = schema_version();
    if schema != server {
        tracing::info!(target: log_area::SYNC, desktop = schema, server, "pull refused: another schema version");
        return PullOutcome::OtherSchema { server };
    }
    let scope = PullScope {
        user: signed_in.user,
        household: database_id,
    };
    match database.read_pull_page(&scope, cursor, PULL_WINDOW).await {
        Ok(read) => PullOutcome::Page {
            page: Box::new(page_of(read, database_id, server, cursor, signed_in)),
        },
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "pull: can't read the page");
            PullOutcome::Unavailable
        }
    }
}

/// The page to send, from what was read.
fn page_of(
    read: ReadPage,
    database_id: DatabaseId,
    schema: i64,
    cursor: u64,
    signed_in: &SignedIn,
) -> PullPage {
    if cursor > read.server_sequence {
        // A copy ahead of the server: restored from an old snapshot? Milestone 7's protective
        // mode acts on this; for now the desktop just gets nothing new.
        tracing::warn!(target: log_area::SYNC, cursor, server_sequence = read.server_sequence, "a desktop's copy is ahead of the server");
    }
    let page = PullPage {
        database_id,
        schema,
        cursor,
        next_cursor: read.next_cursor,
        server_sequence: read.server_sequence,
        tables: read.tables,
    };
    tracing::debug!(target: log_area::SYNC, device = %signed_in.device, cursor, next = page.next_cursor, rows = page.row_count(), "pull");
    page
}

// --- Push ---------------------------------------------------------------------------------

/// What a push needs.
pub struct PushContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Its ID.
    pub database_id: DatabaseId,
    /// Who is pushing, from which device (the session's).
    pub user: &'a SignedIn,
    /// The server's health.
    pub health: Health,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
    /// Where the files are (the server's folder, or a desktop's files database), for adding a
    /// photo or document; `None` if there's no file store (adding is then unavailable).
    pub files: Option<&'a Files>,
}

/// What running one operation did ([`run_operation`]).
#[derive(Debug, Clone, PartialEq)]
pub enum Ran {
    /// Applied.
    Applied,
    /// Refused, with the server function's answer.
    Refused(Refusal),
    /// Not now (unhealthy, or a failure): the operation and the rest wait.
    Unavailable,
}

/// Answers `operations` (oldest first, at most [`PUSH_LIMIT`]) from `context`'s desktop, whose
/// schema version is `schema`.
pub async fn push(context: &PushContext<'_>, schema: i64, operations: &[Operation]) -> PushOutcome {
    if let Err(refused) = may_push(context, schema).await {
        return refused;
    }
    let mut answers = Vec::new();
    let mut stopped = None;
    for operation in operations.iter().take(PUSH_LIMIT) {
        match answer(context, operation).await {
            Ok(answer) => answers.push(answer),
            Err(stop) => {
                stopped = Some(stop);
                break;
            }
        }
    }
    if stopped.is_none()
        && !answers.is_empty()
        && let Err(error) = context
            .database
            .clear_clock_ahead(context.user.device)
            .await
    {
        tracing::warn!(target: log_area::SYNC, %error, "push: can't clear the clock flag");
    }
    tracing::info!(target: log_area::SYNC, device = %context.user.device, sent = operations.len(), answered = answers.len(), ?stopped, "push");
    PushOutcome::Answers { answers, stopped }
}

/// `Ok` if `context`'s desktop may push now: the server is well (an unhealthy one takes no
/// changes, 7b, #078), it's a paired desktop, and its schema is the server's.
async fn may_push(context: &PushContext<'_>, schema: i64) -> Result<(), PushOutcome> {
    if !context.health.is_healthy() {
        tracing::info!(target: log_area::SYNC, "push refused: the server isn't well");
        return Err(PushOutcome::Unhealthy);
    }
    match is_paired_desktop(context.database, context.user).await {
        Some(true) => {}
        Some(false) => return Err(PushOutcome::NotAllowed),
        None => return Err(PushOutcome::Unavailable),
    }
    let server = schema_version();
    if schema != server {
        tracing::info!(target: log_area::SYNC, desktop = schema, server, "push refused: another schema version");
        return Err(PushOutcome::OtherSchema { server });
    }
    Ok(())
}

/// The answer to one operation, or why the push stops here.
async fn answer(
    context: &PushContext<'_>,
    operation: &Operation,
) -> Result<OperationAnswer, PushStop> {
    acceptable(context, operation).await?;
    let id = operation.id;
    if let Some(answer) = past_answer(context, id).await? {
        return Ok(answer);
    }
    let refusal = match run_operation(context, &upcast(operation)).await {
        Ran::Applied => None,
        Ran::Refused(refusal) => Some(refusal),
        Ran::Unavailable => return Err(PushStop::Unavailable),
    };
    record(context, operation, refusal.clone()).await;
    Ok(match refusal {
        None => {
            tracing::debug!(target: log_area::SYNC, operation = %id, "applied");
            OperationAnswer::Applied { id }
        }
        Some(refusal) => {
            tracing::info!(target: log_area::SYNC, operation = %id, ?refusal, "refused");
            OperationAnswer::Refused { id, refusal }
        }
    })
}

/// Stops the push at an operation in a format the server can't take, or with a clock far
/// ahead (flagging the device for the Admin).
async fn acceptable(context: &PushContext<'_>, operation: &Operation) -> Result<(), PushStop> {
    let format = operation.format_version;
    if format > OPERATION_FORMAT_VERSION {
        return Err(PushStop::ServerTooOld { format });
    }
    let accepted = accepted_formats();
    if !accepted.contains(&format) {
        return Err(PushStop::AppTooOld {
            format,
            oldest: *accepted.start(),
        });
    }
    let now = context.clock.now();
    let Err(too_far) = check_not_too_far_ahead(operation.hlc, now, MAX_AHEAD_MILLIS) else {
        return Ok(());
    };
    let ahead_millis = too_far.ahead_millis;
    tracing::warn!(target: log_area::SYNC, device = %context.user.device, ahead_millis, "push: the desktop's clock is far ahead");
    if let Err(error) = context
        .database
        .flag_clock_ahead(context.user.device, ahead_millis, now)
        .await
    {
        tracing::error!(target: log_area::SYNC, %error, "push: can't flag the clock");
    }
    Err(PushStop::ClockAhead { ahead_millis })
}

/// The answer already given to operation `id`, if it was sent before.
async fn past_answer(
    context: &PushContext<'_>,
    id: OperationId,
) -> Result<Option<OperationAnswer>, PushStop> {
    match context.database.operation_answer(id).await {
        Ok(None) => Ok(None),
        Ok(Some(None)) => Ok(Some(OperationAnswer::Applied { id })),
        Ok(Some(Some(refusal))) => Ok(Some(OperationAnswer::Refused { id, refusal })),
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "push: can't read past answers");
            Err(PushStop::Unavailable)
        }
    }
}

/// Keeps the answer, so a resend gets the same one.
async fn record(context: &PushContext<'_>, operation: &Operation, refusal: Option<Refusal>) {
    let record = AppliedOperation {
        id: operation.id,
        device: context.user.device,
        user: context.user.user,
        hlc: operation.hlc,
        format_version: operation.format_version,
        refusal,
        received_at: context.clock.now(),
    };
    if let Err(error) = context.database.record_operation(&record).await {
        // Applied, but not recorded: a resend runs it again, and its commands (derived from
        // it) are recognized, so nothing is applied twice.
        tracing::error!(target: log_area::SYNC, %error, "push: can't record the answer");
    }
}

/// `operation` in this build's format. Format 1 is the first, so there is nothing to upcast
/// yet; the next format adds its conversion from 1 here (#072).
fn upcast(operation: &Operation) -> Operation {
    let mut operation = operation.clone();
    operation.format_version = OPERATION_FORMAT_VERSION;
    operation
}

/// Runs `operation` through its server function, as an online save would, with the IDs and
/// times it derives (`crate::origin`). The server runs it for a push; a desktop runs it on its
/// copy first (with the copy as `context.database`), so both make the same records.
pub async fn run_operation(context: &PushContext<'_>, operation: &Operation) -> Ran {
    let assets = || AssetsContext {
        database: context.database,
        database_id: context.database_id,
        user: context.user,
        health: context.health,
        clock: context.clock,
        origin: Origin::pushed(operation),
    };
    match &operation.change {
        OperationChange::AddEvent { input } => {
            ran_event(events::add(&assets(), input.clone()).await)
        }
        OperationChange::EditEvent { id, changes } => {
            ran_event(events::edit_changes(&assets(), *id, changes.clone()).await)
        }
        OperationChange::FinalizeEvent { id } => {
            ran_event(events::finalize_now(&assets(), *id).await)
        }
        OperationChange::FinishEvent { id } => ran_event(events::finish(&assets(), *id).await),
        OperationChange::DeleteEvent { id } => ran_event(events::delete(&assets(), *id).await),
        OperationChange::CorrectEvent {
            original,
            input,
            reason,
        } => ran_event(events::correct(&assets(), *original, input.clone(), reason).await),
        OperationChange::OpenSquawk { input } => {
            ran_squawk(squawks::open(&assets(), input.clone()).await)
        }
        OperationChange::EditSquawk { id, changes } => {
            ran_squawk(squawks::edit_changes(&assets(), *id, changes.clone()).await)
        }
        OperationChange::DeleteSquawk { id } => ran_squawk(squawks::delete(&assets(), *id).await),
        OperationChange::SetGarageOrder { order } => {
            match assets::set_order(&assets(), order.clone()).await {
                AssetsOutcome::Done | AssetsOutcome::Added { .. } => Ran::Applied,
                AssetsOutcome::Unavailable => Ran::Unavailable,
                other @ (AssetsOutcome::Garage { .. }
                | AssetsOutcome::Asset { .. }
                | AssetsOutcome::MovePreview { .. }
                | AssetsOutcome::NotFound
                | AssetsOutcome::NotAllowed
                | AssetsOutcome::Invalid { .. }) => Ran::Refused(Refusal::Assets(other)),
            }
        }
        OperationChange::SetMyPreferences { patch } => {
            let preferences = PreferencesContext {
                database: context.database,
                database_id: context.database_id,
                user: context.user,
                health: context.health,
                clock: context.clock,
                origin: Origin::pushed(operation),
            };
            match preferences::set_mine(&preferences, *patch).await {
                PreferencesOutcome::Preferences { .. } => Ran::Applied,
                PreferencesOutcome::Unavailable => Ran::Unavailable,
                other @ PreferencesOutcome::NotAllowed => Ran::Refused(Refusal::Preferences(other)),
            }
        }
        OperationChange::AddPlace { .. }
        | OperationChange::EditPlace { .. }
        | OperationChange::DeletePlace { .. } => run_place(context, operation).await,
        OperationChange::AddAttachment { input } => {
            let Some(files) = context.files else {
                return Ran::Unavailable;
            };
            ran_attachment(attachments::add(&assets(), files, input.clone()).await)
        }
        OperationChange::MoveAttachment { id, task } => {
            ran_attachment(attachments::move_to(&assets(), *id, *task).await)
        }
        OperationChange::RemoveAttachment { id } => {
            ran_attachment(attachments::remove(&assets(), *id).await)
        }
    }
}

fn ran_attachment(outcome: AttachmentsOutcome) -> Ran {
    match outcome {
        AttachmentsOutcome::Saved { .. } | AttachmentsOutcome::Done => Ran::Applied,
        AttachmentsOutcome::Unavailable => Ran::Unavailable,
        other @ (AttachmentsOutcome::Attachments { .. }
        | AttachmentsOutcome::NotFound
        | AttachmentsOutcome::NotAllowed
        | AttachmentsOutcome::Invalid { .. }) => Ran::Refused(Refusal::Attachments(other)),
    }
}

/// Runs a saved place's operation.
async fn run_place(context: &PushContext<'_>, operation: &Operation) -> Ran {
    let places = PlacesContext {
        database: context.database,
        user: context.user,
        health: context.health,
        clock: context.clock,
        geocoder: None,
        origin: Origin::pushed(operation),
    };
    let outcome = match &operation.change {
        OperationChange::AddPlace { form } => places::add(&places, form).await,
        OperationChange::EditPlace { id, changes } => {
            places::edit_changes(&places, *id, changes.clone()).await
        }
        OperationChange::DeletePlace { id } => places::delete(&places, *id).await,
        OperationChange::AddEvent { .. }
        | OperationChange::EditEvent { .. }
        | OperationChange::FinalizeEvent { .. }
        | OperationChange::FinishEvent { .. }
        | OperationChange::DeleteEvent { .. }
        | OperationChange::CorrectEvent { .. }
        | OperationChange::OpenSquawk { .. }
        | OperationChange::EditSquawk { .. }
        | OperationChange::DeleteSquawk { .. }
        | OperationChange::SetMyPreferences { .. }
        | OperationChange::SetGarageOrder { .. }
        | OperationChange::AddAttachment { .. }
        | OperationChange::MoveAttachment { .. }
        | OperationChange::RemoveAttachment { .. } => return Ran::Unavailable,
    };
    match outcome {
        PlacesOutcome::Added { .. } | PlacesOutcome::Done => Ran::Applied,
        PlacesOutcome::Unavailable => Ran::Unavailable,
        other @ (PlacesOutcome::Places { .. }
        | PlacesOutcome::Located { .. }
        | PlacesOutcome::Countries { .. }
        | PlacesOutcome::DataInfo { .. }
        | PlacesOutcome::NotFound
        | PlacesOutcome::Invalid { .. }) => Ran::Refused(Refusal::Places(other)),
    }
}

fn ran_event(outcome: EventsOutcome) -> Ran {
    match outcome {
        EventsOutcome::Saved { .. } | EventsOutcome::Done => Ran::Applied,
        EventsOutcome::Unavailable => Ran::Unavailable,
        other @ (EventsOutcome::Events { .. }
        | EventsOutcome::Event { .. }
        | EventsOutcome::LogContext { .. }
        | EventsOutcome::NotFound
        | EventsOutcome::NotAllowed
        | EventsOutcome::Invalid { .. }) => Ran::Refused(Refusal::Events(other)),
    }
}

fn ran_squawk(outcome: SquawksOutcome) -> Ran {
    match outcome {
        SquawksOutcome::Saved { .. } | SquawksOutcome::Done => Ran::Applied,
        SquawksOutcome::Unavailable => Ran::Unavailable,
        other @ (SquawksOutcome::Squawks { .. }
        | SquawksOutcome::Squawk { .. }
        | SquawksOutcome::NotFound
        | SquawksOutcome::NotAllowed
        | SquawksOutcome::Invalid { .. }) => Ran::Refused(Refusal::Squawks(other)),
    }
}
