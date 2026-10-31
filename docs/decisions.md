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
*Decided: 2026-10-29 · Plan: §3 Reverse geocoding, §10 Phase 2 open questions (geocoder dataset; geocoding on device vs. server)*

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

