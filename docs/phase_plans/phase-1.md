# Phase 1: Walking skeleton (plan)

*Agreed 2026-10-28 · Roadmap: `docs/plan.md` §9 Phase 1 · Open questions: none*

**Goal:** prove the full pipeline end to end, across separate devices and a separate server, before building real features. The web UI served from the NAS comes first; the desktop app (macOS first, decision #027) is the last milestone.

**Exit criteria:** from separate devices, both the web UI and the desktop app reach the server on the NAS over Tailscale and show the test response. A wrong credential or incompatible version is rejected cleanly. **Met 2026-10-29** (milestone 3 and 4 test runs).

**Status: complete.** All four milestones are in `dev` (PRs #1–#4). Released as `v0.1.0`, the first release, which also tests the release pipeline (`docs/releasing.md`).

**How this file works** (decision #021)
- Each **milestone** has one branch (`milestone/p1-mN-<slug>`) and **one PR into `dev`** when the milestone is finished. That PR is merged with a **merge commit**.
- Each **step** is one Conventional Commit on the milestone branch, made by the human from the commands the agent drafts.
- Before each step starts, the agent writes a detailed plan, waits for approval, then adds it under "Step details" below.
- The status column is kept current: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.
- Steps 0–3b were merged as individual PRs, before decision #021.

**Decisions shaping this phase:** #016 app identity, #018 translation layer in Phase 1, #019 auth before any server function, #020 license policy, #021 one PR per milestone, #022 translation layer, #023 public repository, #024 test credential, #025 amd64 server image, #026 `dev` as default branch, #027 macOS desktop first.

---

## Milestone 1: Local skeleton
*Branch: `milestone/p1-m1-local-skeleton` · Milestone PR: **#1, merged 2026-10-29** (merge commit, steps 3c–8)*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 0 | ✅ | *(initial commit)* Setting up workspace | Four-crate workspace, pinned toolchain, lints, `domain::identity`, docs reorganized, decision log | `dioxus =0.7.10` |
| 1 | ✅ | `docs: record Phase 1 sequencing decisions` | Decisions #017–#019; Phase 1 roadmap and Extensibility row updated; this file; AGENTS.md rule for `docs/phase_plans/` | none |
| 2 | ✅ | `ci: add fast checks, PR-to-main gate, and Dependabot` | **Fast checks:** fmt, clippy, test, Conventional PR title check. **PR-to-`main` gate:** those plus `cargo doc`, `cargo-deny`, Linux/Windows/macOS build matrix, WASM build. `deny.toml`. Dependabot targeting `dev`. Docker and migration-fixture jobs are added once those exist | CI actions only, pinned to commit SHAs |
| 3 | ✅ | `feat(domain): passed-in clock and test support` | `UnixMillis` and the `Clock` trait in `domain::time`; `domain::test_support` (feature `test-support`) with `ManualClock`; builders grow with Phase 2 types | none |
| 3a | ✅ | `chore: allow read-only gh pr commands for agents` | Narrow the `gh pr:*` deny rule in `.claude/settings.json` to the write subcommands (incl. `checkout`, which changes the local branch), so `gh pr list/view/diff/checks/status` work | none |
| 3b | ✅ | `fix(ci): run the PR title check on pull_request` | `pull_request_target` workflows only run from the default branch (`main`), so the title check never ran on PRs 2–3. Switch to `pull_request` (opened, edited, synchronize, reopened). Weekly checks start running once `main` has them (first release). Also: `docs/plan.md` links to phase plans, and AGENTS.md requires that link for new phase plans | none |
| 3c | ✅ | `docs: switch to one PR per milestone` | Decision #021 (supersedes #017); AGENTS.md §8a, `docs/plan.md` §8, and this file updated; fast checks no longer re-run on pushes to `dev` | none |
| 4 | ✅ | `feat: debug log foundation` | Capture layer feeding a ring buffer (5,000 entries) and a size-, count- and age-capped JSON-lines file writer; runtime level control; console output; export renderers. File wiring and crash file arrive with M3/M4 paths. Viewer UI in step 6 | `tracing`, `tracing-subscriber`, `tracing-wasm` (web), `web-time`, `serde`, `serde_json`; dev: `tempfile` (all already in `Cargo.lock`) |
| 5 | ✅ | `feat: translation layer` | `domain::i18n` on `fluent-bundle`: embedded `en-US` catalog, `MessageId` constants with a consistency test, args and plurals, bidi isolation off. Seeded with the navigation labels step 6 needs | `fluent-bundle`, `unic-langid` |
| 6 | ✅ | `feat(app): skeleton UI and navigation` | Placeholder screens (vehicle list, vehicle detail, log event, what's due, basic log viewer); adaptive wide/medium/narrow layout; Light/Dark/System theme, default Dark | `dioxus` `router` feature |
| 8 | ✅ | `ci: release workflow and git-cliff` | `cliff.toml`; tag-triggered release workflow with version/tag guards; CHANGELOG entry check in the PR-to-`main` gate | — |

## Milestone 2: Auth stub, handshake & test server functions
*Branch: `milestone/p1-m2-auth-handshake` · Milestone PR: **#2, merged 2026-10-29** (merge commit, steps 7.0–7)*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 7.0 | ✅ | `docs: record making the repository public` | Decision #023; plan §7/§8 wording; milestone 1 marked merged | none |
| 7 | ✅ | `feat: auth middleware, handshake, and test server functions` | One auth + API-version layer on `/api/*` (credential from an env var, decision #024); `ping` and `handshake` server functions; Diagnostics "Connection" panel with pass/fail | `axum`, `tower`, `subtle` (server); `serde` (domain); dev: `tower`, `http-body-util` (all already in `Cargo.lock`) |

## Milestone 3: Web UI on the NAS
*Branch: `milestone/p1-m3-nas-web` · Milestone PR: **#3, merged 2026-10-29** (merge commit, steps 9a–9g)*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 9a | ✅ | `docs: target linux/amd64 for the first server image` | Decision #025; plan §7 rewritten for the DS1019+; `rust-toolchain.toml` → `x86_64-unknown-linux-gnu`; Phase 2 open question on the snapshot disk; this milestone plan | none |
| 9b | ✅ | `feat(app): write server logs to the data directory` | `VEHICLEMANAGER_DATA_DIR` → `<dir>/logs` with the server file policy; "last crash" file | none |
| 9c | ✅ | `build: server Docker image and compose file` | Multi-stage `Dockerfile`, `.dockerignore`, `deploy/compose.yaml`, `deploy/.env.example`, `docs/deploying-nas.md`; Dependabot watches the base images | Base images `rust:1.95-trixie`, `debian:trixie-slim`; `dioxus-cli` 0.7.10 inside the build stage only |
| 9d | ✅ | `ci: publish server images to GHCR` | `image.yml`: build on push to `dev` and on "Run workflow" → `:dev` + `:sha-<commit>`; release workflow adds `:<version>` + `:latest` | Docker's official actions (`setup-buildx`, `login`, `metadata`, `build-push`), pinned to SHAs |
| 9e | ✅ | `docs: NAS deployment test plan` | `docs/test-plans/p1m3-nas-web.md`: Project setup, `tailscale serve`, checks from another tailnet device | none |
| 9f | ✅ | `fix: name the compose project so docker compose matches the NAS Project` | `name: vehiclemanager` in `deploy/compose.yaml`, so `docker compose` over SSH updates the Container Manager Project's container; test pins it to `APP_ID`; troubleshooting entry | none |
| 9g | ✅ | `docs: record the NAS test run` | Test plan passed; web half of the Phase 1 exit criteria met | none |

## Milestone 4: Desktop over Tailscale (macOS first)
*Branch: `milestone/p1-m4-desktop` · Milestone PR: **#4, merged 2026-10-29** (merge commit, steps 10.0–10d)*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 10.0 | ✅ | `docs: milestone 4 plan` | Milestone 3 marked merged; decision #027 (macOS first, address at launch, credential in memory); plan §9 updated; this milestone plan | none |
| 10a | ✅ | `feat(app): desktop data folder and log files` | Platform data folder from `DATA_DIR_NAME`; desktop log files (`FileLogPolicy::DESKTOP`) and crash file | `dirs` 6.0 (approved; already in `Cargo.lock`) |
| 10b | ✅ | `feat(app): server address setting (desktop)` | Address rules in `domain`; `settings.json` in the data folder; applied before launch; Diagnostics field with "restart to apply" | none |
| 10c | ✅ | `docs: macOS desktop test plan` | `docs/test-plans/p1m4-desktop-macos.md` | none |
| 10d | ✅ | `docs: record the macOS test run` | Test plan passed; Phase 1 exit criteria met; window-size idea noted | none |


---

## Step details

*Detailed plans are added here as each step is planned and approved. Steps 2 and 3 were standalone PRs.*

### PR 2: `ci: add fast checks, PR-to-main gate, and Dependabot`
*Approved 2026-10-28 · Branch: `ci/fast-checks`*

**Toolchain in CI:** no toolchain action. Runners already have `rustup`, which reads `rust-toolchain.toml` (`rustup toolchain install`), so CI uses the exact local compiler, components, and targets. This setup plus the cargo cache lives in one composite action, `.github/actions/setup-rust`, shared by every workflow.

**Actions (pinned to commit SHAs, version in a comment; Dependabot keeps them current)**

| Action | Version |
|---|---|
| `actions/checkout` | v7.0.1 |
| `Swatinem/rust-cache` | v2.9.3 |
| `amannn/action-semantic-pull-request` | v6.1.1 |
| `EmbarkStudios/cargo-deny-action` | v2.1.1 |

**Files**
1. `.github/workflows/fast-checks.yml`: runs on pushes to `dev`, PRs into `dev`, and calls from the main gate. Linux only. Runs fmt, clippy `-D warnings`, and tests. Cancels superseded runs. Permissions: `contents: read`.
2. `.github/workflows/pr-title.yml`: Conventional Commit PR titles (`feat fix docs refactor test chore ci build perf`) for PRs into `dev` and `main`. Uses `pull_request_target` so title edits re-run it; this is safe because it never checks out PR code. *(Split out of fast checks so a title edit doesn't re-run the build.)*
3. `.github/workflows/main-gate.yml`: runs on PRs into `main`.
   - Reuses the fast checks.
   - `cargo doc` with `-D warnings`.
   - `cargo-deny`.
   - `cargo build --workspace` on Linux, Windows, and macOS.
   - WASM check of `domain` and `app`, which **fails if `storage`, `server`, or `sqlx` appear in the web dependency tree**.
4. `.github/workflows/weekly.yml`: runs Mondays and on manual trigger. Runs advisories and tests on both `main` and `dev`.
5. `.github/dependabot.yml`: weekly `cargo` and `github-actions` updates into `dev`.
   - Commit prefixes: `build(deps):` for cargo, `ci(deps):` for actions.
   - Cargo minor and patch updates are grouped into one PR.
6. `deny.toml`:
   - Vulnerabilities and yanked crates fail.
   - Unmaintained crates fail only when they're **direct** dependencies.
   - Duplicate versions only warn.
   - Sources: crates.io only.
   - License allow-list per decision #020.

**Changes from the approved plan, found during implementation**
- **Unmaintained-crate scope:** `cargo-deny` flagged three unmaintained transitive crates inside Dioxus (`paste`, `proc-macro-error`, `fxhash`). We can't fix those, so the check covers direct dependencies only (`unmaintained = "workspace"`). Vulnerabilities still fail at any depth.
- **Lockfile fix:** two real advisories were fixed with `cargo update -p rustls-webpki -p webpki-roots`: RUSTSEC-2026-0381 and RUSTSEC-2026-0382, distrusted emSign certificates. `Cargo.lock` is updated by cargo, not by hand.
- **AGENTS.md:** `cargo deny check` was added to the self-review commands.

**Not in this PR:** Docker image build, migration-fixture tests, CHANGELOG check (PR 8), and the desktop-feature build (needs WebKitGTK; milestone 4).

**First-run checklist (on GitHub, after pushing)**
1. Opening the PR into `dev` runs **Fast checks** (2 jobs) and **PR title**, and all pass.
2. Edit the PR title to something non-conventional (e.g. `add ci`). **PR title** fails. Change it back, and it passes.
3. After the squash-merge, the push to `dev` runs **Fast checks** again.
4. Actions → **Weekly checks** → "Run workflow" runs 4 jobs (advisories and tests × `main`/`dev`).
   - The `dev` jobs should pass.
   - The `main` jobs may fail until `main` has `deny.toml`, because it's added in this PR.
5. Insights → Dependency graph → Dependabot shows both ecosystems. Update PRs target `dev`.
6. The **Main gate** gets its first real run at the first `dev` → `main` PR.

### PR 3: `feat(domain): passed-in clock and test support`
*Approved 2026-10-28 · Branch: `feat/domain-clock`*

**Goal:** establish the rule that `domain` never reads the real clock before any time-dependent logic exists, and give every crate one place for test helpers.

**Changes**
1. `crates/domain/src/time.rs`:
   - **`UnixMillis(i64)`** is a UTC instant in epoch milliseconds (decision #008). It's `Copy` and ordered, with `from_millis`, `as_millis`, and `EPOCH`.
   - **`trait Clock { fn now(&self) -> UnixMillis; }`**.
   - Domain functions take `now: UnixMillis` where they can, and `&impl Clock` when they need to read the time repeatedly.
   - Real-clock implementations live outside `domain` (native `SystemTime`; web `Date.now()`, since `SystemTime::now` panics on WASM). They're added when first needed (PR 4 or 7).
2. `crates/domain/src/test_support.rs`:
   - Compiled for `domain`'s tests or with the `test-support` feature, which other crates enable only in `[dev-dependencies]`. No new crate, so decision #003 holds.
   - First helper: `ManualClock` (`new`, `set`, `advance`). It uses an atomic, so it can be shared with the code under test.
3. `domain/Cargo.toml`: adds the `test-support = []` feature.
4. `AGENTS.md` §7: says where the shared builders live and how to enable them.

**Changes from the approved plan**
- No `time_utils.rs` yet; it's created with its first helper.
- No empty `tests/` folders.

**Tests:** `UnixMillis` ordering (including before 1970) and its millisecond round-trip; `ManualClock` set, advance, moving backward, and saturation; doctests on `UnixMillis` and `Clock`.

**Manual verification:** none needed.

### Step 4: `feat: debug log foundation`
*Planned 2026-10-28 · Branch: `milestone/p1-m1-local-skeleton` · Options compared in chat; chosen: own rotating writer, JSON-lines files, `Targets` filter*

**Where things go**
- `domain::log_area`: the log area names (`app`, `ui`, `sync`, `database`, `auth`) as `&str` constants. They're used as the tracing `target`, so every crate logs with `tracing::debug!(target: log_area::SYNC, ...)` and the viewer can filter by area. They're plain constants with no I/O, so they belong in the lowest crate.
- `crates/app/src/logging/` (one setup for every target, because the server binary is `app` with the `server` feature):
  - `entry.rs`: `LogEntry` holds time (`UnixMillis`), level, area, message, and fields. It serializes to and from one JSON line.
  - `capture.rs`: one tracing layer that builds a `LogEntry` per event and hands it to the ring buffer and, if one is configured, the file sink. Entries are built once and shared.
  - `ring_buffer.rs`: a fixed-capacity buffer (5,000 entries; the oldest are evicted first) behind a lock, with a cloneable handle for the viewer and export.
  - `rotating_file.rs` *(native only)*:
    - Writes the active file `current.jsonl`. When the next line would pass `max_file_bytes`, it renames the file to `log-<unix-ms>.jsonl` and starts a new one.
    - Cleanup runs at startup and after each rotation. It deletes the oldest rotated files beyond `max_files`, and any rotated file older than `max_age` (30 days).
    - File age comes from the timestamp in the file name, not the file system, so tests use `ManualClock` and no extra crate.
    - Per-platform policies are named constants: server 10 × 10 MB, desktop 5 × 10 MB, mobile 3 × 2 MB.
    - A write failure can't be logged through `tracing` (it would recurse), so it increments a failure counter that the diagnostics screen can show later.
  - `control.rs`: the runtime level switch. It's a `Targets` filter (default `info` for every area) behind `tracing-subscriber`'s `reload` handle, with `set_area_level` and `reset`. Not persisted, so it resets to the default on restart.
  - `export.rs`:
    - Filters entries by time range, minimum level, and areas.
    - Renders them as JSON lines or plain text, behind a header.
    - The header has the app version and platform now. API version and device ID are added when those exist (step 7, Phase 3).
  - `clock.rs`: `SystemClock`, the first real `domain::time::Clock`, via `web-time` so the same code works on web and native.
  - `mod.rs`: `init()` installs the subscriber **before** `dioxus::launch`, so Dioxus's own logger steps aside. It adds console output: `fmt` to the terminal on native, `tracing-wasm` to the browser devtools on web. It returns a `LogHandles` value (ring buffer and level control) that the UI gets through context in step 6.
- `crates/app/src/main.rs`: calls `logging::init()` first and logs one `info` startup line (area `app`).

**Deferred, with reasons**
- **Wiring the file sink:** the log directory comes from the platform data folder (desktop, M4) or the container volume (server, M3). Step 4 implements and tests the writer; it gets wired in when those paths exist.
- **"Last crash" file:** it also needs the log directory, so it comes with that same wiring.
- **No `non_blocking` writer:** writing directly keeps the last lines before a crash on disk.

**Dependencies** (all already in `Cargo.lock` through Dioxus, so nothing new is downloaded):
- `tracing`;
- `tracing-subscriber` (`std`, `registry`, `fmt`, `ansi`; no `env-filter`);
- `tracing-wasm` (wasm32 only);
- `web-time`;
- `serde` (`derive`);
- `serde_json`;
- dev-only: `tempfile`.

**Tests** (unit tests next to the code, using `ManualClock`, with a fresh temp directory per test)
- `ring_buffer_evicts_oldest_entries_beyond_capacity`, `capture_records_level_area_message_and_fields`
- `log_entry_round_trips_through_a_json_line`
- `rotating_file_rotates_before_exceeding_max_file_bytes`, `rotating_file_keeps_at_most_max_files`, `rotating_file_deletes_rotated_files_older_than_max_age`, `rotating_file_resumes_current_file_size_after_restart`
- `area_level_change_takes_effect_and_reset_restores_default`
- `export_filters_by_time_range_level_and_area`; a snapshot-style check of the plain-text and JSON-lines renderings, including the header

**Verification**
- The usual self-review commands, plus `cargo check -p app --target wasm32-unknown-unknown`. The web-build guard must still pass, because the rotating writer is `cfg(not(wasm32))`.
- A manual smoke test: `dx serve` shows the startup line in the terminal and the browser console. This is too small to need a `docs/test-plans/` file.

**Changes from the plan, found during implementation**
- **`app` is now a library plus a thin binary** (`src/lib.rs` + `src/main.rs`). Without the library, items the UI won't use until step 6 (export, rotating file, level control) would fail the `dead_code` lint. The binary has `doc = false` to avoid a rustdoc output-name collision.
- **`domain::time_utils::format_utc_iso8601`** was added (exact integer calendar math, no new crate), so plain-text exports show readable UTC times. This is the first entry in `time_utils.rs`.
- **The `ansi` feature was dropped from `tracing-subscriber`**, because it would have added `nu-ansi-term`, the one crate not already in `Cargo.lock`. Terminal output is uncolored. Adding it back is a one-word change if you want colors.
- **Extra tests:**
  - oversized lines, same-millisecond rotations, rotated-file name parsing;
  - `area_level` reporting, log-level ordering, the JSON shape;
  - log area name rules.
- **AGENTS.md §6:** always log with a `domain::log_area` target.

**Result:** 43 tests pass (22 `app`, 16 `domain`, 5 doctests). Clippy (native and wasm32), docs, the WASM guard, the `server` feature build, and `cargo deny` are all clean.

### Step 5: `feat: translation layer`
*Planned 2026-10-28 · Branch: `milestone/p1-m1-local-skeleton` · Options compared in chat. Chosen: `fluent-bundle` with our own wrapper (A), in `domain`, bidi isolation off*

**Where things go**
- `crates/domain/locales/en-US/app.ftl`: the English catalog, embedded at compile time with `include_str!`. There's no file I/O, so `domain` stays WASM-safe.
- `crates/domain/src/i18n/`:
  - `message_id.rs`: `MessageId`, a typed message ID. A small `define_messages!` macro declares each constant **and** the `ALL` list in one place, so neither can be forgotten.
    - First messages: the navigation labels from plan §5 UI direction that step 6 needs (Garage, Due & squawks, Messages, Timeline, Reports, Vehicles, Settings, Diagnostics).
    - Also one plural example (`open-squawk-count`) to prove plurals work.
  - `translator.rs`: `Translator` wraps a thread-safe `fluent_bundle` bundle.
    - `Translator::english()` returns a shared instance, built once on first use with `std::sync::LazyLock`, so no extra crate is needed.
    - `tr(id)` and `tr_with(id, &[("count", MessageArg::from(3))])` return the text.
    - Our own `MessageArg` type keeps Fluent types out of callers, so the engine could be swapped later.
  - Bidi isolation marks are off.
  - **Missing message** (only possible if the consistency test is skipped): `tr` returns the message ID itself, e.g. `nav-garage`. It never panics, so a gap is visible on screen.
- `crates/domain/src/lib.rs`: `pub mod i18n;`.
- `docs/decisions.md` #022: translation is Fluent through `fluent-bundle` in `domain::i18n`, with typed message IDs and a consistency test. Message IDs are kebab-case and grouped by prefix (`nav-`, `error-`, ...).
- `AGENTS.md` §3: says where the translation layer lives and how to add a message (`.ftl` entry plus a `define_messages!` line).

**Not in this step:** choosing the language per user or device, and switching language while running (the Phase 2 display-preferences question); locale formatting of dates, numbers, and units (Phase 2); any UI wiring (step 6).

**Dependencies:** `fluent-bundle` 0.16 and `unic-langid` 0.9, both Apache-2.0 OR MIT. They bring in `fluent-syntax`, `intl-memoizer`, `intl_pluralrules`, `self_cell`, and a few other small crates, checked with `cargo deny`.

**Tests** (in `domain`, written first)
- `every_message_id_exists_in_english_catalog`
- `every_english_message_has_a_message_id`: parses the `.ftl` file and compares its IDs with `ALL`.
- `english_catalog_parses_without_errors`
- `tr_returns_english_text`
- `tr_with_substitutes_arguments_without_isolation_marks`
- `plural_message_picks_one_and_other_forms`
- `missing_message_falls_back_to_its_id`

**Verification:** the usual self-review commands, plus `cargo check -p domain --target wasm32-unknown-unknown` and `cargo deny check`. There's no UI yet, so nothing to verify manually.

**Changes from the plan, found during implementation**
- **`MessageArg` also converts from `i32`,** so plain integer literals (`MessageArg::from(2)`) work without a type suffix.
- **Loading errors are a hand-written `CatalogError` enum** (invalid language, syntax, duplicates), because `thiserror` isn't a `domain` dependency yet.
- **One extra test,** `invalid_catalogs_are_rejected_with_a_reason`.
- **New crates in `Cargo.lock`:** `fluent-bundle`, `fluent-langneg`, `fluent-syntax`, `intl-memoizer`, `intl_pluralrules`, `self_cell`, `type-map`, `unic-langid`, `unic-langid-impl`. `cargo deny` passes.

**Result:** 52 tests pass (24 `domain`, 22 `app`, 6 doctests). Clippy (native and wasm32), docs, and `cargo deny` are clean.

### Step 6: `feat(app): skeleton UI and navigation`
*Planned 2026-10-28 · Branch: `milestone/p1-m1-local-skeleton`*

**Scope agreed in chat**
- Screens are the sidebar entries from plan §5, plus **+ Log** (log event) and a placeholder **vehicle detail** screen.
- The theme choice is held **in memory only** for now; saving it per device comes with local storage later. The plan's Phase 2 "UI/UX" open question about finalizing the design stays open: this step is placeholders, not final design.

**Routes** (`crates/app/src/ui/routes.rs`, Dioxus router, already in `Cargo.lock`)

| Path | Screen | Nav |
|---|---|---|
| `/` | Garage (vehicle list placeholder) | sidebar, bottom tab |
| `/due` | Due & squawks | sidebar, bottom tab |
| `/messages` | Messages | sidebar, bottom tab |
| `/timeline` | Timeline | sidebar |
| `/reports` | Reports | sidebar |
| `/vehicles` | Vehicles | sidebar |
| `/vehicles/:asset_id` | Vehicle detail placeholder (`asset_id` per the terminology rule) | — |
| `/log` | Log event placeholder | **+ Log** button / floating button |
| `/settings` | Settings | sidebar |
| `/diagnostics` | Diagnostics: basic log viewer | sidebar |
| anything else | Not found | — |

**Layout** (`ui/shell.rs`): one adaptive shell (decision #011), switched by CSS media queries.
- Wide (≥ 1024 px): full sidebar.
- Medium (640–1023 px): icon-only sidebar.
- Narrow (< 640 px): ☰ menu, bottom tabs (Garage, Due, Messages), and a floating **+ Log** button.
- The top bar has the page title, a **sync status placeholder**, and the theme toggle. On narrow screens the toggle moves into the ☰ menu.
- The mode chip (Driver ▾) waits until users exist (Phase 2).

**Theme** (`ui/theme.rs`): `ThemeChoice { Light, Dark, System }`, default **Dark** (decision #012).
- Held in a context signal and applied as `data-theme` on the root element.
- `System` follows `prefers-color-scheme` in CSS.

**Styling:** one plain CSS file, `crates/app/assets/main.css`, loaded with `asset!` + `document::Stylesheet`. CSS custom properties hold the colors for each theme, and media queries handle the three layouts. Minimal styling: readable and garage-friendly (large tap targets), not final design.

**Icons:** a handful of **Lucide** SVG icons (ISC license) copied into `crates/app/assets/icons/`, with their license file. They're needed for the icon-only sidebar.

**Log viewer** (`ui/screens/diagnostics.rs`):
- `main.rs` passes the logging handles to the UI with `LaunchBuilder::with_context`.
- The screen lists entries newest first, with a minimum-level filter, an area filter, and a **Refresh** button.
- Entries load **after the page mounts**. With fullstack, the server renders the page first, then the browser takes it over (hydration). Each side has its own log buffer, so rendering entries during that first server render would cause a hydration mismatch. The browser's own log is the one to show; the server log is Admin-only later (plan §5).
- Filtering reuses `ExportFilter`.
- Copy/export, search, auto-scroll, and live auto-refresh come later in Phase 1 or Phase 2. They need clipboard, download, and timer support per platform.

**Text:** every label goes through `domain::i18n`. New messages:
- screen titles and placeholder text (`screen-*`, `placeholder-coming-soon`);
- `action-log`, `action-refresh`, `menu-open`, `menu-close`, `sync-status-unknown`;
- `theme-light`, `theme-dark`, `theme-system`;
- log viewer labels, plus translated **area names and level names** (`log-area-*`, `log-level-*`), because these appear on screen.

No app display name anywhere; it isn't decided.

**Module layout:** `crates/app/src/ui/` contains `mod.rs` (`App` root), `routes.rs`, `shell.rs`, `nav.rs` (nav items: route, message, icon, where shown), `theme.rs`, and `screens/` (one file per screen). `main.rs` stays thin.

**Dependencies:** the `dioxus` `router` feature only. It's already in `Cargo.lock`, so there are no new crates. Lucide is copied files, not a crate.

**Tests** (logic, no browser)
- `every_route_round_trips_through_its_path`
- `unknown_paths_route_to_not_found`
- `every_nav_item_has_a_route_message_and_icon`
- `bottom_tabs_are_garage_due_and_messages`
- `theme_defaults_to_dark`, `theme_choice_maps_to_data_theme_attribute`
- `log_viewer_filters_by_level_and_area_newest_first`
- `every_log_area_has_a_translated_label`

**Manual test plan:** `docs/test-plans/p1m1-skeleton-ui.md`. It covers navigation, each layout at its width, the theme toggle, the log viewer, and what to capture if a step fails (the `ui` area at `debug`).

**Verification:** the usual self-review commands, the WASM build and its guard, then your manual run of `dx serve`.

**Changes from the plan, found during implementation**
- **One lint override, used three times.** `#[expect(unused_qualifications)]` sits on `Shell`, `ThemeToggle`, and `Diagnostics`, each with a justifying comment. `rsx!` expands event handlers into fully qualified `dioxus_elements::events::on*` calls. The prelude glob imports those same functions, so rustc flags the macro's own paths.
  - Explicit imports instead of the glob were tried. The macros also need `dioxus_core` and `dioxus_signals` in scope, so that approach would depend on Dioxus internals.
  - `#[expect]` errors once a Dioxus update fixes this, so the overrides can't go stale.
- **Simple placeholder screens share one file**, `screens/placeholders.rs`, instead of one file per screen; Diagnostics has its own file. Ten near-identical files would only add noise.
- **Timeline uses Lucide `list`**, because `history` isn't in the Lucide repo.
- **`LogLevel::ALL`** was added in `logging::entry`. Field formatting moved into the shared `logging::format_fields`, used by both the plain-text export and the viewer, so it isn't duplicated.
- **The time column is labeled "Time (UTC)",** because locale and time-zone formatting is Phase 2.

**Result**
- 62 tests pass (32 `app`, 24 `domain`, 6 doctests).
- Clippy is clean for native, wasm32, `server`, and `desktop`. Docs, the WASM guard, and `cargo deny` are also clean.
- `dx build --platform web` succeeds, with the server and client built and `main.css` bundled.
- A local smoke run of the built server returned 200 with the right title for `/`, `/due`, `/vehicles/abc123`, `/diagnostics`, and an unknown path. `data-theme="dark"` was set and icons were inlined.
- **Manual test plan:** `docs/test-plans/p1m1-skeleton-ui.md`. **Passed on web, 2026-10-29**, after the narrow-menu theme toggle fix.

### Step 8: `ci: release workflow and git-cliff`
*Planned 2026-10-28 · Branch: `milestone/p1-m1-local-skeleton`*

**Goal:** the release pipeline exists before the first tag, so `v0.1.0` at the end of Phase 1 exercises it (plan §8 CI/CD). Build artifacts are added by the milestones that create them: the Docker image in M3, desktop packages in M4.

**Files**
1. `cliff.toml` (git-cliff 2.x config):
   - Groups: Breaking changes, Features, Fixes, Performance, Refactoring, Documentation, Tests, Build, CI, Chores, Dependencies (`build(deps)` / `ci(deps)` from Dependabot).
   - Skips merge commits and non-Conventional commits. That includes the initial "Setting up workspace." commit and the milestone PR merge commits, whose steps are listed individually.
   - Links commits and PRs to `github.com/inner-marker/vehicle_manager_2`.
2. `.github/workflows/release.yml`: runs on tags `v*.*.*`.
   - **Guards:** the tag must equal `v` + the workspace version, read with `cargo metadata` and `jq`. The tagged commit must be on `main`.
   - **Gate:** reuses the main gate as a reusable workflow, so it gets a `workflow_call` trigger.
   - **Release notes:** this version's section, taken from the **committed** `CHANGELOG.md` (the reviewed text), not regenerated.
   - **Release:** created with the preinstalled `gh` CLI (`gh release create`). The token gets `contents: write` in this job only.
   - **Artifacts:** none yet. The checksums step comes with the first artifact.
3. `.github/workflows/main-gate.yml`:
   - Adds `workflow_call`.
   - New job: `CHANGELOG.md` must have a `## [x.y.z]` section for the `Cargo.toml` version.
4. `scripts/release/` (small shell scripts, so CI and local runs share them):
   - `check-version-tag.sh`
   - `changelog-section.sh <version>`: prints one version's notes and fails if they're missing.
   - `check-changelog.sh`
5. `docs/releasing.md`: the human's release checklist:
   - bump the version on `dev`;
   - `git cliff --tag vX.Y.Z -o CHANGELOG.md`, review, commit;
   - PR `dev` → `main` (merge commit);
   - tag on `main`, push the tag.
   - Also how to install git-cliff locally.
6. **`AGENTS.md` §9:** the scripts get a `shellcheck`-clean requirement if `shellcheck` is available. Otherwise nothing changes.

**Tests**
- The scripts are run locally against fixture files: a matching and a mismatched tag, a changelog with and without the section.
- `git cliff --unreleased` is run locally to check grouping and skipping on our real history.
- The workflows are only proven on GitHub. The first real run is the `v0.1.0` release at the end of Phase 1, with a checklist in `docs/releasing.md`.

**Dependencies:** none in the code.
- Tools: `git-cliff` 2.14 (MIT/Apache) for release prep on your machine. Install with `cargo binstall git-cliff`. CI doesn't need it, because notes come from the committed changelog.
- No new third-party actions.

**Changes from the plan, found during implementation**
- **`scripts/release/` has five scripts, not three:**
  - `workspace-version.sh` holds the version lookup in one place, shared by the other scripts;
  - `test.sh` runs 12 tests against `scripts/release/fixtures/CHANGELOG.md`. The main gate's changelog job runs it.
- **Group names in `cliff.toml` use zero-padded sort keys** (`<!-- 01 -->`). Plain numbers sorted "Dependencies" (10) before "Fixes" (2).
- **No AGENTS.md shellcheck rule,** because `shellcheck` isn't installed. AGENTS.md §9 instead lists `scripts/release/test.sh` for changes to the release scripts.
- **Grouping was checked with `git cliff --with-commit`** (injected test messages), because the agent deny rules rightly block `git commit`, even in a scratch repo.
- **`docs/plan.md` §8** now links to `docs/releasing.md`.

**Verification:**
- `git cliff --unreleased` on the real history groups every Conventional Commit and skips the initial commit and the PR #1 merge.
- With injected messages: breaking changes come first, Dependabot `build(deps)` and `ci(deps)` go under Dependencies, and merge and unconventional messages are skipped.
- A `git cliff --tag v0.1.0` output round-trips through `changelog-section.sh` and `check-changelog.sh`.
- All workflow YAML parses.
- The workflows are proven on GitHub at the `v0.1.0` release; the checklist is in `docs/releasing.md`.

### Step 7: `feat: auth middleware, handshake, and test server functions`
*Planned 2026-10-29 · Branch: `milestone/p1-m2-auth-handshake`*

**Choices made in chat (Dioxus 0.7.10 source checked)**
- **One router-level auth layer on `/api/*`, plus a coverage test.** It's default-deny: every server function is covered automatically. A test walks `ServerFunction::collect()` to prove it.
- **The credential comes from an environment variable, never from the repo** (decision #024). On the web, it's typed into the Diagnostics "Connection" panel. A missing variable means every request is refused (fail closed).
- **Version handshake:** the API version travels in a header on every request and is checked by the same layer. A `handshake` server function reports server info.

**Where things go**
- **`domain::api`** (new module; pure, WASM-safe):
  - `API_VERSION: u32 = 1` and `MIN_SUPPORTED_API_VERSION: u32 = 1`. These are separate from the app version (plan §8).
  - Header names: `x-api-version`, plus the standard `Authorization: Bearer <credential>`.
  - `check_api_version(client) -> Result<(), ApiError>`: the compatibility rule, shared by server and client.
  - `ApiError` reason codes (serde, kebab-case): `missing-credential`, `invalid-credential`, `auth-not-configured`, `missing-api-version`, `malformed-api-version`, `client-too-old`, `client-too-new`. Each maps to an HTTP status and a `domain::i18n` message, so the UI never shows server text.
  - `Handshake { app_version, api_version, min_supported_api_version, server_time_ms }`.
- **`server::auth`** (new module):
  - `AuthConfig::from_env()` reads `VEHICLEMANAGER_TEST_CREDENTIAL`. The name is built from `domain::identity::APP_ID`, never from the display name, and a test checks that.
  - `require_auth` is an Axum 0.8 middleware (`middleware::from_fn_with_state`). For any path under `/api/`, it:
    1. checks the credential in constant time (`subtle`);
    2. checks the API version;
    3. otherwise returns the JSON `ApiError`: **401** for a missing or invalid credential, **503** if auth isn't configured, **400** for a missing, malformed, or too-new version, **426** if the client is too old.
  - Auth is checked first, so unauthenticated callers learn nothing about versions.
  - Paths outside `/api/` (pages, assets) pass through, so the UI loads.
  - `debug` logs on `log_area::AUTH` say *why* a request was refused, never the credential. `warn` at startup if the variable isn't set.
- **`app`:**
  - `src/api.rs` holds the server functions under `/api/v1/`: `ping() -> Pong { server_time_ms }` and `handshake() -> Handshake`. They're thin: they only build the response, with the time from `SystemClock`.
  - `main.rs` (`server` feature): `dioxus::serve` with `Router::new().serve_dioxus_application(ServeConfig::new().context(log_handles), App)`, then `.layer(require_auth)`. Other targets keep `LaunchBuilder` as today.
  - Diagnostics gets a **Connection** panel above the log viewer:
    - a credential field (password input, held in memory only);
    - a **Run checks** button. It sets the headers with `set_request_headers`, calls `handshake` and then `ping`, and shows **pass/fail** for *version* and *auth*, the server's app and API version, and the round-trip time;
    - checks run only on click, never during server rendering (the SSR caveat above);
    - all labels and reasons go through `domain::i18n`.
- **Docs:**
  - decision #024 (credential source and fail-closed rule);
  - an AGENTS.md rule: "never call server functions during SSR; every server function lives under `/api/`";
  - `docs/test-plans/p1m2-auth-handshake.md`.

**Dependencies:**
- `server`: `axum` 0.8, `tower` 0.5, `subtle` 2.6, and `serde`/`serde_json` (already in the workspace).
- `domain`: `serde` (derive), for `Handshake` and `ApiError`.
- Dev-only: `tower` (`ServiceExt::oneshot`) and `http-body-util`.
- All are already in `Cargo.lock`, so nothing new is downloaded.

**Not in this step:**
- Rate limiting (`tower_governor`, plan §6), web login, session cookies, and device tokens (Phase 2 and 3).
- Showing the mode or user.
- The handshake's health and database-ID fields (Phase 2 and 3).
- Feature negotiation (`docs/ideas-to-discuss.md` #2, undecided).

**Tests**
- `domain`:
  - `api_version_check_accepts_supported_range`
  - `client_older_than_minimum_is_too_old`
  - `client_newer_than_server_is_too_new`
  - `every_api_error_has_a_status_and_message`
- `server` (tower `oneshot` on a test router):
  - `request_without_credential_is_401`
  - `wrong_credential_is_401`
  - `unconfigured_auth_refuses_every_api_request`
  - `missing_or_malformed_version_is_400`
  - `old_client_is_426`
  - `newer_client_is_400`
  - `valid_request_reaches_the_handler`
  - `non_api_paths_pass_through`
  - `credential_env_var_name_comes_from_app_id`
- `app` (`server` feature):
  - `every_server_function_is_under_api_prefix`
  - `every_server_function_rejects_unauthenticated_requests`, which builds the real router and calls each collected function.

**Verification:**
- The usual self-review commands, clippy for every feature set, the WASM guard, and `dx build --platform web`.
- A local run with `curl` covering the right, wrong, and missing credential and old, new, and missing version headers.
- Your manual test plan in the browser.

**Changes from the plan, found during implementation**
- **Lint override `#[cfg_attr(feature = "server", expect(clippy::unused_async))]`** on `ping` and `handshake`, each with a justifying comment.
  - Server functions must be async, and these two don't await anything yet. The lint fires on the server-side wrapper `#[get]` generates, so the expectation applies to the server build only.
  - Once a body awaits something, the `expect` errors, so it can't go stale.
- **The panel uses the existing `#[expect(unused_qualifications)]` rsx! override** (the same false positive as step 6).
- **Server functions are `pub(crate)`,** because they're only called inside the crate. With `pub`, `unreachable_pub` flagged the generated wrapper.
- **Imports used only in server-function bodies are declared inside the bodies.** `#[get]` keeps bodies in the server build only, so top-level imports would be unused on web.
- **The refusal body matches Dioxus's `ErrorPayload` shape** (`{"message","code","data"}`). Clients then receive `ServerFnError::ServerError { code, details: Some("<reason>") }`, and the panel reads the reason from `details`.
- **The coverage test uses `serve_api_application`** (server functions plus the render fallback, no built web assets), so it runs in a plain `cargo test`.
- **Fast checks now also run clippy and the tests for the `server` build** (`--no-default-features --features server`). Otherwise CI would never run the auth coverage tests.
- **Extra dev-only dependency: `tokio`** (`macros`, `rt`) in `server` and `app`, as the async test runtime. It's already in `Cargo.lock` through Dioxus, so `Cargo.lock` gains no new packages. It wasn't in the approved list, so it's **flagged for review**.
- **Extra tests:**
  - `non_bearer_authorization_is_401`, `credential_is_checked_before_version`, `refusals_use_the_dioxus_error_payload_shape`;
  - `debug_output_never_contains_the_credential`, `api_version_header_parses_whole_numbers_only`, `api_errors_serialize_as_kebab_case_codes`, `only_credential_problems_count_as_auth_failures`;
  - Connection panel tests: outcome mapping, request headers, and a message for every problem.

**Result**
- **Tests:** 88 pass on the default build (37 `app`, 31 `domain`, 13 `server`, 7 doctests); the `server` build passes 39 `app` tests, including both coverage tests.
- **Builds:** clippy is clean for native, wasm32, `server`, and `desktop`. Docs, the WASM guard, and `cargo deny` are clean, and `dx build --platform web` succeeds.
- **curl smoke run of the built server:**
  - every refusal case returned the documented status and reason;
  - an unknown `/api/` path was refused before routing;
  - a valid handshake and ping returned 200;
  - with the variable unset, every API request got `503 auth-not-configured` while pages loaded.
  - The log shows only the variable's *name*.
- **Manual test plan:** `docs/test-plans/p1m2-auth-handshake.md`.


### Milestone 3 plan (steps 9a–9e): web UI on the NAS
*Planned 2026-10-29 · Branch: `milestone/p1-m3-nas-web` · Choices agreed in chat*

**Target:** Synology DS1019+ (Intel J3455, x86_64), DSM 7.4.1, Container Manager and Tailscale installed, MagicDNS and HTTPS certificates enabled (decision #025).

**On the NAS** (human-made; the docs describe it, nothing here creates it):

```
/volume1/docker/vehicle_manager/   Container Manager Project folder
├── compose.yaml                   copy of deploy/compose.yaml
├── .env                           credential, host port, user/group IDs (never in git)
└── data/                          mounted as /data; owned by DSM user vehiclemanager (1030:100)
    └── logs/                      rotating JSON-lines logs + last-crash.txt
```

**9b: logs to the data directory** (`crates/app`)
- With the `server` feature, `VEHICLEMANAGER_DATA_DIR` (the name is built from `domain::identity::APP_ID`) adds a `FileSink` writing to `<dir>/logs` with `FileLogPolicy::SERVER` (10 × 10 MB, 30 days).
- If the variable is unset (local `dx serve`), logs go to the console and ring buffer only.
- If the folder isn't writable, the server logs a warning and continues without file logs. It never refuses to start over logging.
- **Crash file:** a panic hook writes `<dir>/logs/last-crash.txt` (time, message, location), then runs the default hook.
  - Our code doesn't panic, but dependencies can.
  - The file sits outside the rotation, so the record survives the restart (plan §5).
- **Tests:**
  - the environment variable name comes from the app ID;
  - the file sink gets attached and receives entries;
  - the crash report text is formatted correctly (a pure function);
  - an unwritable folder falls back to no file logs.

**9c: the image and compose file**
- **`Dockerfile`** (multi-stage):
  1. **Build stage:** `rust:1.95-trixie`. Install `dioxus-cli` 0.7.10 with `cargo install --locked` (its own cached layer). Copy the source and run `dx bundle --platform web --release --out-dir /out` (output verified: `server` + `public/`). BuildKit cache mounts hold the cargo registry and `target/`.
  2. **Runtime stage:** `debian:trixie-slim` plus `ca-certificates`.
     - `/app/server` and `/app/public/` sit side by side; the server finds `public/` next to its binary.
     - `IP=0.0.0.0`, `PORT=8080`, `VEHICLEMANAGER_DATA_DIR=/data`.
     - Default `USER 10001` (non-root). The compose file overrides it with the NAS user.
- Builds keep the **default x86-64 baseline**, never `target-cpu=native` (#025).
- **`.dockerignore`:** `target/`, `.git/`, `.claude/`, and local databases and logs.
- **`deploy/compose.yaml`:**
  - image `ghcr.io/inner-marker/vehicle_manager_2:${VEHICLEMANAGER_IMAGE_TAG:-dev}`;
  - `user: "${VEHICLEMANAGER_UID}:${VEHICLEMANAGER_GID}"`;
  - port `127.0.0.1:${VEHICLEMANAGER_HOST_PORT:-8410}:8080`, **loopback only**;
  - `./data:/data`;
  - the credential from `.env`;
  - `restart: unless-stopped`;
  - hardening: `read_only: true` with a `/tmp` tmpfs, `cap_drop: [ALL]`, `no-new-privileges`.
- **`deploy/.env.example`:** every variable with comments (UID 1030, GID 100, port 8410, tag `dev`), and a placeholder credential the server would reject as the example value.
- **`docs/deploying-nas.md`:** one-time NAS setup (folder, permissions, Project, `.env`, `tailscale serve`), updating (pull and recreate), rollback (pin `:sha-…`), and reading logs.
- **Dependabot:** adds the `docker` ecosystem for the Dockerfile's base images.

**9d: publishing to GHCR**
- **`.github/workflows/image.yml`:**
  - Runs on push to `dev`, and manually ("Run workflow") on any branch.
  - Builds `linux/amd64` with buildx, using the GitHub Actions cache.
  - Pushes `ghcr.io/inner-marker/vehicle_manager_2:dev` and `:sha-<short>`.
  - `permissions: packages: write` only in that job.
- **The release workflow** builds and pushes `:<version>` and `:latest` after the gate. The plan's "Docker image build (not published)" PR-to-main check becomes the main gate building the image without pushing it.
- **GHCR packages start private.** The deploy doc includes the one-time step to make the package public, so the NAS can pull without logging in.

**9e: manual test plan** (`docs/test-plans/p1m3-nas-web.md`)
- Set up the Project.
- `tailscale serve --bg --https=443 http://127.0.0.1:8410`, using the Synology package's CLI over SSH, with a fallback HTTPS port if 443 is taken.
- **Checks:**
  - from another tailnet device: open `https://<nas>.<tailnet>.ts.net`;
  - Diagnostics → Run checks passes with the right credential and fails with a wrong one;
  - logs appear in `data/logs`;
  - the container restarts after a NAS reboot;
  - the port isn't reachable from the LAN (`http://<nas-lan-ip>:8410` fails).
- Also what to capture if a step fails.

**Not in this milestone:** arm64 images; the database and its volume use (Phase 2); snapshots (Phase 2, the new open question); offsite backups; the desktop app (milestone 4).

**Step 9b: done, with notes**
- **New `domain::identity::env_var_name`:** `VEHICLEMANAGER_DATA_DIR` and `VEHICLEMANAGER_TEST_CREDENTIAL` are both named through it, and `server::auth` now uses it, so the naming rule lives in one place.
- **`logging::init()` is unchanged for clients.** The new native `init_with_log_dir(dir, policy)` adds the file sink and crash file. If the folder isn't usable, it warns and continues without file logs.
- **The crash file** is `logging::crash`: a panic hook writes `last-crash.txt` (time, version, panic text, backtrace), then runs the previous hook.
- **Tests (6 new):**
  - `env_var_names_share_the_app_id_prefix`, `data_dir_env_var_comes_from_app_id`;
  - `file_sink_writes_captured_events_to_the_log_dir`, `unusable_log_dir_is_an_error_not_a_panic`;
  - `crash_report_has_time_version_panic_and_backtrace`, `crash_file_is_outside_the_log_rotation`.
- **Smoke run of the built server:**
  - with a data dir, `logs/current.jsonl` is created and filled;
  - with a read-only one, pages still load and a `WARN` names the folder and the OS error.
  - The panic hook itself wasn't triggered end to end (there's no safe way to make the real binary panic); its report formatting is unit-tested.

**Step 9c: done, with notes**
- **`Dockerfile`:** `rust:1.95.0-trixie` build stage, with `cargo install --locked dioxus-cli@0.7.10` in its own layer, then `dx bundle --platform web --release --out-dir /out` with registry and `target/` cache mounts. The `debian:trixie-slim` runtime holds `/app/server` and `/app/public`, sets `IP=0.0.0.0`, `PORT=8080`, `VEHICLEMANAGER_DATA_DIR=/data`, and runs as `USER 10001:10001`. There's no `HEALTHCHECK`: it would need `curl` in the image. Container Manager's restart policy covers crashes.
- **`deploy/compose.yaml`:**
  - `user` is required from `.env` (`${…:?}`), so a missing value is a clear error;
  - the port is `127.0.0.1` only;
  - `./data:/data`;
  - `read_only`, a 16 MB `/tmp` tmpfs, `cap_drop: ALL`, and `no-new-privileges`.
- **`deploy/.env.example`:** the credential is left **empty** rather than a placeholder, so an unedited copy fails closed (503) instead of accepting a known value.
- **New `crates/app/tests/deploy_config.rs` (6 tests)** keeps the deploy files in step with the code:
  - the data-dir and credential variable names match `env_var_name`;
  - `./data` is mounted at `/data`;
  - every published port is `127.0.0.1`;
  - neither the image nor the container runs as root;
  - `.env.example`'s credential is empty.
- **Also:** `.gitignore` ignores `.env` and `deploy/.env`, and Dependabot watches the Dockerfile's base images.
- **`docs/deploying-nas.md`:** one-time setup, image tags, updating, rollback, and troubleshooting.
- **Not yet verified:** no real image build (there's no Docker on the dev Mac). The first build runs in CI in step 9d. The Dockerfile follows the bundle layout verified locally (`server` + `public/`).


**Step 9d: done, with notes**
- **One reusable workflow, `server-image.yml`,** builds linux/amd64 with buildx and the GitHub Actions cache, and adds OCI labels that link the package to the repo. It requests **no permissions itself**, because a reusable workflow can't ask for more than its caller grants. The publishers grant `packages: write`, and the main gate's build-only check grants nothing extra.
- **`dev-image.yml` triggers on push to `dev` or `milestone/**`** when code, the Dockerfile, deploy files, or the image workflows change, and also on "Run workflow".
  - **Changed from the plan:** "Run workflow" alone can't start a branch's first build, because GitHub only offers it for workflow files on the default branch.
  - Tags are `type=ref,event=branch` (`:dev`, `:milestone-p1-m3-nas-web`) plus `:sha-<commit>`.
- **The default branch is now `dev`** (decision #026, plan §8), so `workflow_dispatch` and `schedule` workflows run without waiting for a release.
- **`release.yml`** publishes `:<version>` and `:latest` after the gate. The GitHub Release waits for the image.
- **`main-gate.yml`** builds the image without pushing.
- **First real build (2026-10-29):** about 15 minutes, successful.
  - An anonymous pull from GHCR works (the package is public).
  - The image index holds linux/amd64 plus buildx's provenance attestation.
  - Tags: `milestone-p1-m3-nas-web`, `sha-354aa4a`.

**Step 9e: written**
- `docs/test-plans/p1m3-nas-web.md` has 16 steps:
  - Project creation and the container log;
  - log files and their owner;
  - loopback `curl`, then `tailscale serve`;
  - HTTPS with a valid certificate from another tailnet device and a phone;
  - wrong and right credentials;
  - not reachable from the LAN;
  - surviving a stop/start and a NAS restart;
  - the update and rollback flows.
- **Uses the milestone branch's files and image tag,** since milestone 3 isn't merged yet.
- **Passed on the NAS, 2026-10-29:** all 16 steps, after the step 9f fix.
  - `tailscale serve` on port 443 worked, so the 8443 fallback wasn't needed.
  - Step 15 first failed with "container name already in use": `docker compose` over SSH named the project after the folder (`vehicle_manager`), not the Project (`vehiclemanager`). Fixed in step 9f.
- **Phase 1 exit criteria, web half: met.** The web UI loads over HTTPS from other tailnet devices (laptop and phone), and the handshake passes with the right credential and is rejected cleanly with a wrong one. The desktop half is milestone 4.

**Step 9f: written**
- `deploy/compose.yaml` sets `name: vehiclemanager`, matching the Container Manager Project name.
- `crates/app/tests/deploy_config.rs`: `compose_project_and_container_are_named_after_the_app_id`.
- `docs/deploying-nas.md`: the Project name must match `name:`; a troubleshooting entry for the conflict error.

### Milestone 4 plan (steps 10.0–10c): desktop over Tailscale, macOS first
*Planned 2026-10-29 · Branch: `milestone/p1-m4-desktop` · Approved in chat, with macOS instead of Linux for the first runs (decision #027)*

**Goal:** finish Phase 1. The desktop app on the Mac reaches the NAS at `https://<nas>.<tailnet>.ts.net`, passes the handshake with the right credential, and is rejected cleanly with a wrong one.

**The constraint that shapes it:** in Dioxus 0.7.10, `set_server_url` stores the address in a `OnceLock`, once per process. If it's unset at launch, Dioxus uses `http://127.0.0.1:8080` (`DIOXUS_DEVSERVER_IP`/`PORT`). So the app reads the saved address **before** launching, and a changed address applies after a restart. Requests go through Dioxus's `reqwest` with `rustls` and the webpki roots, which trust the Tailscale (Let's Encrypt) certificate; macOS App Transport Security doesn't apply, because the requests don't come from the webview.

**10a: desktop data folder and log files** (`crates/app`, `desktop` feature)
- The data folder is `dirs::data_dir()` joined with `domain::identity::DATA_DIR_NAME`: `~/Library/Application Support/vehiclemanager` on macOS, `~/.local/share/vehiclemanager` on Linux. Never the display name.
- Logs go to `<data folder>/logs` with `FileLogPolicy::DESKTOP`, plus the crash file, reusing the server's wiring from step 9b.
- If the folder can't be found or created, the app logs a warning and runs without file logs.
- **Tests:** the folder path ends in `DATA_DIR_NAME`; an unwritable folder falls back to no file logs.

**Step 10a: written**
- `app::data_folder`: `platform_data_folder()` (`dirs::data_dir()` + `DATA_DIR_NAME`) and the pure `data_folder_in(base)`.
- `main.rs`: with the `desktop` feature, logs go to `<data folder>/logs` with `FileLogPolicy::DESKTOP` and the crash file, through the same `logging::init_with_log_dir` as the server. With no data folder, a warning and console-only logs.
- **Change from the plan:** `dirs` is a dependency of every **native** build (`cfg(not(target_arch = "wasm32"))`), not only the `desktop` feature. Its tests then run in the default `cargo test`, which CI runs; CI can't build the `desktop` feature on Linux without WebKitGTK. The web build still doesn't include it (`cargo tree --target wasm32-unknown-unknown -i dirs` is empty). The server image now compiles `dirs` too, unused.

**10b: server address setting** (`domain`, `crates/app`)
- **Rules in `domain`** (no I/O): trim; `https://` with a host; or `http://` only for `localhost`, `127.0.0.1`, or `[::1]`; no path, query, or fragment beyond `/`; a trailing `/` is dropped. Errors are translated messages.
- **`settings.json`** in the data folder holds the address. A missing, unreadable, or invalid file means "not set", with a warning in the log; the app still starts.
- **Before launch:** if an address is saved, `set_server_url` gets it. If not, Dioxus's default stays (local `dx serve`).
- **UI:** on desktop, the Diagnostics Connection panel gets a server address field showing the address in use. Saving shows "Saved. Restart the app to use the new address." Wrong addresses show the rule they break. The web build doesn't show the field (it always uses its own origin).
- The credential stays memory-only (#024, #027).
- **Tests:** address rules (accepted, rejected, normalized); settings round-trip; missing and corrupt files fall back to "not set" (temp directory).
- New messages go in `app.ftl` and `define_messages!`.

**Step 10b: written**
- `domain::server_address`: `ServerAddress::parse` (normalizes; `https://`, or `http://` for `localhost`, `127.0.0.0/8`, `[::1]`) and `ServerAddressError` with a translated message per rule.
- `app::settings` (native): `Settings { server_address }` in `settings.json`, unknown fields ignored; `load`, `save` (temp file + rename), `saved_server_address`, `save_server_address`.
- `main.rs`: with the `desktop` feature, `use_saved_server_address()` calls `set_server_url` before launch.
- Connection panel: on desktop, a server address field, "Using <address>", and the save result. Web and mobile render nothing there.
- 12 new messages in `app.ftl` and `define_messages!`.
- **New lint override:** `#[expect(unused_qualifications)]` on `ServerAddressField`, the same rsx! false positive as the other panels.

**10c: manual test plan** (`docs/test-plans/p1m4-desktop-macos.md`)
- Run with `dx serve --platform desktop` on the Mac, with Tailscale connected.
- **Checks:** set the address to the NAS's `ts.net` address, restart, and the panel shows it; wrong credential → Fail with "The server didn't accept this credential"; right credential → both Pass with the server version; an unreachable address → "Unreachable"; an `http://` non-loopback address is refused; logs appear in the data folder; the credential appears nowhere in them or in `settings.json`.

**Step 10c: written**
- `docs/test-plans/p1m4-desktop-macos.md` has 16 steps: data folder and logs; address rules in the UI; save, restart, and "Using"; wrong and right credential against the NAS; the address surviving a restart while the credential doesn't; Tailscale off → "Couldn't reach the server."; a corrupt `settings.json`; the credential absent from logs and settings.
- An incompatible API version can't be produced from the UI; the plan points to the automated tests and the milestone 2 `curl` checks.
- Found while writing it: `dx serve` may give the local development server a port other than 8080, so the plan says "the local development server" rather than a fixed address.
- **Passed on the Mac, 2026-10-29:** all 16 steps, against the NAS over Tailscale.
- **Phase 1 exit criteria: met.** The web UI (milestone 3, `docs/test-plans/p1m3-nas-web.md`) and the macOS desktop app (this milestone) both reach the server on the NAS over Tailscale and show the handshake result; a wrong credential is rejected cleanly on both. An incompatible version is rejected by the server (automated tests, and milestone 2's `curl` checks).
- **Raised during the run:** the desktop app should remember its window size in `settings.json`. Noted as idea 10 in `docs/ideas-to-discuss.md`, not scheduled.

**Verification:** the usual self-review commands, plus `cargo clippy -p app --features desktop --all-targets -- -D warnings` and `cargo check -p app --target wasm32-unknown-unknown` (the web build must not pull in `dirs` or the settings file code).

**Not in this milestone:** Linux and Windows runs, desktop packages, and saving the credential (#027).

