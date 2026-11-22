# Test plan: Phase 6, polish

*`docs/phase_plans/phase-6.md` · decisions #081, #089 · built up milestone by milestone, run at the end of the phase*

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
