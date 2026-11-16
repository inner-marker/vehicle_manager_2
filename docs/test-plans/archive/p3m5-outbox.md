# Manual test plan: Phase 3 milestone 5, outbox and push

*Phase 3 milestone 5 · branch `milestone/p3-m5-outbox` · `docs/phase_plans/phase-3.md` · decision #076 · sketch `docs/ui-sketches/p3m5-outbox-options.svg` (1A, 2A, 3B)*

## 1. Intended behavior

- **Offline, the desktop saves what Driver mode allows:** log, edit, finish, finalize, delete, and correct events; open, edit, and remove squawks; your settings, saved places, and Garage order. Each is saved on this computer's copy at once (the same checks as the server's, so a mistake shows in the form) and kept in an **outbox**.
- **Sending:** online, a change goes straight away and the form shows the server's answer as before. Offline, it waits; it's sent when the server can be reached (every minute, at unlock, when a screen catches up, at **Sync now**). Changes go in order and are never applied twice.
- **↑ 3 to send** (top bar, 1A) while changes wait; its tooltip says how many and from when; a click opens Diagnostics.
- **Two devices, one record:** each field keeps its newest change, so changes to different fields of one event both stand.
- **The server checks everything again.** A change it refuses (say, a reading lower than one logged meanwhile elsewhere) becomes an amber **"Not saved: …"** message at the top of Messages on this computer (2A), with the reason, **Open** (back in its form, as typed) and **Discard**. After a refusal, the copy downloads the server's records again.
- **Signing out** with changes not sent lists them first (3B): **Send now**, **Cancel**, **Sign out anyway**.
- **A clock far ahead** (more than 5 minutes): its changes wait, Messages on the desktop says "Changes can't be sent", and the Admin gets "A computer's clock is wrong" until it's set right.
- **Still needs the server:** adding or removing photos and documents (milestone 6), changing the password, and anything in Admin mode.
- **The web app is unchanged** (the top bar's old "Sync not set up" placeholder is gone).

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m5-outbox` finished.
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m5-outbox` in its `.env`, then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- **The desktop app** from this branch, pointed at the test server: `cd crates/app && dx serve --platform desktop`, signed in with a PIN.
- **Offline** here means **Tailscale off on the Mac** (the network stays up, but the server can't be reached). That also cuts off the Mac's browser, so the "web" side of some steps is **your phone** (on Tailscale), signed in to the test server.
- A vehicle with an odometer and a few events (from earlier tests). Note its latest odometer reading: **R** below.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m5-outbox · …`; `database open … schema_now=19` (`schema_before=18` on the first start). |
| [x] | 2 | **Desktop, online:** log a reading of **R + 5**, then open it | Saved and shown at once, as before. No chip in the top bar (nothing waits). **Phone:** the reading is there. |
| [x] | 3 | **Desktop:** log a reading **lower** than R + 5 (dated now) | Refused in the form, as before (the copy's own check). Nothing waits. |
| [x] | 4 | **Offline:** turn **Tailscale off**. Wait for the **OFFLINE** tag (or Diagnostics → Sync now) | The OFFLINE tag. **+ Log**, **+ Squawk**, Save, Edit, Finish are **on** (not dimmed). |
| [x] | 5 | Offline: log a fuel-up on the vehicle (odometer **R + 20**), then open it | Saved, and it shows straight away. The top bar shows **↑ 1 to send**; hover: "1 change waiting to be sent, from (date time)…". |
| [x] | 6 | Offline: open a squawk on the vehicle; then change your Garage sort (Settings or the Garage's Sort menu) | Both saved and shown. **↑ 3 to send**. Diagnostics → Offline copy: **Waiting to send 3**, **Not saved 0**, **Oldest waiting** (UTC). |
| [x] | 7 | Offline: open a document or photo strip: **+ Add**; open a photo: **Remove**; account menu → **Change password** | **+ Add** and **Remove** dimmed, with the tooltip "This needs the server, which can't be reached (offline)."; the password dialog's save is dimmed, with "Saving needs the server (offline)." |
| [x] | 8 | **Tailscale on.** Wait up to a minute (or Diagnostics → **Sync now**) | The OFFLINE tag and the chip go. **Phone** (reload): the fuel-up, the squawk, and your Garage sort are there. Diagnostics: Waiting to send 0. |
| [x] | 9 | **The lower-reading conflict:** **Tailscale off.** Desktop: log a reading of **R + 25** (dated now). **Phone:** log a reading of **R + 40** dated **30 minutes earlier** | Desktop: saved, **↑ 1 to send**. Phone: saved. |
| [x] | 10 | **Tailscale on**, wait (or Sync now) | The chip goes. **Messages** (desktop): an amber **"⚠ Not saved: Reading on (vehicle)"** card at the top, with the reason (a reading out of order), "Made on this computer, (date time).", **Open**, **Discard**. The Messages badge counts it. The desktop's timeline no longer shows R + 25 (the copy downloaded again). |
| [x] | 11 | Messages → **Open**. In the form, change the reading to **R + 45**, **Save** | **Open** shows the log form filled in with R + 25 and its date. After Save: saved; the "Not saved" card is gone. **Phone:** R + 45 is there. |
| [x] | 12 | Repeat steps 9–10 (with R + 50 and R + 60), then **Discard** | The card goes; the change is gone for good (not on the phone). |
| [x] | 13 | **Two devices, one event:** desktop online, log a fuel-up and leave it a **draft** (don't finalize). **Tailscale off.** Desktop: edit its **notes**. **Phone:** edit the same draft's **cost** | Both save. |
| [x] | 14 | **Tailscale on**, wait | **Phone** (reload) and **desktop**: the draft has **both** the desktop's notes and the phone's cost. |
| [x] | 15 | **Squawk offline:** **Tailscale off.** On a vehicle's Squawks tab, **+ Squawk**, save one; then **Edit** it and change its severity | Both work offline. **Tailscale on**, wait: the phone shows the squawk with the new severity. |
| [x] | 16 | **Sign out with changes waiting:** **Tailscale off.** Log a reading (R + 70). Account menu → **Sign out** | A dialog: **"1 change hasn't been sent"**, the change named ("Reading on (vehicle)"), "Signing out deletes this computer's copy, and these changes with it.", **Send now** (dimmed offline), **Cancel**, **Sign out anyway**. **Cancel**: still signed in. |
| [x] | 17 | **Tailscale on** (don't wait for the chip). Account menu → **Sign out** → **Send now** | It sends, then signs out (the sign-in card). **Phone:** R + 70 is there. |
| [x] | 18 | Sign in again on the desktop, choose a PIN | A new copy downloads (Diagnostics: Up to date, Waiting to send 0). |
| [ ] | 19 | *(Optional)* **A clock ahead:** Mac System Settings → General → Date & Time: automatic off, set the time **1 hour ahead**. Desktop (online): log a reading | Saved here; the chip stays (**↑ 1 to send**); Messages (desktop): **"⚠ Changes can't be sent"**: "This computer's clock is 60 minutes ahead…". **Phone in Admin mode**, Messages: **"A computer's clock is wrong"** naming the Mac. |
| [ ] | 20 | *(If 19 was run)* Set the clock back to automatic. Wait (or Sync now) | The change goes; the desktop's notice and the Admin's message go. |
| [x] | 21 | **Mac browser:** the Garage, a vehicle, Messages | As before: no chip, no "Sync not set up", no "Not saved" cards. |
| [x] | 22 | Afterwards: **leave the test server on `:milestone-p3-m5-outbox`** until this milestone merges, then move it to `:dev` | Its database is now at schema 19; today's `:dev` (schema 18) would refuse to open it. |

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer with **Area: Sync** (Minimum level: Debug): "saved on this computer", "push", "the server refused a change", "the server stopped the push", "the copy's records were cleared to download again", "no push: can't reach the server".
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 | grep -iE "push|refused|clock"`.
- **Diagnostics → Offline copy:** Status, Waiting to send, Not saved, Oldest waiting, Sending.
- **Never share** a PIN, a password, a token, `.env`, a cookie, `device-credentials.json`, or the copy's file.

# Human Notes

## 5. Result (2026-11-12)

Passed on the test server: steps 1–18, 21, and 22. The optional clock-ahead steps (19–20) weren't run; the server's tests cover them (`crates/server/tests/sync.rs`: a clock an hour ahead stops the push, flags the device with the Admin's message, and clears once the clock is right).

