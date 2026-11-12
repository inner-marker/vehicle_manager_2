//! This computer's encrypted copy of its server's records (phase-3.md 4c, decision #075).
//!
//! - **Where:** `servers/<database ID>` in the data folder, with `-<instance>` for a named
//!   instance (`-test`), so main and a test server copied from it never share a copy (#062).
//!   The ID and instance come from the recent-servers list (`settings.json`), which the
//!   handshake fills in, so the folder is found with no connection.
//! - **Open while unlocked:** [`open`] at unlock (or right after choosing a PIN), with the key
//!   sealed with the sign-in; [`close`] when the desktop locks. A copy the key doesn't open
//!   (left behind by an earlier pairing) is deleted and started again.
//! - **Pulled:** [`CopyPuller`] pulls pages until the copy is up to date, then every minute, or
//!   sooner after [`pull_soon`] (a change made on the server; 4d).
//! - **Deleted** with the sign-in ([`delete()`]): signing out, or the 10th wrong try.
//!
//! What the copy is doing is in [`STATUS`], for Diagnostics (4e).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use dioxus::prelude::*;
use domain::id::DatabaseId;
use domain::log_area;
use domain::sync::{PullOutcome, PullPage};
use domain::time::{Clock, UnixMillis};
use storage::local::delete_local;
use storage::sync::schema_version;
use storage::{CopyOf, Database, LocalKey, StorageError};

use crate::logging::SystemClock;

/// How often an up-to-date copy asks for news.
const PULL_EVERY: Duration = Duration::from_mins(1);

/// The folder, in the data folder, that holds every server's copy.
pub const SERVERS_FOLDER: &str = "servers";

/// What the copy is doing.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub enum CopyStatus {
    /// No copy open (locked, signed out, or a server this desktop hasn't remembered yet).
    #[default]
    Closed,
    /// Downloading: `cursor` of the server's `of` change numbers.
    Downloading {
        /// The copy's cursor.
        cursor: u64,
        /// The server's newest change number.
        of: u64,
    },
    /// Up to date as of `at`.
    UpToDate {
        /// When the last pull finished.
        at: UnixMillis,
    },
    /// The server couldn't be reached; the copy is as of `since` (if it has been pulled).
    Offline {
        /// The last pull, if any.
        since: Option<UnixMillis>,
    },
    /// The server's schema version differs from the app's: update one of them (#075).
    OtherSchema {
        /// The server's.
        server: i64,
        /// This app's.
        app: i64,
    },
    /// Something went wrong (in the log).
    Failed,
}

/// What the copy is doing, for Diagnostics.
pub static STATUS: GlobalSignal<CopyStatus> = Signal::global(CopyStatus::default);

/// The open copy, while unlocked.
static COPY: Mutex<Option<Database>> = Mutex::new(None);

/// Wakes the puller early.
static PULL_NOW: tokio::sync::Notify = tokio::sync::Notify::const_new();

fn copy_slot() -> std::sync::MutexGuard<'static, Option<Database>> {
    COPY.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The open copy, if there is one.
#[must_use]
pub fn current() -> Option<Database> {
    copy_slot().clone()
}

/// The server this desktop uses now.
fn server() -> &'static str {
    dioxus_fullstack::get_server_url()
}

/// The folder name for a server's copy: its database ID, and `-<instance>` for a named one;
/// `None` for an instance name that isn't lowercase letters and digits (the server's own rule,
/// #062), which would otherwise reach outside the folder (`../`).
#[must_use]
pub fn folder_name(database_id: DatabaseId, instance: Option<&str>) -> Option<String> {
    match instance.filter(|name| !name.is_empty()) {
        Some(instance)
            if instance
                .bytes()
                .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit()) =>
        {
            Some(format!("{database_id}-{instance}"))
        }
        Some(_) => None,
        None => Some(database_id.to_string()),
    }
}

/// Where `address`'s copy lives and which server it's of, from the recent-servers list; `None`
/// if this desktop doesn't know the server's database ID yet.
fn place_of(address: &str) -> Option<(PathBuf, CopyOf)> {
    let data_folder = crate::data_folder::platform_data_folder()?;
    let known = crate::settings::load(&data_folder)
        .servers
        .into_iter()
        .find(|known| known.address == address)?;
    let database_id: DatabaseId = known.database_id.as_deref()?.parse().ok()?;
    let instance = known.instance.unwrap_or_default();
    let folder = data_folder
        .join(SERVERS_FOLDER)
        .join(folder_name(database_id, Some(&instance))?);
    Some((
        folder,
        CopyOf {
            database_id,
            instance,
        },
    ))
}

/// A new key for a new copy: 32 random bytes from the operating system.
///
/// # Errors
///
/// What went wrong, if the OS has no randomness.
pub fn new_key() -> Result<[u8; 32], String> {
    let mut key = [0; 32];
    getrandom::fill(&mut key).map_err(|error| error.to_string())?;
    Ok(key)
}

