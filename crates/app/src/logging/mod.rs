//! The in-app debug log: capture, in-memory buffer, rotating files, levels, and export.
//!
//! Every target calls one of these once, before launching, so Dioxus's own logger
//! sees a subscriber already installed and steps aside:
//! - [`init`]: console and ring buffer (web, and native without a log directory).
//! - `init_with_log_dir` (native): also the rotating log file and the "last crash"
//!   file, e.g. the server with [`DATA_DIR_ENV_NAME`] set.
//!
//! Events then flow:
//!
//! ```text
//! tracing event ─▶ level filter (per area, adjustable) ─┬─▶ console (terminal / browser devtools)
//!                                                       └─▶ capture ─┬─▶ ring buffer (viewer, export)
//!                                                                    └─▶ log file (native, with a log directory)
//! ```
//!
//! See plan §5 "Debug log" and `docs/phase_plans/phase-1.md` step 4.

mod capture;
mod clock;
mod control;
#[cfg(not(target_arch = "wasm32"))]
mod crash;
mod entry;
mod export;
mod ring_buffer;
#[cfg(not(target_arch = "wasm32"))]
mod rotating_file;

use std::sync::Arc;

pub use capture::{CaptureLayer, EntrySink};
pub use clock::SystemClock;
pub use control::{DEFAULT_LEVEL, LevelControl, default_levels};
#[cfg(not(target_arch = "wasm32"))]
pub use crash::{CRASH_FILE_NAME, crash_report, install_crash_file};
use domain::log_area;
pub use entry::{LogEntry, LogLevel};
pub use export::{ExportFilter, ExportHeader, format_fields, to_json_lines, to_plain_text};
pub use ring_buffer::{RING_BUFFER_CAPACITY, RingBuffer};
#[cfg(not(target_arch = "wasm32"))]
pub use rotating_file::{FileLogPolicy, FileSink, RotatingFile};
use tracing::Subscriber;
use tracing::subscriber::SetGlobalDefaultError;
use tracing_subscriber::Layer;
use tracing_subscriber::layer::SubscriberExt;
use tracing_subscriber::registry::LookupSpan;
use tracing_subscriber::reload;

/// What the rest of the app needs after logging starts: the entries for the log
/// viewer and export, and the runtime level switch.
#[derive(Clone)]
pub struct LogHandles {
    /// Recent entries, oldest first.
    pub ring_buffer: Arc<RingBuffer>,
    /// Per-area level control.
    pub levels: LevelControl,
}

/// The environment variable naming the server's data directory, without the app
/// prefix (`domain::identity::env_var_name` makes it `VEHICLEMANAGER_DATA_DIR`).
/// Logs go to its `logs/` subfolder.
pub const DATA_DIR_ENV_NAME: &str = "DATA_DIR";

/// The log folder inside the data directory.
pub const LOG_DIR_NAME: &str = "logs";

/// Installs the global logging subscriber: console and in-memory ring buffer only.
/// Call once, before launching.
///
/// # Errors
///
/// Returns an error if a global subscriber is already installed.
pub fn init() -> Result<LogHandles, SetGlobalDefaultError> {
    install(Vec::new())
}

/// Installs the global logging subscriber with rotating log files in `log_dir`
/// (created if missing) and the "last crash" file. Call once, before launching.
///
/// If `log_dir` can't be used (e.g. not writable), logging still starts without
/// files and a warning says why: the app never refuses to start over logging.
///
/// # Errors
///
/// Returns an error if a global subscriber is already installed.
#[cfg(not(target_arch = "wasm32"))]
pub fn init_with_log_dir(
    log_dir: &std::path::Path,
    policy: FileLogPolicy,
) -> Result<LogHandles, SetGlobalDefaultError> {
    let clock: Arc<dyn domain::time::Clock + Send + Sync> = Arc::new(SystemClock);
    let file_sink = open_file_sink(log_dir, policy, Arc::clone(&clock));
    let extra_sinks = file_sink.as_ref().map_or_else(
        |_| Vec::new(),
        |sink| vec![Arc::clone(sink) as Arc<dyn EntrySink>],
    );
    let handles = install(extra_sinks)?;

    // Logged only now, once the subscriber exists to record it.
    match file_sink {
        Ok(_) => {
            install_crash_file(log_dir, clock);
            tracing::info!(target: log_area::APP, log_dir = %log_dir.display(), "writing log files");
        }
        Err(error) => tracing::warn!(
            target: log_area::APP,
            log_dir = %log_dir.display(),
            %error,
            "can't write log files here; logging to the console and in-app viewer only"
        ),
    }
    Ok(handles)
}

