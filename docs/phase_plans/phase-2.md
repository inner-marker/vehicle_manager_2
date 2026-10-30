# Phase 2: Data model & server storage (plan)

*Draft 2026-10-29, **awaiting approval** · Roadmap: `docs/plan.md` §9 Phase 2 · Open questions: none (decisions #028–#041)*

**Goal:** the household can really use the app, online, from the web UI on the NAS (phones and computers) and the macOS desktop app. Vehicles, meters, fuel-ups, readings, maintenance, repairs, squawks, photos, and documents are stored in the server's SQLite database, every change goes through a command, and the data survives restarts, updates, and a bad day (integrity checks and nightly snapshots).

**Not in Phase 2:** offline use and sync (Phase 3), what's due and fuel economy statistics (Phase 5), the mobile apps (post-MVP, #041).

**Exit criteria (proposed):**
- On the NAS, signed in as real users: add the household's vehicles, then log a fuel-up, a reading, a maintenance event with tasks and a squawk sign-off, and a repair, each with a location and photos, from a phone browser and the desktop app.
- Wrong roles are refused (a Driver can't log maintenance), finalized events can only be amended, and a wrong password is rejected cleanly.
- After a container restart and an image update, everything is still there; the nightly snapshot exists and passes its integrity check.

**How this file works:** the same as `phase-1.md` (decision #021).
- One branch and **one PR into `dev` per milestone** (`milestone/p2-mN-<slug>`), merged with a merge commit.
- Each **step** is one Conventional Commit. Steps are planned in detail, and approved, when their milestone starts.
- Screens get **SVG layout options** in `docs/ui-sketches/` before they're built (#040).
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | What it delivers | You can check on the NAS | Likely new dependencies (approved per step) |
|---|---|---|---|---|---|
| 1 | **Database foundation** | ⬜ | SQLite master database in `data/`; migration framework (numbered, forward-only) with a fixture test; database ID; startup integrity check; HLC in `domain`; the **command pipeline** (validate in `domain`, apply in `server`, HLC last-write-wins, soft deletes); nightly `VACUUM INTO` snapshot, keep 7, configurable folder (#035), run on a nightly timer (whether that becomes one general job system is idea 9, asked first) | The database and a snapshot appear in `data/`; a restart keeps them; a corrupted copy is detected at startup | `sqlx` (SQLite), `uuid` (v7 IDs, if approved) |
| 2 | **Users, login & roles** | ⬜ | Users, argon2 passwords, session cookies, rate-limited login, first-Admin setup code (#028); **retires the test credential** (#024); per-asset roles and active mode with the Admin-mode confirmation; desktop sign-in; settings records for server defaults and per-user preferences (#029); locale formatting of dates, numbers, and units | Create the Admin with the setup code; add a second user; sign in on web and desktop; a wrong password and a Driver doing Admin work are refused | `argon2`, a rate limiter (e.g. `tower_governor`) |
| 3 | **Message center** | ⬜ | Inbox with severity, read/dismiss, one action per message, filters (#040); system messages from earlier milestones (snapshot failed, integrity problem, "app update required") | A forced snapshot failure shows an Admin message; mark read and dismiss work | none expected |
| 4 | **Assets, profiles & meters** | ⬜ | The six profiles as seed data (#030); assets with typed identifiers, lifecycle status, parent field, archive; meters (distance/duration); custom field definitions including quantity types (#030); **Garage** screen (urgency order, My order) and **vehicle detail** shell with tabs (#040) | Add the real vehicles with their meters; the Garage cards show them | `ron` (profile seed files) |
| 5 | **Location** | ⬜ | Coordinates (degrees × 10⁷, accuracy, fix time), required city/state/country; browser location; saved places; the offline geocoder with cities1000, Natural Earth 1:10m, and the nearest-boundary fallback (#038); geocoder data file and attribution | From a phone browser, a test location resolves to the right city and state, including a coastal point; saved places work on desktop | a geocoder crate or our own lookup (chosen in the step plan) |
| 6 | **Events: fuel-ups & readings** | ⬜ | The event spine (UTC + time zone, location, readings, notes, multi-day dates, performed-by vs. logged-by); drafts, **Finalize now** with confirmation, automatic finalizing after 24 h by the server, amendments (#034); monotonic readings and required readings per event type; fuel-ups with highway/towing/hauling %, towed vehicles, vehicle-reported fuel economy, trip fields (#030, #033); **+ Log** and quick logging (#040); Timeline tab | Log fuel-ups and readings from the phone; a lower odometer is refused; an event locks after 24 h and can then only be amended | none expected |
| 7 | **Maintenance, repairs & squawks** | ⬜ | Maintenance and repair events with tasks, labor, parts and labor cost (#032); complaint and cause; **Finish** with checks and confirmation; squawks with sign-off and required disposition; requirements with interval rules (meter, calendar with end-of-month, fixed date) and service bundles as data, "Copy schedule from another asset" (#031); forgotten-job reminder after 14 days | Log an oil change with two tasks, sign off a squawk, log a repair with complaint and cause; Finish refuses while a squawk is undispositioned | none expected |
| 8 | **Photos & documents** | ⬜ | Photo upload with the Small/Medium/Large picker, thumbnails, metadata fields, gated originals (#036); PDF upload with page count and the pdf.js viewer (#037); attachments on events, squawks, and assets; **official asset documents** (title, registration, insurance) with renewal history; Documents tab | Take a Large damage photo and a Small meter photo on the phone; open a PDF on web and desktop | `image` (+ HEIC support), pdf.js (bundled asset) |
| 9 | **Asset lifecycle** | ⬜ | Acquisition and disposal records with attachments; status changes; send-out / return tracking; archive instead of delete; aircraft-ready groundwork that isn't already in place (reference documents, one-time and recurring requirements) | Send a vehicle out for service and return it; archive one and find it again | none expected |

**Releases:** propose `v0.2.0` after milestone 2 (first release with real login) and `v0.3.0` at the end of Phase 2, each following `docs/releasing.md`. Or one release at the end; your call.

---

## Why this order

- **1 before everything:** every later milestone stores data through the command pipeline and migrations, so they come first, with the safety net (integrity check, snapshots) from day one.
- **2 next:** real users and roles replace the test credential before any real data is entered, so nothing is ever stored without an owner and a permission check.
- **3 early and small:** later milestones send messages (snapshot failures, forgotten jobs), so the inbox exists before they need it.
- **5 before 6:** every event requires a location, so locations exist before the first event.
- **6 before 7:** fuel-ups and readings are the simplest events and prove the spine, finalizing, and amendments; maintenance builds on them.
- **8 and 9 last:** attachments and lifecycle hang off assets and events that already exist.

## To decide when each milestone is planned

- **ID format:** UUIDv7 (idea 5 in `docs/ideas-to-discuss.md`) or plain integers. Needed in milestone 1, since IDs are hard to change later. **This is an undecided idea; I'll ask before milestone 1 starts.**
- **Other ideas that touch Phase 2** (`docs/ideas-to-discuss.md`): field-level commands (idea 1, milestone 1), content-addressed blob storage (idea 3, milestone 8), groups (idea 6), notes format (idea 8, milestone 6), and the background job system (idea 9, milestone 1). Each is raised with you before the milestone it affects, as AGENTS.md requires.
