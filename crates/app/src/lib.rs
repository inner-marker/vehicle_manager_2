//! The app library: everything behind the entry point in `main.rs`.
//!
//! UI components never touch the database and never contain business rules;
//! those live in `domain`, `storage`, and `server`. Keeping this a library makes
//! its modules unit-testable and lets later code use them before the UI does.

pub mod api;
#[cfg(not(target_arch = "wasm32"))]
pub mod data_folder;
pub mod logging;
#[cfg(not(target_arch = "wasm32"))]
pub mod settings;
pub mod ui;
