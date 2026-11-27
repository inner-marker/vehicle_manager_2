# Phase 6: Polish (plan)

*Agreed 2026-11-20 · Roadmap: `docs/plan.md` §9 Phase 6 · Open questions: settled (decision #089) · Builds on #041, #071, #072, #081*

**Goal:** the app is **ready for everyday use by anyone in the household, on any computer.** The rough edges found in earlier test runs are fixed, logging is quicker, costs and history can be reported and printed, data can be exported and brought back, one computer can use the app with no server at all, and signed desktop packages are published with each release.

**Who it's for:** the web app and the desktop app alike. New kinds of change work offline on the desktop and sync, as before (#071, #072).

**Not in Phase 6:** encryption on the server, and the web sign-in's idle and absolute limits (#65) (Phase 7); the phone apps and voice logging (post-MVP, #041); digital signatures (on hold, sub-plan 01).

**Exit criteria:**
- **Each milestone's "You can check"** works on the web and the desktop, and each new kind of change works offline on the desktop and syncs.
- **Logging:** a fuel-up is saved with its receipt photo in one go; a dealer's job gets its labor cost from the dealer's rate.
- **Vehicles:** a custom "Track car" type needs no registration or insurance; a timing belt is due first at 100,000 mi, then every 50,000 mi.
- **People:** a custom role limits what its holder can do; a person's new username works at the next sign-in; a password can be reset by email when email is set up, and everything still works without it.
- **Reports:** a vehicle's costs per year and per mile, fuel trends, and its service history printed.
- **Import and export:** an exported vehicle imports back the same; a CSV opens in a spreadsheet.
- **Standalone:** a standalone Mac keeps its records through an update, and moves onto a server with none lost.
- **Packages:** the macOS and Windows packages install from the GitHub Release; the Mac one opens with no Gatekeeper warning.
- **Sync:** an app one API version behind still syncs.

**How this file works:** the same as `phase-5.md` (decision #021).
- One branch and **one PR into `dev` per milestone** (`milestone/p6-mN-<slug>`), merged with a merge commit.
- Each **step** is one Conventional Commit. Steps are planned in detail, and approved, when their milestone starts.
- Screens get **SVG layout options** in `docs/ui-sketches/` before they're built (#040).
- **One cumulative test plan for the phase** (#081): each milestone's last step adds its section to `docs/test-plans/p6-phase.md`. The whole plan is run at the end of the phase, on the test server (#062) with the web and desktop apps, as `v0.6.0`'s regression pass.
- `cargo clean` at each step's end, once its checks have run.
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | Time | What it delivers | You can check |
|---|---|---|---|---|---|
| 1 | **Fixes and Settings** | ✅ | not tracked | **Settings reorganized** (tabs down the left side, or another layout from the sketches), since later milestones add settings. The text to type in **type-to-confirm** dialogs stands out. **Lock now** in the account menu on a desktop with a PIN. The desktop **window's size** restored, and tested with a **second monitor**. **No 8 s wait** when the computer has no network at all. **Timeline paging** (100 at a time, Previous and Next, Jump to date) | Settings' new layout; a long timeline pages; the window reopens at its size |
| 2 | **Faster logging** | ✅ | not tracked | **Photos and documents added before the first save.** **Multi-page document capture** on the web (pages joined into one PDF). **Pinch to zoom** in the photo viewer on a phone. **Save a new place while logging** (#26). **Labor rates:** one for the household, overridden by a shop's own; typed labor time fills in its cost, which can still be changed (#089). **A main photo for each vehicle** (#24), on the Garage and the Overview | A fuel-up with its receipt photo in one go; a dealer job's cost from its rate |
| 3 | **Vehicles and schedules** | ✅ | 3 h 25 min | **Documents needed by kind of vehicle, and custom vehicle types** (idea 23). **Pinned vehicles** at the top of the Garage (idea 15). **A catalogue of ready-made fields and sets** (idea 16). **Chained intervals**, e.g. first at 100,000 mi, then every 50,000 mi (idea 21) | A "Track car" type needing no registration; a timing belt's chained rule |
| 4 | **People and security** | 🔨 | est. 4 h 50 min | **Change a person's display name and username** (#44). **Custom roles** (#27), from their own sub-plan, written first (#071). **Optional email:** SMTP setup and password reset by email (idea 13, sub-plan 03, once confirmed); email notifications come later. Sub-plan 04's **Phase 6 items:** password or PIN to enter Admin mode, rate limits across the API, growing sign-in delays kept in the database, the fail2ban log format, and the guide for running on the internet (#29's code items) | A "Chauffeur" role; repeated wrong sign-ins slowed; a reset email |
| 5 | **Reports** | ⬜ | est. 3 h | **Cost per vehicle per year**, **cost per mile**, **fuel trends**, and a **printable service history** (a page or a PDF) (#089) | A truck's 2026 costs; its history printed |
| 6 | **Import and export** | ⬜ | est. 3 h | **One versioned JSON format** (idea 4) for full backups and per-vehicle exports, which can be imported back; **CSV exports** of lists (fuel-ups, jobs, costs) for spreadsheets (#089) | Export a vehicle and import it on the test server; open a CSV in Numbers |
| 7 | **Standalone mode** | ⬜ | est. 4 h | The desktop app runs **with no server**, the server's own code inside it. A snapshot **each day it's used and before each update**, keeping 7, and **Export backup…** to any folder (#089). **Moving to a server** later, with milestone 6's format | Use the Mac with no NAS; move its records onto the test server |
| 8 | **Help in the app** | ⬜ | est. 1 h 30 min | **Help pages** (idea 22), starting with formulas (in place of `docs/help/formulas.md`); **ⓘ popups** with a short summary, linking to the full page | ⓘ on Filled full opens the fuel economy page |
| 9 | **Desktop packages and signing** | ⬜ | est. 1 h 30 min | The **Release workflow publishes** macOS, Windows, and Linux packages. **macOS signed and notarized** (Apple's developer program); **Windows signed** once its certificate is bought, and unsigned until then (#089) | A downloaded Mac package opens with no warning |

**Time** (agent work time, AGENTS.md): milestones 1 and 2 weren't tracked. **The rest of Phase 6: est. 17 h 50 min** (milestones 4–9; milestone 4 planned step by step at 4 h 50 min; re-estimated after milestone 3, from 37 h: its steps took a third to a half of their estimates, so each later milestone's is halved; re-estimated after 3f, from 40 h 40 min; after 3e, from 41 h; after 3d, from 43 h; and after 3c, from 68 h). A full check run took 7 to 20 minutes in milestone 3, even after `cargo clean` (35 minutes in milestone 2), and clippy run on every build first means most code steps need only one.

**Releases:** **`v0.6.0` at the end of Phase 6** (`docs/releasing.md`), with a migration fixture and a regression pass, as `v0.5.0` had. It's the **first release with desktop packages**.

---

## Why this order

- **1 first:** every later milestone adds settings, so Settings gets its new layout before they do.
- **2–4** change records and screens, each with its own migrations; done early, the later milestones read settled records.
- **5 and 6** read every kind of record, so they follow 2–4.
- **6 before 7:** moving a standalone computer onto a server uses the export format.
- **8 near the end:** the help describes finished features.
- **9 last:** it needs only the Release workflow, and leaves time for the Windows certificate.

## To decide when each milestone is planned

- **Milestone 1:** the Settings layout (SVG options); how the no-network check works on each platform.
- **Milestone 2:** how multi-page capture orders and joins its pages; where a vehicle's main photo is chosen; where labor rates are set (Settings and the shop's place).
- **Milestone 3:** whether a kind's or custom type's settings follow existing vehicles or only new ones; which fields and sets the catalogue starts with; what a chained interval counts from.
- **Milestone 4:** the custom roles sub-plan and its questions; confirming sub-plan 03 (email); the rate limits' values.
- **Milestone 5:** each report's layout (SVG options); PDFs made by the server, or the browser's print.
- **Milestone 6:** the format's schema and versioning; what an import does with records already there.
- **Milestone 7:** how the desktop runs the server's code; how moving to a server works.
- **Milestone 8:** help written as the app's messages (translated) or as bundled Markdown; which topics after formulas.
- **Milestone 9:** the package formats (`.dmg`, `.msi`, AppImage or `.deb`); where the signing secrets are kept.
- **API version:** new operations raise it once per milestone that adds them. The server keeps accepting the one before (#072).
- **Open issues** are reviewed with each milestone's plan (AGENTS.md).

## Reviewed against

- **Proposed for Phase 6:** ideas 4 (milestone 6), 13 (milestone 4), 15, 16, 21, 23 (milestone 3), 22 (milestone 8); issues #24, #26 (milestone 2), #27, #44, #29's Phase 6 code items (milestone 4); sub-plan 03 (milestone 4, once confirmed), sub-plan 04's Phase 6 items (milestone 4).
- **No effect now:** ideas 2, 7, 11, 19; idea 10 (built in Phase 3; the size bug is in milestone 1); idea 12 and sub-plan 01 (on hold); idea 14 and sub-plan 02 (post-MVP, they need the phones); issues #15 (post-MVP), #31 and #65 (Phase 7).

---

## Step details

*Each milestone's steps are planned here when it starts.*

### Milestone 1: Fixes and Settings

*Planned 2026-11-21 · Branch `milestone/p6-m1-fixes-settings` · Sketch: `docs/ui-sketches/p6m1-settings-options.svg`*

**Reviewed against:** no new issues since the phase plan; the ideas and sub-plans have no effect on this milestone.

**What the code shows:**
- **Settings** is one page under three tabs (You, Devices, Household in Admin mode). You has 12 rows in one list, then This device (theme, keep on top, lock and PIN).
- **Type-to-confirm** is in two places: deleting a vehicle (`delete-type`) and **REBUILD**. The text to type is inside the sentence, in plain type.
- **The lock:** `lock_now()` exists (the idle watcher calls it), but no menu offers it.
- **The window's size:** saved as the inner size and restored with `with_inner_size`, which looks right. The bug was seen only under `dx serve`, which may restore its own window state while developing; a bundled build is checked first.
- **The wait:** the desktop waits up to `SESSION_WAIT` (8 s) for the session check before opening its copy.
- **The timeline** loads all of a vehicle's events (the Overview and Fuel tab need them too) and draws every row. Paging only draws a page at a time; no server or API change.

**The human's choices (2026-11-21):**
- **Settings: option 1,** a column of sections down the left; on a narrow window, a list that opens each section, with ‹ Settings to go back. Sections: **You** (Units and formats; Vehicles and messages; Photos; Sign-in), **This device** (Appearance and window; Lock and PIN), **Devices**, and **Household** (Defaults for everyone, Admin mode only).
- **Timeline page size:** a 50 / 100 / 250 menu on the timeline, remembered on the device (100 by default). No migration or API change.

| Step | Status | Commit title | Contents | New dependencies |
|---|---|---|---|---|
| 1a | ☑️ | `docs: Phase 6 milestone 1 plan (fixes and Settings)` | This plan and the sketch | none |
| 1b | ☑️ | `feat(app): Settings in sections` | Option 1, with the groups above; each row moves unchanged; Household's defaults in Admin mode only; the last section remembered on the device; web, desktop, and a narrow window | none |
| 1c | ☑️ | `feat(app): the text to type stands out` | Deleting a vehicle and REBUILD: "To confirm, type:" and the text in a monospace, highlighted chip; one component for both | none |
| 1d | ☑️ | `feat(app): Lock now` | In the account menu and in **Lock and PIN**, on a desktop with a PIN | none |
| 1e | ☑️ | `fix(app): the window's size` | The cause, found in a bundled build, fixed with a test of the pure part; or, if it's only `dx serve`, why, noted here | none |
| 1f | ☑️ | `feat(app): no wait without a network` | Before the session check, ask the system for a route to the server (a UDP "connect", which sends nothing). "Network unreachable" opens the copy at once. With Tailscale off but Wi-Fi on, the wait stays. Checked on macOS, and on Windows and Linux in CI; left off on a platform where it isn't reliable | none (standard library) |
| 1g | ☑️ | `feat(app): timeline paging` | 100 a page (50 / 100 / 250, remembered on the device), ‹ Newer / Older ›, "Records 101–200 of 1,342", **Jump to** a date; the filters page the same way; the page chosen by a pure, tested function | none |
| 1h | ☑️ | `docs: milestone 1 section of the Phase 6 test plan` | `docs/test-plans/p6-phase.md` begins, with the second-monitor check (`p3m1-groundwork.md` step 10) | none |

**Risks:** 1f's answer differs by operating system (so it can be left off per platform); 1e may be a `dx serve` quirk.

**Written so far**
- **1a:** this plan; the sketch `p6m1-settings-options.svg` (three options; option 1 chosen; also the confirm chip, Lock now in the account menu, and the paging bar).
- **1b:** Settings is a column of sections (`settings.rs`, option 1):
  - **You:** Units and formats (distance, volume, economy, pressure, date format, currency, language); Vehicles and messages (the squawk word, due warnings, when due messages start); Photos (keeping originals, in a Maintainer's mode or above); Sign-in (sign out when idle).
  - **This device:** Appearance (theme, and keep on top on a desktop); Lock and PIN (only on a desktop with a PIN, `lock::has_lock`); Devices.
  - **Household:** Defaults for everyone, for the household Admin in Admin mode, with every household row under the same four headings.
  - Each row is unchanged and saves as before. Which sections show, and which opens, are pure and tested (`Offered`): a section remembered but not offered now (Household after leaving Admin mode) opens Units instead.
  - **The last section** is remembered on the device (`ui/device_memory.rs`, local storage, as the + Log chooser keeps its last vehicle; only the section's name, nothing secret, #071). The timeline's page size (1g) will use it too.
  - **Narrow windows** (under 640 px): the column becomes a list of sections with ›; a section opens on its own with **← Settings** above it.
  - **Messages:** seven new (the section names and ← Settings); "THIS DEVICE" and "SIGN-IN" are now "This device" and "Sign-in" (the column's headings are uppercased by the stylesheet).
- **1c:** one component, `TypeToConfirm` (`screens/account.rs`), for both places that ask for typed confirmation: **deleting a vehicle** and **REBUILD**.
  - It shows **"To confirm, type:"**, then the exact text in a monospace chip, outlined and tinted in the warning colour (spaces shown as they are; a long name wraps; a tap selects all of it), then a monospace box to type in. The chip sits inside the box's label, so a screen reader reads it with the box.
  - The checks are unchanged: a vehicle's name ignores case and spaces at either end (`confirms_name`); REBUILD is exact (now the constant `REBUILD_WORD`).
  - **Messages:** `delete-type` and `rebuild-type` are replaced by one, `confirm-type`.
  - Any later typed confirmation uses the same component.
- **1d:** **🔒 Lock now** locks a desktop with a PIN at once, the same way "Lock after" does when it sits idle (`lock::lock_now`: the key, the token, and the copy's key leave memory, and the lock shows).
  - **The account menu:** between Change password and Sign out. Whether it shows (`lock::has_lock`) is checked as the menu opens, since it reads the keychain.
  - **Settings → Lock and PIN:** a row between Lock after and Change PIN, with "Locks at once, as Lock after does."
  - Not on the web, or on a desktop without a PIN.
  - **Messages:** two new, `lock-now` and `lock-now-note`.
- **1e:** the window's size wasn't saved on macOS, in any build (not a `dx serve` quirk; Dioxus's own window restore steps aside when the app sets a size).
  - **The cause:** tao's `inner_size` on macOS keeps reporting the size the window opened at (tao #889, which Dioxus works around too), so every resize saved the old size. Found by running the app with a scratch home folder and a temporary hook that resized it: the window was 1000 × 700, and the saved size stayed 900 × 650.
  - **Also:** on macOS a window opens with its *content's* corner at the saved place, but the frame's corner was saved, so after each move it reopened a title bar (32 px) higher.
  - **The fix (macOS only):** the size comes from the frame, less the title bar (`window_placement::content_size`, pure and tested), and the place saved is the content's corner. Windows and Linux are unchanged: their `inner_size` is right, and their windows open with the frame's corner at the saved place.
  - **Checked:** the app resized itself to 1000 × 700 and moved, quit, and reopened at exactly that size and place. An older `settings.json` opens once a title bar higher, then stays put.
  - Dragging by hand and the second monitor are in the test plan (1h).
- **1f:** with no network at all, a desktop opens its copy at once instead of after the 8 s wait (`network_route.rs`, asked at the start of `local_copy::check_session`).
  - **Asking sends nothing:** a UDP socket "connects" to an address, which only asks the system for a route (standard library only).
  - **Changed from the plan:** a route to the server alone isn't enough, because Tailscale's tunnel keeps its routes with Wi-Fi off. So "no network" is: **no route to the internet** (a public IPv4 or IPv6 address), **and** the server can't be reached without it (its name can't be looked up within 1 s, it has no route, or its only addresses are Tailscale ones: 100.64.0.0/10, `fd7a:115c:a1e0::/48`).
  - **Waits as before:** Wi-Fi on but the server down or Tailscale off; Tailscale with an exit node; a server on a home network with no internet.
  - **Tests:** reading the server address (ports, IPv6, bad ones), Tailscale addresses, the verdict in each case, and a route to loopback. Only the asking itself is desktop-only, so CI's Build job now also runs `cargo test -p app --lib network_route` on macOS, Windows, and Linux (`main-gate.yml`). Turning Wi-Fi off is in the test plan (1h).
- **1g:** the Timeline, the Fuel tab, and the Maintenance tab's Jobs page (`timeline.rs`); no server or API change.
  - **A record** is an event that counts, with the originals it corrected beneath it, so a correction is never split across pages. A month's heading shows again at the top of a page that starts partway through it.
  - **The bar**, above and below the list once there are more than 50 records: "Records 101–200 of 1,342", ‹ Newer and Older › (greyed out at either end). Above the list it also has **Per page** (50 / 100 / 250; 100 by default, remembered on the device with `device_memory`, key `timelinePageSize`) and **Jump to** a date, which opens the page with the first record on or before it (before every record: the last page). The bar below the list scrolls back up to the top one.
  - **Starts at the newest page** when the filter, the page size, the vehicle, or the tab changes.
  - **Pure and tested:** the records (`entries`) and their months (`by_month`), the page (`page_at`, kept in range), the remembered size (`page_size_from`, only an offered one), Jump to (`page_for_date`), and grouping thousands.
  - **Messages:** six new (`timeline-range`, `-newer`, `-older`, `-per-page`, `-jump`, `-pages`).
- **1h:** `docs/test-plans/p6-phase.md` begins (#081: run at the end of the phase, on the test server): its setup, and milestone 1's section, steps 1.1–1.25:
  - **Settings** on the web (wide), on the phone (the list and ← Settings), and on the desktop (Lock and PIN); Photos in Maintainer mode; Household in Admin mode only; the section remembered.
  - **Type to confirm** by deleting a throwaway vehicle; REBUILD can't be caused on purpose, so it's noted, not stepped.
  - **Lock now** from the menu and from Settings, and not on the web.
  - **The window:** smaller, larger, maximized, and the **second monitor** (plugged in, then unplugged; `p3m1-groundwork.md` step 10); the one-time title-bar shift from an older saved place.
  - **No network:** Wi-Fi off opens the copy at once; Tailscale off still waits.
  - **Paging:** checks the truck has more than 50 records first (or adds readings), then the bar, the page size, Jump to, the filters, and the size remembered per device.

### Milestone 2: Faster logging

*Planned 2026-11-22 · Branch `milestone/p6-m2-faster-logging` · Sketch: `docs/ui-sketches/p6m2-faster-logging-options.svg`*

**Reviewed against:** no new issues since the phase plan; #24 and #26 are in this milestone. Decision #089 (labor rates). Sub-plans: none affect it.

**What the code shows:**
- **Attachments** upload in two steps: the file first (`data::upload`, tied to nothing), then `add_attachment` ties it to an owner. A new record's form has no strip: photos are added once it's saved (`log_form.rs`, `Mode::Edit` only). Offline, the desktop keeps the upload and the add in its outbox, and sends the file first.
- **PDFs:** viewed with pdf.js; nothing makes one. The server has `lopdf` (for page counts) and `image`. A document can already hold many files.
- **The photo viewer** (`attachments.rs`, `Viewer`) shows a plain image: no zoom, no gestures.
- **Places** are personal (#059) and already work offline (`AddPlace`); only the Places screen adds one.
- **Labor:** each task has a labor time and a labor cost, typed separately. "Done by" is someone in the household or a free-text name and contact (`Performer::Named`). No rate anywhere.
- **Vehicles** can already own photos (the Overview's strip, with the right to manage the vehicle); nothing marks one as the main photo.
- Migrations reach **0027**; the API is version **2**.

**The human's choices (2026-11-22):**
- **Shops:** a **household list of shops** (name, contact, labor rate per hour), kept by Maintainers and Admins. "Done by" suggests them; choosing one fills its rate.
- **Where rates are set:** a new Settings section, **HOUSEHOLD → Labor rates**, in Maintainer mode and above: the household's rate, then the shops.
- **Scanned pages** are joined into one PDF by the **server** (the desktop's own copy, offline), with `lopdf`: each JPEG goes in as it is. No new dependency.
- **Main photo:** chosen from the vehicle's own photos in the viewer: a **hollow ☆** on each (tooltip "Make Main Photo"), a **gold ★** on the main one; clicking toggles (clicking the gold one leaves no main photo).
- **The Garage card's photo:** option **A**, a square thumbnail at the card's left (sketch part 5), so the card keeps its height. The Overview's header shows it beside the name.

| Step | Status | Commit title | Contents | New dependencies |
|---|---|---|---|---|
| 2a | ☑️ | `docs: Phase 6 milestone 2 plan (faster logging)` | This plan and the sketch; milestone 1 marked done | none |
| 2b | ☑️ | `feat(app): photos and documents before the first save` | A new fuel-up, reading, maintenance, or repair gets the strip (sketch part 1). Files upload as they're added and are tied to the record when it's saved; ✕ or Cancel drops them. Offline on the desktop, they go into the outbox after the record. The pure part (which files to tie, in order) tested | none |
| 2c | ☑️ | `feat: scan pages into one document` | **+ Add → Scan pages** on the web (sketch part 2): each shot a page, ‹ › to move one, ✕ to drop it, **Make PDF**. A new operation joins uploaded photos into one PDF (server, and the desktop's copy offline); API version 3. Tests: the PDF has the pages in order, its page count, size limits, and refusing a file that isn't a photo | none (`lopdf` already used) |
| 2d | ☑️ | `feat(app): pinch to zoom a photo` | The viewer's own gestures (sketch part 6): pinch and pan, double-tap; on a computer, the trackpad's pinch, Ctrl + scroll, double-click. ‹ › and swiping only when not zoomed; zoom resets on the next photo. The zoom's limits and panning bounds pure and tested | none |
| 2e | ☑️ | `feat(app): save a place while logging` (#26) | **☆ Save as a place…** under Where on every log form (sketch part 3): name (from the place found), radius, then added to your places, online or offline. Hidden once the record matches a saved place | none |
| 2f | ☑️ | `feat: shops and labor rates` | Migration 0028: the household's labor rate and a **shops** table (name, contact, rate per hour, in the household's currency). Adding, changing, and removing them, for Maintainers and Admins; offline on the desktop, and synced. Tests: who may change them, the checks (a rate isn't negative, a name is unique), sync | none |
| 2g | ☑️ | `feat(app): labor cost from the rate` | **Settings → HOUSEHOLD → Labor rates** (sketch part 4). On a job's task, "Done by" suggests shops (choosing one fills its name and contact); typed labor time fills the cost from the shop's rate, or the household's, with the sum shown; a typed cost stays, with **↺ Use the rate**. A job keeps the cost it was saved with. The cost and its rounding pure and tested | none |
| 2h | ☑️ | `feat: a vehicle's main photo` (#24) | Migration 0029: a vehicle's main photo (one of its own photos, or none; cleared if that photo is removed). The viewer's ☆ / ★ toggle; the photo on the Garage card (a thumbnail at the left) and the Overview's header; a vehicle with none shows its kind's icon. Offline on the desktop, and synced | none |
| 2i | ☑️ | `docs: milestone 2 section of the Phase 6 test plan` | Milestone 2's steps in `docs/test-plans/p6-phase.md`: each feature on the web and the desktop, and each new kind of change offline, then synced | none |

**API version:** stays at **2** (the human's choice, 2026-11-22, in 2f): scanned pages are a new upload purpose (2c), and shops and the rate are changed on the server only, as templates are, so the desktop gains no sync operations. An older desktop is stopped by the schema check (migration 0028) and told to update, as in Phase 5.

**Risks:** pinch gestures differ between iOS Safari and other browsers (checked on the phone in the test plan); a scanned PDF can reach the 50 MB limit with many large pages (the pages use the photo size chosen in + Add).

**Written so far**
- **2a:** this plan; the sketch `p6m2-faster-logging-options.svg` (photos before the first save, scanning pages, saving a place, labor rates and the job's cost, the main photo with two Garage card options (A chosen), and pinch to zoom); milestone 1 marked done.
- **2b:** a new fuel-up or reading has the photo strip under the form, and a new maintenance or repair has its job photos where "Save the job first" used to be (`HeldStrip`, `attachments.rs`).
  - **+ Add** is the same sheet (Take photo, Choose photos, Choose PDF, the size). Each file is prepared and uploaded as it's added, as before, but **held** instead of added; its tile shows from what was uploaded (the shrunk photo, or the PDF's first page), with **✕** to drop it.
  - **On Save,** the held files are tied to the new record, in the order added (`attach_held`), before Finish (a finished job takes no more). Offline on the desktop each goes into the outbox after the record. One that can't be added is logged, and can be added again on the record's page. Cancel, or leaving, drops them; their uploads are left for the server's file check, as after any upload that isn't added.
  - **Keep the original** is offered to whoever may keep originals on the vehicle's own photos (asked of the vehicle, since the record doesn't exist yet); others add originals after saving.
  - Corrections still add photos after saving, as before.
  - **Tested:** the held files become the record's attachments in order (`held_inputs`).
  - **Messages:** one new, `attachment-added-on-save`.
- **2c:** **+ Add → Scan pages** (`ScanPages`, `attachments.rs`; sketch part 2): **📷 First page**, then **📷 Next page** (the camera, or photos chosen); each page shrunk to the size chosen in + Add; ‹ › move a page, ✕ drops it; **Make PDF (n pages)**. The PDF is named "Scan 2026-11-22.pdf" (today), and is added like any PDF, before or after the first save. Everywhere + Add is: on the desktop, the camera choice opens the file picker.
  - **One upload:** the pages are packed together (`domain::attachment::pack_pages`: `VMPAGES1`, then each page's length and bytes) and sent with a new purpose, `for=scanned-pages&size=…`. Up to 100 pages (`SCAN_PAGES_MAX`; "A scan can have up to 100 pages.").
  - **Joined by the server** (`server::attachments::join_pages`), or by the desktop's copy offline, which sends the pages the same way once it's back online: each page is prepared like any photo (upright, shrunk, a JPEG), then embedded in one PDF as it is, one page each, its long edge 11 inches; the thumbnail is page 1's. Over 50 MB is refused.
  - **Changed from the plan:** the pages are prepared like photos, so they're re-encoded once (as every photo upload is), not embedded exactly as sent: that turns them upright and reads a PNG too. And the **API version stays at 2**: a new upload purpose isn't a sync operation, as in Phase 5 (2f decides whether its new operations need version 3).
  - **Tested:** packing and unpacking (order, damage, an empty page, none, 100 and 101 pages); the upload purpose's query; joining (page order and sizes, landscape and portrait, a PNG page, the thumbnail, a page that isn't a photo); the upload stores a PDF of the right page count; moving a page.
  - **Messages:** fourteen new (`attachment-too-many-pages`, `scan-*`).
- **2d:** the photo viewer handles its own gestures (`ui/photo_zoom.rs`; sketch part 6), so only the photo zooms, never the page (its frame has `touch-action: none`):
  - **Pinch** zooms around the fingers, from 1× up to 5×; with one finger, a zoomed photo **pans**, never past its edges (it stays within its own frame). **Double-tap** (or double-click) zooms to 2× there, and back to 1×.
  - **On a computer:** the trackpad's pinch (Safari's gesture events, which the Mac desktop app's web view sends) and **Ctrl + scroll** (about ×1.2 a notch).
  - **Swiping** left or right changes photos, only when it isn't zoomed (it presses the viewer's ‹ ›). The ‹ › buttons and arrow keys work either way. Each photo has its own frame, so the next one starts at 1×.
  - **Changed from the plan:** the gesture maths runs in the page, not in tested Rust: on the desktop, every pointer move crossing into Rust would make pinching lag. Instead it was checked by running the script against a stand-in photo in macOS's JavaScript engine: double-tap to 2× and back, panning stopped at the edge, a pinch keeping the point under the fingers in place (centred and off-centre), the 5× limit, pinching back below 1×, a swipe only when not zoomed, and Ctrl + scroll. (That check found, and the script now fixes, a pinch that drifted.) Real fingers on the phone are in the test plan.
- **2e (#26):** **☆ Save as a place…** under Where on every log form (new, a draft, or a correction), once the record has a point that isn't within one of your saved places (`places::offers_saving`); both when Where is open and when it's folded to its summary.
  - It opens **Save as a place** (`places::SaveHere`; sketch part 3): "Here: 44.0462, −123.0220 · Springfield, OR", the **name** (the town found there, to change), and the **radius** (the Places screen's choices, 150 m to start). **Save** adds it to your places with the Places screen's own call, so it works offline on the desktop as that does (`AddPlace`), and is only yours (#059).
  - Then the point is looked up again, so the record shows the new place as where it is (📍 its name), and the link goes.
  - **Changed from the sketch:** the radius is the Places screen's menu (25 m to 2 km), not three chips, so both ask the same way.
  - **Tested:** the place saved is the record's point (to 7 decimals) with the name, radius, and town; when it's offered.
  - **Messages:** three new (`log-save-place`, `place-save-here-title`, `place-save-here-at`).
- **2f:** shops and the household's labor rate, on the server (`domain::shop`, `server::shops`, `storage::shops`).
  - **A labor rate** (`LaborRate`) is the household currency's smallest unit an hour, from 0 to 100,000.00. **A shop** has a name (unique, ignoring capitals), a contact (up to 100 characters), and its own rate or none (then the household's).
  - **The household's rate** is a household-only preference (`Preferences::labor_rate`), like when due messages start: a person's own is refused, and nobody has one until it's set.
  - **Migration 0028:** the `shops` table (with its pull index), and `preferences.labor_rate`. `shops` is pulled to desktops with the other records.
  - **Who:** anyone signed in sees them; a Maintainer or higher, in that mode, adds, changes, and removes shops and sets the rate.
  - **The human's choice (2026-11-22): changed on the server only,** as templates are: the desktop reads them from its copy (`data::list_shops`), offline too, so a job logged offline is priced from them; changing them needs the server. So there are no new sync operations, and the **API version stays at 2**.
  - **Tested:** the rate's limits; the shop's checks; a change naming only what changed; a person's rate ignored; who may see and change them (a Driver, and a Maintainer in Driver mode, can't); adding, a taken name, a bad contact, changing (including clearing the contact), a renaming that clashes, removing, a shop that isn't there; setting and clearing the household's rate; no shops or rate in every older release's fixture.
  - **Messages:** three new (`shop-name-taken`, `shop-bad-contact`, `shop-bad-rate`). Settings and the job form come in 2g.
  - **Also (its own commit):** `url` 2.5.8 → 2.5.9 in `Cargo.lock` (with `form_urlencoded` 1.2.3), for RUSTSEC-2026-0592 (a deeply nested `blob:` URL overflowed the stack in `Url::origin()`), published 2026-11-22. It comes in through Dioxus, reqwest, and sqlx, not the app's own code. `cargo deny` refused it until then.
- **2g (#089):** labor rates in Settings, and a job's labor cost from them (sketch part 4).
  - **Settings → HOUSEHOLD → Labor rates** (`screens/labor_rates.rs`), in Maintainer mode and above (so a Maintainer sees a HOUSEHOLD heading with only this): the **household rate** (typed, then **Save**; empty for none), then the **shops** (name, contact, its rate or "household rate"), with **+ Shop**, **Edit**, and **Remove**. Offline on the desktop they show, from its copy, without the buttons and with "This needs the server" (2f's choice).
  - **Done by:** with **Someone else**, the shops show as chips under the name ("Joe's Auto · $120.00 / h"), those whose name has what's typed in it (up to five; all while it's empty; none once it's a shop's whole name). Choosing one fills its name and contact (a contact already typed stays if the shop has none).
  - **The rate:** the shop named in Done by (ignoring capitals), with its own rate; otherwise the household's (also when it's done by someone in the household). None if the rates are in another currency than the job's.
  - **A task's labor cost** follows its time at the rate: "2:30 × $120.00 / h (Joe's Auto)". Typing a cost keeps it (emptying it lets the rate fill it again); then, if it differs, "At the rate: … is $300.00. **↺ Use the rate**". A saved job's costs are kept, so a later change of rate or shop never touches them; changing Done by refills only the costs the rate filled.
  - **Changed from the sketch:** Done by is the job's (as before), not each task's, so its shop prices every task; times stay typed as 2:30, not 2.5 h. The suggestions are chips under the name rather than a drop-down, which a phone's keyboard would hide.
  - **Tested:** the cost at the rate, to the nearest cent (a half up) and without overflowing; a shop's own rate, else the household's; the suggestions; filling only costs not kept, emptied with no rate; a saved cost staying until ↺; the job's currency; a rate and a shop as typed in Settings; the section shown to a Maintainer, under HOUSEHOLD.
  - **Messages:** twenty-four new (`settings-section-labor-rates`, `shop-*`, `shops-*`, `work-use-rate`, `work-rate-fills`, `work-rate-would-be`).
  - **Also (its own commit):** `cookie` 0.18.2 → 0.18.3 in `Cargo.lock`, for RUSTSEC-2026-0594 (private cookie nonces could repeat in a process that forks), published 2026-11-23. It comes in through reqwest and wry, not the app's own code. `cargo deny` refused it until then.
- **2h (#24):** a vehicle's main photo (sketch part 5, option A).
  - **Kept on the vehicle** (`Asset::main_photo`, migration 0029's `assets.main_photo`), as one more of its fields, so it syncs and merges with the rest of the vehicle (#043). The setup form never changes it.
  - **The human's choice (2026-11-23): chosen on the server only,** as the vehicle's setup is (#076 keeps offline changes to Driver mode's): `server::assets::set_main_photo`. The desktop shows it from its copy, offline too; offline, the star is greyed out with "This needs the server". No new sync operation, so the **API version stays at 2**.
  - **The viewer's star**, on the Overview's photos: a hollow **☆** ("Make Main Photo") on each of the vehicle's own photos, a gold **★** on the main one ("Main Photo: click for none"); clicking toggles. Whoever may change the vehicle's own photos (Admin on it, in Admin mode) chooses; anyone else sees only the gold star on the main one. Not on PDFs.
  - **Only one of its own photos:** not a document, not a photo on one of its records, not another vehicle's (`attachment::is_main_photo_of`). **A removed photo** stops being the main one where it's read (the Garage and the vehicle's page ignore it), so nothing else has to change when a photo goes, online or offline.
  - **Shown** on the Garage card, a 64 px square at its left (the card keeps its height), and in the Overview's header, 48 px beside the name. Without one, the vehicle's kind's icon: four new Lucide icons (truck, motorbike, caravan for a trailer, zap for a generator; the car's is already there, and other equipment gets the wrench).
  - **Tested:** only a vehicle's own photo; who may choose it (a Driver can't, nor an Admin in Maintainer mode); choosing, the page and another person's Garage showing it, choosing none, and a removed photo counting as none; no main photo in every older release's fixture; each built-in kind's icon.
  - **Messages:** three new (`main-photo-make`, `main-photo-clear`, `main-photo-is`).
- **2i:** milestone 2's section of `docs/test-plans/p6-phase.md`, steps 2.1–2.37:
  - **Photos before the first save** on the phone (added, ✕, saved, Cancel), a job's PDF on the web, and a fuel-up's photo offline on the desktop, then synced.
  - **Scan pages** on the phone (three pages moved and dropped, the PDF's page order), and offline on the desktop from chosen photos, then synced.
  - **Pinch to zoom** on the phone (pinch, pan, double-tap, swipe only unzoomed), the desktop (trackpad, Ctrl + scroll, double-click), and the Mac browser.
  - **Save as a place** on the phone (only Sam's), and offline on the desktop.
  - **Labor rates:** not in Driver mode; the household rate and two shops (a taken name refused); a repair priced from Joe's Auto, a typed cost kept with ↺ Use the rate, the household's rate for Me; a saved draft keeping its cost after the rate changes; read-only offline on the desktop.
  - **The main photo:** chosen and cleared in Admin mode; seen by Sam (gold star only) and not chosen in Maintainer mode; the Garage card and the header; a removed photo back to the kind's icon; offline on the desktop.
  - **Not by hand:** a scan's 100-page and 50 MB limits.

### Milestone 3: Vehicles and schedules

*Planned 2026-11-24 · Branch `milestone/p6-m3-vehicles-schedules` · Sketch: `docs/ui-sketches/p6m3-vehicles-schedules-options.svg` · Decision #090*

**Reviewed against:** no new issues since the phase plan (#65 stays in Phase 7). Ideas 15, 16, 21, and 23 are this milestone's, now decided (#090). Sub-plans: none affect it.

**What the code shows:**
- **Kinds** are data-driven profiles (`crates/domain/profiles/*.ron`, #030): each sets the event kinds, sections (fuel economy, towing), meters, fields, documents, and the default usage's documents (#086). `Asset::kind` is the profile's ID. Nothing household-wide changes what a kind starts with; each vehicle's usages change one vehicle at a time.
- **Templates** (#084) copy a vehicle's requirements, bundles, specs, and usages onto another, matching meters by name and kind.
- **The Garage** sorts each group by the person's choice (most urgent, newest activity, name, or custom); the custom order is a per-person `GarageOrder`, changed offline with `SetGarageOrder`. Cards have no menu; Arrange mode moves them with ▲▼.
- **Custom fields** are copied onto each vehicle from its profile, with a fixed key and kind (text, number, date, yes/no, choice, distance, duration, volume, fuel economy, calculated). No pressure or small length kind yet; specs (#084) already convert psi, kPa, bar, inches, and mm.
- **Interval rules** are every N on a meter, every N days, months, or years, a fixed date, or a meter reaching a mark. A requirement never done counts from "last done before these records", else the vehicle's first reading of the meter, else the day it was added.
- Migrations reach **0029**; the API is version **2**.

**The human's choices (2026-11-24, #090):**
- **Pinning:** each person's own, first in their own group whatever the sort; 📌 before the name, **Pin / Unpin** in a new ⋯ menu on the card and in Arrange mode (sketch part 1, option A).
- **Chained intervals:** a meter rule's first interval counts from the **meter's zero**; a calendar rule's from the vehicle's **In service since** date (new, optional, can be marked estimated, e.g. a used vehicle's estimated build date), else its Acquired date, else the day the requirement was added. Once done, or with "last done before" typed, only the repeating part. **To test by hand:** a used vehicle with In service since backdated to an estimated build date.
- **The catalogue:** fields are **copied** onto the vehicle, then editable; the four sets **Trip computer**, **Trip computer (hybrid)**, **Engine and idle**, and **Tires**, which asks for the axles and the tires on each (2, or 4 for duals).
- **Vehicle types:** household types **built on a kind**, for **new vehicles only**, each optionally naming a **template**; offered after the kinds in **one list** when adding a vehicle (sketch part 5, option A).

| Step | Status | Time | Commit title | Contents | New dependencies |
|---|---|---|---|---|---|
| 3a | ☑️ | 20 min (no estimate) | `docs: Phase 6 milestone 3 plan (vehicles and schedules)` | This plan and the sketch; decision #090; ideas 15, 16, 21, 23 closed; milestone 2 marked done | none |
| 3b | ☑️ | 1 h 15 min (no estimate) | `feat: pinned vehicles` | Migration 0030: `garage_orders.pinned`. A person's pinned vehicles come first in their group, in whatever sort (`domain::garage`). A new offline operation, `SetGaragePins`, so **API version 3** (the server keeps accepting 2, #072). The card's ⋯ menu (Open, Pin / Unpin, Move to group… for an Admin) and 📌 before the name; Pin / Unpin in Arrange mode. Tests: the sort with pins in each sort, a pinned vehicle that's gone or in another group, offline and synced | none |
| 3c | ☑️ | 35 min (est. 1 h 45 min) | `feat: chained intervals and In service since` | Migration 0031: `assets.in_service_since` and whether it's estimated, on the Setup tab ("12 Jun 2019 (est.)"). A meter or calendar rule's optional **first interval** (`IntervalRule`, stored with the rule; no migration); what's due counts it from the meter's zero or In service since (else Acquired, else the day added) until done or "last done before" is typed. The requirement dialog's "A different first interval" (sketch part 2); "First at 100,000 mi, then every 50,000 mi" on the Schedule, only the repeating part once done. Copy schedule and templates keep it. Tests: each base, a used vehicle backdated, done once, last done before, copying | none |
| 3d | ☑️ | 25 min (est. 1 h 15 min) | `feat: a catalogue of ready-made fields` | Two new field kinds, **pressure** (in each person's pressure unit) and **tread depth** (32nds of an inch or mm, by their distance unit). The catalogue in data (`crates/domain/catalogue.ron`): its fields and the four sets (average speed and EV share as calculated fields, #087). **+ Add field → From the catalogue** (sketch part 3): a field or set copied on, ones already there greyed out, a set again with " B"; **Tires** asks for the axles and tires on each and names each tire (1L, 1R, 2L outer, …). Tests: the catalogue loads and every key is unique, a set copied (and again with " B"), the tires' names for 1–4 axles, the new kinds' units | none |
| 3e | ☑️ | 25 min (est. 45 min) | `feat: vehicle types on the server` | Migration 0032: a **vehicle_types** table (name, icon, the kind it's built on, its default usage's documents, the meters and fields it starts with, a template or none) and `assets.vehicle_type`. Added, changed, and removed by the Admin in Admin mode, on the server only, as templates are; read by everyone, from the desktop's copy offline. Adding a vehicle of a type gives it the type's documents, meters, and fields and a copy of its template (left-out rules listed). Tests: who may change them, the checks (a unique name, a kind that exists), a new vehicle of a type, a type's change not touching existing vehicles, a removed type's vehicles showing their kind, older fixtures | none |
| 3f | ☑️ | 20 min (est. 45 min) | `feat(app): vehicle types` | **Manage → Vehicle types** (sketch part 4): the list, then the chosen type (name, icon, built on, documents, starts with, template, Delete). **Add vehicle** lists the household's types after the kinds, and shows what one starts with (part 5A); the card and header show the type's name and icon (when there's no main photo); Setup can change a vehicle's type (label and icon only). A few new Lucide icons for types | none |
| 3g | ☑️ | 5 min (est. 15 min) | `docs: milestone 3 section of the Phase 6 test plan` | Milestone 3's steps in `docs/test-plans/p6-phase.md`, including a used vehicle with In service since backdated to an estimated build date | none |

**Time:** 3 h 25 min for the milestone, all actual (est. 10 h when planned, before times were tracked step by step). Re-estimated after 3f, from 4 h: 3f took 20 minutes. After 3e, from 4 h 20 min: 3e took 25 minutes, its tests passing on the first run. After 3d, from 6 h 15 min: 3d took a third of its estimate, with one full check run and nothing found by clippy once the new kinds compiled. Earlier, re-estimated after 3c, from 10 h: 3c took a third of its estimate. Its check runs reused the build (minutes each, not 35), and a script now adds a new field to every test that builds the type, from the compiler's errors. Clippy is run on every build before a full check run, since the script's clippy stops at the first crate that fails and hides the rest.

**API version:** **3**, for `SetGaragePins` (3b). The other changes are made on the server only (setup, requirements, fields, types), as the shops were in milestone 2. An older desktop is stopped by the schema check (migration 0030) and told to update.

**Risks:** the Tires set can add many fields (a four-axle truck with duals: 28 fields), so the form groups them by axle; a chained rule copied onto a vehicle already past the first interval shows overdue until a date or "last done before" is entered (accepted in #090).

**Written so far**
- **3a:** this plan; the sketch `p6m3-vehicles-schedules-options.svg` (pinning, chained intervals and In service since, the catalogue and the Tires set, the Vehicle types screen, adding a vehicle of a type; 1A and 5A chosen); decision #090; ideas 15, 16, 21, and 23 closed; milestone 2 marked done.
- **3b:** pinned vehicles (idea 15; sketch part 1A).
  - **Kept with the person's Garage order** (`GarageOrder::pinned`; migration 0030's `garage_orders.pinned`, a JSON list, empty to start), as a field of its own, so a pin and a ▲▼ move made on two devices both stand (#043).
  - **The sort:** each group is sorted as before, then its pinned vehicles move to the front, keeping the sort among themselves and among the rest (`garage::pinned_first`, a stable sort). A pinned vehicle that's gone, or in another group, changes nothing.
  - **The card:** 📌 before the name (said as "Pinned"), and a **⋯** at its top right opening **Open** and **Pin / Unpin**; in Arrange mode, a 📌 toggle under ▲▼ instead. A ▲▼ move stays among the pinned or among the rest, and changes only the custom order, so an unpinned vehicle goes back to its place.
  - **Offline:** a new operation, `SetGaragePins` (the whole list), run by the desktop's copy and pushed like `SetGarageOrder`. **API version 3**; the server still takes version 2 (#072), so version-1 desktops (`v0.2.0`, unpaired) are now refused.
  - **Changed from the plan:** the ⋯ menu has no "Move to group…": moving stays on the vehicle's Setup tab, with its preview (#057).
  - **Time:** 1 h 15 min. A check run that hung for about 2 h 15 min on the desktop docs build is left out; the docs build then passed on its own.
  - **Also:** `AssetsOutcome::Garage` now boxes its view, which the pins made too large for clippy's limit on an error type; nothing changes on the wire.
  - **Tested:** the pins' patch fields; pinned first in each of the four sorts, two pinned in the sort's order, a pin that's gone; pinning listing each vehicle once; on the server, each person's own pins (a Driver too), a vehicle pinned twice kept once, pins and order kept apart, unpinning all; pinned on a desktop's copy and pushed, kept once, the order unchanged; no pins in every older release's fixture.
  - **Messages:** six new (`change-garage-pins`, `vehicles-card-menu`, `vehicles-open`, `vehicles-pin`, `vehicles-unpin`, `vehicles-pinned`).
- **3c:** chained intervals (idea 21) and In service since (sketch part 2).
  - **A different first interval** on an every-N meter or calendar rule (`IntervalRule::Meter::first`, `Calendar::first`, in the same unit). Stored with the rule's JSON, left out when there's none, so stored rules read as before and need no migration.
  - **What's due** (`due::requirement_due`): until the requirement is done once or "last done before these records" is typed, a meter rule's first interval counts from the **meter's zero** (from its first reading for a meter counting down), and a calendar rule's from the vehicle's **start** (`DueContext::start`), else the day the requirement was added. After that, only the repeating part. A used truck bought at 130,000 mi with a belt due first at 100,000 shows overdue by 30,000 until "last done before" is typed.
  - **The vehicle's start** (`server::due::vehicle_start`): its **In service since** date, else the day it was acquired (its Acquired event, in the household's zone).
  - **In service since** (`Asset::in_service`: a date and whether it's estimated; migration 0031's `assets.in_service_since` and `in_service_estimated`): on the Setup tab's details ("In service since 12 Jun 2019 (est.)") and its form (a date, and **Estimated**, with what it's for). A date after today wherever it's latest on Earth (UTC+14) is refused. A field of its own, so it merges like the vehicle's others (#043).
  - **The requirement dialog** (sketch part 2): under a meter or calendar rule, **A different first interval**; once ticked, "First at [ ] mi on the meter (from zero)" or "[ ] years from In service since (else Acquired)". **The Schedule** reads "First at 100,000 mi, then every 50,000 mi" until it's done once (or "last done before" is typed), then "Every 50,000 mi"; beside other rules, "Every 6 months, or first at …".
  - **Copying** a schedule or template keeps a rule's first interval (`IntervalRule::on_meter`).
  - **Tested:** from the meter's zero, then from when done; a used vehicle past its first interval, then "last done before"; a calendar rule from a backdated start, from long ago (overdue), from the day added, and from when done; the start changing nothing without a first interval; a first interval's checks (zero, negative, over 50 years) and its JSON (none left out); In service since on the form (round trip, today, tomorrow, not a date, empty); the Schedule's text before and after; on the server, from the day added, then Acquired, then a backdated estimate (overdue), a future date refused, and cleared; no In service since in every older release's fixture.
  - **Messages:** eleven new (`asset-bad-in-service`, `field-in-service*`, `setup-in-service-estimated`, `schedule-first-*`).
  - **Time:** 35 min (est. 1 h 45 min), leaving out the AGENTS.md change made in the middle of it. Three check runs: clippy found two functions too long (`due::rule_due` and `rules_text`, each now split into helpers) and a redundant clone.
- **3d:** the catalogue of ready-made fields (idea 16; sketch part 3).
  - **Two new field kinds:** **pressure** (stored in pascals, typed and shown in each person's pressure unit: psi to tenths, kPa whole, bar to hundredths) and **tread depth** (stored in micrometers, in 32nds of an inch for someone who uses miles, else mm; shown "7/32 in" or "5.6 mm"). Both take a sensible range; formulas can use a tread depth as a distance, not a pressure (`docs/help/formulas.md` says so). Stored as JSON like the other kinds, so no migration.
  - **The catalogue** (`crates/domain/catalogue.ron`, read by `domain::catalogue`): twelve fields (trip distance, time, fuel economy, fuel used, EV distance, EV time, idle time, engine run time, oil life %, DEF level %, and two calculated: **average speed** `trip-distance ÷ trip-time` and **EV share** `trip-ev-distance ÷ trip-distance`, #087), the sets **Trip computer** (5), **Trip computer (hybrid)** (8), and **Engine and idle** (4), and the Tires set's two per-tire fields.
  - **Copying** (`catalogue::fields_to_add`): the set's fields the vehicle doesn't have are added, on; ones it has (same key and kind) are used as they are, and **turned on** if they're off (a car's Trip EV distance). **Added again** (all already there), a set gets " B" on its labels and `-b` on its keys, then " C", …, and its formulas use the new keys. A key used before (a removed field, or a meter's) is never reused: that field gets the next free key from its label. Fields only for kinds of events the vehicle doesn't have are left out (a trailer gets no trip computer, and Engine and idle on readings only). One field already on the vehicle can't be added again.
  - **Tires** (`CatalogueChoice::Tires`): 1–4 axles, 2 tires or 4 (duals) on each; a **Tire pressure** and a **Tread depth** for each tire, named by position front to back (1L, 1R, 2L outer, 2L inner, 2R inner, 2R outer), keyed `tire-pressure-2l-outer`. A four-axle truck with duals behind gets 28 fields. Shown on readings and maintenance.
  - **On the server** (`server::assets::add_from_catalogue`, `/api/v1/fields/catalogue`; Maintainer work, server only as fields are): every formula is checked against the vehicle's fields as they'll be before any is saved.
  - **The Setup tab:** **+ From the catalogue** beside **+ Add field** opens the catalogue (`catalogue_view.rs`): sets with their fields listed, Tires, then single fields with their kind; one already on the vehicle, or not for its kind, is greyed out. **Tires** opens its form: axles 1–4, 2 or 4 (dual) per axle, what it adds ("6 tires · 12 fields: … (1L, 1R, 2L outer, …)"), and **Add 12 fields**. **Pressure** and **Tread depth** are also in Add field's types.
  - **Tested:** the catalogue loads, every key is unique, valid, and not a reserved word, every label is a message, and every set's formulas check; a set copied, again with " B" (its formula on the B keys), and a third time with " C"; a car's own trip time used and its EV distance turned on; a removed field's key not reused; a single field already there refused; a trailer's sets; the tires' names for 1–4 axles (28 fields on a four-axle truck with duals), and Tires again with " B"; the choice's JSON; the new kinds' units both ways (psi, kPa, bar, 32nds, mm) and on events; on the server, a Driver refused, the hybrid set on a truck (its EV distance turned on), again with " B", six tires, and a field already there refused.
  - **Messages:** 45 new (the catalogue's field labels and set names, `catalogue-*`, `tires-*`, `tire-*`, `kind-pressure`, `kind-tread-depth`, `unit-abbr-32nds`, `setup-add-from-catalogue`).
  - **Time:** 25 min (est. 1 h 15 min). One full check run, all passing.
- **3e:** vehicle types on the server (idea 23).
  - **A type** (`domain::vehicle_type::VehicleType`; migration 0032's `vehicle_types`): a name (unique, ignoring case), an icon (13: car, truck, motorcycle, trailer, generator, tractor, boat, aircraft, bus, van, bicycle, flag, tool; the app's drawings for the new ones come in 3f), **the kind it's built on**, the documents its default usage needs, **which of its kind's fields start on**, what it adds **from the catalogue** (sets, fields, Tires; up to 20), and a template or none. `VehicleType::from_kind` starts one as its kind is.
  - **Its meters are its kind's**, unchanged: the kind's rules (fuel economy, a fuel-up's required odometer) depend on them. The plan said "the meters and fields it starts with"; only fields are chosen.
  - **Who:** the household's Admin, in Admin mode, adds, changes, and removes them (`server::vehicle_types`; `/api/v1/vehicle-types/…`), on the server only, as templates; everyone signed in reads them, from the desktop's copy offline (`vehicle_types` is a synced table; `data::list_vehicle_types`, used from 3f).
  - **Checked:** a unique name, a kind the app has, fields that are its kind's, catalogue choices that fit its kind (a trailer's trip computer refused), a template that still exists.
  - **A vehicle added as a type** (`AssetForm::vehicle_type`; `assets.vehicle_type`): takes the type's kind whatever the form says, its kind's meters, the type's fields (its kind's on or off as it says, then the catalogue's), a default usage needing the type's documents, and a copy of its template; the template's meter rules without a matching meter are left out and answered (`AssetsOutcome::Added::left_out`, "requirement: meter").
  - **Afterwards:** a type's change never touches vehicles already added as it; a removed type's vehicles keep its ID and show their kind. **Setup** may change a vehicle's type only to one built on its kind, or none: its name and icon, never its fields (`TypeProblem::WrongKind`).
  - **Tested:** a type left as its kind makes its kind's fields; a type's fields, catalogue, and documents on a new vehicle; a set using a field the kind has; the checks (name in any case, unknown kind, a field not its kind's, the catalogue on a trailer, too many choices); its patch and JSON; on the server, a Maintainer and a Driver refused and everyone reading them, the checks (a taken name, an unknown kind, a removed template, no such type), a track car (kind from the type, trip time off, eight tire fields, insurance only), a generator type with the pickup's template (its odometer rule left out), a truck type with it (copied whole), an unknown type refused, a type changed and removed with its vehicle untouched, a vehicle's type changed to another kind's (refused), its own kind's, and none; no types in every older release's fixture.
  - **Messages:** four new (`type-name-taken`, `type-bad-catalogue`, `type-template-gone`, `type-wrong-kind`).
  - **Time:** 25 min (est. 45 min). One full check run, all passing.
- **3f:** vehicle types in the app (sketch parts 4 and 5A).
  - **Manage → Vehicle types** (`vehicle_types.rs`, `/vehicle-types`; flag icon), laid out like Groups: the list (each type's icon, name, and how many vehicles), **+ Add type** (it starts as a car, "New type"), and the chosen type: name, icon (13, as large buttons), **Built on** (changing it starts the type over as that kind), the documents its default usage needs (its kind's, ticked), **Starts with** (its kind's meters; its kind's fields on or off; **From the catalogue**: each set, Tires for 1–4 axles with duals behind the front, and each field, each removable), **Template** (or none), what it adds up to ("Needs Insurance · 1 meters · 10 fields · the Track day schedule"), Save, and **Delete type** (confirmed; "its vehicles keep everything; they show as their kind"). Everyone sees it; only the Admin in Admin mode can change it.
  - **Add vehicle** (5A): the household's types under "Your household's types" after the kinds, each with its icon; picking one shows its name and what it starts with, and the vehicle is added as it. Its template's rules left out are listed before the dialog closes.
  - **The card and header** show the type's icon where there's no main photo, and its name in place of the kind's: "Track car · Miata" on the card, "Track car (Car) · …" in the header. A removed type's vehicles show their kind (the server sends a type's name and icon only while it exists: `AssetSummary::vehicle_type`, `AssetView::vehicle_type`).
  - **Setup → Edit details:** **Type**, among the household's types built on its kind, or none; only its label and icon change.
  - **New Lucide icons** (ISC, none Feather's): `bus`, `van`, `bike`, `flag`; the type icons also reuse the car, truck, motorcycle, trailer, generator, tractor, sailboat, plane, and wrench.
  - **Tested:** every type icon has a name and a drawing; the Tires layouts and their names; a type's summary with and without documents and a template; the header's type and kind; the nav and routes with the new screen.
  - **Messages:** 51 new (`nav-vehicle-types`, `types-*`, `type-icon-*`, `type-summary` and its parts, `type-of-kind`, `add-vehicle-your-types`, `add-vehicle-left-out`, `setup-type*`).
  - **Time:** 20 min (est. 45 min). One full check run: a new message said "squawk" outright instead of the person's own word (`{ $squawk }`); fixed, and the workspace's tests run again, all passing.
- **3g:** milestone 3's section of the Phase 6 test plan (`docs/test-plans/p6-phase.md`, steps 3.1–3.29): pins per person (and offline), In service since backdated and estimated (a future date refused), a calendar rule's first interval from it (overdue), then "last done before", a timing belt first at 100,000 mi then every 50,000, copied; the hybrid trip computer (EV distance turned on) and again with " B"; Tires on a truck with duals behind, its pressure and tread in psi and 32nds, then kPa and mm; Vehicle types read only for Sam, Track car made, its checks, a vehicle added as it (card, header, readiness, fields), a type's change not reaching it, a vehicle's type changed on Setup, offline, and deleted.
  - **Time:** 5 min (est. 15 min). The milestone took 3 h 25 min; the rest of the phase is re-estimated from it (above).

### Milestone 4: People and security

*Planned 2026-11-27 · Branch `milestone/p6-m4-people-security` · Sketch: `docs/ui-sketches/p6m4-people-security-options.svg` · Decision #091 · Sub-plans: 05 (custom roles, new), 03 (email, confirmed), 04 (its Phase 6 items)*

**Reviewed against:** issues #27 (custom roles), #44 (names), #29's code items, and idea 13 with sub-plan 03 belong here; #65 (session limits) stays in Phase 7 as its issue says; #31 (Phase 7) and #15 (post-MVP) have no effect. #24 and #26 were built in milestone 2 but are still open. Ideas: 13 is this milestone's; a new one (an SMTP setup screen once the server's database is encrypted, the human's wish) is added for later. Sub-plan 04's §4.2 (a PIN for Admin mode) and §5 items are this milestone's, as decision #071 placed them.

**What the code shows:**
- **Roles** are fixed (`domain::access::Role`: Viewer < Driver < Maintainer < Admin), granted on all vehicles, a group, or one vehicle (`RoleGrant`); the **mode** (Driver, Maintainer, Admin) caps them. Permissions are gathered in `AssetAction` and `AdminAction`, but about 80 places compare roles directly (`role >= Role::Maintainer`, `highest_role`), in the server and the app.
- **Users** have a username and a display name, set when added; neither can be changed. Sessions and paired desktops point at the user's ID.
- **Sign-in limits** (`server::login_limiter`): 5 failures in 15 minutes per username and per client, kept in memory (a restart clears them). Hashing is capped at 2 at once; bodies and times are limited (`server::limits`). No limit on other requests.
- **Admin mode** asks for the password, and ends after 15 idle minutes.
- **Recovery:** the Admin gives a temporary password (#046); an Admin's own uses a code in the server log (#047). No email anywhere; users have no email address.
- Migrations reach **0032**; the API is version **3**.

**The human's choices (2026-11-27, #091):**
- **Custom roles are permission sets:** a name and ticked permissions, each at the Driver or Maintainer level, granted like today's roles. The mode picker stays Driver / Maintainer / Admin; in a mode, a custom role gives the ticked permissions at or below it. The built-in roles stay fixed; Admin always has everything (sub-plan 05).
- **Email, the whole of sub-plan 03:** SMTP set **only in `.env`**, the password **in a file** (`VEHICLEMANAGER_SMTP_PASSWORD_FILE`), never in the database; **one base URL** (`VEHICLEMANAGER_BASE_URL`) for links; a **Send test email** button; reset by email; the Admin's "Create your new password" link for a new user; an optional email address per person. A setup screen in the app waits until the server's database is encrypted (new idea 24).
- **Names (#44):** the Admin changes anyone's display name and username; a household setting, **off by default**, lets each person change their own.
- **Rate limits as proposed:** per client 300 requests a minute with bursts of 100; sign-in, reset, and PIN 10 a minute per client; wrong sign-ins lock 15 minutes after 5, then doubling up to a day, kept in the database; one fixed log line per failure for fail2ban or CrowdSec.

| Step | Status | Time | Commit title | Contents | New dependencies |
|---|---|---|---|---|---|
| 4a | ☑️ | 15 min (est. 25 min) | `docs: Phase 6 milestone 4 plan (people and security)` | This plan and the sketch; sub-plan 05 (custom roles); sub-plan 03 confirmed with the choices; decision #091; idea 13 closed, idea 24 added | none |
| 4b | ⬜ | est. 30 min | `feat: change a person's display name and username` | The Admin (Users, Admin mode) changes anyone's; Settings → You changes one's own when the household allows it (Settings → Household, off; migration 0033). Usernames stay unique (ignoring case); a change takes effect at the next sign-in; sessions and paired desktops keep working; records keep pointing at the person. Tests: who may, a taken username, signing in with the new one, the old one refused, the setting | none |
| 4c | ⬜ | est. 45 min | `feat: custom roles on the server` | Migration 0034: a **custom_roles** table (name, ticked permissions) and role grants naming one. Every role comparison becomes a permission check (`domain::access`), the same on the desktop offline; synced. Added, changed, and removed by the Admin in Admin mode; a removed role's grants go with it. Tests: a Chauffeur (fuel and readings, no squawks), an Apprentice (maintenance, not sign-off or setup) in each mode, grants on a group and a vehicle, the built-in roles unchanged, older fixtures | none |
| 4d | ⬜ | est. 35 min | `feat(app): custom roles` | **Manage → Roles** (sketch part 1): the built-in roles (read only) and the household's, each with its permissions ticked, by level. The Users screen's **+ Add role** offers them; screens show only what a person's permissions allow (#069) | none |
| 4e | ⬜ | est. 30 min | `feat: a PIN for Admin mode` | Sub-plan 04 §4.2: each Admin chooses the password (the default) or a PIN (6–12 digits, stored hashed) to enter Admin mode (sketch part 2); 10 a minute per client; wrong PINs count toward the sign-in lockout. Tests: setting, using, and clearing a PIN; wrong PINs locking; the password still working when no PIN is set | none |
| 4f | ⬜ | est. 35 min | `feat: rate limits and lasting sign-in lockouts` | A small limiter of our own (per client, and per account once signed in) on the whole API: 300 a minute, bursts of 100; sign-in, reset, and PIN 10 a minute; 429 with Retry-After. Lockouts kept in the database (migration 0035): 15 minutes after 5 failures, doubling up to a day, cleared by a good sign-in. One fixed log line per failure (`auth-failure client=… user=…`). Tests: the limits and their reset, a lockout surviving a restart, the doubling, the log line's format | none |
| 4g | ⬜ | est. 15 min | `docs: running on the internet` | `docs/deploying-internet.md` (sub-plan 04 §5.2): Tailscale stays the recommended setup; otherwise Caddy with TLS (a sample Compose file), a firewall with only HTTPS open, the fail2ban filter for 4f's line, upstream DDoS protection, and updates | none |
| 4h | ⬜ | est. 45 min | `feat: optional email and reset by email` | SMTP from `.env` (`VEHICLEMANAGER_SMTP_*`, the password from a file) with `lettre`; Settings → Household shows whether email is set up (never the password) and **Send test email** (sketch part 3). An optional, unique email address per person (Users, Settings → You; migration 0036). **Forgot password?** on the sign-in card: the same answer whatever was typed; a single-use link (stored hashed) for 30 minutes to `VEHICLEMANAGER_BASE_URL`; the new password ends the person's other sessions; 10 a minute per client. Tests: no email set up (the app as before), the link's checks (wrong, used, expired), no account enumeration, the password file | `lettre` (MIT or Apache-2.0) |
| 4i | ⬜ | est. 20 min | `feat: a new person's password by email` | When email is set up, **Add user** can send a "Create your new password" link (once, 10 minutes, #072) instead of a temporary password; the Admin can send it again. Tests: the link, its expiry, sending again | none |
| 4j | ⬜ | est. 10 min | `docs: milestone 4 section of the Phase 6 test plan` | Milestone 4's steps in `docs/test-plans/p6-phase.md`, including a Chauffeur role, repeated wrong sign-ins slowed, and a reset email | none |

**Time:** est. 4 h 50 min for the milestone (the steps' sum; milestone 3's steps took a third to a half of their first estimates, so these are smaller than the phase plan's 11 h, and a little under its halved 5 h 30 min).

**API version:** **4**, for custom roles: grants and roles sync to the desktop, which checks permissions offline. The server keeps accepting 3 (#072); an older desktop is stopped by the schema check (migration 0034) and told to update.

**Risks:** replacing about 80 role comparisons can open or close something by mistake, so 4c tests every built-in role's permissions as they are today before changing them; a rate limit set too low slows a big first sync, so a desktop's sync requests are counted per account, not only per client.

**Written so far**
- **4a:** this plan and the sketch `p6m4-people-security-options.svg` (Manage → Roles, 1A; a person's roles, name, and username; Admin mode with a PIN; email's status, test, and Forgot password?); sub-plan 05 (custom roles, confirmed); sub-plan 03 confirmed with the human's choices; decision #091; idea 13 closed and idea 24 (an SMTP setup screen after encryption) added.
  - **Time:** 15 min (est. 25 min), leaving out the time waiting for the human's answers.
