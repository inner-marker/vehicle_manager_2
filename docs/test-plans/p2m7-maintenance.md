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
| [ ] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager-test 2>&1` | <ul><li>`backed up before migrating` with `schema_before=11`.</li><li>`database open` with `created=false`, `schema_before=11 schema_now=13`, and the **same database ID** as before.</li><li>`place data opened`, `database integrity check passed`, and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [ ] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v11.sqlite`, owned by `1030 100`. |
| [ ] | 3 | **Mac**, as `admin`: open the test server | <ul><li>The sidebar says **Due & issues**.</li><li>`Test truck` → **Overview**: the first card is **OPEN ISSUES**, "No open issues.", with **+ Issue** and **All issues ›**.</li><li>The tabs: Overview, **Issues**, Timeline, **Maintenance**, Fuel, Documents, Setup.</li><li>**Timeline:** milestone 6's fuel-ups and readings are all still there, with their readings.</li></ul> |
| [ ] | 4 | **Admin mode** → **Settings** → **Household** | A **Call problems** row: **Issues** (chosen), Squawks, Faults, Write-ups. |
| [ ] | 5 | Tap **Squawks** | The sidebar changes to **Due & squawks** at once. **You** tab: "Call problems: Squawks", with "The household's Admin chooses this, for everyone." and no buttons. Reload: still Squawks. *(At the end you can choose the word you want to keep.)* |
| [ ] | 6 | **Leave** Admin mode. **Phone**, as **Sam** (Driver): open `Test truck`, tap the floating **+** | The tiles: **Fuel-up**, **Reading**, and **Squawk** ("Something's wrong"). |
| [ ] | 7 | **Squawk** | A sheet, **New squawk · Test truck**: **What's wrong?**, **Severity** (Low chosen), **▸ Notes, when noticed**, **Cancel** and **Save**. |
| [ ] | 8 | What's wrong `Brake lights out, both sides`; tap **High** | High turns red, and a note says "High keeps it out of use: shown as Not drivable." |
| [ ] | 9 | **Save** | <ul><li>The sheet closes.</li><li>Overview's card: **HIGH · Not drivable**, **Open**, "Brake lights out, both sides", "Sam · today".</li><li>The tab says **Squawks · 1**.</li></ul> |
| [ ] | 10 | Card's **+ Squawk** → `Rattle from the rear on bumps` → **▸ Notes, when noticed** → notes `Worse with the trailer on`, Noticed → yesterday → **Save** | The card lists the brake lights first (High), then the rattle (**LOW**, "Sam · yesterday"). **Squawks · 2**. |
| [ ] | 11 | **Save** an empty one (+ Squawk → **Save**) | "Say what's wrong." under the field. **Cancel**. |
| [ ] | 12 | `Test camper` → **+ Squawk** → `Hitch latch sticks` → **High** | The note says "shown as **Not towable**". **Save**: the camper's card says **HIGH · Not towable**. |
| [ ] | 13 | **Mac**, as `admin` (Driver mode): `Test truck` → **Squawks** tab | <ul><li>Filters: **Outstanding · 2** (chosen), **Resolved · 0**, **All**; then **+ Squawk**.</li><li>The two squawks, High first; "Choose a squawk to see its history." beside them.</li></ul> |
| [ ] | 14 | Click the rattle | <ul><li>Beside the list: **LOW**, **Open**, "Rattle from the rear on bumps", "Opened by Sam · yesterday", **NOTES** "Worse with the trailer on", **HISTORY**: "yesterday · Opened · Low".</li><li>**No Edit or Remove:** the admin in Driver mode didn't open it, and only a Maintainer may change someone else's.</li></ul> |
| [ ] | 15 | Switch to **Maintainer** mode (the mode chip) | The rattle now shows **Edit** and **Remove**. |
| [ ] | 16 | **Edit** → severity **Medium** → **Save** | The sheet said **Change this squawk**, filled in. The rattle is now **MEDIUM**; its history's opening says "Opened · Medium" (a severity change before any job is the squawk's own). |
| [ ] | 17 | Brake lights row → its page (from the Overview card) | <ul><li>Links **‹ Test truck** and **‹ Squawks**; the top bar says **Squawk**.</li><li>The chips, the description, "Opened by Sam · today", **HISTORY** with its opening, **Edit** and **Remove**.</li></ul> |
| [ ] | 18 | **Remove a mistake:** + Squawk → `Typo` → **Save**, then open it → **Remove** | "Remove this squawk?" and "Only for one opened by mistake: it's gone for everyone." **Remove**: back on the **Squawks** tab, without it. |
| [ ] | 19 | **Report from an event:** **Timeline** → a fuel-up → **Report a squawk** | The sheet says **New squawk · Test truck**. `Fuel door sticks` → **Save**. |
| [ ] | 20 | **Squawks** tab → `Fuel door sticks` → its page (open it from the Overview card) | "Found on: Fuel-up · <its date> ›". The link opens that fuel-up. |
| [ ] | 21 | **Phone** (Sam): `Test truck` → **Squawks** tab | The list (filters, **+ Squawk**, rows); tapping a row shows that squawk alone, with **‹ Squawks** to go back. Sam's own squawks show **Edit** and **Remove**. |
| [ ] | 22 | **Phone** (Sam): `Test truck` → **Maintenance** | <ul><li>**Schedule** and **Jobs** only (no Set up for a Driver).</li><li>Schedule: "No schedule yet" and "Nothing is tracked yet."</li><li>**Jobs:** "No maintenance or repairs logged yet."</li></ul> |
| [ ] | 23 | **Mac** (Maintainer mode): `Test truck` → **Maintenance** | **Schedule**, **Jobs**, **Set up**. Schedule: "No schedule yet", **Set it up** and **Copy from…**. **Set it up**: the Set up view. |
| [ ] | 24 | **+ Requirement** → Name `Engine oil & filter` → **+ Meter rule** (Odometer) every `5000` → **+ Calendar rule** every `6` → **Save** | <ul><li>The dialog: Name, Notes, **In use** (on), "DUE WHEN, WHICHEVER COMES FIRST", each rule as a row with ✕; **+ Meter rule** goes once the Odometer is used, and **+ Calendar rule** once it's added.</li><li>The row: "Engine oil & filter · Every 5,000 mi or 6 months" (or km).</li></ul> |
| [ ] | 25 | **Refusals:** + Requirement `Wipers` → + Calendar rule every `0` → **Save**; then every `abc` | "Every 1 to 600 months." both times. **Cancel**. |
| [ ] | 26 | + Requirement `Rotate tires` (no rules) → **Save**; + Requirement `State inspection` → **+ Fixed date** → first due: the 30th of next month → **Save**; then **+ Bundle** → `5,000-mile service` → tick Engine oil & filter and Rotate tires → **Save** | <ul><li>"Rotate tires · When needed"; "State inspection · On a fixed date · next 30 <next month>".</li><li>**SERVICE BUNDLES:** "5,000-mile service · Engine oil & filter, Rotate tires".</li><li>**Copy from…** is gone (the truck has requirements now).</li></ul> |
| [ ] | 27 | **Schedule** | A card each: the name and rules, and **LAST DONE** "Not done yet" (jobs arrive in part 2). **SERVICE BUNDLES** below. |
| [ ] | 28 | **Set up** → Rotate tires → **Edit** → turn **In use** off → **Save** | Its row is dimmed with **Off**. **Schedule**: no Rotate tires card; "Turned off: Rotate tires". |
| [ ] | 29 | **Set up** → State inspection → **Remove** | "Remove State inspection?" → **Remove**: gone. (Removing one a job has done is refused; that's part 2.) |
| [ ] | 30 | **Copy onto a trailer:** `Test camper` → **Maintenance** → **Copy from…** | "Copy a schedule to Test camper", **From** another vehicle → choose `Test truck`: "2 requirements and 1 bundle. Copies, never links." and **Left out (no matching meter on Test camper):** "Engine oil & filter: Odometer" (a trailer has no odometer). |
| [ ] | 31 | **Copy** | The camper's Schedule: Engine oil & filter with "Every 6 months" (its odometer rule left out), "Turned off: Rotate tires", and the bundle. |
| [ ] | 32 | **Admin mode** → **+ Add vehicle** → **Truck** `Test van` → its **Maintenance** → **Copy from…** → `Test truck` → **Copy** | Nothing left out (both have an Odometer). The van's Schedule matches the truck's. On the van, **Set up** → **Edit** Engine oil & filter → every `7500` → **Save**: the truck's is still 5,000 (copies, never links). |
| [ ] | 33 | `Test truck` → **Setup** | Below Custom fields: "The maintenance schedule is under Maintenance › Set up ›". It opens the Maintenance tab on **Set up**, and the address ends in `/tab/set-up`. |
| [ ] | 34 | **Phone** (as `admin`, Maintainer mode): `Test truck` → **Maintenance** | The Schedule | Jobs | Set up control spans the screen; the cards show **LAST DONE** under the name; **Set up**'s rows and dialogs fit. |
| [ ] | 35 | **Secrets check:** `sudo docker logs vehiclemanager-test 2>&1 \| grep -ic "password="` | `0`. |
| [ ] | 36 | **When done:** in Admin mode, **Settings › Household › Call problems**: the word you want to keep on the test server | Every screen follows it. |

## 4. Checklist (part 2)

*Written in step 7k: the work log, Finish, corrections, the forgotten job, and the desktop app.*

## 5. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **The step's number,** a screenshot, and the device (phone or Mac).
- **Never share** a password, `.env`, or a cookie.

## 6. Results

*Filled in at step 7l.*
