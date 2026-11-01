# Ideas to Discuss

*Companion to `docs/plan.md` · started October 19, 2026*

Ideas that have been raised but **not yet discussed or decided.** Nothing here is part of the plan until it's talked through and moved into `docs/plan.md`, with the decision recorded in `docs/decisions.md`.

> **For agents:** do not implement anything from this file. If a task touches one of these ideas, point it out to the human and ask.

**Status key:** 🟡 proposed (not yet discussed) · 🔵 under discussion · ✅ decided (move to the plan, then remove from here) · ❌ dropped

---

## Future-proofing

Proposed to keep early phases from blocking later growth. Each is cheap now and expensive to retrofit.

### 1. Don't let old app versions erase new data ✅
*Decided 2026-10-29: `docs/decisions.md` #043 (field-level commands, last-write-wins per field, unknown fields and types kept).*

### 2. Feature negotiation in the handshake 🟡
*Suggested phase: Phase 1 (extends the existing version handshake)*

- Besides version numbers, client and server exchange a **list of supported features.**
- New features can roll out gradually; older clients hide what they can't handle instead of erroring.

### 3. Separate, content-addressed blob storage 🟡
*Suggested phase: Phase 2*

- Photos and documents stored in their own table, **identified by a content hash (SHA-256),** behind a small storage interface. Attachment records hold the metadata (owner, kind, EXIF).
- **Benefits:**
  - Deduplication: the same receipt attached twice is stored once.
  - Integrity checks: a damaged file is detectable.
  - Easy to move later: if multi-page documents make the database too large, files can move to disk or object storage without touching anything else.
- **To discuss:** keep blobs in SQLite at first (current plan) vs. files on disk from the start.

### 4. One canonical interchange format, defined early 🟡
*Suggested phase: Phase 2*

- A single versioned, documented data format used for **backups, full-archive exports, the next-owner transfer package, imports, and test fixtures,** instead of inventing one per feature.
- Unlocks **importers from other apps** (spreadsheets, other maintenance apps) as a later feature.
- **To discuss:** format choice (e.g. JSON with a published schema), and how it's versioned alongside the sync command format.

### 5. Time-ordered IDs (UUIDv7) ✅
*Decided 2026-10-29: `docs/decisions.md` #042 (UUIDv7 now; human-friendly record numbers later, with reports or logbooks).*

### 6. Groups ("garages" or "hangars") ✅
*Decided 2026-10-31: `docs/decisions.md` #052 (every asset in exactly one group; roles per group; built in Phase 2 milestone 4).*

### 7. One layered settings model 🟡
*Suggested phase: Phase 2*

