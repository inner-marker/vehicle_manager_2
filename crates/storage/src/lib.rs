//! SQLite storage for devices and the server: migrations, row structs,
//! row ↔ domain `TryFrom` conversions, queries, and snapshots.
//!
//! This is the **only** place unit conversion and rounding happen between the
//! integer database representation and the `uom` domain types.
//!
//! Native only: the web (WASM) build must never depend on this crate.

pub mod assets;
pub mod attachments;
pub mod commands;
pub mod convert;
pub mod database;
pub mod documents;
mod error;
pub mod events;
pub mod files;
mod integrity;
pub mod jobs;
pub mod message_marks;
pub mod places;
pub mod preferences;
pub mod requirements;
pub mod snapshot;
pub mod squawks;
pub mod users;

pub use database::{Database, NewDatabaseIds, OpenReport};
pub use error::StorageError;
