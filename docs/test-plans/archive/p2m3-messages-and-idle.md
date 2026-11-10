# Manual test plan: leaving Admin mode and signing out when idle; the message center

*Phase 2, milestone 3 · `docs/phase_plans/phase-2.md` · decisions #049, #050, #053*

## 1. Intended behavior

- **Idle (#049):**
  - Only real input counts as activity: a tap or click, a key, scrolling, a touch. Background requests and just moving the mouse don't.
  - **Admin mode ends after 15 minutes without input,** by itself, within about 15 seconds of the 15 minutes. The orange banner goes, the chip says **Driver**, and a notice under the top bar says "Admin mode ended after 15 idle minutes." with **Dismiss**.
  - **Working in Admin mode keeps it,** even on a screen that makes no requests. While there's input, the app checks in with the server once a minute.
  - **Sign out when idle** (Settings → You → **Sign-in**): After 1 hour, 4 hours, **8 hours** (default), 24 hours, or Never. Each person can choose their own, or **Use household default**. The Admin sets the default on the Household tab. When the time passes, the app signs out, and the sign-in card says "You were signed out after N idle hours." Signing in again clears it.
  - **A computer that sleeps** wakes up with the whole sleep counted as idle, so it leaves Admin mode or signs out at once if the time has passed.
  - The server's own limits still apply: 15 idle minutes for Admin mode, and 30 idle days for a session.
- **Messages (#050, #053):**
  - **Messages come from what's true now:** a failed or skipped nightly snapshot, a failed hourly database check, and the server refusing changes. A message disappears by itself when its cause is fixed. A new failure is a new message.
  - **Who sees what:** job messages are for the household's Admin, **in Admin mode only**. "The server isn't accepting changes" is for everyone. Technical **details** show only in Admin mode.
  - **The screen:** a list on a darker panel, a divider, and the chosen message. Filters: All, Urgent, Warning, Info. Opening a message marks it read. **Dismiss** puts it away for you only, and opens the next one. Phones show the list, then a page with **← Messages**.
  - **Everywhere:** an unread badge on **Messages** (sidebar and phone tabs), and a red **URGENT** banner under the top bar while an urgent message is showing, with **View**.
  - Marks (read, dismissed) are saved on the server, so they're the same on every device and survive restarts.
  - Times show as UTC (e.g. `2026-11-01 07:00 UTC`) for now. Dates follow each person's format and time zone from milestone 6.

## 2. Setup

- **The milestone image:** after the branch is pushed, the "Dev image" workflow builds `milestone-p2-m3-messages`. Check Actions → it finished successfully.
- **On the NAS over SSH:**

  ```sh
  cd /volume1/docker/vehicle_manager
  vi .env
  ```

  Set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m3-messages`, then `Esc` and `:wq`. For test timers (below), also get the milestone's `compose.yaml`, which passes `VEHICLEMANAGER_TEST_MINUTES` into the container:

  ```sh
  curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-m3-messages/deploy/compose.yaml
  ```
- **People:** your Admin account ("stuart" below) and Sam from milestone 2 (any role but Admin). If Sam is disabled, enable them in Users.
- **Devices:** the browser on the Mac (or another computer), a phone browser, and the desktop app on the Mac (`dx serve --platform desktop` from `crates/app`, with the NAS address saved).
- **Timing:**
  - Steps 5–7 need about 20 minutes each, and step 8 about an hour (do other things on another device meanwhile).
  - Steps 13–19 need the server running past **07:00 UTC** (03:00 Eastern daylight time; 02:00 after November 1, 2026). Set up step 12 one evening, then do 13–19 the next morning.
  - Leave the tab **visible** during the waits. A hidden tab still works, but the browser may slow its timers to once a minute.
- **Test timers (optional, decision #054): minutes instead of hours.** The milestone image can shorten the timers. Get the milestone's `compose.yaml` (above), pull the newest image, then in `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, run `sudo docker compose up -d`, and reload the app. `sudo docker inspect vehiclemanager --format '{{range .Config.Env}}{{println .}}{{end}}' \| grep TEST` should show the setting. A **TEST TIMERS · 2 min** tag shows in the top bar, and the log has a `TEST TIMERS ON` warning. With 2 minutes:

  | Steps | Wait with test timers | Notes |
  |---|---|---|
  | 5, 7, 10, 11 | **3 minutes** instead of 16–20 (Admin mode ends after 2 idle minutes) | The banner and dialog say "2 idle minutes"; the notice says "Admin mode ended after 2 idle minutes." |
  | 6 | Click about **every minute** for **5 minutes** | Still in Admin mode at 5 minutes. |
  | 8 | **3 minutes** instead of 61 ("After 1 hour (test: 2 min)") | The sign-in card says "You were signed out after 2 idle minutes." |
  | 9 | No wait | The list shows each choice's test length, e.g. "After 4 hours (test: 4 min)". |
  | 12–19 | **No overnight wait:** the snapshot and the check run **every 2 minutes** | **Each run is a new message:** while the snapshot folder is missing, a new "skipped" message replaces the last one every 2 minutes, unread again, with a new time. So step 18's "badge already gone" and step 19's "stays dismissed" hold only until the next run. |
  | 20 | **3 minutes** | After the fix, the next run succeeds and the message **disappears by itself** (no dismissing); a new snapshot appears in `data/snapshots/` every 2 minutes. |

  **When you're done, empty `VEHICLEMANAGER_TEST_MINUTES=` and run `sudo docker compose up -d`:** the tag disappears and the timers are normal again. Recovery codes also last 2 minutes while test timers are on.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager` | `backed up before migrating` with `schema_before=3`; `database open` with `created=false`, `schema_before=3 schema_now=5`, and the **same database ID** as before; `database integrity check passed`. No `ERROR` lines and no setup code. |
| [x] | 2 | `ls -ln data/backups/` | A new `pre-migration-<UTC time>-v3.sqlite`, owned by `1030 100`. |
| [x] | 3 | Sign in as stuart in the browser. Open **Messages** | Driver mode: "No messages. You're all caught up." No badge on Messages, no red banner. (If a real snapshot failed recently, you'd see it only in Admin mode; see step 15.) |
| [x] | 4 | **Settings → You**, scroll to **SIGN-IN** | **Sign out when idle**: "After 8 hours", with **Use household default** ticked. |
| [x] | 5 | **Admin idle:** enter Admin mode, then don't touch the computer for **16 minutes** | Without any click, Admin mode ends: the orange banner goes, the chip says **Driver**, and "Admin mode ended after 15 idle minutes." shows under the top bar. **Dismiss** hides it. |
| [x] | 6 | **Working keeps Admin mode:** enter Admin mode; about every 5 minutes, scroll or click somewhere harmless (e.g. a Settings tab), for **20 minutes** | Still in Admin mode at 20 minutes. Then open **Users**: it lists the users (the server still agrees you're in Admin mode). |
| [x] | 7 | **The server's own limit:** while in Admin mode, close the browser tab, wait **16 minutes**, open the app again | Driver mode: the server ended Admin mode by itself. |
| [x] | 8 | **Sign out when idle:** Settings → You → **After 1 hour** (the switch unticks). Don't touch the computer for **61 minutes** | The sign-in card says "You were signed out after 1 idle hour." Sign in again: the notice is gone. Then tick **Use household default** again. |
| [x] | 9 | **Household default:** in Admin mode, Settings → **Household** → **After 4 hours**. Then sign in as Sam on the phone → Settings → You | Sam's **Sign out when idle** shows "After 4 hours", following the household. Put the household back to **After 8 hours**. |
| [x] | 10 | **Sleep:** on the Mac, enter Admin mode in the browser, close the lid for **20 minutes**, open it | Within about 15 seconds: Driver mode, with the "Admin mode ended…" notice. |
| [x] | 11 | **Desktop app:** sign in, enter Admin mode, wait **16 minutes** without touching it | The same as step 5. Then open **Messages**: the screen shows, with its list, divider, and detail panel. |
| [x] | 12 | **Evening: make tomorrow's snapshot skip.** In `.env` set `VEHICLEMANAGER_SNAPSHOT_DIR=/snapshots` (nothing is mounted there), then `sudo docker compose up -d` | The server restarts normally. |
| [x] | 13 | **Morning (after 07:00 UTC):** `sudo docker logs vehiclemanager 2>&1 \| grep nightly-snapshot` | A `job skipped job="nightly-snapshot"` line: "snapshot folder /snapshots is missing (not mounted?)". |
| [x] | 14 | As stuart in **Driver** mode, and as Sam: look at **Messages** | No badge, no messages for either (it's an Admin message, Admin mode only). |
| [x] | 15 | As stuart, enter **Admin mode** | Badge **1** on Messages in the sidebar. No red banner (it's a warning, not urgent). |
| [x] | 16 | Open **Messages** | One row, with an unread dot and an amber stripe: "The nightly snapshot was skipped", "WARNING · <date> 07:00 UTC". "Choose a message to read it." on the right. |
| [x] | 17 | Open it | The right side shows the text about backups, **DETAILS**: "snapshot folder /snapshots is missing (not mounted?)", **Open Diagnostics** and **Dismiss**. The dot and the badge are gone, and the list says "All read". **Open Diagnostics** goes to Diagnostics. |
| [x] | 18 | On the phone (stuart, Admin mode): **Messages** tab | The list; tapping the message opens its page with **← Messages**, which goes back. Its badge was already gone (read on the Mac). |
| [x] | 19 | Back on the Mac: **Dismiss**. Then `sudo docker compose restart`, reload the page (sign in again if asked), Admin mode, Messages | "No messages. You're all caught up." It stays dismissed after the restart. |
| [ ] | 20 | **Fix it:** empty `VEHICLEMANAGER_SNAPSHOT_DIR=` in `.env`, `sudo docker compose up -d` | The server restarts normally. *(Optional, the next morning: a new snapshot in `data/snapshots/`, and still no message.)* |
| [x] | 21 | **Secrets check:** `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` | `0`. |

**Not testable by hand:** "The server isn't accepting changes" for everyone (urgent, with the red banner on every screen). It needs a database that opens but fails its integrity check, which can't be made safely by hand. If the database can't be opened at all (milestone 1's drill), nobody can sign in, so no messages can show. The server's automated tests cover it (`an_unhealthy_server_tells_everyone_and_refuses_marks`). The urgent banner itself was checked in a browser with a failed hourly check during development.

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager`. Sign-in and mode switches are on the `auth` area; jobs and the database on `database`; the message center on `app`.
- **App log** (browser or desktop app): Diagnostics → the log viewer, set to **Debug**. The idle watcher logs on the `auth` area ("idle watcher started", "keeping Admin mode alive", "leaving Admin mode", "signing out"). The inbox logs on the `ui` area ("inbox loaded" with the item and unread counts).
- **Steps 5–11:** roughly when you last touched the device, and when the change happened (or didn't).
- **Screenshots** of the screen in question, in both themes if it's a display problem.
- **Never share** a password, `.env`, or a cookie.
