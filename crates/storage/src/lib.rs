//! SQLite storage for devices and the server: migrations, row structs,
//! row ↔ domain `TryFrom` conversions, queries, and snapshots.
//!
//! This is the **only** place unit conversion and rounding happen between the
//! integer database representation and the `uom` domain types.
//!
//! Native only: the web (WASM) build must never depend on this crate.

pub mod commands;
pub mod convert;
pub mod database;
mod error;
mod integrity;
pub mod jobs;
pub mod snapshot;

pub use database::{Database, NewDatabaseIds, OpenReport};
pub use error::StorageError;
