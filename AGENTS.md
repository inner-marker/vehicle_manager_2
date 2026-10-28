# AGENTS.md

Instructions for AI coding agents (Claude Code, GitHub Copilot, and others) working on this repository. Humans should follow them too.

The code must stay **human-readable and human-maintainable**, even though most of it is written by agents.

---

## 1. Read first

- **`docs/plan.md`** is the source of truth for architecture and features.
- **`docs/ideas-to-discuss.md`** lists ideas that are **not decided.** Never implement anything from it; if a task touches one of those ideas, point it out and ask.
- **`docs/decisions.md`** records settled decisions (SQLite everywhere, HLC ordering, table per event type, etc.). Do not re-open a recorded decision; if you think one is wrong, say so and stop.
- For any task larger than a small fix, **write a short plan and wait for approval** before writing code.
- **`docs/phase_plans/`** holds the agreed plan for each roadmap phase (`phase-N.md`), so the human can follow along. Once a phase plan, or the detailed plan for one of its PRs, is agreed, **save it there** and keep its status column current as work progresses. When a new phase plan file is created, **link it from that phase's section** in `docs/plan.md` §9 Roadmap.
- **Before starting work on a roadmap phase,** check that phase's section in "Open Questions by Phase" in `docs/plan.md`. If any question is unresolved, **ask the user and stop** until it's answered. Then record the answer in `docs/decisions.md`, update the plan, and remove the question from the list.
- Don't answer open questions yourself by picking a default. If a question blocks only part of a task, say which part and continue with the rest.
- **`../xplm-oxide`** (the human's X-Plane plugin library) is available as **read-only reference** context. Look at it for conventions or when a task mentions it; never edit it, and don't add it as a dependency unless that is recorded in `docs/decisions.md`.

## 2. Pinned versions

Your training data may describe older APIs. **Do not write code from memory for these crates.** Check the version in `Cargo.toml` / `Cargo.lock` and use that version's documentation.

- **Dioxus 0.7.x** (APIs changed significantly across 0.5 → 0.6 → 0.7, especially fullstack and server functions)
- **Axum 0.8.x**
- **sqlx**: check the lockfile
- **uom**: check the lockfile

## 3. Architecture: where things go

The workspace has four crates. **Dependencies flow one way:** `domain` ← `storage` ← `server` ← `app`. Never add a dependency against that direction.

| Concern | Lives in | Rule |
|---|---|---|
| Domain types (`Asset`, `Event`, `Meter`, ...) | `domain` | Defined once, used everywhere |
| Validation (readings, permissions, squawk disposition) | `domain` | Runs on device *and* server; never duplicated in UI or server code |
| Sync commands and upcasters | `domain` | The **only** way data is changed |
| HLC | `domain` | |
| Row structs, `TryFrom` row ↔ domain, queries, snapshots | `storage` | The **only** place unit conversion and rounding happen |
| Migrations | `storage` (`migrations/`) | Numbered, forward-only, **never edit a released migration** |
| Sync engine, auth/permission enforcement, integrity checks, admin ops | `server` (library) | Used by the NAS server and (later) desktop host mode |
| UI components | `app` | Never touches the database directly; never contains business rules |
| Server functions | `app` (server feature) | Thin: authenticate, call into `server`/`domain`, respond |

Crate rules:
- **`domain` has no I/O and no database code.** It must compile to WASM.
- **`storage` and `server` are native-only.** In `app` they sit behind native-only features, so the web (WASM) build never pulls in sqlx. If a change makes the web build depend on them, stop and redesign.

Hard rules:
- **All writes go through commands.** No direct database writes from UI or server handlers.
- **Every server function goes through the auth middleware.** No exceptions.
- **Quantities are integers in the database** (meters, seconds, milliliters) and `uom` types in the domain.
- **Money is integer minor units plus an ISO currency code.** Never assume dollars or two decimal places.
- **Times are a UTC instant plus the IANA time zone where the event happened.** Never store local times without a zone.
- **Terminology:** the tracked thing is an **`Asset`** in code, tables, commands, and APIs (`asset_id`); the UI says "vehicle" (or the kind's own word). Never introduce `Vehicle` types or `vehicle_id` fields.
- **No hard-coded asset-kind logic.** Behavior that differs by kind comes from **asset profiles** (data), not `if kind == ...` checks scattered through the code.
- **Keep core and specialty code apart:** generic concepts in the core of `domain`; ground-vehicle specifics (fuel economy, towing) in their own module.
- **All user-facing text goes through the translation layer.** No hard-coded UI strings, error messages shown to users, or report text.
- **No `unsafe` code.**
- **App identity comes from one place:** the bundle ID, internal app ID, and data-folder name are constants in `domain`'s identity module. Read them from there; never hard-code copies.
- **Never derive file paths, data directories, keychain entries, or other storage locations from the display name.** The display name may change; storage locations must not.
- **Never change the bundle ID.** If a task seems to require it, stop and ask.

## 4. Reuse before you write

**Before writing any helper function, search the codebase for an existing one.** Duplicated functions are treated as a bug.

Shared helpers live in `*_utils.rs` modules, grouped by topic, in the **lowest crate that can hold them** (usually `domain`). Add new helpers to the matching module rather than inside the file that first needs them.

| Module | Contents |
|---|---|
| `units_utils.rs` | Unit conversion and display rounding |
| `hlc_utils.rs` | Hybrid logical clock helpers |
| `time_utils.rs` | Date and calendar-interval helpers |
| *(add rows as modules are created)* | |

If a helper doesn't fit an existing module, propose a new `xxx_utils.rs` in your plan.

## 5. Documentation and comments

**Doc comments (rustdoc conventions)**
- Every module starts with a `//!` doc comment explaining its purpose.
- Every public item has a `///` doc comment: the first line is a one-sentence summary.
- Include sections where they apply:
  - `# Errors`: when and why a function returns `Err`
  - `# Panics`: if it can panic (it should almost never)
  - `# Examples`: for non-obvious public APIs in `domain` and `storage`

**In-code comments**
- Comments explain **intent and reasons**, not what the syntax already says.
- **Iterator chains and other intricate patterns** (`.iter().filter().map().fold()`, nested combinators, closures with captured state):
  - A comment above the chain says **what it produces and why**.
  - Add per-step comments on any step that isn't obvious.
- If a chain needs a paragraph to explain, **split it into named intermediate variables or a well-named helper**. Readable code beats commented clever code.
- Explain every "magic" number with a named constant or a comment.

## 6. Code style

- Small, single-purpose functions. Watch for clippy's `too_many_lines` and `cognitive_complexity` warnings; treat them as a signal to split.
- Descriptive names; no abbreviations beyond the common ones (`id`, `db`, `hlc`).
- **Errors:** `thiserror` enums in the library crates (`domain`, `storage`, `server`); propagate with `?`.
- **No `unwrap()`, `expect()`, or `panic!()` outside tests.** Handle the error or return it.
- **Logging:** `tracing` only; never `println!` / `eprintln!` / `dbg!`. Always pass an area from `domain::log_area` as the target (e.g. `tracing::debug!(target: log_area::SYNC, ...)`), so the log viewer can filter by area and its level can be raised at runtime.
- **Lint overrides (`#[allow(...)]` / `#[expect(...)]`) are a last resort.** Fix the code first. Override only when it is strictly necessary or there is a genuinely good reason (e.g. a false positive, generated or macro code, a deliberate and documented trade-off).
  - **Every override has a justifying comment** directly above it explaining *why* the lint is wrong here or why fixing it is worse, plus a `reason = "..."` in the attribute. Clippy enforces the `reason`; reviewers check the comment.
  - **Prefer `#[expect]` over `#[allow]`:** `expect` errors once the lint stops firing, so stale overrides don't linger. Use `#[allow]` when `expect` doesn't fit, e.g. a lint that fires only on some platforms or `cfg` combinations.
  - Apply it to the **smallest possible item** (one statement or function, never a whole module or crate). Never disable lints crate-wide.
  - List every override you added in your final summary so the human can review it.

  ```rust
  // `meters` was validated above as finite, non-negative, and below MAX_ODOMETER_M,
  // so after rounding it always fits in i64; the cast cannot truncate.
  #[expect(clippy::cast_possible_truncation, reason = "range checked against MAX_ODOMETER_M above")]
  let odometer_m = meters.round() as i64;
  ```
- `rustfmt` formatting, always.

## 7. Testing

See "Testing strategy" and "Manual testing by the human" in `docs/plan.md` for the full approach.

**Automated tests**
- **Every change includes tests.**
- Domain logic (intervals, fuel economy, validation, permissions, HLC) is written **test-first**.
- **Where tests go:** unit tests in a `#[cfg(test)] mod tests` block in the same file; integration tests in the crate's `tests/` folder; `# Examples` in doc comments must compile and run.
- **Per crate:** `domain` uses pure unit tests; `storage` uses a fresh temporary SQLite database per test; `server` uses integration tests with real sync round-trips; `app` has component logic tests plus a few end-to-end web tests.
- **Deterministic:** code in `domain` never reads the real clock, random numbers, or location directly; they are passed in. Tests never depend on the current time.
- **Use the shared test data builders** in `domain::test_support` to set up assets, meters, intervals, history, and users (and `ManualClock` for time). Extend them rather than writing one-off setup code. Other crates enable them only in `[dev-dependencies]` with `domain = { workspace = true, features = ["test-support"] }`.
- Use **property-based tests** for invariants (HLC monotonic, unit conversion round-trips, command idempotency) and **snapshot tests** for generated output (reports, user-facing messages).
- Sync changes need **sync scenario tests**; schema changes need a **migration fixture test**.
- No network access and no shared state between tests.
- **Name tests after the behavior** (e.g. `oil_interval_resets_from_start_reading`).
- **Bug fixes start with a failing test** that reproduces the bug.
- Don't write filler tests to raise coverage; coverage is informational only.

**Manual test plans (when the human must verify something)**

If any part of a change can only be verified by a person (real devices, the NAS, Tailscale, camera, OBD adapter, UI behavior), include a **manual test plan** with your work, saved as `docs/test-plans/<feature>.md`:

1. **Intended behavior:** what the feature should do, including edge cases and what should be rejected.
2. **Setup:** device, data, mode, and server state needed first.
3. **Checklist:** numbered steps, each with its **expected result**, so each item is clearly pass or fail.
4. **If a step fails, capture:** which debug log area and level to enable before testing (e.g. `sync` at `debug`), what to export afterward (Settings → Diagnostics → Export), and any screenshots or error messages.

When the human reports a failure, **ask for the exported debug log** (preferably the JSON-lines export, limited to the relevant time range and areas) and other captured details. **Read and analyze the log before proposing a fix;** don't guess.

Add **useful `tracing` messages** at debug level in new code paths (never logging secrets), so a failed manual test produces a log that explains what happened.

## 8. Scope discipline

- **One focused task per change.** Don't refactor, rename, or reformat unrelated code.
- **No new dependencies without approval.** Propose them in your plan with a reason.
- Don't modify generated code, released migrations, or `Cargo.lock` by hand.
- Keep commits small, with clear messages describing *why*.

## 8a. Branches, commits, and pull requests

- **The human runs all git and GitHub write operations. Agents never do.** Do not run any command that changes repository state or history, locally or on GitHub, including:
  - `git add`, `git commit`, `git push`, `git merge`, `git rebase`, `git cherry-pick`, `git tag`, `git reset`, `git checkout`/`git switch` to create or change branches, `git branch -d`, `git stash`
  - creating, merging, or closing pull requests, or creating releases (e.g. via `gh`)
- **Read-only git is fine:** `git status`, `git diff`, `git log`, `git show`, `git blame`.
- **Instead, draft the commands** for the human to run when your work is ready, in a code block, for example:

  ```sh
  git switch -c milestone/p2-m1-meters dev     # milestone branch from dev (once per milestone)
  git add crates/domain/src/meter.rs           # stage only the files this step touched
  git commit -m "feat: add meter reading validation"
  git push                                     # back up the branch; no PR until the milestone is done
  ```

- **One PR per milestone** (decision #021). Work happens on a **milestone branch cut from `dev`**, named `milestone/p<phase>-m<n>-<slug>`. Each planned step is **one Conventional Commit** on that branch. When the milestone is finished, draft the **PR title and description** for the human to paste. The PR is merged into `dev` with a **merge commit**, not a squash.
  - Standalone branches and PRs (e.g. `fix/hlc-overflow`) are only for work outside a milestone, such as urgent fixes.
- Never work directly on `main` or `dev`. If the current branch is `main` or `dev`, say so and draft the command to create or switch to the milestone branch before making changes.
- **Conventional Commits** for every commit message and PR title: `feat:`, `fix:`, `docs:`, `refactor:`, `test:`, `chore:`, `ci:`, `build:`, `perf:`. Mark breaking changes with `!` (e.g. `feat!:`) or a `BREAKING CHANGE:` footer. The changelog is generated from these messages, so write them for a reader.
- **Don't edit `CHANGELOG.md` by hand;** it is generated by `git-cliff` during release preparation.
- **Don't change the version** in `Cargo.toml` or create tags unless the task is explicitly release preparation.
- Don't change the sync API version unless the task explicitly changes the API; it is separate from the app version.

## 9. Before you finish: self-review checklist

Run and pass:

```sh
cargo fmt --all --check                              # formatting
cargo clippy --workspace --all-targets -- -D warnings # all lints as errors
cargo test --workspace                                # all tests
cargo doc --workspace --no-deps                       # docs build cleanly
cargo deny check                                      # advisories, licenses, sources (when dependencies change)
```

Then confirm:
- [ ] Every new public item has a doc comment
- [ ] Intricate patterns have intent comments
- [ ] I searched for and reused existing helpers; no duplicated functions
- [ ] New helpers went into the right `*_utils.rs` module (and the table in section 4 is updated)
- [ ] Tests cover the change
- [ ] If the human must verify anything, a manual test plan is written in `docs/test-plans/`
- [ ] New code paths have debug-level `tracing` messages useful for troubleshooting
- [ ] No unrelated changes
- [ ] No new dependencies without approval