/// Opens the rotating log file in `log_dir` as a sink.
///
/// # Errors
///
/// Returns the I/O error if the folder can't be created or the file can't be opened.
#[cfg(not(target_arch = "wasm32"))]
pub fn open_file_sink(
    log_dir: &std::path::Path,
    policy: FileLogPolicy,
    clock: Arc<dyn domain::time::Clock + Send + Sync>,
) -> std::io::Result<Arc<FileSink>> {
    RotatingFile::open(log_dir, policy, clock).map(|file| Arc::new(FileSink::new(file)))
}

/// Builds and installs the subscriber, with the ring buffer plus `extra_sinks`.
fn install(extra_sinks: Vec<Arc<dyn EntrySink>>) -> Result<LogHandles, SetGlobalDefaultError> {
    let ring_buffer = Arc::new(RingBuffer::new(RING_BUFFER_CAPACITY));
    let mut sinks: Vec<Arc<dyn EntrySink>> = vec![Arc::clone(&ring_buffer) as Arc<dyn EntrySink>];
    sinks.extend(extra_sinks);
    let capture = CaptureLayer::new(Arc::new(SystemClock), sinks);

    // The level filter sits directly on the registry, so it gates every layer above it.
    let (level_filter, level_handle) = reload::Layer::new(default_levels());
    let subscriber = tracing_subscriber::registry()
        .with(level_filter)
        .with(capture)
        .with(console_layer());
    tracing::subscriber::set_global_default(subscriber)?;

    tracing::debug!(
        target: log_area::APP,
        ring_buffer_capacity = RING_BUFFER_CAPACITY,
        "debug log initialized"
    );
    Ok(LogHandles {
        ring_buffer,
        levels: LevelControl::new(level_handle),
    })
}

/// Console output on native: human-readable lines on the terminal (stdout).
#[cfg(not(target_arch = "wasm32"))]
fn console_layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    tracing_subscriber::fmt::layer()
}

/// Console output on web: the browser's devtools console.
#[cfg(target_arch = "wasm32")]
fn console_layer<S>() -> impl Layer<S>
where
    S: Subscriber + for<'span> LookupSpan<'span>,
{
    tracing_wasm::WASMLayer::default()
}

#[cfg(all(test, not(target_arch = "wasm32")))]
mod tests {
    use domain::test_support::ManualClock;
    use domain::time::UnixMillis;
    use tracing_subscriber::layer::SubscriberExt;

    use super::*;

    fn clock() -> Arc<dyn domain::time::Clock + Send + Sync> {
        Arc::new(ManualClock::new(UnixMillis::from_millis(1_700_000_000_000)))
    }

    #[test]
    fn file_sink_writes_captured_events_to_the_log_dir() {
        let temp = tempfile::tempdir().unwrap();
        let log_dir = temp.path().join(LOG_DIR_NAME);
        let sink = open_file_sink(&log_dir, FileLogPolicy::SERVER, clock()).unwrap();
        let capture = CaptureLayer::new(clock(), vec![sink as Arc<dyn EntrySink>]);

        tracing::subscriber::with_default(tracing_subscriber::registry().with(capture), || {
            tracing::info!(target: log_area::APP, "written to disk");
        });

        let written = std::fs::read_to_string(log_dir.join("current.jsonl")).unwrap();
        let entry = LogEntry::from_json_line(written.trim_end()).unwrap();
        assert_eq!(entry.message, "written to disk");
    }

    #[test]
    fn unusable_log_dir_is_an_error_not_a_panic() {
        let temp = tempfile::tempdir().unwrap();
        // A regular file where the folder should be: the directory can't be created.
        let blocked = temp.path().join("not-a-folder");
        std::fs::write(&blocked, "").unwrap();

        assert!(
            open_file_sink(&blocked.join(LOG_DIR_NAME), FileLogPolicy::SERVER, clock()).is_err()
        );
    }

    #[test]
    fn data_dir_env_var_comes_from_app_id() {
        assert_eq!(
            domain::identity::env_var_name(DATA_DIR_ENV_NAME),
            "VEHICLEMANAGER_DATA_DIR"
        );
    }
}
