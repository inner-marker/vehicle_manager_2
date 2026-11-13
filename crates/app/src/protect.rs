//! Protective mode, snapshots, and rebuilding (phase-3.md 7c, decision #078; plan §6).
//!
//! The desktop's copy is the household's backup, so a damaged server must never damage it:
//!
//! - **Detecting** (every pull, `local_copy`): a page from another server (the database ID
//!   changed: reset, or another server) or from one gone back in time (its newest change
//!   number below the copy's cursor), the handshake naming another database ID than the one
//!   remembered, or the server no longer knowing this desktop (it signed it out): the desktop
//!   goes into **protective mode** ([`PROTECTED`]): nothing pulled is applied, nothing is
//!   pushed, and the copy, its files, and its outbox are kept. The app still reads and saves
//!   here.
//! - **A suspicious pull** ([`is_suspicious`]: more than 25 records and more than 10% of the
//!   copy deleted) waits too, until [`apply_held`] (a snapshot first).
//! - **Snapshots** of the copy: one a day before its first pull (`daily_snapshot`), and
//!   before every risky step; the 7 newest kept; [`restore`] puts one back.
//! - **The choices:** keep protected (nothing to do), [`use_this_server`] (a snapshot first),
//!   or, the household Admin with a complete copy and nothing unsent, [`rebuild`] a fresh
//!   server from this computer.

use std::path::PathBuf;
use std::sync::Mutex;

use dioxus::prelude::*;
use domain::id::DatabaseId;
use domain::log_area;
use domain::secret::Secret;
use domain::sync::{PullPage, RebuildOutcome, TemporaryPassword};
use domain::time::Clock;
use storage::outbox::OutboxState;
use storage::sync::{Deletions, PageCheck, PullScope};
use storage::{Database, LocalKey};

use crate::logging::SystemClock;

/// Why the desktop is protecting its copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Protection {
    /// The server's database ID isn't the one this copy is of: reset, or another server.
    /// `now` is the server's new ID, when known.
    ServerChanged {
        /// The server's database ID now, if known.
        now: Option<DatabaseId>,
    },
    /// The server's newest change number is below this copy's cursor: restored from an older
    /// snapshot.
    WentBack {
        /// This copy's cursor.
        cursor: u64,
        /// The server's newest change number.
        server_sequence: u64,
    },
    /// The server no longer knows this desktop (it signed it out, or forgot the pairing in a
    /// reset or restore): sign in again, rebuild, or delete the copy.
    Forgotten,
    /// A pull would delete a lot; it waits for a person.
    PullHeld {
        /// What it would delete.
        deletions: Deletions,
        /// The page's next cursor (so approving applies that page).
        next_cursor: u64,
    },
}

/// Why the desktop is protecting its copy, if it is (1A's strip and Messages).
pub static PROTECTED: GlobalSignal<Option<Protection>> = Signal::global(|| None);

/// A held page approved by a person: its next cursor.
static APPROVED: Mutex<Option<u64>> = Mutex::new(None);

fn approved_slot() -> std::sync::MutexGuard<'static, Option<u64>> {
    APPROVED
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// More than this many records deleted by one pull may be suspicious (#078)…
pub const SUSPICIOUS_RECORDS: u64 = 25;

/// …if they're also more than this percentage of the copy.
pub const SUSPICIOUS_PERCENT: u64 = 10;

/// Whether deleting `deletions` of a copy of `records` records waits for a person.
#[must_use]
pub const fn is_suspicious(deletions: u64, records: u64) -> bool {
    deletions > SUSPICIOUS_RECORDS
        && deletions.saturating_mul(100) > records.saturating_mul(SUSPICIOUS_PERCENT)
}

/// Goes into protective mode (or changes why).
pub fn protect(why: Protection) {
    if PROTECTED.peek().as_ref() == Some(&why) {
        return;
    }
    tracing::warn!(target: log_area::SYNC, ?why, "protecting this computer's copy: syncing stopped");
    *PROTECTED.write() = Some(why);
}

/// Whether the desktop is protecting its copy now.
#[must_use]
pub fn is_protected() -> bool {
    PROTECTED.peek().is_some()
}

/// Leaves protective mode (the copy deleted, or signed in again after being forgotten).
pub fn clear() {
    settled();
}

/// Leaves protective mode.
fn settled() {
    if PROTECTED.peek().is_some() {
        tracing::info!(target: log_area::SYNC, "protective mode settled; syncing again");
        *PROTECTED.write() = None;
    }
}

