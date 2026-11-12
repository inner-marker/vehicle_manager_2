//! Server library: the sync engine (apply commands, serve pulls), auth and
//! permission enforcement, integrity checks, nightly snapshots, and admin
//! operations.
//!
//! Used by the NAS server and, later, desktop host mode, so both share the
//! same code. Native only: the web (WASM) build must never depend on this crate.

pub mod assets;
pub mod attachments;
pub mod auth;
pub mod client_address;
pub mod codes;
pub mod commands;
pub mod devices;
pub mod documents;
pub mod events;
pub mod groups;
pub mod health;
pub mod ids;
pub mod jobs;
pub mod lifecycle;
pub mod limits;
pub mod login_limiter;
pub mod messages;
pub mod passwords;
pub mod places;
pub mod preferences;
pub mod recovery;
pub mod references;
pub mod schedule;
pub mod sessions;
pub mod setup;
pub mod squawks;
pub mod startup;
pub mod sync;
pub mod user_admin;
