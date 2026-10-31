//! Signing in and out, and the signed-in user behind each request (decision #028).
//!
//! - A **session token** is 32 random bytes, sent to the client as 64 hex characters: in an
//!   `HttpOnly` cookie for the web, or as `Authorization: Bearer` from the desktop app.
//!   The database keeps only its SHA-256, so a copy of the database can't sign anyone in.
//! - A session lasts until **30 days without use**; each use extends it.
//! - Every session **starts in Driver mode**. Admin mode needs the password again and drops
//!   back to Driver after **15 minutes** without activity.
//! - A **disabled or deleted** user's sessions stop working at once, and a mode their
//!   grants no longer allow falls back to Driver.

use std::fmt;

use domain::access::{ActiveMode, RoleGrant, available_modes};
use domain::id::DeviceId;
use domain::log_area;
use domain::time::UnixMillis;
use domain::user::{UserId, Username};
use sha2::{Digest, Sha256};
use storage::users::Session;
use storage::{Database, StorageError};

use crate::ids::{NewIdError, new_id};
use crate::login_limiter::LoginLimiter;
use crate::passwords::{hash_password, needs_rehash, verify_password};

/// A session ends after this long without use: 30 days.
pub const IDLE_LIFETIME_MILLIS: i64 = 30 * 24 * 60 * 60 * 1000;

/// Admin mode ends after this long without activity: 15 minutes. Shared with the app's idle
/// watcher, which leaves Admin mode at the same point (decision #049).
pub const ADMIN_IDLE_MILLIS: i64 = domain::idle::ADMIN_IDLE_MILLIS;

/// A session's last use is written at most this often (once a minute), so ordinary requests
/// don't each cost a database write. The app's Admin-mode keep-alive uses the same interval.
const TOUCH_INTERVAL_MILLIS: i64 = domain::idle::KEEP_ALIVE_EVERY_MILLIS;

/// Token length in bytes.
const TOKEN_BYTES: usize = 32;

/// A session token, as the client holds it. Never logged: `Debug` hides it.
#[derive(Clone, PartialEq, Eq)]
pub struct SessionToken(String);

impl SessionToken {
    /// The 64-character hex form, for the cookie or `Authorization` header.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for SessionToken {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("SessionToken(…)")
    }
}

/// The SHA-256 of a presented token, or `None` if it isn't 64 hex characters.
#[must_use]
pub fn token_hash(token: &str) -> Option<[u8; 32]> {
    let bytes = decode_hex(token)?;
    Some(Sha256::digest(bytes).into())
}

fn decode_hex(text: &str) -> Option<[u8; TOKEN_BYTES]> {
    if text.len() != TOKEN_BYTES * 2 {
        return None;
    }
    let mut bytes = [0; TOKEN_BYTES];
    for (byte, pair) in bytes.iter_mut().zip(text.as_bytes().chunks_exact(2)) {
        let pair = core::str::from_utf8(pair).ok()?;
        *byte = u8::from_str_radix(pair, 16).ok()?;
    }
    Some(bytes)
}

/// A new random token and its hash.
fn new_token() -> Result<(SessionToken, [u8; 32]), getrandom::Error> {
    let mut bytes = [0; TOKEN_BYTES];
    getrandom::fill(&mut bytes)?;
    // Two lowercase hex digits per byte.
    let text = bytes
        .iter()
        .fold(String::with_capacity(TOKEN_BYTES * 2), |mut text, byte| {
            use core::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        });
    Ok((SessionToken(text), Sha256::digest(bytes).into()))
}

/// Who is behind a request, attached by the auth layer for server functions to read.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignedIn {
    /// The session's token hash (to sign out or switch mode).
    pub token_hash: [u8; 32],
    /// The user.
    pub user: UserId,
    /// Their username.
    pub username: Username,
    /// Their display name.
    pub display_name: String,
    /// They must choose a new password before doing anything else.
    pub must_change_password: bool,
    /// Their current grants.
    pub grants: Vec<RoleGrant>,
    /// The mode the session is in.
    pub mode: ActiveMode,
    /// The device this session acts as (for command stamps).
    pub device: DeviceId,
}

impl SignedIn {
    /// The modes this user may switch to.
    #[must_use]
    pub fn available_modes(&self) -> Vec<ActiveMode> {
        available_modes(&self.grants)
    }
}

