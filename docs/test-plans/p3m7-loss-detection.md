# Manual test plan: Phase 3 milestone 7, loss detection (and the rest of milestone 6)

*Phase 3 milestone 7 · branch `milestone/p3-m7-loss-detection` · `docs/phase_plans/phase-3.md` · decision #078 · sketch `docs/ui-sketches/p3m7-loss-detection-options.svg` (1A, 2A, 3B)*

Milestone 6's plan (`docs/test-plans/p3m6-files.md`) got through its step 11 with this milestone's desktop build against milestone 6's server, which refused the push ("another schema version": the desktop is at 21, that server at 20). This build includes milestone 6, so **part A finishes milestone 6's plan here** (its steps 12–23), carrying on with the same copy and the changes it still has waiting. **Part B** is milestone 7.

## 1. Intended behavior

**Milestone 6 (files), unchanged by milestone 7:**
- Photos and PDFs are kept in `files.sqlite` beside the copy (encrypted, same key). Thumbnails and PDFs download by themselves; a photo downloads when it's opened, then it's kept.
- Adding, removing, and moving photos and documents work offline. Each waits with a blue **↑** and is uploaded right before the change that uses it is sent.
- **Keep photos up to** (Diagnostics; default 2 GB): past it, the photos opened longest ago go.
- The server keeps an audit record of each erased file.

