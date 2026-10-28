//! Filtering and rendering log entries for export (Settings → Diagnostics → Export).
//!
//! Exports are designed to be fed back to an agent: JSON lines for analysis, or
//! plain text for reading, each behind a header describing where they came from.
//! Secrets are redacted at the source, so exports are safe to share.

use domain::time::UnixMillis;
use domain::time_utils::format_utc_iso8601;
use serde::Serialize;

use super::entry::{LogEntry, LogLevel};

/// Which entries to include in an export.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ExportFilter {
    /// Include entries at or after this instant (`None`: no lower bound).
    pub from: Option<UnixMillis>,
    /// Include entries at or before this instant (`None`: no upper bound).
    pub to: Option<UnixMillis>,
    /// Include entries at this severity or higher.
    pub min_level: LogLevel,
    /// Include only these areas (`None`: every area).
    pub areas: Option<Vec<String>>,
}

impl Default for ExportFilter {
    /// Everything: all times, all levels, all areas.
    fn default() -> Self {
        Self {
            from: None,
            to: None,
            min_level: LogLevel::Trace,
            areas: None,
        }
    }
}

impl ExportFilter {
    /// Whether `entry` passes every part of the filter.
    pub fn matches(&self, entry: &LogEntry) -> bool {
        let time = entry.time();
        let after_from = self.from.is_none_or(|from| time >= from);
        let before_to = self.to.is_none_or(|to| time <= to);
        let level_ok = entry.level >= self.min_level;
        let area_ok = self
            .areas
            .as_ref()
            .is_none_or(|areas| areas.contains(&entry.area));
        after_from && before_to && level_ok && area_ok
    }
}

/// Where an export came from, written as its first line(s).
///
/// API version and device ID are added once they exist (Phase 1 step 7, Phase 3).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ExportHeader {
    /// The app version, e.g. `0.1.0`.
    pub app_version: String,
    /// The platform the log came from, e.g. `web`, `desktop`, `server`.
    pub platform: String,
}

impl ExportHeader {
    /// A header for this build of the app on `platform`.
    pub fn for_this_build(platform: impl Into<String>) -> Self {
        Self {
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: platform.into(),
        }
    }
}

/// The header line of a JSON-lines export: `{"export":"log", ...header}`.
#[derive(Serialize)]
struct JsonHeaderLine<'a> {
    export: &'static str,
    #[serde(flatten)]
    header: &'a ExportHeader,
}

/// Renders the header and matching entries as JSON lines, one object per line.
///
/// # Errors
///
/// Returns an error only if a value can't be represented as JSON, which doesn't
/// happen for captured entries.
pub fn to_json_lines(
    header: &ExportHeader,
    entries: &[LogEntry],
    filter: &ExportFilter,
) -> Result<String, serde_json::Error> {
    let header_line = JsonHeaderLine {
        export: "log",
        header,
    };
    let mut output = serde_json::to_string(&header_line)?;
    output.push('\n');
    for entry in entries.iter().filter(|entry| filter.matches(entry)) {
        output.push_str(&entry.to_json_line()?);
        output.push('\n');
    }
    Ok(output)
}

/// Renders the header and matching entries as readable text, one entry per line:
/// `2023-11-14T22:13:20.123Z WARN  sync: message key=value`.
pub fn to_plain_text(header: &ExportHeader, entries: &[LogEntry], filter: &ExportFilter) -> String {
    let mut output = format!(
        "# app_version: {}\n# platform: {}\n",
        header.app_version, header.platform
    );
    for entry in entries.iter().filter(|entry| filter.matches(entry)) {
        output.push_str(&plain_text_line(entry));
        output.push('\n');
    }
    output
}