/// Why signing in failed. Wrong username and wrong password are deliberately the same, so a
/// caller can't learn which usernames exist.
#[derive(Debug)]
pub enum SignInError {
    /// Unknown username, wrong password, or a disabled user.
    InvalidCredentials,
    /// Too many failures; try again after this many milliseconds.
    RateLimited {
        /// How long to wait.
        retry_after_millis: i64,
    },
    /// The server couldn't check (database or randomness failed); see the log.
    Unavailable,
}

impl From<StorageError> for SignInError {
    fn from(error: StorageError) -> Self {
        tracing::error!(target: log_area::AUTH, %error, "sign-in failed: database error");
        Self::Unavailable
    }
}

impl From<NewIdError> for SignInError {
    fn from(error: NewIdError) -> Self {
        tracing::error!(target: log_area::AUTH, %error, "sign-in failed: can't make a device ID");
        Self::Unavailable
    }
}

/// A real hash of a throwaway password, checked when the username doesn't exist, so a wrong
/// username takes as long as a wrong password. Made once, at the first such attempt.
static DUMMY_HASH: tokio::sync::OnceCell<String> = tokio::sync::OnceCell::const_new();

async fn dummy_hash() -> String {
    DUMMY_HASH
        .get_or_init(|| async {
            hash_password("never a real password".to_owned())
                .await
                .unwrap_or_default()
        })
        .await
        .clone()
}

/// Signs a user in. `client` identifies where the request came from, for the rate limit.
///
/// # Errors
///
/// [`SignInError`]; every failure except [`SignInError::Unavailable`] counts toward the limit.
pub async fn sign_in(
    database: &Database,
    limiter: &LoginLimiter,
    username: &str,
    password: String,
    client: &str,
    now: UnixMillis,
) -> Result<(SessionToken, SignedIn), SignInError> {
    let user_key = format!("user:{}", username.trim().to_ascii_lowercase());
    let client_key = format!("client:{client}");
    let keys = [user_key.as_str(), client_key.as_str()];
    limiter
        .check(&keys, now)
        .map_err(|retry_after_millis| SignInError::RateLimited { retry_after_millis })?;

    let Some(user) = find_signable_user(database, username).await? else {
        // Same work as a real check, so timing doesn't reveal that the user doesn't exist.
        let _ = verify_password(password, dummy_hash().await).await;
        limiter.record_failure(&keys, now);
        tracing::debug!(target: log_area::AUTH, "sign-in refused: unknown or disabled user");
        return Err(SignInError::InvalidCredentials);
    };
    let stored_hash = database.password_hash(user.id).await?;
    let matches = match stored_hash.clone() {
        Some(hash) => verify_password(password.clone(), hash).await,
        None => false,
    };
    if !matches {
        limiter.record_failure(&keys, now);
        tracing::debug!(target: log_area::AUTH, user = %user.id, "sign-in refused: wrong password");
        return Err(SignInError::InvalidCredentials);
    }
    limiter.clear(&user_key);
    if stored_hash.as_deref().is_some_and(needs_rehash) {
        upgrade_hash(database, user.id, password, now).await;
    }
    start_session(database, user, now).await
}

async fn find_signable_user(
    database: &Database,
    username: &str,
) -> Result<Option<storage::users::StoredUser>, StorageError> {
    let Ok(username) = Username::parse(username) else {
        return Ok(None);
    };
    Ok(database
        .user_by_username(&username)
        .await?
        .filter(|stored| !stored.user.disabled))
}

/// Replaces an old-parameter hash after a successful sign-in; a failure only means it waits
/// for the next sign-in.
async fn upgrade_hash(database: &Database, user: UserId, password: String, now: UnixMillis) {
    match hash_password(password).await {
        Ok(hash) => {
            if let Err(error) = database.set_password_hash(user, &hash, now).await {
                tracing::warn!(target: log_area::AUTH, %error, "couldn't store the upgraded password hash");
            }
        }
        Err(error) => {
            tracing::warn!(target: log_area::AUTH, %error, "couldn't upgrade the password hash");
        }
    }
}

