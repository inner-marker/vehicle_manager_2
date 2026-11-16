//! Password hashing with argon2id (decision #028).
//!
//! Hashes are PHC strings (`$argon2id$v=19$m=…,t=…,p=…$salt$hash`) with a random salt, using
//! the `argon2` crate's recommended parameters. A hash made with older parameters still
//! verifies, and [`needs_rehash`] tells sign-in to replace it with a current one.
//!
//! Hashing is deliberately slow (tens of milliseconds), so callers run it on a blocking
//! thread ([`hash_password`] and [`verify_password`] do this themselves).
//!
//! **At most [`HASHES_AT_ONCE`] run at once** (decision #071), so a flood of sign-ins can't
//! take every core. Each waits up to [`HASH_WAIT`] for its turn, then gives up as [`Busy`].

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params};
use domain::secret::Secret;
use std::sync::{Arc, LazyLock};
use std::time::Duration;
use tokio::sync::{OwnedSemaphorePermit, Semaphore};

/// How many passwords may be hashed or checked at once (the NAS has 4 cores).
pub const HASHES_AT_ONCE: usize = 2;

/// How long a hash waits for its turn before giving up.
pub const HASH_WAIT: Duration = Duration::from_secs(10);

/// Too many passwords were being hashed; try again shortly.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Busy;

impl core::fmt::Display for Busy {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("too many passwords being checked at once")
    }
}

impl std::error::Error for Busy {}

/// Turns for hashing: a limited number at once, each waited for up to a limit.
#[derive(Debug, Clone)]
struct Turns {
    permits: Arc<Semaphore>,
    wait: Duration,
}

impl Turns {
    fn new(at_once: usize, wait: Duration) -> Self {
        Self {
            permits: Arc::new(Semaphore::new(at_once)),
            wait,
        }
    }

    /// Waits for a turn, which lasts until the permit is dropped.
    async fn take(&self) -> Result<OwnedSemaphorePermit, Busy> {
        if let Ok(Ok(permit)) =
            tokio::time::timeout(self.wait, Arc::clone(&self.permits).acquire_owned()).await
        {
            Ok(permit)
        } else {
            // Timed out (the semaphore is never closed).
            tracing::warn!(target: domain::log_area::AUTH, "password check refused: too many at once");
            Err(Busy)
        }
    }

    /// Runs `work` on a blocking thread in its turn. The turn lasts until `work` finishes,
    /// even if the caller stops waiting.
    async fn run<T: Send + 'static>(
        &self,
        work: impl FnOnce() -> T + Send + 'static,
    ) -> Result<Result<T, tokio::task::JoinError>, Busy> {
        let permit = self.take().await?;
        Ok(tokio::task::spawn_blocking(move || {
            let result = work();
            drop(permit);
            result
        })
        .await)
    }
}

/// The server's turns for hashing.
static TURNS: LazyLock<Turns> = LazyLock::new(|| Turns::new(HASHES_AT_ONCE, HASH_WAIT));

/// Why a password couldn't be hashed.
#[derive(Debug)]
pub struct HashError(String);

impl core::fmt::Display for HashError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "can't hash the password: {}", self.0)
    }
}

impl std::error::Error for HashError {}

/// Hashes `password` with a new random salt.
///
/// # Errors
///
/// [`HashError`] if the OS has no random bytes for the salt, the blocking task fails, or
/// too many are being hashed ([`Busy`]).
pub async fn hash_password(password: Secret) -> Result<String, HashError> {
    TURNS
        .run(move || {
            let password = password.into_exposed();
            Argon2::default()
                .hash_password(password.as_bytes())
                .map(|hash| hash.to_string())
                .map_err(|error| HashError(error.to_string()))
        })
        .await
        .map_err(|busy| HashError(busy.to_string()))?
        .map_err(|error| HashError(error.to_string()))?
}