/// What to do with a pulled `page` (`local_copy`'s pull, before applying): `true` to apply it.
/// Otherwise it's protected against, and the pull stops.
pub(crate) async fn page_may_apply(copy: &Database, page: &PullPage) -> bool {
    match copy.check_pull_page(page).await {
        Ok(PageCheck::Fine(deletions)) => {
            if deletions.total == 0 {
                return true;
            }
            if approved_slot().take() == Some(page.next_cursor) {
                return true;
            }
            let records = copy.record_count().await.unwrap_or(u64::MAX);
            if !is_suspicious(deletions.total, records) {
                return true;
            }
            protect(Protection::PullHeld {
                deletions,
                next_cursor: page.next_cursor,
            });
            false
        }
        Ok(PageCheck::OtherServer) => {
            protect(Protection::ServerChanged {
                now: Some(page.database_id),
            });
            false
        }
        Ok(PageCheck::WentBack {
            cursor,
            server_sequence,
        }) => {
            protect(Protection::WentBack {
                cursor,
                server_sequence,
            });
            false
        }
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "couldn't check a pulled page; not applied");
            false
        }
    }
}

// --- Snapshots ---------------------------------------------------------------------------

/// Snapshots the open copy now (before a risky step). Whether it worked.
pub async fn snapshot(why: &str) -> bool {
    let (Some(copy), Some((folder, key))) = (
        crate::local_copy::current(),
        crate::local_copy::open_place(),
    ) else {
        return false;
    };
    match copy
        .snapshot_local(&folder, &LocalKey::new(key), SystemClock.now())
        .await
    {
        Ok(path) => {
            tracing::info!(target: log_area::SYNC, why, snapshot = %path.display(), "snapshotted this computer's copy");
            true
        }
        Err(error) => {
            tracing::error!(target: log_area::SYNC, why, %error, "couldn't snapshot this computer's copy");
            false
        }
    }
}

/// One snapshot a day: if the newest isn't from today (UTC), one now.
pub(crate) async fn daily_snapshot() {
    let Some((folder, _)) = crate::local_copy::open_place() else {
        return;
    };
    let today = domain::time_utils::format_utc_file_stamp(SystemClock.now());
    let day = today.get(..8).unwrap_or_default().to_owned();
    let newest = storage::local_snapshots::local_snapshots(&folder)
        .ok()
        .and_then(|found| found.into_iter().next());
    let taken_today = newest
        .and_then(|path| {
            path.file_name()
                .map(|name| name.to_string_lossy().into_owned())
        })
        .is_some_and(|name| name.contains(&format!("-{day}T")));
    if !taken_today {
        snapshot("daily").await;
    }
}

/// The open copy's snapshots, newest first (Diagnostics).
#[must_use]
pub fn snapshots() -> Vec<PathBuf> {
    crate::local_copy::open_place()
        .and_then(|(folder, _)| storage::local_snapshots::local_snapshots(&folder).ok())
        .unwrap_or_default()
}

/// Puts `snapshot` back as the copy: the copy closes, the snapshot takes its place, and it
/// opens again; the next pull catches up from its cursor. Whether it worked.
pub async fn restore(snapshot: PathBuf) -> bool {
    let Some((folder, key)) = crate::local_copy::open_place() else {
        return false;
    };
    crate::local_copy::close().await;
    let restored = storage::local_snapshots::restore_local_snapshot(&folder, &snapshot);
    if let Err(error) = &restored {
        tracing::error!(target: log_area::SYNC, %error, "couldn't restore a snapshot");
    }
    crate::local_copy::open(key).await;
    restored.is_ok()
}

// --- The choices ------------------------------------------------------------------------

/// **Apply them** (2A): the held pull applies, after a snapshot.
pub async fn apply_held() {
    let Some(Protection::PullHeld { next_cursor, .. }) = PROTECTED.peek().clone() else {
        return;
    };
    if !snapshot("before a big delete").await {
        return;
    }
    *approved_slot() = Some(next_cursor);
    settled();
    crate::local_copy::pull_soon();
}

/// **Use this server:** after a snapshot, the copy follows the server as it is now. A changed
/// server: a new copy of it, with this copy's outbox moved over (its changes are sent and
/// checked there as usual); one gone back in time: the copy downloads it again (outbox kept).
pub async fn use_this_server() {
    let Some(why) = PROTECTED.peek().clone() else {
        return;
    };
    if !snapshot("before using this server").await {
        return;
    }
    match why {
        Protection::ServerChanged { now } => {
            let now = match now {
                Some(now) => Some(now),
                None => handshake_database_id().await,
            };
            if let Some(now) = now {
                switch_to(now).await;
            }
        }
        Protection::WentBack { .. } => {
            if let Some(copy) = crate::local_copy::current()
                && let Err(error) = copy.forget_pulled().await
            {
                tracing::error!(target: log_area::SYNC, %error, "couldn't clear the copy to download it again");
                return;
            }
            settled();
            crate::local_copy::pull_soon();
        }
        Protection::Forgotten | Protection::PullHeld { .. } => {}
    }
}

