//! First-Admin setup with a one-time code (decision #028).
//!
//! While the server has no users, each start makes a new **setup code** and writes it to the
//! log, where only someone with access to the NAS can read it. The first person to enter it,
//! with a username and password, becomes the household's Admin (Admin on all assets). After
//! that the code stops working, and no new one is made.
//!
//! The code is 16 characters from an alphabet without look-alikes (no `0`/`O`, `1`/`I`),
//! shown in groups of 4: `K7QM-4XRT-9PWD-H2CN`. It's kept in memory only, so it never
//! reaches the database or a snapshot. Entry ignores case, dashes, and spaces.

use std::sync::Mutex;

use domain::access::{GrantTarget, Role, RoleGrant, RoleGrantPatch};
use domain::command::Change;
use domain::id::DeviceId;
use domain::log_area;
use domain::time::Clock;
use domain::user::{
    DisplayNameError, PasswordError, User, UserId, UserPatch, Username, UsernameError,
    check_display_name, check_new_password,
};
use storage::Database;
use storage::users::{RoleGrantWriter, UserWriter};
use subtle::ConstantTimeEq;

use crate::commands::{ApplyError, issue};
use crate::health::Health;
use crate::ids::new_id;
use crate::passwords::hash_password;

/// The code's characters: 32 of them, so each random byte maps to one without bias.
const ALPHABET: &[u8; 32] = b"23456789ABCDEFGHJKLMNPQRSTUVWXYZ";

/// Characters in a code.
const CODE_CHARS: usize = 16;

/// The current code (normalized: uppercase, no dashes), while setup is open.
static CODE: Mutex<Option<String>> = Mutex::new(None);

/// One setup at a time, so two people entering the code at once can't both become Admin.
static SETUP_LOCK: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What the person setting up the server entered.
#[derive(Clone)]
pub struct SetupRequest {
    /// The setup code from the server log.
    pub code: String,
    /// The Admin's username.
    pub username: String,
    /// The Admin's display name.
    pub display_name: String,
    /// The Admin's password.
    pub password: String,
}

// Never prints the code or password.
impl core::fmt::Debug for SetupRequest {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("SetupRequest")
            .field("username", &self.username)
            .finish_non_exhaustive()
    }
}

/// Why setup failed.
#[derive(Debug)]
pub enum SetupError {
    /// A user already exists.
    AlreadySetUp,
    /// The code is wrong (or setup isn't open).
    WrongCode,
    /// The username breaks a rule.
    InvalidUsername(UsernameError),
    /// The display name breaks a rule.
    InvalidDisplayName(DisplayNameError),
    /// The password breaks a rule.
    InvalidPassword(PasswordError),
    /// The database, randomness, or hashing failed; see the log.
    Unavailable,
}

/// Opens setup if there are no users: makes a new code, logs it, and returns it formatted.
/// Returns `None` (and closes setup) if users exist.
pub fn open_if_needed(user_count: i64) -> Option<String> {
    if user_count > 0 {
        set_code(None);
        return None;
    }
    let code = match new_code() {
        Ok(code) => code,
        Err(error) => {
            tracing::error!(target: log_area::AUTH, %error, "can't make a setup code");
            return None;
        }
    };
    let shown = format_code(&code);
    set_code(Some(code));
    // Deliberately logged: reading the server log (NAS access) is what proves you may set it up.
    tracing::warn!(target: log_area::AUTH, setup_code = %shown, "no users yet: create the first Admin in the app with this setup code");
    Some(shown)
}

/// Whether setup is open (no Admin yet).
#[must_use]
pub fn is_open() -> bool {
    lock_code().is_some()
}

