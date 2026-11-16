# Manual test plan: squawks, the maintenance schedule, and maintenance and repairs

*Phase 2, milestone 7 · `docs/phase_plans/phase-2.md` · decisions #031, #032, #034, #051, #063, #064; choices 1–15*

**Part 1** is the checkpoint (step 7h): what squawks are called, opening and changing squawks, and the Maintenance tab's schedule (requirements, bundles, copying a schedule). **Part 2** (step 7k) adds the maintenance and repair work log, **Finish**, corrections, the forgotten job, and the desktop app.

Everything runs on the **test server** (decision #062): its own folder, port 8443, and data.

## 1. Intended behavior

- **What squawks are called** (#064): the household's Admin chooses one word for everyone, in **Settings › Household**: **Issues** (the default), **Squawks**, **Faults**, or **Write-ups**. Every screen uses it ("Due & issues", "+ Issue"). The You tab shows the word, and that the Admin chooses it. This plan says "squawk"; read it as your word.
- **A squawk** is a problem noticed on a vehicle. It has a description, notes, a **severity**, and who opened it and when (#063).
  - **Severity:** Low (where every squawk starts), Medium, or High. **High keeps the vehicle out of use**, and each kind names it: "Not drivable" (car, truck, motorcycle), "Not towable" (trailer), "Out of service" (equipment).
  - **Status:** Open until a maintenance or repair deals with it; then Deferred (with a reason) or Resolved. Jobs arrive in part 2, so every squawk in part 1 stays Open.
- **Opening one** is a quick sheet: what's wrong, and the severity. "▸ Notes, when noticed" unfolds the notes and the date and time. It opens from:
  - **+ Log**'s **Squawk** tile (a Driver or higher);
  - the Overview's card and the Squawks tab (**+ Squawk**);
  - an event's page (**Report a squawk**), which records the event it was found on.
- **Where they show:**
  - **The Overview's card** comes first: the outstanding ones, High first, with **All squawks ›**.
  - **The Squawks tab**, after Overview, counts them ("Squawks · 2"). It has Outstanding / Resolved / All filters, and on a wide screen the list and the chosen one side by side.
  - **A squawk's page:** links back to the vehicle and the tab, the event it was found on, its notes, and its history.
- **Changing and removing:** whoever opened a squawk (while they may still open squawks), or a Maintainer or Admin in that mode, can edit or remove it, **until a job names it**.
- **The Maintenance tab** has a **Schedule | Jobs | Set up** control (choice 15):
  - **Schedule** (everyone): each requirement in use, with its rules on one line and **Last done** (the date, the reading, and the job). Turned-off ones and the bundles are listed below.
  - **Jobs** (everyone): maintenance and repairs, drafts first. Empty until part 2.
  - **Set up** (a Maintainer or Admin, in Maintainer or Admin mode): requirements and bundles, with dialogs.
    - A requirement has a name, notes, **In use**, and rules, **whichever comes first**: every N on a meter, every N months (optionally by the month's end), or a fixed date. With no rules it's done when needed.
    - **A bundle** is a service done as one (e.g. a 5,000-mile service): a name and the requirements it covers.
  - **Copy from…** copies another vehicle's requirements and bundles onto a vehicle with none yet (#031: copies, never links). A meter rule moves to the meter with the same name and kind; one with no match is left out, and the dialog lists it.
- **The Setup tab** keeps meters and custom fields, and links to Maintenance › Set up.

## 2. Setup

- **The milestone image:** the "Server image" workflow builds `milestone-p2-m7-maintenance` for each push to `milestone/p2-m7-maintenance`. Check that Actions shows the newest run ("feat(app): requirements, bundles, and the Maintenance tab") finished successfully.
- **On the NAS over SSH, the test server's folder:**

  ```sh
  cd /volume1/docker/vehicle_manager_test
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m7-maintenance`, and leave `VEHICLEMANAGER_TEST_MINUTES=` empty. Then press `Esc` and type `:wq`. `compose.yaml` is unchanged.
- **The main server is untouched** (it stays on `:dev`).
- **People** (on the test server): your Admin account (`admin`), and Sam with the **Driver** role.
- **Devices:** a phone with its browser at the test server (`https://<nas>.<tailnet>.ts.net:8443`), and the browser on the Mac.
- **Vehicles:** milestone 6's `Test truck` and `Test camper`, with their fuel-ups and readings. Step 27 adds `Test van`.

## 3. Checklist (part 1)

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | <ul><li>`backed up before migrating` with `schema_before=11`.</li><li>`database open` with `created=false`, `schema_before=11 schema_now=13`, and the **same database ID** as before.</li><li>`place data opened`, `database integrity check passed`, and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [x] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v11.sqlite`, owned by `1030 100`. |
| [x] | 3 | **Mac**, as `admin`: open the test server | <ul><li>The sidebar says **Due & issues**.</li><li>`Test truck` → **Overview**: the first card is **OPEN ISSUES**, "No open issues.", with **+ Issue** and **All issues ›**.</li><li>The tabs: Overview, **Issues**, Timeline, **Maintenance**, Fuel, Documents, Setup.</li><li>**Timeline:** milestone 6's fuel-ups and readings are all still there, with their readings.</li></ul> |
| [x] | 4 | **Admin mode** → **Settings** → **Household** | A **Call problems** row: **Issues** (chosen), Squawks, Faults, Write-ups. |
| [x] | 5 | Tap **Squawks** | The sidebar changes to **Due & squawks** at once. **You** tab: "Call problems: Squawks", with "The household's Admin chooses this, for everyone." and no buttons. Reload: still Squawks. *(At the end you can choose the word you want to keep.)* |
| [x] | 6 | **Leave** Admin mode. **Phone**, as **Sam** (Driver): open `Test truck`, tap the floating **+** | The tiles: **Fuel-up**, **Reading**, and **Squawk** ("Something's wrong"). |
| [x] | 7 | **Squawk** | A sheet, **New squawk · Test truck**: **What's wrong?**, **Severity** (Low chosen), **▸ Notes, when noticed**, **Cancel** and **Save**. |
| [x] | 8 | What's wrong `Brake lights out, both sides`; tap **High** | High turns red, and a note says "High keeps it out of use: shown as Not drivable." |
| [x] | 9 | **Save** | <ul><li>The sheet closes.</li><li>Overview's card: **HIGH · Not drivable**, **Open**, "Brake lights out, both sides", "Sam · today".</li><li>The tab says **Squawks · 1**.</li></ul> |
| [x] | 10 | Card's **+ Squawk** → `Rattle from the rear on bumps` → **▸ Notes, when noticed** → notes `Worse with the trailer on`, Noticed → yesterday → **Save** | The card lists the brake lights first (High), then the rattle (**LOW**, "Sam · yesterday"). **Squawks · 2**. |
| [x] | 11 | **Save** an empty one (+ Squawk → **Save**) | "Say what's wrong." under the field. **Cancel**. |
| [x] | 12 | `Test camper` → **+ Squawk** → `Hitch latch sticks` → **High** | The note says "shown as **Not towable**". **Save**: the camper's card says **HIGH · Not towable**. |
| [x] | 13 | **Mac**, as `admin` (Driver mode): `Test truck` → **Squawks** tab | <ul><li>Filters: **Outstanding · 2** (chosen), **Resolved · 0**, **All**; then **+ Squawk**.</li><li>The two squawks, High first; "Choose a squawk to see its history." beside them.</li></ul> |
| [x] | 14 | Click the rattle | <ul><li>Beside the list: **LOW**, **Open**, "Rattle from the rear on bumps", "Opened by Sam · yesterday", **NOTES** "Worse with the trailer on", **HISTORY**: "yesterday · Opened · Low".</li><li>**No Edit or Remove:** the admin in Driver mode didn't open it, and only a Maintainer may change someone else's.</li></ul> |
| [x] | 15 | Switch to **Maintainer** mode (the mode chip) | The rattle now shows **Edit** and **Remove**. |
| [x] | 16 | **Edit** → severity **Medium** → **Save** | The sheet said **Change this squawk**, filled in. The rattle is now **MEDIUM**; its history's opening says "Opened · Medium" (a severity change before any job is the squawk's own). |
| [x] | 17 | Brake lights row → its page (from the Overview card) | <ul><li>Links **‹ Test truck** and **‹ Squawks**; the top bar says **Squawk**.</li><li>The chips, the description, "Opened by Sam · today", **HISTORY** with its opening, **Edit** and **Remove**.</li></ul> |
| [x] | 18 | **Remove a mistake:** + Squawk → `Typo` → **Save**, then open it → **Remove** | "Remove this squawk?" and "Only for one opened by mistake: it's gone for everyone." **Remove**: back on the **Squawks** tab, without it. |
| [x] | 19 | **Report from an event:** **Timeline** → a fuel-up → **Report a squawk** | The sheet says **New squawk · Test truck**. `Fuel door sticks` → **Save**. |
| [x] | 20 | **Squawks** tab → `Fuel door sticks` → its page (open it from the Overview card) | "Found on: Fuel-up · <its date> ›". The link opens that fuel-up. |
| [x] | 21 | **Phone** (Sam): `Test truck` → **Squawks** tab | The list (filters, **+ Squawk**, rows); tapping a row shows that squawk alone, with **‹ Squawks** to go back. Sam's own squawks show **Edit** and **Remove**. |
| [x] | 22 | **Phone** (Sam): `Test truck` → **Maintenance** | <ul><li>**Schedule** and **Jobs** only (no Set up for a Driver).</li><li>Schedule: "No schedule yet" and "Nothing is tracked yet."</li><li>**Jobs:** "No maintenance or repairs logged yet."</li></ul> |
| [x] | 23 | **Mac** (Maintainer mode): `Test truck` → **Maintenance** | **Schedule**, **Jobs**, **Set up**. Schedule: "No schedule yet", **Set it up** and **Copy from…**. **Set it up**: the Set up view. |
| [x] | 24 | **+ Requirement** → Name `Engine oil & filter` → **+ Meter rule** (Odometer) every `5000` → **+ Calendar rule** every `6` → **Save** | <ul><li>The dialog: Name, Notes, **In use** (on), "DUE WHEN, WHICHEVER COMES FIRST", each rule as a row with ✕; **+ Meter rule** goes once the Odometer is used, and **+ Calendar rule** once it's added.</li><li>The row: "Engine oil & filter · Every 5,000 mi or 6 months" (or km).</li></ul> |
| [x] | 25 | **Refusals:** + Requirement `Wipers` → + Calendar rule every `0` → **Save**; then every `abc` | "Every 1 to 600 months." both times. **Cancel**. |
| [x] | 26 | + Requirement `Rotate tires` (no rules) → **Save**; + Requirement `State inspection` → **+ Fixed date** → first due: the 30th of next month → **Save**; then **+ Bundle** → `5,000-mile service` → tick Engine oil & filter and Rotate tires → **Save** | <ul><li>"Rotate tires · When needed"; "State inspection · On a fixed date · next 30 <next month>".</li><li>**SERVICE BUNDLES:** "5,000-mile service · Engine oil & filter, Rotate tires".</li><li>**Copy from…** is gone (the truck has requirements now).</li></ul> |
| [x] | 27 | **Schedule** | A card each: the name and rules, and **LAST DONE** "Not done yet" (jobs arrive in part 2). **SERVICE BUNDLES** below. |
| [x] | 28 | **Set up** → Rotate tires → **Edit** → turn **In use** off → **Save** | Its row is dimmed with **Off**. **Schedule**: no Rotate tires card; "Turned off: Rotate tires". |
| [x] | 29 | **Set up** → State inspection → **Remove** | "Remove State inspection?" → **Remove**: gone. (Removing one a job has done is refused; that's part 2.) |
| [x] | 30 | **Copy onto a trailer:** `Test camper` → **Maintenance** → **Copy from…** | "Copy a schedule to Test camper", **From** another vehicle → choose `Test truck`: "2 requirements and 1 bundle. Copies, never links." and **Left out (no matching meter on Test camper):** "Engine oil & filter: Odometer" (a trailer has no odometer). |
| [x] | 31 | **Copy** | The camper's Schedule: Engine oil & filter with "Every 6 months" (its odometer rule left out), "Turned off: Rotate tires", and the bundle. |
| [x] | 32 | **Admin mode** → **+ Add vehicle** → **Truck** `Test van` → its **Maintenance** → **Copy from…** → `Test truck` → **Copy** | Nothing left out (both have an Odometer). The van's Schedule matches the truck's. On the van, **Set up** → **Edit** Engine oil & filter → every `7500` → **Save**: the truck's is still 5,000 (copies, never links). |
| [x] | 33 | `Test truck` → **Setup** | Below Custom fields: "The maintenance schedule is under Maintenance › Set up ›". It opens the Maintenance tab on **Set up**, and the address ends in `/tab/set-up`. |
| [x] | 34 | **Phone** (as `admin`, Maintainer mode): `Test truck` → **Maintenance** | The Schedule | Jobs | Set up control spans the screen; the cards show **LAST DONE** under the name; **Set up**'s rows and dialogs fit. |
| [x] | 35 | **Secrets check:** `sudo docker logs vehiclemanager-test 2>&1 \| grep -ic "password="` | `0`. |
| [x] | 36 | **When done:** in Admin mode, **Settings › Household › Call problems**: the word you want to keep on the test server | Every screen follows it. |

## 4. Checklist (part 2)

*Step 7k: the work log (tasks, parts, labor, readings at the end, squawks), **Finish** and its checks, a repair and **Repair this**, a job's page, correcting a job, the forgotten job with test timers, the phone, and the desktop app. It carries on from part 1's `Test truck`: its open squawks (the brake lights High, the rattle Medium, the fuel door Low) and its schedule (Engine oil & filter, Rotate tires turned off, the 5,000-mile service).*

**Setup:** the "Server image" run for "feat(app): maintenance on the event page and Timeline" (or newer) finished successfully; `VEHICLEMANAGER_TEST_MINUTES=` is empty. For the desktop app (step 26), on the Mac: `cd crates/app` in the repository, then `dx serve --platform desktop`, with the test server's address (`:8443`) saved.

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | `database open` with `schema_now=13` (no migration this time, so no backup line), `database integrity check passed`, and no `ERROR` lines. |
| [x] | 2 | **Mac**, as `admin` in **Maintainer** mode: `Test truck` → **Maintenance** → **Set up** → Rotate tires → **Edit** → **In use** on → **Save**; then **+ Requirement** `State inspection` → **+ Fixed date** (leave the date empty) → **Save** | Rotate tires is no longer **Off**; "State inspection · On a fixed date". |
| [x] | 2b | **Calendar units** (after the checkpoint's changes): **+ Requirement** `Cabin air filter` → **+ Calendar rule** → the unit menu (**days**, months, years) → **days**, every `90`; then **+ Requirement** `Brake fluid` → years, `2`; then one with years `60` | "By the month's end" goes away for days. The rows: "Every 90 days", "Every 2 years". The third: "Every 1 to 3,650 days, 1 to 600 months, or 1 to 50 years." **Cancel**. On **Schedule**, the 5,000-mile service is a card ("Covers Engine oil & filter, Rotate tires", "SERVICE BUNDLE · 2 requirements"). Remove the two new requirements (Set up → **Remove**). |
| [x] | 3 | **Phone**, as **Sam** (Driver): `Test truck` → **+** | Fuel-up, Reading, and Squawk only: no Maintenance or Repair for a Driver. |
| [x] | 4 | **Mac** (Maintainer): `Test truck` → **+ Log** | The tiles add **Maintenance** ("Service, inspections, the schedule") and **Repair** ("Fixing something wrong"). |
| [x] | 5 | **Maintenance** | <ul><li>The page is titled **Maintenance · Test truck**.</li><li>Every meter is open (the readings at the start), then **+ Readings at the end (after a test drive)**.</li><li>**TASKS**: **+ From a requirement…**, **+ From a bundle…**, **+ Task**, and "No tasks yet…".</li><li>**SQUAWKS · 3 TO SIGN OFF OR DEFER**: the brake lights (**HIGH · Not drivable**), the rattle, the fuel door, each with **Sign off** and **Defer**.</li><li>**When and where**, **Who and notes**, and a bar with **TOTAL 0.00 USD**, **Save draft**, and **Finish…**.</li></ul> |
| [x] | 6 | **+ From a bundle…** → `5,000-mile service` | Two tasks, folded: "▸ Engine oil & filter" and "▸ Rotate tires", each "0 parts · 0.00 USD". |
| [x] | 7 | Tap **Engine oil & filter** → its first part: `Oil filter`, `FL-500S`, quantity `1`, unit price `12.99` → **+ Part** → `Engine oil 5W-30`, quantity `5.5`, unit price `8.99` → Labor: type `030` → Labor cost `45` | <ul><li>"For: Engine oil & filter".</li><li>Labor shows **0:30** as you type.</li><li>"Task: 107.44 USD", and the bar's total **107.44 USD**.</li></ul> |
| [x] | 8 | **Resets the requirement from**: **End readings** | The **Readings at the end** section opens, with each meter. |
| [x] | 9 | **+ From a requirement…** → `State inspection` | A new task, open, with **Next due (from the paperwork)**: pick a date next year. (The requirement menu no longer lists what's already a task.) |
| [x] | 10 | **+ Task** → title `Brake light bulbs` → part `Bulb 1157`, quantity `2`, unit price `4.99` → Labor `010`, cost `15` | The total is now **132.42 USD**. A task folded to one line shows its parts, labor, and cost. |
| [x] | 11 | Brake lights → **Sign off** → **Fixed by** `Brake light bulbs` → notes `New bulbs` | The card turns green. Tapping **Sign off** again undoes it; tap it once more. |
| [x] | 12 | The odometer at the start: the truck's latest + 10; set the place → **Save draft** | <ul><li>The job's page: "Draft · finish it when the work is done", with **Edit**, **Finish…**, **Delete** (no Finalize now).</li><li>**Maintenance › Jobs**: the job under **IN PROGRESS**, with **Draft**.</li></ul> |
| [x] | 13 | **Finish…** (on the page) | The work log again, as saved. Its **Finish…** → "Finish this job?": ✓ the required readings, ✓ where it was done, ✕ "Rattle…: sign it off or defer it", ✕ "Fuel door sticks: …"; **Finish and lock** is off. **Back**. |
| [x] | 14 | Rattle → **Defer** → why `Parts on order`, severity **High**; fuel door → **Sign off** (no task); the odometer at the end: start + 5 → **Finish…** | Every check ✓. **Finish and lock**: the job's page, **Finished · today**, with **Correct…** and no **Edit** or **Delete**. |
| [x] | 15 | The job's page | <ul><li>**‹ Test truck** and **‹ Jobs**.</li><li>Tiles: **TOTAL 132.42 USD** "parts … · labor 60.00 USD", **TASKS 4**, **ODOMETER** "start → end mi", **LABOR 0:40**.</li><li>**TASKS**: Engine oil & filter open (its two parts and labor, "resets from the end readings"), the others folded; State inspection shows its next due date.</li><li>**SQUAWKS DEALT WITH**: the brake lights "Signed off · by Brake light bulbs · New bulbs", the rattle "Deferred · Parts on order · → High", the fuel door "Signed off".</li></ul> |
| [x] | 16 | `Test truck` → **Overview** and **Squawks** | The card: only the rattle, now **HIGH · Not drivable**, **Deferred**. The tab: **Outstanding · 1**, **Resolved · 2**; the brake lights' history: "Job · today › / Signed off: New bulbs". |
| [x] | 17 | **Maintenance › Schedule** | Engine oil & filter and Rotate tires: **LAST DONE** today with a reading (the oil change's is the **end** reading, as it resets from the end); State inspection "On a fixed date · next <the date from step 9>". **Set up** → Engine oil & filter → **Remove** → **Remove**: "A task has done this requirement, so it stays in the history. Turn it off instead." **Cancel**. |
| [x] | 18 | **Repair this:** the rattle's page | **Repair this** (you're a Maintainer). Tap it: the top bar says **Repair**; the **Complaint** is filled in with the rattle's description and its notes ("Worse with the trailer on"); the rattle is already **Sign off**; and a **REPAIR** section has Complaint and Cause. |
| [x] | 19 | **+ Task** `Tighten exhaust hanger`, labor `015`; Cause `Loose hanger`; the odometer; the place → **Finish…** → **Finish and lock** | Finished. Its page shows **REPAIR** (complaint and cause) above the tasks; the rattle is **Resolved**, and the Overview card says "No open squawks." |
| [x] | 20 | **A squawk found during a job:** **+ Log** → **Maintenance** → **+ Found something new? Report a squawk** → `Coolant smell` → **Save** | The sheet closes; "Coolant smell" joins the job's list, undecided (the count goes up). Then **Save draft** → **Delete** → **Delete**. The squawk stays open on the truck. |
| [x] | 21 | **Correct a job:** step 14's job → **Correct…** → Why `Wrong oil quantity` → Engine oil 5W-30 quantity `6` → **Finish…** | The form came filled in (tasks and parts). Its squawks: the brake lights and the fuel door (signed off by the original, so the correction signs them off again) and **Coolant smell** (outstanding now), but **not the rattle**: step 19's repair resolved it since. Finish lists ✕ "Coolant smell". **Back** → Coolant smell → **Defer** `Watch it` → **Finish…** → **Finish and lock**. |
| [x] | 22 | The correction's page, then **Maintenance › Jobs** | "Correction of the maintenance on today: "Wrong oil quantity"" with **See the original ›**; the total is 4.49 USD higher (each part's cost is rounded to the cent). In Jobs, the original sits dimmed beneath its correction. The rattle is still **Resolved**. |
| [x] | 23 | **The forgotten job, with test timers:** in the test server's `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, `sudo docker compose up -d`, reload. **+ Log** → **Repair** → a task `Wiper blade`, the odometer, the place → **Save draft** | **TEST TIMERS · 2 min** in the top bar. The draft's page. |
| [x] | 24 | Wait 3 minutes (Messages reload every minute with test timers), then **Messages** | **A job is still a draft**: "A maintenance or repair job on Test truck hasn't been touched for a while…", with **Open the job**: it opens the draft. **Sam** (who didn't log it, and isn't a Maintainer) has no such message. Then **Delete** the draft, empty `VEHICLEMANAGER_TEST_MINUTES=`, and `up -d`. |
| [x] | 25 | **Phone** (as `admin`, Maintainer mode): **+** → **Maintenance** on `Test truck` → **+ Task** with a part | Each part on three short rows (what; number and quantity; price and ✕); the bar's total and both buttons on one line, above the bottom tabs. A finished job's page: tiles two by two, the parts table without the part number column. **Cancel** out (or **Save draft** → **Delete**). |
| [x] | 26 | **Desktop app:** sign in → `Test truck` → **Maintenance › Jobs** → open the corrected job; then **+ Log** → **Maintenance** → a task → **Save draft** → **Delete** | The job's page as on the Mac's browser; the work log saves and deletes the same way. |
| [x] | 27 | **The word:** Admin mode → **Settings › Household › Call problems** → **Faults**; open a new **Maintenance** form | "FAULTS · 1 TO SIGN OFF OR DEFER" and "+ Found something new? Report a fault"; the job's page says "FAULTS DEALT WITH". Set it back to your word. |
| [x] | 28 | `sudo docker compose restart`, then reload | The jobs, squawks, and schedule are unchanged. |
| [x] | 29 | **Secrets check:** `sudo docker logs vehiclemanager-test 2>&1 \| grep -ic "password="` | `0`. |

**Not covered by hand:**
- What's due, due dates, and readiness (Phase 5; High squawks are counted then).
- Photos and documents on squawks and tasks (milestone 8).
- Recalls and bulletins (milestone 9).

## 5. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **The step's number,** a screenshot, and the device (phone or Mac).
- **Never share** a password, `.env`, or a cookie.

## 6. Results

**Part 1** (the human, on the test server, 2026-11-03): done. Notes, each fixed before part 2:
- **The work log:** + Task (and the requirement and bundle menus) should be below the last task, so tasks are added at the bottom; and the triangle before a task's title should fold and unfold it. Now the add controls sit under the tasks, and ▸ / ▾ is the toggle (▾ on the open task folds it).
- **A squawk's page:** **Repair this** was on its own line; it now sits with **Edit** and **Remove**.
- **Calendar rules** should count days, months, or years. Now "Every 90 days", "Every 6 months", "Every 2 years" (by the month's end only for months and years). Rules saved before read as months.
- **Schedule:** the service bundles should be cards like the requirements. Now each is a card with what it covers.
- **For a later phase:** chained meter intervals ("after the first 100,000 miles, then every 50,000"), recorded as idea 21 in `docs/ideas-to-discuss.md`.

**Part 2** (the human, on the test server, 2026-11-04): all 30 steps passed. The human's notes, as written, and what came of each:

- *"On the phone web browser, in a maintenance event, the labels above each field (what, part number, quantity, unit price) are not visible."* On a phone the parts table hides its column headings to fit, and the fields had labels only for screen readers. **Fixed:** each field now has its own label on a phone (What, Part number, Quantity, Unit price); a wide screen keeps the headings.
- *"Can we display a currency symbol ($, £, €, etc.) in front of the unit price and total cost fields?"* Then: *a drop-down with a list of currencies, with the household's default currency at the top as the first item and the default.* **Done:**
  - **Every amount shows its currency's symbol:** "$" for the person's own dollars, "CA$", "A$", "MX$" for others; "€", "£", "¥"; otherwise the code ("CHF 45.00"). The unit price, labor cost, and fuel price fields show it in front.
  - **Jobs and fuel-ups have a Currency menu:** the person's currency (the household's unless they chose their own) first and chosen, then the common ones. A saved event keeps its own.
- *"Labor cost should be a setting available to maintainers and admins. Entering labor time should auto-populate labor cost with the ability to override."* **Added to Phase 6 (Polish)** in the plan, with whose rate it is (the household's, each Maintainer's, or each shop's) as an open question there.
- *"All of the 'Where' fields [should] live inside another wireframe or box … This section and the 'Who and notes' section should have the same triangles to fold and unfold."* **Done:** **When and where** is a fold like **Who and notes**, open while the place is still missing and folded once there is one, with its summary ("Now · Asheville, NC"). Every fold now has the task cards' ▸ / ▾ (the browser's own marker didn't show).
- *"Meters at the end of a maintenance event should have a small 'Copy from Start' button."* **Done:** under each end reading, when its start reading is filled in.

Checked in the browser at a phone's width (headless Chrome): the labels, the symbols ($, then CA$ after choosing CAD, the total too), the menu's order, the folds and their ▸ / ▾, and Copy from start.
