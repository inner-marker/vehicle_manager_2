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
//! - **Changes made on the desktop** (5d, decision #076) go into its **outbox** and onto the
//!   copy (`crate::data`), and are sent ([`send_now`] after a save, and before every pull);
//!   after the server refuses one, the copy's records are downloaded again.
//!
//! What the copy is doing is in [`STATUS`], for Diagnostics (4e).

use std::path::PathBuf;
use std::sync::Mutex;
use std::time::Duration;

use dioxus::prelude::*;
use domain::hlc::HlcClock;
use domain::id::DatabaseId;
use domain::log_area;
use domain::operation::{Operation, OperationChange, OperationId, Refusal};
use domain::sync::{OperationAnswer, PUSH_LIMIT, PullOutcome, PullPage, PushOutcome, PushStop};
use domain::time::{Clock, UnixMillis};
use storage::local::delete_local;
use storage::outbox::OutboxState;
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

/// One pull at a time: the puller and a read's catching up never apply pages together.
static PULL_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// The copy's key, kept while the server's database ID isn't known yet (a first pairing,
/// before the session check has remembered the server); [`open_if_waiting`] uses it.
static WAITING_KEY: Mutex<Option<[u8; 32]>> = Mutex::new(None);

/// The signed-in person's session, as the auth state last said (for reading the copy as them).
static SESSION: Mutex<Option<domain::api::SessionInfo>> = Mutex::new(None);

fn copy_slot() -> std::sync::MutexGuard<'static, Option<Database>> {
    COPY.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn waiting_key() -> std::sync::MutexGuard<'static, Option<[u8; 32]>> {
    WAITING_KEY
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

fn session_slot() -> std::sync::MutexGuard<'static, Option<domain::api::SessionInfo>> {
    SESSION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The signed-in person, as the auth state last said.
