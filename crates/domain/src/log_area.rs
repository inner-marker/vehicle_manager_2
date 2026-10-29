//! Names of the debug-log areas, shared by every crate.
//!
//! Each area is used as the `tracing` **target**, so the in-app log viewer and
//! exports can filter by area and the level of each area can be raised at runtime
//! (e.g. `sync` at `debug` while chasing a sync problem). Always log with an
//! explicit area rather than the default module-path target.
//!
//! # Examples
//!
//! ```
//! use domain::log_area;
//!
//! // In any crate: tracing::debug!(target: log_area::SYNC, pending = 3, "pushing outbox");
//! assert_eq!(log_area::SYNC, "sync");
//! assert!(log_area::ALL.contains(&log_area::SYNC));
//! ```

/// App lifecycle: startup, shutdown, configuration.
pub const APP: &str = "app";

/// User interface: navigation, rendering, user actions.
pub const UI: &str = "ui";

/// Sync engine: push, pull, handshake, protective mode.
pub const SYNC: &str = "sync";

/// Local and master database: migrations, queries, snapshots.
pub const DATABASE: &str = "database";

/// Authentication and permissions. Never log tokens, cookies, or credentials.
pub const AUTH: &str = "auth";

/// Every area, in the order the log viewer lists them.
pub const ALL: [&str; 5] = [APP, UI, SYNC, DATABASE, AUTH];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn area_names_are_unique() {
        let mut names = ALL.to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), ALL.len());
    }

    #[test]
    fn area_names_are_single_lowercase_words() {
        // `tracing` targets match by prefix on `::`, so an area must not contain it.
        for area in ALL {
            assert!(!area.is_empty());
            assert!(area.chars().all(|c| c.is_ascii_lowercase()), "{area:?}");
        }
    }
}
