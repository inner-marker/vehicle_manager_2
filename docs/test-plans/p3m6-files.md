# Manual test plan: Phase 3 milestone 6, files

*Phase 3 milestone 6 · branch `milestone/p3-m6-files` · `docs/phase_plans/phase-3.md` · decision #077 · sketch `docs/ui-sketches/p3m6-files-options.svg` (1A, 2A, 3B)*

## 1. Intended behavior

- **The desktop keeps photos and PDFs** in a second encrypted database, `files.sqlite`, beside its copy (`servers/<database ID>-test/`), under the same key. It goes with the copy at sign-out.
- **Thumbnails and PDFs download by themselves** after each pull; a **photo when it's opened**, then it's kept. **Originals** only on request (the Original button), never kept.
- **Offline:** what's here shows; a photo whose full size isn't here opens as its **thumbnail, enlarged**, with "Full size downloads when this computer can reach the server (a small preview)." (1A).
- **Adding, removing, and moving** photos and documents work offline. A photo added offline shows at once with a small blue **↑** on its tile (2A); it's uploaded right before the change that uses it, and the ↑ goes. **Keep the original** is off offline.
- **The limit:** Diagnostics → Offline copy → **Keep photos up to** (default 2 GB; 3B). Past it, the photos opened longest ago go (and download again when opened). Thumbnails, PDFs, and anything not yet sent never go.
- **The server keeps an audit record** of each erased file (who removed it, when, and when it was erased), once the 8 days after removal pass (#065); with test timers, 8 test intervals.
- **The web app is unchanged.**

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m6-files` finished.
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m6-files` in its `.env`. For the optional erasure steps (20–21), also `VEHICLEMANAGER_TEST_MINUTES=2`. Then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- **The desktop app** from this branch: `cd crates/app && dx serve --platform desktop`, signed in with a PIN.
- **Offline** is **Tailscale off on the Mac**; the web side is **your phone** (on Tailscale).
- A vehicle with a few photos (on the vehicle or its events) and a PDF document (from earlier tests). Note one photo you **haven't opened on the desktop** yet: **P**.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m6-files · …`; `database open … schema_now=20` (`schema_before=19` on the first start). |
| [x] | 2 | **Desktop:** start it, unlock, wait a minute. Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/*-test/` | `files.sqlite` beside `vehiclemanager.sqlite`. Diagnostics → Offline copy: **Files** (a count and a size), **Photos kept** "0.0 MB of 2 GB" (or a little), **Keep photos up to** 2 GB. |
| [x] | 3 | **Encrypted:** `head -c 15 ~/Library/Application\ Support/vehiclemanager/servers/*-test/files.sqlite; echo` | Gibberish, **not** `SQLite format 3`. |
| [x] | 4 | **Desktop:** open a vehicle's photos, then **open one photo** (not P) full size, and open the **PDF** | They show as before. |
| [x] | 5 | **Offline:** **Tailscale off**; wait for the OFFLINE tag | The OFFLINE tag. |
| [x] | 6 | Offline: the same vehicle's photos and documents; the photo from step 4; the PDF | Every thumbnail shows. The photo from step 4 opens full size. The PDF opens (it downloaded by itself). |
| [x] | 7 | Offline: open **P** | Its thumbnail, enlarged, with "Full size downloads when this computer can reach the server (a small preview)." (1A). |
| [x] | 8 | Offline: on an event of yours, **+ Add** a photo (Choose photos) | **Keep the original** is dimmed (tooltip: "This needs the server, which can't be reached (offline).") if you may keep originals. The photo shows at once, with a small blue **↑** on its tile (hover: "Waiting to upload (offline)."). The top bar: **↑ 1 to send**. |
| [x] | 9 | Offline: open that photo | It opens full size (it's here). |
| [x] | 10 | Offline: add a **PDF** to the same event | It shows, with its first page as the thumbnail and the ↑. **↑ 2 to send**. |
| [x] | 11 | Offline: **Remove** an older photo (one that was on the server) | Gone here. **↑ 3 to send**. |
| [ ] | 12 | **Tailscale on**; wait (or Diagnostics → Sync now) | The chip goes; the **↑** badges go. **Phone** (reload): the new photo and PDF are on the event; the removed photo is gone. Open the new photo on the phone: full size. |
| [ ] | 13 | **Desktop:** open the new photo again | It still opens (kept here). |
| [ ] | 14 | **Moving with a task:** on a maintenance draft with two tasks and a photo on the second task, remove the **first** task and save (online) | The photo moves with its task (now the first). **Phone:** the same. |
| [ ] | 15 | Repeat 14 **offline** (Tailscale off), then Tailscale on and wait | The same on the desktop at once, and on the phone after it's sent. |
| [ ] | 16 | **The limit:** Diagnostics → **Keep photos up to** → **1 GB**; note **Photos kept** | Saved (the choice stays after leaving and coming back). Photos kept is within 1 GB. |
| [ ] | 17 | *(If you have more than 1 GB of photos to open, otherwise skip)* open photos until past 1 GB | Photos kept stays at or under 1 GB; the first ones opened download again when reopened (online). |
| [ ] | 18 | Set **Keep photos up to** back to **2 GB** | Saved. |
| [ ] | 19 | **Sign out** (account menu; nothing waiting). Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/` | The `-test` folder is gone (with `files.sqlite`). Sign in again, choose a PIN: a new copy, and thumbnails download again by themselves. |
| [ ] | 20 | *(Optional, with `VEHICLEMANAGER_TEST_MINUTES=2`)* After step 11's removal, wait about **20 minutes** (8 test intervals and the next file check). **NAS:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 \| grep "job=file-check"` | A `job finished` line with `checked … files; erased 2 unused` (or more: the removed photo and its thumbnail). |
| [ ] | 21 | *(If 20 was run)* **NAS:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 \| grep -iE "erased file\|file check"` | No "can't record an erased file" or "can't find what an erased file belonged to". |
| [ ] | 22 | **Mac browser:** a vehicle's photos and documents; add and remove a photo | As before: no ↑ badges; adding and removing work. |
| [ ] | 23 | Afterwards: **leave the test server on `:milestone-p3-m6-files`** until this milestone merges, then move it to `:dev` (and remove `VEHICLEMANAGER_TEST_MINUTES` if you added it) | Its database is now at schema 20; today's `:dev` (schema 19) would refuse to open it. |

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer with **Area: Sync** (Minimum level: Debug): "this computer's files are open", "downloaded thumbnails and PDFs", "couldn't download a file", "the server refused an upload made offline", "dropped the photos opened longest ago".
- **Diagnostics → Offline copy:** Status, Waiting to send, Files, Photos kept.
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 | grep -iE "upload|attachment|file check"`.
- **Never share** a PIN, a password, a token, `.env`, a cookie, `device-credentials.json`, the copy's file, or `files.sqlite`.

# Human Notes

- After turning tailscale back on, the "offline" tag goes away, but the "2 to send" never does even after clicking "Sync now"