/// The server's database ID now, from its handshake.
async fn handshake_database_id() -> Option<DatabaseId> {
    crate::client::within(crate::client::SESSION_WAIT, crate::api::handshake())
        .await
        .ok()?
        .database_id?
        .parse()
        .ok()
}

/// Makes `now` this server's database ID here and opens a new copy of it (same key), with the
/// old copy's outbox moved over.
async fn switch_to(now: DatabaseId) {
    let Some((_, key)) = crate::local_copy::open_place() else {
        return;
    };
    let entries = crate::local_copy::outbox_entries().await;
    crate::ui::recent_servers::accept_database_id(now);
    crate::local_copy::close().await;
    settled();
    crate::local_copy::open(key).await;
    if let Some(copy) = crate::local_copy::current() {
        for entry in entries {
            if let Err(error) = copy.add_to_outbox(&entry.operation, entry.made_at).await {
                tracing::warn!(target: log_area::SYNC, %error, "couldn't move a change to the new copy");
                continue;
            }
            if let OutboxState::Refused { refusal, at } = entry.state {
                let _ = copy
                    .refuse_in_outbox(entry.operation.id, &refusal, at)
                    .await;
            }
        }
    }
    crate::local_copy::count_outbox().await;
    tracing::info!(target: log_area::SYNC, database = %now, "using this server: a new copy of it");
}

// --- Rebuild ----------------------------------------------------------------------------

/// Why a rebuild didn't happen.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RebuildProblem {
    /// The copy wasn't complete at its last pull, or changes wait or were refused.
    NotReady,
    /// The setup code is wrong.
    WrongCode,
    /// Too many wrong codes; wait this many seconds.
    TooManyAttempts(i64),
    /// The server isn't fresh (it has people or vehicles).
    NotFresh,
    /// The person isn't the household's Admin, or the password breaks a rule.
    NotAllowed,
    /// Something failed (the log says what).
    Failed,
}

/// What a rebuild did.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Rebuilt {
    /// Everyone else's new temporary password, to give them.
    pub passwords: Vec<TemporaryPassword>,
    /// The attachments left out (their files weren't on this computer).
    pub left_out: Vec<String>,
}

/// Whether this computer may rebuild a server: its copy complete at its last pull, nothing
/// waiting or refused (3B's checks).
pub async fn may_rebuild() -> bool {
    let Some(copy) = crate::local_copy::current() else {
        return false;
    };
    let complete = copy
        .pull_state()
        .await
        .ok()
        .flatten()
        .is_some_and(|state| state.complete());
    let summary = copy.outbox_summary().await.unwrap_or_default();
    complete && summary.waiting == 0 && summary.refused == 0
}

/// Rebuilds the fresh server this desktop points at from its copy (3B), as the household
/// Admin whose desktop this is, with the server's setup `code` and the Admin's `password`
/// (theirs from now on): the records, then signing in (pairing), the files it has, and
/// settling; then this desktop uses the rebuilt server.
///
/// # Errors
///
/// [`RebuildProblem`].
pub async fn rebuild(code: Secret, password: Secret) -> Result<Rebuilt, RebuildProblem> {
    if !may_rebuild().await {
        return Err(RebuildProblem::NotReady);
    }
    let copy = crate::local_copy::current().ok_or(RebuildProblem::NotReady)?;
    let from = copy
        .pull_state()
        .await
        .ok()
        .flatten()
        .ok_or(RebuildProblem::NotReady)?
        .database_id;
    let username = crate::device_credential::load(crate::local_copy::server_address())
        .and_then(|sealed| sealed.username)
        .ok_or(RebuildProblem::NotAllowed)?;
    let admin = copy
        .user_by_username(
            &domain::user::Username::parse(&username).map_err(|_| RebuildProblem::NotAllowed)?,
        )
        .await
        .ok()
        .flatten()
        .ok_or(RebuildProblem::NotAllowed)?;
    if !snapshot("before a rebuild").await {
        return Err(RebuildProblem::Failed);
    }
    let token = begin(code, from).await?;
    send_records(&copy, &token, from, admin.id).await?;
    let passwords = finish(&token, &username, password.clone()).await?;
    sign_in_again(&username, &password).await?;
    restore_files(&copy).await;
    let left_out = match crate::api::rebuild_settle().await {
        Ok(RebuildOutcome::Settled { left_out }) => left_out,
        other => {
            tracing::warn!(target: log_area::SYNC, ?other, "couldn't settle the rebuild's files");
            Vec::new()
        }
    };
    if let Some(now) = handshake_database_id().await {
        protect(Protection::ServerChanged { now: Some(now) });
        switch_to(now).await;
    }
    tracing::info!(target: log_area::SYNC, people = passwords.len() + 1, left_out = left_out.len(), "rebuilt the server from this computer");
    Ok(Rebuilt {
        passwords,
        left_out,
    })
}

