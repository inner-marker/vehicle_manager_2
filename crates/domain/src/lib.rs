//! Pure domain logic shared by every target: domain types, validation,
//! sync commands and upcasters, and the hybrid logical clock.
//!
//! This crate performs **no I/O** and contains no database code, so it
//! compiles to WebAssembly for the web UI. It never reads the real clock,
//! random numbers, or location directly; callers pass those in.

pub mod identity;
pub mod time;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
