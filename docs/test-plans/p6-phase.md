# Test plan: Phase 6, polish

*`docs/phase_plans/phase-6.md` · decisions #081, #089, #090 · built up milestone by milestone, run at the end of the phase*

**How this plan works (#081):** each milestone adds its section here as its last step. Nothing is run until the end of Phase 6. Then the human runs the whole plan on the **test server**, before the `v0.6.0` release, as its regression pass. Its last part, **Earlier releases and the exit criteria**, is added with the release. A failure is fixed on `dev`, and its section is run again.

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
- **Devices:** the Mac (the desktop app, and a browser) and the phone's browser. **Offline** means **Tailscale off on the Mac**; the phone stays on Tailscale. Milestone 1 also turns the Mac's **Wi-Fi** off once, and needs a **second monitor** for one step.
- **The vehicle:** `Test truck`, with its odometer.

## If a step fails, capture

- **The step's number,** a screenshot, the device and browser (or the desktop app), and who was signed in, in which mode.
- **Desktop:** Diagnostics → the log viewer (Minimum level: Debug), or the log export.
- **The desktop's settings file:** `~/Library/Application Support/vehiclemanager/settings.json` (the server address and the window's place; no passwords).
- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **Never share** a password, a PIN, `.env`, a cookie, a token, or a document with personal details on it.

---

## Milestone 1: fixes and Settings