/// Creates the first Admin, if `code` is right and no user exists yet, and returns their ID.
/// The server makes the commands itself (`device` is its own device ID); `health` is the
/// server's current health (an unhealthy server writes nothing).
///
/// # Errors
///
/// [`SetupError`]; the code stays valid after a mistake in the username or password.
pub async fn create_first_admin(
    database: &Database,
    device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
    request: SetupRequest,
) -> Result<UserId, SetupError> {
    let SetupRequest {
        code,
        username,
        display_name,
        password,
    } = request;
    let _one_at_a_time = SETUP_LOCK.lock().await;
    if database
        .user_count()
        .await
        .map_err(|_| SetupError::Unavailable)?
        > 0
    {
        return Err(SetupError::AlreadySetUp);
    }
    if !code_matches(&code) {
        tracing::debug!(target: log_area::AUTH, "setup refused: wrong code");
        return Err(SetupError::WrongCode);
    }
    let username = Username::parse(&username).map_err(SetupError::InvalidUsername)?;
    let display_name = check_display_name(&display_name).map_err(SetupError::InvalidDisplayName)?;
    check_new_password(&password).map_err(SetupError::InvalidPassword)?;
    let hash = hash_password(password)
        .await
        .map_err(|_| SetupError::Unavailable)?;

    let user_id: UserId = new_id(clock.now()).map_err(|_| SetupError::Unavailable)?;
    let user = User {
        username,
        display_name,
        disabled: false,
        must_change_password: false,
    };
    let grant = RoleGrant {
        user: user_id,
        target: GrantTarget::AllAssets,
        role: Role::Admin,
    };
    let grant_id = new_id(clock.now()).map_err(|_| SetupError::Unavailable)?;
    let create_user = Change::Create {
        id: user_id,
        values: UserPatch::all(user),
    };
    let create_grant = Change::Create {
        id: grant_id,
        values: RoleGrantPatch::all(grant),
    };
    issue(
        database,
        &UserWriter,
        create_user,
        device,
        None,
        health,
        clock,
    )
    .await
    .map_err(|error| unavailable(&error))?;
    issue(
        database,
        &RoleGrantWriter,
        create_grant,
        device,
        None,
        health,
        clock,
    )
    .await
    .map_err(|error| unavailable(&error))?;
    database
        .set_password_hash(user_id, &hash, clock.now())
        .await
        .map_err(|_| SetupError::Unavailable)?;

    set_code(None);
    tracing::info!(target: log_area::AUTH, user = %user_id, "first Admin created; setup closed");
    Ok(user_id)
}

fn unavailable(error: &ApplyError) -> SetupError {
    tracing::error!(target: log_area::AUTH, %error, "setup failed");
    SetupError::Unavailable
}

/// Whether `entered` matches the open code: case, dashes, and spaces ignored; constant time.
fn code_matches(entered: &str) -> bool {
    let entered = normalize(entered);
    lock_code()
        .as_deref()
        .is_some_and(|code| bool::from(code.as_bytes().ct_eq(entered.as_bytes())))
}

/// Uppercase, without dashes or whitespace.
fn normalize(code: &str) -> String {
    code.chars()
        .filter(|character| *character != '-' && !character.is_whitespace())
        .map(|character| character.to_ascii_uppercase())
        .collect()
}

/// A new random code (normalized form).
fn new_code() -> Result<String, getrandom::Error> {
    let mut bytes = [0_u8; CODE_CHARS];
    getrandom::fill(&mut bytes)?;
    // 256 is a multiple of 32, so `byte % 32` picks every character equally often.
    Ok(bytes
        .iter()
        .map(|byte| char::from(ALPHABET[usize::from(*byte) % ALPHABET.len()]))
        .collect())
}

/// `K7QM4XRT9PWDH2CN` → `K7QM-4XRT-9PWD-H2CN`.
fn format_code(code: &str) -> String {
    code.as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join("-")
}

fn set_code(code: Option<String>) {
    *CODE
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = code;
}

fn lock_code() -> std::sync::MutexGuard<'static, Option<String>> {
    CODE.lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codes_use_only_unambiguous_characters_and_are_random() {
        let code = new_code().unwrap();
        assert_eq!(code.len(), CODE_CHARS);
        assert!(code.bytes().all(|byte| ALPHABET.contains(&byte)), "{code}");
        assert!(!code.contains(['0', 'O', '1', 'I']));
        assert_ne!(new_code().unwrap(), code);
    }

    #[test]
    fn codes_are_shown_in_groups_of_four_and_entered_loosely() {
        assert_eq!(format_code("K7QM4XRT9PWDH2CN"), "K7QM-4XRT-9PWD-H2CN");
        assert_eq!(normalize(" k7qm-4xrt 9pwd-h2cn "), "K7QM4XRT9PWDH2CN");
    }
}
