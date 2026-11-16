//! Rebuilding a fresh server from a desktop's copy (phase-3.md 7b, decision #078; plan §6).
//!
//! When the household's server is lost, the household Admin's desktop (its copy complete and
//! nothing unsent: the desktop checks) rebuilds a **fresh** server (no people or vehicles yet:
//! it shows setup):
//!
//! 1. [`begin`] with the server's **setup code** (from its log: what proves access to it, as
//!    for setup) and the old server's database ID: a token for this rebuild (memory only).
//! 2. [`page`] for each page of the copy's records (the pull's format, the other way), with
//!    their field clocks and tombstones, so newest-wins carries on.
//! 3. [`finish`] with the Admin's username and password: the household's settings moved to
//!    this server's database ID, the change numbers moved past the imported ones, the Admin's
//!    password set, and everyone else given a new temporary password (returned once). Setup
//!    closes.
//! 4. The desktop signs in as usual (pairing), uploads the files it has
//!    (`UploadPurpose::Restore`, stored exactly as sent), then [`settle`]s: attachments whose
//!    files never came are left out (and named), and an original or metadata that never came is
//!    taken out of its attachment.

use std::sync::Mutex;

use domain::access::{GrantTarget, Role};
use domain::attachment::Content;
use domain::command::Change;
use domain::id::{DatabaseId, DeviceId};
use domain::log_area;
use domain::secret::Secret;
use domain::sync::{PullPage, RebuildOutcome, TemporaryPassword};
use domain::time::Clock;
use domain::user::{UserPatch, check_new_password};
use storage::users::UserWriter;
use storage::{Database, StorageError};
use subtle::ConstantTimeEq;

use crate::commands::issue;
use crate::health::Health;
use crate::passwords::hash_password;
use crate::sessions::SignedIn;

/// The rebuild going on: its token and the server whose copy it's from.
struct Session {
    token: String,
    from: DatabaseId,
}

static SESSION: Mutex<Option<Session>> = Mutex::new(None);

fn session() -> std::sync::MutexGuard<'static, Option<Session>> {
    SESSION
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The server whose copy the rebuild with `token` is from, if that's the rebuild going on.
fn rebuilding(token: &str) -> Option<DatabaseId> {
    session().as_ref().and_then(|session| {
        bool::from(session.token.as_bytes().ct_eq(token.as_bytes())).then_some(session.from)
    })
}

fn failed(what: &str, error: &dyn std::fmt::Display) -> RebuildOutcome {
    tracing::error!(target: log_area::SYNC, what, %error, "rebuild failed");
    RebuildOutcome::Unavailable
}

/// Starts rebuilding this fresh server from the copy of the server `from`, if `code` is the
/// setup code. The caller counts wrong codes against the sign-in limit, as for setup.
pub async fn begin(
    database: &Database,
    health: Health,
    clock: &(dyn Clock + Sync),
    code: &str,
    from: DatabaseId,
) -> RebuildOutcome {
    if !health.is_healthy() {
        return RebuildOutcome::Unavailable;
    }
    if !crate::setup::code_is_valid(code) {
        return RebuildOutcome::WrongCode;
    }
    match database.is_fresh().await {
        Ok(true) => {}
        Ok(false) => return RebuildOutcome::NotFresh,
        Err(error) => return failed("fresh", &error),
    }
    // A rebuild started before a restart, with nothing brought in yet: start again.
    match database.rebuild_state().await {
        Ok(None) => {}
        Ok(Some(_)) => {
            if let Err(error) = database.forget_rebuild().await {
                return failed("restart", &error);
            }
        }
        Err(error) => return failed("state", &error),
    }
    if let Err(error) = database.start_rebuild(from, clock.now()).await {
        return failed("start", &error);
    }
    let token = match crate::codes::random_code(32) {
        Ok(token) => token,
        Err(error) => return failed("token", &error),
    };
    *session() = Some(Session {
        token: token.clone(),
        from,
    });
    tracing::info!(target: log_area::SYNC, from = %from, "rebuilding this server from a desktop's copy");
    RebuildOutcome::Started {
        token: Secret::new(token),
    }
}

/// Brings in one page of the copy's records.
pub async fn page(database: &Database, token: &str, page: &PullPage) -> RebuildOutcome {
    let Some(from) = rebuilding(token) else {
        return RebuildOutcome::NotAllowed;
    };
    if page.database_id != from {
        return RebuildOutcome::NotAllowed;
    }
    match database.import_page(page).await {
        Ok(()) => RebuildOutcome::PageTaken,
        Err(StorageError::InvalidValue { column }) => {
            tracing::warn!(target: log_area::SYNC, column, "rebuild: a page doesn't fit this server");
            RebuildOutcome::Invalid
        }
        Err(error) => failed("page", &error),
    }
}

