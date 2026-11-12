//! Pure domain logic shared by every target: domain types, validation,
//! sync commands and upcasters, and the hybrid logical clock.
//!
//! This crate performs **no I/O** and contains no database code, so it
//! compiles to WebAssembly for the web UI. It never reads the real clock,
//! random numbers, or location directly; callers pass those in.

pub mod access;
pub mod api;
pub mod asset;
pub mod asset_form;
pub mod attachment;
pub mod build_info;
pub mod command;
pub mod custom_field;
pub mod dates;
pub mod device;
pub mod device_signature;
pub mod document;
pub mod event;
pub mod event_check;
pub mod event_form;
pub mod garage;
pub mod group;
pub mod hlc;
pub mod hlc_utils;
pub mod i18n;
pub mod id;
pub mod identity;
pub mod idle;
pub mod inbox;
pub mod label;
pub mod lifecycle;
pub mod location;
pub mod log_area;
pub mod meter;
pub mod notes;
pub mod pin;
pub mod plate;
pub mod preferences;
pub mod profile;
pub mod quantity;
pub mod reference;
pub mod requirement;
pub mod secret;
pub mod server_address;
pub mod squawk;
pub mod sync;
pub mod time;
pub mod time_utils;
pub mod timers;
pub mod user;

#[cfg(any(test, feature = "test-support"))]
pub mod test_support;
