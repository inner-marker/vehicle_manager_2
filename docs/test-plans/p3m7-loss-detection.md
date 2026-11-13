# Manual test plan: Phase 3 milestone 7, loss detection

*Phase 3 milestone 7 · branch `milestone/p3-m7-loss-detection` · `docs/phase_plans/phase-3.md` · decision #078 · sketch `docs/ui-sketches/p3m7-loss-detection-options.svg` (1A, 2A, 3B)*

## 1. Intended behavior

- **The desktop's copy is the household's backup:** a damaged, reset, or restored server never damages it.
- **Protective mode** (a red **PROTECTED** tag and strip, 1A): when a pulled page comes from **another server** (the database changed), from one **gone back in time** (restored from an older snapshot), when the server **no longer knows this computer**, or when a pull would **delete a lot** (more than 25 records and more than 10% of the copy; a vehicle deleted counts with its whole history). Nothing pulled is applied, nothing is pushed; reading and saving here go on.
- **A big delete** asks in a dialog (2A): **Keep this computer's copy** or **Apply them** (a snapshot first).
- **The choices** (Details): Keep protected; **Use this server** (a snapshot first); **Rebuild the server from this computer** (the household Admin, onto a fresh server, step by step, 3B).
- **Signed out by the server:** the copy and its unsent changes are kept; signing in again carries on with it.
- **Snapshots of the copy** (encrypted): one a day, before every risky step, and on request (Diagnostics), with Restore.
- **An unhealthy server** takes and serves nothing; the desktop waits ("The server isn't well").
- **The web app is unchanged.**

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m7-loss-detection` finished.
- **The test server** (`/volume1/docker/vehicle_manager_test`, container `vehiclemanager-test`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m7-loss-detection` and **`VEHICLEMANAGER_TEST_MINUTES=2`** (snapshots every 2 minutes) in its `.env`, then `sudo docker compose pull && sudo docker compose up -d`.
- **The desktop app** from this branch (`cd crates/app && dx serve --platform desktop`), signed in **as the household's Admin**, with a PIN.
- A **spare test vehicle** with **more than 25 records** (events, meters, schedule items, documents…) that you're happy to delete. If none has that many, log quick readings on one on the web until it does.
- Mac Terminal, for the copy's folder: `~/Library/Application\ Support/vehiclemanager/servers/`.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m7-loss-detection · …`; `database open … schema_now=21` (`schema_before=20` on the first start). |
| [ ] | 2 | **Desktop:** unlock; Diagnostics → **Snapshots of this copy**; **Take one now** | At least one snapshot (today's, UTC); Take one now adds one at the top. |
| [ ] | 3 | Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/` then `head -c 15 ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/$(ls ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/ \| tail -1); echo` | Files named `snapshot-…Z.sqlite`; the 15 bytes are gibberish, **not** `SQLite format 3`. |
| [ ] | 4 | **A big delete:** Mac browser, Admin mode: delete the spare vehicle (typing its name). **Desktop:** wait a minute (or Diagnostics → Sync now) | A dialog: **"The server would delete N records"** with the vehicle and its count (2A), **Keep this computer's copy** and **Apply them**. The red **PROTECTED** tag and strip. The desktop's Garage still has the vehicle. |
| [ ] | 5 | **Keep this computer's copy** | The dialog closes; the strip stays. **Messages:** a red "Protecting this computer's copy" card. |
| [ ] | 6 | Strip → **Details** → **Apply them** | The vehicle goes from the desktop; the tag and strip go. Diagnostics: one more snapshot. |
| [ ] | 7 | **Gone back in time:** **NAS:** `ls -lt data/snapshots/ \| head -3` in the test server's folder, and note the newest snapshot's name (wait for one taken **after** step 6). **Mac browser:** log two readings on a vehicle. **Desktop:** Sync now | The two readings show on the desktop. |
| [ ] | 8 | **NAS:** restore the noted snapshot (docs/deploying-nas.md "Restoring a snapshot", in `vehicle_manager_test`, container `vehiclemanager-test`). **Desktop:** Sync now | **PROTECTED**: "the server went back in time". The desktop still shows the two readings (the server no longer has them). |
| [ ] | 9 | While protected, on the desktop: log a reading | Saved here; **↑ 1 to send**; it isn't sent (nothing is pushed while protected). |
| [ ] | 10 | Details → **Use this server** | A snapshot, then the copy downloads again: the two readings from step 7 go (the server lost them); the reading from step 9 is sent (**Mac browser:** it's there). The tag goes. |
| [ ] | 11 | **The server forgets this computer:** Mac browser, Admin mode: Users → you → Devices → sign the Mac out. **Desktop:** wait a minute (or quit and start, then unlock) | The sign-in card, with "The server no longer knows this computer. Its copy and 0 unsent changes are kept: sign in again to carry on, or delete the copy" and **Delete this computer's copy**. Terminal: the `-test` folder is still there. |
| [ ] | 12 | Sign in again on the desktop (same account), choose a PIN | The app opens with the same copy (no long download; Diagnostics → Records as before); the tag and strip go. |
| [ ] | 13 | **A fresh server:** **NAS** (in the test server's folder): `sudo docker compose stop`, `sudo mkdir -p data/before-rebuild && sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/before-rebuild/` (a missing `-wal` or `-shm` is fine), `sudo docker compose start`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1 \| grep -i "setup code"` | A new setup code in the log. |
| [ ] | 14 | **Desktop:** quit, start, unlock | The setup card, with **"This computer has a copy of the household's records. Rebuild this server from it?"** and **Rebuild the server from this computer**; **PROTECTED** ("the server's database changed"). |
| [ ] | 15 | **Rebuild the server from this computer** | Step 1: ✓ complete at its last pull, ✓ nothing waiting or refused, ✓ the server is new; **Next**. Step 2: what happens; **Next**. Step 3: the setup code, your password, and REBUILD typed (Rebuild stays off until all three are in). |
| [ ] | 16 | **Rebuild** | "Rebuilding…", then **"The server is rebuilt."** with each other test person's temporary password (name, username, password), and any photos left out (ones this Mac never opened). Then the PIN step: choose a PIN. |
| [ ] | 17 | After it: the desktop's Garage, a timeline, a photo you'd opened before | All there; no PROTECTED tag. Diagnostics → Offline copy: a new folder (the rebuilt server's database ID). |
| [ ] | 18 | **Mac browser:** sign in with **your password**; the Garage, a timeline, photos | The household's vehicles and events as before; photos this Mac had are there; ones it never opened are gone. |
| [ ] | 19 | **Phone** (or a private window): sign in as another test person with their temporary password | Signs in and asks for a new password. |
| [ ] | 20 | **A desktop snapshot:** log a reading on the desktop and let it send. Diagnostics → Snapshots: **Restore** the newest snapshot from *before* that reading → confirm | The copy goes back (the reading disappears on the desktop), then the next pull brings it back from the server. Nothing waiting is lost. |
| [ ] | 21 | **Mac browser:** the Garage, Messages | As before: no PROTECTED tag, no protective alerts. |
| [ ] | 22 | Afterwards: the test server now holds the **rebuilt** database. Keep it, or put back the one in `data/before-rebuild/` (stop, move the rebuilt one aside, move those back, start; the desktop then says "the server's database changed" and **Use this server** follows it). Either way **leave the test server on `:milestone-p3-m7-loss-detection`** until this milestone merges, then `:dev`; remove `VEHICLEMANAGER_TEST_MINUTES` if you added it | Its database is at schema 21; today's `:dev` (schema 20 or lower) would refuse to open it. |

**Not tested here: an unhealthy server.** There's no safe way to make the test server's database fail its check; the server's tests cover it (a push to an unhealthy server is refused; a pull answers "unhealthy"), and the desktop shows "Waiting: the server isn't well" on Diagnostics and a card in Messages.

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer with **Area: Sync** (Minimum level: Debug): "protecting this computer's copy", "protective mode settled", "snapshotted this computer's copy", "using this server", "rebuilt the server from this computer", "couldn't …".
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 | grep -iE "rebuild|setup code|push|pull"`.
- **Diagnostics → Offline copy** and **Snapshots of this copy**.
- **Never share** a PIN, a password (yours or a temporary one), the setup code, a token, `.env`, a cookie, `device-credentials.json`, the copy's or a snapshot's file, or `files.sqlite`.

# Human Notes
