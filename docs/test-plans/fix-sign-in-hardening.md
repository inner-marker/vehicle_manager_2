# Manual test plan: sign-in hardening

*Before Phase 3 · branch `fix/sign-in-hardening` · decision #071 · `docs/sub-plans/04-security.md` §5.1*

## 1. Intended behavior

- **The real client's address:** the sign-in limit uses the address `tailscale serve` adds to the **end** of `X-Forwarded-For`, never one the client wrote itself. `VEHICLEMANAGER_PROXY_HOPS` says how many proxies are in front (empty means 1, for `tailscale serve`). A failed sign-in logs the address it counted (`client=…`), with no username or password.
- **At most 2 passwords checked at once.** A sign-in that waits more than 10 seconds for its turn gets "Too many tries; try again in 5 seconds". Nothing a household does can reach this, so it isn't tested here; tests in `server::passwords` cover it.
- **Size limits:** a request body over 2 MB is refused (**413**); photo and PDF uploads keep their 51 MB limit. The app refuses a file over 50 MB before sending it, as before.
- **Time limits:** a request not answered within 1 minute gets **408** (10 minutes for uploads and downloads). Nothing the app does today takes that long.
- **Nothing else changes:** signing in, Admin mode, uploads, downloads, and pages work as before.

## 2. Setup

- **An image of this branch:** on GitHub, Actions → **Server image** → **Run workflow**, branch `fix/sign-in-hardening`. Or from the Mac:

  ```
  gh workflow run 'Server image' --ref fix/sign-in-hardening
  ```

  When it finishes, its summary lists the `sha-<commit>` tag it pushed.
- **The test server** (`:8443`): in its `.env`, set `VEHICLEMANAGER_IMAGE_TAG=sha-<commit>`, leave `VEHICLEMANAGER_PROXY_HOPS` out (or empty), then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- The Mac on the tailnet, with Terminal. Your Mac's tailnet address: `tailscale ip -4` (a `100.x.y.z` address).

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1 | **Browser:** open the test server and sign in with your usual account | Signed in as before; Diagnostics shows the `sha-` build. |
| [ ] | 2 | Sign out, then sign in with a **wrong password** once, then sign in properly | "Wrong username or password.", then signed in. |
| [ ] | 3 | **NAS:** `sudo docker logs --since 5m vehiclemanager-test 2>&1 \| grep 'sign-in refused'` | One line, `sign-in refused: wrong password`, with `client="100.x.y.z"`: **your Mac's tailnet address**. No username or password in it. *(If it says `client="direct"`, `tailscale serve` isn't adding the header: stop here and send the line.)* |
| [ ] | 4 | **Mac Terminal, a forged address:** `curl -s https://<nas-name>.<tailnet>.ts.net:8443/api/v1/sign-in -H 'x-api-version: 1' -H 'content-type: application/json' -H 'X-Forwarded-For: 1.2.3.4' -d '{"username":"nobody","password":"not-a-real-password","want_token":false}'` | `{"outcome":"invalid-credentials"}`. |
| [ ] | 5 | Step 3's command again | A new line, `unknown or disabled user`, with `client="100.x.y.z"` (your Mac again), **not** `1.2.3.4`. |
| [ ] | 6 | **An oversized body:** `head -c 3000000 /dev/zero > /tmp/big.bin && curl -s -o /dev/null -w '%{http_code}\n' https://<nas-name>.<tailnet>.ts.net:8443/api/v1/sign-in -H 'x-api-version: 1' -H 'content-type: application/json' --data-binary @/tmp/big.bin` | `413`. The log has `request refused: body too large`. |
| [ ] | 7 | **Browser:** on a vehicle, add a **photo** and a **PDF** (a few MB each), then open both | Both upload and open as before. |
| [ ] | 8 | **Phone:** sign in, open the Garage and a vehicle, add a photo from the camera | Works as before. |
| [ ] | 9 | Admin mode: enter it with your password, leave it | As before. |
| [ ] | 10 | Afterwards: set the test server's `VEHICLEMANAGER_IMAGE_TAG` back to what it was, `pull` and `up -d` | Back on its usual image. |

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **What Compose runs:** `sudo docker compose config` in the test server's folder.
- **Tailscale:** `sudo /var/packages/Tailscale/target/bin/tailscale serve status`.
- **Never share** a password, `.env`, or a cookie.
