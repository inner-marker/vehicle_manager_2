//! Whether the server is healthy enough to accept changes (plan §6 "Server self-checks").
//!
//! An unhealthy server keeps serving pages and the handshake, so devices and people can
//! see what's wrong, but refuses every command (from step 1e), rather than writing to a
//! database that's missing or damaged. The handshake reports it.
//!
//! One state for the whole process, set at startup and by the periodic checks, with the time
//! it last changed (the message center's "since", #050).

use std::sync::RwLock;

use domain::time::UnixMillis;

/// The server's health.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Health {
    /// Still opening and checking the database.
    Starting,
    /// The database is open and passed its last check.
    Healthy,
    /// Something is wrong; see the server log.
    Unhealthy(HealthProblem),
}

/// What's wrong with an unhealthy server.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HealthProblem {
    /// The database couldn't be opened (e.g. written by a newer version, or unreadable).
    DatabaseUnavailable,
    /// SQLite's integrity check found problems.
    IntegrityCheckFailed,
}

impl Health {
    /// Whether the server accepts changes.
    #[must_use]
    pub const fn is_healthy(self) -> bool {
        matches!(self, Self::Healthy)
    }
}

/// The health, and when it last changed.
static CURRENT: RwLock<(Health, UnixMillis)> = RwLock::new((Health::Starting, UnixMillis::EPOCH));

/// The current health and when it became so.
fn read() -> (Health, UnixMillis) {
    // A poisoned lock means a panic while setting it; the last value is still the best answer.
    *CURRENT
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The current health.
#[must_use]
pub fn current() -> Health {
    read().0
}

/// What's wrong and since when, if the server is unhealthy.
#[must_use]
pub fn unhealthy_since() -> Option<(HealthProblem, UnixMillis)> {
    match read() {
        (Health::Unhealthy(problem), since) => Some((problem, since)),
        (Health::Starting | Health::Healthy, _) => None,
    }
}

/// Sets the health at `now` (startup and periodic checks only). Setting the same health again
/// keeps the time it started.
pub fn set(health: Health, now: UnixMillis) {
    let mut current = CURRENT
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if current.0 != health {
        *current = (health, now);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_healthy_accepts_changes() {
        assert!(Health::Healthy.is_healthy());
        assert!(!Health::Starting.is_healthy());
        assert!(!Health::Unhealthy(HealthProblem::IntegrityCheckFailed).is_healthy());
    }
}
