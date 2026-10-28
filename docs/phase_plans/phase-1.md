# Phase 1: Walking skeleton (plan)

*Agreed 2026-10-28 · Roadmap: `docs/plan.md` §9 Phase 1 · Open questions: none*

**Goal:** prove the full pipeline end to end, across separate devices and a separate server, before building real features. The web UI served from the NAS comes first; the Linux desktop app is the last milestone.

**Exit criteria:** from separate devices, both the web UI and the desktop app reach the server on the NAS over Tailscale and show the test response. A wrong credential or incompatible version is rejected cleanly.

**How this file works**
- Each row is one focused PR into `dev`, squash-merged with a Conventional Commit title (decision #017).
- Before each PR starts, the agent writes a detailed plan, waits for approval, then adds it under "PR details" below.
- The status column is kept current: ⬜ not started · 📝 planning · 🔨 in progress · 👀 in review · ✅ merged.

**Decisions shaping this phase:** #016 app identity, #017 merge strategy, #018 translation layer in Phase 1, #019 auth before any server function.

---

## PR sequence

| # | Status | PR title | Contents | New dependencies (need approval) |
|---|---|---|---|---|
| 0 | ✅ | *(initial commit)* Setting up workspace | Four-crate workspace, pinned toolchain, lints, `domain::identity`, docs reorganized, decision log | `dioxus =0.7.10` |
| 1 | ✅ | `docs: record Phase 1 sequencing decisions` | Decisions #017–#019; Phase 1 roadmap and Extensibility row updated; this file; AGENTS.md rule for `docs/phase_plans/` | none |
| 2 | 👀 | `ci: add fast checks, PR-to-main gate, and Dependabot` | **Fast checks:** fmt, clippy, test, Conventional PR title check. **PR-to-`main` gate:** those plus `cargo doc`, `cargo-deny`, Linux/Windows/macOS build matrix, WASM build. `deny.toml`. Dependabot targeting `dev`. Docker and migration-fixture jobs are added once those exist | CI actions only, pinned to commit SHAs |
| 3 | ⬜ | `feat(domain): passed-in clock and test support` | Domain code takes "now" as a parameter, never reads the clock; `time_utils.rs`; shared `test_support` scaffold (builders grow with Phase 2 types) | none |
| 4 | ⬜ | `feat: debug log foundation` | `tracing` setup, in-memory ring buffer (~5,000 entries), size-capped rotating log file, JSON-lines export format. The viewer UI comes in PR 6 | `tracing`, `tracing-subscriber`, a size-rotation crate *or* a custom writer (options compared in the PR plan) |
| 5 | ⬜ | `feat: translation layer` | Fluent-based, English only, usable from every target including WASM | e.g. `fluent-bundle`, `unic-langid` (specifics in the PR plan) |
| 6 | ⬜ | `feat(app): skeleton UI and navigation` | Placeholder screens (vehicle list, vehicle detail, log event, what's due, basic log viewer); adaptive wide/medium/narrow layout; Light/Dark/System theme, default Dark | `dioxus` `router` feature |
| 7 | ⬜ | `feat: auth middleware, handshake, and test server functions` | Auth middleware with a hardcoded test credential; API version handshake; ping and server version/time behind auth; pass/fail shown in the web UI | likely `axum`, `serde`, `thiserror` |
| 8 | ⬜ | `ci: release workflow and git-cliff` | `cliff.toml`; tag-triggered release workflow with version/tag guards; CHANGELOG entry check in the PR-to-`main` gate | — |
| 9 | ⬜ | Milestone 3: web UI on the NAS | Docker image (arm64), compose file, loopback-only port, `tailscale serve`; manual test plan in `docs/test-plans/` | — |
| 10 | ⬜ | Milestone 4: Linux desktop over Tailscale | Desktop app with a configurable server address; passes version and auth checks against the NAS; manual test plan | — |

Milestone mapping: PRs 1–6 and 8 are milestone 1 (local skeleton), PR 7 is milestone 2, PR 9 is milestone 3, PR 10 is milestone 4.

---

## PR details

*Detailed plans are added here as each PR is planned and approved.*

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
