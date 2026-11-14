# Deploying the server on the NAS

*A Synology NAS (x86_64), DSM 7.4, Container Manager and Tailscale installed (decisions.md #025). Phase 1 milestone 3.*

The server runs as a Docker container from the image `ghcr.io/inner-marker/vehicle_manager_2`. Container Manager runs it as a **Project** defined by `deploy/compose.yaml`.

**Two servers run side by side** (decision #062), each its own Project with its own data:

| | Main (the household's) | Test |
|---|---|---|
| Project and container | `vehiclemanager` | `vehiclemanager-test` |
| Folder | `/volume1/docker/vehicle_manager` | `/volume1/docker/vehicle_manager_test` |
| Port on the NAS | 8410 | 8411 |
| Address | `https://<nas-name>.<tailnet>.ts.net` | `https://<nas-name>.<tailnet>.ts.net:8443` |
| Image | A release, e.g. `0.2.0` (decision #070) | `milestone-<name>`, or `dev` |
| In `.env` | `VEHICLEMANAGER_PROJECT_NAME=vehiclemanager`, `VEHICLEMANAGER_INSTANCE=` | `VEHICLEMANAGER_PROJECT_NAME=vehiclemanager-test`, `VEHICLEMANAGER_INSTANCE=test` |

- **The test server says so** with a **TEST SERVER** tag (just **TEST** on a phone) and its browser tab's title.
- **Each server has its own sign-in:** you can be signed in to both in one browser.
- **Test plans run on the test server.** Commands in this guide use the main server's folder and container (`vehiclemanager`); for the test server, use `vehicle_manager_test` and `vehiclemanager-test` instead.
- **Moving from one server to two:** see "Splitting into a main server and a test server" below.

- **Access:** it listens only on the NAS's loopback. **Tailscale is the only way in**, over HTTPS with `tailscale serve` (plan §6, §7).
- **Data:** everything that must survive updates lives in one folder on the NAS.

```
/volume1/docker/vehicle_manager/   Project folder
├── compose.yaml                   copy of deploy/compose.yaml
├── .env                           settings (from deploy/.env.example); NAS only
└── data/                          the container's /data
    ├── vehiclemanager.sqlite      the database (plus -wal and -shm files while it runs)
    ├── files/                     photos and PDFs, each named by its SHA-256 (decision #065)
    ├── backups/                   copies made before each database upgrade
    ├── snapshots/                 nightly snapshots, newest 7
    └── logs/                      log files and last-crash.txt
```

> **Never put `data/` on a network share** (NFS/SMB). It must be a local NAS volume (decision #002).
> **Never copy the live database file while the server runs;** use a snapshot (see "Database, backups, and snapshots").

## Image tags

| Tag | What it is | Use it for |
|---|---|---|
| `0.2.0` (each release's `X.Y.Z`) | A release, from the release workflow, **without** test timers | **The main server** (decision #070): it moves to a new release only when its `.env` says so |
| `dev` | Newest build of the `dev` branch, **without** test timers | The test server: a regression pass before a release, or what's merged since |
| `milestone-<name>` (e.g. `milestone-p2-m9-lifecycle`) | Newest build of a milestone branch, **with** test timers | The test server: testing a milestone before its PR |
| `sha-<commit>` | One exact build | Pinning, or rolling back to a known-good build |
| `latest` | The newest release | Not used on the NAS, so an update is always a deliberate change of tag |

## One-time setup

### 1. Folder and user (DSM)
- **The DSM user `vehiclemanager`** exists (UID 1030, group `users` 100; `id vehiclemanager` over SSH shows both).
- **The folders:** in File Station, `docker/vehicle_manager` exists, and so does `docker/vehicle_manager/data` inside it.
- **Permissions:** right-click `data` → Properties → Permission → give **`vehiclemanager`** **Read/Write**, and tick *Apply to this folder, sub-folders and files*.

### 2. Make the image public (GitHub, once)
GHCR packages start private, even for a public repo. After the first image is published (Actions → "Server image" workflow; it runs when code or deploy files change on `dev` or a milestone branch):
- On GitHub, go to your profile → **Packages** → `vehicle_manager_2` → **Package settings** → *Change visibility* → **Public**.
- That's it: the NAS can then pull without a login.

### 3. Put `compose.yaml` and `.env` in the Project folder
Enable SSH if needed: Control Panel → Terminal & SNMP → *Enable SSH service*. Then:

```sh
ssh <your-admin-user>@<nas-lan-ip>
cd /volume1/docker/vehicle_manager
curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/dev/deploy/compose.yaml
curl -fsSLo .env https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/dev/deploy/.env.example
chmod 600 .env
vi .env
```

In `.env`, check `VEHICLEMANAGER_UID=1030`, `VEHICLEMANAGER_GID=100` and `VEHICLEMANAGER_HOST_PORT=8410`. Use `nano` instead of `vi` if it's installed. You can also make the two files on your Mac and upload them with File Station; files starting with `.` upload fine.

### 4. Create the Project (Container Manager)
- Go to Container Manager → **Project** → **Create**.
  - **Project name:** `vehiclemanager` (must match the `name:` line in `compose.yaml`)
  - **Path:** `/docker/vehicle_manager`
  - **Source:** *Use existing docker-compose.yml*. It should find `compose.yaml`. If your Container Manager version only looks for `docker-compose.yml`, rename the file to that.
- Skip the web portal (Web Station) option.
- **Finish.** It pulls the image and starts the container.

**Check:** Container → `vehiclemanager` shows **Running**, and its **Log** tab includes `app starting` and `writing log files` lines. `data/logs/current.jsonl` appears in File Station.

### 5. Publish it on the tailnet (`tailscale serve`)
Over SSH:

```sh
sudo /var/packages/Tailscale/target/bin/tailscale serve --bg --https=443 http://127.0.0.1:8410
sudo /var/packages/Tailscale/target/bin/tailscale serve status
```

`status` shows the HTTPS address, `https://<nas-name>.<tailnet>.ts.net`. That's the app's address from any device on your tailnet. `--bg` keeps it running across restarts of the Tailscale service.

**If 443 is refused**, because something on the NAS already serves it, reset and use another port, e.g. 8443:

```sh
sudo /var/packages/Tailscale/target/bin/tailscale serve reset
sudo /var/packages/Tailscale/target/bin/tailscale serve --bg --https=8443 http://127.0.0.1:8410
```

The address is then `https://<nas-name>.<tailnet>.ts.net:8443`.

**`tailscale serve` is the one proxy in front of the server,** which is what the server assumes (`VEHICLEMANAGER_PROXY_HOPS` empty, meaning 1; decision #071). It adds each visitor's tailnet address to the request, and the sign-in limit counts failures by that address. Set `VEHICLEMANAGER_PROXY_HOPS=2` only if another proxy (e.g. Caddy) also sits in front; a wrong number makes the limit see the wrong addresses.

### 6. Create the first Admin

A new server has no users. Until it has one, it writes a one-time **setup code** to its log at every start (decision #028). The setup and recovery codes go **only to the container's log** (`docker logs`), never to the files in `data/logs/`, so log files you share for debugging never contain them:

```sh
sudo docker logs vehiclemanager 2>&1 | grep -i "setup code"
```

The line ends with `setup_code=K7QM-4XRT-9PWD-H2CN` (yours differs). Open the app's address from step 5; it shows **first-time setup**. Enter the code, your username, name, and a password of at least 12 characters. You're signed in as the household's **Admin**, and the code stops working.

- **Only someone who can read the server's log can do this,** so another device on the tailnet can't claim Admin first.
- Add the rest of the household afterwards, in Admin mode.
- **Forgot your password?** Anyone else asks an Admin, who resets it from **Users** (they get a temporary password). **An Admin** recovers it from the sign-in screen (decision #047):
  1. **Forgot your password?** → enter your username → **Continue**. Everyone sees "Ask your Admin for further instructions."
  2. For an Admin account only, the server writes a one-time code to its log. Over SSH:

     ```sh
     sudo docker logs vehiclemanager 2>&1 | grep -i "recovery code"
     ```

  3. Enter the code (the newest line; it works once, for 15 minutes) and a new password. You're signed in, and your other sessions end.

## Test timers (the test server's milestone images only)

To run a test plan in minutes instead of hours, set `VEHICLEMANAGER_TEST_MINUTES` in the **test server's** `.env` to a number from 1 to 5 and run `sudo docker compose up -d` (decision #054). Admin mode, Sign out when idle, the nightly snapshot, the hourly check, and recovery codes then use minutes; the app shows a **TEST TIMERS** tag in the top bar. Empty it again (and `up -d`) when you're done. `dev` and release images ignore it, with a warning in the log, so the main server can't run them.

## Updating

Over SSH:

```sh
cd /volume1/docker/vehicle_manager
sudo docker compose pull
sudo docker compose up -d
```

This pulls the newest image for the tag in `.env` and recreates the container. `data/` is untouched. In Container Manager, the Project's *Action → Build* may do the same; if the new image doesn't show up, use the commands above.

**Moving the main server to a new release** (decision #070), once it's tagged and its Release workflow has published the image:

1. **Copy `data/` first** (the database and `files/` together):

   ```sh
   cd /volume1/docker/vehicle_manager
   sudo docker compose stop
   sudo cp -a data ../vehicle_manager-before-X.Y.Z
   sudo docker compose start
   ```

2. In `.env`, set `VEHICLEMANAGER_IMAGE_TAG=X.Y.Z`.
3. Run the two update commands above, then `sudo docker logs --since 5m vehiclemanager 2>&1`: `database open` with the expected `schema_now`, `database integrity check passed`, and no `ERROR` lines.
4. Once it's been fine for a while, the copy can go: `sudo rm -rf ../vehicle_manager-before-X.Y.Z`.

> **The server always backs the database up before upgrading it** (`data/backups/pre-migration-…`), but that's the database alone; the copy in step 1 keeps `files/` with it, and matters most when an upgrade rebuilds tables (as `v0.2.0`'s migration 0015 does with `events` and `attachments`). The test server gets the same release first, through the regression pass.

## Rolling back

Set `VEHICLEMANAGER_IMAGE_TAG=sha-<commit>` in `.env` to a known-good build (the "Server image" workflow run lists the tags it pushed), then run the two update commands. Set it back to the release (main) or `dev` (test) afterwards.

> **After a database upgrade, an older build won't open the database.** A newer image may upgrade the database's schema (backing it up first, in `data/backups/`). Rolling back to a build from before that upgrade makes the server refuse the database: it keeps running but reports itself unhealthy, and its log says `can't open the database` with "newer than this app". Either go forward again, or restore the matching pre-upgrade backup the same way as a snapshot (below), knowing that anything saved since the upgrade is lost.

## Database, backups, and snapshots

The server keeps its records in `data/vehiclemanager.sqlite` and protects them three ways (photos and PDFs are files in `data/files/`; see below):

| What | When | Where | Kept |
|---|---|---|---|
| **Startup check** | Every start | Container log: `database integrity check passed` | none |
| **Hourly quick check** | Every hour | Container log: `job finished job="quick-check"` | none |
| **Nightly snapshot** | 07:00 UTC (03:00 US Eastern daylight time) | `data/snapshots/snapshot-<UTC time>.sqlite` | Newest 7 |
| **Pre-upgrade backup** | Before an update changes the database's schema | `data/backups/pre-migration-<UTC time>-v<N>.sqlite` | All, until you delete them |

- **If a check fails,** the server keeps serving pages but **refuses every change**, and Diagnostics → Connection shows **Server health: Fail**. The log lists the problems (`database integrity problem`). Restart it after fixing the cause; a problem found by the hourly check lasts until a restart.
- **Each snapshot is checked** after it's written. A snapshot that missed its time (the NAS was off) runs soon after the next start.
- **Snapshots are in the same storage pool** as the database (decision #035). They protect against corruption, a bad upgrade, and mistakes, not against losing the NAS itself; offsite backups come later.
- **Encryption:** the database, snapshots, and files are **not encrypted by the app** yet (that comes in Phase 7, decision #071). To protect them if a disk or the NAS is stolen, keep the Project folder on a **Synology encrypted shared folder** (Control Panel → Shared Folder → Create, with Encrypt this shared folder ticked, and the key kept somewhere other than the NAS). Converting an existing folder means moving its contents into a new encrypted shared folder while the Project is stopped. Set the encrypted folder to mount automatically at startup through Synology's Key Manager, or the server won't start after a reboot until you mount the folder yourself.
- **Checking the last snapshot:** `ls -l data/snapshots/` over SSH shows them, newest last. The container log has a `job finished` line for each (or `job failed` with the reason).
- **Photos and PDFs** are files in `data/files/`, each named by its SHA-256 and never changed once written (decision #065). Snapshots and pre-upgrade backups copy **only the database**: they stay small, and the files they point at are still in `data/files/`. The server keeps a removed attachment's files for **8 days** (a day past the oldest of the 7 snapshots), then erases them, so restoring any kept snapshot finds its files. A **pre-upgrade backup** older than that may miss photos removed since. **Back up `data/files/` along with the database** when you copy `data/` anywhere.
- **The nightly file check** (08:00 UTC, an hour after the snapshot) checks every photo and PDF against its name, then erases the unused ones. A missing or damaged file shows in **Messages** (Admin mode) as "Some photos or documents are missing or damaged", and the log names each one. If a file is missing when the server starts, the check runs within the minute.
- **Place data:** the image carries its own (`/app/geodata/geodata-<version>.gpkg`, decision #059), so nothing needs setting. To use another file (e.g. a newer one), copy it into `data/` and set `VEHICLEMANAGER_GEODATA_FILE=/data/<file>.gpkg` in `.env`. If the file can't be opened, the server still runs: place lookup is off, saved places and typed places still work, and the Admin sees **Place lookup isn't available** in Messages.
- **Another snapshot folder** (e.g. a USB disk, later): mount it in `compose.yaml` and set `VEHICLEMANAGER_SNAPSHOT_DIR` in `.env` to its path inside the container (see the comments in both files). That folder is never created by the server: if the disk isn't mounted, the snapshot is **skipped** and logged, rather than written to the pool.

### Restoring a snapshot

Restoring replaces the database with an older copy: **everything saved after that snapshot is lost.** Over SSH:

1. Stop the server, and list the snapshots:

   ```sh
   cd /volume1/docker/vehicle_manager # or local path
   sudo docker compose stop
   ls data/snapshots/
   ```

2. Move the current database aside (keep it until the restore is confirmed; a missing `-wal` or `-shm` file is fine):

   ```sh
   sudo mkdir -p data/replaced
   sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/replaced/
   ```

3. Copy the chosen snapshot in as the database, owned by the server's user:

   ```sh
   sudo cp data/snapshots/snapshot-20261030T070000Z.sqlite data/vehiclemanager.sqlite
   sudo chown 1030:100 data/vehiclemanager.sqlite
   ```

4. Start the server and check its log:

   ```sh
   sudo docker compose start
   sudo docker logs --since 5m vehiclemanager
   ```

   It should show `database open` with `created=false` and `database integrity check passed`.

The database ID is the same as before (it's inside the snapshot), so devices treat it as the same database. A **desktop** that pulled after the snapshot notices the server **went back in time** and protects its copy (a red PROTECTED tag). The desktop's own copy may have records the server lost; its choices are to keep its copy protected, **Use this server** (its copy downloads again, losing those records, but its unsent changes are kept and sent), or rebuild (below). Once everything looks right, delete `data/replaced/`.

**Leave `data/files/` as it is:** the restored database finds its photos and PDFs there. Files added after the snapshot stay too, unused, and are erased later.

### Rebuilding the server from a desktop

When the server's database is lost and no snapshot or backup will do, the household's records can be rebuilt from a **desktop app's copy** (decision #078). Every desktop keeps a full copy of what its person may see. Only the **household Admin's** copy has everything, so the rebuild runs from that desktop.

**Prefer restoring a snapshot** (above) when one is recent enough: it brings back everything, including the files and every photo's original. A rebuild brings back only what the desktop has:

- **All records**: vehicles, events, readings, schedules, people, and settings, as of the desktop's last pull, plus its unsent changes once they're sent (it must have none waiting to start).
- **Photos and PDFs the desktop has:** every thumbnail and PDF (they download by themselves), but only the photos opened on that desktop. **Any other photo is left out** (its record removed), and the rebuild lists them by name.
- **No originals and no photo metadata files:** desktops never keep them, so they're taken out of each photo.
- **Passwords:** the Admin keeps theirs. **Everyone else gets a temporary password**, shown once, at the end of the rebuild. Note them down, and give each person theirs. Each must choose a new password at their next sign-in.
- **Devices:** every device signs in again. The rebuilt server has a **new database ID**, so other desktops see "the server's database changed".

**What the desktop checks before it starts:** you're the household Admin, its copy was complete at its last pull, nothing is waiting to send or refused, and the server is **fresh** (no people yet: it shows setup).

1. **Give the server a fresh, empty database.** If the old one is still there (damaged), move it aside rather than deleting it. Over SSH:

   ```sh
   cd /volume1/docker/vehicle_manager
   sudo docker compose stop
   sudo mkdir -p data/before-rebuild
   sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/before-rebuild/
   sudo docker compose start
   ```

   A missing `-wal` or `-shm` file is fine. Leave `data/files/` as it is: files the rebuild uploads that are already there are kept once.

2. **Get the setup code** from the log (as for the first Admin):

   ```sh
   sudo docker logs --since 5m vehiclemanager 2>&1 | grep -i "setup code"
   ```

3. **On the Admin's desktop:** start the app and unlock it. It shows the setup card, with **Rebuild the server from this computer**, and the red PROTECTED tag ("the server's database changed"). The same button is in the PROTECTED strip's **Details** and in Diagnostics.
4. **Rebuild the server from this computer:** step 1 shows the checks (each ✓), and step 2 explains what happens. In step 3, enter the **setup code**, **your password**, and type **REBUILD**, then **Rebuild**.
5. **When it says "The server is rebuilt":** copy down each person's **temporary password** and the list of **photos left out**, then choose your PIN. Nothing more is needed on the desktop: it signs in, uploads its files, and settles the rebuild by itself.
6. **Everyone else:** each person signs in on the web or phone with their temporary password and chooses a new one. Each **other desktop** shows PROTECTED ("the server's database changed"). Choose **Use this server** in its **Details**: it takes a snapshot of its own copy, then downloads the rebuilt one, keeping its unsent changes. Then it signs in again.
7. **Check:** the container log shows the rebuild (`sudo docker logs --since 15m vehiclemanager 2>&1 | grep -i rebuild`). Then sign in on the web and look over the Garage, a timeline, and some photos. Once you're happy, delete `data/before-rebuild/`.

**If it says the files aren't sent yet** ("The server is rebuilt, but this computer's photos and documents aren't sent yet"), the rebuild itself is done: **note the temporary passwords first**. Then press **Send the files** once the server can be reached. The same button stays in Diagnostics (under Snapshots of this copy) until it works, even after the app restarts. Until then, the desktop stays PROTECTED, and photos it doesn't have aren't left out yet.

**If the rebuild stops partway** (the network drops, the app quits) before "The server is rebuilt", **start over**. The desktop's copy is untouched (it took a snapshot first, too). Move the partial database aside as in step 1, start the server for a new setup code, and run the rebuild again. A rebuild isn't picked up where it stopped: once the server has accepted the records, the temporary passwords exist only in that one answer, so a fresh start is the way to get them.

## Splitting into a main server and a test server

*Once, in 2026-11 (decision #062). Today's install, with its test vehicles and events, **becomes the test server**; a new, empty **main** server is set up beside it. Takes about 20 minutes; the app is down for part of it.*

**Before you start:**
- The branch with this change (`milestone/p2-test-server`, or `dev` once it's merged) has a finished "Server image" run. Until it merges, both servers use its image, `milestone-p2-test-server`; step 10 moves main to `dev` afterwards.
- Over SSH, note how Tailscale serves the app now:

  ```sh
  sudo /var/packages/Tailscale/target/bin/tailscale serve status
  ```

  It should show `https://<nas-name>.<tailnet>.ts.net` (port 443) going to `http://127.0.0.1:8410`. **If the app is on port 8443 instead** (setup step 5's fallback), use **8444** for the test server below wherever it says 8443.

### Part A: today's install becomes the test server

1. **Stop and remove today's container** (its `data/` folder stays):

   ```sh
   cd /volume1/docker/vehicle_manager
   sudo docker compose down
   ```

2. **Remove the old Project from Container Manager** (Container Manager → **Project** → `vehiclemanager` → **Action** → **Delete**). It only removes the Project's entry: the folder and its `data/` stay. If it offers to delete the folder too, **don't**.

3. **Move the folder,** then get this change's `compose.yaml` (use `dev` instead of `milestone/p2-test-server` in the address once it's merged):

   ```sh
   cd /volume1/docker
   sudo mv vehicle_manager vehicle_manager_test
   cd vehicle_manager_test
   sudo curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-test-server/deploy/compose.yaml
   sudo vi .env
   ```

4. **In `.env`,** set these (add the first two lines if they aren't there), then save (`Esc`, `:wq`):

   ```
   VEHICLEMANAGER_PROJECT_NAME=vehiclemanager-test
   VEHICLEMANAGER_INSTANCE=test
   VEHICLEMANAGER_IMAGE_TAG=milestone-p2-test-server
   VEHICLEMANAGER_HOST_PORT=8411
   VEHICLEMANAGER_TEST_MINUTES=
   ```

5. **Check what Compose will run:**

   ```sh
   sudo docker compose config | grep -E "^name:|container_name|VEHICLEMANAGER_INSTANCE|127.0.0.1|image:"
   ```

   Expect `name: vehiclemanager-test`, `container_name: vehiclemanager-test`, `VEHICLEMANAGER_INSTANCE: test`, `127.0.0.1:8411`, and the `milestone-p2-test-server` image. If anything else shows, fix `.env` first.

6. **Create its Project:** Container Manager → **Project** → **Create**, name **`vehiclemanager-test`**, path **`/docker/vehicle_manager_test`**, *Use existing docker-compose.yml*, **Finish**. Then check its log:

   ```sh
   sudo docker logs --since 5m vehiclemanager-test 2>&1
   ```

   It shows `this is a second server, not the household's main one` with `instance="test"`, `database open` with `created=false` (your existing data), and `database integrity check passed`.

7. **Serve it on the tailnet,** on port 8443, beside main's 443:

   ```sh
   sudo /var/packages/Tailscale/target/bin/tailscale serve --bg --https=8443 http://127.0.0.1:8411
   sudo /var/packages/Tailscale/target/bin/tailscale serve status
   ```

   `status` lists both: 443 → 8410 (main, not running yet) and 8443 → 8411 (test). Open `https://<nas-name>.<tailnet>.ts.net:8443`: sign in with your usual account. The **TEST SERVER** tag shows, and your test vehicles are there.

### Part B: the new main server

8. **Its folder:** in File Station, create `docker/vehicle_manager` and `docker/vehicle_manager/data` inside it, and give **`vehiclemanager`** **Read/Write** on `data` (as in setup step 1). Then, over SSH:

   ```sh
   cd /volume1/docker/vehicle_manager
   sudo curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-test-server/deploy/compose.yaml
   sudo curl -fsSLo .env https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-test-server/deploy/.env.example
   sudo chmod 600 .env
   sudo vi .env
   ```

   In `.env`: `VEHICLEMANAGER_PROJECT_NAME=vehiclemanager`, `VEHICLEMANAGER_INSTANCE=` (empty), `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-test-server` (for now; step 10), `VEHICLEMANAGER_UID=1030`, `VEHICLEMANAGER_GID=100`, `VEHICLEMANAGER_HOST_PORT=8410`, and `VEHICLEMANAGER_TEST_MINUTES=` (empty, always, on main). Then check it as in step 5: `name: vehiclemanager`, `container_name: vehiclemanager`, `VEHICLEMANAGER_INSTANCE: ""`, and `127.0.0.1:8410`.

9. **Create its Project** (name **`vehiclemanager`**, path **`/docker/vehicle_manager`**), then **create the main server's first Admin** with the setup code from its log (setup step 6):

   ```sh
   sudo docker logs vehiclemanager 2>&1 | grep -i "setup code"
   ```

   Open `https://<nas-name>.<tailnet>.ts.net` (no port): first-time setup, and **no** TEST SERVER tag. Tailscale already sends 443 to 8410, so nothing more is needed there.

10. **After this change merges into `dev`** and its "Server image" run finishes: in main's `.env` set `VEHICLEMANAGER_IMAGE_TAG=dev` (from `v0.2.0` on, a release tag instead: decision #070), then update main (see "Updating"). From then on main runs without test timers: setting `VEHICLEMANAGER_TEST_MINUTES` there only logs `test timers setting ignored` ("this build doesn't include test timers").

**Afterwards:**
- **Bookmarks and the desktop app:** main is the address without a port; the test server is `:8443`. The desktop app keeps one server address: set it to main's, and use a browser for the test server.
- **Each milestone:** put its `milestone-<name>` image in the test server's `.env` and run its test plan there. Once its PR merges, update main (it follows `dev`).
- **Snapshots, backups, and logs** are per server, inside each folder's `data/`.

## Copying the main server's data into the test server

*When a test needs real data (decision #062). It replaces the test server's database with main's newest nightly snapshot; the test server's own data is set aside, not deleted. The copy stays on the NAS.*

1. **Stop the test server** and set its database aside (a missing `-wal` or `-shm` file is fine):

   ```sh
   cd /volume1/docker/vehicle_manager_test
   sudo docker compose stop
   sudo mkdir -p data/replaced
   sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/replaced/
   ```

2. **Copy in main's newest snapshot** (the last one `ls` lists), owned by the server's user:

   ```sh
   ls /volume1/docker/vehicle_manager/data/snapshots/
   sudo cp /volume1/docker/vehicle_manager/data/snapshots/snapshot-20261104T070000Z.sqlite data/vehiclemanager.sqlite
   sudo chown 1030:100 data/vehiclemanager.sqlite
   ```

3. **Copy in main's photos and PDFs** (only files the test server doesn't have; each name is its contents, so nothing is overwritten):

   ```sh
   sudo mkdir -p data/files
   sudo cp -an /volume1/docker/vehicle_manager/data/files/. data/files/
   sudo chown -R 1030:100 data/files
   ```

4. **Start it** and check its log:

   ```sh
   sudo docker compose start
   sudo docker logs --since 5m vehiclemanager-test 2>&1
   ```

   `database open` with `created=false`. If the test server runs a newer milestone, it may migrate the copy (backing it up first); main's own database is never touched.

- **Sign in with your main account:** the copy has main's people and passwords.
- **The copy keeps main's database ID.** That's fine now. Once sync arrives (Phase 3), don't connect a desktop app to the test server's copy.
- **To go back** to the test server's own data, stop it, move `data/replaced/vehiclemanager.sqlite*` back into `data/`, and start it. Its own files are still in `data/files/` (main's were only added).

## Logs and troubleshooting

| What | Where |
|---|---|
| Live console log | Container Manager → Container → `vehiclemanager` → Log |
| Log files (JSON lines, 10 × 10 MB, 30 days) | `data/logs/current.jsonl`, `data/logs/log-<time>.jsonl` |
| Last crash | `data/logs/last-crash.txt` (only exists after a crash) |
| Snapshots and checks | See "Database, backups, and snapshots" |
| Is the container listening? | `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8410/` over SSH → `200` |
| What Tailscale is serving | `sudo /var/packages/Tailscale/target/bin/tailscale serve status` |

- **"Permission denied" writing logs:** `data/` isn't writable by `vehiclemanager`. Redo setup step 1's permissions. The server keeps running without file logs and says so in its log.
- **Container exits at once with "set VEHICLEMANAGER_UID in .env":** `.env` is missing those lines, or isn't in the Project folder.
- **`docker compose up` says "container name "/vehiclemanager" is already in use":** the `compose.yaml` in the Project folder is an old copy without the `name: vehiclemanager` line. Download it again (setup step 3), or add `-p vehiclemanager` to both update commands.
- **Upgrading from Phase 1:** the `VEHICLEMANAGER_TEST_CREDENTIAL` line in an older `.env` is ignored now; delete it. Signing in replaces it.
- **Diagnostics shows "Server health: Fail":** the database couldn't be opened or failed a check; nothing is being saved. The container log says which (`can't open the database` or `database integrity problem`). See "Database, backups, and snapshots".
- **`can't open the place data`:** the file in `VEHICLEMANAGER_GEODATA_FILE` is missing or isn't place data; empty the setting to use the image's own, then `sudo docker compose up -d`.
- **`job skipped` for the nightly snapshot:** the snapshot folder set in `VEHICLEMANAGER_SNAPSHOT_DIR` isn't there (e.g. the USB disk isn't mounted). The household's Admin also sees it in **Messages** (in Admin mode), as does a failed snapshot or hourly check; the message goes away once the cause is fixed.
- **Failed sign-ins:** each one logs `sign-in refused` with the visitor's address (`client="100.x.y.z"`, a tailnet address), never the username or password: `sudo docker logs --since 1h vehiclemanager 2>&1 | grep 'sign-in refused'`. `client="direct"` means the request didn't come through `tailscale serve` (or `VEHICLEMANAGER_PROXY_HOPS` is too high).
- **`request refused: body too large` (413) or `request timed out` (408):** a request over the size limit (2 MB; uploads 51 MB) or not answered in time (1 minute; uploads and downloads 10 minutes). The app never sends one that large; a timeout means the server was stuck, so look at the lines before it.
- **Pulls fail with "denied":** the GHCR package is still private (setup step 2).
- **Signing in to one server signs you out of the other:** one of them still has `compose.yaml` from before the split, which doesn't pass `VEHICLEMANAGER_INSTANCE`. Download the new one (as in "Splitting", step 3), check with `sudo docker compose config`, and `sudo docker compose up -d`.
- **The main server shows a TEST SERVER tag:** its `.env` sets `VEHICLEMANAGER_INSTANCE`. Empty it, then `sudo docker compose up -d`.