fn plain_text_line(entry: &LogEntry) -> String {
    // Pad levels to the longest name ("ERROR") so messages line up.
    let mut line = format!(
        "{} {:<5} {}: {}",
        format_utc_iso8601(entry.time()),
        entry.level.as_str(),
        entry.area,
        entry.message
    );
    for (name, value) in &entry.fields {
        line.push(' ');
        line.push_str(name);
        line.push('=');
        // Strings print bare (no JSON quotes); numbers and booleans print as JSON does.
        match value.as_str() {
            Some(text) => line.push_str(text),
            None => line.push_str(&value.to_string()),
        }
    }
    line
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    fn entry(time_ms: i64, level: LogLevel, area: &str, message: &str) -> LogEntry {
        LogEntry {
            time_ms,
            level,
            area: area.to_owned(),
            message: message.to_owned(),
            fields: BTreeMap::new(),
        }
    }

    fn sample_entries() -> Vec<LogEntry> {
        vec![
            entry(1_000, LogLevel::Debug, "sync", "pull started"),
            entry(2_000, LogLevel::Info, "ui", "opened garage"),
            entry(3_000, LogLevel::Warn, "sync", "server slow"),
            entry(4_000, LogLevel::Error, "auth", "token rejected"),
        ]
    }

    fn header() -> ExportHeader {
        ExportHeader {
            app_version: "0.1.0".to_owned(),
            platform: "desktop".to_owned(),
        }
    }

    fn matching_messages(filter: &ExportFilter) -> Vec<String> {
        sample_entries()
            .into_iter()
            .filter(|entry| filter.matches(entry))
            .map(|entry| entry.message)
            .collect()
    }

    #[test]
    fn export_filters_by_time_range_level_and_area() {
        assert_eq!(matching_messages(&ExportFilter::default()).len(), 4);

        let time_range = ExportFilter {
            from: Some(UnixMillis::from_millis(2_000)),
            to: Some(UnixMillis::from_millis(3_000)),
            ..ExportFilter::default()
        };
        assert_eq!(
            matching_messages(&time_range),
            ["opened garage", "server slow"]
        );

        let warnings_and_up = ExportFilter {
            min_level: LogLevel::Warn,
            ..ExportFilter::default()
        };
        assert_eq!(
            matching_messages(&warnings_and_up),
            ["server slow", "token rejected"]
        );

        let sync_only = ExportFilter {
            areas: Some(vec!["sync".to_owned()]),
            ..ExportFilter::default()
        };
        assert_eq!(
            matching_messages(&sync_only),
            ["pull started", "server slow"]
        );
    }

    #[test]
    fn json_lines_export_has_a_header_and_one_line_per_entry() {
        let filter = ExportFilter {
            min_level: LogLevel::Warn,
            ..ExportFilter::default()
        };

        let output = to_json_lines(&header(), &sample_entries(), &filter).unwrap();

        assert_eq!(
            output,
            concat!(
                r#"{"export":"log","app_version":"0.1.0","platform":"desktop"}"#,
                "\n",
                r#"{"time_ms":3000,"level":"WARN","area":"sync","message":"server slow"}"#,
                "\n",
                r#"{"time_ms":4000,"level":"ERROR","area":"auth","message":"token rejected"}"#,
                "\n",
            )
        );
    }

    #[test]
    fn plain_text_export_is_readable_and_aligned() {
        let mut entries = sample_entries();
        entries[2].fields = BTreeMap::from([
            ("attempt".to_owned(), serde_json::json!(2)),
            ("host".to_owned(), serde_json::json!("nas")),
        ]);
        let filter = ExportFilter {
            min_level: LogLevel::Info,
            ..ExportFilter::default()
        };

        let output = to_plain_text(&header(), &entries, &filter);

        assert_eq!(
            output,
            "# app_version: 0.1.0\n\
             # platform: desktop\n\
             1970-01-01T00:00:02.000Z INFO  ui: opened garage\n\
             1970-01-01T00:00:03.000Z WARN  sync: server slow attempt=2 host=nas\n\
             1970-01-01T00:00:04.000Z ERROR auth: token rejected\n"
        );
    }
}
