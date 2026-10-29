# Decision Log

*Companion to `docs/plan.md` · started October 19, 2026*

Settled decisions. **Don't re-open a recorded decision.** If one seems wrong, raise it with the human first. Each entry gives the decision, the reason for it, and where the plan covers it. Add new entries at the bottom.

**Entry template**

```markdown
### NNN. Short title
*Decided: YYYY-MM-DD · Plan: section name*

- **Decision:** what was decided.
- **Why:** the main reasons.
```

---

### 001. Rust 2024 with Dioxus fullstack
*Decided: before 2026-10-19 · Plan: §2 Technology Choices, §6 Backend*

- **Decision:** Rust (2024 edition) everywhere. Dioxus (0.7.x) for the UI on web, desktop, and mobile. Dioxus fullstack (on Axum) for the backend.
- **Why:** one codebase and shared types for client and server. Alternatives (egui, Iced, Slint, Tauri) were weaker on mobile or needed a second UI language.

### 002. SQLite everywhere
*Decided: before 2026-10-19 · Plan: §2, §6 Master database*

- **Decision:** SQLite on every device and as the master database on the NAS. WAL mode, busy timeout, `STRICT` tables, and the file on a local volume (never a network share). Backups via `VACUUM INTO` or the online backup API.
- **Why:** one migration set and shared SQL, no database container or port, and a single file to back up. Easily handles personal scale. sqlx keeps a later move to PostgreSQL possible.

### 003. Four-crate workspace with one-way dependencies
*Decided: before 2026-10-19 · Plan: §3 Workspace layout*

- **Decision:** crates `domain` ← `storage` ← `server` ← `app`. `storage`/`server` are native-only and sit behind native-only features in `app`.
- **Why:** sqlx doesn't compile to WASM; each layer is testable on its own; desktop host mode reuses the `server` library.

### 004. Hybrid logical clocks with last-write-wins per record
*Decided: before 2026-10-19 · Plan: §3 Sync & conflict handling, Hybrid logical clocks*

- **Decision:** every change gets an HLC stamp (48-bit ms + 16-bit counter packed into a `u64`, device ID as tiebreak). Drafts and configuration use last-write-wins per record. Finalized events are immutable and corrected by amendments.
- **Why:** device clocks drift; HLC gives a total, deterministic order without heavy conflict machinery.

### 005. Sync: push typed commands, pull current state
*Decided: before 2026-10-19 · Plan: §3 Sync protocol*

- **Decision:** devices push a versioned, idempotent outbox of typed domain commands (absolute values, never deltas). The server re-validates them and upcasts old formats. Devices pull current state since a sequence-number cursor. Deletes are tombstones.
- **Why:** client-sent SQL is unsafe, and SQLite changesets tie sync to the table layout. Commands are a stable, versioned API.

### 006. A table per event type, plus a JSON `extras` column
*Decided: before 2026-10-19 · Plan: §4 Event*

- **Decision:** a common `events` spine table plus 1:1 detail tables keyed by `event_id` (e.g. `fuel_details`). User-defined custom field values go in a validated JSON `extras` column. Core fields never go there.
- **Why:** real typed columns with `CHECK` constraints and plain-SQL statistics, while custom fields still stay flexible.

### 007. Integers in the database, `uom` in the domain
*Decided: before 2026-10-19 · Plan: §4 Units of measure, Storing quantities*

- **Decision:** quantities are stored as integers in a fixed unit (meters, seconds, milliliters) and are `uom` types in the domain. Conversion and rounding happen only in `storage`'s row ↔ domain `TryFrom`. Sync payloads carry the integer form.
- **Why:** exact, deterministic values on every device, with clean SQL comparisons, plus type safety in app logic.

### 008. Time: UTC epoch ms plus IANA zone; dates are date-only
*Decided: before 2026-10-19 · Plan: §4 Time storage*

- **Decision:** instants are stored as `i64` Unix epoch milliseconds (UTC) plus the IANA time zone where they happened. Calendar dates (due dates, expirations) are date-only `YYYY-MM-DD`.
- **Why:** unambiguous and sortable. Events display in local time after travel, and due dates never shift between time zones.

### 009. Money: integer minor units plus ISO currency code
*Decided: before 2026-10-19 · Plan: §4 Storing quantities*

- **Decision:** every cost is an integer in minor units with an ISO 4217 currency code.
- **Why:** exact, and not every currency has two decimal places.

### 010. `Asset` in code, "vehicle" in the UI
*Decided: before 2026-10-19 · Plan: §4 Terminology*

