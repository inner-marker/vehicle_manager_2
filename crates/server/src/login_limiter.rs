//! Limits failed sign-ins (phase-2.md milestone 2): after [`MAX_FAILURES`] failures within
//! [`WINDOW_MILLIS`] for one key, further attempts with that key are refused until the oldest
//! failure leaves the window.
//!
//! Keys are the username tried and the client (where the request came from), so guessing
//! one person's password and spraying one password across many usernames are both slowed.
//! Kept in memory: a restart clears it, which is acceptable for a household server behind
//! Tailscale. A successful sign-in clears its username's failures.

use std::collections::HashMap;
use std::sync::Mutex;

use domain::time::UnixMillis;

/// Failures allowed within the window before attempts are refused.
pub const MAX_FAILURES: usize = 5;

/// The window: 15 minutes.
pub const WINDOW_MILLIS: i64 = 15 * 60 * 1000;

/// Failed sign-in times, per key.
#[derive(Debug, Default)]
pub struct LoginLimiter {
    failures: Mutex<HashMap<String, Vec<UnixMillis>>>,
}

impl LoginLimiter {
    /// A limiter with no failures recorded.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// `Ok` if an attempt with these keys may go ahead; otherwise how many milliseconds until
    /// it may (the longest wait among the keys).
    ///
    /// # Errors
    ///
    /// The wait in milliseconds, if any key is at the limit.
    pub fn check(&self, keys: &[&str], now: UnixMillis) -> Result<(), i64> {
        let mut failures = self.lock();
        let mut longest_wait = None;
        for key in keys {
            let Some(times) = failures.get_mut(*key) else {
                continue;
            };
            prune(times, now);
            // At the limit, the oldest failure in the window decides when it frees up.
            if times.len() >= MAX_FAILURES
                && let Some(oldest) = times.first()
            {
                let wait = oldest.as_millis() + WINDOW_MILLIS - now.as_millis();
                longest_wait = Some(longest_wait.map_or(wait, |longest: i64| longest.max(wait)));
            }
        }
        longest_wait.map_or(Ok(()), Err)
    }

    /// Records a failed attempt against each key.
    pub fn record_failure(&self, keys: &[&str], now: UnixMillis) {
        let mut failures = self.lock();
        for key in keys {
            let times = failures.entry((*key).to_owned()).or_default();
            prune(times, now);
            times.push(now);
        }
    }

    /// Forgets the failures of one key (after a successful sign-in).
    pub fn clear(&self, key: &str) {
        self.lock().remove(key);
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, Vec<UnixMillis>>> {
        // A panic while holding the lock leaves plain data behind; keep using it.
        self.failures
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
    }
}

/// Drops failures older than the window.
fn prune(times: &mut Vec<UnixMillis>, now: UnixMillis) {
    times.retain(|time| now.as_millis() - time.as_millis() < WINDOW_MILLIS);
}

#[cfg(test)]
mod tests {
    use super::*;

    const NOW: UnixMillis = UnixMillis::from_millis(1_700_000_000_000);

    fn at(offset_millis: i64) -> UnixMillis {
        UnixMillis::from_millis(NOW.as_millis() + offset_millis)
    }

    #[test]
    fn five_failures_are_allowed_and_the_sixth_attempt_waits() {
        let limiter = LoginLimiter::new();
        for minute in 0..5 {
            assert_eq!(limiter.check(&["user:stuart"], at(minute * 60_000)), Ok(()));
            limiter.record_failure(&["user:stuart"], at(minute * 60_000));
        }

        // The first failure was at NOW, so attempts free up at NOW + 15 minutes.
        assert_eq!(
            limiter.check(&["user:stuart"], at(10 * 60_000)),
            Err(5 * 60_000)
        );
        assert_eq!(limiter.check(&["user:stuart"], at(WINDOW_MILLIS)), Ok(()));
    }

    #[test]
    fn keys_are_independent_and_the_longest_wait_wins() {
        let limiter = LoginLimiter::new();
        for _ in 0..5 {
            limiter.record_failure(&["user:stuart", "client:phone"], NOW);
        }

        assert!(limiter.check(&["user:other", "client:laptop"], NOW).is_ok());
        assert!(
            limiter.check(&["user:other", "client:phone"], NOW).is_err(),
            "same client"
        );
        assert!(
            limiter
                .check(&["user:stuart", "client:laptop"], NOW)
                .is_err(),
            "same user"
        );
    }

    #[test]
    fn a_successful_sign_in_clears_that_users_failures() {
        let limiter = LoginLimiter::new();
        for _ in 0..5 {
            limiter.record_failure(&["user:stuart"], NOW);
        }
        limiter.clear("user:stuart");
        assert_eq!(limiter.check(&["user:stuart"], NOW), Ok(()));
    }
}
