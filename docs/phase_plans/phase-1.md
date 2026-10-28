# Phase 1: Walking skeleton (plan)

*Agreed 2026-10-28 · Roadmap: `docs/plan.md` §9 Phase 1 · Open questions: none*

**Goal:** prove the full pipeline end to end, across separate devices and a separate server, before building real features. The web UI served from the NAS comes first; the Linux desktop app is the last milestone.

**Exit criteria:** from separate devices, both the web UI and the desktop app reach the server on the NAS over Tailscale and show the test response. A wrong credential or incompatible version is rejected cleanly.

**How this file works** (decision #021)
- Each **milestone** has one branch (`milestone/p1-mN-<slug>`) and **one PR into `dev`** when the milestone is finished. That PR is merged with a **merge commit**.
- Each **step** is one Conventional Commit on the milestone branch, made by the human from the commands the agent drafts.
- Before each step starts, the agent writes a detailed plan, waits for approval, then adds it under "Step details" below.
- The status column is kept current: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.
- Steps 0–3b were merged as individual PRs, before decision #021.

**Decisions shaping this phase:** #016 app identity, #018 translation layer in Phase 1, #019 auth before any server function, #020 license policy, #021 one PR per milestone.

---

## Milestone 1: Local skeleton
*Branch: `milestone/p1-m1-local-skeleton` · Milestone PR: not opened yet*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 0 | ✅ | *(initial commit)* Setting up workspace | Four-crate workspace, pinned toolchain, lints, `domain::identity`, docs reorganized, decision log | `dioxus =0.7.10` |
| 1 | ✅ | `docs: record Phase 1 sequencing decisions` | Decisions #017–#019; Phase 1 roadmap and Extensibility row updated; this file; AGENTS.md rule for `docs/phase_plans/` | none |
| 2 | ✅ | `ci: add fast checks, PR-to-main gate, and Dependabot` | **Fast checks:** fmt, clippy, test, Conventional PR title check. **PR-to-`main` gate:** those plus `cargo doc`, `cargo-deny`, Linux/Windows/macOS build matrix, WASM build. `deny.toml`. Dependabot targeting `dev`. Docker and migration-fixture jobs are added once those exist | CI actions only, pinned to commit SHAs |
| 3 | ✅ | `feat(domain): passed-in clock and test support` | `UnixMillis` and the `Clock` trait in `domain::time`; `domain::test_support` (feature `test-support`) with `ManualClock`; builders grow with Phase 2 types | none |
| 3a | ✅ | `chore: allow read-only gh pr commands for agents` | Narrow the `gh pr:*` deny rule in `.claude/settings.json` to the write subcommands (incl. `checkout`, which changes the local branch), so `gh pr list/view/diff/checks/status` work | none |
| 3b | ✅ | `fix(ci): run the PR title check on pull_request` | `pull_request_target` workflows only run from the default branch (`main`), so the title check never ran on PRs 2–3. Switch to `pull_request` (opened, edited, synchronize, reopened). Weekly checks start running once `main` has them (first release). Also: `docs/plan.md` links to phase plans, and AGENTS.md requires that link for new phase plans | none |
| 3c | ☑️ | `docs: switch to one PR per milestone` | Decision #021 (supersedes #017); AGENTS.md §8a, `docs/plan.md` §8, and this file updated; fast checks no longer re-run on pushes to `dev` | none |
| 4 | ☑️ | `feat: debug log foundation` | Capture layer feeding a ring buffer (5,000 entries) and a size-, count- and age-capped JSON-lines file writer; runtime level control; console output; export renderers. File wiring and crash file arrive with M3/M4 paths. Viewer UI in step 6 | `tracing`, `tracing-subscriber`, `tracing-wasm` (web), `web-time`, `serde`, `serde_json`; dev: `tempfile` (all already in `Cargo.lock`) |
| 5 | ⬜ | `feat: translation layer` | Fluent-based, English only, usable from every target including WASM | e.g. `fluent-bundle`, `unic-langid` (specifics in the step plan) |
| 6 | ⬜ | `feat(app): skeleton UI and navigation` | Placeholder screens (vehicle list, vehicle detail, log event, what's due, basic log viewer); adaptive wide/medium/narrow layout; Light/Dark/System theme, default Dark | `dioxus` `router` feature |
| 8 | ⬜ | `ci: release workflow and git-cliff` | `cliff.toml`; tag-triggered release workflow with version/tag guards; CHANGELOG entry check in the PR-to-`main` gate | — |

## Milestone 2: Auth stub, handshake & test server functions
*Branch: `milestone/p1-m2-auth-handshake`*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 7 | ⬜ | `feat: auth middleware, handshake, and test server functions` | Auth middleware with a hardcoded test credential; API version handshake; ping and server version/time behind auth; pass/fail shown in the web UI | likely `axum`, `serde`, `thiserror` |

## Milestone 3: Web UI on the NAS
*Branch: `milestone/p1-m3-nas-web`*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 9 | ⬜ | *(steps planned when the milestone starts)* | Docker image (arm64), compose file, loopback-only port, `tailscale serve`; manual test plan in `docs/test-plans/` | — |

## Milestone 4: Linux desktop over Tailscale
*Branch: `milestone/p1-m4-desktop`*

| Step | Status | Commit title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 10 | ⬜ | *(steps planned when the milestone starts)* | Desktop app with a configurable server address; passes version and auth checks against the NAS; manual test plan | — |


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
