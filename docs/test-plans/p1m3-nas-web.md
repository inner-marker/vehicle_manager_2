# Manual test plan: web UI on the NAS

*Phase 1, milestone 3 · `docs/phase_plans/phase-1.md` · `docs/deploying-nas.md` · decisions #023–#026*

## 1. Intended behavior

- **The image:** `ghcr.io/inner-marker/vehicle_manager_2` (linux/amd64) runs on the DS1019+ as the Container Manager Project `vehiclemanager`:
  - as the DSM user `vehiclemanager` (1030:100), never root;
  - with a read-only filesystem except `/data` and a small `/tmp`;
  - with logs in `data/logs/`.
- **Reachable only over Tailscale:**
  - The container publishes port 8410 on the NAS's **loopback only**.
  - `tailscale serve` provides **HTTPS** with a valid certificate at `https://<nas>.<tailnet>.ts.net`.
  - From the LAN (or the NAS's LAN IP), the app must **not** be reachable on 8410.
- **Phase 1 exit criterion (web half):** from another device on the tailnet, the web UI loads over HTTPS, and Diagnostics → Run checks **passes** with the right credential and is **rejected cleanly** with a wrong one.
- **Survives restarts:** the container comes back after a restart or a NAS reboot (`restart: unless-stopped`), and so does `tailscale serve` (`--bg`).
- **Updates and rollbacks** leave `data/` untouched.

## 2. Setup

- **NAS:** done in our earlier steps:
  - folder `/volume1/docker/vehicle_manager/data`;
  - `vehiclemanager` has Read/Write on it;
  - Container Manager and Tailscale installed; MagicDNS and HTTPS certificates enabled;
  - SSH enabled (Control Panel → Terminal & SNMP).
- **Credential:** run `openssl rand -base64 32` on your Mac and keep the output private.
- **Files for this test come from the milestone branch, not `dev`**, because milestone 3 isn't merged yet. Over SSH on the NAS:

  ```sh
  cd /volume1/docker/vehicle_manager
  curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p1-m3-nas-web/deploy/compose.yaml
  curl -fsSLo .env https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p1-m3-nas-web/deploy/.env.example
  chmod 600 .env
  vi .env
  ```

  In `.env`, set `VEHICLEMANAGER_IMAGE_TAG=milestone-p1-m3-nas-web` and paste the credential into `VEHICLEMANAGER_TEST_CREDENTIAL=`. Keep UID 1030, GID 100 and port 8410.
- **Devices:** a second tailnet device (phone or laptop) with Tailscale connected. For step 12, a LAN device with Tailscale **off**, or the NAS's own LAN IP.

## 3. Checklist

| # | Step | Expected result |
|---|---|---|
| 1 | Container Manager → **Project** → **Create**: name `vehiclemanager`, path `/docker/vehicle_manager`, *use existing* compose file; skip Web Station; Finish | It pulls the image and the Project shows **Running**. No "set VEHICLEMANAGER_UID" error. |
| 2 | Container → `vehiclemanager` → **Log** | Lines `writing log files` (with `log_dir=/data/logs`), `app starting version="0.1.0"`, and `API credential configured`. **No** `can't write log files` or `no API credential configured` warnings. **The credential appears nowhere.** |
| 3 | File Station → `docker/vehicle_manager/data/logs` | `current.jsonl` exists and is owned by `vehiclemanager`. |
| 4 | Over SSH: `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8410/` | `200` |
| 5 | Over SSH: `sudo /var/packages/Tailscale/target/bin/tailscale serve --bg --https=443 http://127.0.0.1:8410`, then `... serve status` | Status shows `https://<nas>.<tailnet>.ts.net` → `http://127.0.0.1:8410`. If 443 is refused, use the 8443 fallback in `docs/deploying-nas.md` and add `:8443` to the address in the steps below. |
| 6 | On the second tailnet device, open `https://<nas>.<tailnet>.ts.net` | The Garage screen loads, **with a padlock** (valid certificate, no warning), Dark theme, and working navigation. |
| 7 | On that device: Diagnostics → enter a **wrong** credential → **Run checks** | Credential **Fail**, API version **Not checked**, "The server didn't accept this credential." |
| 8 | Enter the **right** credential → **Run checks** | Both **Pass**; "Server app 0.1.0, API 1 (accepts 1–1)"; a round-trip time. |
| 9 | Diagnostics log viewer → **Refresh** | Shows this browser's entries (e.g. "app starting"). Server logs aren't shown here by design; they're in step 2's log and `data/logs`. |
| 10 | On a phone: repeat steps 6 and 8 in the phone's browser over Tailscale | Same results. The narrow layout (bottom tabs, ☰ menu) works. |
| 11 | Look at `data/logs/current.jsonl` (File Station → download, or `tail -n 20` over SSH) | JSON lines including the startup lines from step 2. **The credential appears nowhere in the file.** (API checks themselves aren't logged at the default `info` level; refusals are `debug`.) |
| 12 | From a LAN device with **Tailscale off**: open `http://<nas-lan-ip>:8410` | **Fails** (connection refused or timeout). The app is not on the LAN. |
| 13 | Container Manager → Project → **Stop**, then **Start** | Running again; step 6's address loads again. |
| 14 | Restart the NAS (DSM top-right account menu → **Restart**), and wait for it to come back | The container is **Running** again without you doing anything, and `tailscale serve status` still shows the mapping. Step 6's address loads. |
| 15 | **Update flow:** over SSH, in the Project folder: `sudo docker compose pull && sudo docker compose up -d` | Recreated and running; `data/logs/current.jsonl` still contains the earlier lines. |
| 16 | **Rollback flow:** set `VEHICLEMANAGER_IMAGE_TAG=sha-354aa4a` in `.env`, then repeat step 15's commands, then set it back to `milestone-p1-m3-nas-web` and repeat | Each time the container recreates and runs. `sudo docker ps --format '{{.Image}}'` shows the tag in use. |

## 4. If a step fails, capture

- **Container log:** Container Manager → Container → `vehiclemanager` → **Log**, or `sudo docker logs --tail 200 vehiclemanager` over SSH. **Check that the credential isn't in it before sharing.**
- **Log files:** `data/logs/current.jsonl`, and `data/logs/last-crash.txt` if it exists.
- **Container state:** `sudo docker ps -a --filter name=vehiclemanager` and `sudo docker inspect vehiclemanager --format '{{.State.Status}} {{.State.ExitCode}} {{.Config.User}}'`.
- **Tailscale:** `sudo /var/packages/Tailscale/target/bin/tailscale serve status` and `... status`. **Blank out your tailnet name** if you'd rather not share it.
- **Steps 6–10:** a screenshot, plus the browser devtools console and Network tab entry for `/api/v1/handshake` (status and response). **Blank out the `authorization` header.**
- **Never share `.env` or the credential.**
