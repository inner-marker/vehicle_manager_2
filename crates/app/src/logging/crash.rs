//! The "last crash" file (native only, plan §5 Debug log, Retention).
//!
//! If anything panics (our code must not, but a dependency can), a panic hook writes
//! `last-crash.txt` into the log directory before the process goes down. It sits
//! outside the log rotation (`rotating_file` only touches `current.jsonl` and
//! `log-*.jsonl`), so the record survives the restart and isn't pushed out by the
//! logging that follows it. Each crash replaces the previous file.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use domain::time::{Clock, UnixMillis};
use domain::time_utils::format_utc_iso8601;

/// The crash file's name inside the log directory.
pub const CRASH_FILE_NAME: &str = "last-crash.txt";

/// Installs the panic hook that writes the crash file into `log_dir`, then runs the
/// previous hook (so the panic still reaches stderr and the container's log).
pub fn install_crash_file(log_dir: &Path, clock: Arc<dyn Clock + Send + Sync>) {
    let path: PathBuf = log_dir.join(CRASH_FILE_NAME);
    let previous_hook = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |info| {
        let backtrace = std::backtrace::Backtrace::force_capture();
        let report = crash_report(clock.now(), &info.to_string(), &backtrace.to_string());
        // Nothing useful can be done if this write fails while panicking, and logging
        // through `tracing` here could deadlock on a lock the panicking code holds;
        // the previous hook below still reports the panic.
        let _unwritten = std::fs::write(&path, report);
        previous_hook(info);
    }));
}

/// The crash file's text: when, which version, what panicked and where, and the backtrace.
pub fn crash_report(time: UnixMillis, panic: &str, backtrace: &str) -> String {
    format!(
        "time: {}\napp_version: {}\n\n{panic}\n\nbacktrace:\n{backtrace}\n",
        format_utc_iso8601(time),
        env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crash_report_has_time_version_panic_and_backtrace() {
        let report = crash_report(
            UnixMillis::from_millis(1_700_000_000_123),
            "panicked at src/sync.rs:10:5:\nindex out of bounds",
            "0: app::sync::apply",
        );

        assert!(
            report.starts_with("time: 2023-11-14T22:13:20.123Z\n"),
            "{report}"
        );
        assert!(report.contains(concat!("app_version: ", env!("CARGO_PKG_VERSION"))));
        assert!(report.contains("panicked at src/sync.rs:10:5:\nindex out of bounds"));
        assert!(report.contains("backtrace:\n0: app::sync::apply"));
    }

    #[test]
    fn crash_file_is_outside_the_log_rotation() {
        // The rotating writer only touches current.jsonl and log-<ms>.jsonl.
        let extension = Path::new(CRASH_FILE_NAME)
            .extension()
            .and_then(|e| e.to_str());
        assert_ne!(extension, Some("jsonl"));
        assert!(!CRASH_FILE_NAME.starts_with("log-"));
    }
}
