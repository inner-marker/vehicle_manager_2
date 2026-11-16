# Manual test plan: a vehicle's life, recalls and bulletins, and showing only what each person may use

*Phase 2, milestone 9 · `docs/phase_plans/phase-2.md` · decisions #067, #068, #069; questions 1–12*

One run on the **test server** (decision #062). **Start only once milestone 8 is signed off:** this image upgrades the test server's database to schema 15, **rebuilding two of its tables** (migration 0015), and an older image won't open it afterwards. Step 1 copies the whole data folder first, as well as the backup the server makes itself.

## 1. Intended behavior

- **A vehicle's life is four kinds of event** (#067), in **+ Log** under **VEHICLE'S LIFE**, only when they fit and only for whoever may record them:
  - **Sent out** (a Maintainer, in Maintainer or Admin mode): the shop and a contact, the work order, the due-back date, an estimate, and a **Shipping** fold (carrier and tracking). The vehicle shows **Out for service** (amber) and a banner under its header on every tab, with **Record return**.
  - **Returned** (a Maintainer): offered only while it's out; it shows which send-out it ends ("Back from Joe's Auto, sent out on …") and asks for the final cost. The vehicle goes back to the status Setup gave it.
  - **Acquired** (the vehicle's Admin, in Admin mode): from whom, the price, and the starting readings; offered until one is recorded.
  - **Disposed** (the Admin): how (Sold, Traded in, Scrapped, Given away, Returned to the vendor, Lost or stolen, Other), to whom, the price, the final readings. **Save** asks first. A draft shows **Disposed** with a note that it can still be changed or deleted; once it's **finalized** (24 hours, or **Finalize now**), **nothing new** can be added to the vehicle (no + Log for it, no + Issue, no + Plate or + Document, no photos on it), but its records can still be **corrected**, the disposal too ("It wasn't disposed of after all" brings it back).
  - Each is a draft for 24 hours, like a fuel-up, and shows on the Timeline with its own icon and a one-line summary.
- **An overdue return:** from the day after the due-back date (in the send-out's own time zone), the banner turns amber with "N days late", and **Messages** has **"A vehicle isn't back yet"** (with **Open the send-out**) for whoever sent it out and for Maintainers in Maintainer mode. Recording the return ends it.
- **Recalls and bulletins** (#068) are shared by the household:
  - **Maintenance › Set up › + Recall or bulletin** (a Maintainer) offers **+ A new one**, or one already recorded on the person's other vehicles. A new one is the document (type, issued by, reference numbers, title, what it's about, remedy, dates, revision, applies to as written, web address) and, **On <vehicle>**, that vehicle's requirement for it (its name, **One-time** or **Recurring**, and **Applies**, **Not applicable** with why, or **Under review**).
  - **Its own view** (**Open it ›**): its details, **Open the source ↗**, its **PDF and photos** (the recall's, the same on every vehicle that links it), the vehicles that link it, and **Edit** for a Maintainer of every vehicle that links it (or the Admin in Admin mode).
  - **The Schedule:** a status chip on each requirement (**Open** amber, **Under review** grey, **Complied <date>** green, **Not applicable** grey; a recurring one shows none). Complied and not-applicable ones move to **DONE AND NOT APPLICABLE** at the bottom. A one-time requirement is closed by the first finished job with a task for it.
  - **Any requirement** can be **One-time** (no intervals) or **Recurring**, and apply or not.
- **Each person sees only what they may use** (#069): the role, capped by the mode. **Users** and **Groups** only for the Admin in Admin mode; **+ Log** (and the phone's **+**) only when there's something to log; Settings' **You / Household** picker only for the Admin in Admin mode, and its **Keep photo originals** row only in Maintainer or Admin mode. A form opened from an address the app wouldn't offer says "Your role or mode doesn't allow that."

## 2. Setup

- **The milestone image:** the "Server image" workflow builds `milestone-p2-m9-lifecycle` for each push to `milestone/p2-m9-lifecycle`. Check that Actions shows the newest run finished successfully.
- **On the NAS over SSH, the test server's folder:**

  ```sh
  cd /volume1/docker/vehicle_manager_test
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m9-lifecycle`, and leave `VEHICLEMANAGER_TEST_MINUTES=` empty. Then press `Esc` and type `:wq`. `compose.yaml` is unchanged.
- **The main server is untouched** (it stays on `:dev` until this milestone merges).
- **People:** your Admin account (`admin`), Sam with the **Driver** role, and (step 37) a new **Viewer**.
- **Devices:** a phone with its browser at the test server, and the Mac's browser.
- **Vehicles:** `Test truck` (with its jobs, photos, documents, and plate from milestones 7 and 8). Step 15 adds a second, `Disposal test`, so `Test truck` is never disposed of.
- **Files to have ready:** a recall notice as a PDF (any PDF will do; nothing is shared).

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **Copy the data folder first:** `sudo docker compose stop`, then `sudo cp -a data ../vehicle_manager_test-before-m9`, then `sudo docker compose start` | A full copy of `data/` (the database and `files/`) beside the test server's folder. *(To go back: stop, swap the folders, set the image tag back to the milestone 8 one, start.)* |
| [x] | 2 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1` | <ul><li>`backed up before migrating` with `schema_before=14`.</li><li>`database open` with `created=false`, `schema_before=14 schema_now=15`, and the **same database ID** as before.</li><li>`database integrity check passed` and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [x] | 3 | **Mac**, as `admin`: `Test truck`, then its **Timeline**, **Maintenance**, and **Documents**; open a job with photos, and a PDF | Everything from milestones 7 and 8 is there: every fuel-up, reading, and job, the schedule, the squawks, the photos and PDFs (the PDF opens), the documents and plate. The header shows **In service** (or the status Setup gave it). |
| [x] | 4 | **Maintainer mode** → **+ Log** with `Test truck` | Below the everyday tiles, **VEHICLE'S LIFE** with only **Sent out** ("Off to a shop, and when it's due back"). |
| [x] | 5 | **Sent out**, then **Save** with nothing typed | The top bar says "Sent out · Test truck". Under **THE SHOP**: "Enter the shop or vendor." |
| [x] | 6 | Shop `Joe's Auto`, contact `555-0100`, work order `WO-1182`, **Due back** = **two days ago**, estimate `450`; open **Shipping**: carrier `UPS`, tracking `1Z999`. Where: your location. **Save** | The event's page: **THE SHOP** with "Joe's Auto · 555-0100", the work order, due back, "$450.00", the carrier and tracking. A **Draft** chip. |
| [x] | 7 | `Test truck`'s page | The chip says **Out for service** in amber. Under the header, on every tab: "At Joe's Auto since <today> · due back <that date>", then "**N days late** · WO-1182 · estimate $450.00" in amber, and **Record return**. |
| [x] | 8 | **Messages** | **"A vehicle isn't back yet"**: "Test truck was expected back from service on <that date>…". **Open the send-out** opens the event. |
| [x] | 9 | **+ Log** | **VEHICLE'S LIFE** now offers **Returned** ("Back from the shop, and what it cost"), not Sent out. |
| [x] | 10 | **Phone**, as **Sam** (Driver): `Test truck` | The amber chip and banner, but **no Record return**. **Messages** has no "isn't back yet" (Sam didn't send it out). **+** shows Fuel-up, Reading, and Issue, and **no VEHICLE'S LIFE**. |
| [x] | 11 | **Mac**, `admin` (Maintainer mode): **Record return** | The Returned form: "Back from Joe's Auto, sent out on <today>." and **Final cost**. |
| [x] | 12 | Final cost `512.50`, your location, **Save** | Saved. `Test truck` shows its Setup status again, **no banner**, and the Messages item is gone. |
| [x] | 13 | `Test truck` → **Timeline** | 🚚 **Sent out** ("Joe's Auto · WO-1182 · due back … · $450.00") and ↩ **Returned** ("$512.50"), each a Draft. |
| [x] | 14 | **Admin mode** → **+ Log** with `Test truck` | **VEHICLE'S LIFE**: **Sent out**, **Acquired**, **Disposed**. |
| [x] | 15 | **Acquired**: from `Main St Motors`, price `24000`, the odometer as bought (Readings), a date in the past. **Save**. Then **+ Log** again | Saved, with "How it was acquired" on its page. **+ Log** no longer offers **Acquired**. |
| [x] | 16 | **Add vehicle** → a car named `Disposal test`. Then **+ Log** with it → **Disposed** → **Save** without choosing how | "Choose how it was disposed of." |
| [x] | 17 | How: **Sold**, to `A neighbour`, price `8000`, your location → **Save** | **"Record this disposal?"**: "Once it's finalized (in 24 hours, or with Finalize now), nothing new can be added to Disposal test. Its records can still be corrected." |
| [x] | 18 | **Save the disposal**, then `Disposal test`'s page | A grey **Disposed** chip; the banner: "Sold on <today> to A neighbour" and "The disposal is a draft: it can still be changed or deleted…". **+ Log** still works for it. |
| [x] | 19 | The Garage | `Disposal test` is gone from the list (archived); **Show archived** shows it with **Disposed**. |
| [x] | 20 | Open the disposal (Timeline) → **Finalize now** → confirm. Then `Disposal test`'s page, on a **phone** too | The banner: "Its records stay; they can be corrected, but nothing new can be added." **No + Log or + on its page**, **no + Issue**, **Documents** has no + Plate or + Document, and the Overview's photos have **no + Add**. On the Garage page, **+ Log** doesn't offer `Disposal test`. |
| [x] | 21 | The disposal's page → **Correct…** → tick **It wasn't disposed of after all**, reason `Sale fell through` → **Save** | The form offers **It wasn't disposed of after all** (and hides How, To whom, and Price once it's ticked). The correction saves as a draft, marked as a correction. |
| [x] | 22 | `Disposal test`'s page, and the Garage | Its Setup status again (**In service**), no banner, back in the Garage; **+ Log** offers it again. |
| [x] | 23 | **Maintainer mode** → `Test truck` → **Maintenance** → **Set up** | Beside **+ Requirement**: **+ Recall or bulletin**. |
| [x] | 24 | **+ Recall or bulletin** | **Recall or bulletin**: only **+ A new one** (nothing recorded yet). |
| [x] | 25 | **+ A new one**: Type **Recall**, issued by `NHTSA`; numbers `NHTSA campaign` `24V-123`, then **+ Another number** `Ford` `24S12`; a title over 40 characters (e.g. `Front passenger air bag inflator may rupture during deployment`); a remedy; issued and effective dates; applies to `2021-2024 F-150`; web address `https://www.nhtsa.gov/recalls` | **ON TEST TRUCK**: the requirement's name is the title cut at a word ("Front passenger air bag inflator may"); **One-time** and **Applies** are chosen. |
| [x] | 26 | **Save** | Its view opens: the details, **On** "Test truck", **Open the source ↗**, and **PDF AND PHOTOS** with **+ Add**. |
| [x] | 27 | **+ Add** → **Choose a PDF** → the recall notice → **Add 1**. Then **Open the source ↗** | The PDF's tile, with its page count; it opens. The source opens in a new tab. |
| [x] | 28 | **Close** → **Schedule** | The recall's card: **Open** (amber), "One-time · Recall · NHTSA campaign 24V-123", **Open it ›**, "Not done yet". |
| [x] | 29 | `Disposal test` → **Maintenance** → **Set up** → **+ Recall or bulletin** | Under **ALREADY RECORDED**: the recall, with "Recall · NHTSA campaign 24V-123 · Test truck". |
| [x] | 30 | Pick it | **New requirement**: "Recall · NHTSA campaign 24V-123 · Open it ›" at the top, the name filled in, **One-time** chosen, no intervals. |
| [x] | 31 | **Not applicable** → **Save**; then **Why not?** `Built after the VIN range` → **Save** | First "Say why it doesn't apply…", then saved. **Schedule**: under **DONE AND NOT APPLICABLE**, the recall with **Not applicable** (grey) and "… · Built after the VIN range". |
| [x] | 32 | **Open it ›** on either vehicle | **On**: "Test truck, Disposal test". The PDF is there on both. **Edit** → revision `B` → **Save**: the view shows it, on both vehicles. |
| [x] | 33 | `Test truck` → **+ Log** → **Maintenance**, with a task for the recall's requirement (and the odometer), **Finish…** → **Finish and lock** | The Schedule moves the recall to **DONE AND NOT APPLICABLE** with **Complied <date>** (green), and its job under **LAST DONE**. |
| [x] | 34 | **Set up** → **Edit** the oil requirement: **One-time**, then **Recurring** again → **Cancel** | One-time hides **DUE · WHICHEVER COMES FIRST** and says "Done once: the first finished job that does it closes it."; Recurring brings the intervals back. Nothing saved. |
| [x] | 35 | **Phone**, Sam: `Test truck` → **Maintenance** | **Schedule** and **Jobs** only (no Set up). The recall's **Open it ›** shows its details and PDF, with **no Edit** and **no + Add**. |
| [x] | 36 | **Mac**, `admin`: **Leave** Admin mode (Driver mode), then the sidebar and **Settings** | **No Users or Groups**. Settings has **no You / Household picker** and no **Keep photo originals** row. `Test truck` → **Setup**: "Changing this vehicle's setup needs Maintainer or Admin mode." Back in **Admin mode**: Users, Groups, and the picker are back. |
| [x] | 37 | **Admin mode** → **Users** → add `vera` as a **Viewer**; sign in as her (a private window), choose her password | No **+ Log** and no **+**; `Test truck` has **no + Issue**; **Setup** says "You can see this vehicle's setup; a Maintainer or the Admin changes it."; Settings has no picker and no originals row. |
| [x] | 38 | **Phone**, Sam: Settings, then `Test truck` → **Setup** | No picker, no originals row. Setup: "You can see this vehicle's setup; a Maintainer or the Admin changes it." |
| [x] | 39 | **Phone**, Sam: open `<test server>/vehicles/<Test truck's ID>/log/maintenance` (copy the ID from the address bar on its page) | "Your role or mode doesn't allow that. Switch mode, or ask your Admin." (not a form). |
| [x] | 40 | `sudo docker logs --since 30m vehiclemanager-test 2>&1 \| grep -E "ERROR\|WARN"` | No `ERROR` lines (a `WARN` for a refused change from steps 5, 16, 31, or 39 is fine). |

## 4. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **The step's number,** a screenshot, the device (phone or Mac) and browser, and who was signed in, in which mode.
- **Never share** a password, `.env`, a cookie, or a document with personal details on it.

## 5. Results

**The run** (the human, on the test server, 2026-11-06): **all 40 steps passed**, including a look at the changes below on the next milestone image. Step 33's recall showed **Complied** once the job was finished (first reported as a bug, then withdrawn by the human; a server test now covers it, `a_finished_job_with_a_task_for_a_recall_closes_it`). The human's findings, and what came of each:

- *Recall view: "What it's about" and "Remedy" right-justified; a darker background for what the user filled in.* **Done:** the recall's view puts each label above its value, which sits left-aligned on the input background (`.reference-rows`).
- *Recalls and bulletins listed separately from other requirements, in the Schedule and in the job form's menus.* **Done:** the Schedule has **YOUR SCHEDULE** and **RECALLS AND BULLETINS** sections, and the job form has **+ From a recall or bulletin…** beside **+ From a requirement…** (each listing only its own).
- *A filter on the Schedule: Open and Done, hiding completed one-time ones, or ones not in use.* **Done** (the human chose **Open / All**): **Open**, the default, hides complied, not-applicable, and turned-off requirements and says how many it hides; **All** shows everything, a turned-off one with an **Off** chip. This replaces the "Done and not applicable" section from 9f.

Checked in the browser after the changes: Open showed one requirement and the recall in their sections with "1 more is done, not applicable, or turned off: All shows it."; All added the turned-off one with Off; the job form showed the two menus; the recall's view showed its long text left-aligned on the darker background.

# Human Findings

- On the diologue to display a recall (after clicking "Open it"), the text for "What It's About" and "Remedy" is right-justified. It should be left. Also, can we add a background color (maybe slightly darker grey) to the background of the text that the user filled in to distinguish it from labels?
- Recalls and bulitins should be listed seperatly from other maintenance requirements. This includes in the "Schedule" view, and when logging a maintnance job, a seperate drop-down menu for recalls and bulletins should be available.
- (DISREGARD THIS) ~~**BUG** I added a task from a recall to a maintnance job and marked the job as finished. On the Maintnance->Schedule view, the recall is still listed as "Open" and "Last Done: Not Done Yet". **Expected Behavior:** The recall should be listed as "Done" and the date of the job should be listed as the "Last Done" date.~~
- There should be a filter option on the Maintnance->Schedule view to filter by "Open" and "Done" requirements. This would hide any completed one-time requirement/recall/SB, or for another option, any requirement that is not in-use.