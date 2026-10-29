//! The translation layer: every user-facing string comes from here.
//!
//! Messages are written in [Fluent](https://projectfluent.org/) files under
//! `crates/domain/locales/<language>/`, embedded at compile time (no file I/O, so
//! this works on WASM). Code refers to them through typed [`MessageId`] constants,
//! and a test checks the constants and the English file match exactly.
//!
//! **Adding a message:** add it to `locales/en-US/app.ftl` *and* add one line to
//! `define_messages!` in `i18n/message_id.rs`.
//!
//! Only English exists for now. Choosing a language per user or device, and locale
//! formatting of dates, numbers, and units, come in Phase 2 (decisions.md #018, #022).
//!
//! # Examples
//!
//! ```
//! use domain::i18n::{messages, MessageArg, Translator};
//!
//! let english = Translator::english();
//! assert_eq!(english.tr(messages::NAV_GARAGE), "Garage");
//! assert_eq!(
//!     english.tr_with(messages::OPEN_SQUAWK_COUNT, &[("count", MessageArg::from(2))]),
//!     "2 open squawks"
//! );
//! ```

mod message_id;
mod translator;

pub use message_id::{MessageId, messages};
pub use translator::{CatalogError, MessageArg, Translator};