- A single settings system with layers: **server → group → user → device → asset;** more specific settings override broader ones.
- Would also answer the open Phase 2 question about display preferences (units, currency, language, date format).
- **Partly decided (#029):** display preferences use two layers, server defaults and per-user overrides, one record per setting with last-write-wins by HLC.
- **To discuss:** which settings live at which layers by default.

### 8. Notes format 🟡
*Suggested phase: Phase 2*

- Decide between **plain text** and a small **Markdown subset** (bold, lists, links) for notes.
- Affects storage, search, and how notes render in PDF and Word reports; changing later means converting every existing note.
- **Suggestion:** a Markdown subset.

### 9. One background job system on the server ✅
*Decided 2026-10-29: `docs/decisions.md` #044.*

---

## Other ideas

*(Add new ideas here as they come up.)*

### 10. Desktop app remembers its window size 🟡
*Raised by the human, 2026-10-29, during the milestone 4 desktop test · Suggested phase: undecided (whenever desktop settings grow, or Phase 6 Polish)*

- The desktop app's `settings.json` (in the data folder, `app::settings`) should remember the **window size** between launches, and probably its position.
- **To discuss:**
  - size only, or position and maximized state too, and what happens when the saved position is on a monitor that's no longer connected;
  - when to save (on resize, debounced, or on close);
  - whether it belongs in the layered settings model (idea 7) as a device-only setting.

### 11. A maintenance tool for the NAS 🟡
*Raised by the human, 2026-10-30, during the Phase 2 milestone 1 NAS test · Suggested phase: undecided (before real data is on the NAS would be ideal, e.g. late Phase 2)*

- **Idea:** one command-line tool, kept in the Project folder (`/volume1/docker/vehicle_manager/`), for the NAS maintenance tasks that are now copy-and-paste command sequences in `docs/deploying-nas.md`:
  - restore a snapshot (stop, move the database aside, copy the snapshot in, fix the owner, start, check the log);
  - restore a pre-upgrade backup after a rollback;
  - update and roll back the image (`.env` tag, `pull`, `up -d`);
  - show status: container state, database ID, last snapshot, last job results, health;
  - list snapshots and backups, and clean up old backups and `data/replaced/` folders.
- **Why:** fewer manual steps where a typo could lose data (e.g. moving the wrong file, forgetting the `-wal` file or the owner), and each task checked the same way every time.
- **To discuss:**
  - **Language:** Python (the human's suggestion; DSM 7 includes a system Python 3, but its version is DSM's choice and can change with DSM updates), a POSIX shell script (always there, no version risk, but harder to test), or **a subcommand of the server binary itself**, run with `docker exec` or `docker compose run` (reuses the Rust code for snapshots and integrity checks, and is versioned with the image; the stop/start steps would still need the host).
  - **How it's installed and kept up to date:** downloaded like `compose.yaml`, or shipped inside the image and copied out.
  - **Testing:** how it's tested in CI without a NAS (e.g. against a fake Project folder and a stub `docker`).
  - **Safety:** always confirm before replacing the database, keep what it replaces, and refuse to run while the container is up for steps that need it stopped.

### 12. Digital records and signatures that follow standards 🔵
*Raised by the human, 2026-10-31 · Plan received 2026-10-31: `docs/sub-plans/01-signatures-and-records.md` (draft) · Suggested phase: before signatures are built (post-MVP logbook entries), and checked before Phase 2 milestone 6 (finalized records and amendments)*

- **The plan:** `docs/sub-plans/01-signatures-and-records.md`, the human's draft design for electronic signatures and record-keeping that meet the intent of FAA AC 120-78B and 14 CFR 43.9 / 43.11 / 91.417 as far as practical, with zero paid dependencies: Ed25519 signatures over canonical (RFC 8785) records, a self-hosted root → intermediate → per-user-and-device key certificate chain, a per-vehicle hash chain, corrections by superseding entries, revocation, a signed export bundle with a verifier CLI, and a PDF logbook. Its phases are S1–S6, with its own open decisions (§13).
- **Rule until it's confirmed:** **don't implement digital signatures** (or anything from the sub-plan) until it's reviewed with the human and they **confirm** the approach. The sub-plan replaces the Signatures part of "Logbook entries, statement templates & signatures" in `docs/plan.md` once confirmed.
- **To review with the human** (where it meets settled decisions; nothing decided yet):
  - **Signatures become cryptographic:** `docs/plan.md` currently says a signature "is not a cryptographic signature"; the sub-plan makes the drawn or typed mark only the *appearance*, with an Ed25519 signature behind it.
  - **New crate and dependencies:** a `vm-records` crate (AGENTS.md has four crates with one-way dependencies) and `ed25519-dalek`, `serde_jcs`, `keyring`, `hex` (all need approval); its sample code uses `OffsetDateTime` where `domain` uses `UnixMillis`.
  - **Immutability:** SQLite triggers that block updates to signed rows, alongside the command pipeline (#043: field-level last-write-wins, deletes as tombstones); signed entries as content-addressed and conflict-free.
  - **Milestone 6:** its finalized records and amendments (#034) next to the sub-plan's `draft → signed → superseded` states and correction entries, so the two fit together.
  - **Sync and time:** per-vehicle hash chains with server-signed chain-join entries for offline forks, server receipts (`received_at`), and device time as untrusted, versus the HLC and Phase 3 sync design.
  - **Where S1–S6 go in the roadmap,** the `docs/records-procedures.md` template, and the tests it asks to add to AGENTS.md (§12).
  - Its "Related" line points to `vehicle-maintenance-plan.md`; in this repo that's `docs/plan.md`.

### 13. Optional email and email password reset 🔵
*Raised by the human, 2026-10-31 · Plan: `docs/sub-plans/03-email-and-password-reset.md` (draft) · Suggested phase: "early phases" for the reset and SMTP setup, "approaching MVP" for email notifications (the sub-plan's own phasing; not yet placed in the roadmap)*

- **The plan:** email stays **optional** (the app works fully without it), any SMTP provider (Gmail with an app password as the reference), configured by environment variables or Docker secrets; an email reset link (token stored only as a hash, about 30 minutes, one use, the same generic answer for everyone, rate-limited, other sessions ended); a per-person email on/off switch and a "send test email" button; later, email notifications for maintenance due, with per-person preferences.
- **Rule until it's confirmed:** don't build anything from it until it's reviewed with the human and they confirm it.
- **To review with the human** (where it meets what's built or decided; nothing decided yet):
  - **Recovery today (#046, #047):** an Admin resets anyone's password with a **temporary password** (the sub-plan's path 2 says a one-time code); only an **Admin** can recover their own with the log code (path 1 reads as if anyone can). The sub-plan's reset by **username or email** needs an **email field on users**, which doesn't exist yet.
  - **Codes in log files (worth settling soon):** the setup and recovery codes go to the server log, which is also written to `data/logs/` and may be shared when debugging. The sub-plan asks for them to be **redacted from exported logs or written to a separate file**; today the test plans only say to blank them out before sharing.
  - **New dependency:** `lettre` (needs approval); SMTP passwords kept out of the database.
  - **Email notifications vs. #050:** messages are derived from the current state and nothing records that one was "sent", so emailing each new message once needs a small record of what was emailed to whom.
  - **Base URL** for links (the tailnet name vs. a local address) and **host-mode desktop** installs are the sub-plan's own open questions.
  - Its first line points to `vehicle-maintenance-plan.md`; in this repo that's `docs/plan.md`.

### 14. Voice logging (hands-free fuel-ups, readings, and squawks) 🔵
*Raised by the human, 2026-10-31 · Plan: `docs/sub-plans/02-voice-capture.md` (draft v2) · Suggested phase: not placed yet; its phase 1 (a typed, text-only dialog engine) has no dependency on mobile, but most of the rest needs the mobile apps (post-MVP, #041)*

- **The plan:** opt-in voice logging that never starts by itself (driving detection only *suggests* a "Voice log" chip); on-device speech (`whisper-rs`, `tts`, `cpal`); a guided dialog (vehicle first, then that vehicle's fields, with "cancel", "repeat", "go back", "skip", "options"); a Rust dialog engine that's text in, text out, so it's testable without audio; entries saved as **drafts** with the raw transcript, reviewed on a **Voice drafts** list.
- **Rule until it's confirmed:** don't build anything from it until it's reviewed with the human and they confirm it.
- **To review with the human** (where it meets what's built or decided; nothing decided yet):
  - **Drafts and finalizing (#034):** fuel-ups and readings are finalized **automatically after 24 hours** by the server; voice drafts waiting for review would be finalized too unless they're treated differently. The sub-plan says drafts are "signed later", but signatures are on hold (idea 12) and fuel-ups aren't signed.
  - **Per-vehicle fields:** these are the asset profiles, required readings, and custom fields (#030, milestone 4), plus per-asset overrides; one definition should drive forms, voice, and review, as the sub-plan says.
  - **Naming:** its sketch uses `VehicleId`; AGENTS.md requires `AssetId` in code.
  - **Groundwork for Phase 2:** recording each event's **source** (typed, voice, later import) is cheap when events are created (milestone 6) and hard to add later; a squawk's "ground it vs. monitor" maps to **blocks use** (#051, milestone 7).
  - **Where it runs:** until the mobile apps, phones use the **web UI**; on-device Whisper in a phone browser isn't covered (and the browser's own speech recognition usually isn't offline). Desktop works first.
  - **New dependencies** (`whisper-rs`, `tts`, `cpal`) and a downloadable speech model (tens to hundreds of MB) need approval.
  - **"Voice drafts" on the dashboard:** which screen (Garage, the vehicle's Overview, Messages, or + Log), and its relation to the forgotten-job reminder (milestone 7).
  - Its own open questions (§14): push-to-talk, model size, keeping audio clips, squawk severity, and more.

