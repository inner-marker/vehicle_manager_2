# Manual test plan: photos, PDFs, official documents, and license plates

*Phase 2, milestone 8 · `docs/phase_plans/phase-2.md` · decisions #036, #037, #065, #066; choices 1–8*

**Part 1** is the checkpoint (step 8g): photos and PDFs on vehicles, events, tasks, and squawks, the PDF viewer, keeping originals, and the nightly file check. **Part 2** (step 8i) adds the Documents tab, license plates, and the desktop app.

Everything runs on the **test server** (decision #062). **Start only once milestone 7 is signed off:** this image upgrades the test server's database (to schema 14), and an older image won't open it afterwards.

## 1. Intended behavior

- **Where photos and PDFs show:** a strip of thumbnails, with **+ Add** at the end for whoever may add there (option A of `docs/ui-sketches/p2m8-photos-options.svg`):
  - a **vehicle's Overview** ("Photos & documents"; an Admin adds them);
  - a **fuel-up's or reading's page**, and its form once saved;
  - a **job's page** and its **work log**: each task's ("Photos & receipts"), then the job's own ("Photos & documents"). A new job's work log says "Save the draft to add photos and documents" until it's saved;
  - a **squawk's page** ("Photos"; whoever may change the squawk, at any time).
- **+ Add** opens **Add a photo or PDF**: **Take photo** (the camera), **Choose photos** (several at once), **Choose a PDF** (up to 50 MB), the **Size** (Small, about 200 KB; Medium, about 1 MB; Large, about 3 MB), and **Keep the original, as taken** for a Maintainer or Admin when the household allows it.
  - **The size starts at** Small on fuel-ups and readings, Medium everywhere else (choice 8).
  - **A photo is kept** upright, its long side at most 1280 / 2560 / 4096 px for Small / Medium / Large, and never enlarged. The JPEG kept carries no camera metadata; its time, place, and camera are kept as fields.
  - **A PDF's tile** is its first page, with its page count ("3 p").
- **Tapping one opens it** full screen: the photo, or the PDF's pages one under another (− / **Fit width** / + and "Page 2 of 12"). Beside it (below on a phone): when it was **Taken**, **Where** (the place for its GPS, looked up each time), the **Camera**, its **Size**, and who **Added** it and when. **Download**, **Original** (when one was kept, for Maintainers and Admins), and **Remove**. ‹ › or the arrow keys move between them; Escape closes it.
- **On a job:** a draft's photos can be added, moved, and removed. Once it's **finished**, they're locked; a **correction** starts with copies of them.
  - **Removing a task** in the work log: the tasks' strips hide ("Save the draft to see each task's photos again"); on **Save draft**, each photo moves with its task, and a removed task's photos stay on the job.
- **The server** keeps each photo and PDF as a file in `data/files/`, named by its contents (#065). A removed one's files stay for **8 days** (so any kept snapshot can be restored with its photos), then the nightly **file check** erases them. The check also compares every file with its name; a missing or damaged one shows in **Messages** (Admin mode) as **"Some photos or documents are missing or damaged"**.
- **License plates** became their own records in this upgrade (#066). The plate typed on a vehicle before this milestone has moved into a plate record, so **the vehicle's header and Garage card show no plate until part 2** (where plates get their own screens). That's expected.

## 2. Setup

- **The milestone image:** the "Server image" workflow builds `milestone-p2-m8-documents` for each push to `milestone/p2-m8-documents`. Check that Actions shows the newest run finished successfully.
- **On the NAS over SSH, the test server's folder:**

  ```sh
  cd /volume1/docker/vehicle_manager_test
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m8-documents`, and leave `VEHICLEMANAGER_TEST_MINUTES=` empty for now (step 31 sets it). Then press `Esc` and type `:wq`. `compose.yaml` is unchanged.
- **The main server is untouched** (it stays on `:dev`).
- **People:** your Admin account (`admin`), and Sam with the **Driver** role.
- **Devices:** a phone with its browser at the test server, and the Mac's browser (Safari, and Chrome for step 12).
- **Vehicles:** `Test truck` (with its odometer, and a plate typed in milestone 4), and its squawks and jobs from milestone 7.
- **Files to have ready:**
  - on the phone: a photo taken **with location on** (the camera's location setting);
  - on the Mac: a few photos, one of them **HEIC** (straight from Photos, e.g. dragged to the Desktop);
  - a **real PDF from a DMV or insurer** (often locked against editing, which is fine), and a **multi-page scan**;
  - a **PDF over 50 MB** (any big manual), and **one that isn't a PDF** renamed to `.pdf`.

## 3. Checklist (part 1)

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | Note `Test truck`'s plate (Setup tab). `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | <ul><li>`backed up before migrating` with `schema_before=13`.</li><li>`database open` with `created=false`, `schema_before=13 schema_now=14`, and the **same database ID** as before.</li><li>`database integrity check passed` and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [x] | 2 | `ls -ln data/` | A new `files/` folder (with `incoming/` inside, empty), owned by `1030 100`. A new `pre-migration-<UTC time>-v13.sqlite` in `data/backups/`. |
| [x] | 3 | **Mac**, as `admin`: `Test truck` | <ul><li>The header and Garage card show **no plate** (expected until part 2), and Setup's identifiers no longer list it. Its VIN (if any) is still there.</li><li>Milestone 7's squawks, jobs, and schedule are all still there.</li></ul> |
| [x] | 4 | **Admin mode** → `Test truck` → **Overview** | A **PHOTOS & DOCUMENTS** panel with a dashed **+ Add** tile. |
| [x] | 5 | **+ Add** | **Add a photo or PDF**: Take photo, Choose photos, Choose a PDF; **SIZE** with **Medium** chosen; **Keep the original, as taken**. |
| [x] | 6 | **Choose photos** → pick three photos (not the HEIC one) | "Adding 1 of 3…", then the sheet closes and three thumbnails appear. |
| [x] | 7 | Tap the first | It opens full screen: the photo, **Size** "Medium · 2560 × 1920 · …" (or its own shape, at most 2560 on the long side; a smaller photo keeps its size), **Added** "by …, today". "1 of 3"; **›** and the → key go to the next; Escape closes. |
| [x] | 8 | Open one taken by a phone, and look at **Taken** and **Camera** | The time it was taken (in your time), and the phone ("Apple iPhone …"). |
| [x] | 9 | **Download** | The JPEG downloads with its original name. Open it: upright, and (in Preview › Tools › Show Inspector) **no** GPS or camera details in the file. |
| [x] | 10 | **Remove** → **Remove** | It's gone from the strip. |
| [x] | 11 | **Leave** Admin mode, then open the Overview again | The photos still show, but **no + Add** (a vehicle's own photos are an Admin's). |
| [x] | 12 | **Safari** on the Mac (Admin mode): **+ Add** → **Choose photos** → the HEIC photo. Then the same in **Chrome** | Safari: added as a JPEG. Chrome: either added, or "IMG_….HEIC: That photo is HEIC, which the server doesn't convert…" (Chrome can't read HEIC). |
| [x] | 13 | **Phone**, as **Sam** (Driver): `Test truck` → **+** → **Fuel-up**, fill it in, **Save** | Saved. Its page has **PHOTOS** with **+ Add**. |
| [x] | 14 | **+ Add** | **Small** is chosen (a fuel-up's default). **No** "Keep the original" (Sam is a Driver). |
| [x] | 15 | **Take photo** → photograph the pump, with location on → use it | The thumbnail appears. Open it: "Small · 1280 × 960 · about 200 KB", **Where** names the town, **Taken** is now. |
| [x] | 16 | Back on the fuel-up page: **Edit** | The form shows the same **PHOTOS** strip. |
| [x] | 17 | Open a squawk on `Test truck` (as Sam, one he opened) | **PHOTOS** with **+ Add**; add one. It stays after a job dealt with the squawk too (try one that's Deferred). |
| [x] | 18 | **Mac**, `admin`, **Maintainer mode**: `Test truck` → **+ Log** → **Repair**, with two tasks (`Front pads`, `Rear pads`) | Under the tasks: "Save the draft to add photos and documents." |
| [x] | 19 | **Save draft**, then open it again (**Edit**) | Each task card has **PHOTOS & RECEIPTS** with **+ Add**; below the tasks, **PHOTOS & DOCUMENTS** for the job. |
| [x] | 20 | On **Rear pads**: **+ Add** → **Medium**, tick **Keep the original** → a photo. On the job: **Choose a PDF** → the multi-page scan | The photo shows on Rear pads; the PDF's first page shows as its tile with its page count. |
| [x] | 21 | Open the photo | **Original** shows beside Download. **Original** downloads the file as taken (same name). |
| [x] | 22 | Remove **Front pads** (the first task) | A note: "Save the draft to see each task's photos again"; the task strips hide. |
| [x] | 23 | **Save draft**, then **Edit** again | The photo is on the one task left (**Rear pads**, now first). The PDF is still on the job. |
| [x] | 24 | **Finish…** → **Finish and lock** | Its page shows the photo and PDF, with **no + Add**; opening the photo shows **no Remove**. |
| [x] | 25 | **Correct…** → a reason → **Save** | The correction's page shows the same photo and PDF (copies), which can be removed from it; the original keeps them. |
| [x] | 26 | Open the PDF | Its pages, one under another, white on the dark viewer. "Page 1 of N"; scrolling moves it on. **+** and **−** zoom; **Fit width** goes back. Text can be selected (if the scan has text). |
| [x] | 27 | On the job (or a fresh draft): **Choose a PDF** → the **DMV or insurer PDF** | Added, with its first page as the tile and the right page count. It opens and shows its pages. *(If it's refused with "couldn't read that file", note which PDF.)* |
| [x] | 28 | **Choose a PDF** → the one **over 50 MB**; then the **renamed non-PDF** | "PDFs can be up to 50 MB…" (before uploading); then "Documents must be PDFs." |
| [x] | 29 | **Admin mode** → **Settings** → **Household** → **Keep photo originals**: **Off**. Then **Maintainer mode**, a job draft → **+ Add** | Settings shows Allowed / Off with a note. In the sheet, **no** "Keep the original". The **You** tab shows "Keep photo originals: Off" and that the Admin chooses it. Originals already kept (step 21) can still be downloaded. Set it back to **Allowed**. |
| [x] | 30 | **Phone**, Sam: open the repair's page from step 24, then its photo | No **Original** button (only Maintainers and Admins). |
| [x] | 31 | **The file check, with test timers.** In `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, then `sudo docker compose up -d`. Note `find data/files -type f -not -path '*/incoming/*' \| wc -l` | The count of stored files (each photo is two: it and its thumbnail; a PDF one or two; originals and metadata one each). |
| [x] | 32 | Remove a photo in the app (one on a draft or a squawk). Wait **18 minutes** (the 8 kept "days" at 2 minutes each, and a run) | The count drops by that photo's files (2, or more with an original). A photo removed less than 16 minutes before isn't erased yet. |
| [x] | 33 | Damage a file: `f=$(find data/files -type f -not -path '*/incoming/*' \| head -1); sudo sh -c "echo x >> $f"`. Wait 2 minutes | **Messages** (Admin mode): **"Some photos or documents are missing or damaged"**, naming the start of the file's name. The log has `stored file problem` with `damaged`. |
| [x] | 34 | Clean up: in the app, remove the attachment whose file you damaged (the message stays until it's removed, or the file is copied back from a backup of `data/files/`). Then empty `VEHICLEMANAGER_TEST_MINUTES=` again and `sudo docker compose up -d` | The server starts normally (no TEST TIMERS tag). After the next file check, the message is gone. |

## 4. Checklist (part 2)

*Step 8i: the Documents tab, license plates (giving the truck's moved plate its state, then moving it to a new one), renewals, who may change them, and the desktop app (its uploads, Downloads, the PDF viewer, and HEIC from Photos). It carries on from part 1's `Test truck`.*

**Setup:** the "Server image" run for "feat(app): the Documents tab and license plates" (or newer) finished successfully; `VEHICLEMANAGER_TEST_MINUTES=` is empty. Have ready: a PDF or photo of a **registration** card, an **insurance** card, and a **title** (any will do; nothing is shared). For the desktop app (steps 21–25), on the Mac: `cd crates/app` in the repository, then `dx serve --platform desktop`, with the test server's address (`:8443`) saved.

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | `database open` with `schema_now=14` (no migration this time), `database integrity check passed`, no `ERROR` lines. |
| [x] | 2 | **Mac**, `admin`: `Test truck` | The header shows the plate from part 1's step 1, **as just its number** (no state yet). The Garage card shows it too. |
| [x] | 3 | **Documents** tab | **LICENSE PLATE**: the plate drawn like one, "Choose who issued it: Edit." in red. **DOCUMENTS**: "No documents yet." **+ Plate** and **+ Document** show only in Maintainer or Admin mode. |
| [x] | 4 | **Maintainer mode** → the plate's **Edit** | **Change the license plate**: "Issued by" with **none chosen** and "Choose who issued it", the number, and empty dates. |
| [x] | 5 | **US state** → **North Carolina** (or your state); its issue date → **Save** | The card: "North Carolina · <number>", "Since …". The header and Garage card: "NC · <number>". |
| [x] | 6 | **Edit** again → **Other**, leave the name empty → **Save** | "Enter who issued the plate." Cancel. *(A plate that has a state keeps one.)* |
| [x] | 7 | **+ Document** | **Add a document**: **Type** (Title, Registration, Insurance, Other…), Number, Issued by, Issued, Takes effect, Expires, **License plate** (the NC plate chosen), "Its expiry becomes the plate's.", Notes, "Add its scans on its card once it's saved." |
| [x] | 8 | **Registration**: a number, `NC DMV`, issued a year ago, **expires in 3 weeks** → **Save** | A **Registration** card: number · NC DMV, the dates, a yellow **Expires in N days** chip, **SCANS** with **+ Add**, **Renew…** and **Edit**. The plate's card now says "expires … (from its registration)". |
| [x] | 9 | On its **SCANS**: **+ Add** → **Choose a PDF** (or a photo) → the registration card | Its tile shows, first page as the thumbnail. Open it: it reads. |
| [x] | 10 | **+ Document** → **Insurance** (expires in 6 months, no plate menu), then **Title** (no expiry) | Each its own card: Insurance **Current** with **Renew…**; Title "never expires", **Current**, and **Edit** only (nothing to renew). |
| [x] | 11 | **+ Document** → **Other…** → name `Parking permit`, expires before it's issued → **Save** | "It expires before it's issued or takes effect. Check the dates." Fix the dates → **Save**: a **Parking permit** card. |
| [x] | 12 | The registration's **Renew…** | **Renew it**: "A new record; the one it renews is kept as history." Type (fixed), number, issuer, and plate filled in; **dates empty**. |
| [x] | 13 | Issued today, expires in a year → **Renew** | One **Registration** card, **Current**, with the new dates, and **▸ Earlier (1)** below it. The plate's expiry moved to the new one. The scan stayed with the earlier one (open **Earlier (1)**). |
| [x] | 14 | **Earlier (1)** → **Edit** → **Remove** → **Remove it and its scans** | "A later document renews this one. Remove that one first." Cancel. |
| [x] | 15 | The plate's **Edit** → **Remove** → **Remove the plate** | "A registration names this plate. Change the registration first." Cancel. |
| [x] | 16 | **The truck moves to Virginia:** **+ Plate** → **US state** → **Virginia**, a new number, issued today → **Add** | The new plate is the current one; **▸ Earlier plates (1)** has the NC one, "… – today". The header and Garage card show "VA · …". |
| [x] | 17 | The registration's **Edit** → **License plate**: the new VA plate → **Save** | Saved. The VA plate's card now shows the registration's expiry. |
| [x] | 18 | The **Parking permit**'s **Edit** → **Remove** → **Remove it and its scans** | The card goes. |
| [x] | 19 | **Phone**, as **Sam** (Driver): `Test truck` → **Documents** | The plate and the cards, with their chips and scans (which open), and **no** + Plate, + Document, Renew, or Edit. |
| [x] | 20 | **Phone**, `admin`, **Maintainer mode**: the insurance card's **SCANS** → **+ Add** → **Take photo** → photograph the insurance card | Added, Medium. Open it: readable. |
| [x] | 21 | **Desktop app:** sign in → `Test truck` → **Overview** (Admin mode) → **+ Add** → **Choose photos** → the **HEIC** photo from part 1 | Added as a JPEG (the Mac's webview reads HEIC). Open it: upright, with its **Taken** and **Camera**. |
| [x] | 22 | In the viewer: **Download** | "Saved to …/Downloads/<name>". The file is there and opens. |
| [x] | 23 | **Documents** → the registration's earlier scan | The PDF viewer: its pages, zoom, "Page 1 of N". |
| [x] | 24 | A job draft (Maintainer mode) → a task → **+ Add** → **Choose a PDF** | Added, with its first page as the tile. |
| [x] | 25 | Quit and reopen the desktop app; open the same photo | It shows (fetched again from the server with the app's session). |

## 5. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **The step's number,** a screenshot, the device (phone or Mac) and browser, and for a photo or PDF that misbehaves, the file if it's nothing private.
- **Never share** a password, `.env`, a cookie, or a document with personal details on it.

## 6. Results

*Filled in at step 8j.*

# Human Notes

- As we add more API endpoints, the number of log lines at `docker logs` will increase. The log lines look like `2026-11-04T23:45:33.371273Z  INFO dioxus_server::server: Registering: GET /api/v1/handshake`. Doing a tail of 40 rows cuts off the initial few log messages during startup. Tail commands should be by time, not by number of lines.
- In app/src/main.rs, I want to start configuring the launcher with `with_cfg(...)` (line 197?) for desktop
- On mobile in the web browser, pinching to zoom on an image should only zoom and pan the image, not the whole page. If this is not an easy fix now, then it can go to the "Polish" phase.
- On mobile in the browser, after I take a photo, the "Add a photo or PDF" diolog should remain open after I take the picture and choose "Use this photo" with the iOS interface so I can still choose Small/Medium/Large, or cancel the upload.
- I should be able to add photos and documents in a Repair or Maintenance job in Edit mode, not just in the draft view. Same for Fuel-ups and Readings. This is a "Polish" phase item.