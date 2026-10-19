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