**Intended behavior:**
- **Settings** is a column of sections down the left: **You** (Units and formats; Vehicles and messages; Photos; Sign-in), **This device** (Appearance; Lock and PIN, on a desktop with a PIN; Devices), and **Household** (Defaults for everyone, in Admin mode only). Each row works as before. The last section opened is remembered on each device. On a narrow window, the column is a list; a section opens on its own, with **← Settings** above it.
- **Type to confirm:** "To confirm, type:" and then the exact text, in a highlighted monospace chip.
- **🔒 Lock now** in the account menu and in Settings → Lock and PIN, on a desktop with a PIN.
- **The desktop window** reopens at its size and place (it used to forget its size on macOS), and back on the main screen if its monitor is gone.
- **No network at all** (Wi-Fi off): the desktop opens its copy at once, without the 8-second wait. With Wi-Fi on but Tailscale off, the wait is as before.
- **Timeline paging:** once a list has more than 50 records, a bar above and below it: "Records 1–100 of …", ‹ Newer and Older ›, **Per page** (50, 100, 250; remembered on each device), and **Jump to** a date.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1.1 | **Mac browser**, `admin` in Driver mode: **Settings** | A column on the left: **YOU** (Units and formats, Vehicles and messages, Sign-in), **THIS DEVICE** (Appearance, Devices). No Photos (Maintainer mode and above), no Lock and PIN (the web has no lock), no Household. **Units and formats** is open, with its rows as before. |
| [ ] | 1.1a | **Maintainer mode** → Settings | **Photos** appears under YOU, with keeping originals. |
| [ ] | 1.2 | Open each section in turn; in **Units and formats**, change the date format, then change it back | Each shows its own rows; the chosen one is highlighted in the column. The change saves as before. |
| [ ] | 1.3 | Open **Vehicles and messages**; reload the page | It opens on **Vehicles and messages** again. |
| [ ] | 1.4 | **Admin mode** (with your password) → Settings | A **HOUSEHOLD** heading with **Defaults for everyone**, which has the household rows under the same four headings. |
| [ ] | 1.4a | Open **Defaults for everyone**, then **Leave** Admin mode | Settings shows **Units and formats** (Household is gone). |
| [ ] | 1.5 | **Phone**, Sam: **Settings** | A list of the sections, each with ›. Tap one: it opens on its own, with **← Settings** above it; **← Settings** goes back to the list. |
| [ ] | 1.6 | **Desktop:** Settings | **THIS DEVICE** also has **Lock and PIN** (Lock after, Lock now, Change PIN) and Appearance has **Keep window on top**. |
| [ ] | 1.7 | **Mac browser**, Maintainer mode: **+ Vehicle** `Scrap car` → **Save**; its **Setup** → **Delete…** | "To confirm, type:", then `Scrap car` in a monospace chip with an amber outline; the box below is monospace. **Delete** stays off until it's typed (capitals and spaces at either end don't matter). Type it → **Delete**: the vehicle is gone. |
| [ ] | 1.8 | **Desktop:** the account menu (your name, top right) → **🔒 Lock now** (between Change password and Sign out) | The lock screen shows at once. Unlock with your PIN: back where you were. |
| [ ] | 1.9 | Settings → **Lock and PIN** → **🔒 Lock now** | Locks at once, as in 1.8. Unlock. |
| [ ] | 1.10 | **Mac browser:** the account menu | No **Lock now**. |
| [ ] | 1.11 | **Desktop:** quit (⌘Q) and start it again (the first time after this milestone) | It may open one title bar higher than before (the older saved place); that's expected once. |
| [ ] | 1.12 | Drag the window's corner to make it clearly **smaller** and move it somewhere else; quit (⌘Q); start again | It opens **at that size and in that place**. |
| [ ] | 1.13 | Make it **larger** by dragging an edge; quit; start again. Then once more without touching it | The larger size, in the same place, both times (it doesn't creep up the screen). |
| [ ] | 1.14 | Maximize it (double-click the title bar, or Window → Zoom); quit; start again; then un-maximize | It opens maximized; un-maximizing returns it to step 1.13's size and place. |
| [ ] | 1.15 | **Second monitor:** plug it in; move the window onto it; quit; start again | It opens on the second monitor, at its size. |
| [ ] | 1.16 | Quit; **unplug** the second monitor; start again | It opens in the middle of the Mac's own screen, at its size. |
| [ ] | 1.17 | **Desktop:** quit. Turn the Mac's **Wi-Fi off** (leave Tailscale running). Start the desktop app and unlock | It opens its copy **at once** (**OFFLINE**), with no "Connecting to the server…" countdown. Turn **Wi-Fi on**: it reconnects (or **Sync now**). |
| [ ] | 1.18 | Quit. **Wi-Fi on, Tailscale off.** Start and unlock | As before: "Connecting to the server…" with its countdown (up to 8 s, and **Start in Offline Mode Now**), then **OFFLINE**. Turn Tailscale on afterwards. |
| [ ] | 1.19 | **Mac browser:** `Test truck` → **Timeline**; note how many records it has | **50 or fewer:** no paging bar. To see the bar, add readings until it has 51 or more (the phone's **+ Reading** is quickest), then go on. |
| [ ] | 1.20 | With more than 50 records: the **Timeline** | Above and below the list: "Records 1–50 of …" (or 1–100), **‹ Newer** greyed out, **Older ›**. Above it: **Per page** (100) and **Jump to**. |
| [ ] | 1.21 | **Per page: 50**; **Older ›** in the bar *below* the list | "Records 51–100 of …" (or 51 to the end); the page scrolls back to the top bar. **‹ Newer** is on. A correction and its corrected original stay together on one page. |
| [ ] | 1.22 | **Jump to:** a date in the middle of the truck's history | The page holding that date's records (or the nearest older ones). |
| [ ] | 1.23 | The **Fuel-ups** filter, then **Readings** | Each starts on its newest page, paged the same way (the bar only if that filter has more than 50). |
| [ ] | 1.24 | Reload the page; open the Timeline | **Per page** is still 50 (remembered in this browser). The **phone's** browser still shows 100 (each device keeps its own). Set the Mac back to 100 if you like. |
| [ ] | 1.25 | **Desktop:** `Test truck` → **Timeline** | The same paging; **Per page** is its own (100 until chosen). |

**Milestone 1 also checks, in the automated tests:** which Settings sections show and which opens; the window's size worked out from its frame on macOS; the no-network rule (including Tailscale addresses), and a route check on macOS, Windows, and Linux in CI; the timeline's records, pages, remembered page size, and Jump to.

**Not by hand:** the **REBUILD** dialog uses the same type-to-confirm box as step 1.7, but it shows only when the desktop's copy needs rebuilding, which can't be caused on purpose.

---

## Milestone 2: Faster logging

**Intended behavior:**
- **Photos and documents before the first save:** a new fuel-up, reading, maintenance, or repair has the photo strip ("Added when you save. ✕ removes one first."). Files upload as they're added and are tied to the record on **Save**; ✕ or **Cancel** drops them. Offline on the desktop they go into the outbox after the record.
- **Scan pages:** **+ Add → Scan pages**: 📷 First page, 📷 Next page, ‹ › to move a page, ✕ to drop one, **Make PDF (n pages)**: one PDF, "Scan" and today's date, its pages in order. On the desktop, the camera choice opens the file picker. Offline, the desktop's copy makes the PDF.
- **Pinch to zoom** in the photo viewer: pinch (up to 5×) and pan, double-tap for 2× and back; on a computer, the trackpad's pinch, Ctrl + scroll, and double-click. Swiping changes photos only when not zoomed; the next photo starts at 1×.
- **☆ Save as a place…** under Where on every log form, while the record's point isn't in one of your saved places: name and radius, then it's yours (offline too).
- **Labor rates** (Maintainer mode and above): **Settings → HOUSEHOLD → Labor rates**: the household rate, then the shops (name, contact, own rate). On a job, **Done by → Someone else** suggests the shops; a task's labor time fills its cost from the shop's rate, or the household's; a typed cost stays, with **↺ Use the rate**. Changed on the server only; the desktop shows them offline.
- **A vehicle's main photo:** in the Overview's photo viewer, ☆ "Make Main Photo" / gold ★ (click again for none), for an Admin on the vehicle in Admin mode; shown on the Garage card (at the left) and in the Overview's header; without one, the vehicle kind's icon. Chosen on the server only.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 2.1 | **Phone**, Sam: `Test truck` → **+ Fuel-up** | Under the form, **PHOTOS** with **+ Add** and "Added when you save. ✕ removes one first." |
| [ ] | 2.2 | **+ Add → Take photo**: a receipt. Add a second photo, then **✕** it | The receipt's tile shows; the second goes. |
| [ ] | 2.3 | Fill in the fuel-up → **Save**; open it from the Timeline | Saved with the **receipt photo** on it, and only that one. |
| [ ] | 2.4 | **+ Fuel-up** again; **+ Add** a photo; **Cancel** | Nothing saved; the photo isn't on any record. |
| [ ] | 2.5 | **Mac browser**, Maintainer mode: **+ Maintenance**; **+ Add** a PDF to the job's photos; finish the job | The job has the PDF. |
| [ ] | 2.6 | **Desktop, offline** (Tailscale off): **+ Fuel-up** with a photo → **Save** | Saved in the copy with its photo; the tile shows **↑** (waiting to upload). |
| [ ] | 2.7 | Tailscale on; wait for the sync (or **Sync now**). **Mac browser:** open that fuel-up | It has the photo. |
| [ ] | 2.8 | **Phone**: `Test truck` → Overview → **Photos & documents** → **+ Add → Scan pages** | **Scan pages**, with **📷 First page**. |
| [ ] | 2.9 | Shoot three pages (📷 First page, 📷 Next page ×2); **›** on page 1; **✕** on page 2 (as it now is); **📷 Next page** once more | Three pages; the strip's order is what you made it. **Make PDF (3 pages)**. |
| [ ] | 2.10 | **Make PDF**; open it | One PDF, "Scan" and today's date, **3 pages, in the strip's order**, upright. |
| [ ] | 2.11 | **Desktop, offline:** a vehicle's **+ Add → Scan pages**; choose two photos as pages → **Make PDF** | The PDF is added (made by the copy), 2 pages. Tailscale on: after the sync, the **Mac browser** shows it with 2 pages. |
| [ ] | 2.12 | **Phone**: open a photo in a strip; **pinch** to zoom in, drag, **double-tap** | Only the photo zooms (not the page), up to 5×; dragging stops at its edges; double-tap toggles 2× and back to fit. |
| [ ] | 2.13 | Not zoomed: **swipe** left, then right. Zoom in and swipe | Unzoomed, swiping changes photos (each starts at 1×); zoomed, it pans instead. |
| [ ] | 2.14 | **Desktop:** open a photo; **pinch** on the trackpad; **Ctrl + scroll**; **double-click** | Each zooms the photo only; ‹ › and the arrow keys still change photos. |
| [ ] | 2.15 | **Mac browser:** the same as 2.14 (pinch, Ctrl + scroll, double-click) | The same; the page itself doesn't zoom. |
| [ ] | 2.16 | **Phone**, Sam, somewhere **not** a saved place: **+ Reading** → let it find where you are | Under Where, **☆ Save as a place…** |
| [ ] | 2.17 | **☆ Save as a place…** | **Save as a place**: "Here: …" with the point and the town; the name (the town); radius 150 m. |
| [ ] | 2.18 | Name it `Test spot`, **Save**; then save the reading | Where shows 📍 **Test spot**; the link has gone. **Places** lists **Test spot** for Sam only (`admin` doesn't see it). |
| [ ] | 2.19 | **Desktop, offline:** a log form with a point that isn't a saved place → **☆ Save as a place…** → **Save** | Added in the copy; after the sync, **Places** on the web shows it. Remove **Test spot** and this one afterwards. |
| [ ] | 2.20 | **Mac browser**, `admin`, **Driver mode**: Settings | No **Labor rates** (and no HOUSEHOLD heading). |
| [ ] | 2.21 | **Maintainer mode** → Settings | **HOUSEHOLD → Labor rates**: "For everyone in the household, in USD…", **Household rate** (empty), **SHOPS**: "No shops yet…", **+ Shop**. |
| [ ] | 2.22 | Household rate `95` → **Save** | Saved; it reads 95.00. |
| [ ] | 2.23 | **+ Shop**: `Joe's Auto`, contact `541-555-0100`, own rate `120` → Save. **+ Shop**: `Tire Barn`, no rate → Save. **+ Shop**: `joe's auto` | Joe's Auto "$120.00 / h"; Tire Barn "household rate". The third is refused: "There's already a shop with that name." |
| [ ] | 2.24 | **+ Repair** on `Test truck`; **Who and notes → Done by: Someone else** | Under the name, chips: **Joe's Auto · $120.00 / h**, **Tire Barn · household rate**. Type `ti`: only Tire Barn. |
| [ ] | 2.25 | Choose **Joe's Auto** | Name `Joe's Auto`, contact `541-555-0100`; the chips go. |
| [ ] | 2.26 | Add a task; labor time `230` (it shows 2:30) | Labor cost **300.00**, and "2:30 × $120.00 / h (Joe's Auto). Change it for a discount or a fixed price…". The total includes it. |
| [ ] | 2.27 | Type the cost `250` | It stays 250; "At the rate: 2:30 × $120.00 / h (Joe's Auto) is $300.00. **↺ Use the rate**". |
| [ ] | 2.28 | **↺ Use the rate**; then change Done by to **Me** | 300.00 again; then **237.50** (the household's 95.00), "(household rate)". |
| [ ] | 2.29 | Save the draft; in Settings make the household rate `100`; open the draft | The task still costs **237.50** (a job keeps the cost it was saved with). |
| [ ] | 2.30 | **Desktop, offline:** Settings → **Labor rates**; then a new job's Done by | The rates and shops show, with no buttons and "This needs the server…"; the chips and the cost from the rate work offline. Delete the draft afterwards. |
| [ ] | 2.31 | **Mac browser**, **Admin mode**: `Test truck` → Overview → **Photos & documents**; add a photo if it has none; open it | A hollow **☆** in the viewer's bar; its tooltip **Make Main Photo**. |
| [ ] | 2.32 | Click **☆** | It turns gold **★**. The header shows the photo beside the name; **Garage**: the truck's card has it at its left, the card no taller than the others. |
| [ ] | 2.33 | **Phone**, Sam: the Garage; open the truck's photo | The photo on the card; in the viewer, only the gold **★** on that photo (nothing to click). |
| [ ] | 2.34 | **Mac browser**, `admin` in **Maintainer mode**: the photo viewer | No star to click (only the gold one shows). |
| [ ] | 2.35 | Back in **Admin mode**: click the gold **★** | No main photo: the header and the card show the truck icon. |
| [ ] | 2.36 | Make it the main photo again; then **Remove** that photo | The card and the header go back to the truck icon. Other vehicles show their kind's icon (car, trailer, generator…). |
| [ ] | 2.37 | **Desktop, offline:** the Garage and the truck's Overview; open a photo | The main photo (or icon) shows from the copy; the star is greyed out, its tooltip "This needs the server…". |

**Milestone 2 also checks, in the automated tests:** the held files becoming the record's attachments in order; packing scanned pages and joining them into one PDF (order, page sizes, orientation, the 100-page and 50 MB limits); the place saved while logging; the rate's limits and a shop's checks; who may change shops and rates; the cost at the rate (rounded to the cent) and which rate applies; the suggestions; which costs the rate fills and which stay; the main photo's rules (only a vehicle's own photo, who may choose, a removed photo counting as none); no shops, rate, or main photo in any older release's database.

**Not by hand:** the 100-page and 50 MB limits on a scan (too many photos to take by hand; the tests cover them).

---

## Milestone 3: Vehicles and schedules

**Intended behavior:**
- **Pinned vehicles** (each person's own): **Pin** in a card's **⋯** menu (and in Arrange mode) puts 📌 before its name, and it comes first in its own group whatever the sort; among the pinned, the sort holds. **Unpin** undoes it. Works offline on the desktop.
- **Chained intervals:** a meter or calendar rule's **A different first interval** ("First at 100,000 mi, then every 50,000 mi"). Until it's done once, or "last done before these records" is typed, the first interval counts from the meter's zero, or from the vehicle's **In service since** (else Acquired, else the day it was added); then only the repeating part.
- **In service since** on Setup → Edit details: a date (not after today) and **Estimated**; shown "12 Jun 2019 (est.)".
- **The catalogue:** Setup → Custom fields → **+ From the catalogue**: the sets (Trip computer, Trip computer (hybrid), Engine and idle, **Tires**), then single fields; one already on the vehicle is greyed out. A set added again gets " B". **Tires** asks for the axles and the tires on each, and adds a **pressure** (in your pressure unit) and a **tread depth** (32nds of an inch with miles, mm with kilometers) for each tire.
- **Vehicle types** (Manage → **Vehicle types**; the Admin, in Admin mode, changes them): each built on a kind, with an icon, its default usage's documents, its kind's fields on or off, fields from the catalogue, and a template. **+ Add vehicle** lists them after the kinds; a vehicle of a type shows its icon (with no main photo) and name. A change reaches only vehicles added afterwards; a deleted type's vehicles show their kind.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 3.1 | **Phone**, Sam: the Garage; `Test truck`'s **⋯** | **Open**, **Pin 📌** (no Move to group: Sam isn't the Admin). |
| [ ] | 3.2 | **Pin**; then try each sort (most urgent, newest activity, name, custom) | 📌 before its name; it stays first in its group in every sort. |
| [ ] | 3.3 | **Mac browser**, `admin`: the Garage | `Test truck` isn't pinned for `admin` (pins are each person's own). |
| [ ] | 3.4 | **Desktop, offline**, as Sam: **Arrange** → **Unpin** on the truck; Tailscale on, sync | Unpinned in the copy at once; after the sync, unpinned on the phone too. |
| [ ] | 3.5 | **Mac browser**, `admin`, **Admin mode**: **+ Add vehicle** → Car: `Used car`, year 2015. Setup → **Edit details**: In service since **2018-06-01**, **Estimated** → Save | The details show **In service since 1 Jun 2018 (est.)**. |
| [ ] | 3.6 | Edit details again: In service since **tomorrow** | Refused: the date can't be after today. Put back 2018-06-01. |
| [ ] | 3.7 | **Maintainer mode**: `Used car` → Schedule → **+ Requirement** `Coolant`: **Every 2 years**, tick **A different first interval**: **First at 5 years** → Save | The Schedule reads **First at 5 years, then every 2 years**; **overdue** (since 1 Jun 2023, five years after In service since). |
| [ ] | 3.8 | Edit `Coolant`: **Last done before these records** `2025-09-01` → Save | **Every 2 years**; due **1 Sep 2027** (two years after it), no longer the first interval. |
| [ ] | 3.9 | `Test truck` → Schedule → **+ Requirement** `Timing belt`: **Every 50,000 mi** on the odometer, **First at 100,000 mi** → Save | **First at 100,000 mi, then every 50,000 mi**; due at **100,000 mi** (from the meter's zero), not at the odometer now + 50,000. |
| [ ] | 3.10 | **Log this service** for `Timing belt`, at today's odometer reading | Now **Every 50,000 mi**, due 50,000 mi after that reading. |
| [ ] | 3.11 | **Copy schedule** from `Test truck` to `Used car` (or save it as a template and apply it) | The timing belt's first interval comes too (Used car shows "First at 100,000 mi…"; its own odometer is used). |
| [ ] | 3.12 | `Test truck` → **Setup** → Custom fields → **+ From the catalogue** | **From the catalogue · Test truck**: SETS (Trip computer, Trip computer (hybrid), Engine and idle, Tires), FIELDS; **Trip time** greyed "Duration · already on this vehicle". |
| [ ] | 3.13 | **Trip computer (hybrid) → Add** | Six new fields (trip distance, average speed, fuel economy, fuel used, EV share, EV time); **Trip time** kept; **Trip EV distance** turned **on**. Average speed and EV share show **fx** and their formulas. |
| [ ] | 3.14 | **+ From the catalogue** → **Trip computer (hybrid)** again | Eight more, labelled **… B** (e.g. **Trip distance B**, key `trip-distance-b`); **Trip EV share B**'s formula is `trip-ev-distance-b ÷ trip-distance-b`. Remove the B fields afterwards. |
| [ ] | 3.15 | **+ From the catalogue → Tires → Add**: **2** axles; Axle 1 **2**, Axle 2 **4 (dual)** | "6 tires · 12 fields: … (1L, 1R, 2L outer, 2L inner, 2R inner, 2R outer)"; the button reads **Add 12 fields**. |
| [ ] | 3.16 | **Add 12 fields**; then **+ Reading** on the truck | The reading form has **Tire pressure 1L (psi)** … and **Tread depth 1L (/32 in)** …, by tire. |
| [ ] | 3.17 | Type pressure `35` and tread `7` on 1L → Save; open the reading | **35 psi** and **7/32 in**. |
| [ ] | 3.18 | Settings → pressure **kPa**, distance **kilometers**; open the reading again | **241 kPa** and **5.6 mm**. Put your units back. |
| [ ] | 3.19 | **Phone**, Sam: Manage → **Vehicle types** | The list (none yet); no **+ Add type** (Sam can't change types). |
| [ ] | 3.20 | **Mac browser**, `admin`, **Admin mode**: Manage → **Vehicle types** → **+ Add type**: name `Track car`, icon **Race** (flag), Built on **Car**; documents: only **Insurance**; Trip time **off**; from the catalogue: **Tires: 2 axles, 4 tires**; template: one you have (or none) → **Save** | Saved; the list shows 🏁 **Track car · 0 vehicles**; the summary reads "Needs Insurance · 1 meters · 10 fields · …". |
| [ ] | 3.21 | Add a second type with the same name in other capitals (`TRACK CAR`) | Refused: "Another vehicle type has that name." |
| [ ] | 3.22 | Make a type built on **Trailer** with **Trip computer** from the catalogue | Refused: something from the catalogue doesn't fit (a trailer has no fuel-ups). |
| [ ] | 3.23 | **+ Add vehicle** | The kinds, then **Your household's types**: 🏁 **Track car**. |
| [ ] | 3.24 | **Track car** → name `Miata` → **Add** | Added. If the template had meter rules with no matching meter, they're listed before the dialog closes. |
| [ ] | 3.25 | The Garage, and `Miata`'s Overview | The card: the **flag** icon, **Track car**; the header: **Track car (Car)**. Its readiness needs only **Insurance**. Setup: Trip time off; four tires' pressure and tread depth fields; the template's schedule (if one was chosen). |
| [ ] | 3.26 | Change `Track car`'s documents (add Registration) → Save; open `Miata` | `Miata` is unchanged (still Insurance only): a change reaches only vehicles added afterwards. |
| [ ] | 3.27 | `Used car` → Setup → **Edit details** → **Type** | Lists **Track car** (built on Car) and "None: shows as its kind". Choose **Track car** → Save: only its label and icon change (its fields are as they were). Set it back to None. |
| [ ] | 3.28 | **Desktop, offline:** Manage → **Vehicle types**; **+ Add vehicle** | The types show from the copy (read only); adding a vehicle needs the server. |
| [ ] | 3.29 | **Mac browser**, Admin mode: **Track car → Delete type** → confirm | Gone from the list; `Miata` shows as a **Car** (the car icon, "Car"), with everything it had. Remove `Miata` and `Used car` afterwards. |

**Milestone 3 also checks, in the automated tests:** the Garage's sort with pins in every sort, and a pin for a vehicle that's gone or in another group; the pin operation offline and synced (API version 3, version 2 still accepted); a first interval from each base (zero, In service since, Acquired, the day added), when done, and with "last done before"; its checks and stored form; copying it; the catalogue's data (unique keys, labels, every formula checking); a set again with " B" and " C", a kind's own fields used and turned on, a removed field's key never reused, the vehicle's kinds of events; the tires' names for 1–4 axles; pressure and tread depth both ways in every unit; a vehicle type's checks, who may change it, a vehicle added as one (kind, fields, documents, template, rules left out), a type's change and removal leaving its vehicles be, and a vehicle's type only one of its kind; no pins, In service since, or types in any older release's database.

**Not by hand:** an older desktop against the new server (API versions; the tests cover it).
