# Manual test plan: a test server beside the main server

*Phase 2, between milestones 6 and 7 · `docs/phase_plans/phase-2.md` · decision #062*

## 1. Intended behavior

- **Two servers on the NAS:**
  - **main** (the household's): `vehiclemanager`, port 8410, `https://<nas-name>.<tailnet>.ts.net`;
  - **test**: `vehiclemanager-test`, port 8411, `:8443`.
  - Each has its own folder, data, snapshots, and logs.
- **The test server says so:** a **TEST SERVER** tag in the top bar (**TEST** on a phone) and on the sign-in card; its browser tab's title is "test server". Main shows nothing.
- **Separate sign-ins:** each server has its own session cookie, so one browser can be signed in to both, as different people.
- **Images:** main runs `:dev`, built without test timers. The test server runs `:milestone-<name>` images, with them. Until this change merges, both run `:milestone-p2-test-server`.
- **Copying main into test:** an optional step replaces the test server's database with main's newest snapshot, setting the test server's own aside.

## 2. Setup

- The "Server image" run for the newest push to `milestone/p2-test-server` finished successfully.
- A phone and the Mac's browser on the tailnet.
- **Do the switch-over first:** `docs/deploying-nas.md` → "Splitting into a main server and a test server", Parts A and B (steps 1–9). Steps 1–8 below check it.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1 | `sudo /var/packages/Tailscale/target/bin/tailscale serve status` | Two entries: 443 → `http://127.0.0.1:8410`, and 8443 → `http://127.0.0.1:8411`. |
| [ ] | 2 | `sudo docker ps --format '{{.Names}} {{.Image}} {{.Ports}}'` | `vehiclemanager` and `vehiclemanager-test`, both on the `milestone-p2-test-server` image, on `127.0.0.1:8410` and `127.0.0.1:8411`. |
| [ ] | 3 | **Mac browser:** open the test server (`:8443`) and sign in with your usual account | The **TEST SERVER** tag on the sign-in card and in the top bar; the tab says "test server"; your test vehicles and their events are there. |
| [ ] | 4 | **Same browser, another tab:** open main (no port) and sign in as main's new Admin | No tag; the tab's title is the usual one; "No vehicles yet." |
| [ ] | 5 | Go back to the test server's tab and reload; then main's tab and reload | Both still signed in, each as its own account. |
| [ ] | 6 | **Phone:** the same, test server first, then main | Both signed in at once. On the test server the top bar says **TEST**, and the account chip still fits. |
| [ ] | 7 | **Main:** Admin mode → **+ Add vehicle** → a real vehicle (or `Main check`) | It's on main only; the test server's Garage doesn't have it. |
| [ ] | 8 | **Test timers only on test:** in the test server's `.env` set `VEHICLEMANAGER_TEST_MINUTES=2`, `sudo docker compose up -d` in its folder, reload it | **TEST TIMERS · 2 min** on the test server only; main has no tag. Empty it again and `up -d`. |
| [ ] | 9 | **Copy main into test** (`docs/deploying-nas.md` → "Copying the main server's data into the test server"). Main makes its first snapshot at 07:00 UTC, so do this the next day, or skip it for now | The test server opens with main's vehicles (step 7's among them) and main's people; sign in with main's account. The log shows `database open` with `created=false`. Then put the test server's own data back ("To go back"). |
| [ ] | 10 | **After the PR merges and `dev`'s image is built:** main's `.env` → `VEHICLEMANAGER_IMAGE_TAG=dev`, then `sudo docker compose pull && sudo docker compose up -d` in main's folder | Main runs `:dev`. `sudo docker logs --tail 20 vehiclemanager 2>&1` has no `TEST TIMERS` lines. Your sign-in on main is kept. |
| [ ] | 11 | *(Optional)* On main, set `VEHICLEMANAGER_TEST_MINUTES=2` and `up -d`, then `sudo docker logs --since 2m vehiclemanager 2>&1 \| grep -i "test timers"` | `test timers setting ignored`, "this build doesn't include test timers"; no tag in the app. Empty it again and `up -d`. |
| [ ] | 12 | **Secrets check,** on both: `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` and the same for `vehiclemanager-test` | `0` and `0`. |

## 4. If a step fails, capture

- **Container logs:** `sudo docker logs --since 30m vehiclemanager 2>&1` and the same for `vehiclemanager-test`.
- **What Compose runs:** `sudo docker compose config` in that server's folder.
- **Tailscale:** `sudo /var/packages/Tailscale/target/bin/tailscale serve status`.
- **Never share** a password, `.env`, or a cookie.
