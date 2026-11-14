# Migration fixtures

One SQLite database per release, made by that release, named `v<version>.sqlite` (e.g. `v0.2.0.sqlite`). `tests/migration_fixtures.rs` migrates every one to the latest schema and checks the data survived.

- **Add one at each release** (from `v0.2.0`, the first with a database), as part of release preparation (`docs/releasing.md`). `crates/server/tests/make_fixture.rs` makes one through the server's own pipeline: `cargo test -p server --test make_fixture -- --ignored`. For the next release, copy its test to write `v<version>.sqlite`, adding records for what that release brings; it refuses to overwrite a fixture that's there.
- **Never edit or delete one.** Like released migrations, they're kept forever.
  - **The one exception (2026-11-14):** in `v0.2.0.sqlite`, a person's real first name was used for the test Admin. It was renamed to `jordan` (username and display name), and the file was vacuumed so no old copy remained. Nothing else in it changed: its schema, its other records, and its settings (`user_version`, journal mode, page size) are as `v0.2.0` made them.
- **Keep them small and free of personal data** (no real names, places, or equipment): a database with a few made-up records, not a copy of the NAS's real database.
