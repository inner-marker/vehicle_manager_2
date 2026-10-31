//! Password recovery for Admins: a one-time code in the server log (decision #047).
//!
//! 1. Anyone asks, with a username ([`request_code`]). The answer is always the same, so it
//!    reveals nothing. **Only for an enabled household Admin** does the server make a code
//!    and write it to its log, where only someone with NAS access can read it.
//! 2. The code, with a new password, completes it ([`complete`]): the password is set, every
//!    old session ends, and they're signed in.
//!
//! A code works **once** and for **15 minutes**; asking again replaces it. Codes are kept in
//! memory only. Requests and wrong codes count toward the sign-in attempt limit.

use std::collections::HashMap;
use std::sync::Mutex;

use domain::access::is_household_admin;
use domain::log_area;
use domain::time::{Clock, UnixMillis};
use domain::user::{PasswordError, UserId, Username, check_new_password};
use storage::Database;
use subtle::ConstantTimeEq;

use crate::codes::{group_by_four, random_code};
use crate::login_limiter::LoginLimiter;
use crate::passwords::hash_password;
use crate::sessions::{SessionToken, SignedIn, sign_in};

/// How long a code works: 15 minutes.
pub const CODE_LIFETIME_MILLIS: i64 = 15 * 60 * 1000;

/// Characters in a recovery code (before the dashes), as for the setup code.
const CODE_CHARS: usize = 16;

/// Open codes, by user: the normalized code and when it stops working.
static CODES: Mutex<Option<HashMap<UserId, (String, UnixMillis)>>> = Mutex::new(None);

fn with_codes<T>(action: impl FnOnce(&mut HashMap<UserId, (String, UnixMillis)>) -> T) -> T {
    let mut guard = CODES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    action(guard.get_or_insert_with(HashMap::new))
}

/// The limiter keys for a recovery attempt: the username tried, and the client.
fn keys(username: &str, client: &str) -> [String; 2] {
    [
        format!("recover:{}", username.trim().to_ascii_lowercase()),
        format!("client:{client}"),
    ]
}

/// Asks for a recovery code for `username`. Always "succeeds" from the caller's point of view;
/// a code is made and logged only for an enabled household Admin.
///
/// # Errors
///
/// The wait in milliseconds if the attempt limit is reached (the caller still shows the same
/// answer).
pub async fn request_code(
    database: &Database,
    limiter: &LoginLimiter,
    username: &str,
    client: &str,
    now: UnixMillis,
) -> Result<(), i64> {
    let keys = keys(username, client);
    let key_refs = [keys[0].as_str(), keys[1].as_str()];
    limiter.check(&key_refs, now)?;
    // Every request counts, so nobody can fill the log with codes.
    limiter.record_failure(&key_refs, now);

    let Some(admin) = recoverable_admin(database, username).await else {
        tracing::debug!(target: log_area::AUTH, "recovery asked for a non-Admin or unknown user; no code made");
        return Ok(());
    };
    let code = match random_code(CODE_CHARS) {
        Ok(code) => code,
        Err(error) => {
            tracing::error!(target: log_area::AUTH, %error, "can't make a recovery code");
            return Ok(());
        }
    };
    let shown = group_by_four(&code);
    let expires = UnixMillis::from_millis(now.as_millis() + CODE_LIFETIME_MILLIS);
    with_codes(|codes| codes.insert(admin, (code, expires)));
    // Deliberately logged: reading the server log (NAS access) is what proves you may recover.
    tracing::warn!(target: log_area::AUTH, user = %admin, recovery_code = %shown, "password recovery asked for: enter this recovery code in the app within 15 minutes");
    Ok(())
}

/// The user, if `username` is an enabled household Admin.
async fn recoverable_admin(database: &Database, username: &str) -> Option<UserId> {
    let username = Username::parse(username).ok()?;
    let stored = database.user_by_username(&username).await.ok()??;
    if stored.user.disabled {
        return None;
    }
    let grants: Vec<_> = database
        .grants_for(stored.id)
        .await
        .ok()?
        .into_iter()
        .map(|(_, grant)| grant)
        .collect();
    is_household_admin(&grants).then_some(stored.id)
}