#[must_use]
pub fn session() -> Option<domain::api::SessionInfo> {
    session_slot().clone()
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

/// The current server's instance name as last remembered (`None` for a main server), for its
/// TEST SERVER tag while offline.
#[must_use]
pub fn known_instance() -> Option<String> {
    place_of(server())
        .map(|(_, copy_of)| copy_of.instance)
        .filter(|instance| !instance.is_empty())
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
        tracing::info!(target: log_area::SYNC, "the copy waits: this server's database ID isn't known yet");
        *waiting_key() = Some(key);
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

/// Opens the copy if its key was waiting for the server's database ID (called once the
/// session check has remembered the server).
pub async fn open_if_waiting() {
    let key = waiting_key().take();
    if let Some(key) = key {
        open(key).await;
    }
}

/// Closes the copy (the desktop locked): nothing reads it until it's opened again.
pub async fn close() {
    *waiting_key() = None;
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

/// Why the last push stopped before the end, if it did (an app or server to update, a clock
/// ahead, the server busy): for the top bar and Diagnostics (5e).
pub static PUSH_STOPPED: GlobalSignal<Option<PushStop>> = Signal::global(|| None);

/// The copy shows a change the server refused: download its records again once nothing waits
/// to be sent (5d).
static DOWNLOAD_AGAIN: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

/// The most bytes of operations one push sends (the server takes 2 MB a request).
const PUSH_BYTES: usize = 1024 * 1024;

/// A new operation for `change`, made now on this desktop's clock (which moves past it).
///
/// # Errors
///
/// Why none could be made (no randomness, or a clock out of range).
pub async fn new_operation(copy: &Database, change: OperationChange) -> Result<Operation, String> {
    let now = SystemClock.now();
    let id: OperationId = ::server::ids::new_id(now).map_err(|error| error.to_string())?;
    let last = copy.device_hlc().await.map_err(|error| error.to_string())?;
    let hlc = HlcClock::new(SystemClock, last)
        .tick()
        .map_err(|error| error.to_string())?;
    copy.advance_device_hlc(hlc)
        .await
        .map_err(|error| error.to_string())?;
    Ok(Operation::new(id, hlc, change))
}

/// What sending a change at once did.
pub enum Sent {
    /// The server applied it.
    Applied,
    /// The server refused it (taken out of the outbox: the form shows why).
    Refused(Refusal),
    /// Not sent yet (offline, or the server stopped before it): it waits in the outbox.
    Waiting,
}

/// Sends the outbox now, for a change just saved (`id`), and says what became of it. Offline
/// it waits.
pub async fn send_now(copy: &Database, id: OperationId) -> Sent {
    if is_offline() {
        return Sent::Waiting;
    }
    {
        let _one_at_a_time = PULL_LOCK.lock().await;
        let last_pull = copy
            .pull_state()
            .await
            .ok()
            .flatten()
            .and_then(|state| state.last_pull);
        push_waiting(copy, last_pull).await;
    }
    let sent = match copy.outbox_entry(id).await {
        Ok(None) => Sent::Applied,
        Ok(Some(entry)) => match entry.state {
            OutboxState::Refused { refusal, .. } => {
                // Answered in the form, not in Messages.
                if let Err(error) = copy.remove_from_outbox(id).await {
                    tracing::warn!(target: log_area::SYNC, %error, "couldn't take a refused change out of the outbox");
                }
                Sent::Refused(refusal)
            }
            OutboxState::Waiting => Sent::Waiting,
        },
        Err(error) => {
            tracing::warn!(target: log_area::SYNC, %error, "couldn't read the outbox");
            Sent::Waiting
        }
    };
    pull_soon();
    sent
}

/// The oldest waiting operations, up to [`PUSH_LIMIT`] and [`PUSH_BYTES`] (at least one).
fn first_batch(waiting: Vec<Operation>) -> Vec<Operation> {
    let mut bytes = 0;
    let mut batch = Vec::new();
    for operation in waiting {
        bytes += serde_json::to_vec(&operation).map_or(0, |json| json.len());
        if !batch.is_empty() && bytes > PUSH_BYTES {
            break;
        }
        batch.push(operation);
    }
    batch
}

/// What one batch of a push did.
enum Batch {
    /// All answered: send the next.
    Next,
    /// Stop pushing; whether the pull goes on (the server could be reached, same schema).
    Stop(bool),
}

/// Sends the outbox, oldest first, until it's empty or the server stops (called with the pull
/// lock held). Whether the server could be reached on the same schema (the pull goes on).
async fn push_waiting(copy: &Database, last_pull: Option<UnixMillis>) -> bool {
    loop {
        let waiting = match copy
            .outbox_waiting(u32::try_from(PUSH_LIMIT).unwrap_or(100))
            .await
        {
            Ok(waiting) => waiting,
            Err(error) => {
                tracing::error!(target: log_area::SYNC, %error, "couldn't read the outbox");
                return true;
            }
        };
        if waiting.is_empty() {
            break;
        }
        match push_batch(copy, first_batch(waiting), last_pull).await {
            Batch::Next => {}
            Batch::Stop(go_on) => return go_on,
        }
    }
    if DOWNLOAD_AGAIN.swap(false, std::sync::atomic::Ordering::Relaxed)
        && let Err(error) = copy.forget_pulled().await
    {
        tracing::error!(target: log_area::SYNC, %error, "couldn't clear the copy to download it again");
        DOWNLOAD_AGAIN.store(true, std::sync::atomic::Ordering::Relaxed);
    }
    true
}

/// Sends one batch and takes in the answers.
async fn push_batch(
    copy: &Database,
    batch: Vec<Operation>,
    last_pull: Option<UnixMillis>,
) -> Batch {
    let sent = batch.len();
    let outcome = crate::client::within(
        crate::client::PULL_WAIT,
        crate::api::push(schema_version(), batch),
    )
    .await;
    match outcome {
        Ok(PushOutcome::Answers { answers, stopped }) => {
            take_answers(copy, &answers).await;
            if *PUSH_STOPPED.peek() != stopped {
                *PUSH_STOPPED.write() = stopped;
            }
            if let Some(stop) = stopped {
                tracing::info!(target: log_area::SYNC, ?stop, "the server stopped the push");
                return Batch::Stop(true);
            }
            if answers.len() < sent {
                Batch::Stop(true)
            } else {
                Batch::Next
            }
        }
        Ok(PushOutcome::OtherSchema { server }) => {
            *STATUS.write() = CopyStatus::OtherSchema {
                server,
                app: schema_version(),
            };
            Batch::Stop(false)
        }
        Ok(other) => {
            tracing::warn!(target: log_area::SYNC, ?other, "the server refused the push");
            Batch::Stop(true)
        }
        Err(error) => {
            tracing::debug!(target: log_area::SYNC, %error, "no push: can't reach the server");
            *STATUS.write() = CopyStatus::Offline { since: last_pull };
            Batch::Stop(false)
        }
    }
}

/// Takes the server's answers out of the outbox: applied ones leave it, refused ones stay,
/// with why, and the copy is downloaded again.
async fn take_answers(copy: &Database, answers: &[OperationAnswer]) {
    for answer in answers {
        let result = match answer {
            OperationAnswer::Applied { id } => copy.remove_from_outbox(*id).await.map(|_| ()),
            OperationAnswer::Refused { id, refusal } => {
                tracing::info!(target: log_area::SYNC, operation = %id, ?refusal, "the server refused a change");
                DOWNLOAD_AGAIN.store(true, std::sync::atomic::Ordering::Relaxed);
                copy.refuse_in_outbox(*id, refusal, SystemClock.now())
                    .await
                    .map(|_| ())
            }
        };
        if let Err(error) = result {
            tracing::error!(target: log_area::SYNC, %error, "couldn't record the server's answer");
        }
    }
}

/// Pulls pages into `copy` until it's up to date, the server can't be reached, or it refuses;
/// the outbox goes first, so a pull never covers a change that hasn't been sent.
async fn pull_until_current(copy: &Database) {
    let _one_at_a_time = PULL_LOCK.lock().await;
    let last_pull = copy
        .pull_state()
        .await
        .ok()
        .flatten()
        .and_then(|state| state.last_pull);
    if !push_waiting(copy, last_pull).await {
        return;
    }
    loop {
        let Ok(Some(state)) = copy.pull_state().await else {
            *STATUS.write() = CopyStatus::Failed;
            return;
        };
        let outcome = crate::client::within(
            crate::client::PULL_WAIT,
            crate::api::pull(state.cursor, schema_version()),
        )
        .await;
        let Some(page) = page_of(outcome, state.last_pull) else {
            return;
        };
        if let Err(error) = copy.apply_pull_page(&page, SystemClock.now()).await {
            tracing::error!(target: log_area::SYNC, %error, "couldn't apply a pulled page");
            *STATUS.write() = CopyStatus::Failed;
            return;
        }
        // The desktop's clock moves past every time it has seen (#043).
        if let Ok(newest) = copy.newest_field_hlc().await
            && let Err(error) = copy.advance_device_hlc(newest).await
        {
            tracing::warn!(target: log_area::SYNC, %error, "couldn't move the desktop's clock");
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

/// The copy, caught up, for a screen to read (phase-3.md 4d): `None` if there's no open copy,
/// it hasn't finished its first download, or the server's schema version differs (then the
/// screen asks the server). Online, it pulls first, so a change just made on the server
/// shows; offline, it reads what it has.
pub async fn for_reading() -> Option<(Database, storage::sync::PullState)> {
    let copy = current()?;
    let status = STATUS.peek().clone();
    if !matches!(
        status,
        CopyStatus::Offline { .. } | CopyStatus::OtherSchema { .. }
    ) {
        pull_until_current(&copy).await;
    }
    if matches!(
        *STATUS.peek(),
        CopyStatus::OtherSchema { .. } | CopyStatus::Failed
    ) {
        return None;
    }
    let state = copy.pull_state().await.ok()??;
    state.complete().then_some((copy, state))
}

/// Keeps the open copy up to date: pulls when it opens, every minute, and after
/// [`pull_soon`]; and keeps the signed-in person for reading the copy. Renders nothing.
#[component]
pub fn CopyPuller() -> Element {
    let auth = crate::ui::auth::use_auth();
    use_effect(move || {
        use crate::ui::auth::AuthState;
        match auth() {
            AuthState::SignedIn(session) => *session_slot() = Some(session),
            AuthState::SignedOut | AuthState::NeedsSetup => *session_slot() = None,
            // Unreachable: the copy is read as the person last signed in (offline, 4e).
            AuthState::Loading | AuthState::Unreachable(_) => {}
        }
    });
    use_future(move || async move {
        loop {
            if let Some(copy) = current() {
                let was_offline = is_offline();
                pull_until_current(&copy).await;
                // Back online: the real session replaces the offline one (4e).
                if was_offline && !is_offline() {
                    tracing::info!(target: log_area::SYNC, "the server can be reached again");
                    crate::ui::auth::refresh(auth).await;
                }
            }
            let _ = tokio::time::timeout(PULL_EVERY, PULL_NOW.notified()).await;
        }
    });
    rsx! {}
}

/// Whether the last pull couldn't reach the server (screens show the OFFLINE tag and turn
/// saving off; phase-3.md 4e).
#[must_use]
pub fn is_offline() -> bool {
    matches!(*STATUS.peek(), CopyStatus::Offline { .. })
}

/// While a session check waits with the copy open (the PIN was accepted): when it gives up and
/// the copy opens offline. The waiting screen counts down to it (`ui::auth`).
pub static CONNECTING: GlobalSignal<Option<std::time::Instant>> = Signal::global(|| None);

/// "Start in Offline Mode Now" on the waiting screen.
static OFFLINE_NOW: tokio::sync::Notify = tokio::sync::Notify::const_new();

/// Stops waiting for the server: the session check in progress counts it as unreachable, so the
/// copy opens offline. Nothing if no check is waiting.
pub fn open_offline_now() {
    tracing::info!(target: log_area::SYNC, "opening this computer's copy without waiting for the server");
    OFFLINE_NOW.notify_waiters();
}

/// Asks `request` (the session check) for up to `crate::client::SESSION_WAIT`. With the copy
/// open, [`CONNECTING`] says when that ends, and [`open_offline_now`] ends it sooner.
///
/// # Errors
///
/// The request's error, or a timeout (which reads as "can't reach the server").
pub async fn check_session<T>(
    request: impl Future<Output = Result<T, ServerFnError>>,
) -> Result<T, ServerFnError> {
    let wait = crate::client::SESSION_WAIT;
    if current().is_none() {
        return crate::client::within(wait, request).await;
    }
    *CONNECTING.write() = Some(std::time::Instant::now() + wait);
    let answer = unless_skipped(crate::client::within(wait, request), OFFLINE_NOW.notified()).await;
    *CONNECTING.write() = None;
    answer
}

/// `request`'s answer, or a timeout as soon as `skip` finishes first.
async fn unless_skipped<T>(
    request: impl Future<Output = Result<T, ServerFnError>>,
    skip: impl Future<Output = ()>,
) -> Result<T, ServerFnError> {
    let mut request = std::pin::pin!(request);
    let mut skip = std::pin::pin!(skip);
    std::future::poll_fn(|context| {
        if let std::task::Poll::Ready(answer) = request.as_mut().poll(context) {
            return std::task::Poll::Ready(answer);
        }
        skip.as_mut().poll(context).map(|()| {
            Err(ServerFnError::Request(
                dioxus_fullstack::RequestError::Timeout("opened offline without waiting".into()),
            ))
        })
    })
    .await
}

/// The session to open the app with when the server can't be reached (phase-3.md 4e): the
/// person this desktop is sealed for, in **Driver mode** (Admin mode needs the server), with no
/// mode to switch to. `None` unless the copy is open and was complete at its last pull.
pub async fn offline_session() -> Option<domain::api::SessionInfo> {
    let copy = current()?;
    let state = copy.pull_state().await.ok()??;
    if !state.complete() {
        return None;
    }
    let sealed = crate::device_credential::load(server())?;
    let username = domain::user::Username::parse(sealed.username.as_deref()?).ok()?;
    copy.user_by_username(&username).await.ok()??;
    tracing::info!(target: log_area::SYNC, "the server can't be reached; opening this computer's copy");
    *STATUS.write() = CopyStatus::Offline {
        since: state.last_pull,
    };
    Some(domain::api::SessionInfo {
        display_name: sealed
            .person
            .unwrap_or_else(|| username.as_str().to_owned()),
        username,
        must_change_password: false,
        mode: domain::access::ActiveMode::Driver,
        available_modes: Vec::new(),
        timers: domain::timers::Timers::NORMAL,
    })
}

/// What Diagnostics shows about the copy (2B): its folder, its size on disk, and how many
/// records it holds.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopyDetails {
    /// The folder.
    pub folder: PathBuf,
    /// Its files' total size, in bytes.
    pub bytes: u64,
    /// The records in it (live and deleted).
    pub records: u64,
    /// When the last pull finished, if one has.
    pub last_pull: Option<UnixMillis>,
}

/// The open copy's details, if there is one.
pub async fn details() -> Option<CopyDetails> {
    let copy = current()?;
    let (folder, _) = place_of(server())?;
    let bytes = std::fs::read_dir(&folder)
        .ok()?
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(std::fs::Metadata::is_file)
        .map(|metadata| metadata.len())
        .sum();
    let records = copy.record_count().await.ok()?;
    let last_pull = copy.pull_state().await.ok()??.last_pull;
    Some(CopyDetails {
        folder,
        bytes,
        records,
        last_pull,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_push_batch_stays_under_its_size_but_always_sends_one() {
        use domain::hlc::Hlc;
        use domain::test_support::test_id;
        // A Garage order of `vehicles` IDs: about 40 bytes each as JSON.
        let order = |n: u32, vehicles: u32| {
            Operation::new(
                test_id(n),
                Hlc::new(UnixMillis::from_millis(1_700_000_000_000), 0).unwrap(),
                OperationChange::SetGarageOrder {
                    order: (0..vehicles).map(test_id).collect(),
                },
            )
        };
        assert_eq!(
            first_batch((0..50).map(|n| order(n, 1)).collect()).len(),
            50
        );
        // About 600 KB each: the second would pass 1 MB.
        let big = vec![order(1, 15_000), order(2, 15_000), order(3, 1)];
        assert_eq!(first_batch(big).len(), 1);
        // One bigger than the limit still goes, alone.
        assert_eq!(first_batch(vec![order(1, 40_000), order(2, 1)]).len(), 1);
        assert!(first_batch(Vec::new()).is_empty());
    }

    #[tokio::test]
    async fn starting_offline_now_stops_waiting_for_the_server() {
        let skipped = unless_skipped(std::future::pending::<Result<(), ServerFnError>>(), async {
        })
        .await;
        assert!(matches!(
            skipped,
            Err(ServerFnError::Request(
                dioxus_fullstack::RequestError::Timeout(_)
            ))
        ));
        let answered = unless_skipped(async { Ok::<_, ServerFnError>(7) }, async {}).await;
        assert!(matches!(answered, Ok(7)), "an answer wins over a skip");
        let waited =
            unless_skipped(async { Ok::<_, ServerFnError>(7) }, std::future::pending()).await;
        assert!(matches!(waited, Ok(7)));
    }

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
