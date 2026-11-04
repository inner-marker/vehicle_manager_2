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
- **Refined by #043:** last-write-wins is now **per field**, with field-level commands.

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
- **Retired 2026-10-30** (Phase 2 step 2e): replaced by sign-in with sessions (#028). The variable, its handling, and the Connection panel's credential field are gone.

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

### 028. Login: username and password with session cookies; first Admin by one-time setup code
*Decided: 2026-10-29 · Plan: §6 Security (Application), §10 Phase 2 open questions · Replaces the Phase 1 test credential (#024)*

- **Decision:**
  - **Sign-in is a username and password.** Passwords are hashed with **argon2** on the server. Login attempts are rate-limited.
  - **The web UI gets a session cookie** after login: `HttpOnly`, `Secure`, `SameSite=Strict`. The auth middleware accepts it on every `/api/` request.
  - **Native apps** (desktop now, mobile later) sign in with the same username and password until Phase 3 pairing issues per-device tokens.
  - **First Admin:** while the server has no users, it writes a **one-time setup code** to its log at every startup. The first visitor enters the code, then chooses the Admin's username and password. The code stops working once an Admin exists.
  - **Passkeys are later,** not in Phase 2.
- **Why:**
  - A password is enough behind Tailscale (only approved devices can reach the server), and works the same in every browser and in the native apps.
  - The setup code means only someone who can read the server's log (NAS access) can claim Admin, so another tailnet device can't get there first.
  - Passkeys need extra browser and platform work for little gain on a private tailnet.

### 029. Display preferences: per user, with server-wide defaults; each setting its own record
*Decided: 2026-10-29 · Plan: §10 Phase 2 open questions; ideas-to-discuss.md idea 7 (in part)*

- **Decision:**
  - **Units** (distance, volume, and the rest, chosen separately, so miles with litres is allowed), **default currency**, **language**, and **date format** have **server-wide defaults** set by the Admin.
  - **Each user can override any of them.** A user's choices follow them to every device they sign in on.
  - **Not per device.** The theme stays the only device-only setting.
  - **Each setting is its own record** (one per user per setting, and one per server default), stamped with the **HLC** of the change. Changes go through commands like every other write. Two devices changing settings offline then merge by **last-write-wins per setting**: a newer change to one setting doesn't undo an older change to a different one.
  - Display only: stored values stay metric integers and currency codes (§3, AGENTS.md).
- **Why:**
  - One household default saves every user setting things up, while still allowing a personal choice.
  - A record per setting makes #004 (last-write-wins per record, by HLC) work per setting, without a new merge rule. The HLC is the "timestamp": it follows real time, but stays ordered even when device clocks disagree.
  - Per-device units would show the same person different numbers on phone and laptop.
- **Idea 7 (layered settings):** this sets two layers, server and user. Whether device and asset layers are needed is still open.

### 030. Phase 2 asset kinds and profiles; trip computer readings on fuel-ups
*Decided: 2026-10-29 · Plan: §3 Asset profiles, Custom fields, §10 Phase 2 open questions*

- **Decision: six kinds ship in Phase 2,** each as a profile (seed data, never `if kind == ...`):

  | Kind | Default meters | Event types | Extra sections |
  |---|---|---|---|
  | Car | Odometer | Fuel, maintenance, repair, reading | Fuel economy |
  | Truck | Odometer | Fuel, maintenance, repair, reading | Fuel economy, towing and hauling |
  | Motorcycle | Odometer | Fuel, maintenance, repair, reading | Fuel economy |
  | Trailer | none (odometer optional) | Maintenance, repair, reading | none |
  | Generator | Run hours | Fuel, maintenance, repair, reading | none |
  | Other equipment | none (add any) | Maintenance, repair, reading | none |

  - Identifier types: VIN or serial number, and license plate.
  - Expected documents: title, registration, and insurance for cars, trucks, motorcycles, and trailers.
  - A fuel-up requires the odometer (run hours for the generator).
  - Boats, aircraft, and components are later; the model already allows them.
- **Decision: groundwork for trip computer readings** (what the vehicle's own display reports at a fill-up):
  - **Quantity types for custom fields:** besides text, number, date, yes/no, and choice list, a custom field can be a **distance**, **duration** (entered and shown as `HH:MM:SS`), **volume**, or **fuel economy**. Values are stored as integers in base units (meters, seconds, milliliters, meters per liter) and displayed in the user's units (#029).
  - **Profiles can ship custom field definitions** per event type, which the user can turn off or extend. Car, truck, and motorcycle fuel-ups ship with **trip time** (duration) and **trip EV distance** (distance; for hybrids, off by default), and the user can add more (e.g. trip distance, average speed).
  - **Vehicle-reported fuel economy is a real, optional column** on the fuel-up details, not a custom field, because the app compares it with its own calculated fuel economy, and the plan's rule is that anything the app computes on gets a real column (§3 Custom fields).
- **Why:**
  - These six cover the household's vehicles and equipment, and "other equipment" catches anything else without a new profile.
  - Trip computer values vary by vehicle, so most belong in defined custom fields; the one the app calculates with gets a column, so comparisons are plain SQL.

### 031. Intervals and service bundles belong to one asset; templates are copies
*Decided: 2026-10-29 · Plan: §3 Recurring maintenance, §10 Phase 2 open questions*

- **Decision:**
  - **Every requirement's interval rules and every service bundle belong to one asset.** There's no live link to a shared definition.
  - **Phase 2:** "Copy schedule from another asset" copies its requirements, interval rules, and bundles onto a new asset, where they can then be changed.
  - **Phase 5 (with what's due):** **named household templates** (e.g. "F-150 factory schedule"). Applying one **copies** it onto the asset; editing the template later doesn't change assets it was applied to. The copy may remember which template it came from, for information only.
  - Factory schedules from reference data packs (post-MVP) arrive as templates too.
- **Why:**
  - Each asset drifts from its siblings (synthetic oil on one truck, not the other). A live link would let one template edit silently move every asset's due dates.
  - Copies keep each asset's schedule stable and its history easy to follow, and templates can be added later without changing how intervals are stored.

### 032. Repairs reuse maintenance tasks, plus complaint and cause; labor on every task
*Decided: 2026-10-29 · Plan: §3 Maintenance & repair events, data model, §10 Phase 2 open questions*

- **Decision:**
  - **A repair event uses the same workflow and task structure as maintenance:** draft while in progress, tasks with parts, cost, and notes, squawk sign-off or deferral, and the same finishing checks.
  - **`repair_details`** (1:1 with a repair event, per #006) holds two optional fields: **complaint** (what was wrong, as noticed) and **cause** (the diagnosis). The **correction** is the event's tasks.
  - **Every task, maintenance or repair, gets optional labor:** **labor time** (entered as `HH:MM`, stored in seconds) and its cost split into **parts cost** and **labor cost** (minor units plus currency).
  - **A task's requirement is optional:** a repair task often fixes something no interval tracks.
  - Squawks resolved by a repair link to it through the existing squawk sign-off; nothing new.
- **Why:**
  - One task structure means one set of rules; a separate repair model would duplicate them.
  - Complaint, cause, and correction is how repair shops write up work, and it makes a repair's history readable later.
  - Labor on every task makes "shop vs. DIY" and labor cost reports possible for maintenance too.

### 033. Towing and hauling are percentages, shown by profile with a per-asset switch
*Decided: 2026-10-29 · Plan: §3 Driving conditions & towing, §10 Phase 2 open questions*

- **Decision:**
  - **Towing % and hauling % are integer percentages (0–100),** like highway %, not yes/no flags. Entered with the detent slider; stored as plain integers.
  - **Both default to 0%,** so a normal fill-up needs no extra input.
  - **Shown on kinds whose profile has the "towing and hauling" section** (the truck, #030), and on **any other asset where the user turns it on** (e.g. a car pulling a small trailer).
  - Fuel-ups keep listing which asset(s) were towed.
- **Why:**
  - One tank often mixes towing and normal driving; a flag would label the whole tank "towing" and distort both averages.
  - Percentages let fuel economy be broken down by condition, as the plan describes.
  - The per-asset switch follows the plan's rule that an asset's settings start from its profile and can be changed per asset.

### 034. When events finalize; confirm before finalizing by hand
*Decided: 2026-10-29 · Plan: §3 Finalized records and amendments, Maintenance & repair events, §10 Phase 2 open questions*

- **Decision:**
  - **Fuel-ups and readings finalize 24 hours after their last edit,** or sooner with **Finalize now**. Every edit restarts the 24 hours.
  - **Maintenance and repair events finalize only when the user taps Finish,** which runs the finishing checks first. They **never finalize automatically.**
  - **A forgotten job** (a maintenance or repair draft untouched for **14 days**) gets a reminder in the message center. It's only a reminder; nothing is locked.
  - **Later event types** (documents, lifecycle) default to 24 hours after the last edit unless their own section says otherwise.
  - **The delays are defaults the Admin can change** per event type.
  - **The server finalizes automatically,** so every device agrees on when an event locked.
  - **Finalizing by hand always asks for confirmation** ("Finalize this event?"), for both Finalize now and Finish: the dialog says that afterwards it can only be corrected with an amendment, and has Finalize and Cancel buttons. Automatic finalizing has no dialog.
- **Why:**
  - Finalizing can't be undone, so tapping it by accident shouldn't lock an event.
  - A 24-hour window lets a mistyped odometer be fixed without an amendment.
  - Maintenance can take a weekend or wait weeks for parts; locking it halfway would force amendments for normal work.

### 035. Nightly snapshots in the same pool for now, with a configurable folder for a USB disk later
*Decided: 2026-10-29 · Plan: §6 Data integrity & loss detection, Backups, §10 Phase 2 open questions · Changes the plan's "different NAS disk"*

- **Decision:**
  - **The nightly `VACUUM INTO` snapshot goes to `data/snapshots/` by default,** in the same storage pool as the live database. The server keeps the **last 7** and integrity-checks each one after writing it.
  - **Groundwork for a USB disk:** the snapshot folder is configurable (`VEHICLEMANAGER_SNAPSHOT_DIR`, built from `APP_ID`), so it can later point at a second mount in `compose.yaml`, such as a USB disk. If that folder is missing or not writable, the server keeps running, skips the snapshot, and reports it (log and Admin message).
  - **Not a focus now:** no USB setup, Hyper Backup task, or docs for it in Phase 2. They come later, or with offsite backups (post-MVP).
- **Why:**
  - The DS1019+ has one storage pool (4 data disks + 1 parity), so there's no "different NAS disk". Its parity already survives one failed disk.
  - Same-pool snapshots still cover what they're mainly for: corruption, a bad migration, a bad sync, and accidental deletion.
  - A configurable folder makes a USB disk (or any second volume) a deployment change later, not a code change.
- **Not covered until offsite backups:** losing the whole pool or the NAS.

### 036. Photos: Small, Medium, or Large JPEG per photo; originals only for Maintainers and Admins
*Decided: 2026-10-29 · Plan: §3 Photos from the web UI on a phone, Asset photos, §10 Phase 2 open questions*

- **Decision:**
  - **Every stored photo is a JPEG** (quality 85), turned upright, plus a **400 px thumbnail**. HEIC and other formats are converted. **Upload limit: 25 MB** per photo.
  - **A size picker on every photo:**

    | Size | Long edge | Typical size | For |
    |---|---|---|---|
    | **Small** | 1280 px | ~200 KB | Meter close-ups, receipts, labels |
    | **Medium** (default) | 2560 px | ~0.5–1 MB | General photos |
    | **Large** | 4096 px | ~2–3 MB | Damage, condition at the start of an event |

    A photo smaller than the chosen size is never enlarged. **Profiles can set the default size per event type** (e.g. Small on fuel-ups and readings).
  - **Keep original (gated):** a Maintainer (on that asset) or an Admin can also keep the **original file exactly as taken** (HEIC or camera JPEG), alongside the JPEG the app shows. Others never see the option. The Admin can turn the option off server-wide.
    - An original keeps everything the camera wrote, including GPS. **Exports leave originals out** unless chosen, with a warning that they include location and camera details.
  - **Metadata follows the existing photo rule:** time taken, GPS, and camera are read before resizing and kept as structured fields (shown in the photo's details and offered as suggestions); exports strip GPS by default. The JPEG files themselves carry no metadata.
  - **Where it happens:** the browser or app resizes to the chosen size before upload (less data to send); the server checks type, size, and dimensions, and converts anything that isn't a correctly sized JPEG.
  - The image-processing crate (likely `image`, plus HEIC support) is proposed for approval when the photo step is planned.
- **Why:**
  - Photos live in the database, so their size drives database, sync, and snapshot size. Choosing per photo keeps quick shots small and evidence photos sharp.
  - Originals are large and hold the most private metadata, so they're limited to the roles that do maintenance and documentation.

### 037. Documents: 50 MB per PDF; pdf.js as the viewer on every platform
*Decided: 2026-10-29 · Plan: §3 Attachments, Official asset documents, §10 Phase 2 open questions*

- **Decision:**
  - **Limit: 50 MB per PDF,** with no separate page limit.
  - **The server checks every upload** really is a PDF within the limit, and records its **page count** (shown on the document card, e.g. "12 pages").
  - **Viewer: Mozilla's pdf.js** (Apache-2.0), bundled with the app and used **on every platform:** web, macOS, Linux, Windows, and later mobile.
  - **Every PDF also has Download / Open in system viewer.**
  - The exact pdf.js version is proposed for approval when the documents step is planned.
- **Why:**
  - Size, not page count, is what affects the database, sync, and snapshots. 50 MB fits receipts, invoices, registrations, and most owner's manuals; very large manuals are better kept outside the app.
  - Linux's WebKitGTK can't show PDFs itself, and one bundled viewer looks and works the same everywhere, with room for thumbnails and search later.

### 038. Offline geocoder: GeoNames cities1000 and Natural Earth 1:10m, nearest boundary as a fallback
*Decided: 2026-10-29 · Plan: §3 Reverse geocoding, §10 Phase 2 open questions (geocoder dataset; geocoding on device vs. server) · **Cities changed to cities500 by #059***

- **Decision:**
  - **Cities:** GeoNames **cities1000** (every place with at least 1,000 people), nearest place wins.
  - **States/provinces and countries:** **Natural Earth admin boundaries at 1:10m**, slightly simplified for size.
  - **Outside every polygon, the nearest one is used.** Natural Earth doesn't trace exact coastlines, so a point on a beach, pier, causeway, or island can fall just outside its country or state. The lookup then takes the **nearest country, and the nearest state/province** (within that country where possible), by distance to the boundary. A point never ends up with no country or state.
  - **Shipped as a versioned data file** the server loads at startup, not compiled into the program, so the data can be updated without a new build (and fits reference data packs later).
  - **The crate** (`reverse_geocoder` or a small lookup of our own) is chosen, with dependency approval, when the location step is planned.
  - **Phones (Phase 4):** a budget of about **15–20 MB** for the bundled data, checked in Phase 4. If it's too large, phones store coordinates and the server fills in the place at sync, as the plan already allows.
  - **Attribution:** "Contains GeoNames data (CC BY 4.0)" on the About screen.
  - The looked-up place is a suggestion; the user can still correct the event's city, state, and country.
- **Why:**
  - cities1000 resolves small towns without the size of cities500; cities5000 would put small-town events in the next bigger town.
  - Coarser boundaries put events near a border in the wrong state, and the nearest-boundary fallback covers the coastline gaps any simplified dataset has.

### 039. No map picker in Phase 2; map tiles decided with the event map
*Decided: 2026-10-29 · Plan: §3 Event location, Event map (post-MVP), §10 Phase 2 open questions*

- **Decision:**
  - **Phase 2 has no map.** An event's location comes from the device's **GPS**, a **saved place**, or **manual entry** (city, state, country, with the geocoder's suggestion).
  - **The map picker arrives with the post-MVP event map,** and both use the same tile source, decided then.
  - **Preferred option for then:** self-hosted tiles, a **PMTiles** file (Protomaps) for the region, served by the server on the tailnet: no outside service or usage limits, and it works offline. The alternative is an online OpenStreetMap-based tile provider.
  - The `map` location source stays in the data model, so map picks need no migration later.
- **Why:**
  - GPS, saved places, and manual entry cover nearly every real event without an online dependency.
  - One tile decision for both map features avoids choosing twice.

### 040. UI direction for Phase 2 screens; SVG layout options before building each screen
*Decided: 2026-10-29 · Plan: §5 UI direction, §10 Phase 2 open questions*

- **Decision:**
  - **Garage cards:** ordered by **urgency** by default (overdue → due soon → OK, then most recent activity). Each user can switch to **My order** and drag cards (a per-user setting, #029).
  - **Vehicle detail:** a header (photo, name, main identifiers, current meter readings) and tabs **Overview** (next due, open squawks, latest fill-ups and fuel economy, last activity), **Timeline**, **Maintenance** (requirements, intervals, bundles), **Fuel** (fill-ups, economy by condition), **Documents**, and **Setup** (meters, profile overrides, custom fields). On phones the tabs are a scrollable row.
  - **Message center:** an inbox, newest first and unread on top, with a severity marker (info / warning / urgent); each message can be marked read, dismissed, and have one action; filters by severity and vehicle.
  - **Log viewer:** the Phase 1 viewer stays; search and export come later.
  - **Garage-friendly:** touch targets at least **48 px**, large numbers, high contrast in both themes, and nothing that needs precise taps (no tiny icons or hover-only controls).
  - **Quick logging:** the **+ Log** chooser (Fuel-up, Reading, Squawk, Maintenance) remembers the last vehicle and offers "Same vehicle as last time"; the fuel-up form starts with the odometer (last value as a hint), then volume and price; a vehicle card's long-press or ⋯ menu offers "Log fuel-up for this vehicle".
  - **Process:** before a screen is built, its step plan includes **SVG layout options** in `docs/ui-sketches/` for the human to choose from: usually two or three, or **one when only one makes sense**. The chosen layout is linked from the step plan.
- **Why:**
  - Urgency first puts what needs attention on top; a personal order covers households that think in their own order.
  - The tabs group a vehicle's information the way it's used, and the garage-friendly rules come from using the app with gloves or dirty hands.
  - Choosing between drawn options is quicker and clearer than describing a layout in words.

### 041. Mobile apps move to post-MVP
*Decided: 2026-10-29 · Plan: §9 Roadmap (Post-MVP: Mobile), §10 Open questions · Replaces "Phase 4: Mobile"*

- **Decision:**
  - The Android and iOS apps, and everything that needs them (offline "glovebox", native camera and document scanning, native location, the bundled geocoder), are **post-MVP**.
  - **Phones use the web UI** over Tailscale in the meantime: camera and photo library through the browser, browser location, and the adaptive narrow layout.
  - **The phase numbers stay** (Phase 3, then 5 and 6) so existing references remain valid.
  - Phase 4's open questions move: mobile distribution and camera to Post-MVP; **desktop signing** to Phase 6, since it's needed by the first release with desktop packages.
- **Why (the human's reasons):** the web UI already works on phones; app signing, sideloading, and distribution are a burden for the first mobile release; and the focus is on core features first.

### 042. Record IDs are UUIDv7; human-friendly record numbers later
*Decided: 2026-10-29 · Plan: §3 Data model, §9 Roadmap · Was idea 5 in ideas-to-discuss.md*

- **Decision:**
  - **Every record's ID is a UUIDv7,** generated on the device that creates the record (as the plan already required for offline use).
  - **The database ID** (plan §6) is a UUIDv7 too.
  - **Human-friendly record numbers** (e.g. "F-150 entry 42") are **later**, with reports or logbooks (post-MVP). When added, the server assigns them as events finalize, and a migration numbers existing finalized events in finalize order.
- **Why:**
  - UUIDv7 starts with the creation time, so IDs sort in creation order: SQLite appends new rows instead of inserting them at random places, and IDs in logs line up with time. It's as unique as random UUIDs, and can only be chosen before records exist.
  - It reveals roughly when a record was created, which doesn't matter for private, self-hosted data.
  - Nothing in Phase 2 prints records, and numbers can be added later without losing anything.

### 043. Field-level commands with last-write-wins per field; unknown fields and types are kept
*Decided: 2026-10-29 · Plan: §3 Sync & conflict handling, Sync protocol · Was idea 1 in ideas-to-discuss.md · **Refines #004** (per record → per field)*

- **Decision:**
  - **Update commands carry only the fields that changed,** each with its new absolute value, never the whole record.
  - **Conflicts resolve per field:** the server stores the HLC (and device ID) of the last accepted change **for each field** of each record. An incoming change to a field applies only if its HLC is newer than that field's; changes to different fields always merge. A record's own HLC is the newest of its fields', for ordering pulls.
  - **Creates** set every field with the command's HLC. **Deletes** are tombstones with their own HLC; a change older than the tombstone is ignored.
  - **Unknown fields are kept:** an app that receives fields it doesn't know stores and returns them untouched instead of dropping them. **Unknown event types** are kept too and shown as "Unsupported record. Update the app to view."
  - Finalized events stay immutable (#004); amendments are new records.
- **Why:**
  - Whole-record updates from an older app would silently delete fields it doesn't know about.
  - With field-level commands, last-write-wins per *record* could still drop an older-stamped edit to a *different* field (e.g. price on the laptop, notes on an offline phone). Per-field clocks merge those, like per-setting records do in #029.
  - The command format is the hardest thing to change after release, so it's settled before the first command exists (Phase 2 milestone 1).

### 044. One background job system on the server
*Decided: 2026-10-29 · Plan: §6 Data integrity, Backups · Was idea 9 in ideas-to-discuss.md*

- **Decision:**
  - **One small scheduler inside the server** runs all timed work: the nightly snapshot and periodic integrity checks first (Phase 2 milestone 1), then automatic finalizing, forgotten-job reminders, document expirations, re-geocoding, and later reference data refreshes.
  - **Each job's last run, result, and next run** are stored in the database and logged.
  - **Failures become Admin messages** in the message center (once it exists, milestone 3).
  - **An Admin "Jobs" status view** comes later, when there are enough jobs to need one.
  - Built on the server's existing async runtime (tokio timers); no scheduler crate.
- **Why:** one place to see, log, and test timed work, instead of several ad-hoc timers.

### 045. Roles are granted on one asset or on all assets; groups later
*Decided: 2026-10-30 · Plan: §6 Users, roles & permissions · ideas-to-discuss.md idea 6 (in part)*

- **Decision:**
  - **A role grant targets one asset, or "all assets".** "All assets" includes assets added later, so a household member gets one grant instead of one per vehicle.
  - **Grants combine** as the plan already says: a person's permissions on an asset are the union of every grant that covers it.
  - **The grant's target is an extensible kind** (`Asset(id)` or `AllAssets`), so a third kind, `Group(id)`, can be added later without changing existing grants.
  - **Groups ("garages") wait** until there's a real need (a club, co-ownership, a second household).
- **Why:** covers a household with almost no setup, avoids building group screens in Phase 2, and keeps the door open for groups.

### 046. User management: list and detail; server-made temporary passwords; no orange sidebar edge
*Decided: 2026-10-30 · Plan: §5 UI direction, §6 Users, roles & permissions · Phase plan: phase-2.md step 2g · Sketch: `docs/ui-sketches/p2m2-users-options.svg`, option A*

- **Decision:**
  - **The Users screen is a list and a detail page** (option A): users on the left, the selected person's roles and account actions on the right; on phones, the list then a page per person. Per-vehicle roles go under the person's **Roles** in milestone 4.
  - **Temporary passwords are made by the server** (12 characters from the setup code's unambiguous alphabet) and shown to the Admin **once**, when adding a user or resetting a password. The user must choose their own at their next sign-in. Admins never type or see anyone's real password.
  - **Disable and reset ask "Are you sure?" first.** For the last enabled Admin, disabling and changing the role are unavailable, with the reason shown.
  - **Admin mode shows as the orange mode chip and the banner only**, with no orange edge on the sidebar (removed from step 2f).
- **Why:** one page per person has room for per-vehicle roles later and keeps destructive actions visible; server-made temporary passwords are never weak or reused; the banner and chip are enough to make Admin mode unmistakable.

### 047. Password recovery: a one-time code in the server log, for Admins
*Decided: 2026-10-30 · Plan: §6 Users, roles & permissions · Phase plan: phase-2.md step 2h · Extends #028*

- **Decision:**
  - The sign-in screen has a **"Forgot your password?"** link for everyone. It asks for the username and always answers **"Ask your Admin for further instructions."**, whatever the username is.
  - **Only for an Admin account** does the server then write a **one-time recovery code** to its log (like the setup code). Entering the username, the code, and a new password signs them in and **ends all their other sessions**.
  - The code works **once** and **expires after 15 minutes**; requests and wrong codes count toward the sign-in attempt limit. It's kept in memory only.
  - Anyone else who forgot their password asks an Admin, who resets it from the Users screen (#046).
- **Why:**
  - If the only Admin forgets their password, there's otherwise no way back in.
  - Only someone who can read the NAS's log can finish a recovery, like first-Admin setup; anyone on the tailnet can ask, but can't use the code.
  - One generic answer for everyone reveals nothing about which usernames exist or which are Admins, and a real Admin knows to check the log.

### 048. Settings: tabs and buttons; the units, formats, and defaults offered
*Decided: 2026-10-30 · Plan: §5 UI direction · Phase plan: phase-2.md step 2i · Sketch: `docs/ui-sketches/p2m2-settings-options.svg`, option B · Refines #029*

- **Decision:**
  - **Layout (option B):** two tabs, **You** and **Household** (the Household tab works only in Admin mode). Each setting is a row of buttons; on the You tab each has a **"Use household default"** switch. Currency and language use a list. The theme stays under **This device**. Changes save at once.
  - **Choices and household defaults** (the default is listed first):
    - **Distance:** miles, kilometers.
    - **Fuel volume:** US gallons, imperial gallons, liters.
    - **Fuel economy:** mpg (US), mpg (UK), L/100 km, km/L.
    - **Pressure** (e.g. tires): psi, kPa, bar.
    - **Date format:** **30 Oct 2026**, 10/30/2026, 30/10/2026, 2026-10-30.
    - **Currency:** any ISO 4217 code; default USD, with common ones listed first.
    - **Language:** English (US) only, for now.
  - **Storage (refines #029):** one preferences record for the household and one per user, **one field per setting**. With last-write-wins per field (#043), each setting still merges on its own, as #029 requires. A user's unset field means "follow the household default".
- **Why:** the human preferred buttons and an explicit "use household default" switch; pressure and the day-month-name date were asked for; per-field records give #029's per-setting merging without a second kind of record ID.

### 049. Leave Admin mode and sign out when idle
*Decided: 2026-10-30 · Phase plan: phase-2.md milestone 3 (its first step) · Refines #028 (sessions) and #048 (adds a setting)*

- **Decision:**
  - **The app watches for inactivity** while someone is signed in. Only real input counts: pointer down, key down, wheel or scroll, touch. Background requests don't.
  - **After 15 idle minutes in Admin mode,** the app leaves Admin mode by itself (as **Leave** does) and shows a short notice: "Admin mode ended after 15 idle minutes."
  - **After a longer idle time,** the app signs out and the sign-in card shows a short notice ("You were signed out after N idle hours."). A setting, **Sign out when idle**, offers 1 hour, 4 hours, **8 hours** (default), 24 hours, and Never.
  - **Keeping the server in step:** while in Admin mode, the app checks its session at most once a minute (the server's own interval for recording use), and only if there was input since the last check. Real work keeps Admin mode alive on the server even without other requests, and the app notices at once if the server ended Admin mode or the session.
  - **The server stays the authority:** its 15 idle minutes for Admin mode and 30 idle days for sessions are unchanged and still apply when the app is closed.
  - **How (no new dependency):** one long-lived `document::eval` script records the last input and reports the idle time to Rust about every 15 seconds; this works on web and desktop. The decision itself (stay, keep alive, leave Admin, sign out) is a pure function, unit-tested.
  - **The setting is per person** (settled 2026-10-31, milestone 3 plan): a new field on the preferences record (#048), so each person chooses their own and the household sets the default, like the units.
- **Why:**
  - Today the app only notices the end of Admin mode at the next navigation or click, so the orange banner can outlive Admin mode on a screen left open.
  - A browser left open could stay signed in for up to 30 idle days.
  - Raised by the human during the milestone 2 NAS test.

### 050. Messages are derived from the current state; only read and dismissed are stored
*Decided: 2026-10-31 · Plan: §5 In-app notifications · Phase plan: phase-2.md milestone 3 · Refines #040 (message center), #044 (job failures become Admin messages)*

- **Decision:**
  - **A message is worked out from what's true now,** not saved when something happens: "Last night's snapshot failed" exists while the snapshot job's latest result is a failure; later, "Oil change due soon" exists while the interval says so. When the cause is fixed, the message goes away by itself.
  - **Each message has a stable key** built from its cause (e.g. `job-failed:nightly-snapshot:<finish time>`), so the same cause is the same message everywhere, and a new failure is a new message.
  - **Only each person's mark is stored:** read and dismissed, per key, through the command pipeline. A dismissed message stays dismissed; a new key shows again.
  - **Who sees a message** follows roles and mode: Admin messages only in Admin mode; asset messages only for assets the person can view.
- **Why:**
  - The plan says messages are "generated locally from synced data", so they work offline with no push infrastructure (Phase 3); derived messages need no message table to sync or clean up.
  - A message can't go stale: one that's still showing is still true.
  - The human chose this over storing each message as a row (which would keep a history but need its own syncing and cleanup).

### 051. A per-vehicle dashboard and a "Ready to use?" summary
*Decided: 2026-10-31 · Plan: §4 Readiness, Squawks, Asset profiles; §5 UI direction; §10 Phase 5 open questions · Refines #040 (the Overview tab)*

- **Decision:**
  - **The vehicle's Overview tab is its dashboard:** "Ready to use?" at the top, then time and distance to each major service, open and deferred squawks (blocking ones first), latest fill-ups, and last activity. The Garage card shows the same readiness as a badge.
  - **Readiness is generic, from profiles:** requirements and official documents can be **needed for use**; profiles set the defaults per kind, and each asset can change them. Each profile lists **operation types** ("ready for what?"), each adding its own items (e.g. IFR, for hire, heavy towing). No kind-specific code.
  - **Squawks can block use,** marked when opened or at any disposition.
  - **Each item shows why** (✓ or ✗, its due date or meter value, a link), with a warning before it's due.
  - **Advisory only:** the summary reflects the records and certifies nothing; for aircraft, the pilot in command decides airworthiness.
  - **Phasing:** the Overview shell in Phase 2 milestone 4; the squawk flag in milestone 7 (it can't be filled in later); the summary, operation types, and service countdowns in Phase 5; aviation items in the aircraft phase; a person's currency (e.g. a pilot's flight review and medical) post-MVP, with credentials.
- **Why:**
  - The human asked for one place per vehicle showing open squawks and what's coming due, and for aircraft, airworthiness for a flight.
  - "Road-ready" and "airworthy" are the same question; making it data keeps aircraft possible without special cases.
  - Readiness depends on the intended use (IFR needs more than VFR), so a single yes/no would be wrong.

### 052. Groups: every asset in exactly one; roles can be granted per group
*Decided: 2026-10-31 · Plan: §4 Asset, Asset lifecycle & components; §5 UI direction; §6 Users, roles & permissions · Phase plan: phase-2.md milestone 4 · Was idea 6 in ideas-to-discuss.md · Extends #045*

- **Decision:**
  - **Every asset is in exactly one group,** the place it lives ("Garage", "Hangar", "Boathouse", "Storage", …). Each group has a **name** and an **icon** from a built-in set; an uploaded logo can come after Phase 2 milestone 8 (images).
  - **A default group,** "Garage" with the garage icon, is created at setup, so every asset always has a group and a one-place household never has to think about it.
  - **Roles can be granted on a group,** as a third grant target next to one asset and all assets (#045 reserved it). A group grant covers every asset in the group, including ones added later. Permissions remain the union of all of a person's grants.
  - **Assets can move** between groups, kept in the asset's history. **Access follows the current group;** before a move is confirmed, the app shows who gains and who loses access.
  - **Components** are in their parent's group while installed and inherit its access; uninstalled, they have their own group.
  - **The household Admin stays global** (users, server, settings). A per-group Admin could come much later.
  - **The Garage screen** shows a section per group; filters can use groups.
  - **Built in Phase 2 milestone 4,** with assets: the groups table, the default group, the asset's group, group grants, moving assets, and a screen to manage groups (SVG options first, #040).
- **Why:**
  - The human wants every asset assigned to a group with its own name and icon, and different permissions per group.
  - One group per asset keeps permissions and the Garage screen easy to follow; an asset that needs different people uses a per-asset grant.
  - Deciding before milestone 4 means the first asset is already in a group, with nothing to move later.

### 053. Messages screen: list and detail, with a divider; the badge and the urgent banner
*Decided: 2026-10-31 · Plan: §5 In-app notifications · Phase plan: phase-2.md step 3e · Sketches: `docs/ui-sketches/p2m3-messages-options.svg` (option B), drawn as `docs/ui-sketches/p2m3-messages-chosen.svg` · Refines #040*

- **Decision:**
  - **List and detail,** like the Users screen (#046): short rows on the left (severity stripe and word, title, time, an unread dot), the chosen message on the right (text, Admin-only details, one action, **Dismiss**). **Opening a message marks it read;** there's no Mark read button. Dismissing it chooses the next one.
  - **A clear divider on wide and medium screens:** a full-height grey line between the columns, with the list on a slightly darker panel.
  - **Phones:** the list, then a page per message with **← Messages**.
  - **Filters:** All, Urgent, Warning, Info (by vehicle once vehicles exist, milestone 4). Newest first, unread on top.
  - **Everywhere:** an unread badge on **Messages** (sidebar and phone tabs; Admin messages count only in Admin mode), and a red **urgent banner** under the top bar on every screen while an urgent message is showing, with **View**.
  - **Severity** is a colour stripe (red urgent, amber warning, blue info) **and** the word, never colour alone.
- **Why:** the human chose option B and asked for a stronger line between the columns on large screens. The list stays tidy as Phase 5 adds maintenance, squawk, and document messages, and it works like Users.

### 054. Test timers: minutes instead of hours, on test images only
*Decided: 2026-11-01 · Phase plan: phase-2.md step 3i · Applies to #044 (jobs), #047 (recovery codes), #049 (idle)*

- **Decision:**
  - **Two locks:** test timers are **built only into test images** (the `test-timers` build option, turned on only by the Dev image workflow for `dev` and `milestone-…` images; never in a release or the main gate's check build), and **switched on only by `VEHICLEMANAGER_TEST_MINUTES`** = N, from 1 to 5. Anything else, or the setting on a release, is ignored with a warning.
  - **What N shortens:** Admin mode's idle limit → N minutes; Sign out when idle 1 h / 4 h / 8 h / 24 h → N / 2N / 3N / 4N minutes (still in order); the Admin keep-alive and the server's "last used" → every 15 seconds; messages reload on an open screen → every minute; the nightly snapshot and the hourly check → every N minutes; recovery codes → N minutes.
  - **Never shortened:** the sign-in attempt limit (it protects against guessing) and the 30-day session.
  - **The server decides and tells the app** with the session (`SessionInfo::timers`), so the browser, the phone, and the desktop app all follow the NAS's setting.
  - **Visible:** a `TEST TIMERS ON` warning at startup, a **TEST TIMERS · N min** tag in the top bar, and each sign-out choice shows its test length.
- **Why:** manual test plans had steps of an hour or a night; the human asked for 1–5 minute timers on milestone test builds. Two locks make sure a real install can't run them.

### 055. Notes are a Markdown subset, shown as plain text until rendering arrives
*Decided: 2026-11-01 · Plan: §3 Event, Squawks, Asset · Phase plan: phase-2.md milestone 4 · Was idea 8 in ideas-to-discuss.md*

- **Decision:**
  - Every free-text notes field (an asset's description first, then event notes, squawks, tasks, documents) is stored as **text in a small Markdown subset:** bold, italics, lists, and links.
  - **For now it's entered and shown as plain text** (the Markdown characters show as typed). A Markdown editor and rendering (screens, PDF and Word reports) come later.
  - Nothing else (no HTML, images, or tables); anything outside the subset shows as typed.
- **Why:** changing the format later would mean converting every note; a subset keeps notes readable as plain text today and renderable later, in reports too (the idea's own suggestion, chosen by the human).

### 056. Vehicles: groups in the sidebar, cards per group, a Sort menu, and adding a vehicle in two steps
*Decided: 2026-11-01 · Plan: §5 UI direction (Home "Garage" screen) · Phase plan: phase-2.md step 4e · Sketches: `docs/ui-sketches/p2m4-garage-options.svg` (option A), drawn as `docs/ui-sketches/p2m4-garage-chosen.svg` · Refines #040*

- **Decision:**
  - **Each group is a sidebar item,** with its icon and name, opening that group's own page (only its vehicles; **+ Add vehicle** starts in it). Above them, **All vehicles** shows every group as a section; it appears only when the person can see more than one group, so a one-group household sees just **Garage**. The screen called "Garage" goes away: "Garage" only ever means the group. The sidebar shows the groups the person can see a vehicle in (every group, for the household's Admin in Admin mode), in name order; on the icon-only sidebar, each group shows its icon with its name as a tooltip. The lower "Vehicles" placeholder becomes **Groups** (managing groups, the household's Admin, step 4g).
  - **Phones:** the first bottom tab is **Vehicles**; a menu at the top picks All vehicles or a group, and the ☰ menu lists the groups too. Each group page has its own address, so bookmarks and Back work per group.
  - **Cards in a grid, a section per group** (its icon, name, and count) on All vehicles. Cards show the name, kind with year, make, and model, and the plate or serial number; photos, readings, due items, and squawks fill them in as milestones 6–8 add them. **Show archived** adds retired and disposed vehicles, dimmed.
  - **A Sort menu** instead of the earlier "urgency or My order" toggle: **Newest activity**, **Name (A–Z)**, or **Custom**. Custom shows **Arrange**, which puts ▲▼ buttons (48 px) on each card until **Done**. The sort applies within each group; groups stay in name order. The sort and the custom order are each person's own, saved like their other settings. **Urgency** joins the menu when vehicles have due items (Phase 5).
  - **+ Add vehicle** (the household's Admin, in Admin mode) is a two-step dialog: the **kind** as large tiles, then the **details** (name, make, model, year, colour, that kind's identifiers, group).
  - **Pinning** vehicles to the top of their group is for later (idea 15).
- **Why:** the human chose option A, asked for the common Sort-menu convention, and asked for the groups in the sidebar (which also ends "Garage" meaning both a screen and a group); cards are where the Garage is heading (#040), and choosing the kind first keeps the details form to that kind's identifiers.

### 057. A vehicle's page (header, tabs, Overview, Setup); stable field keys; calculations in Phase 5
*Decided: 2026-11-01 · Plan: §3 Custom fields, §5 UI direction · Phase plan: phase-2.md step 4f · Sketch: `docs/ui-sketches/p2m4-vehicle-options.svg`, option A · Refines #040, #051*

- **Decision:**
  - **Layout (option A):** a header (name; kind, year, make, model, colour; identifiers and group as chips; status), then the tabs Overview, Timeline, Maintenance, Fuel, Documents, Setup across the top (scrolling sideways on phones). Tabs without content yet say which milestone fills them.
  - **Overview** is the dashboard (#051) as panels: Ready to use? and Coming due (Phase 5), Open squawks (milestone 7), Latest fuel-ups (milestone 6), and the meters (now).
  - **Setup** is one page of sections: Details (Edit details), Meters (+ Add meter; edit, remove), Custom fields (+ Add field; on/off, edit), Required readings (a grid of meters × event kinds), and Group and status (Move to another…, with who gains and loses access shown first; the status; Delete…, with the name typed).
  - **Every custom field has a stable key** (e.g. `trip-ev-distance`) beside its label: from the profile (or later a catalogue) for built-in fields, made from the label (unique on the vehicle) for fields people add. Formulas, reports, comparisons across vehicles, and voice logging (idea 14) refer to fields by key, so renaming a label never breaks them. The key is shown, read-only, on Setup.
  - **Calculated fields** (formulas over built-in and custom fields, unit-aware, worked out when shown and never stored) come in **Phase 5**, with fuel-economy statistics. A **catalogue of ready-made fields and sets** (e.g. "Trip computer (hybrid)": trip distance, time, EV distance, fuel economy) is idea 16.
- **Why:** the human chose option A and asked for richer trip-computer fields and calculations; keys cost little before any field has values and would mean backfilling every vehicle later.


### 058. Groups screen (list and detail) and roles as a list with + Add role
*Decided: 2026-11-01 · Plan: §6 Roles · Phase plan: phase-2.md step 4g · Sketch: `docs/ui-sketches/p2m4-groups-roles-options.svg` (Groups option B, roles option A) · Refines #046, #052*

- **Decision:**
  - **Groups** is a screen under Manage (the household's Admin, in Admin mode), laid out like Users: the groups on the left (icon, name, count; **+ Add group**), the chosen group on the right: its **name and icon** (saved when changed), its **vehicles** (each opens its page), **who sees it** (each person's role there and where it comes from: all vehicles, this group, or one of its vehicles), and **Delete group**, only for an empty group that isn't the last (otherwise greyed out with the reason). On phones: the list, then a page per group. Moving a vehicle stays on its own Setup tab, with its preview (#057).
  - **Roles on the Users screen** are a list of the roles the person has, each with its role menu and ✕: on **all vehicles**, **a group**, or **one vehicle**. **+ Add role** opens a dialog: what it's on (all vehicles, a group, or a vehicle) and the role; adding one where they already have a role replaces it. A note says roles add up: on each vehicle, the highest applies.
  - The list of users summarises each person's roles (e.g. "Admin · all vehicles", or "Maintainer · Hangar" when they have one role elsewhere, or "3 roles").
- **Why:** the human chose as recommended: seeing who can see a group belongs next to the group, and a short list of roles avoids menus that look like they lower a role but can't (the highest role always applies).

### 059. Location: our own geocoder on a GeoPackage built with GDAL; saved places are each person's aliases
*Decided: 2026-11-02 · Plan: §3 Event location, Saved places, Reverse geocoding · Phase plan: phase-2.md milestone 5 · Refines #038*

- **Decision:**
  - **The geocoder is our own,** with only what's needed: the containing country and state or province, the nearest boundary when a point is outside every polygon (#038), and the nearest city with its **IANA time zone** (milestone 6 needs one on every event). **Lookups go through a spatial index** (R-tree), never a scan.
  - **The data file is a GeoPackage** (`.gpkg`, the OGC standard): a SQLite database with three layers (countries, states and provinces, cities), each with its R-tree index, and a metadata table (data version, sources, checksums, licences). The server reads it read-only through `sqlx`; no new dependency.
  - **It's built in a short-lived Docker stage** from the official GDAL image (pinned version and digest): pinned GeoNames **`cities500`** and Natural Earth 1:10m downloads, refused unless their SHA-256 matches, then `ogr2ogr`. Only the `.gpkg` reaches the runtime image; GDAL never runs on the NAS. A development script does the same with `docker run --rm`. The file is never committed.
  - **Cities: GeoNames `cities500`** (every place with at least 500 people), changed from #038's `cities1000`: the raw file grows from about 32 MB to 41 MB, for better accuracy in small places.
  - **If the file is missing or broken,** the server still starts; location falls back to saved places and typed entry, and an Admin message says so.
  - **Saved places are each person's own and act as aliases:** a name, coordinates, a radius (150 m by default, 25 m–2 km), and the city, state, and country. Choosing one, or being inside its radius, copies all of it into the event, so editing or removing a place never changes past events. Each person manages their own; no role is needed.
  - **Places** is a screen under Manage for everyone; attribution goes in an **About** section on Diagnostics.
- **Why:**
  - The human asked for our own lookup with a good spatial index, and for common file formats where possible. GeoPackage is the standard, opens in QGIS and GDAL, and is SQLite, which the server already reads.
  - SpatiaLite needs a native extension; FlatGeobuf needs one file per layer and two new crates; GeoJSON has no index; GDAL is the reference tool for writing GeoPackages, and a build stage keeps it off the NAS.
  - `cities500`: the human's choice; it names more small places correctly, and the size is fine on the NAS (phones are post-MVP, #038's budget).
  - Saved places as each person's aliases: the human's choice; copying keeps events unchanged when places change.

### 060. Places: one list with a place dialog that puts "where" first
*Decided: 2026-11-02 · Plan: §3 Event location, Saved places · Phase plan: phase-2.md step 5f · Sketch: `docs/ui-sketches/p2m5-places-options.svg` (Places A, dialog A) · Refines #059*

- **Decision:**
  - **Places** (under Manage, for everyone): one list of the person's saved places, each with its name, city, state, and country, its radius, **Edit**, and **Remove**; **+ Add place**. Phones show the same list; tapping a place edits it.
  - **The place dialog is the location picker** milestone 6's events reuse, in three parts: **1 · Where** (**Use my location**, or **Type coordinates**, with the accuracy and age of the fix), **2 · Place** (the suggestion fills city, state or province, and country, all editable; any city or state can be typed), **3 · Name and radius**.
  - **What it can say:** offshore ("nearest is …, check it"); inside a saved place ("You're at Home garage; its city, state, and country are used"); place lookup unavailable ("type the city, state, and country").
  - **About** on Diagnostics: the app version, the place data version (or "not available"), and the GeoNames and Natural Earth credits.
  - **Not built from the sketch:** its "typed" mark on places whose city isn't from the place data. A saved place doesn't record where its city came from, and the city as typed already shows.
- **Why:** the human chose as recommended: places are short lists, one dialog serves adding, editing, and later events, and putting "where" first follows the plan's fallback order and shows where each value came from.

### 061. The location picker offers nearby places, with their distance, direction, and type
*Decided: 2026-11-02 · Plan: §3 Reverse geocoding · Phase plan: phase-2.md step 5f.2 · Refines #059, #060*

- **Decision:**
  - After **Use my location** or typed coordinates, the picker lists **up to 5 nearby places** in the point's country, **within 25 km**, each with its **name**, **distance and compass direction** (eight points: N, NE, E, …, in the person's miles or kilometers), and **type**. Choosing one fills **City or town**; the state or province and country still come from the boundaries at the point. Typing any city still works.
  - **The default choice is the nearest place that isn't a district;** districts stay in the list, labelled. A district is GeoNames `PPLX`, **or a numbered part of a bigger place within 25 km**: its name, a space, then a digit (e.g. "Paris 04 Hôtel-de-Ville" beside Paris, "Lyon 3e" beside Lyon), because GeoNames codes some cities' districts as ordinary places (asked for during the test run).
  - **Types,** from GeoNames' feature code and population (population only approximates size): **Capital** (`PPLC`), **District** (`PPLX`), **Locality** (`PPLL`), otherwise **City** (100,000+ people), **Town** (10,000+), or **Village**; a seat of an administrative division (`PPLA`–`PPLA4`) adds its role, e.g. "Town · county seat".
  - **The data gains each place's feature code and population, and leaves out abandoned, destroyed, and historical places** (`PPLQ`, `PPLW`, `PPLH`, `PPLCH`), which could otherwise be "the nearest town". The data version becomes **`2026-11-02.2`** (same sources, new build); the file's format version becomes 2.
  - **Layout:** option A of `docs/ui-sketches/p2m5-nearby-options.svg`: a list of tappable rows above **City or town** ("Nearby: choose the city or town"), the chosen one marked; typing over the field still works.
  - **Where's buttons show the choice:** **Use my location** and **Type coordinates** are both plain until one is chosen; the chosen one is highlighted (asked for during the test run).
- **Why:** the human asked for choices on "my location"; the nearest GeoNames place can be a district in a big city or a small locality, and seeing the few nearest, labelled, lets the person pick the name they use.

### 062. A test server beside the everyday ("main") server; migrations freeze in `dev`
*Decided: 2026-11-03 · Plan: §7 Deployment · Phase plan: phase-2.md, between milestones 6 and 7 · Closes idea 20 · Refines #026, #054*

- **Decision:**
  - **Two servers on the NAS, each its own Container Manager Project with its own data folder:**
    - **Main**, the one the household uses: `/volume1/docker/vehicle_manager`, port 8410, `https://<nas>.<tailnet>.ts.net`. It runs **`:dev`**.
    - **Test**: `/volume1/docker/vehicle_manager_test`, port 8411, `https://<nas>.<tailnet>.ts.net:8443`. It runs **`:milestone-<name>`** images (or `:dev`, or a `:sha-` build), and test plans run on it from now on.
    - **Today's install becomes the test server** (its test vehicles and events with it); **main starts empty**, with a new Admin.
  - **`:dev` is built without test timers;** milestone images keep them (#054). So main can't run test timers even by mistake, and test timers stay a test-server thing.
  - **A server instance name,** `VEHICLEMANAGER_INSTANCE` (empty on main; `test` on the test server):
    - it names the session cookie (`vehiclemanager_session` on main, `vehiclemanager_test_session` on test). Both servers answer on the same NAS hostname, and browsers share cookies across ports, so they'd otherwise sign each other out;
    - the app shows a **TEST SERVER** tag in the top bar and on the sign-in card, and the browser tab's title says "(test)".
  - **The test server keeps its own data.** When a test needs real data, a documented step copies main's newest nightly snapshot in (it stays on the NAS). The copy keeps main's database ID, which doesn't matter until sync arrives (Phase 3).
  - **Migrations freeze once merged into `dev`:** main runs `dev`, so a merged migration never changes. One on a milestone branch may still change until its PR merges. This replaces "may change until v0.2.0". **Existing migration files keep their old header comment:** sqlx checksums each applied migration, comments included, so editing one would make existing databases refuse to open. New migrations start with: `-- Final once merged into dev (decision #062).`
- **Why:** the app is close to everyday use (the human, 2026-11-03). Tests write records that can't be deleted (finalized events are only corrected), and test timers change how a server behaves; neither belongs on the household's server. Waiting for the v0.2.0 release would keep Phase 2's work out of real use until the end of the phase.
- **Unaffected:** releases (`:<version>`, `:latest`) and the release process; `main` still holds released code only.

### 063. Squawks have a severity: Low, Medium, or High; High keeps the asset out of use
*Decided: 2026-11-03 · Plan: §3 Squawks, Readiness · Phase plan: phase-2.md milestone 7, choice 1 · Refines #051 · Raised by sub-plan 02 (voice capture)*

- **Decision:**
  - **Every squawk has a severity: Low, Medium, or High.** **A new squawk starts at Low**: a squawk doesn't keep an asset out of use unless someone says so.
  - **High keeps the asset out of use** (what aviation calls a grounding squawk). It replaces #051's yes-or-no "blocks use": Phase 5's readiness summary counts High squawks.
  - **Each kind's profile names High:** "Not drivable" (car, truck, motorcycle), "Not towable" (trailer), "Out of service" (generator, other equipment); aircraft (later) "Grounding". Shown as e.g. "High · Not drivable". A new kind brings its own word.
  - **Severity can change** when the squawk is opened and at any disposition (signed off or deferred), and each change is kept in its history.
- **Why:** the human, 2026-11-03: "A squawk should not block by default. In aviation there are 'grounding' squawks. I like low/medium/high severity with high as 'grounding'." Sub-plan 02 had asked whether a spoken squawk should carry a severity; three levels answer that too, and one of them is what readiness needs.
- **Unaffected:** the required disposition at every maintenance or repair (plan §3), and deferrals not carrying over.

### 064. The household chooses what squawks are called: Issues (the default), Squawks, Faults, or Write-ups
*Decided: 2026-11-03 · Plan: §3 Squawks, §5 Preferences · Phase plan: phase-2.md milestone 7, choice 13 · Refines #051, #063*

- **Decision:**
  - **The Admin chooses one word for the whole household,** in Settings' household section: **Issues** (the default for a new household), **Squawks**, **Faults**, or **Write-ups**. People can't choose their own; every screen, message, and button uses the household's word.
  - **Only the words on screen change.** The code, the API, the database, and these documents keep saying "squawk" (AGENTS.md terminology).
  - Messages that name them get the word, its plural, a capitalized form, and its article ("an issue", "a squawk") from the translator, so no screen has to pass it.
- **Why:** the human, 2026-11-03: "Squawk is familiar in the aviation community, but might not be outside of it." One household-wide word keeps lists that mix vehicles consistent; per-kind words were considered and turned down.
- **Unaffected:** what squawks are and do (#051, #063).

### 065. Photos and documents are files on disk, each named by its SHA-256; the database holds what they are
*Decided: 2026-11-04 · Plan: §4 Asset, Attachment; §7 Data & operations · Phase plan: phase-2.md milestone 8, question 1 · Closes idea 3 · Refines #035, #036*

- **Decision:**
  - **Every stored file** (a photo's JPEG and thumbnail, a kept original, a PDF and its first-page thumbnail) **is a file in `data/files/`,** named by the SHA-256 of its contents (`data/files/ab/cdef…`), behind a small storage interface in `storage`. It's written once and **never changed**; the same file attached twice is stored once.
  - **The database holds the attachment records** (owner, kind, size, page count, original name, photo metadata) and the hashes they point to.
  - **The integrity check** verifies files against their hashes, and reports a missing or damaged one as an Admin message (#050).
  - **A file is erased only when nothing points to it any more** (removed attachments are soft-deleted, #043).
  - **Backups and restores are the database plus `data/files/`.** Nightly snapshots stay database-only (#035): files never change, so the folder needs no copies of its own. Copying main into the test server (#062) copies both.
- **Why:** the plan kept photos as blobs in the database, but every nightly snapshot copies the whole database and 7 are kept, so each photo and PDF would take about 8× its size on the NAS. Files named by their hash keep snapshots small, make damage detectable, and can move to other storage later without touching the records.
- **Unaffected:** the photo sizes, originals, and metadata (#036); the PDF limit and viewer (#037); lazy blob sync (Phase 3).

### 066. License plates are their own records, with an issuing authority and a history
*Decided: 2026-11-04 · Plan: §3 Official asset documents; §4 Asset · Phase plan: phase-2.md milestone 8, question 5*

- **Decision:**
  - **A plate is a record on the vehicle:** an issuing authority (a US state or Canadian province from a list, a country, or other text), a number, and from / until dates. **The current plate** shows on the vehicle's page; **earlier ones are its history,** e.g. after a move to another state.
  - **A registration links to the plate it's for;** renewing the registration updates that plate's expiration.
  - **Plates only, for now.** Other identifiers (VIN, serial, a boat's registration number) stay typed identifiers; the record is general enough to add other kinds later.
  - **Existing plates move over:** a migration turns each `plate` identifier into a vehicle's first plate record, with **no authority yet**, and the vehicle's page asks for one.
- **Why:** the human, 2026-11-03: a plate belongs to a state and changes when the vehicle moves; a typed identifier has neither an authority nor a history.
- **Unaffected:** the other typed identifiers (#030).
