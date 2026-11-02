# Manual test plan: place lookup, saved places, and the location picker

*Phase 2, milestone 5 · `docs/phase_plans/phase-2.md` · decisions #038, #059, #060, #061*

## 1. Intended behavior

- **Place lookup (#038, #059):**
  - The server turns coordinates into a **city, state or province, and country**, plus the city's **time zone**, offline. It uses place data built into the image: GeoNames `cities500` and Natural Earth 1:10m boundaries, data version **2026-11-02**.
  - A point just outside the map's coastline gets the **nearest** country and state, with a note to check it. Natural Earth's coastline is coarse, so a beach, a pier, or a wide river or bay can count as "outside".
  - The city comes from the same country as the point, so a point just inside a border isn't named after a town across it.
  - **The suggestion only pre-fills the fields.** Any city or state can be typed, for example a base or community GeoNames doesn't list. Only the country must be a real one.
  - **If the place data can't be opened,** the server still runs:
    - lookups give no suggestion;
    - saved places and typed places still work;
    - without the country list, the country is a two-letter code;
    - the Admin sees **Place lookup isn't available** in Messages (in Admin mode).
- **Saved places (#059, #060):**
  - Each person has their own. Only they see them, no role is needed, and any mode works.
  - A saved place is an alias: a name, coordinates, a radius (25 m to 2 km, 150 m by default), and its city, state, and country.
  - Inside a saved place's radius, the picker says **You're at …** and uses that place's own city, state, and country. Milestone 6's events will copy them, so editing or removing a place never changes past events.
- **The place dialog** is the location picker milestone 6's events reuse:
  1. **Where:** **Use my location** (the browser asks permission once; needs HTTPS, which `tailscale serve` gives) or **Type coordinates** (typed or pasted, e.g. `35.5951, -82.5515`). It shows the accuracy and age, and warns when a location is over 20 minutes old.
  2. **Place:** the suggestion fills the fields, which stay editable. If you've typed and then pick new coordinates, your values stay until **Use the suggestion**.
  3. **Name and radius.**
- **About** (Diagnostics): the app version, the place data version (or "Not available"), and the GeoNames and Natural Earth credits.

## 2. Setup

- **The milestone image:** after the branch is pushed, the "Dev image" workflow builds `milestone-p2-m5-location`. Check that Actions shows it finished successfully. It's about 60 MB bigger than before: that's the place data.
- **On the NAS over SSH:**

  ```sh
  cd /volume1/docker/vehicle_manager
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m5-location`, then press `Esc` and type `:wq`. Step 18 also needs this milestone's `compose.yaml`, which passes the new `VEHICLEMANAGER_GEODATA_FILE` setting into the container:

  ```sh
  curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-m5-location/deploy/compose.yaml
  ```
- **People:**
  - Your Admin account (`admin`).
  - Sam, any role.
- **Devices:**
  - A phone, with its browser at the NAS address (HTTPS, through Tailscale). Location Services must be on for the browser.
  - The browser on the Mac.
  - The desktop app on the Mac.
- **About the data:** saved places are kept. Add the ones you mean to keep; the test ones (Pier, Base gate) are removed in step 13.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager 2>&1` | <ul><li>`backed up before migrating` with `schema_before=7`.</li><li>`database open` with `created=false`, `schema_before=7 schema_now=8`, and the **same database ID** as before.</li><li>`place data opened` with `path=/app/geodata/geodata-2026-11-02.gpkg`, `data_version=2026-11-02`, and `size_mb=57`.</li><li>`database integrity check passed`.</li><li>No `ERROR` lines.</li></ul> |
| [x] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v7.sqlite`, owned by `1030 100`. |
| [x] | 3 | Sign in as `admin` in the Mac browser (Driver mode is fine). Open **Diagnostics** | **ABOUT**: App `0.1.0`, Place data `2026-11-02`, and the GeoNames and Natural Earth credits. |
| [x] | 4 | Open **Places** (under Manage, with a map pin) | "No places yet…" and **+ Add place**. |
| [x] | 5 | **Mac:** **+ Add place** → **Save** without a location | "Set where it is first: use your location, or type coordinates." |
| [x] | 6 | **Type coordinates** → `35.5951, -82.5515` → **Use these** | <ul><li>The error clears.</li><li>"35.59510, -82.55150" and "Typed coordinates".</li><li>A green **Suggested: Asheville, North Carolina, US**, with "Time zone America/New_York · from place data 2026-11-02".</li><li>City, state, and country (**United States of America**) filled in.</li></ul>Name it `Test Asheville`, **Save**: it's in the list with "150 m". |
| [x] | 7 | **Phone**, at home: sign in as `admin` → **Places** → **+ Add place** → **Use my location** | <ul><li>The browser asks for location permission once; allow it.</li><li>Your coordinates with "± N m · just now".</li><li>The suggestion names your town, state, and country, and fills the fields.</li></ul>Name it `Home garage`, pick a radius, **Save**. |
| [x] | 8 | **Phone**, still at home: **+ Add place** → **Use my location** | "You're at Home garage (N m away). Its city, state, and country are used." **Cancel**. |
| [x] | 9 | **Mac:** **+ Add place** → **Type coordinates** `34.2133, -77.7850` (just off the Wrightsville Beach pier) | "Outside the map's coastline (at sea, or on a wide river or bay): the nearest place is Wrightsville Beach, North Carolina, US. Check it." Fields filled with Wrightsville Beach. Name it `Pier`, **Save**. |
| [x] | 10 | **A typed city:** **+ Add place** → **Type coordinates** `34.6600, -77.3500` → change **City or town** to `Camp Lejeune` → name it `Base gate` → **Save** | The list shows "Base gate · Camp Lejeune, North Carolina, US". |
| [x] | 11 | **Other places** (then **Cancel** each): type `42.3149, -83.0364`; then `48.8566, 2.3522` | <ul><li>Windsor, Ontario, CA (Canada), not Detroit across the river.</li><li>A Paris district ("Paris 04 Hôtel-de-Ville": GeoNames lists big cities' districts as places), region Paris, FR (France), time zone `Europe/Paris`.</li></ul> |
| [x] | 12 | **Edit** `Test Asheville` → **Type coordinates** `36.2168, -81.6746` | <ul><li>The fields still say Asheville (your saved values), with **Use the suggestion**.</li><li>**Use the suggestion** changes them to Boone.</li><li>Set the radius to **250 m**, **Save**: the list shows Boone and 250 m.</li></ul> |
| [x] | 13 | **Remove** `Pier`, then `Base gate`, then `Test Asheville` | Each asks "Remove …? Past events keep their own copy of it." and then it's gone. |
| [x] | 14 | **Sam** (phone or Mac): **Places** | "No places yet": Sam never sees yours. Sam can add their own. |
| [x] | 15 | **Phone layout:** the Places list | One column; tapping a place opens it to edit (no Edit and Remove buttons on the row). The dialog scrolls. |
| [x] | 16 | **Desktop app:** **Places** → **+ Add place** → **Use my location**, then **Type coordinates** `35.5951, -82.5515` | Your places are listed. **Use my location** may say "The location isn't available here. Type coordinates instead." (the desktop app's webview may not offer location). Typed coordinates work as in step 6. **Cancel**. |
| [x] | 17 | *(Optional)* **Location refused:** in the phone browser's settings, deny location for the site → **+ Add place** → **Use my location** | "Location permission was refused. Allow it in the browser's settings, or type coordinates." Allow it again afterwards. |
| [x] | 18 | **Missing place data:** in `.env` set `VEHICLEMANAGER_GEODATA_FILE=/data/none.gpkg`, run `sudo docker compose up -d`, then `sudo docker logs --since 2m vehiclemanager 2>&1 \| grep -i "place data"` | `can't open the place data; place lookup is unavailable` with the path. The server is otherwise normal. |
| [x] | 19 | With the data missing: Mac, Admin mode → **Messages** | A warning: **Place lookup isn't available**, with the error under DETAILS. In Driver mode, and for Sam, it isn't shown. |
| [x] | 20 | Still missing: **Places** → **+ Add place** → **Type coordinates** `35.5951, -82.5515` | "Place lookup isn't available: type the city, state, and country." A **Country code (e.g. US)** box instead of the list. Typing a city, state, and `us` saves the place (shown as US). Then remove it. **Diagnostics → ABOUT**: Place data **Not available**. |
| [x] | 21 | **Fix it:** empty `VEHICLEMANAGER_GEODATA_FILE=` in `.env`, `sudo docker compose up -d` | `place data opened` again. The Messages warning disappears by itself (#050). |
| [x] | 22 | `sudo docker compose restart`, then reload the browser | `Home garage` is still there, with its place and radius. |
| [x] | 23 | **Secrets check:** `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` | `0`. |

**Not covered by hand:**
- Events using the picker: milestone 6 adds them.
- A location over 20 minutes old: the browser rarely gives one. The stale warning is tested in code (`a_fix_goes_stale_after_twenty_minutes`).
- The R-tree's role in each lookup: covered by the geocoder's tests (`the_r_tree_is_what_finds_candidates`).
- More real places (Point Roberts, Lake Erie, Fiji): `crates/geocoder/tests/real_data.rs` checks them against the data file.

## 4. Follow-up: nearby places and the dialog fixes (step 5f.2, decision #061)

Run after sections 1–3 passed on the earlier build. What changed:
- **Nearby places:** the picker now lists up to 5 places within 25 km in the same country, nearest first, with the distance and compass direction (in your miles or km setting) and the place's type.
- **The default:** the nearest place that isn't a district. A district is what GeoNames marks as one (e.g. New York's neighbourhoods), or a numbered part of a bigger place nearby (e.g. "Paris 04 Hôtel-de-Ville" beside Paris), so central Paris defaults to Paris.
- **The buttons:** only the chosen **Where** button is blue.
- **The data:** version `2026-11-02.2`, with each place's type and population, and abandoned and historical places left out.
- **Dialogs** scroll on phones, and the page behind them stays still.

No new migration: the schema stays at 8.

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | F1 | Push 5f.2 and wait for the "Dev image" workflow. Then `sudo docker compose pull && sudo docker compose up -d` and `sudo docker logs --tail 20 vehiclemanager 2>&1 \| grep -i "place data"` | `place data opened` with `path=/app/geodata/geodata-2026-11-02.2.gpkg`, `data_version=2026-11-02.2`, and `size_mb=59`. |
| [x] | F2 | **Diagnostics → ABOUT** | Place data `2026-11-02.2`. |
| [x] | F3 | **Places → + Add place** | Neither **Use my location** nor **Type coordinates** is blue. Choosing **Type coordinates** makes it blue; choosing **Use my location** afterwards moves the blue to it. |
| [x] | F4 | **Type coordinates** `35.5951, -82.5515` | <ul><li>"Nearby: choose the city or town", with **Asheville · 0.4 mi N · Town · county seat** first and marked.</li><li>Below it: Woodfin, Biltmore Forest, Bent Creek, Weaverville (villages), each with its distance and direction.</li><li>City or town says Asheville.</li></ul> |
| [x] | F5 | Tap **Woodfin** | It's marked, and City or town says Woodfin. Typing over the field still works. **Cancel**. |
| [x] | F6 | **+ Add place → Type coordinates** `48.8566, 2.3522` | **Paris · 0.3 mi SW · Capital** is marked, and City or town says Paris. The arrondissements ("Paris 04 Hôtel-de-Ville", "Paris 01 Louvre"…) are listed as **District**. **Cancel**. |
| [x] | F7 | **Phone:** **+ Add place → Use my location** | The button turns blue, and your nearby places are listed with the distance and direction from where you're standing. |
| [x] | F8 | **Settings → You → Distance → Kilometers**, then repeat F4 | The distances show in km, e.g. "Asheville · 0.7 km N". Put Distance back afterwards. |
| [x] | F9 | **Type coordinates** `-16.7000, 179.9500` (at sea, off Fiji) | No nearby list (nothing within 25 km). The suggestion still names the nearest place, Savusavu, with the coastline note. **Cancel**. |
| [x] | F10 | **Phone:** **+ Add place**, then swipe up and down on the dialog | The dialog scrolls to **Save** and **Cancel**, and the page behind it never moves. Two fields side by side (City and State) both fit on the screen. |

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager 2>&1`. Place data and saved places are on the `app` area.
- **App log** (browser or desktop app): Diagnostics → the log viewer, set to **Debug**. The picker logs on the `ui` area (e.g. "no device location", "places loaded").
- **For a wrong place:** the coordinates you used, what the suggestion said, and what you expected.
- **Screenshots** of the screen or dialog in question.
- **Never share** a password, `.env`, or a cookie.
