# Test plan: Phase 5, maintenance intelligence

*`docs/phase_plans/phase-5.md` · decisions #080, #081 · built up milestone by milestone, run at the end of the phase*

**How this plan works (#081):** each milestone adds its section here as its last step. Nothing is run until the end of Phase 5. Then the human runs the whole plan on the **test server**, before the `v0.5.0` release, as its regression pass, together with the phase's exit criteria. A failure is fixed on `dev`, and its section is run again.

## Setup (for the whole run)

- **The image:** the test server on `:dev` (or the release PR's merge into `dev`), once every milestone is in. On the NAS over SSH, in `/volume1/docker/vehicle_manager_test`: `.env` has `VEHICLEMANAGER_IMAGE_TAG=dev` and `VEHICLEMANAGER_TEST_MINUTES=` empty.
- **The desktop app, bundled from `dev`:**

  ```sh
  git switch dev
  git pull
  cd crates/app
  dx bundle --platform desktop --package-types app
  ```

  Open the `.app` from `target/dx/app/bundle/macos/` and sign in to the **test server** as `admin`.
- **People:** `admin` (the household's Admin, also a Maintainer) and Sam (Driver).
- **Devices:** the Mac (the desktop app, and a browser) and the phone's browser. **Offline** means **Tailscale off on the Mac**; the phone stays on Tailscale.
- **The vehicle:** `Test truck`, with its odometer.

## If a step fails, capture

- **The step's number,** a screenshot, the device and browser (or the desktop app), and who was signed in, in which mode.
- **Desktop:** Diagnostics → the log viewer (**Area: Sync**, Minimum level: Debug), or the log export.
- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **Never share** a password, a PIN, `.env`, a cookie, a token, or a document with personal details on it.

---

## Milestone 1: meters (percentage, counting down, resettable)

**Intended behavior:**
- A meter can measure a **percentage**. It **counts up or down**; the direction is fixed once it has a reading. It can be **resettable**, with a reset value (e.g. Oil life back to 100%). A distance meter that fuel-ups need can't be resettable.
- A counting-down meter may stay level (Oil life at 0% again and again) but never rise, unless the reading is marked **↺ Reset**. A reset starts afresh. Only a resettable meter's reading can be a reset.
- Resets show ↺ wherever readings show. Existing meters and readings come through migration 0022 unchanged, counting up.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1.1 | **NAS:** `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `backed up before migrating`, then `database open` with `schema_before=21` and `schema_now=22`; `database integrity check passed`; no `ERROR` lines. |
| [ ] | 1.2 | **Mac browser**, `admin`: `Test truck` → **Setup** → **Meters**; then its **Timeline** | The odometer as before ("Distance · miles · whole"), with no "counts down" or "resets". Every reading on the timeline as before, with no ↺. |
| [ ] | 1.3 | **Maintainer mode** → **Setup** → **+ Meter**: name `Oil life`, **Measures: Percentage** (the unit becomes **%**), **Decimal places: whole**, **Counts: Down**, tick **Can be reset** → **Save** | "to [100] %" appears when ticked. Saved; Meters lists "Oil life · Percentage · % · whole · counts down · resets to 100 %". |
| [ ] | 1.4 | Edit the **odometer**, tick **Can be reset** → **Save** | Refused: "A distance meter that fuel-ups need can't be reset: fuel economy needs its running total." The odometer is unchanged. |
| [ ] | 1.5 | **Phone**, Sam: **+** → **Reading** on `Test truck`: Oil life `80` → **Save**. Then another reading a minute later: Oil life `60` → **Save** | Both saved. The odometer has no **↺ Reset** tick; Oil life has one. The timeline shows "80 %" and "60 %". |
| [ ] | 1.6 | Another reading: Oil life `60` again → **Save** | Saved (staying level is fine). |
| [ ] | 1.7 | Another reading: Oil life `70`, **Reset not ticked** → **Save** | Refused under Oil life: "This meter counts down: higher than 60 % on …", with an **↺ It was a reset** button. |
| [ ] | 1.8 | Tap **↺ It was a reset** → **Save** | The message goes and **↺ Reset** is ticked. Saved; the timeline shows "70 % ↺", and the event's page shows a "70 % ↺" tile. |
| [ ] | 1.9 | **+ Reading**: tick **↺ Reset** on Oil life (leave the value it fills) → **Save** | Ticking fills `100`, and a line says "Oil life was reset: 70 → 100 %." Saved; the timeline shows "100 % ↺". |
| [ ] | 1.10 | Edit that reading (while it's a draft) | **↺ Reset** is still ticked, with 100. Cancel. |
| [ ] | 1.11 | **Mac browser**, Maintainer mode: edit **Oil life**, **Counts: Up** → **Save** | Refused: "Which way a meter counts can't change once it has a reading." |
| [ ] | 1.12 | Edit **Oil life**: reset value `95` → **Save** | Saved; Meters says "resets to 95 %". The readings already logged are unchanged. |
| [ ] | 1.13 | `Test truck` → **+ Log** → **Maintenance**: Oil life at the start `90`, a task → **Save** as a draft | Saved; the job's page shows Oil life "90 %". (A job's *end* readings get their reset with milestone 2.) |
| [ ] | 1.14 | **Desktop:** unlock, Sync now; `Test truck` → **Setup** and **Timeline** | Oil life, with its summary, and every reading of steps 1.5–1.13, the ↺ ones marked. |
| [ ] | 1.15 | **Desktop, Tailscale off** (wait for **OFFLINE**): **+ Reading**: Oil life `95` with **Reset not ticked** → **Save** | Refused on the desktop itself, with **↺ It was a reset**. |
| [ ] | 1.16 | Tick **↺ Reset** (it fills `95`) → **Save**; then **Tailscale on**, Sync now; **phone** (reload) | Saved on the desktop with **↑ 1 to send**; after syncing, on the phone with "95 % ↺". |

**Milestone 1 also checks:** the release fixtures' migration (`v0.2.0`, `v0.3.0`) and the reading rules, in the automated tests.

## Milestone 2: what's due

**Intended behavior:**
- Each requirement on a vehicle's schedule, and each document with an expiry, has a step: **Coming up** (yellow), **Due soon** (orange), **Due very soon** (red), **⚠ Overdue** (a darker red), or OK. Each step always shows its word.
- Interval rules warn at 20 / 10 / 5% of the interval left. Dates without an interval (a fixed date, an expiry) warn at 60 / 30 / 14 days. Whichever rule comes first sets the step.
- A requirement counts from its last job, else from **Last done before these records**, else from the first reading ("Not done yet").
- Mileage dates are projected from the last 90 days of readings, shown with "~".
- The household's Admin sets the default warnings. A requirement or a document can have its own.
- A job doing the oil change can **↺ Reset Oil life to 100 %** at its end.
- The Overview's **Coming up** lists the 5 most urgent items, OK ones included. The Garage shows a chip only past OK.
- It works the same on the desktop, offline included.

**Before you start:** `Test truck` has Oil life from milestone 1, at about 95 %. Note its odometer (call it **N**).

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 2.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 23 (`0023_due`); `database integrity check passed`; no `ERROR` lines. |
| [ ] | 2.2 | **Mac browser**, `admin`: the **Garage** | The sort menu offers **Most urgent**, and it's chosen (unless you'd picked another before). Vehicles with nothing coming due have no chip. |
| [ ] | 2.3 | **Admin mode** → **Settings** → **Household** | **Due soon warnings** shows yellow 20, orange 10, and red 5 (% of the interval left), and 60 / 30 / 14 (days before a date). On the **You** tab: "20 / 10 / 5% left · 60 / 30 / 14 days before a date", set by the Admin. |
| [ ] | 2.4 | **Maintainer mode**: `Test truck` → **Maintenance** → **Set up** → **+ Requirement** `Engine oil & filter`: **+ Every** 5,000 mi, **+ Calendar** 12 months, and **+ When a meter reaches** Oil life 15 %. Under **Last done before these records**: a date two months ago, the odometer at **N − 4,700**, and Oil life 100. **Save** | Saved. The dialog shows a reading field for the odometer and for Oil life (the meters its rules use). **Warn me** says "Household default: 20 / 10 / 5% left · 60 / 30 / 14 days before a date". |
| [ ] | 2.5 | **Maintenance** → **Schedule** | The card has an orange edge and **Due soon** (300 mi of 5,000 left: 6 %), with "in 300 mi or by … or Oil life: 80 % to go" in orange, above its three rules. |
| [ ] | 2.6 | The vehicle's **Overview** | **Coming up** lists `Engine oil & filter` with "in 300 mi …" and **Due soon**. **See all on Maintenance →** opens the Schedule. |
| [ ] | 2.7 | The **Garage** | `Test truck`'s card shows **Due soon**, and it's first in its group. |
| [ ] | 2.8 | **Phone**, Sam: **+ Reading** for `Test truck`, the odometer at **N + 400** → **Save**. Then reload the Overview | **⚠ Overdue** in the darker red, with "100 mi over …". The Garage card says **⚠ Overdue** too. |
| [ ] | 2.9 | **Mac browser**, Maintainer: edit the requirement → **Warn me** → **Change**: yellow 40, orange 30, red 10 → **Save**. Edit again → **Use the household's warnings** → **Save** | The steps open, filled with the household's. Saved, it's still **⚠ Overdue** (past is past). It folds back to "Household default …". Yellow 5, orange 10, red 20 is refused: "Each warning must come no earlier than the one before it …". |
| [ ] | 2.10 | `Test truck` → **Documents** → **+ Document**: Insurance, expiring in 20 days → **Save**. Then edit it → **Warn me** → **Change**: 10 / 5 / 2 days → **Save** | First, the Overview's **Coming up** lists Insurance, **Due soon**, "by …". With its own days it's **OK** (20 days is beyond 10), still listed with its **OK** chip. |
| [ ] | 2.11 | **Admin mode** → **Settings** → **Household** → **Due soon warnings**: days 30 / 20 / 10 → **Save**. Then **Use the built-in warnings** | Saved; the You tab's words follow. Back to 20 / 10 / 5 · 60 / 30 / 14. |
| [ ] | 2.12 | **Maintainer mode**: `Test truck` → **+ Log** → **Maintenance**: the task from **Engine oil & filter** (the menu), the odometer at the start | The task shows **↺ Reset Oil life to 100 % at the end of the job**, ticked. |
| [ ] | 2.13 | **Finish…** → **Finish and lock** | The job's page shows Oil life "… → 100 % ↺". The Schedule card is **OK** (green edge), last done today, and **Coming up** shows it **OK**. The Garage card has no chip (unless the insurance still needs one). |
| [ ] | 2.14 | Edit a draft job with the reset ticked (start another, or before finishing in 2.13) | The tick comes back ticked; Oil life isn't among the typed end readings. Unticked and saved, no reset reading is added. |
| [ ] | 2.15 | **Desktop:** Sync now; the Garage, then `Test truck`'s Overview and Maintenance | The same chips, steps, and words as on the web. |
| [ ] | 2.16 | **Desktop, Tailscale off** (wait for **OFFLINE**): **+ Reading** with the odometer 5,100 mi past the job's, then the Overview | Offline, it's worked out on the desktop: the oil change is **⚠ Overdue**, "100 mi over". Tailscale on, Sync now: the web agrees. |
| [ ] | 2.17 | **Phone**, Sam (Driver): `Test truck` → **Overview**, **Maintenance**, and the Garage | Sam sees the chips, **Coming up**, and the Schedule, but no **Set up**, and no **Due soon warnings** on the Household tab. |

**Milestone 2 also checks:** the due arithmetic (steps, whichever comes first, never done, projection, resets, documents), the server's refusals, and the migration of the release fixtures, all in the automated tests.

## Milestone 3: due messages and snooze

**Intended behavior:**
- What's due sends a message from **Due soon** (the household's choice: Coming up, Due soon, or Due very soon) to anyone with a role on the vehicle. Overdue always sends one. Coming up is **Info**; the rest are **Warnings**, with no red banner.
- A due message is titled by vehicle and item ("Test truck: Engine oil & filter"), with its chip and what's left, and **Open on Maintenance** (or **Open Documents**).
- **Snooze ▾**: 1 day, 1 week, 1 month, an amount on the item's meter (500 mi), the two together, or **Custom…**, whichever comes first. Snoozes are each person's own. Snoozed messages wait under **Snoozed (n)**, with **Wake now**.
- **Overdue replaces** a message that was snoozed or dismissed while due soon: a new, unread one. Signing the item off ends its messages.
- The Garage's **fleet strip** (All vehicles): **⚠ Overdue**, **Due soon**, **Open** issues, and on the desktop **Synced**. Each opens **Due & issues** with its filter. (Your household's word for squawks shows throughout.)
- The desktop shows its due messages offline. Snoozing, dismissing, and marking read need the server.

**Before you start:** after milestone 2, `Test truck`'s oil change (every 5,000 mi) is **⚠ Overdue**, "100 mi over" (2.16). Note the odometer's latest reading (call it **M**).

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 3.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 24 (`0024_due_messages`); `database integrity check passed`; no `ERROR` lines. |
| [ ] | 3.2 | **Mac browser**, `admin`: **All vehicles** | Above the groups, three tiles: **⚠ Overdue** at least 1 (in the darker red), **Due soon**, and **Open issues**. No **Synced** tile on the web. A group's own page has no strip. |
| [ ] | 3.3 | Click **⚠ Overdue** | **Due & issues** opens with the **Overdue** chip on: "Engine oil & filter", "Test truck · 100 mi over …", **⚠ Overdue**. Clicking the row opens `Test truck` → **Maintenance**. |
| [ ] | 3.4 | **Due & issues** → **All**, then **Issues** | **All** lists every coming-up item, most urgent first, then the open and deferred issues (each with its vehicle, severity, and status). **Issues** shows only them; a row opens the issue. With nothing to show: "Nothing coming due, and no open issues." |
| [ ] | 3.5 | **Phone**, Sam (Driver on `Test truck`): **Messages** | The unread badge counts it. "Test truck: Engine oil & filter", **Warning** with its time, unread. No red banner under the top bar. |
| [ ] | 3.6 | Open it | The **⚠ Overdue** chip beside the title; "100 mi over …" in the darker red; **Open on Maintenance**, **Snooze ▾**, and **Dismiss**. **Open on Maintenance** opens the Maintenance tab. |
| [ ] | 3.7 | Back in the message: **Snooze ▾** | The menu offers 1 day, 1 week, 1 month, 500 mi, 1 week or 500 mi, and **Custom…** (blue). |
| [ ] | 3.8 | Choose **1 week or 500 mi** | It leaves the list; the badge drops by one. A **Snoozed (1)** chip appears; under it, the message says "Snoozed until … UTC or **M + 500** mi". |
| [ ] | 3.9 | **Mac browser**, `admin` (Jordan, a role on the truck): **Messages** | Jordan still has the oil change message, unread: a snooze is Sam's alone. |
| [ ] | 3.10 | **Phone**, Sam: **+ Reading** for `Test truck`, the odometer at **M + 600** → **Save**, then **Messages** | The message is back in the list (read, as Sam left it), "700 mi over …". **Snoozed** is gone. |
| [ ] | 3.11 | Open it → **Snooze ▾** → **Custom…**: both fields empty → **Snooze**; then Days 400; then Days 2 | Empty and 400 are refused: "That snooze can't be used. Choose days, or an amount more than zero." Days 2: snoozed until two days from now. |
| [ ] | 3.12 | **Snoozed** → open it → **Wake now** | Back in the list at once. |
| [ ] | 3.13 | **Desktop**, Sync now: **All vehicles**, then **Messages** | Four tiles, the last **✓ Synced**, opening Messages. The same due messages as on the web. |
| [ ] | 3.14 | **Desktop, Tailscale off** (wait for **OFFLINE**): **Messages**, **Due & issues**, then **+ Reading** for `Test truck` (**M + 650**) | Offline, Messages still lists the oil change (worked out on the desktop), now "750 mi over". **Snooze ▾** → **1 day** says "Couldn't reach the server." Due & issues works. The sync tile says **– Offline**. |
| [ ] | 3.15 | **Tailscale on**, Sync now | The reading is sent; the tile goes back to **✓ Synced**. Snooze works again. |
| [ ] | 3.16 | **Mac browser**, Maintainer: `Test truck` → **+ Log** → **Maintenance**: the task from **Engine oil & filter** (reset ticked), the odometer at **M + 650** → **Finish and lock** | Sam's and Jordan's oil change messages are gone (reload Messages). The ⚠ Overdue tile drops by one. |
| [ ] | 3.17 | **Phone**, Sam: **+ Reading**, the odometer at **M + 5,350** (300 mi of 5,000 left), then **Messages** → open it → **Snooze ▾** → **1 month** | A new "Test truck: Engine oil & filter", **Due soon**, unread. Snoozed for a month. |
| [ ] | 3.18 | **+ Reading** at **M + 5,700**, then **Messages** | A new message, **⚠ Overdue**, unread: Overdue gets past the month's snooze. **Snoozed** no longer lists the Due soon one. |
| [ ] | 3.19 | **Mac browser**, Maintainer: `Test truck` → **Documents** → Insurance (milestone 2's, about 20 days left) → **Warn me** → **Change**: 30 / 10 / 5 → **Save** | Its step is **Coming up** (yellow). No message about it (the household starts at Due soon). |
| [ ] | 3.20 | **Admin mode** → **Settings** → **Household** → **Messages about what's due start at**: **Coming up**; then **Messages** | The menu offers Coming up, Due soon, and Due very soon. "Test truck: Insurance" appears, **Info**, with the **Coming up** chip and **Open Documents**. Its **Snooze ▾** offers only 1 day, 1 week, 1 month, and Custom (a document has no meter). |
| [ ] | 3.21 | Set it back to **Due soon**; then **Phone**, Sam: **Settings** → **You** | The insurance message goes. Sam's You tab shows "Messages about what's due start at: Due soon", chosen by the Admin, with nothing to change. |

**Milestone 3 also checks:** message keys and cycles, Overdue past a dismiss or a snooze, waking by time or meter, snooze requests and the mark's field, the starting step's checks, who gets messages, retired vehicles left out, the fleet counts matching Due & issues, and migration 0024 on the release fixtures, all in the automated tests.

## Results

# Human Notes