/// Opens the copy in `folder`; one the key doesn't open (left behind by an earlier pairing,
/// whose key is gone) is deleted and started again.
async fn open_or_restart(
    folder: &std::path::Path,
    key: &LocalKey,
    copy_of: &CopyOf,
) -> Result<Database, StorageError> {
    let now = SystemClock.now();
    match Database::open_local(folder, key, copy_of, now).await {
        Err(StorageError::WrongKey | StorageError::OtherServer) => {
            tracing::warn!(target: log_area::SYNC, "a copy this key doesn't open was here; starting a new one");
            delete_local(folder)?;
            Database::open_local(folder, key, copy_of, now).await
        }
        other => other,
    }
    .map(|(database, _)| database)
}

/// Opens this server's copy with `key` (creating it if there's none), so screens can read it
/// and the puller can fill it. Nothing if the server's database ID isn't known yet.
pub async fn open(key: [u8; 32]) {
    let Some((folder, copy_of)) = place_of(server()) else {
        tracing::info!(target: log_area::SYNC, "no copy yet: this server's database ID isn't known");
        return;
    };
    match open_or_restart(&folder, &LocalKey::new(key), &copy_of).await {
        Ok(database) => {
            *copy_slot() = Some(database);
            PULL_NOW.notify_one();
        }
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "couldn't open this computer's copy");
            *STATUS.write() = CopyStatus::Failed;
        }
    }
}

/// Closes the copy (the desktop locked): nothing reads it until it's opened again.
pub async fn close() {
    let open = copy_slot().take();
    if let Some(database) = open {
        database.close().await;
        tracing::debug!(target: log_area::SYNC, "this computer's copy is closed");
    }
    *STATUS.write() = CopyStatus::Closed;
}

/// Closes and deletes this server's copy (signing out, or too many wrong tries; #075).
pub async fn delete() {
    close().await;
    if let Some((folder, _)) = place_of(server())
        && let Err(error) = delete_local(&folder)
    {
        tracing::warn!(target: log_area::SYNC, %error, "couldn't delete this computer's copy");
    }
}

/// Asks the puller to pull now (after a change on the server).
pub fn pull_soon() {
    PULL_NOW.notify_one();
}

/// The page in `outcome`, or `None` after recording why there's none (`last_pull`: the
/// copy's last pull, for "offline since").
fn page_of(
    outcome: Result<PullOutcome, ServerFnError>,
    last_pull: Option<UnixMillis>,
) -> Option<Box<PullPage>> {
    let status = match outcome {
        Ok(PullOutcome::Page { page }) => return Some(page),
        Ok(PullOutcome::OtherSchema { server }) => {
            tracing::info!(target: log_area::SYNC, server, app = schema_version(), "no pull: the server's schema version differs");
            CopyStatus::OtherSchema {
                server,
                app: schema_version(),
            }
        }
        Ok(other) => {
            tracing::warn!(target: log_area::SYNC, ?other, "the server refused the pull");
            CopyStatus::Failed
        }
        Err(error) => {
            tracing::debug!(target: log_area::SYNC, %error, "no pull: can't reach the server");
            CopyStatus::Offline { since: last_pull }
        }
    };
    *STATUS.write() = status;
    None
}

/// Pulls pages into `copy` until it's up to date, the server can't be reached, or it refuses.
async fn pull_until_current(copy: &Database) {
    loop {
        let Ok(Some(state)) = copy.pull_state().await else {
            *STATUS.write() = CopyStatus::Failed;
            return;
        };
        let outcome = crate::api::pull(state.cursor, schema_version()).await;
        let Some(page) = page_of(outcome, state.last_pull) else {
            return;
        };
        if let Err(error) = copy.apply_pull_page(&page, SystemClock.now()).await {
            tracing::error!(target: log_area::SYNC, %error, "couldn't apply a pulled page");
            *STATUS.write() = CopyStatus::Failed;
            return;
        }
        *STATUS.write() = if page.up_to_date() {
            CopyStatus::UpToDate {
                at: SystemClock.now(),
            }
        } else {
            CopyStatus::Downloading {
                cursor: page.next_cursor,
                of: page.server_sequence,
            }
        };
        if page.up_to_date() {
            return;
        }
    }
}

/// Keeps the open copy up to date: pulls when it opens, every minute, and after
/// [`pull_soon`]. Renders nothing.
#[component]
pub fn CopyPuller() -> Element {
    use_future(|| async {
        loop {
            if let Some(copy) = current() {
                pull_until_current(&copy).await;
            }
            let _ = tokio::time::timeout(PULL_EVERY, PULL_NOW.notified()).await;
        }
    });
    rsx! {}
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_named_instance_gets_its_own_folder() {
        let id: DatabaseId = domain::test_support::test_id(1);
        assert_eq!(folder_name(id, None), Some(id.to_string()));
        assert_eq!(folder_name(id, Some("")), Some(id.to_string()));
        assert_eq!(folder_name(id, Some("test2")), Some(format!("{id}-test2")));
        for hostile in ["../x", "a/b", "Test", "a b", "..", "x\\y"] {
            assert_eq!(folder_name(id, Some(hostile)), None, "{hostile}");
        }
    }
}