**Milestone 7 (loss detection):**
- **The desktop's copy is the household's backup:** a damaged, reset, or restored server never damages it.
- **Protective mode** (a red **PROTECTED** tag and strip, 1A) starts when a pulled page comes from **another server** (the database changed), or from one **gone back in time** (restored from an older snapshot). It also starts when the server **no longer knows this computer**, or when a pull would **delete a lot**: more than 25 records and more than 10% of the copy, with a deleted vehicle counting its whole history. While it's on, nothing pulled is applied and nothing is pushed, but reading and saving here go on.
- **A big delete** asks in a dialog (2A): **Keep this computer's copy** or **Apply them** (a snapshot first).
- **The choices** (Details): Keep protected; **Use this server** (a snapshot first); **Rebuild the server from this computer** (the household Admin, onto a fresh server, step by step, 3B).
- **Signed out by the server:** the copy and its unsent changes are kept; signing in again carries on with it. **Signing out yourself** still deletes the copy, its files, and its snapshots.
- **Snapshots of the copy** (encrypted): one a day, one before every risky step, and one on request (Diagnostics), with Restore.
- **An unhealthy server** takes and serves nothing; the desktop waits ("The server isn't well").
- **The web app is unchanged.**

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m7-loss-detection` finished.
- **The test server** (`/volume1/docker/vehicle_manager_test`, container `vehiclemanager-test`): in its `.env`, set `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m7-loss-detection` and **`VEHICLEMANAGER_TEST_MINUTES=2`** (snapshots every 2 minutes; files erased 8 intervals after removal). Then `sudo docker compose pull && sudo docker compose up -d`.
- **The desktop app** from this branch (`cd crates/app && dx serve --platform desktop`), signed in **as the household's Admin**, with a PIN. **Keep its current copy**: don't move or delete the `-test` folder. Its waiting changes are part A's first step.
- **Offline** is **Tailscale off on the Mac**; the web side is **your phone** (on Tailscale).
- A **spare test vehicle** with **more than 25 records** (events, meters, schedule items, documents…) that you're happy to delete (part B). If none has that many, log quick readings on one on the web until it does.
- A **maintenance draft with two tasks**, with a photo on the second task (steps 4–5), or make one on the desktop at step 4.

## 3. Checklist

### Part A: the rest of milestone 6 (its steps 12–23)

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m7-loss-detection · …`; `database open … schema_before=20 schema_now=21`. |
| [x] | 2 | *(M6 step 12)* **Desktop:** Tailscale on; Diagnostics → **Sync now** | No "another schema version". The waiting changes are sent: the **↑** badges and the **to send** chip go; no PROTECTED tag. **Phone** (reload): the new photo and PDF are on the event, and the removed photo is gone. Open the new photo on the phone: full size. |
| [x] | 3 | *(M6 13)* **Desktop:** open the new photo again | It still opens (kept here). |
| [x] | 4 | *(M6 14)* **Moving with a task:** on the maintenance draft, remove the **first** task and save (online) | The photo moves with its task (now the first). **Phone:** the same. |
| [x] | 5 | *(M6 15)* Repeat 4 **offline** (Tailscale off; make the draft again if needed), then Tailscale on and wait | The same on the desktop at once, and on the phone after it's sent. |
| [x] | 6 | *(M6 16)* **The limit:** Diagnostics → **Keep photos up to** → **1 GB**; note **Photos kept** | Saved (the choice stays after leaving and coming back). Photos kept is within 1 GB. |
| [x] | 7 | *(M6 17; skip unless you have more than 1 GB of photos)* open photos until past 1 GB | Photos kept stays at or under 1 GB; the first ones opened download again when reopened (online). |
| [x] | 8 | *(M6 18)* Set **Keep photos up to** back to **2 GB** | Saved. |
| [x] | 9 | *(M6 20)* At least **20 minutes after step 2** (8 test intervals, then the next file check). **NAS:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 \| grep "job=file-check"` | A `job finished` line with `checked … files; erased 2 unused` or more (the removed photo and its thumbnail). Part B can go on meanwhile; come back to this. |
| [x] | 10 | *(M6 21)* **NAS:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 \| grep -iE "erased file\|file check"` | No "can't record an erased file" or "can't find what an erased file belonged to". |
| [x] | 11 | *(M6 19)* **Sign out** (account menu; nothing waiting). Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/` | The `-test` folder is gone, with `files.sqlite` and `snapshots/`. Sign in again and choose a PIN: a new copy, and thumbnails download again by themselves. Then **open two or three photos full size**, so part B's rebuild has some photos to restore. |

### Part B: loss detection

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 12 | **Desktop:** Diagnostics → **Snapshots of this copy**; **Take one now** | Take one now adds a snapshot at the top (today's date, UTC). |
| [x] | 13 | Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/` then `head -c 15 ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/$(ls ~/Library/Application\ Support/vehiclemanager/servers/*-test/snapshots/ \| tail -1); echo` | Files named `snapshot-…Z.sqlite`; the 15 bytes are gibberish, **not** `SQLite format 3`. |
| [x] | 14 | **A big delete:** Mac browser, Admin mode: delete the spare vehicle (typing its name). **Desktop:** wait a minute (or Diagnostics → Sync now) | A dialog: **"The server would delete N records"** with the vehicle and its count (2A), **Keep this computer's copy** and **Apply them**. The red **PROTECTED** tag and strip. The desktop's Garage still has the vehicle. |
| [x] | 15 | **Keep this computer's copy** | The dialog closes; the strip stays. **Messages:** a red "Protecting this computer's copy" card. |
| [x] | 16 | Strip → **Details** → **Apply them** | The vehicle goes from the desktop; the tag and strip go. Diagnostics: one more snapshot. |
| [x] | 17 | **Gone back in time:** **NAS:** `ls -lt data/snapshots/ \| head -3` in the test server's folder, and note the newest snapshot's name (wait for one taken **after** step 16). **Mac browser:** log two readings on a vehicle. **Desktop:** Sync now | The two readings show on the desktop. |
| [x] | 18 | **NAS:** restore the noted snapshot (docs/deploying-nas.md "Restoring a snapshot", in `vehicle_manager_test`, container `vehiclemanager-test`). **Desktop:** Sync now | **PROTECTED**: "the server went back in time". The desktop still shows the two readings (the server no longer has them). |
| [x] | 19 | While protected, on the desktop: log a reading | Saved here; **↑ 1 to send**; it isn't sent (nothing is pushed while protected). |
| [x] | 20 | Details → **Use this server** | A snapshot, then the copy downloads again: the two readings from step 17 go (the server lost them); the reading from step 19 is sent (**Mac browser:** it's there). The tag goes. |
| [x] | 21 | **The server forgets this computer:** Mac browser, Admin mode: Users → you → Devices → sign the Mac out. **Desktop:** wait a minute (or quit and start, then unlock) | The sign-in card, with "The server no longer knows this computer. Its copy and 0 unsent changes are kept: sign in again to carry on, or delete the copy" and **Delete this computer's copy**. Terminal: the `-test` folder is still there. |
| [x] | 22 | Sign in again on the desktop (same account), choose a PIN | The app opens with the same copy (no long download; Diagnostics → Records as before); the tag and strip go. |
| [x] | 23 | **A fresh server:** **NAS** (in the test server's folder): `sudo docker compose stop`, `sudo mkdir -p data/before-rebuild && sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/before-rebuild/` (a missing `-wal` or `-shm` is fine), `sudo docker compose start`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1 \| grep -i "setup code"` | A new setup code in the log. |
| [x] | 24 | **Desktop:** quit, start, unlock | The setup card, with **"This computer has a copy of the household's records. Rebuild this server from it?"** and **Rebuild the server from this computer**; **PROTECTED** ("the server's database changed"). |
| [x] | 25 | **Rebuild the server from this computer** | Step 1: ✓ complete at its last pull, ✓ nothing waiting or refused, ✓ the server is new; **Next**. Step 2: what happens; **Next**. Step 3: the setup code, your password, and REBUILD typed (Rebuild stays off until all three are in). |
| [x] | 26 | **Rebuild** | "Rebuilding…", then **"The server is rebuilt."** with each other test person's temporary password (name, username, password), and the photos left out: those this copy never opened (it's new since step 11, so most of them). Then the PIN step: choose a PIN. |
| [x] | 27 | After it: the desktop's Garage, a timeline, a photo you opened in step 11 | All there; no PROTECTED tag. Diagnostics → Offline copy: a new folder (the rebuilt server's database ID). |
| [x] | 28 | **Mac browser:** sign in with **your password**; the Garage, a timeline, photos and documents | The household's vehicles and events as before. Every thumbnail and PDF is there; the photos from step 11 open full size; the photos left out are gone. |
| [x] | 29 | **Phone** (or a private window): sign in as another test person with their temporary password | Signs in and asks for a new password. |
| [x] | 30 | **A desktop snapshot:** log a reading on the desktop and let it send. Diagnostics → Snapshots: **Restore** the newest snapshot from *before* that reading → confirm | The copy goes back (the reading disappears on the desktop), then the next pull brings it back from the server. Nothing waiting is lost. |
| [x] | 31 | *(M6 22)* **Mac browser:** a vehicle's photos and documents; add and remove a photo; the Garage; Messages | As before: no ↑ badges, no PROTECTED tag, no protective alerts; adding and removing work. |
| [x] | 32 | *(M6 23)* Afterwards: the test server now holds the **rebuilt** database. Keep it, or put back the one in `data/before-rebuild/`: stop, move the rebuilt one aside, move those back, start. The desktop then says "the server's database changed", and **Use this server** follows it. Either way, **leave the test server on `:milestone-p3-m7-loss-detection`** until both milestones merge (#48, then this one), then move it to `:dev` and remove `VEHICLEMANAGER_TEST_MINUTES` | Its database is at schema 21. `:milestone-p3-m6-files` and today's `:dev` would refuse to open it. |

**Not tested here: an unhealthy server.** There's no safe way to make the test server's database fail its check. The server's tests cover it: a push to an unhealthy server is refused, and a pull answers "unhealthy". The desktop then shows "Waiting: the server isn't well" on Diagnostics, and a card in Messages.

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer with **Area: Sync** (Minimum level: Debug):
  - files: "the server refused an upload made offline", "no upload", "no push", "couldn't download a file", "dropped the photos opened longest ago";
  - protection: "protecting this computer's copy", "protective mode settled", "snapshotted this computer's copy", "using this server", "rebuilt the server from this computer", "couldn't …".
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 | grep -iE "upload|attachment|file check|rebuild|setup code|push|pull"`.
- **Diagnostics → Offline copy** (Status, Waiting to send, Files, Photos kept) and **Snapshots of this copy**.
- **Never share** a PIN, a password (yours or a temporary one), the setup code, a token, `.env`, a cookie, `device-credentials.json`, the copy's or a snapshot's file, or `files.sqlite`.

# Human Notes
