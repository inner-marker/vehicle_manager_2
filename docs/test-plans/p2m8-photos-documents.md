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
| [ ] | 1 | Note `Test truck`'s plate (Setup tab). `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | <ul><li>`backed up before migrating` with `schema_before=13`.</li><li>`database open` with `created=false`, `schema_before=13 schema_now=14`, and the **same database ID** as before.</li><li>`database integrity check passed` and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [ ] | 2 | `ls -ln data/` | A new `files/` folder (with `incoming/` inside, empty), owned by `1030 100`. A new `pre-migration-<UTC time>-v13.sqlite` in `data/backups/`. |
| [ ] | 3 | **Mac**, as `admin`: `Test truck` | <ul><li>The header and Garage card show **no plate** (expected until part 2), and Setup's identifiers no longer list it. Its VIN (if any) is still there.</li><li>Milestone 7's squawks, jobs, and schedule are all still there.</li></ul> |
| [ ] | 4 | **Admin mode** → `Test truck` → **Overview** | A **PHOTOS & DOCUMENTS** panel with a dashed **+ Add** tile. |
| [ ] | 5 | **+ Add** | **Add a photo or PDF**: Take photo, Choose photos, Choose a PDF; **SIZE** with **Medium** chosen; **Keep the original, as taken**. |
| [ ] | 6 | **Choose photos** → pick three photos (not the HEIC one) | "Adding 1 of 3…", then the sheet closes and three thumbnails appear. |
| [ ] | 7 | Tap the first | It opens full screen: the photo, **Size** "Medium · 2560 × 1920 · …" (or its own shape, at most 2560 on the long side; a smaller photo keeps its size), **Added** "by …, today". "1 of 3"; **›** and the → key go to the next; Escape closes. |
| [ ] | 8 | Open one taken by a phone, and look at **Taken** and **Camera** | The time it was taken (in your time), and the phone ("Apple iPhone …"). |
| [ ] | 9 | **Download** | The JPEG downloads with its original name. Open it: upright, and (in Preview › Tools › Show Inspector) **no** GPS or camera details in the file. |
| [ ] | 10 | **Remove** → **Remove** | It's gone from the strip. |
| [ ] | 11 | **Leave** Admin mode, then open the Overview again | The photos still show, but **no + Add** (a vehicle's own photos are an Admin's). |
| [ ] | 12 | **Safari** on the Mac (Admin mode): **+ Add** → **Choose photos** → the HEIC photo. Then the same in **Chrome** | Safari: added as a JPEG. Chrome: either added, or "IMG_….HEIC: That photo is HEIC, which the server doesn't convert…" (Chrome can't read HEIC). |
| [ ] | 13 | **Phone**, as **Sam** (Driver): `Test truck` → **+** → **Fuel-up**, fill it in, **Save** | Saved. Its page has **PHOTOS** with **+ Add**. |
| [ ] | 14 | **+ Add** | **Small** is chosen (a fuel-up's default). **No** "Keep the original" (Sam is a Driver). |
| [ ] | 15 | **Take photo** → photograph the pump, with location on → use it | The thumbnail appears. Open it: "Small · 1280 × 960 · about 200 KB", **Where** names the town, **Taken** is now. |
| [ ] | 16 | Back on the fuel-up page: **Edit** | The form shows the same **PHOTOS** strip. |
| [ ] | 17 | Open a squawk on `Test truck` (as Sam, one he opened) | **PHOTOS** with **+ Add**; add one. It stays after a job dealt with the squawk too (try one that's Deferred). |
| [ ] | 18 | **Mac**, `admin`, **Maintainer mode**: `Test truck` → **+ Log** → **Repair**, with two tasks (`Front pads`, `Rear pads`) | Under the tasks: "Save the draft to add photos and documents." |
| [ ] | 19 | **Save draft**, then open it again (**Edit**) | Each task card has **PHOTOS & RECEIPTS** with **+ Add**; below the tasks, **PHOTOS & DOCUMENTS** for the job. |
| [ ] | 20 | On **Rear pads**: **+ Add** → **Medium**, tick **Keep the original** → a photo. On the job: **Choose a PDF** → the multi-page scan | The photo shows on Rear pads; the PDF's first page shows as its tile with its page count. |
| [ ] | 21 | Open the photo | **Original** shows beside Download. **Original** downloads the file as taken (same name). |
| [ ] | 22 | Remove **Front pads** (the first task) | A note: "Save the draft to see each task's photos again"; the task strips hide. |
| [ ] | 23 | **Save draft**, then **Edit** again | The photo is on the one task left (**Rear pads**, now first). The PDF is still on the job. |
| [ ] | 24 | **Finish…** → **Finish and lock** | Its page shows the photo and PDF, with **no + Add**; opening the photo shows **no Remove**. |
| [ ] | 25 | **Correct…** → a reason → **Save** | The correction's page shows the same photo and PDF (copies), which can be removed from it; the original keeps them. |
| [ ] | 26 | Open the PDF | Its pages, one under another, white on the dark viewer. "Page 1 of N"; scrolling moves it on. **+** and **−** zoom; **Fit width** goes back. Text can be selected (if the scan has text). |
| [ ] | 27 | On the job (or a fresh draft): **Choose a PDF** → the **DMV or insurer PDF** | Added, with its first page as the tile and the right page count. It opens and shows its pages. *(If it's refused with "couldn't read that file", note which PDF.)* |
| [ ] | 28 | **Choose a PDF** → the one **over 50 MB**; then the **renamed non-PDF** | "PDFs can be up to 50 MB…" (before uploading); then "Documents must be PDFs." |
| [ ] | 29 | **Admin mode** → **Settings** → **Household** → **Keep photo originals**: **Off**. Then **Maintainer mode**, a job draft → **+ Add** | Settings shows Allowed / Off with a note. In the sheet, **no** "Keep the original". The **You** tab shows "Keep photo originals: Off" and that the Admin chooses it. Originals already kept (step 21) can still be downloaded. Set it back to **Allowed**. |
| [ ] | 30 | **Phone**, Sam: open the repair's page from step 24, then its photo | No **Original** button (only Maintainers and Admins). |
| [ ] | 31 | **The file check, with test timers.** In `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, then `sudo docker compose up -d`. Note `find data/files -type f -not -path '*/incoming/*' \| wc -l` | The count of stored files (each photo is two: it and its thumbnail; a PDF one or two; originals and metadata one each). |
| [ ] | 32 | Remove a photo in the app (one on a draft or a squawk). Wait **18 minutes** (the 8 kept "days" at 2 minutes each, and a run) | The count drops by that photo's files (2, or more with an original). A photo removed less than 16 minutes before isn't erased yet. |
| [ ] | 33 | Damage a file: `f=$(find data/files -type f -not -path '*/incoming/*' \| head -1); sudo sh -c "echo x >> $f"`. Wait 2 minutes | **Messages** (Admin mode): **"Some photos or documents are missing or damaged"**, naming the start of the file's name. The log has `stored file problem` with `damaged`. |
| [ ] | 34 | Clean up: in the app, remove the attachment whose file you damaged (the message stays until it's removed, or the file is copied back from a backup of `data/files/`). Then empty `VEHICLEMANAGER_TEST_MINUTES=` again and `sudo docker compose up -d` | The server starts normally (no TEST TIMERS tag). After the next file check, the message is gone. |

## 4. Checklist (part 2)

*Written in step 8i: the Documents tab, license plates (giving the plate from step 1 its state), renewals, and the desktop app (its uploads, Downloads, and HEIC from Photos).*

## 5. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **The step's number,** a screenshot, the device (phone or Mac) and browser, and for a photo or PDF that misbehaves, the file if it's nothing private.
- **Never share** a password, `.env`, a cookie, or a document with personal details on it.

## 6. Results

*Filled in at step 8j.*