/// Whether `password` matches `hash`. A malformed hash never matches.
///
/// # Errors
///
/// [`Busy`] if too many passwords are being checked at once.
pub async fn verify_password(password: Secret, hash: String) -> Result<bool, Busy> {
    let checked = TURNS
        .run(move || {
            let password = password.into_exposed();
            // The parameters come from the hash itself, so older hashes still verify.
            Argon2::default()
                .verify_password(password.as_bytes(), hash.as_str())
                .is_ok()
        })
        .await?;
    Ok(checked.unwrap_or(false))
}

/// Whether `hash` was made with something other than today's algorithm and parameters,
/// so it should be replaced at the next successful sign-in.
#[must_use]
pub fn needs_rehash(hash: &str) -> bool {
    let Ok(parsed) = PasswordHash::new(hash) else {
        return true;
    };
    let current_algorithm = parsed.algorithm == Algorithm::Argon2id.ident();
    // Compare only the cost settings: a parsed hash records its output length explicitly,
    // which `Params::default()` leaves unset, so whole-struct equality would always differ.
    let current = Params::default();
    let current_params = Params::try_from(&parsed).is_ok_and(|params| {
        (params.m_cost(), params.t_cost(), params.p_cost())
            == (current.m_cost(), current.t_cost(), current.p_cost())
    });
    !(current_algorithm && current_params)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn a_hash_verifies_its_password_and_nothing_else() {
        let hash = hash_password("correct horse battery".into()).await.unwrap();

        assert!(hash.starts_with("$argon2id$"), "{hash}");
        assert!(
            verify_password("correct horse battery".into(), hash.clone())
                .await
                .unwrap()
        );
        assert!(
            !verify_password("correct horse batterY".into(), hash.clone())
                .await
                .unwrap()
        );
        assert!(!verify_password(Secret::default(), hash).await.unwrap());
    }

    #[tokio::test]
    async fn the_same_password_gets_a_different_hash_each_time() {
        let first = hash_password("same password!".into()).await.unwrap();
        let second = hash_password("same password!".into()).await.unwrap();
        assert_ne!(first, second, "random salts");
    }

    #[tokio::test]
    async fn current_hashes_dont_need_rehashing_and_older_ones_do() {
        let current = hash_password("correct horse battery".into()).await.unwrap();
        assert!(!needs_rehash(&current));

        // Same password, weaker parameters (as an older version might have used).
        let weak = Argon2::from(Params::new(8 * 1024, 1, 1, None).unwrap())
            .hash_password(b"correct horse battery")
            .unwrap()
            .to_string();
        assert!(needs_rehash(&weak));
        assert!(
            verify_password("correct horse battery".into(), weak)
                .await
                .unwrap(),
            "still verifies"
        );

        assert!(needs_rehash("not a hash"));
        assert!(
            !verify_password("x".into(), "not a hash".into())
                .await
                .unwrap()
        );
    }

    #[tokio::test]
    async fn a_third_hash_waits_for_a_turn_and_gives_up_as_busy() {
        let turns = Turns::new(2, Duration::from_millis(200));
        let (release, released) = std::sync::mpsc::channel::<()>();
        let released = Arc::new(std::sync::Mutex::new(released));
        let hold = |released: Arc<std::sync::Mutex<std::sync::mpsc::Receiver<()>>>| {
            move || {
                let _ = released.lock().unwrap().recv();
            }
        };
        let first = tokio::spawn({
            let turns = turns.clone();
            let work = hold(Arc::clone(&released));
            async move { turns.run(work).await }
        });
        let second = tokio::spawn({
            let turns = turns.clone();
            let work = hold(Arc::clone(&released));
            async move { turns.run(work).await }
        });
        // Both running: no turn is free.
        while turns.permits.available_permits() > 0 {
            tokio::task::yield_now().await;
        }
        assert_eq!(turns.run(|| ()).await.unwrap_err(), Busy);

        // One finishes, so the next gets its turn.
        release.send(()).unwrap();
        release.send(()).unwrap();
        assert!(first.await.unwrap().is_ok());
        assert!(second.await.unwrap().is_ok());
        assert_eq!(turns.run(|| 7).await.unwrap().unwrap(), 7);
    }
}