async fn start_session(
    database: &Database,
    user: storage::users::StoredUser,
    now: UnixMillis,
) -> Result<(SessionToken, SignedIn), SignInError> {
    let (token, token_hash) = new_token().map_err(|error| {
        tracing::error!(target: log_area::AUTH, %error, "sign-in failed: no random bytes");
        SignInError::Unavailable
    })?;
    let device = new_id(now)?;
    database
        .insert_session(&Session {
            token_hash,
            user: user.id,
            device,
            created_at: now,
            last_used: now,
            expires_at: UnixMillis::from_millis(now.as_millis() + IDLE_LIFETIME_MILLIS),
            active_mode: ActiveMode::Driver,
        })
        .await?;
    let grants = grants_of(database, user.id).await?;
    tracing::info!(target: log_area::AUTH, user = %user.id, %device, "signed in");
    Ok((
        token,
        SignedIn {
            token_hash,
            user: user.id,
            username: user.user.username,
            display_name: user.user.display_name,
            must_change_password: user.user.must_change_password,
            grants,
            mode: ActiveMode::Driver,
            device,
        },
    ))
}

async fn grants_of(database: &Database, user: UserId) -> Result<Vec<RoleGrant>, StorageError> {
    Ok(database
        .grants_for(user)
        .await?
        .into_iter()
        .map(|(_, grant)| grant)
        .collect())
}

/// The signed-in user for a presented token hash, or `None` if the session doesn't exist,
/// has expired, or its user is disabled or gone. Extends the session, and drops Admin mode
/// after 15 idle minutes or a mode the user's grants no longer allow.
///
/// # Errors
///
/// [`StorageError`] if the database fails.
pub async fn resolve(
    database: &Database,
    token_hash: [u8; 32],
    now: UnixMillis,
) -> Result<Option<SignedIn>, StorageError> {
    let Some(session) = database.session(token_hash).await? else {
        return Ok(None);
    };
    if session.expires_at <= now {
        database.delete_session(token_hash).await?;
        return Ok(None);
    }
    let Some(user) = database
        .user(session.user)
        .await?
        .filter(|user| !user.user.disabled)
    else {
        database.delete_sessions_of(session.user).await?;
        return Ok(None);
    };
    let grants = grants_of(database, user.id).await?;
    let mode = effective_mode(&session, &grants, now);
    if mode != session.active_mode {
        database.set_session_mode(token_hash, mode).await?;
    }
    if now.as_millis() - session.last_used.as_millis() >= TOUCH_INTERVAL_MILLIS {
        let expires_at = UnixMillis::from_millis(now.as_millis() + IDLE_LIFETIME_MILLIS);
        database.touch_session(token_hash, now, expires_at).await?;
    }
    Ok(Some(SignedIn {
        token_hash,
        user: user.id,
        username: user.user.username,
        display_name: user.user.display_name,
        must_change_password: user.user.must_change_password,
        grants,
        mode,
        device: session.device,
    }))
}

/// The session's mode now: Driver if Admin mode has been idle too long, or if the user's
/// grants no longer allow the stored mode.
fn effective_mode(session: &Session, grants: &[RoleGrant], now: UnixMillis) -> ActiveMode {
    let admin_idle = session.active_mode == ActiveMode::Admin
        && now.as_millis() - session.last_used.as_millis() >= ADMIN_IDLE_MILLIS;
    let still_allowed = available_modes(grants).contains(&session.active_mode);
    if admin_idle || !still_allowed {
        ActiveMode::Driver
    } else {
        session.active_mode
    }
}

/// Why switching mode was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SwitchModeError {
    /// The user's grants don't allow that mode.
    NotAllowed,
    /// Admin mode needs the password, and it was wrong or missing.
    WrongPassword,
    /// The database failed.
    Unavailable,
}

/// Switches the session to `mode` at `now`. Admin mode needs `password`, checked again.
/// Switching counts as activity, so Admin mode's 15 idle minutes start now.
///
/// # Errors
///
/// [`SwitchModeError`] if the mode isn't allowed or the password is wrong.
pub async fn switch_mode(
    database: &Database,
    signed_in: &SignedIn,
    mode: ActiveMode,
    password: Option<String>,
    now: UnixMillis,
) -> Result<ActiveMode, SwitchModeError> {
    if !signed_in.available_modes().contains(&mode) {
        return Err(SwitchModeError::NotAllowed);
    }
    if mode == ActiveMode::Admin {
        let hash = database
            .password_hash(signed_in.user)
            .await
            .map_err(|_| SwitchModeError::Unavailable)?;
        let matches = match (password, hash) {
            (Some(password), Some(hash)) => verify_password(password, hash).await,
            _ => false,
        };
        if !matches {
            tracing::debug!(target: log_area::AUTH, user = %signed_in.user, "Admin mode refused: wrong password");
            return Err(SwitchModeError::WrongPassword);
        }
    }
    database
        .set_session_mode(signed_in.token_hash, mode)
        .await
        .map_err(|_| SwitchModeError::Unavailable)?;
    // Without this, a session idle for a while before entering Admin mode would look
    // idle in Admin mode too, and drop straight back to Driver.
    let expires_at = UnixMillis::from_millis(now.as_millis() + IDLE_LIFETIME_MILLIS);
    database
        .touch_session(signed_in.token_hash, now, expires_at)
        .await
        .map_err(|_| SwitchModeError::Unavailable)?;
    tracing::info!(target: log_area::AUTH, user = %signed_in.user, ?mode, "mode switched");
    Ok(mode)
}

