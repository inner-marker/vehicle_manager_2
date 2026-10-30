//! Applying commands: the server's single write path (decisions #005, #043).
//!
//! [`apply`] checks a command, then applies it in **one `BEGIN IMMEDIATE` transaction**:
//! everything it changes (the record, field stamps, tombstone, sequence number, the
//! server's HLC, and the "applied" record) is written together or not at all.
//!
//! The rules:
//! - **Refused before anything is written:** the server is unhealthy, the command format
//!   is unknown, or its stamp is more than [`MAX_AHEAD_MILLIS`] ahead of the server clock.
//! - **Retries do nothing:** a command ID already applied returns [`Outcome::AlreadyApplied`].
//! - **Last-write-wins per field:** each field in an update applies only if the command's
//!   stamp is newer than that field's stored stamp; fields that lose are dropped, and the
//!   rest still apply. A create for a record that already exists is treated the same way.
//! - **Deletes win:** a deleted record is never changed again, whatever the stamps; a later
//!   update or create for it changes nothing.
//! - Every command that changes something gets the next **sequence number**, for pulls.
//!
//! Domain validation of the values themselves (e.g. monotonic readings) happens before
//! this, per record type, as those records arrive.

use domain::access::Actor;
use domain::command::{COMMAND_FORMAT_VERSION, Change, Command, Patch, Record};
use domain::hlc::{HlcClock, HlcError, Stamp};
use domain::hlc_utils::{MAX_AHEAD_MILLIS, check_not_too_far_ahead};
use domain::id::DeviceId;
use domain::log_area;
use domain::time::Clock;
use sqlx::SqliteConnection;
use storage::commands::{self as bookkeeping, RecordWriter};
use storage::{Database, StorageError};

use crate::health::Health;
use crate::ids::{NewIdError, new_id};

/// What applying a command did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Outcome {
    /// Something changed.
    Applied {
        /// The change's sequence number.
        sequence: i64,
        /// Fields that were set.
        applied: Vec<&'static str>,
        /// Fields dropped because a newer change already set them.
        superseded: Vec<&'static str>,
    },
    /// Nothing changed, and nothing needs to: see the reason.
    NoChange(NoChange),
    /// This command was applied before (a retry).
    AlreadyApplied,
}

/// Why an accepted command changed nothing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NoChange {
    /// Every field it sets already has a newer change.
    Superseded,
    /// The record is deleted.
    RecordDeleted,
}

/// Why a command was refused without being applied. The device keeps it and shows the
/// reason (plan §3 "Rejection path"); it's never dropped silently.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Rejection {
    /// The server's database is missing or failed its check.
    ServerUnhealthy,
    /// A command format this server doesn't know.
    UnsupportedFormat(u32),
    /// The device's clock is ahead of the server's by more than the limit.
    ClockTooFarAhead {
        /// By how much, in milliseconds.
        ahead_millis: i64,
    },
}

/// Why applying failed.
#[derive(Debug)]
pub enum ApplyError {
    /// Refused; nothing was written.
    Rejected(Rejection),
    /// The database failed; the transaction rolled back.
    Storage(StorageError),
    /// The server's HLC can't advance (its clock is beyond the HLC range).
    Clock(HlcError),
    /// A new command ID couldn't be made (no OS randomness).
    NewId(NewIdError),
}

