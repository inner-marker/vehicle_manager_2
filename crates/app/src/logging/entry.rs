//! One captured log event, and its JSON-lines form.

use std::collections::BTreeMap;

use domain::time::UnixMillis;
use serde::{Deserialize, Serialize};
use tracing::Level;

/// Severity of a log entry, ordered from least (`Trace`) to most (`Error`) severe.
///
/// Our own type (rather than `tracing::Level`) so it serializes and compares the
/// way the log viewer and exports need: `entry.level >= LogLevel::Warn` means
/// "warnings and errors".
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "UPPERCASE")]
pub enum LogLevel {
    /// Very fine-grained detail.
    Trace,
    /// Troubleshooting detail; off by default, switched on per area at runtime.
    Debug,
    /// Normal operation.
    Info,
    /// Something unexpected that the app recovered from.
    Warn,
    /// A failure.
    Error,
}

impl LogLevel {
    /// Every level, most severe first (the order the log viewer offers them).
    pub const ALL: [Self; 5] = [
        Self::Error,
        Self::Warn,
        Self::Info,
        Self::Debug,
        Self::Trace,
    ];

    /// The level's name as shown in logs and exports, e.g. `"WARN"`.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Trace => "TRACE",
            Self::Debug => "DEBUG",
            Self::Info => "INFO",
            Self::Warn => "WARN",
            Self::Error => "ERROR",
        }
    }
}

impl From<&Level> for LogLevel {
    fn from(level: &Level) -> Self {
        // `tracing::Level` is a struct of constants, not an enum, so it can't be
        // matched exhaustively; compare against each constant instead.
        if *level == Level::ERROR {
            Self::Error
        } else if *level == Level::WARN {
            Self::Warn
        } else if *level == Level::INFO {
            Self::Info
        } else if *level == Level::DEBUG {
            Self::Debug
        } else {
            Self::Trace
        }
    }
}

/// One log event as stored in the ring buffer and written as one JSON line.
///
/// Field values keep their JSON type (numbers stay numbers) so exported logs can be
/// analyzed without re-parsing strings. `BTreeMap` keeps field order stable.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LogEntry {
    /// When the event happened, as Unix epoch milliseconds (UTC).
    pub time_ms: i64,
    /// Severity.
    pub level: LogLevel,
    /// The `tracing` target: one of `domain::log_area`, or a dependency's module path.
    pub area: String,
    /// The event's message.
    pub message: String,
    /// The event's other structured fields.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub fields: BTreeMap<String, serde_json::Value>,
}

impl LogEntry {
    /// When the event happened.
    pub const fn time(&self) -> UnixMillis {
        UnixMillis::from_millis(self.time_ms)
    }

    /// Serializes the entry as one JSON line (without the trailing newline).
    ///
    /// # Errors
    ///
    /// Returns an error only if a field value can't be represented as JSON, which
    /// doesn't happen for values captured from `tracing` events.
    pub fn to_json_line(&self) -> Result<String, serde_json::Error> {
        serde_json::to_string(self)
    }

    /// Parses an entry from one JSON line, e.g. when reading a log file back.
    ///
    /// # Errors
    ///
    /// Returns an error if the line isn't a valid entry.
    pub fn from_json_line(line: &str) -> Result<Self, serde_json::Error> {
        serde_json::from_str(line)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_entry() -> LogEntry {
        LogEntry {
            time_ms: 1_700_000_000_123,
            level: LogLevel::Warn,
            area: "sync".to_owned(),
            message: "server rejected command".to_owned(),
            fields: BTreeMap::from([
                ("attempt".to_owned(), serde_json::json!(3)),
                ("reason".to_owned(), serde_json::json!("stale reading")),
            ]),
        }
    }

    #[test]
    fn log_entry_round_trips_through_a_json_line() {
        let entry = sample_entry();
        let line = entry.to_json_line().unwrap();

        assert!(!line.contains('\n'), "must be a single line");
        assert_eq!(LogEntry::from_json_line(&line).unwrap(), entry);
    }

    #[test]
    fn log_entry_json_shape_is_stable() {
        // Exports are read by people and agents; keep the field names and order stable.
        assert_eq!(
            sample_entry().to_json_line().unwrap(),
            r#"{"time_ms":1700000000123,"level":"WARN","area":"sync","message":"server rejected command","fields":{"attempt":3,"reason":"stale reading"}}"#
        );
    }

    #[test]
    fn log_levels_order_by_severity() {
        assert!(LogLevel::Trace < LogLevel::Debug);
        assert!(LogLevel::Info < LogLevel::Warn);
        assert!(LogLevel::Warn < LogLevel::Error);
        assert_eq!(LogLevel::from(&Level::WARN), LogLevel::Warn);
        assert_eq!(LogLevel::from(&Level::TRACE), LogLevel::Trace);
    }
}