/// Why recovery couldn't be completed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RecoveryError {
    /// Wrong, used, or expired code (or not an Admin): deliberately one answer.
    WrongCode,
    /// Too many attempts; wait this many milliseconds.
    RateLimited(i64),
    /// The new password breaks a rule.
    InvalidPassword(PasswordError),
    /// The database or hashing failed.
    Unavailable,
}

/// Completes recovery: if `code` is `username`'s open code, sets `new_password`, ends every
/// session of theirs, and signs them in. The code is used up either way once it matches.
///
/// # Errors
///
/// [`RecoveryError`].
pub async fn complete(
    database: &Database,
    limiter: &LoginLimiter,
    username: &str,
    code: &str,
    new_password: String,
    client: &str,
    clock: &(dyn Clock + Sync),
) -> Result<(SessionToken, SignedIn), RecoveryError> {
    let now = clock.now();
    let keys = keys(username, client);
    let key_refs = [keys[0].as_str(), keys[1].as_str()];
    limiter
        .check(&key_refs, now)
        .map_err(RecoveryError::RateLimited)?;
    check_new_password(&new_password).map_err(RecoveryError::InvalidPassword)?;

    let Some(user) = recoverable_admin(database, username).await else {
        limiter.record_failure(&key_refs, now);
        return Err(RecoveryError::WrongCode);
    };
    if !take_code(user, code, now) {
        limiter.record_failure(&key_refs, now);
        tracing::debug!(target: log_area::AUTH, %user, "recovery refused: wrong or expired code");
        return Err(RecoveryError::WrongCode);
    }

    let hash = hash_password(new_password.clone())
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    database
        .set_password_hash(user, &hash, now)
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    let ended = database
        .delete_sessions_of(user)
        .await
        .map_err(|_| RecoveryError::Unavailable)?;
    tracing::info!(target: log_area::AUTH, %user, sessions_ended = ended, "password recovered");
    sign_in(database, limiter, username, new_password, client, now)
        .await
        .map_err(|_| RecoveryError::Unavailable)
}

/// Whether `entered` is `user`'s open, unexpired code; a match uses it up.
fn take_code(user: UserId, entered: &str, now: UnixMillis) -> bool {
    let entered: String = entered
        .chars()
        .filter(|character| *character != '-' && !character.is_whitespace())
        .map(|character| character.to_ascii_uppercase())
        .collect();
    with_codes(|codes| {
        let matches = codes.get(&user).is_some_and(|(code, expires)| {
            now < *expires && bool::from(code.as_bytes().ct_eq(entered.as_bytes()))
        });
        if matches {
            codes.remove(&user);
        }
        matches
    })
}

#[cfg(test)]
mod tests {
    use domain::access::{GrantTarget, Role, RoleGrant, RoleGrantPatch};
    use domain::command::Change;
    use domain::test_support::{ManualClock, test_id};
    use domain::user::{User, UserPatch};
    use storage::NewDatabaseIds;
    use storage::users::{RoleGrantWriter, UserWriter};

