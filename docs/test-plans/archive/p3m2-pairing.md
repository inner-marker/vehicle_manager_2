# Manual test plan: Phase 3 milestone 2, devices and pairing

*Phase 3 milestone 2 · branch `milestone/p3-m2-pairing` · `docs/phase_plans/phase-3.md` · decision #073 · sketch `docs/ui-sketches/p3m2-devices-options.svg` (1A, 2B, 3A)*

**Result (2026-11-11): all 16 steps passed** on `:milestone-p3-m2-pairing` (commit `a4749cd`), with no findings.

## 1. Intended behavior

- **Every session belongs to a device** (migration 0016). Existing sessions survive the upgrade, each as an unnamed "Browser".
- **Browsers are named** at sign-in from their user agent: "Safari on iPhone", "Chrome on Mac".
- **Signing in on the desktop pairs it:** it makes a device key, the server keeps the public half, and the desktop **stays signed in** across restarts (debug builds keep the key and token in `device-credentials.json` in the data folder; release builds in the macOS Keychain).
- **A paired desktop's requests are signed:** its token alone is refused (`device-signature`, 401).
- **Settings → Devices** (a tab for everyone): this device first (Rename), the others (Sign out), and Sign out everywhere else. A desktop signed out from here forgets its pairing and shows sign-in.
- **Users → a person → Devices (n)** (household Admin, Admin mode): their devices, Sign out one or all; never the Admin's own device in use.
- **Desktop sign-in:** the servers this desktop has used, as cards (the current one checked), and "+ Another server…"; choosing another says "Quit and reopen the app to switch".
- **API version 2.** The main server is on `0.2.0` (API version 1), so this desktop can't sign in there until the next release: it says the server needs updating. That's expected.

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m2-pairing` finished (commit `a4749cd` or later).
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m2-pairing`, then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- **Before upgrading,** sign in to the test server in the Mac's browser and leave the tab open (for step 2).
- **The desktop app** from this branch, pointed at the test server (as in milestone 1): `cd crates/app && dx serve --platform desktop`. It's a debug build: its pairing goes in the file, not the Keychain (step 14 tries the Keychain).
- An iPhone on the tailnet.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m2-pairing · a4749cd…`; `database open … schema_now=16` (`schema_before=15` on the first start). |
| [x] | 2 | **Mac browser:** reload the tab left open, then Settings → **Devices** | Still signed in. Tabs **You \| Devices**. One row, **Browser** (unnamed: from before the upgrade), **This device**. "You're signed in only here." |
| [x] | 3 | Sign out, sign in again; Settings → Devices | The row now says **Chrome on Mac** (or Safari on Mac). |
| [x] | 4 | **iPhone:** sign in; Settings → Devices | **Safari on iPhone · This device** first, then the Mac's browser with **Sign out**. |
| [x] | 5 | **Desktop:** start it | The sign-in card shows **SERVER** with the test server's card checked, "+ Another server…", then username and password. |
| [x] | 6 | **Desktop:** sign in | Signed in. **Mac browser:** Settings → Devices (refresh the page) lists your Mac's name (e.g. Jordans-MacBook-Pro), **Desktop app**. |
| [x] | 7 | **Desktop:** quit (⌘Q) and start again | Signed in straight away, no password. The sign-in card isn't shown. |
| [x] | 8 | **Desktop:** open a vehicle, add a photo, then open it | Upload and viewing work (these requests are signed one by one). |
| [x] | 9 | **A copied token is refused.** Mac Terminal (prints only the server's answer, never the token): `TOKEN=$(python3 -c 'import json,os; d=json.load(open(os.path.expanduser("~/Library/Application Support/vehiclemanager/device-credentials.json"))); print(next(iter(d.values()))["token"])'); curl -s https://<nas-name>.<tailnet>.ts.net:8443/api/v1/devices -H 'x-api-version: 2' -H "authorization: Bearer $TOKEN"; unset TOKEN` | `{"code":401,"data":"device-signature",…}`. |
| [x] | 10 | **Desktop:** Settings → Devices → **Rename** → "Garage Mac" → Save | The row says **Garage Mac**; the Mac browser's list shows it too. |
| [x] | 11 | **Mac browser:** Devices → **Sign out** on Garage Mac. **Desktop:** click to another screen (or wait a minute) | The desktop shows the sign-in card. Quit and start it: still signed out (it forgot the pairing). Signing in pairs it again (a new row). |
| [x] | 12 | **Admin:** Mac browser, Admin mode → Users → you → **Devices (n)** | The dialog lists your devices; the browser you're using has no Sign out button. **Sign out** the iPhone: it disappears; the iPhone shows sign-in at its next action. |
| [x] | 13 | **Desktop:** sign out (account menu), then on the sign-in card **+ Another server…** → type the main server's address → **Use this server** | "Saved. Quit and reopen the app to switch to …". Quit and start: its card is checked; signing in there says the server must be updated (main is on `0.2.0`). Pick the **test server's** card, quit and start: back on the test server. |
| [x] | 14 | *(Optional, the Keychain)* Quit the desktop; `dx serve --platform desktop --release`; sign in | macOS may ask to let "app" use the keychain: **Always Allow**. Quit and start again: still signed in. Keychain Access → search `vehiclemanager`: an item for the test server. |
| [x] | 15 | **Everything else:** Admin mode on the desktop (with your password), log a reading, open a PDF | As before. |
| [x] | 16 | Afterwards: **leave the test server on `:milestone-p3-m2-pairing`** until this milestone merges, then move it to `:dev` | Its database is now at schema 16, and today's `:dev` (schema 15) would refuse to open it ("written by a newer version of the app"). Once the PR merges, `:dev` has migration 0016 too. |

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1` (refusals say `API request refused: device signature` with the reason).
- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer (Minimum level: Debug).
- **Never share** a password, a token, `.env`, a cookie, or `device-credentials.json`.
