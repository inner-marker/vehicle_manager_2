# Deploying the server on the NAS

*Synology DS1019+, DSM 7.4.1, Container Manager and Tailscale installed (decisions.md #025). Phase 1 milestone 3.*

The server runs as one Docker container from the image `ghcr.io/inner-marker/vehicle_manager_2`. Container Manager runs it as a **Project** defined by `deploy/compose.yaml`.

- **Access:** it listens only on the NAS's loopback. **Tailscale is the only way in**, over HTTPS with `tailscale serve` (plan §6, §7).
- **Data:** everything that must survive updates lives in one folder on the NAS.

```
/volume1/docker/vehicle_manager/   Project folder
├── compose.yaml                   copy of deploy/compose.yaml
├── .env                           settings + credential (from deploy/.env.example); NAS only
└── data/                          the container's /data: logs/ now, the database from Phase 2
```

> **Never put `data/` on a network share** (NFS/SMB). It must be a local NAS volume (decision #002).
> **Never copy the database file while the server runs** (from Phase 2); use the app's snapshots.

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

### 3. Choose a credential
On your Mac, run `openssl rand -base64 32` and keep the output for step 4. **Don't put it in the repo, a chat, or a screenshot.**

### 4. Put `compose.yaml` and `.env` in the Project folder
Enable SSH if needed: Control Panel → Terminal & SNMP → *Enable SSH service*. Then:

```sh
ssh <your-admin-user>@<nas-lan-ip>
cd /volume1/docker/vehicle_manager
curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/dev/deploy/compose.yaml
curl -fsSLo .env https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/dev/deploy/.env.example
chmod 600 .env
vi .env
```

In `.env`, check `VEHICLEMANAGER_UID=1030`, `VEHICLEMANAGER_GID=100` and `VEHICLEMANAGER_HOST_PORT=8410`, and paste the credential after `VEHICLEMANAGER_TEST_CREDENTIAL=`. Use `nano` instead of `vi` if it's installed. You can also make the two files on your Mac and upload them with File Station; files starting with `.` upload fine.

### 5. Create the Project (Container Manager)
- Go to Container Manager → **Project** → **Create**.
  - **Project name:** `vehiclemanager` (must match the `name:` line in `compose.yaml`)
  - **Path:** `/docker/vehicle_manager`
  - **Source:** *Use existing docker-compose.yml*. It should find `compose.yaml`. If your Container Manager version only looks for `docker-compose.yml`, rename the file to that.
- Skip the web portal (Web Station) option.
- **Finish.** It pulls the image and starts the container.

**Check:** Container → `vehiclemanager` shows **Running**, and its **Log** tab includes `app starting` and `writing log files` lines. `data/logs/current.jsonl` appears in File Station.

If the log says `no API credential configured`, the `.env` line is empty or wasn't read. Every API request is refused until that's fixed (decision #024).

### 6. Publish it on the tailnet (`tailscale serve`)
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

## Logs and troubleshooting

| What | Where |
|---|---|
| Live console log | Container Manager → Container → `vehiclemanager` → Log |
| Log files (JSON lines, 10 × 10 MB, 30 days) | `data/logs/current.jsonl`, `data/logs/log-<time>.jsonl` |
| Last crash | `data/logs/last-crash.txt` (only exists after a crash) |
| Is the container listening? | `curl -s -o /dev/null -w '%{http_code}\n' http://127.0.0.1:8410/` over SSH → `200` |
| What Tailscale is serving | `sudo /var/packages/Tailscale/target/bin/tailscale serve status` |

- **"Permission denied" writing logs:** `data/` isn't writable by `vehiclemanager`. Redo setup step 1's permissions. The server keeps running without file logs and says so in its log.
- **Container exits at once with "set VEHICLEMANAGER_UID in .env":** `.env` is missing those lines, or isn't in the Project folder.
- **`docker compose up` says "container name "/vehiclemanager" is already in use":** the `compose.yaml` in the Project folder is an old copy without the `name: vehiclemanager` line. Download it again (setup step 4), or add `-p vehiclemanager` to both update commands.
- **Pulls fail with "denied":** the GHCR package is still private (setup step 2).
