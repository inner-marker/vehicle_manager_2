//! Pure domain logic shared by every target: domain types, validation,
//! sync commands and upcasters, and the hybrid logical clock.
//!
//! This crate performs **no I/O** and contains no database code, so it
//! compiles to WebAssembly for the web UI. It never reads the real clock,
//! random numbers, or location directly; callers pass those in.

pub mod access;
pub mod api;
pub mod command;
pub mod hlc;
pub mod hlc_utils;
pub mod i18n;
pub mod id;
pub mod identity;
pub mod log_area;
pub mod preferences;
pub mod server_address;
pub mod time;
pub mod time_utils;
pub mod user;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
