//! Entry point for every target: the web UI, desktop and mobile apps, and
//! (with the `server` feature) the fullstack server.
//!
//! Kept thin: the app's modules live in the `app` library (`lib.rs`).

use app::logging;
use dioxus::prelude::*;
use domain::log_area;

fn main() {
    // Must run before `launch`, so Dioxus's own logger sees ours and steps aside.
    match logging::init() {
        // The handles go to the UI through context once the log viewer exists (step 6).
        Ok(_log_handles) => {}
        // Another subscriber got there first; it still receives our events.
        Err(error) => tracing::warn!(target: log_area::APP, %error, "debug log not installed"),
    }
    tracing::info!(target: log_area::APP, version = env!("CARGO_PKG_VERSION"), "app starting");

    launch(App);
}

/// Root component.
///
/// Intentionally empty for now: the skeleton screens arrive in step 6, and all
/// user-facing text must go through the translation layer (step 5).
#[component]
fn App() -> Element {
    rsx! {}
}
