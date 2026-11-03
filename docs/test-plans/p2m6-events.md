# Manual test plan: events, fuel-ups, and readings

*Phase 2, milestone 6 · `docs/phase_plans/phase-2.md` · decisions #008, #030, #033, #034, #040; choices 1–12*

**Part 1** is the halfway checkpoint (step 6i): logging, the readings check, an event's page, finalizing, and corrections. **Part 2** (step 6k) adds the Timeline and Overview.

## 1. Intended behavior

- **+ Log** (the top bar's button, or the floating **+** on a phone) opens a chooser: the vehicle at the top, then **Fuel-up** and **Reading**.
  - It lists only the vehicles you may log for (the Driver role or higher, in Driver mode or above), and only the kinds each has: a trailer has no fuel-ups.
  - It starts on the vehicle whose page you're on, otherwise last time's ("· same as last time"), remembered on that device.
- **The fuel-up form** is its own page:
  - **Open:** the required readings (the odometer), any required custom fields (a field is shown on, and required for, the kinds of events ticked in its Setup, like a meter), the fuel (volume, total price, the price per gallon, full and missed fill), and when and where.
  - **Folded with a summary:** Driving (highway, and towing and hauling for a truck, in 5% steps), Trip and readings, and Who and notes.
  - **The reading form** is the same with every meter open, and no fuel.
- **Required items** are given, or skipped with a reason; the event is flagged. **A place is required too,** or a reason why there isn't one (choice 1).
- **The readings check (choice 3):** a reading must fit between the readings just before and after it in time.
  - One that doesn't is refused, naming the reading it clashes with ("Lower than 52,310 mi on 2 Nov 2026").
  - **The Admin in Admin mode** may keep it with a reason; the event is flagged "Reading kept by the Admin".
  - A back-dated reading that fits is fine for anyone.
- **Drafts and finalizing (#034):**
  - An event is a draft for 24 hours after its last edit, then the server finalizes it.
  - **Finalize now** does it at once, after a confirmation.
  - A draft can be edited or deleted by whoever logged it, or a Maintainer or Admin in that mode.
  - **A finalized event never changes.** A mistake is fixed with **Correct…**: a new event, linked to the original, with a required reason. The correction's readings count instead of the original's.
  - With test timers (#054), "24 hours" is the test minutes.
- **An event's page** opens after Save. It shows the status ("Draft · locks in 23 h" or "Finalized · date"), the flags, the big numbers, each part as a short list, and the record (logged, edited, finalized by whom and when).

## 2. Setup

- **The milestone image:** the "Server image" workflow builds `milestone-p2-m6-events` for each push. Check that Actions shows the newest run ("feat(app): an event's page and corrections") finished successfully.
- **On the NAS over SSH:**

  ```sh
  cd /volume1/docker/vehicle_manager
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m6-events`, and leave `VEHICLEMANAGER_TEST_MINUTES=` empty for now (step 21 turns test timers on). Then press `Esc` and type `:wq`. This milestone's `compose.yaml` is unchanged from milestone 5.
- **People:**
  - Your Admin account (`admin`).
  - Sam, with the **Driver** role.
- **Devices:**
  - A phone, with its browser at the NAS address (HTTPS, through Tailscale), and Location Services on for the browser.
  - The browser on the Mac.
- **About the data:** a finalized event can't be deleted, only corrected, so this test logs on **test vehicles** (step 3). Your real vehicles are untouched. The test vehicles can be removed afterwards (step 24).

## 3. Checklist (part 1)

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager 2>&1` | <ul><li>`backed up before migrating` with `schema_before=8`.</li><li>`database open` with `created=false`, `schema_before=8 schema_now=10`, and the **same database ID** as before.</li><li>`place data opened`, `database integrity check passed`, and `scheduled jobs started`.</li><li>No `ERROR` lines.</li></ul> |
| [x] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v8.sqlite`, owned by `1030 100`. |
| [x] | 3 | **Mac**, as `admin` in **Admin mode**: **+ Add vehicle** → **Truck**, named `Test truck`; then **Trailer**, named `Test camper` | Both are in the Garage. |
| [x] | 4 | `Test truck` → **Setup** | <ul><li>No **Required readings** grid any more.</li><li>The Odometer row says "Required for: Fuel-up".</li><li>**Edit** Odometer: **Required for** checkboxes, Fuel-up ticked. **Cancel**.</li><li>**Edit** Trip time: a **Required** switch under "Shown on forms". Turn it on, **Save**: the row gets a **Required** badge.</li></ul> |
| [x] | 4b | **After updating to the newest image** (fields like meters, and typing durations): pull and restart as in step 1, then `sudo docker logs --since 5m vehiclemanager 2>&1 \| grep "database open"` | `schema_before=10 schema_now=11`, the same database ID. In Admin mode, `Test truck` → **Setup**: Trip time's rows say "Fuel-up · Duration" and "Required for: Fuel-up" (step 4's Required became required for fuel-ups). **Edit** Trip time: **Shown on** checkboxes (Fuel-up ticked) and **Required for** (only the kinds it's shown on). Tick **Shown on: Reading** → **Save**: "Fuel-up, Reading · Duration". Then **Leave** Admin mode. |
| [x] | 5 | **Leave** Admin mode. **Phone**, as `admin` (Driver mode): open `Test truck`, tap the floating **+** | A sheet from the bottom: **Vehicle** `Test truck`, then **Fuel-up** ("Odometer, fuel, and price") and **Reading** ("Just the meters"). Change the vehicle to `Test camper`: only **Reading**. Change it back. |
| [x] | 6 | **Fuel-up** | <ul><li>The page is titled "Fuel-up · Test truck"; the floating **+** is gone; **Save** sits above the bottom tabs.</li><li>**ODOMETER** and **FUEL** are marked **REQUIRED**; **Trip time** is open too (step 4 made it required).</li><li>**When and where**: "Now", and the location picker.</li></ul> |
| [x] | 7 | Odometer `52310`, volume `18.004`, price `62.27` | "3.459 USD per gal" under the price (or your units). |
| [x] | 8 | **Save** with no place yet | Under the place: "Set where it was, or say why you can't." |
| [x] | 9 | **Use my location** → allow → **Use this place** | The place folds to one line, "📍 your town", with **Change**. |
| [x] | 10 | **Save** | Under Trip time: "Fill in the required items, or skip one with a reason." Tap Trip time (labelled "Trip time (HH:MM)"): the phone shows a **number pad**, the box says **HH:MM**, and typing `3` `4` `0` shows `0:03`, `0:34`, `3:40` (the `:` goes in by itself). Clear it again. |
| [x] | 11 | **Can't give it? Skip with a reason** → `Forgot to reset the trip` → open **Driving** → set **Towing** to 25% → tap **Test camper** → **Save** | <ul><li>The sliders move in 5% steps; **Towed** appears once towing is above 0.</li><li>The event's page opens: "Fuel-up · today's date and time", **Draft · locks in 23 h**, and **⚑ Skipped items**.</li><li>Tiles: "52,310 mi", "18.004 gal · filled to full", "62.27 USD · 3.459 USD per gal".</li><li>**Driving**: Towing "25% · Test camper". **Trip and readings**: Trip time "Skipped: …". **Record**: Logged by you, now, typed.</li></ul> |
| [x] | 12 | **Edit** → open **Who and notes** → notes `Pump 4` → **Save** | <ul><li>The form came filled in, with the place folded.</li><li>Back on the page: Notes "Pump 4". **When and where** still says "Device location · ± N m"; **Last edited** is now.</li></ul> |
| [x] | 13 | Floating **+** → the vehicle says "Test truck · same as last time" → **Reading** → Odometer `52000` → place (**Use my location** → **Use this place**) → **Save** | Under the odometer: "Lower than 52,310 mi on today's date. Check it, or correct that event." No "Keep it anyway" (Driver mode). |
| [x] | 14 | **When**: **Change** → yesterday's date → **Save** | It saves: yesterday, 52,000 fits before today's 52,310. Its page says Reading · yesterday. |
| [x] | 15 | **Mac**, Admin mode: `Test truck` → **+ Log** (it starts on Test truck) → **Fuel-up** → Odometer `52100`, volume `5`, place → **Save** | <ul><li>"Lower than 52,310 mi on today. Check it, or correct that event.", and an orange **ADMIN · KEEP IT ANYWAY** box.</li><li>Reason `Odometer replaced` → **Save**: "Saved as a draft." and "The Odometer reading was kept, out of order." → **Done**.</li><li>The page shows **⚑ Reading kept by the Admin**, and **Reading kept because** "Odometer replaced".</li></ul> |
| [x] | 16 | On that draft: **Finalize now** | "Finalize this event?" with "Afterwards it can't be edited or deleted; a mistake is fixed with a correction." **Finalize**: **Finalized · today**; **Record** says finalized by you. Only **Correct…** is offered. |
| [x] | 17 | **Sam** (Driver), in another browser: open the page address of that event (copy it from the Mac) | Sam sees the event, but no **Correct…** (and on a draft, no **Edit**, **Finalize now**, or **Delete**): only whoever logged it, or a Maintainer or Admin in that mode, may. |
| [x] | 18 | **Mac**: that event → **Correct…** | The form is filled in, with **Why? (required)** at the top. **Save** without a reason: "Give a reason." |
| [x] | 19 | Why: `Misread: it was 52,400`; Odometer `52400` → **Save** | <ul><li>The correction's page: "Correction of the fuel-up on today: "Misread: it was 52,400"" with **See the original ›**, **⚑ Correction**, and no "Reading kept" flag (52,400 fits, so nothing needed keeping).</li><li>**See the original**: "Corrected on today by you: …" with **See the correction ›**, and no **Correct…** any more.</li></ul> |
| [x] | 20 | **Delete a draft:** **+ Log** → **Reading** → Odometer `52450`, place → **Save** → **Delete** | "Delete this draft?" → **Delete**: back on the vehicle's page. Reloading the event's old address says "There's no such event, or you can't see it." |
| [x] | 21 | **Test timers:** in `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, run `sudo docker compose up -d`, and reload the app | **TEST TIMERS · 2 min** in the top bar. Admin mode now ends after 2 idle minutes too. |
| [x] | 22 | Open the step 11 fuel-up's page (still a draft). Reload every couple of minutes, for up to 10 minutes (the job's next run was scheduled with normal timers, every 10 minutes; after that it runs every 2) | <ul><li>Before: **Draft · locks in a moment** (its 2 minutes are long past).</li><li>After: **Finalized · today**, and **Record** says finalized by "the server, 24 hours after the last edit". Only **Correct…** is offered.</li><li>`sudo docker logs --since 10m vehiclemanager 2>&1 \| grep finalize-drafts` shows a `job finished` with `finalized 2` (step 11's fuel-up and step 14's reading), or `1` twice.</li></ul> |
| [x] | 23 | `sudo docker compose restart`, then reload the browser | The events are still there with their status, flags, and links. |
| [x] | 24 | **When done:** empty `VEHICLEMANAGER_TEST_MINUTES=` in `.env` and run `sudo docker compose up -d`. *(Optional)* In Admin mode, `Test truck` → **Setup** → **Delete**, and the same for `Test camper` | The test-timers tag is gone. |
| [x] | 25 | **Secrets check:** `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` | `0`. |

**Not covered by hand in part 1:**
- The Timeline and the Overview's latest fill-ups (part 2, step 6k).
- The desktop app: covered in part 2.