impl core::fmt::Display for ApplyError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Rejected(rejection) => write!(formatter, "command refused: {rejection:?}"),
            Self::Storage(error) => error.fmt(formatter),
            Self::Clock(error) => error.fmt(formatter),
            Self::NewId(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ApplyError {}

impl From<StorageError> for ApplyError {
    fn from(error: StorageError) -> Self {
        Self::Storage(error)
    }
}

impl From<sqlx::Error> for ApplyError {
    fn from(error: sqlx::Error) -> Self {
        Self::Storage(StorageError::Sqlx(error))
    }
}

impl From<HlcError> for ApplyError {
    fn from(error: HlcError) -> Self {
        Self::Clock(error)
    }
}

/// Applies `command` to `database`, writing the record through `writer`. `actor` is who
/// made it and in which mode, for the audit trail (`None` for the server's own changes);
/// the caller has already checked their permission.
///
/// `health` is the server's current health (`crate::health::current()`), and `clock`
/// the real clock; both are passed in so tests control them.
///
/// # Errors
///
/// [`ApplyError::Rejected`] if refused (nothing written), or [`ApplyError::Storage`] /
/// [`ApplyError::Clock`] if applying failed (rolled back).
pub async fn apply<R: Record, W: RecordWriter<R> + Sync>(
    database: &Database,
    writer: &W,
    command: &Command<R>,
    actor: Option<Actor>,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<Outcome, ApplyError> {
    check_acceptable(command, health, clock).map_err(|rejection| {
        tracing::debug!(target: log_area::SYNC, command = %command.id, ?rejection, "command refused");
        ApplyError::Rejected(rejection)
    })?;

    let mut transaction = database.pool().begin_with("BEGIN IMMEDIATE").await?;
    let outcome = apply_in_transaction(&mut transaction, writer, command, actor, clock).await?;
    transaction.commit().await?;

    tracing::debug!(target: log_area::SYNC, command = %command.id, entity = R::ENTITY, ?outcome, "command applied");
    Ok(outcome)
}

/// Makes a command on the server and applies it: for a signed-in user's change from the
/// web or desktop app (online-only in Phase 2, so the server stamps it), or the server's
/// own change. It's stamped with the server's next HLC and `device` (the session's device,
/// or the server's own).
///
/// # Errors
///
/// As [`apply`], plus [`ApplyError::NewId`] if no command ID can be made.
pub async fn issue<R: Record, W: RecordWriter<R> + Sync>(
    database: &Database,
    writer: &W,
    change: Change<R>,
    device: DeviceId,
    actor: Option<Actor>,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<Outcome, ApplyError> {
    let hlc = bookkeeping::next_server_hlc(database, clock).await?;
    let command = Command {
        id: new_id(clock.now()).map_err(ApplyError::NewId)?,
        stamp: Stamp { hlc, device },
        format_version: COMMAND_FORMAT_VERSION,
        change,
    };
    apply(database, writer, &command, actor, health, clock).await
}

/// The checks that refuse a command before any database work.
fn check_acceptable<R: Record>(
    command: &Command<R>,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<(), Rejection> {
    if !health.is_healthy() {
        return Err(Rejection::ServerUnhealthy);
    }
    if !(1..=COMMAND_FORMAT_VERSION).contains(&command.format_version) {
        return Err(Rejection::UnsupportedFormat(command.format_version));
    }
    check_not_too_far_ahead(command.stamp.hlc, clock.now(), MAX_AHEAD_MILLIS).map_err(|too_far| {
        Rejection::ClockTooFarAhead {
            ahead_millis: too_far.ahead_millis,
        }
    })
}

async fn apply_in_transaction<R: Record, W: RecordWriter<R> + Sync>(
    connection: &mut SqliteConnection,
    writer: &W,
    command: &Command<R>,
    actor: Option<Actor>,
    clock: &(dyn Clock + Sync),
) -> Result<Outcome, ApplyError> {
    if bookkeeping::is_applied(connection, command.id).await? {
        return Ok(Outcome::AlreadyApplied);
    }
    receive_stamp(connection, command.stamp, clock).await?;

    let outcome = match &command.change {
        Change::Delete { .. } => delete(connection, writer, command, clock).await?,
        Change::Create { values: patch, .. } | Change::Update { patch, .. } => {
            set_fields(connection, writer, command, patch).await?
        }
    };

    let sequence = match &outcome {
        Outcome::Applied { sequence, .. } => Some(*sequence),
        Outcome::NoChange(_) | Outcome::AlreadyApplied => None,
    };
    bookkeeping::record_applied(
        connection,
        command.id,
        command.stamp,
        actor,
        sequence,
        clock.now(),
    )
    .await?;
    Ok(outcome)
}

/// Moves the server's HLC past the command's stamp, so the server's own later changes
/// sort after it.
async fn receive_stamp(
    connection: &mut SqliteConnection,
    stamp: Stamp,
    clock: &(dyn Clock + Sync),
) -> Result<(), ApplyError> {
    let last = bookkeeping::server_last_hlc(connection).await?;
    let mut server_clock = HlcClock::new(clock, last);
    let advanced = server_clock.receive(stamp.hlc)?;
    bookkeeping::set_server_last_hlc(connection, advanced).await?;
    Ok(())
}

/// A create or update: applies the fields whose stamps win.
async fn set_fields<R: Record, W: RecordWriter<R> + Sync>(
    connection: &mut SqliteConnection,
    writer: &W,
    command: &Command<R>,
    patch: &R::Patch,
) -> Result<Outcome, ApplyError> {
    let id = command.change.record_id();
    let record = id.to_bytes();
    if bookkeeping::tombstone(connection, R::ENTITY, record)
        .await?
        .is_some()
    {
        return Ok(Outcome::NoChange(NoChange::RecordDeleted));
    }

    let stored = bookkeeping::field_stamps(connection, R::ENTITY, record).await?;
    let is_new = stored.is_empty();
    // A field wins if nothing set it before, or it was last set by an older stamp.
    let (applied, superseded): (Vec<&'static str>, Vec<&'static str>) = patch
        .changed_fields()
        .into_iter()
        .partition(|field| stored.get(*field).is_none_or(|last| command.stamp > *last));
    if applied.is_empty() {
        return Ok(Outcome::NoChange(NoChange::Superseded));
    }

    let mut winning = patch.clone();
    winning.keep_only(&applied);
    let sequence = bookkeeping::next_sequence(connection).await?;
    if is_new {
        writer.create(connection, id, &winning, sequence).await?;
    } else {
        writer.update(connection, id, &winning, sequence).await?;
    }
    bookkeeping::set_field_stamps(connection, R::ENTITY, record, &applied, command.stamp).await?;
    Ok(Outcome::Applied {
        sequence,
        applied,
        superseded,
    })
}

/// A delete: records the tombstone and lets the writer mark the record deleted.
async fn delete<R: Record, W: RecordWriter<R> + Sync>(
    connection: &mut SqliteConnection,
    writer: &W,
    command: &Command<R>,
    clock: &(dyn Clock + Sync),
) -> Result<Outcome, ApplyError> {
    let id = command.change.record_id();
    let record = id.to_bytes();
    if bookkeeping::tombstone(connection, R::ENTITY, record)
        .await?
        .is_some()
    {
        return Ok(Outcome::NoChange(NoChange::RecordDeleted));
    }
    let sequence = bookkeeping::next_sequence(connection).await?;
    writer.delete(connection, id, sequence).await?;
    bookkeeping::insert_tombstone(
        connection,
        R::ENTITY,
        record,
        command.stamp,
        sequence,
        clock.now(),
    )
    .await?;
    Ok(Outcome::Applied {
        sequence,
        applied: Vec::new(),
        superseded: Vec::new(),
    })
}
