//! Entry point for every target: the web UI, desktop and mobile apps, and
//! (with the `server` feature) the fullstack server.
//!
//! Kept thin: the app's modules live in the `app` library (`lib.rs`).

use app::logging;
use dioxus::LaunchBuilder;
use domain::log_area;

fn main() {
    // Logging must start before `launch`, so Dioxus's own logger sees ours and steps aside.
    let mut launcher = LaunchBuilder::new();
    match logging::init() {
        // The log viewer reads the handles from context.
        Ok(log_handles) => launcher = launcher.with_context(log_handles),
        // Another subscriber got there first; it still receives our events.
        Err(error) => tracing::warn!(target: log_area::APP, %error, "debug log not installed"),
    }
    tracing::info!(target: log_area::APP, version = env!("CARGO_PKG_VERSION"), "app starting");

    launcher.launch(app::ui::App);
}
