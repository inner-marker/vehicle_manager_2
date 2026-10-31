//! Password hashing with argon2id (decision #028).
//!
//! Hashes are PHC strings (`$argon2id$v=19$m=…,t=…,p=…$salt$hash`) with a random salt, using
//! the `argon2` crate's recommended parameters. A hash made with older parameters still
//! verifies, and [`needs_rehash`] tells sign-in to replace it with a current one.
//!
//! Hashing is deliberately slow (tens of milliseconds), so callers run it on a blocking
//! thread ([`hash_password`] and [`verify_password`] do this themselves).

use argon2::password_hash::phc::PasswordHash;
use argon2::password_hash::{PasswordHasher, PasswordVerifier};
use argon2::{Algorithm, Argon2, Params};

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
/// [`HashError`] if the OS has no random bytes for the salt, or the blocking task fails.
pub async fn hash_password(password: String) -> Result<String, HashError> {
    tokio::task::spawn_blocking(move || {
        Argon2::default()
            .hash_password(password.as_bytes())
            .map(|hash| hash.to_string())
            .map_err(|error| HashError(error.to_string()))
    })
    .await
    .map_err(|error| HashError(error.to_string()))?
}

/// Whether `password` matches `hash`. A malformed hash never matches.
pub async fn verify_password(password: String, hash: String) -> bool {
    tokio::task::spawn_blocking(move || {
        // The parameters come from the hash itself, so older hashes still verify.
        Argon2::default()
            .verify_password(password.as_bytes(), hash.as_str())
            .is_ok()
    })
    .await
    .unwrap_or(false)
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
        assert!(verify_password("correct horse battery".into(), hash.clone()).await);
        assert!(!verify_password("correct horse batterY".into(), hash.clone()).await);
        assert!(!verify_password(String::new(), hash).await);
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
            verify_password("correct horse battery".into(), weak).await,
            "still verifies"
        );

        assert!(needs_rehash("not a hash"));
        assert!(!verify_password("x".into(), "not a hash".into()).await);
    }
}
