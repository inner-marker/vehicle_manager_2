//! Server library: the sync engine (apply commands, serve pulls), auth and
//! permission enforcement, integrity checks, nightly snapshots, and admin
//! operations.
//!
//! Used by the NAS server and, later, desktop host mode, so both share the
//! same code. Native only: the web (WASM) build must never depend on this crate.

pub mod auth;
