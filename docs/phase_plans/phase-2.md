# Phase 2: Data model & server storage (plan)

*Agreed 2026-10-29 · Roadmap: `docs/plan.md` §9 Phase 2 · Open questions: none (decisions #028–#041)*

**Goal:** the household can really use the app, online, from the web UI on the NAS (phones and computers) and the macOS desktop app. Vehicles, meters, fuel-ups, readings, maintenance, repairs, squawks, photos, and documents are stored in the server's SQLite database, every change goes through a command, and the data survives restarts, updates, and a bad day (integrity checks and nightly snapshots).

**Not in Phase 2:** offline use and sync (Phase 3), what's due and fuel economy statistics (Phase 5), the mobile apps (post-MVP, #041).

**Exit criteria (proposed):**
- On the NAS, signed in as real users: add the household's vehicles, then log a fuel-up, a reading, a maintenance event with tasks and a squawk sign-off, and a repair, each with a location and photos, from a phone browser and the desktop app.
- Wrong roles are refused (a Driver can't log maintenance), finalized events can only be amended, and a wrong password is rejected cleanly.
- After a container restart and an image update, everything is still there; the nightly snapshot exists and passes its integrity check.

**How this file works:** the same as `phase-1.md` (decision #021).
- One branch and **one PR into `dev` per milestone** (`milestone/p2-mN-<slug>`), merged with a merge commit.
- Each **step** is one Conventional Commit. Steps are planned in detail, and approved, when their milestone starts.
- Screens get **SVG layout options** in `docs/ui-sketches/` before they're built (#040).
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | What it delivers | You can check on the NAS | Likely new dependencies (approved per step) |
|---|---|---|---|---|---|
| 1 | **Database foundation** | ✅ | SQLite master database in `data/`; migration framework (numbered, forward-only) with a fixture test; database ID; startup integrity check; HLC in `domain`; the **command pipeline** (validate in `domain`, apply in `server`, HLC last-write-wins, soft deletes); nightly `VACUUM INTO` snapshot, keep 7, configurable folder (#035), run on a nightly timer (whether that becomes one general job system is idea 9, asked first) | The database and a snapshot appear in `data/`; a restart keeps them; a corrupted copy is detected at startup | `sqlx` (SQLite), `uuid` (v7, #042) |
| 2 | **Users, login & roles** | 🔨 | Users, argon2 passwords, session cookies, rate-limited login, first-Admin setup code (#028); **retires the test credential** (#024); per-asset roles and active mode with the Admin-mode confirmation; desktop sign-in; settings records for server defaults and per-user preferences (#029); locale formatting of dates, numbers, and units | Create the Admin with the setup code; add a second user; sign in on web and desktop; a wrong password and a Driver doing Admin work are refused | `argon2`, a rate limiter (e.g. `tower_governor`) |
| 3 | **Message center** | ⬜ | Inbox with severity, read/dismiss, one action per message, filters (#040); system messages from earlier milestones (snapshot failed, integrity problem, "app update required") | A forced snapshot failure shows an Admin message; mark read and dismiss work | none expected |
| 4 | **Assets, profiles & meters** | ⬜ | The six profiles as seed data (#030); assets with typed identifiers, lifecycle status, parent field, archive; meters (distance/duration); custom field definitions including quantity types (#030); **Garage** screen (urgency order, My order) and **vehicle detail** shell with tabs (#040) | Add the real vehicles with their meters; the Garage cards show them | `ron` (profile seed files) |
| 5 | **Location** | ⬜ | Coordinates (degrees × 10⁷, accuracy, fix time), required city/state/country; browser location; saved places; the offline geocoder with cities1000, Natural Earth 1:10m, and the nearest-boundary fallback (#038); geocoder data file and attribution | From a phone browser, a test location resolves to the right city and state, including a coastal point; saved places work on desktop | a geocoder crate or our own lookup (chosen in the step plan) |
| 6 | **Events: fuel-ups & readings** | ⬜ | The event spine (UTC + time zone, location, readings, notes, multi-day dates, performed-by vs. logged-by); drafts, **Finalize now** with confirmation, automatic finalizing after 24 h by the server, amendments (#034); monotonic readings and required readings per event type; fuel-ups with highway/towing/hauling %, towed vehicles, vehicle-reported fuel economy, trip fields (#030, #033); **+ Log** and quick logging (#040); Timeline tab | Log fuel-ups and readings from the phone; a lower odometer is refused; an event locks after 24 h and can then only be amended | none expected |
| 7 | **Maintenance, repairs & squawks** | ⬜ | Maintenance and repair events with tasks, labor, parts and labor cost (#032); complaint and cause; **Finish** with checks and confirmation; squawks with sign-off and required disposition; requirements with interval rules (meter, calendar with end-of-month, fixed date) and service bundles as data, "Copy schedule from another asset" (#031); forgotten-job reminder after 14 days | Log an oil change with two tasks, sign off a squawk, log a repair with complaint and cause; Finish refuses while a squawk is undispositioned | none expected |
| 8 | **Photos & documents** | ⬜ | Photo upload with the Small/Medium/Large picker, thumbnails, metadata fields, gated originals (#036); PDF upload with page count and the pdf.js viewer (#037); attachments on events, squawks, and assets; **official asset documents** (title, registration, insurance) with renewal history; Documents tab | Take a Large damage photo and a Small meter photo on the phone; open a PDF on web and desktop | `image` (+ HEIC support), pdf.js (bundled asset) |
| 9 | **Asset lifecycle** | ⬜ | Acquisition and disposal records with attachments; status changes; send-out / return tracking; archive instead of delete; aircraft-ready groundwork that isn't already in place (reference documents, one-time and recurring requirements) | Send a vehicle out for service and return it; archive one and find it again | none expected |

**Releases:** **one release, `v0.2.0`, at the end of Phase 2** (`docs/releasing.md`). Test builds along the way use the existing image tags (`:milestone-<name>`, `:dev`, `:sha-<commit>`; decision #026), not releases.

---

## Why this order

- **1 before everything:** every later milestone stores data through the command pipeline and migrations, so they come first, with the safety net (integrity check, snapshots) from day one.
- **2 next:** real users and roles replace the test credential before any real data is entered, so nothing is ever stored without an owner and a permission check.
- **3 early and small:** later milestones send messages (snapshot failures, forgotten jobs), so the inbox exists before they need it.
- **5 before 6:** every event requires a location, so locations exist before the first event.
- **6 before 7:** fuel-ups and readings are the simplest events and prove the spine, finalizing, and amendments; maintenance builds on them.
- **8 and 9 last:** attachments and lifecycle hang off assets and events that already exist.

## To decide when each milestone is planned

- **ID format: decided,** UUIDv7 (#042). Human-friendly record numbers are post-MVP.
- **Other ideas that touch Phase 2** (`docs/ideas-to-discuss.md`): field-level commands (idea 1, milestone 1), content-addressed blob storage (idea 3, milestone 8), groups (idea 6), notes format (idea 8, milestone 6), and the background job system (idea 9, milestone 1). Each is raised with you before the milestone it affects, as AGENTS.md requires.

---

## Step details

*Detailed plans are added here as each milestone is planned and approved. Steps are numbered per milestone (1.0, 1a, …).*

### Milestone 1 plan: database foundation
*Approved 2026-10-29 (including `sqlx`, `uuid`, and dev-only `proptest`) · Branch: `milestone/p2-m1-database` · Milestone PR: **#9, merged 2026-10-30** (merge commit, steps 1.0–1i) · Decisions #004, #005, #035, #042, #043, #044*

| Step | Status | Commit title | Contents | New dependencies (approved) |
|---|---|---|---|---|
| 1.0 | ✅ | `docs: Phase 2 milestone 1 plan` | This plan; the phase plan's status | none |
| 1a | ✅ | `feat(domain): hybrid logical clock` | `Hlc` (48-bit ms + 16-bit counter in a `u64`; device ID as tiebreak); `HlcClock` with a passed-in `Clock`: `tick` for local edits, `receive` for incoming stamps; the "too far in the future" check (default 5 minutes). Helpers in `hlc_utils.rs` | dev: `proptest` (property tests, AGENTS.md §7) |
| 1b | ✅ | `feat(domain): UUIDv7 record IDs` | A typed `Id<T>` (`AssetId`, `EventId`, … as aliases later), generated from a passed-in clock and random bytes (domain never reads either directly); text and 16-byte forms; `DeviceId`, `DatabaseId` | `uuid` (already in `Cargo.lock`; no default features, `serde`) |
| 1c | ✅ | `feat(storage): SQLite database and migrations` | Open or create `data/vehiclemanager.sqlite` (name from `APP_ID`): WAL, busy timeout, foreign keys on, `STRICT` tables; embedded numbered migrations (`sqlx::migrate!`), each in a transaction; a `VACUUM INTO` backup before migrating; refuse a database with a newer schema. `0001_init`: `database_meta` (database ID, schema info, sequence counter, server device ID), `applied_commands`, `field_clocks`, `tombstones`, `jobs`. Migration fixture test harness (first real fixture saved at `v0.2.0`) | `sqlx` (`sqlite`, `runtime-tokio`, `migrate`; bundled SQLite) |
| 1d | ✅ | `feat(server): startup integrity check and health` | `PRAGMA integrity_check` + `foreign_key_check` at startup; on failure the server keeps serving pages but refuses every command and reports itself unhealthy. The handshake gains a `healthy` field (additive, so API version stays 1); the Connection panel shows it | none |
| 1e | ✅ | `feat(domain,server): command pipeline` | `CommandEnvelope` (command ID, device ID, HLC, format version, operation; actor and active mode reserved for milestone 2); field-level updates (#043): only changed fields, each applied if newer than its `field_clocks` entry; creates; deletes as tombstones; idempotency via `applied_commands`; a sequence number per accepted change; the future-HLC guard; one transaction per command. Proven with a test-only record type; the first real records arrive in milestone 2 | none |
| 1f | ✅ | `feat(server): job scheduler and nightly snapshot` | The scheduler (#044) on tokio timers, with `jobs` rows (last run, result, next run). Jobs: **nightly snapshot** (`VACUUM INTO` `<snapshot dir>/<UTC time>.sqlite`, integrity-checked, keep 7; `VEHICLEMANAGER_SNAPSHOT_DIR`, default `data/snapshots`; skipped and reported if unavailable, #035) and an **hourly `quick_check`**. Snapshot time: 03:00 in the NAS's time zone (`TZ` in `.env`) | none (`tokio` `time` feature; already in `Cargo.lock`) |
| 1g | ✅ | `build: database and snapshots in the NAS deployment` | `.env.example` gains `TZ` and the optional snapshot folder; `docs/deploying-nas.md`: what's in `data/`, how to read snapshot results, and how to restore a snapshot | none |
| 1h | ✅ | `docs: database foundation test plan` | `docs/test-plans/database-foundation.md` | none |
| 1i | ✅ | `docs: record the database foundation test run` | NAS test passed (steps 1–11); idea 11 (NAS maintenance tool) noted | none |

**Written so far**
- **1a:** `domain::hlc` (`Hlc`, `HlcClock` with `tick`/`receive`, full counter rolls into the next millisecond, wall clocks before 1970 never fail) and `domain::hlc_utils` (`check_not_too_far_ahead`, `MAX_AHEAD_MILLIS` = 5 minutes). Property tests: stamps strictly increase over random clock jumps and incoming stamps; packing round-trips. `Clock` is now implemented for `&T`. Dev-only `proptest` 1.12.
  - **Change from the plan:** the device-ID tiebreak moved to 1b, where `DeviceId` exists.
- **1b:** `domain::id`: `Id<T>` (always `UUIDv7`; text, 16-byte, and JSON forms; `created_at`; `cast`), with `DeviceId` and `DatabaseId`; refuses other UUID versions. `hlc::Stamp` (HLC + device ID, ordered HLC first) is the last-write-wins key. `test_support::test_id(n)` makes repeatable IDs. `uuid` 1.28 with no features (it was already in `Cargo.lock`; the lockfile gains one line).

- **1c:** `storage::Database::open`: creates or opens `<data dir>/vehiclemanager.sqlite` (WAL, `NORMAL` sync, 5 s busy timeout, foreign keys); refuses a newer schema without touching it; backs an existing database up to `<data dir>/backups/pre-migration-<UTC>-v<N>.sqlite` before migrating; runs the embedded migrations; records the database and server device IDs on first open. `0001_init.sql`: `database_meta`, `applied_commands`, `field_clocks`, `tombstones`, `jobs` (all `STRICT`; unreleased until `v0.2.0`, so it may still change). `storage::convert` for ID and HLC columns. `domain::time_utils::format_utc_file_stamp` for file names. The migration fixture test and `tests/fixtures/README.md` (first fixture at `v0.2.0`; `.gitignore` lets fixtures in).
  - **`sqlx` 0.9.0 with `sqlite-bundled`,** not `sqlite`: the full `sqlite` feature also enables loading SQLite extensions, which the server should never do.
  - **Complexity limit raised to 30** (`clippy.toml`, agreed with the human): sqlx turns on tracing's `log` feature, and each tracing macro then adds about 2 to clippy's cognitive-complexity score, so ordinary functions with two log calls failed at 15.
  - **Change from the plan:** opening the database at server startup moves to 1d, since it needs random bytes for new IDs (see 1d).

- **1d:** `storage`: `Database::integrity_check` (`integrity_check` + `foreign_key_check`) and `quick_check`. `server::startup::start` opens the database once per process (new IDs from `server::ids::new_id`, OS randomness via `getrandom` 0.4, approved), runs the integrity check, and sets `server::health` (`Starting`, `Healthy`, or `Unhealthy` with `DatabaseUnavailable` / `IntegrityCheckFailed`). The server never stops over it. `Handshake` gains `healthy` (defaults to true when missing, so API version stays 1); the Connection panel gets a **Server health** row and message. The server's data folder is `VEHICLEMANAGER_DATA_DIR`, or `dev-data/` (git-ignored) for local `dx serve`.
  - The real clock is passed in from the app (`app::logging::SystemClock`), not duplicated in `server`.
  - Tests include a deliberately corrupted file and a broken foreign key. Smoke-tested by running the server binary: the handshake reports `"healthy":true`, and a restart reopens the same database ID.

- **1e:** `domain::command`: `Command<R>` (command ID, `Stamp`, format version, `Change`), `Change` (create with all fields, update with only changed fields, delete), and the `Record` / `Patch` traits each record type implements; JSON leaves unchanged fields out. `storage::commands`: the bookkeeping queries and the `RecordWriter` trait each record's table implements. `server::commands::apply`: refuses (unhealthy, unknown format, clock more than 5 minutes ahead) before writing anything; otherwise one `BEGIN IMMEDIATE` transaction for the retry check, the server HLC, per-field last-write-wins, the record write, field stamps, tombstone, sequence number, and the applied record. Proven with a test-only note record: 11 scenario tests (including the offline-phone merge, equal-stamp tiebreak, delete, rollback on a failed write) and a property test that any order of edits, each sent twice, reaches the same state.
  - **Rule chosen (decision #043 didn't spell it out): a delete always wins.** Once a record is deleted, no create or update changes it again, even one with a newer stamp. #043 only says changes older than the tombstone are ignored. Undeleting, if ever wanted, will be its own command.
  - `applied_commands.sequence` may be empty, for commands that changed nothing (still recorded, so retries do nothing). `0001_init.sql` is unreleased, so it was edited rather than given a second migration.
  - Keeping unknown fields (#043) matters on devices, which get local storage in Phase 3; the server is always the newest version, so it knows every field.
  - Commands aren't reachable over the API yet: milestone 2 adds the first real records and the server functions that send them, with the actor and active mode.

- **1f:** `server::jobs`: one scheduler task (tokio timers) running `Job::NightlySnapshot` and `Job::QuickCheck`; each run is logged and recorded in `jobs` (last run, result, message, next run). A new database's jobs are scheduled, not run at once; a job missed while the server was off runs once at the next start. The snapshot writes `snapshot-<UTC>.sqlite` with `VACUUM INTO`, opens it read-only and integrity-checks it, then keeps the newest 7 (`storage::snapshot`). The default folder `data/snapshots` is created if missing; a folder set with `VEHICLEMANAGER_SNAPSHOT_DIR` is **never created**, so an unmounted USB disk makes the snapshot "skipped" instead of writing to the wrong disk. A failed quick check makes the server unhealthy until it's restarted. `server::startup::ServerPaths` holds the data and snapshot folders.
  - **Change from the plan: the snapshot runs at 07:00 UTC** (03:00 US Eastern daylight time, 02:00 standard), not at 03:00 in the NAS's `TZ`. Local-time scheduling needs a time-zone library, which milestone 6 needs anyway (events store their IANA zone) and which should be chosen once, there.
  - Smoke-tested with the server binary: both jobs are scheduled on a new database.
  - **Note for 1h:** the container's `/tmp` is a 16 MB tmpfs; if a large `VACUUM INTO` ever needs more temporary space, the NAS test will show it.

- **1g:** `compose.yaml` passes `VEHICLEMANAGER_SNAPSHOT_DIR` (empty by default) and has a commented-out USB mount; `.env.example` lists it, empty; the server treats an empty value as unset (Compose passes unfilled settings as empty). `docs/deploying-nas.md`: what's in `data/`; a "Database, backups, and snapshots" section (the checks, schedule, where things are, what an unhealthy server does); restoring a snapshot step by step; a rollback warning (an older build refuses a database a newer one upgraded); two troubleshooting entries. `deploy_config` test for the new variable.
  - **Change from the plan:** no `TZ` setting, since the snapshot runs at a fixed UTC time (see 1f). An existing NAS `.env` needs no change: the new setting is optional.

- **1h:** `docs/test-plans/database-foundation.md`: 12 steps over two sessions (one overnight, for the 07:00 UTC snapshot): creation and owner, health in Diagnostics, same database ID after a restart and an update, no credential in the log, the snapshot and hourly check, a restore drill, a damaged-database drill and recovery, and (optional) a missing USB snapshot folder being skipped.
  - The damaged-database drill replaces the file with garbage (the "can't open" path) rather than damaging pages: in WAL mode a new database's data may still be in the `-wal` file, so page damage on the NAS wouldn't reliably show. Page-level corruption is covered by an automated test.
- **Passed on the NAS, 2026-10-30:** steps 1–11, with the `milestone-p2-m1-database` image. Step 12 (an unmounted USB snapshot folder; optional, another overnight run) was skipped; it's covered by the automated test `a_missing_snapshot_folder_that_was_set_explicitly_is_skipped_not_created`.
- **Raised during the run:** a maintenance tool on the NAS for restores, rollbacks, and status. Noted as idea 11 in `docs/ideas-to-discuss.md`, not scheduled.

**Choices in this plan (say if you'd rather otherwise):**
- **Runtime-checked queries, not sqlx's compile-time macros.** The macros need a database or a committed `.sqlx` cache at build time (and `sqlx-cli`); every query is instead covered by tests against a fresh temporary database (AGENTS.md §7).
- **Bundled SQLite** (compiled into the binary), so the server and later the desktop app use the same SQLite version everywhere, whatever the OS ships.
- **HLC in SQLite:** stored as `INTEGER` (signed 64-bit). The packed value stays positive until the year ~6400, and a check rejects anything else.
- **IDs in SQLite:** 16-byte `BLOB`s (half the size of text, still sorted by time with UUIDv7).
- **No real records yet:** milestone 1 builds and proves the engine with a test-only record; users and settings in milestone 2 are its first real users. That keeps this milestone about the foundation.

**Manual test plan (1h), in short:** deploy on the NAS; the database and its ID appear and survive a restart; an update keeps them; a deliberately corrupted copy makes the server report unhealthy and refuse commands; a snapshot appears at the scheduled time (or on a short test interval) and passes its check; an unwritable snapshot folder is skipped and logged.

### Milestone 2 plan: users, login & roles
*Approved 2026-10-30 (including `argon2` and `sha2`, and every choice below) · Branch: `milestone/p2-m2-users` · Milestone PR: not opened yet · Decisions #019, #024, #028, #029, #043, #045*

**Goal:** real people sign in. The test credential (#024) is retired; every API request is made by a signed-in user, in an active mode, with roles that the server enforces. The first Admin is created with a one-time setup code; the Admin adds the rest of the household. Users and settings are the first real records that go through the command pipeline.

| Step | Status | Commit title | Contents | New dependencies |
|---|---|---|---|---|
| 2.0 | ☑️ | `docs: Phase 2 milestone 2 plan` | This plan; decision #045; milestone 1 marked merged | none |
| 2a | ☑️ | `feat(domain): users, roles, and permissions` | `User` (username, display name, disabled), `Role`, `RoleGrant` with `GrantTarget::{AllAssets, Asset(id)}` (#045), `ActiveMode`; a pure `permits(grants, mode, action, asset)` check, test-first, with property tests (more grants never remove a permission; Driver mode never gets Maintainer actions). Username and password rules | none |
| 2b | ☑️ | `feat(storage): users, grants, sessions, and settings tables` | Migration `0002_users.sql`; `RecordWriter`s for users, grants, and settings; session queries | none |
| 2c | ☑️ | `feat(server): passwords, sessions, and the signed-in user` | argon2id password hashes; sessions (random 32-byte token, stored only as its SHA-256); the auth layer resolves the session and attaches the user and active mode to the request (`FullstackContext::extract`); web: `HttpOnly; Secure; SameSite=Strict` cookie; desktop: the same token as `Authorization: Bearer`; login, logout, and "who am I" server functions; login rate limiting; an explicit, tested list of the few `/api/` paths that work without a session (handshake, login, setup) | `argon2`; `sha2` (already in `Cargo.lock`) (approved) |
| 2d | ☑️ | `feat(server): first-Admin setup code` | With no users, a one-time code is generated at startup, kept in memory only, and written to the log; a setup server function takes the code, username, and password, and creates the Admin (Admin on all vehicles) through the command pipeline; the code stops working once any user exists | none |
| 2e | ☑️ | `feat!: retire the test credential` | `VEHICLEMANAGER_TEST_CREDENTIAL` removed from the server, `compose.yaml`, `.env.example`, the docs, and the Connection panel (which now checks the session) | none |
| 2f | ☑️ | `feat(app): sign-in, first-Admin setup, and mode switching` | **SVG layout options first** (#040). Sign-in and setup screens; sign out; the mode switcher (starts in Driver; Admin needs the password again and shows a banner; drops back after 15 minutes without activity); web and desktop | none |
| 2g | ⬜ | `feat(app): user management` | **SVG options first.** Admin mode only: list users, add one (with a temporary password they change at first sign-in), grant and remove roles (on all vehicles for now; per-vehicle grants get their picker in milestone 4, when vehicles exist), disable a user, reset a password | none |
| 2h | ⬜ | `feat(app): display preferences` | **SVG options first.** Settings records: server defaults (Admin) and per-user overrides (#029), last-write-wins per setting; a Settings screen for units, currency, language (English only so far), and date format. **Formatting** with these preferences arrives where values first appear: units in milestone 4, dates in milestone 6 | none |
| 2i | ⬜ | `docs: users and login test plan` | `docs/test-plans/users-and-login.md` | none |

**Written so far**
- **2a:** `domain::user` (`User`, `Username` with its rules, `check_new_password`, `check_display_name`, `UserPatch`; the password hash is deliberately not part of `User`) and `domain::access` (`Role`, `GrantTarget`, `RoleGrant` and its patch, `ActiveMode`, `AssetAction`, `AdminAction`, `role_on`, `available_modes`, `permits_on_asset`, `permits_admin`, `leaves_an_admin`). `domain::id::Asset` is a marker until the asset record arrives in milestone 4. 7 translated messages. Unit tests plus three property tests (more grants never remove a permission; a higher mode never removes one; Driver mode never does Maintainer work).
  - **Added from the plan's §6:** household-wide actions (users, adding assets, server settings) need the **Admin role on all assets** and Admin mode, so the Admin of one vehicle isn't the household's Admin; and `leaves_an_admin` backs the rule that there's always at least one enabled Admin.
  - **Grants are created and deleted, never edited:** changing someone's role is a delete plus a create, one clear entry each in the history.
  - Viewing isn't limited by mode: every mode sees what the user's grants cover.

- **2b:** migration `0002_users.sql`: `users` (username unique among users not deleted), `user_passwords`, `role_grants` (a `CHECK` ties `target_asset` to the target kind), `sessions` (by token hash only). `storage::users`: `UserWriter` and `RoleGrantWriter` for the command pipeline; user, grant, password-hash, and session queries. Tests against a real database, plus one through the command pipeline (create a user and an Admin grant, then rename).
  - **Changes from the plan:** the **settings table moves to 2h**, where the settings record is designed. **Password hashes get their own table** (`user_passwords`), apart from the user record, so a hash can never be part of anything synced or sent to a device.
  - `role_grants.target_asset` gets its foreign key to `assets` when that table exists (milestone 4, by rebuilding the table).

- **2c:** `server::passwords` (argon2id, verify, `needs_rehash` so old-parameter hashes are upgraded at the next sign-in); `server::login_limiter` (5 failures in 15 minutes per username and per client); `server::sessions` (64-hex tokens stored as SHA-256; sign in, resolve, switch mode, sign out; 30 idle days; Driver at sign-in; Admin needs the password and lapses after 15 idle minutes; disabled users and revoked roles take effect at once; an unknown username takes as long as a wrong password). The auth layer now resolves the session (cookie `vehiclemanager_session` or `Authorization: Bearer`) and attaches `SignedIn` to the request; `PUBLIC_API_PATHS` = handshake, sign-in, session check (a test checks each is a real server function). Server functions: `sign_in` (`want_token` for the desktop app, so web scripts never see the token), `session`, `sign_out`, `switch_mode`. New types in `domain::api`: `SessionInfo`, `SignInOutcome`, `SwitchModeOutcome`. The test credential is still accepted until 2e.
  - **Bug found in review and fixed:** switching into Admin mode now counts as activity. Before, a session idle for 15+ minutes that then entered Admin mode would drop straight back to Driver on the next request. Test: `entering_admin_mode_after_a_long_idle_stretch_doesnt_drop_straight_back`.
  - **Test fix:** the 1e convergence property test could generate two edits from one device with identical stamps, which a real device can't produce (its HLC strictly increases), so arrival order decided a tie. Its generator now makes each stamp unique; the engine is unchanged. Passed a long run with many more cases.
  - **Moved to 2d:** commands the server makes on a user's behalf (stamped by the server, with the user and active mode recorded for the audit trail), and changing a password. 2d needs both for the setup code anyway.
  - **The client key** for the login limit is the first `X-Forwarded-For` address, which `tailscale serve` sets; the server listens only on loopback, so the header can't come from anywhere else.
  - **Local web development:** the cookie is `Secure`; Chrome and Firefox accept that on `localhost`, so use one of them with `dx serve`.

- **2d:** `server::setup` (code from 32 unambiguous characters, logged at startup while there are no users, memory only; `create_first_admin` takes a `SetupRequest`, creates the user and an Admin-on-all-assets grant through the command pipeline, one at a time, then closes setup). The **audit trail**: `domain::access::Actor` (user and mode), recorded on every applied command (`applied_commands.user_id`, `active_mode`; empty for the server's own changes). `server::commands::issue` makes a command on the server, stamped with the server's next HLC (`storage::commands::next_server_hlc`, its own immediate transaction) and the acting device. `server::sessions::change_password` (checks the current password, clears "must change password" through a command, signs out the user's other sessions). Server functions: `setup` (public; wrong codes count toward the same limit, by client; signs the new Admin straight in), `change_password`; `session` now returns `SessionState` with `setup_needed`. `/api/v1/setup` added to `PUBLIC_API_PATHS`.
  - **Health is passed in** to setup and password changes, as `apply` already does, so tests control it and an unhealthy server writes nothing (tested).
  - The session tests now create users through the command pipeline, as the app does. (Writing a user straight into its table left it without field stamps, so a later update looked like a create.)
  - Smoke-tested with the server binary and `curl`: wrong code refused; right code creates the Admin and signs them in (Driver mode, `HttpOnly` cookie); the code is then refused; `ping` needs a session; Admin mode needs the right password; sign-out ends the session; the password never appears in the log.

- **2e:** the test credential (#024) is gone: from `server::auth` (sessions only now: `AuthConfig::new(SessionSource)`, or `refuse_all`), `compose.yaml`, `.env.example`, `docs/deploying-nas.md` (old step 3 removed; new step 6 "Create the first Admin" with the setup code; an upgrade note to delete the old `.env` line), the Connection panel, and its messages. The panel's checks are now: **handshake** (public: API version, server health) and **ping** (needs a session: the "Signed in" check, and the round trip). A protected path with no way to check sessions (database not open) is refused with `auth-not-configured` (fail closed). `deploy_config` now checks the credential is gone. #024 is marked retired.
  - **The refusal codes keep their names** (`missing-credential`, `invalid-credential`, `auth-not-configured`): they're the API contract. Their meanings and the text people see changed: "You're not signed in.", "Your session has ended. Sign in again.", "The server can't check sign-ins right now."
  - Smoke-tested: the old credential header is refused on a protected path, and the startup log no longer warns about a missing credential.

- **2f layouts (chosen 2026-10-30, #040):**
  - **Sign-in and first-Admin setup:** `docs/ui-sketches/p2m2-sign-in-chosen.svg`: option A's centered card everywhere, with option B's **two-step setup** (step 1 asks only for the setup code and checks it; step 2 asks for the account). The options considered: `p2m2-sign-in-options.svg`.
  - **Mode switching, the Admin indicator, and signing out:** option A of `docs/ui-sketches/p2m2-mode-options.svg`: a mode chip (`Driver ▾`) and an account chip (`Stuart ▾`: change password, sign out) in the top bar; Admin mode adds an orange banner with **Leave**, an orange chip, and an orange sidebar edge; the same password dialog to enter Admin mode.
  - **Needed for the two-step setup:** a public `check_setup_code` server function (rate-limited like wrong codes), added to `PUBLIC_API_PATHS`. Step 2 still sends the code, and the server checks it again.

- **2f:** built to the chosen layouts. `app::client` sets the headers every server call carries (API version; the desktop app's session token, in memory only), now used everywhere. `ui::auth`: `AuthState` in context and `AuthGate` around the router (loading → setup → sign-in → forced password change → the app; the session is checked only after mounting, never during server rendering, and again on every navigation). `ui::screens::account`: `SignIn`, two-step `Setup` (step 1 calls the new public `check_setup_code`; step 2 sends the code again; ← Back keeps what was typed), `ChangePasswordForm`, `ForcedPasswordChange`. `ui::account` (shell): `ModeChip` (the modes the user's roles allow; Admin opens a password dialog; Viewer-only shows "Read-only"), `AccountChip` (name, change password in a dialog, sign out), `AdminBanner` (with Leave), and an orange sidebar edge in Admin mode. 55 new messages. Styles: auth card, fields (48 px), errors, buttons, chips, menus, dialog, Admin colours in both themes.
  - **Checked in a real browser:** built with `dx build --platform web` and driven with headless Chrome: setup step 1 (wrong code, then the right one, typed in lowercase), step 2, signed in to the Garage in Driver mode, the mode menu, a wrong Admin password, Admin mode (banner, orange chip and sidebar edge), the account menu, the phone layout, Leave, sign out, and a wrong sign-in password. All matched the chosen sketches.
  - **Not checked here:** the desktop app (it needs a window); it's in the 2i test plan.
  - **Admin mode's 15 idle minutes:** the server enforces them; the app notices at the next navigation or action, so the banner can outlive Admin mode on a screen left open. Every Admin action is refused by the server once it lapses.

**Choices in this plan (all approved 2026-10-30):**
- **Passwords:** at least 12 characters, no other composition rules (current NIST guidance); usernames are lowercase letters, digits, `.`, `-`, `_`, 3–32 characters, compared case-insensitively. argon2id with the library's recommended parameters, re-hashed if they change.
- **Sessions:** a web session lasts until **30 days without use** (each use extends it); signing out ends it. A disabled user's sessions end at once.
- **Desktop:** the session token is kept **in memory only**, like the test credential was (#027), so the desktop app asks you to sign in each time it starts. Saving it in the OS keychain could come later.
- **Login rate limit:** after **5 failed attempts in 15 minutes** for a username, or from one device, further attempts wait out the rest of the 15 minutes. In memory, no new dependency; it resets when the server restarts.
- **Admin mode:** entering it asks for the password again; it ends after **15 minutes** without activity, or when you switch back.
- **The setup code:** 16 characters from an unambiguous alphabet (no `0`/`O`, `1`/`I`), shown in groups of 4, e.g. `K7QM-4XRT-9PWD-H2CN`; a new one at each start until the Admin exists. Find it with `sudo docker logs vehiclemanager 2>&1 | grep -i "setup code"`.
- **Breaking change (2e):** removing the test credential is marked `feat!`. Anything that used it (the Connection panel, `curl` checks) moves to signing in.
- **Not in this milestone:** passkeys (#028), per-device tokens and pairing (Phase 3), groups (#045), and the message center (milestone 3).

**Manual test plan (2i), in short:** on the NAS: find the setup code in the log and create the Admin; the code then stops working; add a second user with a temporary password and sign in as them (they must change it); a Driver can't open user management; Admin mode asks for the password and drops back after 15 minutes; 6 wrong passwords hit the limit; sign in on the desktop app; sign out ends the session; a disabled user is signed out; preferences save and survive a restart; nothing secret in the log.