    use super::*;
    use crate::commands::issue;
    use crate::health::Health;
    use crate::sessions::{resolve, token_hash};

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    /// A database with Admin `stuart` (user 1) and Driver `sam` (user 2).
    async fn database() -> (tempfile::TempDir, Database) {
        let temp = tempfile::tempdir().unwrap();
        let ids = NewDatabaseIds {
            database_id: test_id(900),
            server_device_id: test_id(901),
        };
        let (database, _) = Database::open(temp.path(), NOW, ids).await.unwrap();
        for (n, name, role) in [(1, "stuart", Role::Admin), (2, "sam", Role::Driver)] {
            let user = User {
                username: Username::parse(name).unwrap(),
                display_name: name.into(),
                disabled: false,
                must_change_password: false,
            };
            let grant = RoleGrant {
                user: test_id(n),
                target: GrantTarget::AllAssets,
                role,
            };
            let clock = ManualClock::new(NOW);
            issue(
                &database,
                &UserWriter,
                Change::Create {
                    id: test_id(n),
                    values: UserPatch::all(user),
                },
                test_id(901),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap();
            issue(
                &database,
                &RoleGrantWriter,
                Change::Create {
                    id: test_id(n + 100),
                    values: RoleGrantPatch::all(grant),
                },
                test_id(901),
                None,
                Health::Healthy,
                &clock,
            )
            .await
            .unwrap();
            database
                .set_password_hash(
                    test_id(n),
                    &hash_password("old password here".into()).await.unwrap(),
                    NOW,
                )
                .await
                .unwrap();
        }
        (temp, database)
    }

    fn code_for(user: u32) -> Option<String> {
        with_codes(|codes| codes.get(&test_id(user)).map(|(code, _)| code.clone()))
    }

    /// A Driver and an unknown name get the same answer, and no code; the Admin gets one.
    async fn request_codes(database: &Database, limiter: &LoginLimiter) -> String {
        assert_eq!(
            request_code(database, limiter, "sam", "a", NOW).await,
            Ok(())
        );
        assert_eq!(
            request_code(database, limiter, "nobody", "b", NOW).await,
            Ok(())
        );
        assert_eq!(code_for(2), None);
        assert_eq!(
            request_code(database, limiter, "Stuart", "c", NOW).await,
            Ok(())
        );
        let code = code_for(1).unwrap();
        assert_eq!(code.len(), 16);
        code
    }

    /// A wrong code and a short password are refused.
    async fn assert_refusals(
        database: &Database,
        limiter: &LoginLimiter,
        clock: &ManualClock,
        code: &str,
    ) {
        let wrong = complete(
            database,
            limiter,
            "stuart",
            "AAAA-AAAA-AAAA-AAAA",
            "a new password!".into(),
            "d",
            clock,
        )
        .await;
        assert_eq!(wrong.err(), Some(RecoveryError::WrongCode));
        let short = complete(
            database,
            limiter,
            "stuart",
            code,
            "short".into(),
            "e",
            clock,
        )
        .await;
        assert_eq!(
            short.err(),
            Some(RecoveryError::InvalidPassword(PasswordError::TooShort))
        );
    }

    // One test, in order: the codes live in one process-wide table.
    #[tokio::test]
    async fn only_an_admin_gets_a_code_and_it_works_once_within_15_minutes() {
        let (_temp, database) = database().await;
        let limiter = LoginLimiter::new();
        let clock = ManualClock::new(NOW);
        let code = request_codes(&database, &limiter).await;
        assert_refusals(&database, &limiter, &clock, &code).await;

        // The right code, typed loosely: signed in, and an older session ends.
        let (old, _) = sign_in(
            &database,
            &limiter,
            "stuart",
            "old password here".into(),
            "f",
            NOW,
        )
        .await
        .unwrap();
        let loose = group_by_four(&code).to_lowercase();
        let (_, signed_in) = complete(
            &database,
            &limiter,
            "stuart",
            &loose,
            "a new password!".into(),
            "g",
            &clock,
        )
        .await
        .unwrap();
        assert_eq!(signed_in.user, test_id(1));
        assert!(
            resolve(&database, token_hash(old.as_str()).unwrap(), NOW)
                .await
                .unwrap()
                .is_none()
        );

        // Used up: the same code fails now.
        let again = complete(
            &database,
            &limiter,
            "stuart",
            &code,
            "another new one!".into(),
            "h",
            &clock,
        )
        .await;
        assert_eq!(again.err(), Some(RecoveryError::WrongCode));

        // A fresh code expires after 15 minutes.
        request_code(&database, &limiter, "stuart", "i", NOW)
            .await
            .unwrap();
        let fresh = code_for(1).unwrap();
        let late = ManualClock::new(UnixMillis::from_millis(
            NOW.as_millis() + CODE_LIFETIME_MILLIS,
        ));
        let expired = complete(
            &database,
            &limiter,
            "stuart",
            &fresh,
            "too late for this".into(),
            "j",
            &late,
        )
        .await;
        assert_eq!(expired.err(), Some(RecoveryError::WrongCode));
    }
}
