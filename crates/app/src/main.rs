//! Entry point for every target: the web UI, desktop and mobile apps, and
//! (with the `server` feature) the fullstack server.
//!
//! UI components never touch the database and never contain business rules;
//! those live in `domain`, `storage`, and `server`.

use dioxus::prelude::*;

fn main() {
    launch(App);
}

/// Root component.
///
/// Intentionally empty for now: the skeleton screens arrive in Phase 1, and all
/// user-facing text must go through the translation layer (Phase 2).
#[component]
fn App() -> Element {
    rsx! {}
}