/// Finishes bringing the records in: the household's settings re-keyed, the change numbers
/// moved on, `admin`'s password set (they must be the household's Admin), and everyone else's
/// a new temporary one, returned once.
pub async fn finish(
    database: &Database,
    server_device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
    token: &str,
    admin: &str,
    password: Secret,
) -> RebuildOutcome {
    let Some(from) = rebuilding(token) else {
        return RebuildOutcome::NotAllowed;
    };
    if check_new_password(password.expose()).is_err() {
        return RebuildOutcome::Invalid;
    }
    let Ok(username) = domain::user::Username::parse(admin) else {
        return RebuildOutcome::NotAllowed;
    };
    let admin = match database.user_by_username(&username).await {
        Ok(Some(admin)) => admin,
        Ok(None) => return RebuildOutcome::NotAllowed,
        Err(error) => return failed("admin", &error),
    };
    let is_admin = match database.grants_for(admin.id).await {
        Ok(grants) => grants
            .iter()
            .any(|(_, grant)| grant.role == Role::Admin && grant.target == GrantTarget::AllAssets),
        Err(error) => return failed("grants", &error),
    };
    if !is_admin {
        return RebuildOutcome::NotAllowed;
    }
    if let Err(error) = database.finish_import(from).await {
        return failed("finish", &error);
    }
    let hash = match hash_password(password).await {
        Ok(hash) => hash,
        Err(error) => return failed("password", &error),
    };
    if let Err(error) = database
        .set_password_hash(admin.id, &hash, clock.now())
        .await
    {
        return failed("password", &error);
    }
    let passwords =
        match temporary_passwords(database, server_device, health, clock, admin.id).await {
            Ok(passwords) => passwords,
            Err(outcome) => return outcome,
        };
    if let Err(error) = database.finish_rebuild(clock.now()).await {
        return failed("finish", &error);
    }
    crate::setup::close();
    *session() = None;
    tracing::info!(target: log_area::SYNC, people = passwords.len() + 1, "rebuilt: the records are in");
    RebuildOutcome::Finished { passwords }
}

/// A new temporary password for everyone but `admin` (and the disabled, who can't sign in),
/// each to change at their first sign-in.
async fn temporary_passwords(
    database: &Database,
    server_device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
    admin: domain::user::UserId,
) -> Result<Vec<TemporaryPassword>, RebuildOutcome> {
    let users = database
        .users()
        .await
        .map_err(|error| failed("people", &error))?;
    let mut passwords = Vec::new();
    for stored in users {
        if stored.id == admin || stored.user.disabled {
            continue;
        }
        let (password, hash) = crate::user_admin::temporary_password()
            .await
            .map_err(|error| failed("password", &error))?;
        database
            .set_password_hash(stored.id, &hash, clock.now())
            .await
            .map_err(|error| failed("password", &error))?;
        let patch = UserPatch {
            must_change_password: Some(true),
            ..UserPatch::default()
        };
        issue(
            database,
            &UserWriter,
            Change::Update {
                id: stored.id,
                patch,
            },
            server_device,
            None,
            health,
            clock,
        )
        .await
        .map_err(|error| failed("password", &error))?;
        passwords.push(TemporaryPassword {
            username: stored.user.username.as_str().to_owned(),
            display_name: stored.user.display_name.clone(),
            password,
        });
    }
    Ok(passwords)
}

/// After the desktop has uploaded the files it has: leaves out the attachments whose file or
/// thumbnail never came (named in the answer), and takes out of the rest an original or
/// metadata that never came. The household's Admin only, once, right after a rebuild.
pub async fn settle(
    database: &Database,
    signed_in: &SignedIn,
    clock: &(dyn Clock + Sync),
) -> RebuildOutcome {
    let admin = signed_in
        .grants
        .iter()
        .any(|grant| grant.role == Role::Admin && grant.target == GrantTarget::AllAssets);
    let state = match database.rebuild_state().await {
        Ok(state) => state,
        Err(error) => return failed("state", &error),
    };
    if !admin || !state.is_some_and(|state| state.finished.is_some() && state.settled.is_none()) {
        return RebuildOutcome::NotAllowed;
    }
    let attachments = match database.all_attachments().await {
        Ok(attachments) => attachments,
        Err(error) => return failed("attachments", &error),
    };
    let mut left_out = Vec::new();
    for (id, mut attachment) in attachments {
        let here = async |file: &domain::attachment::StoredFile| {
            database.is_stored(&file.hash).await.unwrap_or(false)
        };
        let mut shown_here = here(&attachment.file).await;
        if let Some(thumbnail) = &attachment.thumbnail {
            shown_here = shown_here && here(thumbnail).await;
        }
        if !shown_here {
            if let Err(error) = database.leave_out_attachment(id).await {
                return failed("leave out", &error);
            }
            left_out.push(attachment.name.as_str().to_owned());
            continue;
        }
        if let Content::Photo {
            metadata, original, ..
        } = &mut attachment.content
        {
            let mut changed = false;
            if let Some(kept) = original
                && !here(&kept.file).await
            {
                *original = None;
                changed = true;
            }
            if let Some(raw) = &metadata.raw
                && !here(raw).await
            {
                metadata.raw = None;
                changed = true;
            }
            if changed
                && let Err(error) = database
                    .replace_attachment_content(id, &attachment.content)
                    .await
            {
                return failed("content", &error);
            }
        }
    }
    if let Err(error) = database.settle_rebuild(clock.now()).await {
        return failed("settle", &error);
    }
    tracing::info!(target: log_area::SYNC, left_out = left_out.len(), "rebuilt: the files are settled");
    RebuildOutcome::Settled { left_out }
}
