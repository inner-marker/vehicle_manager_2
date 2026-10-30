//! Whether the server is healthy enough to accept changes (plan §6 "Server self-checks").
//!
//! An unhealthy server keeps serving pages and the handshake, so devices and people can
//! see what's wrong, but refuses every command (from step 1e), rather than writing to a
//! database that's missing or damaged. The handshake reports it.
//!
//! One state for the whole process, set at startup and by the periodic checks.

use std::sync::RwLock;

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

static CURRENT: RwLock<Health> = RwLock::new(Health::Starting);

/// The current health.
#[must_use]
pub fn current() -> Health {
    // A poisoned lock means a panic while setting it; the last value is still the best answer.
    *CURRENT
        .read()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Sets the health (startup and periodic checks only).
pub fn set(health: Health) {
    *CURRENT
        .write()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = health;
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
