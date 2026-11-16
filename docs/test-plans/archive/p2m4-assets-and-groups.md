# Manual test plan: vehicles, groups, meters, custom fields, and roles on a group or vehicle

*Phase 2, milestone 4 · `docs/phase_plans/phase-2.md` · decisions #052, #055, #056, #057, #058*

## 1. Intended behavior

- **Groups (#052, #058):**
  - Every vehicle is in exactly one group. The server makes **Garage** when there's no group yet.
  - **Manage → Groups** (the household's Admin, in Admin mode) adds groups (name and icon), renames them, changes their icon, lists their vehicles and **who sees each group and why**, and deletes a group only when it's empty and isn't the last one.
- **The sidebar (#056):**
  - With one group, the sidebar shows just that group (e.g. **Garage**), which opens every vehicle.
  - With more, it shows **All vehicles**, then each group with its icon. A group's page shows only its vehicles and is titled with its name.
  - On phones, the first tab is **Vehicles**, with a menu at the top to pick All vehicles or a group.
- **Vehicles (#056):**
  - Cards in a section per group: the name, kind, year, make, and model, and a plate or serial number.
  - **Sort menu:** Newest activity (newest added, until vehicles have events), Name (A–Z), or Custom. Custom shows **Arrange**, which puts ▲▼ on each card. The sort and the custom order are each person's own.
  - **Show archived** adds retired and disposed vehicles, dimmed.
  - **+ Add vehicle** (Admin, in Admin mode) asks for the kind first, then the details: that kind's identifiers and the group. A new vehicle starts with its kind's meters (the odometer in your miles or km) and custom fields.
- **A vehicle's page (#057):**
  - The header has a back link to its group, the name, the kind and model and colour, its identifiers, its group, and its status. The top bar shows the vehicle's name.
  - **Tabs:** Overview, Timeline, Maintenance, Fuel (not for trailers or other equipment), Documents, and Setup. Empty tabs say which milestone fills them.
  - **Overview:** panels that say when they fill, and the meters.
  - **Setup:**
    - Details (Edit details, including the description, shown as plain text, #055).
    - Meters: add, edit, remove. What a meter measures is fixed once it exists.
    - Custom fields: add, edit, turn off, remove. Each field shows its **key**, which never changes, even when the field is renamed.
    - **Required readings:** a grid of meters by event kinds.
    - **Move to another group**, showing who gains or loses access first.
    - The **status** menu.
    - **Delete**, only with the name typed.
  - Each control shows only when your role and mode allow it:
    - edit, status, and delete: Admin on the vehicle, in Admin mode;
    - meters and fields: Maintainer, in Maintainer or Admin mode;
    - moving: the household's Admin.
  - Without any of these, Setup is view only, with a note saying so.
- **Roles (#058):**
  - On **Users**, each person's roles are a list, each with its own menu and ✕. A role can be on **all vehicles**, **a group**, or **one vehicle**. **+ Add role** adds one.
  - Roles add up: on each vehicle, the highest one applies. A person sees only the vehicles their roles cover.
  - The list of users summarises roles: "Admin · all vehicles", "Driver · Garage", or "3 roles".

## 2. Setup

- **The milestone image:** after the branch is pushed, the "Dev image" workflow builds `milestone-p2-m4-assets`. Check that Actions shows it finished successfully.
- **On the NAS over SSH:**

  ```sh
  cd /volume1/docker/vehicle_manager
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m4-assets`, then press `Esc` and type `:wq`. If test timers are still set from milestone 3, empty `VEHICLEMANAGER_TEST_MINUTES=`.
- **People:**
  - Your Admin account ("jordan" below).
  - Sam from milestone 2, with a role on all vehicles (Driver is fine). If Sam is disabled, enable them in Users.
- **Devices:**
  - The browser on the Mac.
  - A phone browser (Sam).
  - The desktop app on the Mac: `dx serve --platform desktop` from `crates/app`, with the NAS address saved.
- **The vehicles:** use your real ones. Steps 4–10 use your truck. Anywhere a step names the Truck, use whichever vehicle you like.
- **About the data:** this is the first milestone that keeps vehicles. What you add now stays through later milestones, so add them as you mean to keep them. A test vehicle is deleted in step 21.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager` | <ul><li>`backed up before migrating` with `schema_before=5`.</li><li>`database open` with `created=false`, `schema_before=5 schema_now=7`, and the **same database ID** as before.</li><li>`default group made`.</li><li>`database integrity check passed`.</li><li>No `ERROR` lines and no setup code.</li></ul> |
| [x] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v5.sqlite`, owned by `1030 100`. |
| [x] | 3 | Sign in as jordan in the browser (Driver mode) | <ul><li>The sidebar shows **All vehicles**, then Due & squawks and the rest. The empty Garage group shows only in Admin mode.</li><li>The page says "No vehicles yet."</li><li>No **+ Add vehicle** (Driver mode).</li></ul> |
| [x] | 4 | Enter Admin mode. **+ Add vehicle** → **Truck** | <ul><li>Once in Admin mode, the sidebar shows **Garage** and the page is titled Garage.</li><li>Step 2 asks for: Name, Make, Model, Year, Colour, VIN, License plate, and Group (Garage).</li><li>Try **Add** with no name: "Give the vehicle a name."</li></ul> |
| [x] | 5 | Fill in your truck and **Add**. Then add your other vehicles | <ul><li>Each appears as a card, for example "Truck · 2021 Ford F-150" with its plate.</li><li>A trailer or other equipment asks for a serial number instead.</li></ul> |
| [x] | 6 | **Sort:** choose **Name (A–Z)**, then **Custom** → **Arrange**, move a card up with ▲, then **Done**. Reload the page | <ul><li>Name sorts A–Z, ignoring case.</li><li>Arrange shows ▲▼ on each card, and the card moves.</li><li>After the reload, the sort is still Custom, in your order.</li></ul> |
| [x] | 7 | Open the truck | <ul><li>The top bar says the truck's name.</li><li>The header shows "← Garage", the name, "Truck · year make model · colour", the VIN and plate, **Garage**, and **In service**.</li><li>The tabs are Overview, Timeline, Maintenance, Fuel, Documents, and Setup.</li><li>Overview: four panels that say when they fill, and **Meters**: "Odometer · Distance · miles (or kilometers) · whole · Required for: Fuel-up".</li><li>Timeline, Maintenance, Fuel, and Documents each say which milestone fills them (6, 7, 6, 8).</li></ul> |
| [x] | 8 | **Setup → Edit details:** add a description (e.g. "Synthetic oil only."), change nothing else, **Save** | <ul><li>Details shows "Description: Synthetic oil only."</li><li>Typing `**bold**` shows the asterisks as typed (#055).</li></ul> |
| [x] | 9 | **+ Add meter:** "Engine hours", Measures **Duration** (the units become just hours), Decimal places **tenths**, **Save**. In **Required readings**, tick Engine hours under **Maintenance** | <ul><li>Meters lists "Engine hours · Duration · hours · tenths".</li><li>The tick stays after a reload.</li><li>**Edit** the meter: **Measures** is greyed out ("What a meter measures can't change once it exists.").</li></ul> |
| [x] | 10 | **+ Add field:** label "Trip EV miles", On **Fuel-up**, Type **Number**, Decimal places **1**, Lowest likely `0`, Highest likely `x`, **Save**. Then change Highest likely to `60.5`, **Save** | <ul><li>`x` is refused: "Enter a number for the range, or leave it empty."</li><li>Then the field appears with the key `trip-ev-miles`.</li><li>**Edit** it: rename it "Electric miles" and untick **Shown on forms**, then **Save**. The row says "Electric miles · **Off**", and the key is still `trip-ev-miles`.</li></ul> |
| [x] | 11 | **Status:** set it to **Retired**, then go back to Garage | <ul><li>The truck's card is hidden, and **Show archived (1)** appears.</li><li>Show archived shows it dimmed, with "Retired".</li><li>Open it and set the status back to **In service**.</li></ul> |
| [x] | 12 | **Manage → Groups** | <ul><li>Groups is listed under Users.</li><li>The list shows Garage with its vehicle count.</li><li>"Choose a group…" shows on the right.</li><li>Open **Garage**: its name, the icon picker (Garage chosen), its vehicles with **Open ›**, **WHO SEES IT** (jordan "Admin · all vehicles", Sam "Driver · all vehicles"), and **Delete group…** greyed out.</li></ul> |
| [x] | 13 | **+ Add group:** "Hangar" (or a name you'll use), pick an icon, **Add** | <ul><li>It's in the list.</li><li>The sidebar now shows **All vehicles**, Garage, and Hangar, each with its icon.</li><li>Renaming a group in the Name box saves when you leave the box ("Saved."), and the sidebar follows. Change it back.</li></ul> |
| [x] | 14 | **Roles: Users → Sam.** **+ Add role**: On **Garage** (under Groups), Role **Driver**, **Add**. Then ✕ on **All vehicles** | <ul><li>Sam's roles: "Garage · group: Driver".</li><li>The users list says "Driver · Garage".</li><li>Jordan's own Admin role on all vehicles is greyed out (the last Admin).</li></ul> |
| [x] | 15 | **Move:** open a vehicle you'll keep in the new group → Setup → **Move to another group…** → Hangar | <ul><li>**WHO'S AFFECTED**: "Sam: Driver → no access".</li><li>**Move**: the header shows Hangar and "← Hangar".</li><li>Groups → Hangar lists it; **WHO SEES IT** shows Sam "no access".</li></ul> |
| [x] | 16 | **Sam on the phone:** sign in (Driver mode) | <ul><li>Only the Garage's vehicles show, titled **Garage**. There's no group menu (Sam sees one group).</li><li>Pasting the moved vehicle's address (copied from the Mac's address bar) shows "That vehicle isn't here, or you can't see it."</li></ul> |
| [x] | 17 | Sam opens a Garage vehicle → **Setup** | <ul><li>"Changing this vehicle's setup needs Maintainer or Admin mode."</li><li>No buttons, and the readings grid shows ✓ marks instead of tick boxes.</li></ul> |
| [x] | 18 | **One vehicle:** as jordan, give Sam **Maintainer** on that one vehicle (+ Add role, under Vehicles). Sam switches to **Maintainer** mode (the mode chip) and opens it → Setup | <ul><li>Sam can add, edit, and remove meters and fields, and tick the grid.</li><li>Sam has no Edit details, Move, status menu, or Delete.</li><li>Another Garage vehicle's Setup is still view only.</li><li>jordan's Users list says "2 roles" for Sam.</li></ul> |
| [x] | 19 | Sam on the phone: the **Vehicles** tab | The vehicles show, and the sort menu works. Sam's sort is Sam's own: jordan's Custom order didn't change it. |
| [x] | 20 | **An empty group:** + Add group "Boathouse", choose it, **Delete group…** → **Delete** | It's gone from the list and the sidebar. A group with vehicles can't be deleted (the button is greyed out). |
| [x] | 21 | **Delete a vehicle:** add a test vehicle "Test", open it → Setup → **Delete…**, type "tset" | <ul><li>**Delete** stays greyed out.</li><li>Type "test": it's enabled (case doesn't matter). **Delete** goes back to All vehicles, and Test is gone.</li></ul> |
| [x] | 22 | **Desktop app:** sign in, open a vehicle (Overview and Setup), then Groups (Admin mode) | Like the browser: the header, tabs, panels, the Setup sections, and the Groups screen. |
| [x] | 23 | `sudo docker compose restart`, then reload the browser (sign in again if asked) | <ul><li>Everything is still there: vehicles, groups, meters, fields, required readings, Sam's roles, and your sort and order.</li><li>The log shows `database open` with `created=false` and **no** `default group made`.</li></ul> |
| [x] | 24 | **Secrets check:** `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` | `0`. |

**Afterwards (optional):** give Sam back whatever roles you want them to have (for example Driver on all vehicles), and remove the one-vehicle Maintainer role.

**Not covered by hand:**
- A move that **gains** someone access, such as moving a vehicle into a group where someone has a role. It works the same way as step 15; the server's automated test covers it (`a_move_previews_and_changes_who_can_see_the_vehicle`).
- Deleting the last group, which can't happen while vehicles exist. The server test covers it (`groups_with_vehicles_and_the_last_group_stay`).
- Ranges for distance, duration, volume, and fuel economy fields. Their entry arrives with milestone 6's units; for now only plain number fields take a range.

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager`. Vehicles, groups, and the default group are on the `app` area; role changes on `auth`; the database on `database`.
- **App log** (browser or desktop app): Diagnostics → the log viewer, set to **Debug**. The vehicle screens log on the `ui` area ("garage loaded" with the vehicle and group counts, "vehicle loaded").
- **Screenshots** of the screen in question, in both themes if it's a display problem. For step 15, the move dialog before pressing **Move**.
- **Never share** a password, `.env`, or a cookie.
