# Manual test plan: Phase 3 milestone 3, app lock and PIN

*Phase 3 milestone 3 · branch `milestone/p3-m3-app-lock` · `docs/phase_plans/phase-3.md` · decision #074 · sketch `docs/ui-sketches/p3m3-lock-options.svg` (1B, 2B, 3A)*

## 1. Intended behavior

- **The desktop's sign-in is sealed** with a PIN and with the password (Argon2id, then XChaCha20-Poly1305). The file (debug builds) or Keychain item (release) holds no readable key or token.
- **A pairing from milestone 2 is forgotten** the first time: the desktop asks you to sign in once more.
- **Choose a PIN** right after signing in on the desktop (required): 6 to 12 digits, no repeats (`111111`) or runs (`123456`, `654321`), typed twice. The keypad and the keyboard both work.
- **Locked** when the app opens, and after the idle time in **Lock after** (15 minutes by default). It shows your name and the server's host. The PIN or **Use your password instead** opens it; **Sign out of this computer** signs out and forgets the pairing.
- **Wrong tries:** after 5 wrong PINs only the password works; after 10 wrong tries in all, the desktop signs out and forgets its sign-in ("Signed out of this computer after too many wrong tries…"). The count survives quitting.
- **Failed unlocks are reported** after the next successful unlock: the household Admin, in Admin mode, sees **Wrong PINs on a computer** in Messages.
- **Settings → This device** (desktop only): **Lock after** (5, 15, 30, 60 minutes, or only when the app opens) and **Change PIN** (the current PIN or the password, then the new one twice). A wrong current PIN counts as a wrong unlock.
- **Browsers have no lock** and no Lock after or PIN rows.

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m3-app-lock` finished.
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m3-app-lock` in its `.env`, then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- **The desktop app** from this branch, pointed at the test server: `cd crates/app && dx serve --platform desktop`. It's a debug build, so its sealed sign-in goes in `device-credentials.json` (step 18 tries the Keychain).
- **Two accounts:** yours (the household Admin) on the desktop, and the Mac's browser for Messages. A wrong-PIN report goes to the household Admin, so it can be yours.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m3-app-lock · …`; `database open … schema_now=17` (`schema_before=16` on the first start). |
| [x] | 2 | **Desktop:** start it | Milestone 2's pairing is forgotten: the **sign-in card** shows (not Locked). The `dx serve` terminal says "this device's sign-in is from before the app lock; it signs in once more". |
| [x] | 3 | Sign in | **Choose a PIN** fills the window: dots, a keypad, "6 digits or more…". The app isn't reachable behind it. |
| [x] | 4 | Type `123456` and ✓; then `111111` and ✓; then `28491` and ✓ | "Don't use a run like 123456.", "Don't repeat one digit.", "Use at least 6 digits." |
| [x] | 5 | Type a good PIN with the **keyboard** (digits, Backspace to fix one, Enter); then a different one | "Type it again.", then "The two PINs didn't match. Choose one again." |
| [x] | 6 | Choose the PIN for real (twice, with the keypad's buttons) | "Saving…" for a moment, then the app. |
| [x] | 7 | **Sealed:** Mac Terminal (prints only the entry's field names): `python3 -c 'import json,os; d=json.load(open(os.path.expanduser("~/Library/Application Support/vehiclemanager/device-credentials.json"))); print(sorted(next(iter(d.values()))))'` | `['last_wrong_ms', 'password', 'person', 'pin', 'tries', 'version']`: no `token` or `secret_key`. |
| [x] | 8 | **Desktop:** quit (⌘Q) and start again | **Locked**: "Your name · the server's host", dots, keypad, "Use your password instead", "Sign out of this computer". |
| [x] | 9 | Type the PIN and ✓ | "Unlocking…" briefly, then the app, signed in (no password). |
| [x] | 10 | Quit, start; type **3 wrong PINs** | "Wrong PIN." each time; after the third, "2 more wrong PINs and only your password will work." |
| [x] | 11 | Quit and start again | Still "2 more wrong PINs…" (the count was kept). Unlock with the right PIN. |
| [ ] | 12 | **Mac browser:** Admin mode → **Messages** | **Wrong PINs on a computer**: "Someone typed 3 wrong PINs or passwords on (your Mac's name) (your name) before it was unlocked. …" |
| [x] | 13 | **Desktop:** Settings → **You** | Under **THIS DEVICE**: Theme, Keep window on top, **Lock after: 15 minutes**, **PIN: Change PIN**. **Mac browser:** Settings → You shows Theme only, no Lock after or PIN. |
| [x] | 14 | **Desktop:** Lock after → **5 minutes**; leave the app alone for 5 minutes (don't move the mouse over it) | Within about 15 seconds after the 5 minutes, **Locked**. Unlock with the **password** ("Use your password instead"). Set Lock after back to 15 minutes; quit and start: it still says 15 minutes. |
| [x] | 15 | **Change PIN:** click it; current PIN wrong, new PIN twice → Change PIN | "Wrong PIN. 4 more wrong PINs and only your password will work." Then: right current PIN, new `123456` → "Don't use a run…" (no try counted); new PIN and a different one → "The two PINs didn't match…"; then a good new PIN twice → "PIN changed. Use the new one from now on." |
| [x] | 16 | Quit and start; the **old** PIN, then the **new** one | Old: "Wrong PIN."; new: unlocks. The Admin's Messages show a new report for the one wrong try (and the step 15 one). |
| [x] | 17 | **Too many tries:** quit, start; 5 wrong PINs, then 5 wrong passwords | After the 5th PIN only the password field shows ("Use your password. …"). The 10th wrong try shows the sign-in card with "Signed out of this computer after too many wrong tries. Sign in to use it again." Quit and start: still the sign-in card. Sign in: Choose a PIN again. *(No report reaches Messages for these: the desktop forgot its sign-in.)* |
| [x] | 18 | *(Optional, the Keychain)* Quit; `dx serve --platform desktop --release`; sign in, choose a PIN | **Always Allow** if macOS asks. Quit and start: Locked; the PIN unlocks. Keychain Access → `vehiclemanager`: the item's contents (Show password) are `salt`/`nonce`/`sealed` hex, no readable token. |
| [x] | 19 | **Locked → Sign out of this computer** | The sign-in card. Quit and start: still the sign-in card (the pairing is forgotten). |
| [x] | 20 | **Everything else:** sign in again (with a PIN); Admin mode with your password, log a reading, open a photo | As before. |
| [x] | 21 | Afterwards: **leave the test server on `:milestone-p3-m3-app-lock`** until this milestone merges, then move it to `:dev` | Its database is now at schema 17; today's `:dev` (schema 16) would refuse to open it. |

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve` (the lock logs "locking this desktop", "wrong unlock wrong=…", "unlocked", "PIN changed"), and Diagnostics → the log viewer (Minimum level: Debug).
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **Never share** a PIN, a password, a token, `.env`, a cookie, or `device-credentials.json` (field names, as in step 7, are fine).

# Human Notes

- Not related to this checklist: users should be able to change their name and username. A household admin setting might be allowing users to chance their own name and username. Either way, the admin should always have the ability.
- There were no mesages displayed in the messages page.
- There should be a menu option to lock the app under the menu inder the user's name.

## 5. Findings and fixes (2026-11-11)

- **No message in Messages (steps 12 and 16): a bug, fixed.** After a good unlock, the lock screen opened the app and *then* started the task that refreshes the session and sends the report. Opening the app removes the lock screen, and Dioxus cancels a screen's tasks when it's removed, so the report was never sent. The task now outlives the screen (`spawn_forever`), and the desktop logs the outcome ("reported the wrong unlock tries", or why not). The same fix covers the refresh after signing out from the lock screen. **Retest:** steps 10–12 (and 16) with a new image.
- **Lock now** in the account menu: a Polish-phase note (plan.md §9 Phase 6).
- **Changing a person's name and username:** a GitHub issue, planned later.
