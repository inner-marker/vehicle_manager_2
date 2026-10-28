//! The in-app debug log: capture, in-memory buffer, rotating files, levels, and export.
//!
//! Every target (web, desktop, mobile, server) calls [`init`] once, before
//! `dioxus::launch`, so Dioxus's own logger sees a subscriber already installed
//! and steps aside. Events then flow:
//!
//! ```text
//! tracing event ─▶ level filter (per area, adjustable) ─┬─▶ console (terminal / browser devtools)
//!                                                       └─▶ capture ─┬─▶ ring buffer (viewer, export)
//!                                                                    └─▶ log file (native, once wired)
//! ```
//!
//! See plan §5 "Debug log" and `docs/phase_plans/phase-1.md` step 4.

mod capture;
mod clock;
mod control;
mod entry;
mod export;
mod ring_buffer;
#[cfg(not(target_arch = "wasm32"))]
mod rotating_file;

use std::sync::Arc;

pub use capture::{CaptureLayer, EntrySink};
pub use clock::SystemClock;
pub use control::{DEFAULT_LEVEL, LevelControl, default_levels};
use domain::log_area;
pub use entry::{LogEntry, LogLevel};
pub use export::{ExportFilter, ExportHeader, to_json_lines, to_plain_text};
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

/// Installs the global logging subscriber. Call once, before `dioxus::launch`.
///
/// Captures into the in-memory ring buffer and writes to the console. The log file
/// is added once the platform's log directory is known (milestones 3 and 4).
///
/// # Errors
///
/// Returns an error if a global subscriber is already installed.
pub fn init() -> Result<LogHandles, SetGlobalDefaultError> {
    let ring_buffer = Arc::new(RingBuffer::new(RING_BUFFER_CAPACITY));
    let sinks: Vec<Arc<dyn EntrySink>> = vec![Arc::clone(&ring_buffer) as Arc<dyn EntrySink>];
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
