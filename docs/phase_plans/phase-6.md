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

| # | Milestone | Status | What it delivers | You can check |
|---|---|---|---|---|
| 1 | **Fixes and Settings** | ✅ | **Settings reorganized** (tabs down the left side, or another layout from the sketches), since later milestones add settings. The text to type in **type-to-confirm** dialogs stands out. **Lock now** in the account menu on a desktop with a PIN. The desktop **window's size** restored, and tested with a **second monitor**. **No 8 s wait** when the computer has no network at all. **Timeline paging** (100 at a time, Previous and Next, Jump to date) | Settings' new layout; a long timeline pages; the window reopens at its size |
| 2 | **Faster logging** | 🔨 | **Photos and documents added before the first save.** **Multi-page document capture** on the web (pages joined into one PDF). **Pinch to zoom** in the photo viewer on a phone. **Save a new place while logging** (#26). **Labor rates:** one for the household, overridden by a shop's own; typed labor time fills in its cost, which can still be changed (#089). **A main photo for each vehicle** (#24), on the Garage and the Overview | A fuel-up with its receipt photo in one go; a dealer job's cost from its rate |
| 3 | **Vehicles and schedules** | ⬜ | **Documents needed by kind of vehicle, and custom vehicle types** (idea 23). **Pinned vehicles** at the top of the Garage (idea 15). **A catalogue of ready-made fields and sets** (idea 16). **Chained intervals**, e.g. first at 100,000 mi, then every 50,000 mi (idea 21) | A "Track car" type needing no registration; a timing belt's chained rule |
| 4 | **People and security** | ⬜ | **Change a person's display name and username** (#44). **Custom roles** (#27), from their own sub-plan, written first (#071). **Optional email:** SMTP setup and password reset by email (idea 13, sub-plan 03, once confirmed); email notifications come later. Sub-plan 04's **Phase 6 items:** password or PIN to enter Admin mode, rate limits across the API, growing sign-in delays kept in the database, the fail2ban log format, and the guide for running on the internet (#29's code items) | A "Chauffeur" role; repeated wrong sign-ins slowed; a reset email |
| 5 | **Reports** | ⬜ | **Cost per vehicle per year**, **cost per mile**, **fuel trends**, and a **printable service history** (a page or a PDF) (#089) | A truck's 2026 costs; its history printed |
| 6 | **Import and export** | ⬜ | **One versioned JSON format** (idea 4) for full backups and per-vehicle exports, which can be imported back; **CSV exports** of lists (fuel-ups, jobs, costs) for spreadsheets (#089) | Export a vehicle and import it on the test server; open a CSV in Numbers |
| 7 | **Standalone mode** | ⬜ | The desktop app runs **with no server**, the server's own code inside it. A snapshot **each day it's used and before each update**, keeping 7, and **Export backup…** to any folder (#089). **Moving to a server** later, with milestone 6's format | Use the Mac with no NAS; move its records onto the test server |
| 8 | **Help in the app** | ⬜ | **Help pages** (idea 22), starting with formulas (in place of `docs/help/formulas.md`); **ⓘ popups** with a short summary, linking to the full page | ⓘ on Filled full opens the fuel economy page |
| 9 | **Desktop packages and signing** | ⬜ | The **Release workflow publishes** macOS, Windows, and Linux packages. **macOS signed and notarized** (Apple's developer program); **Windows signed** once its certificate is bought, and unsigned until then (#089) | A downloaded Mac package opens with no warning |

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
| 2g | ⬜ | `feat(app): labor cost from the rate` | **Settings → HOUSEHOLD → Labor rates** (sketch part 4). On a job's task, "Done by" suggests shops (choosing one fills its name and contact); typed labor time fills the cost from the shop's rate, or the household's, with the sum shown; a typed cost stays, with **↺ Use the rate**. A job keeps the cost it was saved with. The cost and its rounding pure and tested | none |
| 2h | ⬜ | `feat: a vehicle's main photo` (#24) | Migration 0029: a vehicle's main photo (one of its own photos, or none; cleared if that photo is removed). The viewer's ☆ / ★ toggle; the photo on the Garage card (a thumbnail at the left) and the Overview's header; a vehicle with none shows its kind's icon. Offline on the desktop, and synced | none |
| 2i | ⬜ | `docs: milestone 2 section of the Phase 6 test plan` | Milestone 2's steps in `docs/test-plans/p6-phase.md`: each feature on the web and the desktop, and each new kind of change offline, then synced | none |

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