async fn begin(code: Secret, from: DatabaseId) -> Result<Secret, RebuildProblem> {
    match crate::api::rebuild_begin(code, from).await {
        Ok(RebuildOutcome::Started { token }) => Ok(token),
        Ok(RebuildOutcome::WrongCode) => Err(RebuildProblem::WrongCode),
        Ok(RebuildOutcome::TooManyAttempts {
            retry_after_seconds,
        }) => Err(RebuildProblem::TooManyAttempts(retry_after_seconds)),
        Ok(RebuildOutcome::NotFresh) => Err(RebuildProblem::NotFresh),
        other => {
            tracing::warn!(target: log_area::SYNC, ?other, "couldn't start the rebuild");
            Err(RebuildProblem::Failed)
        }
    }
}

/// Sends the copy's records, page by page (up to the newest change number they hold).
async fn send_records(
    copy: &Database,
    token: &Secret,
    from: DatabaseId,
    admin: domain::user::UserId,
) -> Result<(), RebuildProblem> {
    let scope = PullScope {
        user: admin,
        household: from,
    };
    let newest = copy
        .newest_sequence()
        .await
        .map_err(|_| RebuildProblem::Failed)?;
    let schema = storage::sync::schema_version();
    let mut cursor = 0;
    loop {
        let read = copy
            .read_page_upto(&scope, cursor, 200, newest)
            .await
            .map_err(|_| RebuildProblem::Failed)?;
        let page = PullPage {
            database_id: from,
            schema,
            cursor,
            next_cursor: read.next_cursor,
            server_sequence: read.server_sequence,
            tables: read.tables,
        };
        match crate::api::rebuild_page(token.clone(), page).await {
            Ok(RebuildOutcome::PageTaken) => {}
            other => {
                tracing::warn!(target: log_area::SYNC, ?other, "a rebuild page wasn't taken");
                return Err(RebuildProblem::Failed);
            }
        }
        if read.next_cursor >= newest {
            return Ok(());
        }
        cursor = read.next_cursor;
    }
}

async fn finish(
    token: &Secret,
    username: &str,
    password: Secret,
) -> Result<Vec<TemporaryPassword>, RebuildProblem> {
    match crate::api::rebuild_finish(token.clone(), username.to_owned(), password).await {
        Ok(RebuildOutcome::Finished { passwords }) => Ok(passwords),
        Ok(RebuildOutcome::NotAllowed | RebuildOutcome::Invalid) => Err(RebuildProblem::NotAllowed),
        other => {
            tracing::warn!(target: log_area::SYNC, ?other, "couldn't finish the rebuild");
            Err(RebuildProblem::Failed)
        }
    }
}

/// Signs in to the rebuilt server as usual, pairing (the PIN step follows, and keeps this
/// copy's key).
async fn sign_in_again(username: &str, password: &Secret) -> Result<(), RebuildProblem> {
    let (key, pairing) = crate::pairing::new_pairing()
        .map_or((None, None), |(key, pairing)| (Some(key), Some(pairing)));
    match crate::api::sign_in(username.to_owned(), password.clone(), true, pairing).await {
        Ok(domain::api::SignInOutcome::SignedIn { token, .. }) => {
            crate::pairing::finish_sign_in(token, key, password);
            Ok(())
        }
        other => {
            tracing::warn!(target: log_area::SYNC, ?other, "couldn't sign in to the rebuilt server");
            Err(RebuildProblem::Failed)
        }
    }
}

/// Uploads every file of the copy's attachments that this computer has, exactly as it is.
async fn restore_files(copy: &Database) {
    let Ok(attachments) = copy.all_attachments().await else {
        return;
    };
    for (_, attachment) in attachments {
        for file in attachment.files() {
            let Some(bytes) = crate::copy_files::bytes_of(&file.hash).await else {
                continue;
            };
            if let Err(error) =
                crate::files::upload(domain::api::UploadPurpose::Restore, bytes).await
            {
                tracing::warn!(target: log_area::SYNC, %error, "couldn't restore a file");
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_pull_is_suspicious_past_25_records_and_10_percent() {
        assert!(!is_suspicious(0, 1_000));
        assert!(!is_suspicious(25, 100), "25 is the line, not over it");
        assert!(is_suspicious(26, 100));
        assert!(!is_suspicious(100, 1_000), "10% exactly");
        assert!(is_suspicious(101, 1_000));
        assert!(!is_suspicious(200, 10_000), "many, but a small share");
    }
}
