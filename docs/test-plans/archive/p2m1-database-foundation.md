# Manual test plan: database foundation on the NAS

*Phase 2, milestone 1 · `docs/phase_plans/phase-2.md` · `docs/deploying-nas.md` "Database, backups, and snapshots" · decisions #035, #042–#044*

## 1. Intended behavior

- **The database:** the server creates `data/vehiclemanager.sqlite` on first start, owned by the `vehiclemanager` user, and reopens the **same database** (same database ID) after every restart and update.
- **Checks:**
  - At every start, SQLite's integrity and foreign-key checks run. A failure makes the server **unhealthy**: it keeps serving pages, but refuses every change, and Diagnostics → Connection shows **Server health: Fail**.
  - Every hour, a quick check runs; a problem also makes the server unhealthy, until a restart.
- **Nightly snapshot** at **07:00 UTC**: `data/snapshots/snapshot-<UTC time>.sqlite`, checked after writing; only the newest 7 are kept. That's **03:00 Eastern daylight time, or 02:00 after US daylight saving time ends (November 1, 2026)**.
- **Updates** that don't change the database's schema make no backup (`data/backups/` stays empty). This milestone has only one schema version, so no backup is expected yet.
- **A restored snapshot** opens as the same database.
- **Not testable from the UI yet:** commands themselves (no screen sends one until milestone 2), and refusing a database from a newer version. Both are covered by automated tests.

## 2. Setup

- **The milestone image:** after the milestone branch is pushed, the "Dev image" workflow builds `milestone-p2-m1-database`. Check Actions → it finished successfully.
- **Over SSH on the NAS:** get the milestone's `compose.yaml` and switch the image tag. Your existing `.env` needs no other change; the new snapshot setting is optional and empty by default.

  ```sh
  cd /volume1/docker/vehicle_manager
  curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-m1-database/deploy/compose.yaml
  vi .env
  ```

  In `.env`, set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m1-database`. Then `Esc` and `:wq`.
- **There's no real data in the database yet,** so steps 9–11 (the restore and damaged-database drills) can't lose anything.
- **A second tailnet device** with the web UI open, and the NAS credential, for the Diagnostics checks.
- **Timing:** steps 7 and 8 need the server to run **overnight** (past 07:00 UTC) and for **an hour** respectively. Do 1–6 one evening, 7–8 the next morning, then 9–11.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 30 vehiclemanager` | Lines `database open` with `created=true schema_before=0 schema_now=1` and a `database_id=…`, then `database integrity check passed` and `scheduled jobs started` with `snapshot_dir=/data/snapshots`. **Write down the database ID.** No `ERROR` lines. |
| [ ] | 2 | `ls -ln data/` | `vehiclemanager.sqlite` (and `-wal`, `-shm`) owned by `1030 100`. `logs/` as before. No `snapshots/` yet (it's made at the first snapshot). |
| [ ] | 3 | On the second device: Diagnostics → enter the credential → **Run checks** | API version, Credential, and **Server health** all **Pass**. |
| [ ] | 4 | `sudo docker compose restart`, then the log again | `database open` with **`created=false`** and the **same database ID** as step 1; `database integrity check passed`. |
| [ ] | 5 | **Update flow:** `sudo docker compose pull && sudo docker compose up -d`, then the log | Same database ID, `created=false`, `schema_before=1 schema_now=1`. `ls data/backups` says it doesn't exist (no upgrade, so no backup). |
| [ ] | 6 | `sudo docker logs vehiclemanager 2>&1 \| grep -i credential` | Only `API credential configured`. **The credential itself appears nowhere.** |
| [ ] | 7 | **The next morning** (after 07:00 UTC): `ls -l data/snapshots/` and `sudo docker logs vehiclemanager 2>&1 \| grep nightly-snapshot` | One `snapshot-<date>T070000Z.sqlite`, a few dozen KB. A `job finished job="nightly-snapshot"` line whose message says `wrote /data/snapshots/…; removed 0 older`. |
| [ ] | 8 | `sudo docker logs vehiclemanager 2>&1 \| grep quick-check` | At least one `job finished job="quick-check"` line with `no problems`, about an hour apart if several. |
| [ ] | 9 | **Restore drill:** follow `docs/deploying-nas.md` → "Restoring a snapshot", with the snapshot from step 7 | After `docker compose start`: `database open` with `created=false`, the **same database ID** as step 1, `database integrity check passed`. Diagnostics → Server health **Pass**. Then delete `data/replaced/`. |
| [ ] | 10 | **Damaged-database drill** (safe: there's no real data). Stop, keep a copy, replace the database with garbage, start: see the commands below | The log shows `can't open the database; refusing all changes` with "file is not a database". The container stays **Running** and the web UI still loads. Diagnostics → **Server health: Fail** with "The server has a problem with its database…". |
| [ ] | 11 | **Recover from the drill:** put the copy back (commands below) and start | `database integrity check passed`, same database ID; Server health **Pass**. |
| [ ] | 12 | *(Optional, needs another overnight run)* Set `VEHICLEMANAGER_SNAPSHOT_DIR=/snapshots` in `.env` (nothing is mounted there), `sudo docker compose up -d`, wait past 07:00 UTC, check the log; then empty the setting again and `up -d` | A `job skipped job="nightly-snapshot"` line: "snapshot folder /snapshots is missing (not mounted?)". No new file in `data/snapshots/`. The server stays healthy. |

**Step 10 commands** (over SSH, in the Project folder). The `-wal` and `-shm` files go too: in WAL mode, recent changes live there, and SQLite would use them to repair the main file.

```sh
sudo docker compose stop
sudo mkdir -p data/before-drill
sudo mv data/vehiclemanager.sqlite data/vehiclemanager.sqlite-wal data/vehiclemanager.sqlite-shm data/before-drill/
sudo sh -c 'head -c 65536 /dev/urandom > data/vehiclemanager.sqlite'
sudo chown 1030:100 data/vehiclemanager.sqlite
sudo docker compose start
sudo docker logs --tail 20 vehiclemanager
```

(Real page-level corruption, which the integrity check catches, is covered by the automated test `a_corrupted_file_fails_the_integrity_check`.)

**Step 11 commands:**

```sh
sudo docker compose stop
sudo rm data/vehiclemanager.sqlite
sudo mv data/before-drill/* data/
sudo rmdir data/before-drill
sudo docker compose start
sudo docker logs --tail 20 vehiclemanager
```

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --tail 200 vehiclemanager`. Database and job messages are on the `database` area; startup on `app`. Check that the credential isn't in it before sharing.
- **Log files:** `data/logs/current.jsonl` for the test's time range, and `data/logs/last-crash.txt` if it exists.
- **Folder contents and owners:** `ls -lnR data/ | head -50`.
- **Container state:** `sudo docker ps -a --filter name=vehiclemanager`.
- **Steps 3, 9–11:** a screenshot of the Connection panel.
- **Never share `.env`** or the credential.