- **Decision:** the tracked thing is `Asset` / `asset_id` in code, tables, commands, and APIs. The UI says "vehicle" (or the kind's own word). Behavior that differs by kind comes from data-driven asset profiles.
- **Why:** not everything tracked is a vehicle, and components (engines, propellers) are assets too. Renaming after release would be very costly.

### 011. One adaptive UI layout
*Decided: before 2026-10-19 · Plan: §5 UI direction*

- **Decision:** a single responsive UI built from the same Dioxus components for desktop, web, and mobile (wide / medium / narrow). Platforms differ only in behavior.
- **Why:** one UI to build and maintain.

### 012. Theme: Light / Dark / System, default Dark
*Decided: before 2026-10-19 · Plan: §5 UI direction*

- **Decision:** a three-way theme setting, defaulting to Dark, stored per device. The toggle sits top-right on desktop/web and in the ☰ menu on narrow layouts.
- **Why:** user preference.

### 013. Tailscale via host `tailscale serve`, no sidecar
*Decided: before 2026-10-19 · Plan: §6 Security, §7 Networking*

- **Decision:** the app container publishes only to host loopback, and the host's Tailscale runs `tailscale serve` for HTTPS on the MagicDNS name. Tailscale is the only access path, even at home.
- **Why:** defense in depth with nothing exposed on the LAN, a real certificate, and a simpler compose setup.

### 014. License: MIT OR Apache-2.0
*Decided: before 2026-10-19 · Plan: §2*

- **Decision:** dual license, with `LICENSE-MIT` and `LICENSE-APACHE` in the repo root.
- **Why:** the Rust ecosystem standard.

### 015. Agent instructions in `AGENTS.md`, enforced by lints
*Decided: before 2026-10-19 · Plan: §8 Development Conventions*

- **Decision:** `AGENTS.md` is the single agent instruction file (`CLAUDE.md` points to it). Strict workspace lints and `clippy.toml` enforce the rules mechanically. Agents never run git or GitHub write operations.
- **Why:** multiple agent tools read `AGENTS.md`; agents reliably fix lint failures but can forget prose rules.

### 016. App identity
*Decided: 2026-10-19 · Plan: §3 App identity · Resolves the Phase 1 open question*

- **Decision:**
  - Bundle ID: `com.innermarker.vehiclemanager`
  - Internal app ID: `vehiclemanager`
  - Data-folder name: `vehiclemanager`
  - All three are constants in `domain::identity`. The display name is still undecided.
- **Why:** the bundle ID must be settled before the first release package, and can't change once installs exist. The internal ID and data folder stay neutral so the display name can change freely.

### 017. Merge strategy: squash into `dev`, merge commit into `main`
*Decided: 2026-10-28 · Plan: §8 Repository & branches · **Superseded by #021***

- **Decision:**
  - Feature PRs into `dev` are **squash-merged**, and the squash commit message is the PR title.
  - `dev` → `main` release PRs use a **merge commit**, so `dev` history stays intact.
  - Rebase merging is disabled on GitHub.
- **Why:** each feature lands as one Conventional Commit, the PR title the PR check already validates, and that gives git-cliff a clean changelog. Merge commits on `main` keep release history traceable to `dev`.

### 018. Translation layer moves into Phase 1
*Decided: 2026-10-28 · Plan: §9 Roadmap (Phase 1), §5 Extensibility groundwork*

- **Decision:** a minimal Fluent-based translation layer (English only) is built in Phase 1, **before** the skeleton UI. Locale formatting of dates, numbers, and units stays in Phase 2, when those values first appear in the UI.
- **Why:** AGENTS.md forbids hard-coded user-facing text, and the Phase 1 skeleton screens need text. Building the layer first means no hard-coded strings ever exist to be cleaned up later.

### 019. Auth middleware ships with or before the first server function
*Decided: 2026-10-28 · Plan: §9 Roadmap (Phase 1)*

- **Decision:** the Phase 1 test server functions (ping, server version/time) move out of milestone 1 and into milestone 2, delivered together with the auth middleware stub (hardcoded test credential) and the version handshake.
- **Why:** AGENTS.md requires every server function to go through auth middleware with no exceptions. This ordering means no unauthenticated server function ever exists, even briefly.

### 020. Dependency license policy: permissive licenses plus MPL-2.0
*Decided: 2026-10-28 · Plan: §8 CI/CD pipelines (cargo-deny)*

- **Decision:** `deny.toml` allows permissive licenses (MIT, Apache-2.0, BSD, ISC, Zlib, Unicode-3.0, BSL-1.0, CC0, Unlicense, NCSA, CDLA-Permissive-2.0) and **MPL-2.0**. Any other license fails the PR-to-`main` gate until reviewed.
- **Why:** MPL-2.0 is a weak, file-level copyleft that only requires sharing changes to the MPL-licensed files themselves, and it's compatible with shipping under MIT OR Apache-2.0. It comes in through Dioxus (`cssparser`, `selectors`, `dtoa-short`, `option-ext`), so it can't be avoided without dropping Dioxus.

### 021. One PR per milestone, merged with a merge commit
*Decided: 2026-10-28 · Plan: §8 Repository & branches, CI/CD pipelines · Supersedes #017*

- **Decision:**
  - Work is done on one **milestone branch** per roadmap milestone (`milestone/p<phase>-m<n>-<slug>`, cut from `dev`). Each planned step is one **Conventional Commit** on that branch.
  - **One PR into `dev` per milestone**, opened when the milestone is finished and merged with a **merge commit**, so every step's commit stays in `dev` history for git-cliff.
  - `dev` → `main` release PRs still use a merge commit.
  - Fast checks run on **PRs into `dev`** only, not again on the push to `dev` after merging.
  - Standalone PRs are only for things outside a milestone: urgent fixes and Dependabot updates.
- **Why:** CI on GitHub takes time to run, and a PR per small step multiplied that wait. The milestone PR still gets the full checks, the PR title check, and the human's review of the whole milestone. Merge commits keep the per-step changelog entries that squashing would collapse.
- **Trade-off:** commit messages on the branch aren't checked by CI (only the PR title is). Agents draft every commit message as a Conventional Commit.

### 022. Translation layer: `fluent-bundle` in `domain::i18n`, with typed message IDs
*Decided: 2026-10-28 · Plan: §5 Extensibility groundwork · Phase plan: phase-1.md step 5*

- **Decision:**
  - All user-facing text comes from Fluent catalogs (`crates/domain/locales/<language>/*.ftl`), embedded at compile time and looked up through `domain::i18n::Translator`.
  - The engine is `fluent-bundle` plus `unic-langid`, behind our own small wrapper (`MessageArg`, fallback to the message ID).
  - Code refers to messages only through `MessageId` constants declared with `define_messages!`. Tests check that the constants and the English file match in both directions.
  - Message IDs are kebab-case and grouped by prefix (`nav-`, `error-`, ...).
  - Bidi isolation marks are off until a right-to-left language is added.
- **Why:**
  - It lives in `domain` so every crate can produce user-facing text, including server-side reports and exports later.
  - `fluent-bundle` is the official, lightest Fluent engine and works on WASM.
  - Typed IDs plus the consistency test catch typos and missing messages without the heavier compile-time macro crates (`i18n-embed-fl`).
  - `dioxus-i18n` was rejected because it would tie translation to the UI crate.
- **Not decided here:** choosing the language per user or device, and switching language at runtime (the Phase 2 display-preferences question); locale formatting of dates, numbers, and units (Phase 2).

### 023. The repository is public during development
*Decided: 2026-10-29 · Plan: §8 Repository & branches, §7 Getting the image onto the NAS · Amends the plan's "private until ready to publish"*

- **Decision:**
  - `inner-marker/vehicle_manager_2` is public now, while still in development. It was always intended as FOSS (MIT OR Apache-2.0, decision #014).
  - Before going public, the history was rewritten so that every commit uses the project's dedicated contact address. The repo was then recreated, which also removed GitHub's hidden PR references to the old commits.
  - Release images go to **GHCR**. The plan's "private repo: attach a `docker save` file to the release" stage no longer applies; manual `docker save`/`load` stays available for development builds before a release exists.
  - **amd64** joins the image before the first image is published to GHCR, instead of "before the repo goes public". Exact tags and timing are settled when milestone 3 is planned.
- **Why:**
  - GitHub Actions jobs on the private repo were refused for billing. Public repos run on standard GitHub-hosted runners at no cost, including the macOS and Windows builds in the main gate.
  - Branch protection and rulesets, secret scanning with push protection, and code scanning are available on public repos.
- **Kept as-is:** the name in the MIT copyright line, the bundle ID, and the example data in the plan and UI sketches. They were reviewed before going public and judged fine.
- **Follow-ups:**
  - branch protection for `main` and `dev` (plan §8);
  - secret scanning and push protection turned on in repository settings.

### 024. Phase 1 test credential: from an environment variable, fail closed
*Decided: 2026-10-29 · Plan: §9 Roadmap, Phase 1 milestone 2 · Phase plan: phase-1.md step 7 · Replaces the plan's "hardcoded test credential"*

- **Decision:**
  - The server reads the single Phase 1 test credential from `VEHICLEMANAGER_TEST_CREDENTIAL`. The name is built from `domain::identity::APP_ID`, never the display name. The credential is never in the repository.
  - If the variable is unset or blank, the server refuses **every** API request with `503 auth-not-configured`. Pages and assets still load.
  - Clients send it as `Authorization: Bearer <credential>`, with `x-api-version` on every request. On the web, it's typed into the Diagnostics "Connection" panel and kept in memory only.
  - One Axum layer in `server::auth` guards every path under `/api/`: credential first (constant-time compare), then API version. Refusals are `401`, `400`, `426`, or `503`, with an `ApiError` reason code the UI translates.
  - A test proves every registered server function is under `/api/` and refuses unauthenticated requests.
- **Why:**
  - The repository is public (#023), and milestone 3 puts this server on the NAS, so a hardcoded credential would protect nothing.
  - An environment variable keeps it out of the repo and lets the exit criterion ("a wrong credential is rejected cleanly") be tested.
  - Failing closed means a misconfigured server can't be open by accident.
- **Replaced later by:** web login with session cookies (Phase 2 open question) and per-device tokens hashed with argon2 (Phase 3 pairing). Rate limiting arrives with them.

### 025. First server image is linux/amd64; arm64 later
*Decided: 2026-10-29 · Plan: §7 Target NAS & image architectures · Reverses the plan's "arm64 first"*

- **Decision:**
  - The NAS is a Synology **DS1019+**: Intel Celeron J3455 (x86_64), 8 GB RAM, DSM 7.4.1, with Container Manager and Tailscale installed.
  - Server images start **amd64-only**. **arm64 is kept for later**, for hosts such as a Raspberry Pi, as a multi-arch image under one tag.
  - Builds target the default `x86-64` baseline and never `-C target-cpu=native`, because the J3455 has no AVX.
  - `rust-toolchain.toml` pins `x86_64-unknown-linux-gnu` instead of `aarch64-unknown-linux-gnu`.
- **Why:**
  - The plan assumed an ARM Synology. The real CPU, checked in DSM's Info Center before milestone 3 as the plan required, is x86_64, so an arm64-only image wouldn't run.
  - amd64 also builds natively on GitHub's standard runners, without cross-compiling or emulation.

### 026. `dev` is the repository's default branch; milestone branches get their own images
*Decided: 2026-10-29 · Plan: §8 Repository & branches, CI/CD pipelines · Phase plan: phase-1.md step 9d*

- **Decision:**
  - The GitHub default branch is **`dev`**. `main` keeps its role: released code only, changed only by release PRs from `dev`, with tags `vX.Y.Z` on `main`.
  - Development server images are built when code, the Dockerfile, or the deploy files change on `dev` or a `milestone/**` branch. Each is tagged by branch (`:dev`, `:milestone-<name>`) and commit (`:sha-<commit>`), so a milestone can be tested on the NAS before its PR without moving `:dev`.
- **Why:**
  - GitHub runs `workflow_dispatch`, `schedule`, and `pull_request_target` workflows only from the default branch. With `main` as default, none of them could run until the first release.
  - PRs now default to targeting `dev`, which is where every milestone PR goes.
  - `workflow_dispatch` still needs the workflow file *on* `dev`, so the push trigger is what lets a milestone branch get an image before it's merged.
- **Unaffected:** tag-triggered releases, which run the workflow from the tagged commit, and the branch rules in plan §8.

### 027. First desktop platform is macOS; server address set at launch; credential in memory
*Decided: 2026-10-29 · Plan: §9 Roadmap, Phase 1 milestone 4 · Phase plan: phase-1.md milestone 4 · Reverses the plan's "Linux first"*

- **Decision:**
  - **macOS is the first desktop platform** for Phase 1's first runs. Linux (WebKitGTK) and Windows follow; all three still build in the PR-to-`main` gate.
  - **The server address is a desktop setting**, saved in `settings.json` in the platform data folder (from `domain::identity::DATA_DIR_NAME`) and **applied at launch**. A changed address takes effect after a restart, and the app says so.
  - Accepted addresses: `https://`, or `http://` only for a loopback host (local `dx serve`).
  - **The desktop credential is typed in and kept in memory only**, as on the web (#024). No keychain.
  - **Desktop packages** (`.deb`, `.rpm`, and the macOS bundle) are set up with the first release, not in milestone 4. During development the app runs with `dx serve --platform desktop`.
- **Why:**
  - The human develops on a Mac, so macOS gives the quickest first end-to-end runs. The exit criterion needs "the desktop app", not a particular OS.
  - Dioxus 0.7.10 stores the server address once per process (`set_server_url` is a `OnceLock`, and Dioxus falls back to `http://127.0.0.1:8080` if it's unset), so it can't change while the app runs.
  - Phase 2 logins replace the test credential, so saving it now would add a keychain dependency for nothing.
  - The exit criterion doesn't need packages, and packaging belongs with the release pipeline it runs in.
