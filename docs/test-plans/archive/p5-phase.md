# Test plan: Phase 5, maintenance intelligence

*`docs/phase_plans/phase-5.md` · decisions #080, #081 · built up milestone by milestone, run at the end of the phase*

**How this plan works (#081):** each milestone adds its section here as its last step. Nothing is run until the end of Phase 5. Then the human runs the whole plan on the **test server**, before the `v0.5.0` release, as its regression pass. Its last part, **Earlier releases and the exit criteria**, checks `v0.2.0`'s and `v0.3.0`'s main paths and matches each exit criterion to its steps. A failure is fixed on `dev`, and its section is run again.

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
| [x] | 1.1 | **NAS:** `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `backed up before migrating`, then `database open` with `schema_now=27` (`schema_before` is what the test server had: 21 on `v0.3.0`); `default usages made` and `meter keys given`; `database integrity check passed`; no `ERROR` lines. **One update for the whole run:** steps 2.1–7.1 read this same log, and each one's "at least" holds at 27. |
| [x] | 1.2 | **Mac browser**, `admin`: `Test truck` → **Setup** → **Meters**; then its **Timeline** | The odometer as before ("Distance · miles · whole"), with no "counts down" or "resets". Every reading on the timeline as before, with no ↺. |
| [x] | 1.3 | **Maintainer mode** → **Setup** → **+ Meter**: name `Oil life`, **Measures: Percentage** (the unit becomes **%**), **Decimal places: whole**, **Counts: Down**, tick **Can be reset** → **Save** | "to [100] %" appears when ticked. Saved; Meters lists "Oil life · Percentage · % · whole · counts down · resets to 100 %". |
| [x] | 1.4 | Edit the **odometer**, tick **Can be reset** → **Save** | Refused: "A distance meter that fuel-ups need can't be reset: fuel economy needs its running total." The odometer is unchanged. |
| [x] | 1.5 | **Phone**, Sam: **+** → **Reading** on `Test truck`: Oil life `80` → **Save**. Then another reading a minute later: Oil life `60` → **Save** | Both saved. The odometer has no **↺ Reset** tick; Oil life has one. The timeline shows "80 %" and "60 %". |
| [x] | 1.6 | Another reading: Oil life `60` again → **Save** | Saved (staying level is fine). |
| [x] | 1.7 | Another reading: Oil life `70`, **Reset not ticked** → **Save** | Refused under Oil life: "This meter counts down: higher than 60 % on …", with an **↺ It was a reset** button. |
| [x] | 1.8 | Tap **↺ It was a reset** → **Save** | The message goes and **↺ Reset** is ticked. Saved; the timeline shows "70 % ↺", and the event's page shows a "70 % ↺" tile. |
| [x] | 1.9 | **+ Reading**: tick **↺ Reset** on Oil life (leave the value it fills) → **Save** | Ticking fills `100`, and a line says "Oil life was reset: 70 → 100 %." Saved; the timeline shows "100 % ↺". |
| [x] | 1.10 | Edit that reading (while it's a draft) | **↺ Reset** is still ticked, with 100. Cancel. |
| [x] | 1.11 | **Mac browser**, Maintainer mode: edit **Oil life**, **Counts: Up** → **Save** | Refused: "Which way a meter counts can't change once it has a reading." |
| [x] | 1.12 | Edit **Oil life**: reset value `95` → **Save** | Saved; Meters says "resets to 95 %". The readings already logged are unchanged. |
| [x] | 1.13 | `Test truck` → **+ Log** → **Maintenance**: Oil life at the start `90`, a task → **Save** as a draft | Saved; the job's page shows Oil life "90 %". (A job's *end* readings get their reset with milestone 2.) |
| [x] | 1.14 | **Desktop:** unlock, Sync now; `Test truck` → **Setup** and **Timeline** | Oil life, with its summary, and every reading of steps 1.5–1.13, the ↺ ones marked. |
| [x] | 1.15 | **Desktop, Tailscale off** (wait for **OFFLINE**): **+ Reading**: Oil life `95` with **Reset not ticked** → **Save** | Refused on the desktop itself, with **↺ It was a reset**. |
| [x] | 1.16 | Tick **↺ Reset** (it fills `95`) → **Save**; then **Tailscale on**, Sync now; **phone** (reload) | Saved on the desktop with **↑ 1 to send**; after syncing, on the phone with "95 % ↺". |

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
| [x] | 2.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 23 (`0023_due`); `database integrity check passed`; no `ERROR` lines. |
| [x] | 2.2 | **Mac browser**, `admin`: the **Garage** | The sort menu offers **Most urgent**, and it's chosen (unless you'd picked another before). Vehicles with nothing coming due have no chip. |
| [x] | 2.3 | **Admin mode** → **Settings** → **Household** | **Due soon warnings** shows yellow 20, orange 10, and red 5 (% of the interval left), and 60 / 30 / 14 (days before a date). On the **You** tab: "20 / 10 / 5% left · 60 / 30 / 14 days before a date", set by the Admin. |
| [x] | 2.4 | **Maintainer mode**: `Test truck` → **Maintenance** → **Set up** → **+ Requirement** `Engine oil & filter`: **+ Every** 5,000 mi, **+ Calendar** 12 months, and **+ When a meter reaches** Oil life 15 %. Under **Last done before these records**: a date two months ago, the odometer at **N − 4,700**, and Oil life 100. **Save** | Saved. The dialog shows a reading field for the odometer and for Oil life (the meters its rules use). **Warn me** says "Household default: 20 / 10 / 5% left · 60 / 30 / 14 days before a date". |
| [x] | 2.5 | **Maintenance** → **Schedule** | The card has an orange edge and **Due soon** (300 mi of 5,000 left: 6 %), with "in 300 mi or by … or Oil life: 80 % to go" in orange, above its three rules. |
| [x] | 2.6 | The vehicle's **Overview** | **Coming up** lists `Engine oil & filter` with "in 300 mi …" and **Due soon**. **See all on Maintenance →** opens the Schedule. |
| [x] | 2.7 | The **Garage** | `Test truck`'s card shows **Due soon**, and it's first in its group. |
| [x] | 2.8 | **Phone**, Sam: **+ Reading** for `Test truck`, the odometer at **N + 400** → **Save**. Then reload the Overview | **⚠ Overdue** in the darker red, with "100 mi over …". The Garage card says **⚠ Overdue** too. |
| [x] | 2.9 | **Mac browser**, Maintainer: edit the requirement → **Warn me** → **Change**: yellow 40, orange 30, red 10 → **Save**. Edit again → **Use the household's warnings** → **Save** | The steps open, filled with the household's. Saved, it's still **⚠ Overdue** (past is past). It folds back to "Household default …". Yellow 5, orange 10, red 20 is refused: "Each warning must come no earlier than the one before it …". |
| [x] | 2.10 | `Test truck` → **Documents** → **+ Document**: Insurance, expiring in 20 days → **Save**. Then edit it → **Warn me** → **Change**: 10 / 5 / 2 days → **Save** | First, the Overview's **Coming up** lists Insurance, **Due soon**, "by …". With its own days it's **OK** (20 days is beyond 10), still listed with its **OK** chip. |
| [x] | 2.11 | **Admin mode** → **Settings** → **Household** → **Due soon warnings**: days 30 / 20 / 10 → **Save**. Then **Use the built-in warnings** | Saved; the You tab's words follow. Back to 20 / 10 / 5 · 60 / 30 / 14. |
| [x] | 2.12 | **Maintainer mode**: `Test truck` → **+ Log** → **Maintenance**: the task from **Engine oil & filter** (the menu), the odometer at the start | The task shows **↺ Reset Oil life to 100 % at the end of the job**, ticked. |
| [x] | 2.13 | **Finish…** → **Finish and lock** | The job's page shows Oil life "… → 100 % ↺". The Schedule card is **OK** (green edge), last done today, and **Coming up** shows it **OK**. The Garage card has no chip (unless the insurance still needs one). |
| [x] | 2.14 | Edit a draft job with the reset ticked (start another, or before finishing in 2.13) | The tick comes back ticked; Oil life isn't among the typed end readings. Unticked and saved, no reset reading is added. |
| [x] | 2.15 | **Desktop:** Sync now; the Garage, then `Test truck`'s Overview and Maintenance | The same chips, steps, and words as on the web. |
| [x] | 2.16 | **Desktop, Tailscale off** (wait for **OFFLINE**): **+ Reading** with the odometer 5,100 mi past the job's, then the Overview | Offline, it's worked out on the desktop: the oil change is **⚠ Overdue**, "100 mi over". Tailscale on, Sync now: the web agrees. |
| [x] | 2.17 | **Phone**, Sam (Driver): `Test truck` → **Overview**, **Maintenance**, and the Garage | Sam sees the chips, **Coming up**, and the Schedule, but no **Set up**, and no **Due soon warnings** on the Household tab. |

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
| [x] | 3.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 24 (`0024_due_messages`); `database integrity check passed`; no `ERROR` lines. |
| [x] | 3.2 | **Mac browser**, `admin`: **All vehicles** | Above the groups, three tiles: **⚠ Overdue** at least 1 (in the darker red), **Due soon**, and **Open issues**. No **Synced** tile on the web. A group's own page has no strip. |
| [x] | 3.3 | Click **⚠ Overdue** | **Due & issues** opens with the **Overdue** chip on: "Engine oil & filter", "Test truck · 100 mi over …", **⚠ Overdue**. Clicking the row opens `Test truck` → **Maintenance**. |
| [x] | 3.4 | **Due & issues** → **All**, then **Issues** | **All** lists every coming-up item, most urgent first, then the open and deferred issues (each with its vehicle, severity, and status). **Issues** shows only them; a row opens the issue. With nothing to show: "Nothing coming due, and no open issues." |
| [x] | 3.5 | **Phone**, Sam (Driver on `Test truck`): **Messages** | The unread badge counts it. "Test truck: Engine oil & filter", **Warning** with its time, unread. No red banner under the top bar. |
| [x] | 3.6 | Open it | The **⚠ Overdue** chip beside the title; "100 mi over …" in the darker red; **Open on Maintenance**, **Snooze ▾**, and **Dismiss**. **Open on Maintenance** opens the Maintenance tab. |
| [x] | 3.7 | Back in the message: **Snooze ▾** | The menu offers 1 day, 1 week, 1 month, 500 mi, 1 week or 500 mi, and **Custom…** (blue). |
| [x] | 3.8 | Choose **1 week or 500 mi** | It leaves the list; the badge drops by one. A **Snoozed (1)** chip appears; under it, the message says "Snoozed until … UTC or **M + 500** mi". |
| [x] | 3.9 | **Mac browser**, `admin` (Jordan, a role on the truck): **Messages** | Jordan still has the oil change message, unread: a snooze is Sam's alone. |
| [x] | 3.10 | **Phone**, Sam: **+ Reading** for `Test truck`, the odometer at **M + 600** → **Save**, then **Messages** | The message is back in the list (read, as Sam left it), "700 mi over …". **Snoozed** is gone. |
| [x] | 3.11 | Open it → **Snooze ▾** → **Custom…**: both fields empty → **Snooze**; then Days 400; then Days 2 | Empty and 400 are refused: "That snooze can't be used. Choose days, or an amount more than zero." Days 2: snoozed until two days from now. |
| [x] | 3.12 | **Snoozed** → open it → **Wake now** | Back in the list at once. |
| [x] | 3.13 | **Desktop**, Sync now: **All vehicles**, then **Messages** | Four tiles, the last **✓ Synced**, opening Messages. The same due messages as on the web. |
| [x] | 3.14 | **Desktop, Tailscale off** (wait for **OFFLINE**): **Messages**, **Due & issues**, then **+ Reading** for `Test truck` (**M + 650**) | Offline, Messages still lists the oil change (worked out on the desktop), now "750 mi over". **Snooze ▾** → **1 day** says "Couldn't reach the server." Due & issues works. The sync tile says **– Offline**. |
| [x] | 3.15 | **Tailscale on**, Sync now | The reading is sent; the tile goes back to **✓ Synced**. Snooze works again. |
| [x] | 3.16 | **Mac browser**, Maintainer: `Test truck` → **+ Log** → **Maintenance**: the task from **Engine oil & filter** (reset ticked), the odometer at **M + 650** → **Finish and lock** | Sam's and Jordan's oil change messages are gone (reload Messages). The ⚠ Overdue tile drops by one. |
| [x] | 3.17 | **Phone**, Sam: **+ Reading**, the odometer at **M + 5,350** (300 mi of 5,000 left), then **Messages** → open it → **Snooze ▾** → **1 month** | A new "Test truck: Engine oil & filter", **Due soon**, unread. Snoozed for a month. |
| [x] | 3.18 | **+ Reading** at **M + 5,700**, then **Messages** | A new message, **⚠ Overdue**, unread: Overdue gets past the month's snooze. **Snoozed** no longer lists the Due soon one. |
| [x] | 3.19 | **Mac browser**, Maintainer: `Test truck` → **Documents** → Insurance (milestone 2's, about 20 days left) → **Warn me** → **Change**: 30 / 10 / 5 → **Save** | Its step is **Coming up** (yellow). No message about it (the household starts at Due soon). |
| [x] | 3.20 | **Admin mode** → **Settings** → **Household** → **Messages about what's due start at**: **Coming up**; then **Messages** | The menu offers Coming up, Due soon, and Due very soon. "Test truck: Insurance" appears, **Info**, with the **Coming up** chip and **Open Documents**. Its **Snooze ▾** offers only 1 day, 1 week, 1 month, and Custom (a document has no meter). |
| [x] | 3.21 | Set it back to **Due soon**; then **Phone**, Sam: **Settings** → **You** | The insurance message goes. Sam's You tab shows "Messages about what's due start at: Due soon", chosen by the Admin, with nothing to change. |

**Milestone 3 also checks:** message keys and cycles, Overdue past a dismiss or a snooze, waking by time or meter, snooze requests and the mark's field, the starting step's checks, who gets messages, retired vehicles left out, the fleet counts matching Due & issues, and migration 0024 on the release fixtures, all in the automated tests.

## Milestone 4: specs and service bundles

**Intended behavior:**
- A vehicle's **Specs** (Maintenance → Specs) each have a name, an amount in its own unit with the other system beside it ("5.5 qt (5.2 L)"), a detail, a part number, and a note. Everyone who sees the schedule sees them; Maintainers change them.
- A requirement ticks the specs its job uses. A task for it gets a **part** for each volume spec and each part-number spec, and a **reminder** for each torque, pressure, or length spec.
- Saving a job whose pre-filled part differs from its spec asks **"Update the specs too?"**, each ticked; the job records what was used either way.
- **Log this service** on a bundle card starts a job with a task for each of its requirements, parts pre-filled. Finishing it resets each requirement's own clock.
- **Templates** (Manage → Templates): a vehicle's requirements, bundles, and specs saved under a name, and copied onto a vehicle with no schedule yet. The Admin renames and removes them.
- Copy schedule brings the specs too.

**Before you start:** `Test truck` has the oil change from milestone 2 (every 5,000 mi, 12 months, Oil life 15 %). Note its odometer (call it **K**).

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 4.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 25 (`0025_specs`); `database integrity check passed`; no `ERROR` lines. |
| [x] | 4.2 | **Mac browser**, Maintainer mode: `Test truck` → **Maintenance** | The views are **Schedule · Specs · Jobs · Set up**. **Specs** says "No specs yet …" and offers **+ Spec**. |
| [x] | 4.3 | **+ Spec**: Engine oil; Kind **Volume**; Amount 5.5, Unit **qt**; Detail "5W-30 full synthetic" → **Save**. Then **+ Spec** Oil filter (Kind **None**, Part number FL-500S), and **+ Spec** Drain plug torque (**Torque**, 30 **ft-lb**) | While typing 5.5 qt the dialog shows "= 5.2 L". The list, by name, each with a line under it: Drain plug torque "30 ft-lb (41 N·m)"; Engine oil "5.5 qt (5.2 L) · 5W-30 full synthetic"; Oil filter "Part FL-500S". |
| [x] | 4.4 | Open Engine oil → Unit **L** → **Save**; then back to **qt** → **Save**. Type Amount "lots" → **Save** | Shown as "5.5 L (5.8 qt)", then "5.5 qt (5.2 L)" again (as entered, never rounded through the other unit). "lots" is refused: "The amount must be more than zero." |
| [x] | 4.5 | **Set up** → edit **Engine oil & filter** | **Specs its job uses** lists the three, unticked. Tick Engine oil, Oil filter, and Drain plug torque → **Save**. |
| [x] | 4.6 | **+ Log** → **Maintenance**: the task from **Engine oil & filter** (the menu) | The task's parts: "Engine oil 5W-30 full synthetic (qt)", 5.5, and "Oil filter", FL-500S, 1. Below: "Reminder: Drain plug torque 30 ft-lb (41 N·m)". |
| [x] | 4.7 | Change the filter's part number to FL-910S and the oil's quantity to 6 → **Save draft** | "Update the specs too?" lists "Engine oil: 5.5 qt → 6 qt" and "Oil filter: FL-500S → FL-910S", both ticked. Untick the oil → **Save job**. The draft is saved. **Specs** shows the filter as FL-910S, the oil still 5.5 qt. |
| [x] | 4.8 | Edit the draft again, change nothing → **Save draft** | Asked again, now only about the oil (6 qt against the spec's 5.5); the filter matches its spec. Tick nothing → **Save job**. Remove the draft afterwards. |
| [x] | 4.9 | **Phone**, Sam (Driver): `Test truck` → **Maintenance** → **Specs** | The same specs, no **+ Spec**, and tapping one opens nothing. |
| [x] | 4.10 | **Mac**, Maintainer: **Set up** → **+ Requirement** Tire rotation (every 6,000 mi) and **+ Bundle** "Annual service" with both requirements | The **Schedule** shows the bundle card with **Log this service**. |
| [x] | 4.11 | **Log this service** | A new maintenance job, titled "Maintenance · Test truck", with two tasks: Engine oil & filter (parts pre-filled, ↺ Reset Oil life ticked, the reminder) and Tire rotation. |
| [x] | 4.12 | Odometer **K** → **Finish…** → **Finish and lock** | No spec question (the parts match their specs). One event with both tasks. On the **Schedule**, both requirements say last done today, each with its own next due (5,000 mi and 6,000 mi on). |
| [x] | 4.13 | **Set up** → **Save as template…** "Pickup basics" → **Save** | The **Templates** screen opens (also in the sidebar under Manage, with a wrench): "Pickup basics · 2 requirements · 1 bundle · 3 specs". |
| [x] | 4.14 | **Admin mode** → add a vehicle `Test van` (a truck, its profile's odometer). Back in Maintainer mode: **Templates** → **Apply to…** → Test van | "Copied 2 requirements, 1 bundle, and 3 specs onto Test van." The van's Maintenance shows them; its Engine oil & filter ticks the van's own specs; **Apply to…** no longer offers the van. |
| [x] | 4.15 | `Test truck` → **Specs** → change Drain plug torque to 35 ft-lb | The van's Drain plug torque is still 30 ft-lb: a copy, never linked. |
| [x] | 4.16 | **Templates** in Maintainer mode, then in **Admin mode** | Maintainer: no Rename or Remove. Admin: **Rename** to "Truck basics" works; **Remove** asks "Remove the template Truck basics? Vehicles it was applied to keep their copy." and removes it. The van keeps its schedule. |
| [x] | 4.17 | A new vehicle with no schedule → **Set up** | **Copy schedule from…** and **From a template…** (to Templates); no **Save as template…** until it has a requirement. Copy from `Test truck`: the specs come too. |
| [x] | 4.18 | **Desktop**, Sync now: `Test truck` → Specs; **Templates**; **Log this service** | The same specs and templates; a service job pre-fills as on the web. **Tailscale off:** Specs and Templates still show (from the copy); **+ Spec**, **Apply to…**, and Save say the server can't be reached; a service job can still be saved as a draft offline. |

**Milestone 4 also checks:** units and conversions, amounts' checks, what a job offers back, templates leaving a vehicle's past behind, copying with specs re-linked, who may change specs and templates, and migration 0025 on the release fixtures, all in the automated tests.

## Milestone 5: fuel economy

**Intended behavior:**
- **A stretch** runs from one full fill-up to the next. Partial fill-ups add their fuel to the next full one and have no figure of their own. A stretch with a **Missed previous fill-up** in it (or a missing reading) counts in nothing, and the chart shows a gap.
- **Averages** (Last 5, 12 months, All time) are total distance over total fuel, in your economy unit.
- **Breakdowns** by highway, towing, and hauling (three bands each) and by towed vehicle.
- Each fuel-up in the Fuel tab's list says how it counted. **ⓘ** beside **Filled full** and **Missed previous fill-up** explains them.
- The Overview's Fuel panel shows the latest figure and the last-5 average. It all works the same on the desktop, offline included.

**Before you start:** the household's units are miles, US gallons, and mpg (Settings → You). The numbers below assume them.

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 5.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open`, `schema_now` at least 25 (no new migration this milestone); `database integrity check passed`; no `ERROR` lines. |
| [x] | 5.2 | **Mac browser**, Admin mode: add a car, `Fuel car` (its profile's odometer), and give Sam the Driver role on it if they don't have it. Open its **Fuel** tab | "Fuel economy shows once there are two full fill-ups." and no fuel-ups yet. |
| [x] | 5.3 | **Phone**, Sam: **+ Log** → **Fuel-up** for `Fuel car`: odometer 10,000, 12 gal. Tap the **ⓘ** beside **Filled full**, then tap it again | The explanation opens under the row ("Filled full: the tank was filled until the pump stopped …") and closes again; **Filled full** stays ticked. Save it. |
| [x] | 5.4 | Fuel-up: 10,150 mi, 5 gal, **Filled full** unticked → Save | Saved. |
| [x] | 5.5 | Fuel-up: 10,400 mi, 7.5 gal, full, and the car's own figure 33.0 mpg (under **More**) → Save | Saved. |
| [x] | 5.6 | **Fuel** tab | Tiles: **Last 5** 32.0 mpg, **12 months** 32.0 mpg, **All time** 32.0 mpg, each "1 stretch". The chart: one dot labelled 32.0, and the average line. The list: the 10,400 fuel-up "32.0 mpg over 400.0 mi · car said 33.0"; the 10,150 one "Partial: added into the next full fill-up's figure"; the 10,000 one "Economy starts here". |
| [x] | 5.7 | Fuel-up: 10,700 mi, 10 gal, full, towing 60 %, towed **Test trailer** (or any towable vehicle), highway 80 % → Save | The Fuel tab: two dots joined by a line (32.0, 30.0); All time 31.1 mpg (700 mi on 22.5 gal), "2 stretches". |
| [x] | 5.8 | Fuel-up: 11,000 mi, 8 gal, full, and **Missed previous fill-up** ticked (its **ⓘ** explains it) → Save | That fuel-up's line: "Not counted: an earlier fill-up wasn't logged". The chart shows a shaded gap after 30.0. The tiles don't change. |
| [x] | 5.9 | Fuel-up: 11,330 mi, 10 gal, full → Save | A new dot at 33.0 after the gap, not joined across it. All time **31.7 mpg**, "3 stretches" (1,030 mi on 32.5 gal); Last 5 the same. |
| [x] | 5.10 | The breakdowns under the chart | **Towing:** None 32.4 mpg · 2 stretches; Some "not enough yet"; Mostly 30.0 mpg · 1 stretch. **Towed vehicles:** Not towing 32.4 mpg; Test trailer 30.0 mpg. Bars start from zero. **Highway** shows a figure for each band with stretches in it. **Hauling:** None with a figure; Some and Mostly "not enough yet". |
| [x] | 5.11 | Tap the 30.0 dot; then **Show as a table**; then **Show as a chart** | Under the chart: the date and "30.0 mpg over 300.0 mi". The table lists all four stretches, newest first, the dropped one saying why. Back to the chart. |
| [x] | 5.12 | The car's **Overview** | The Fuel panel: "33.0 mpg latest · last 5: 31.7" above the latest fuel-ups. |
| [x] | 5.13 | **Settings** → **You** → fuel economy **L/100 km**; back to the Fuel tab | Every figure in L/100 km (33.0 mpg ≈ 7.1 L/100 km), and the chart's scale with it (higher is worse). Set it back afterwards. |
| [x] | 5.14 | Edit the 11,330 fuel-up (still a draft): 11 gal → Save | Its stretch is 30.0 mpg; All time 30.7 mpg (1,030 mi on 33.5 gal). |
| [x] | 5.15 | **Desktop**, Sync now: `Fuel car` → **Fuel**, then **Tailscale off** (wait for **OFFLINE**): a fuel-up at 11,630 mi, 10 gal, full | Online, the same figures as the web. Offline, the new fuel-up saves on the desktop and a new dot appears at 30.0 (worked out from the copy). Tailscale on, Sync now: the web agrees. |
| [x] | 5.16 | `Test trailer` (no fuel-ups) | No Fuel tab, and no Fuel panel on its Overview, as before. |

**Milestone 5 also checks:** stretches, partials, each way a stretch is dropped, the averages' spans, the bands' edges, towed vehicles, drafts counting and corrections standing in for what they correct, and a property test that a dropped stretch never moves an average, all in the automated tests.

## Milestone 6: Ready to use?

**Intended behavior:**
- **Usages:** every vehicle has a default usage, named for its kind: **Drivable** (car, truck, motorcycle), **Towable** (trailer), or **Usable** (equipment). Each usage lists the document types it needs; a road vehicle's default needs Registration and Insurance. Maintainers add more (Maintenance → Set up → **Usages**), rename them, and remove any but the default.
- **Needed for:** each requirement ticks the usages it's needed for. One never changed counts for the default; unticking everything leaves it needed for none.
- **Each usage stands alone:**
  - A requirement needed for it is ✗ when overdue, ! when due soon or very soon, and ✓ otherwise.
  - A needed document is ✗ when expired **or not on file**, ! when due soon, and ✓ otherwise.
  - Every open or deferred **High** issue is ✗ for every usage.
  - The answer is the worst of these.
- **Ready to use?** on the Overview: a chip per usage (when there's more than one), the headline, every item with its mark, name, reason, and a link, and "Advice only: nothing stops anyone from using it." The **Garage** card shows the default usage's answer in words. Colour is never the only sign.
- Copy schedule and templates bring the usages the requirements chose, matched by name. It all reads the same on the desktop, offline included.

**Before you start:** use a new vehicle, so earlier steps' documents and issues don't get in the way. The household's due warnings are the built-in ones (60 / 30 / 14 days before a date).

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 6.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 26 (`0026_usages`); `default usages made` with `made` equal to the number of vehicles (on the first start after updating only); `database integrity check passed`; no `ERROR` lines. |
| [x] | 6.2 | **Mac browser**, `admin`: the **Garage** | Every active vehicle's card has a readiness chip beside any due chip: `Test truck` "✗ Not drivable" or "! Drivable" or "✓ Drivable", `Test trailer` a Towable one, any equipment a Usable one. Archived vehicles have none. Each chip has its symbol and words. |
| [x] | 6.3 | **Admin mode** → add a truck, `Ready truck` (its profile's odometer). Open its **Overview** | Under the squawks, **READY TO USE?**: no usage chips (only one usage); "✗ Not drivable: 2 things" in red; Registration "Not on file" and Insurance "Not on file", each with ✗; "Advice only: nothing stops anyone from using it." Its Garage card says "✗ Not drivable". |
| [x] | 6.4 | Tap **Insurance** in the card | The vehicle's **Documents** tab opens. |
| [x] | 6.5 | **Documents** → **+ Document**: Registration expiring in a year → **Save**; **+ Document**: Insurance expiring in 20 days → **Save**. Back to the **Overview** | "! Drivable: 1 thing due soon" in orange: Insurance (!) "by …" first, then Registration (✓) "by …". The Garage card says "! Drivable". |
| [x] | 6.6 | **Maintainer mode**: **Maintenance** → **Set up**, below the bundles | **USAGES** with **+ Usage**: "Drivable (default)", "Needs: Registration, Insurance", and **Edit**. |
| [x] | 6.7 | **Edit** on Drivable | The dialog has the name, **Documents it needs** (Title, Registration, Insurance; the last two ticked), **Save** and **Cancel**, and **no Remove**. Cancel. |
| [x] | 6.8 | **+ Usage**: Towing, tick only Registration → **Save**. Then **+ Usage** "towing" → **Save** | Towing is listed after Drivable: "Needs: Registration". The second is refused: "This vehicle already has a usage with that name." Cancel. |
| [x] | 6.9 | The **Overview** | Two chips, "! Drivable" (chosen) and "✓ Towing". Tap **Towing**: "✓ Towing", with Registration (✓) its only item. Tap **Drivable** to go back. The Garage card still shows Drivable. |
| [x] | 6.10 | **Set up** → **+ Requirement** | **Needed for** lists Drivable (ticked) and Towing (unticked), with "Overdue: not ready for the ticked usages. Due soon: a warning." |
| [x] | 6.11 | Name it Hitch inspection; **+ Calendar rule** every 1 month; **Last done before these records** two months ago. Under **Needed for**, tick **Towing** and untick **Drivable** → **Save** | Saved. Edit it again: Towing ticked, Drivable not. Cancel. |
| [x] | 6.12 | The **Overview** | "! Drivable" (unchanged: the hitch isn't needed for it) and "✗ Towing". On Towing: "✗ Not towing: 1 thing", Hitch inspection (✗) "was due …" first, then Registration (✓). Tapping Hitch inspection opens **Maintenance**. |
| [x] | 6.13 | **+ Requirement** Wiper blades, every 12 months, leaving **Needed for** alone (Drivable ticked), last done today → **Save**. The Overview on Drivable | Wiper blades is listed with ✓ under Drivable; it isn't on Towing's list. |
| [x] | 6.14 | **Usages** → **Edit** Drivable → rename it "Street legal" → **Save**; check the Overview and the Garage; then rename it back to Drivable | "Street legal (default)"; the chip "! Street legal" on the Overview and the Garage. Back to Drivable afterwards. |
| [x] | 6.15 | **Phone**, Sam (Driver, give them the role on `Ready truck` if needed): its **Overview**, then **+ Squawk**: "Brakes grinding", **High** → **Save**; the Overview again | Before: the same card as the Mac. After: both usages ✗. Drivable: "✗ Not drivable: 1 thing" (Insurance stays !), with "Brakes grinding" (✗) "High issue, open"; Towing: "✗ Not towing: 2 things", the issue and the hitch. Tapping it opens the squawk. The Garage card says "✗ Not drivable". Sam has no **Set up** tab. |
| [x] | 6.16 | **Mac**, **Admin mode**: add a truck, `Ready van`. **Maintainer mode**: its **Set up** → **Copy schedule from…** `Ready truck` | The van's **Usages**: "Drivable (default)" and "Towing" (made on the van, "Needs: Registration"). Its Hitch inspection is needed for its own Towing; its Wiper blades for Drivable. |
| [x] | 6.17 | `Ready truck` → **Set up** → **Save as template…** "Ready basics"; add a truck `Ready pickup`; **Templates** → **Apply to…** Ready pickup | The pickup gets Towing too, and the requirements tick it as on the truck. |
| [x] | 6.18 | `Ready truck` → **Usages** → **Edit** Towing → **Remove**. Then edit Hitch inspection | Towing is gone; the Overview has no chips again. Hitch inspection's **Needed for** has nothing ticked (needed for none, not the default), and it's not on Drivable's list. |
| [x] | 6.19 | **Desktop**, Sync now: the Garage and `Ready truck`'s Overview | The same chips, headline, and items as on the web. |
| [x] | 6.20 | **Desktop, Tailscale off** (wait for **OFFLINE**): the Overview; then **Set up** → **+ Usage** → Save | Ready to use? still shows, worked out from the copy. Saving a usage says the server can't be reached. Tailscale on. |
| [x] | 6.21 | **Phone**, Sam: the squawk from 6.15 → change it to **Low** → **Save**. The Overview | The issue is gone from every usage; "! Drivable: 1 thing due soon" again. |

**Milestone 6 also checks:** each item's mark, the worst-first order, a usage needing nothing and needed for nothing, deferred High issues, never a second default, unknown usages refused, removing a usage leaving requirements needed for none, matching usages by name when copying, sign-offs and deferrals needing a Maintainer, and migration 0026 on the release fixtures, all in the automated tests.

## Milestone 7: calculated fields and derived meters

**Intended behavior:**
- **Keys:** every field and meter has a stable key for formulas, shown in Setup. Older meters get theirs at the server's first start after updating.
- **A formula** uses values by key, numbers with units (`33 mi/h`), `+ − × ÷`, brackets, and functions: `mean`, `median`, `geometric_mean`, `harmonic_mean`, `min`, `max`, `sum`, `count`, `stddev`, `round`, `abs`. Functions work over listed values or over past events with an **Over** (`last 5`, `last 3 months`, `last 1000 mi`, `cumulative`). Units are checked: `trip-time + volume` is refused, with the reason.
- **The formula box** has **Functions ▾** and **Values ▾** menus that insert at the cursor, an **Over** picker, a check as you type, and **ⓘ How to write one** (a short popup and **Full guide →**).
- **A calculated field** (**+ Calculated field**) shows, worked out, on its events: on the event's page and on the log form as values are typed. It's never typed or required. It can use since the last event of its kind (`distance-since-last(odometer)`, `time-since-last`).
- **A derived meter** (**+ Derived meter**) is worked out from typed meters that aren't reset, at every reading of one of them. It shows on the Overview, and what's due counts on it like any meter. Nobody types its readings.
- **The ƒ badge** marks every worked-out value. A field or meter a formula uses can't be removed or turned off.
- It all works the same on the desktop, offline included.

**Before you start:** the household's units are miles, US gallons, and US dollars (Settings → You).

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 7.1 | **NAS:** after updating, `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `database open` with `schema_now` at least 27 (`0027_meter_keys`); `meter keys given` with `made` equal to the number of meters (first start after updating only); `database integrity check passed`; no `ERROR` lines. |
| [x] | 7.2 | **Mac browser**, Maintainer mode: `Test truck` → **Setup** | Under **Meters**: **+ Meter** and **+ Derived meter**; Odometer shows its key `odometer`. Under **Custom fields**: **+ Field** and **+ Calculated field**; Trip time shows `trip-time`. |
| [x] | 7.3 | **Admin mode** → add a truck, `Formula truck`. **Maintainer mode**: its **Setup** → **+ Meter**: Engine hours, Duration, Hours, 1 decimal → **Save** | The row shows the key `engine-hours`. |
| [x] | 7.4 | **+ Derived meter** → **ⓘ How to write one**; then **Full guide →**; then **✕** | "Writing a formula": four short lines (values, operators and units, Over, spaces around a minus) and three examples. **Full guide →** opens `docs/help/formulas.md` on GitHub in a new tab. **✕** closes the popup. |
| [x] | 7.5 | Name Service miles, Measures **Distance**, Unit **Miles**. Formula: `round(odometer, 0)` | Under the box, in red: "A derived meter uses only meters, numbers, + − × ÷, and functions of listed values: no round, past events, or since the last." |
| [x] | 7.6 | Clear the box. **Values ▾**: tap `odometer`; type `+ `; **Values ▾**: tap `engine-hours`; type `× 33 mi/h` → **Save** | **Values ▾** lists Engine hours and Odometer under Meters (not Service miles), then Numbers with units. Each tap puts the key where the cursor was. "✓ Gives a distance". Saved, the row shows the ƒ badge, `odometer + engine-hours × 33 mi/h`, and `service-miles`. |
| [x] | 7.7 | **+ Log** → **Reading**: Odometer 10,000, Engine hours 0 → Save. Then the **Overview** | The form has no Service miles field. The Meters panel: Service miles, with the ƒ badge, at 10,000 mi, "Worked out from Engine hours, Odometer". |
| [x] | 7.8 | A reading: Odometer 10,300, Engine hours 10 → Save | Service miles 10,630 mi (10,300 + 10 h × 33). |
| [x] | 7.9 | **Maintenance** → **Set up** → **+ Requirement** Service, **+ Meter rule** every 1,000 on **Service miles** → **Save**; the Schedule. Then a reading: Odometer 10,500, Engine hours 15 | First **OK**, "in 370 mi" (from 10,000 at the first reading). Then **Due very soon**, "in 5 mi" (10,995 mi). The Garage card shows the chip. |
| [x] | 7.10 | **Setup** → Engine hours → **Remove**. Then **Edit** Engine hours → can be reset → **Save** | Both refused: "Service miles uses this in its formula, so it can't be removed, turned off, or taken off that kind of event. Change Service miles first." |
| [x] | 7.11 | **+ Calculated field**: Label Cost per mile, **Shows as** Money per distance; formula `trip-time + volume` | "trip-time + volume: can't add a time to a volume." |
| [x] | 7.12 | Change it to `round(cost ÷ distance-since-last(odometer), 2)` (**Values ▾** lists `cost`, `volume`, and the others under This event, and `distance-since-last(odometer)` under Since the last one); Shown on **Fuel-up** → **Save** | "✓ Gives money per distance"; no preview yet (no fuel-ups). Saved, with the ƒ badge and its formula. |
| [x] | 7.13 | **Phone**, Sam: **+ Log** → **Fuel-up** for `Formula truck`. Before typing anything, look under the fields; then odometer 10,600, 10 gal, $35.00, Trip time 1:00 | **WORKED OUT**: "ƒ Cost per mile —" and "Shows once the form is filled in."; once filled in, "No value: there's no earlier one to count from." Save. |
| [x] | 7.14 | A fuel-up: odometer 11,000, 12 gal, Trip time 2:00, and type the price $40.00 last | As the price is typed, Cost per mile changes, ending at **$0.10 per mi** ($40 over 400 mi). Save. Its page: **WORKED OUT**, ƒ Cost per mile $0.10 per mi. |
| [x] | 7.15 | **Mac**: **+ Calculated field** Average trip time, **Shows as** Time; type `mean(trip-time` then the **Over** picker's **Cumulative**, then `)` | The box reads `mean(trip-time, cumulative)`; "✓ Gives a time"; "On the latest fuel-up: 1:30". Save. The latest fuel-up's page shows 1:30, the first 1:00. |
| [x] | 7.16 | Edit Average trip time: `cumulative` → `last 1` → **Save** | The latest fuel-up shows 2:00. |
| [x] | 7.17 | **Setup** → Trip time → **Remove**; then **Edit** Trip time → turn it off → **Save** | Both refused: "Average trip time uses this in its formula …". |
| [x] | 7.18 | **Settings** → **You** → distance **Kilometers**, volume **Liters**; back to the second fuel-up's page. Set them back afterwards | Cost per mile shows per km, about **$0.06 per km**. |
| [x] | 7.19 | **Desktop**, Sync now: `Formula truck` → Setup, the Overview, the fuel-ups' pages | The same keys, formulas, ƒ values, and Service miles. **Tailscale off** (wait for **OFFLINE**): a fuel-up at 11,300 mi, 8 gal, $30.00 shows Cost per mile live ($0.10 per mi) and saves; its page shows it; **+ Calculated field** → Save says the server can't be reached. Tailscale on, Sync now: the web agrees. |

**Milestone 7 also checks:** the formula language (precedence, units, every function, each Over, every refusal and "no value"), keys never clashing or reserved, derived readings at every input's reading, a job counting a derived meter, values since the last event, the server's checks and "in use" refusals, older meters' keys, and migration 0027 on the release fixtures, all in the automated tests.

## Earlier releases and the exit criteria

**Intended behavior:** what `v0.2.0` and `v0.3.0` brought still works on `v0.5.0`: signing in, everyday logging with photos, repairs and corrections, recalls, documents, send-outs, and the desktop's offline copy and sync. The desktop opens the encrypted copy it already has, though its SQLCipher is newer (4.20, from 4.10; issue #41). And each of Phase 5's exit criteria passes, by the steps in the table below.

**When:** after the milestones' sections, on the same image and desktop build (once the release PR has merged into `dev`, so the version reads `0.5.0`).

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | E.1 | **Mac browser**, Diagnostics | **About**: App `0.5.0`, and the place data's version. **Run checks** passes. |
| [x] | E.2 | Diagnostics → About → **Licenses** → **Third-party notices** | The notices, scrolling: Lucide, pdf.js, SQLCipher ("Copyright (c) 2025, ZETETIC LLC"), OpenSSL, and the Rust crates (`sqlx 0.9.1`, `libsqlite3-sys 0.40.0` among them). **Close** closes it. |
| [x] | E.3 | Sign out, then sign in as `admin` with a wrong password, then the right one | "Wrong username or password."; then signed in, in **Driver** mode. |
| [x] | E.4 | **Phone**, Sam: **+** → **Fuel-up** for `Test truck` (odometer, fuel, price, **Use my location**) → **Save**; then **+ Add** → **Take photo** → **Add 1** | Saved, with the town filled in; the photo opens full screen. |
| [x] | E.5 | **Phone**, Sam: **+** → **Issue** on `Test truck`: `Regression rattle`, Low | Saved; on **Issues** as Open. |
| [x] | E.6 | **Mac browser**, Maintainer mode: `Test truck` → **+ Log** → **Repair**: a task, the odometer; **Sign off** `Regression rattle`; **Finish…** → **Finish and lock**. Then its page → **Correct…** → change the labor, a reason → **Save** | The issue is **Resolved**. The correction is a **Draft** marked **Correction**; the Timeline shows the original **Corrected**. |
| [x] | E.7 | **Maintenance** → **Set up** → **+ Recall or bulletin** → **+ A new one** → **Save**; **+ Add** → a PDF | The PDF opens in the viewer (pages, zoom). |
| [x] | E.8 | **Documents**: **+ Document** → Registration, with an expiry → **Save**; then **Renew…** | The new one **Current**, the old **Superseded**. |
| [x] | E.9 | **+ Log** → **Sent out** (a shop, due back tomorrow) → **Save**; then **Record return** | **Out for service** and its banner; after the return, no banner. |
| [x] | E.10 | **Desktop** (the copy from before this release, not a new sign-in): unlock with the PIN; Diagnostics | It opens with no new download: **About**: App `0.5.0`, Server `0.5.0`. **Offline copy**: Up to date. **Snapshots of this copy**: today's. (The copy SQLCipher 4.10 made opens in 4.20.) |
| [x] | E.11 | **Tailscale off** (wait for **OFFLINE**): **+ Log** → **Fuel-up** for `Test truck` → **Save**, then **+ Add** a photo. **Tailscale on**, Sync now; **phone** (reload) | **↑ 2 to send** while offline; after syncing, the fuel-up and its photo are on the phone. |
| [x] | E.12 | **Tailscale off**; **desktop:** a **reading** on `Test truck` 10 mi above its latest. **Phone**, Sam: a reading 50 mi above it. **Tailscale on**, Sync now | The desktop's reading is refused and kept in **Messages** ("Not saved: …"), to fix or discard. |
| [x] | E.13 | **Expired insurance** (exit criterion): **Mac**, Maintainer mode: `Ready truck` → **Documents** → edit the Insurance from step 6.5: expiry **yesterday** → **Save**; its **Overview** | "✗ Not drivable", with Insurance ✗ and why (expired). Set its expiry back to 20 days on afterwards: Insurance is ! again. |
| [x] | E.14 | **A deferred High** (exit criterion): **Phone**, Sam: **+ Squawk** on `Ready truck`, "Deferred check", **High**. **Mac**, Maintainer mode: **+ Log** → **Repair** on `Ready truck`, a task; under its issues, **Defer** "Deferred check" with a **Why** → **Finish…** → **Finish and lock**; the **Overview** | The issue is **Deferred**, and the vehicle is still "✗ Not drivable", with "Deferred check" (✗) "High issue, deferred". Afterwards, lower it to Low. |
| [x] | E.15 | **The next morning:** `ls -l data/snapshots/` and `sudo docker logs --since 24h vehiclemanager-test 2>&1 \| grep -E "nightly-snapshot\|file-check\|ERROR"` | A new snapshot from last night; both jobs ran; no `ERROR` lines you can't explain. |

**A fix from this run** (release step R.d2, the first Human Note): run once its image is on the test server.

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | F.1 | **Mac browser**, Maintainer mode: add a vehicle with no schedule (or use one). **Templates** → a template's **Apply to…** → choose that vehicle | Nothing is copied yet: no "Copied …" line, and the vehicle's **Set up** still has no requirements. **Apply** is now on (it was off until a vehicle was chosen). |
| [x] | F.2 | **Apply** | "Copied … onto …", and the vehicle has the template's requirements. The menu goes back to **Apply to…**, and that vehicle is no longer offered. |

**The exit criteria** (`docs/phase_plans/phase-5.md`), and the steps that show each:

| Exit criterion | Steps |
|---|---|
| **Oil life:** a counting-down percentage meter; the oil change due by distance, time, or Oil life 15%, whichever first; yellow, orange, red as readings come in; **Reset Oil life to 100%** moves it on | 1.3–1.9; 2.4–2.8 (the plan's steps use 5,000 mi, so the numbers come sooner); 2.12–2.13 |
| **What's due:** the Maintenance tab and the Overview show time and distance, with projected dates; the Garage marks overdue vehicles, with an **Overdue** chip | 2.5–2.8; 3.2–3.3 |
| **Messages:** due soon on the web, and on the desktop offline; "1 week or 500 mi" comes back at whichever first | 3.5–3.8, 3.10; 3.14 |
| **Specs and bundles:** an oil change pre-fills from the Specs; a service signs off several tasks in one event, each one's clock reset | 4.6–4.8; 4.10–4.12 |
| **Fuel economy:** a partial fill adds into the next full one; a missed fill-up leaves a gap; breakdowns by condition and towed vehicle | 5.4–5.6; 5.8–5.9; 5.10 |
| **Readiness:** expired insurance, not ready; a deferred High still blocks; Towing adds its own items | E.13; E.14 (and 6.15, an open High); 6.8–6.12 |
| **Calculated values:** a derived meter drives an interval rule; a calculated field on a fuel-up | 7.9; 7.12–7.14 |
| **Sync:** each new kind of change offline on the desktop, synced | 1.15–1.16, 2.16, 3.14–3.15, 4.18, 5.15, 6.20, 7.19, E.11 |
| **Sync:** an app one API version behind still syncs | The server's automated tests (an app one version behind syncs; two behind is told to update and keeps its outbox). Nothing to do by hand. |

## Results

*Run by the human on the test server with `:dev` and the desktop app bundled from `dev`, 2026-11-19.*

- **Every step passed** (milestones 1–7, Earlier releases E.1–E.15, and the exit criteria by the table above).
- **Feedback** (Human Notes below), reviewed 2026-11-19:
  - **Applying a template needs a confirmation step:** choose the vehicle, then **Apply**. To fix soon (the human). Fixed in the release (step R.d2), and checked by F.1–F.2 on the test server with `:dev` (both passed, 2026-11-20).
  - **Which documents each kind of vehicle needs, and custom vehicle types:** for the polish phase, as idea 23 in `docs/ideas-to-discuss.md`. Part of it works today: each vehicle's usages (milestone 6) choose the documents they need, so a track-only car's **Drivable** can need neither registration nor insurance.

# Human Notes

- When I apply a template to a vehicle, there should be an extra confirmation step. Process: select vehicle to apply the template to, then an "Apply" button.
- An admin should be able to specify which document kinds are required for each vehicle type. I may have a track-only car that requires no registration or insurance. Along with that, it may be useful to abstract vehicle types so a user (admin) can create a custom vehicle type and specify what rules in the app apply to which types.
