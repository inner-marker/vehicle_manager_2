# Deploying the server on the NAS

*Synology DS1019+, DSM 7.4.1, Container Manager and Tailscale installed (decisions.md #025). Phase 1 milestone 3.*

The server runs as one Docker container from the image `ghcr.io/inner-marker/vehicle_manager_2`. Container Manager runs it as a **Project** defined by `deploy/compose.yaml`.

- **Access:** it listens only on the NAS's loopback. **Tailscale is the only way in**, over HTTPS with `tailscale serve` (plan §6, §7).
- **Data:** everything that must survive updates lives in one folder on the NAS.

```
/volume1/docker/vehicle_manager/   Project folder
├── compose.yaml                   copy of deploy/compose.yaml
├── .env                           settings (from deploy/.env.example); NAS only
└── data/                          the container's /data
    ├── vehiclemanager.sqlite      the database (plus -wal and -shm files while it runs)
    ├── backups/                   copies made before each database upgrade
    ├── snapshots/                 nightly snapshots, newest 7
    └── logs/                      log files and last-crash.txt
```

> **Never put `data/` on a network share** (NFS/SMB). It must be a local NAS volume (decision #002).
> **Never copy the live database file while the server runs;** use a snapshot (see "Database, backups, and snapshots").

## Image tags

| Tag | What it is | Use it for |
|---|---|---|
| `dev` | Newest build of the `dev` branch | Trying the latest merged work |
| `milestone-<name>` (e.g. `milestone-p1-m3-nas-web`) | Newest build of a milestone branch | Testing a milestone on the NAS before its PR |
| `sha-<commit>` | One exact build | Pinning, or rolling back to a known-good build |
| `0.1.0`, `latest` | Releases (from the release workflow) | Normal use once releases exist |

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

### 6. Create the first Admin

A new server has no users. Until it has one, it writes a one-time **setup code** to its log at every start (decision #028):

```sh
sudo docker logs vehiclemanager 2>&1 | grep -i "setup code"
```

The line ends with `setup_code=K7QM-4XRT-9PWD-H2CN` (yours differs). Open the app's address from step 5; it shows **first-time setup**. Enter the code, your username, name, and a password of at least 12 characters. You're signed in as the household's **Admin**, and the code stops working.

- **Only someone who can read the server's log can do this,** so another device on the tailnet can't claim Admin first.
- Add the rest of the household afterwards, in Admin mode.
- **Forgot the password, and you're the only Admin?** There's no reset yet; idea 11 in `docs/ideas-to-discuss.md` (a NAS maintenance tool) is where that would go.

## Updating

Over SSH:

```sh
cd /volume1/docker/vehicle_manager
sudo docker compose pull
sudo docker compose up -d
```

This pulls the newest image for the tag in `.env` and recreates the container. `data/` is untouched. In Container Manager, the Project's *Action → Build* may do the same; if the new image doesn't show up, use the commands above.

## Rolling back

Set `VEHICLEMANAGER_IMAGE_TAG=sha-<commit>` in `.env` to a known-good build (the "Server image" workflow run lists the tags it pushed), then run the two update commands. Set it back to `dev` (or a release) afterwards.

> **After a database upgrade, an older build won't open the database.** A newer image may upgrade the database's schema (backing it up first, in `data/backups/`). Rolling back to a build from before that upgrade makes the server refuse the database: it keeps running but reports itself unhealthy, and its log says `can't open the database` with "newer than this app". Either go forward again, or restore the matching pre-upgrade backup the same way as a snapshot (below), knowing that anything saved since the upgrade is lost.

## Database, backups, and snapshots

The server keeps everything in `data/vehiclemanager.sqlite` and protects it three ways:

| What | When | Where | Kept |
|---|---|---|---|
| **Startup check** | Every start | Container log: `database integrity check passed` | none |
| **Hourly quick check** | Every hour | Container log: `job finished job="quick-check"` | none |
| **Nightly snapshot** | 07:00 UTC (03:00 US Eastern daylight time) | `data/snapshots/snapshot-<UTC time>.sqlite` | Newest 7 |
| **Pre-upgrade backup** | Before an update changes the database's schema | `data/backups/pre-migration-<UTC time>-v<N>.sqlite` | All, until you delete them |

- **If a check fails,** the server keeps serving pages but **refuses every change**, and Diagnostics → Connection shows **Server health: Fail**. The log lists the problems (`database integrity problem`). Restart it after fixing the cause; a problem found by the hourly check lasts until a restart.
- **Each snapshot is checked** after it's written. A snapshot that missed its time (the NAS was off) runs soon after the next start.
- **Snapshots are in the same storage pool** as the database (decision #035). They protect against corruption, a bad upgrade, and mistakes, not against losing the NAS itself; offsite backups come later.
- **Checking the last snapshot:** `ls -l data/snapshots/` over SSH shows them, newest last. The container log has a `job finished` line for each (or `job failed` with the reason).
- **Another snapshot folder** (e.g. a USB disk, later): mount it in `compose.yaml` and set `VEHICLEMANAGER_SNAPSHOT_DIR` in `.env` to its path inside the container (see the comments in both files). That folder is never created by the server: if the disk isn't mounted, the snapshot is **skipped** and logged, rather than written to the pool.

### Restoring a snapshot

Restoring replaces the database with an older copy: **everything saved after that snapshot is lost.** Over SSH:

1. Stop the server, and list the snapshots:

   ```sh
   cd /volume1/docker/vehicle_manager
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
   sudo docker logs --tail 20 vehiclemanager
   ```

   It should show `database open` with `created=false` and `database integrity check passed`.

The database ID is the same as before (it's inside the snapshot), so devices treat it as the same database. Once everything looks right, delete `data/replaced/`.

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
- **`job skipped` for the nightly snapshot:** the snapshot folder set in `VEHICLEMANAGER_SNAPSHOT_DIR` isn't there (e.g. the USB disk isn't mounted).
- **Pulls fail with "denied":** the GHCR package is still private (setup step 2).
