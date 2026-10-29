//! The app library: everything behind the entry point in `main.rs`.
//!
//! UI components never touch the database and never contain business rules;
//! those live in `domain`, `storage`, and `server`. Keeping this a library makes
//! its modules unit-testable and lets later code use them before the UI does.

pub mod logging;
pub mod ui;