/// Ends the session (sign out).
///
/// # Errors
///
/// [`StorageError`] if the database fails.
pub async fn sign_out(database: &Database, token_hash: [u8; 32]) -> Result<(), StorageError> {
    database.delete_session(token_hash).await?;
    tracing::info!(target: log_area::AUTH, "signed out");
    Ok(())
}

/// Why a password change was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ChangePasswordError {
    /// The current password is wrong.
    WrongPassword,
    /// The new password breaks a rule.
    Invalid(domain::user::PasswordError),
    /// The database or hashing failed.
    Unavailable,
}

/// Changes the signed-in user's password, after checking their current one. Clears "must
/// change password" (through a command, so it's in the history) and signs out their other
/// sessions; this one stays signed in. `health` is the server's current health.
///
/// # Errors
///
/// [`ChangePasswordError`].
pub async fn change_password(
    database: &Database,
    signed_in: &SignedIn,
    current: String,
    new: String,
    health: crate::health::Health,
    clock: &(dyn domain::time::Clock + Sync),
) -> Result<(), ChangePasswordError> {
    domain::user::check_new_password(&new).map_err(ChangePasswordError::Invalid)?;
    let stored = database
        .password_hash(signed_in.user)
        .await
        .map_err(|_| ChangePasswordError::Unavailable)?;
    let matches = match stored {
        Some(hash) => verify_password(current, hash).await,
        None => false,
    };
    if !matches {
        return Err(ChangePasswordError::WrongPassword);
    }
    let hash = hash_password(new)
        .await
        .map_err(|_| ChangePasswordError::Unavailable)?;
    database
        .set_password_hash(signed_in.user, &hash, clock.now())
        .await
        .map_err(|_| ChangePasswordError::Unavailable)?;
    if signed_in.must_change_password {
        clear_must_change(database, signed_in, health, clock).await?;
    }
    let ended = database
        .delete_other_sessions_of(signed_in.user, signed_in.token_hash)
        .await
        .map_err(|_| ChangePasswordError::Unavailable)?;
    tracing::info!(target: log_area::AUTH, user = %signed_in.user, other_sessions_ended = ended, "password changed");
    Ok(())
}

async fn clear_must_change(
    database: &Database,
    signed_in: &SignedIn,
    health: crate::health::Health,
    clock: &(dyn domain::time::Clock + Sync),
) -> Result<(), ChangePasswordError> {
    let change = domain::command::Change::Update {
        id: signed_in.user,
        patch: domain::user::UserPatch {
            must_change_password: Some(false),
            ..domain::user::UserPatch::default()
        },
    };
    let actor = domain::access::Actor {
        user: signed_in.user,
        mode: signed_in.mode,
    };
    crate::commands::issue(
        database,
        &storage::users::UserWriter,
        change,
        signed_in.device,
        Some(actor),
        health,
        clock,
    )
    .await
    .map(|_| ())
    .map_err(|error| {
        tracing::error!(target: log_area::AUTH, %error, "couldn't clear must-change-password");
        ChangePasswordError::Unavailable
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tokens_are_64_hex_characters_and_hash_consistently() {
        let (token, hash) = new_token().unwrap();
        assert_eq!(token.as_str().len(), 64);
        assert_eq!(token_hash(token.as_str()), Some(hash));
        assert_ne!(new_token().unwrap().0, token, "random");
    }

    #[test]
    fn malformed_tokens_have_no_hash() {
        assert_eq!(token_hash("too-short"), None);
        assert_eq!(token_hash(&"z".repeat(64)), None);
        assert_eq!(token_hash(&"é".repeat(32)), None);
    }

    #[test]
    fn tokens_never_show_in_debug_output() {
        let (token, _) = new_token().unwrap();
        assert!(!format!("{token:?}").contains(token.as_str()));
    }
}
