# Ideas to Discuss

*Companion to `docs/plan.md` · started October 19, 2026*

Ideas that have been raised but **not yet discussed or decided.** Nothing here is part of the plan until it's talked through and moved into `docs/plan.md`, with the decision recorded in `docs/decisions.md`.

> **For agents:** do not implement anything from this file. If a task touches one of these ideas, point it out to the human and ask.

**Status key:** 🟡 proposed (not yet discussed) · 🔵 under discussion · ✅ decided (move to the plan, then remove from here) · ❌ dropped

---

## Future-proofing

Proposed to keep early phases from blocking later growth. Each is cheap now and expensive to retrofit.

### 1. Don't let old app versions erase new data 🟡
*Suggested phase: Phase 2 (important)*

- **Risk:** a device running an older app version edits a record. If its sync command sends the whole record as it understands it, the server overwrites fields the old version doesn't know about, **silently deleting newer data.**
- **Proposal:**
  - Sync commands carry **only the fields that changed** (field-level updates), not whole records.
  - Clients **preserve unknown fields** they receive instead of dropping them.
  - An app that receives an **unknown event type** shows "Unsupported record. Update the app to view" rather than failing.
- **Why now:** this protects the "multiple app versions at once" requirement, and the command format is very hard to change after release.
- **To discuss:** how field-level updates interact with last-write-wins (per-field HLC vs. per-record HLC).

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

### 5. Time-ordered IDs (UUIDv7) 🟡
*Suggested phase: Phase 2*

- Still generated on the device, but **sorted by creation time** (UUIDv7 instead of random UUIDv4), which makes SQLite indexes faster and records easier to debug.
- **Optional:** human-friendly record numbers for paperwork (e.g. "Entry 2026-0042"), assigned by the server when an event finalizes, since devices can't safely number records while offline.
- **To discuss:** whether human-friendly numbers are wanted, and per asset or global.

### 6. Groups ("garages" or "hangars") 🟡
*Suggested phase: Phase 2 or 3, before multi-user permissions see heavy use*

- Assets belong to one or more **groups,** and roles can be granted per group as well as per asset.
- Covers a family garage now, and later a flying club, co-ownership, or a shop, without redesigning permissions.
- **To discuss:** how group roles combine with per-asset roles and component permission inheritance.

### 7. One layered settings model 🟡
*Suggested phase: Phase 2*

- A single settings system with layers: **server → group → user → device → asset;** more specific settings override broader ones.
- Would also answer the open Phase 2 question about display preferences (units, currency, language, date format).
- **To discuss:** which settings live at which layers by default.

### 8. Notes format 🟡
*Suggested phase: Phase 2*

- Decide between **plain text** and a small **Markdown subset** (bold, lists, links) for notes.
- Affects storage, search, and how notes render in PDF and Word reports; changing later means converting every existing note.
- **Suggestion:** a Markdown subset.

### 9. One background job system on the server 🟡
*Suggested phase: Phase 2 (when nightly snapshots arrive)*

- Nightly snapshots, overdue-return and document-expiration checks, re-geocoding, integrity checks, and later reference data refreshes all run through **one scheduler,** with a status page and logging.
- Avoids several ad-hoc timers.
- **To discuss:** where job status is shown (Admin mode in the web UI, message center for failures).

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
