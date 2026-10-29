//! Shared test helpers and data builders for every crate's tests.
//!
//! Compiled only for `domain`'s own tests or with the `test-support` feature,
//! which other crates enable **only** in their `[dev-dependencies]`:
//!
//! ```toml
//! [dev-dependencies]
//! domain = { workspace = true, features = ["test-support"] }
//! ```
//!
//! Reuse and extend these helpers rather than writing one-off setup in a test.
//! Builders for assets, meters, intervals, history, and users are added here as
//! those types arrive (Phase 2).

mod clock;

pub use clock::ManualClock;
