//! SQLite storage for devices and the server: migrations, row structs,
//! row ↔ domain `TryFrom` conversions, queries, and snapshots; and the desktop's encrypted
//! copy (`local`, with the `sqlcipher` feature).
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
pub mod devices;
pub mod documents;
mod error;
pub mod events;
pub mod file_audit;
pub mod files;
mod integrity;
pub mod jobs;
pub mod local;
pub mod local_files;
pub mod message_marks;
pub mod outbox;
pub mod places;
pub mod preferences;
pub mod references;
pub mod requirements;
pub mod snapshot;
pub mod squawks;
pub mod sync;
pub mod users;

pub use database::{Database, NewDatabaseIds, OpenReport};
pub use error::StorageError;
pub use local::{CopyOf, LocalKey, LocalOpenReport};
