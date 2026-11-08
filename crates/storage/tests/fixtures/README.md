# Migration fixtures

One SQLite database per release, made by that release, named `v<version>.sqlite` (e.g. `v0.2.0.sqlite`). `tests/migration_fixtures.rs` migrates every one to the latest schema and checks the data survived.

- **Add one at each release** (from `v0.2.0`, the first with a database), as part of release preparation (`docs/releasing.md`). `crates/server/tests/make_fixture.rs` makes one through the server's own pipeline: `cargo test -p server --test make_fixture -- --ignored`. For the next release, copy its test to write `v<version>.sqlite`, adding records for what that release brings; it refuses to overwrite a fixture that's there.
- **Never edit or delete one.** Like released migrations, they're kept forever.
- **Keep them small and free of personal data:** a database with a few made-up records, not a copy of the NAS's real database.
