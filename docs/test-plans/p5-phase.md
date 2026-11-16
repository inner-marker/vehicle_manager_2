# Test plan: Phase 5, maintenance intelligence

*`docs/phase_plans/phase-5.md` · decisions #080, #081 · built up milestone by milestone, run at the end of the phase*

**How this plan works (#081):** each milestone adds its section here as its last step. Nothing is run until the end of Phase 5. Then the human runs the whole plan on the **test server**, before the `v0.5.0` release, as its regression pass, together with the phase's exit criteria. A failure is fixed on `dev`, and its section is run again.

## Setup (for the whole run)

- **The image:** the test server on `:dev` (or the release PR's merge into `dev`), once every milestone is in. On the NAS over SSH, in `/volume1/docker/vehicle_manager_test`: `.env` has `VEHICLEMANAGER_IMAGE_TAG=dev` and `VEHICLEMANAGER_TEST_MINUTES=` empty.
- **The desktop app, bundled from `dev`:**

  ```sh
  git switch dev
  git pull
  cd crates/app
  dx bundle --platform desktop --package-types app
  ```

  Open the `.app` from `target/dx/app/bundle/macos/` and sign in to the **test server** as `admin`.
- **People:** `admin` (the household's Admin, also a Maintainer) and Sam (Driver).
- **Devices:** the Mac (the desktop app, and a browser) and the phone's browser. **Offline** means **Tailscale off on the Mac**; the phone stays on Tailscale.
- **The vehicle:** `Test truck`, with its odometer.

## If a step fails, capture

- **The step's number,** a screenshot, the device and browser (or the desktop app), and who was signed in, in which mode.
- **Desktop:** Diagnostics → the log viewer (**Area: Sync**, Minimum level: Debug), or the log export.
- **Container logs:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **Never share** a password, a PIN, `.env`, a cookie, a token, or a document with personal details on it.

---

## Milestone 1: meters (percentage, counting down, resettable)

**Intended behavior:**
- A meter can measure a **percentage**. It **counts up or down**; the direction is fixed once it has a reading. It can be **resettable**, with a reset value (e.g. Oil life back to 100%). A distance meter that fuel-ups need can't be resettable.
- A counting-down meter may stay level (Oil life at 0% again and again) but never rise, unless the reading is marked **↺ Reset**. A reset starts afresh. Only a resettable meter's reading can be a reset.
- Resets show ↺ wherever readings show. Existing meters and readings come through migration 0022 unchanged, counting up.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | 1.1 | **NAS:** `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --since 5m vehiclemanager-test 2>&1` | `backed up before migrating`, then `database open` with `schema_before=21` and `schema_now=22`; `database integrity check passed`; no `ERROR` lines. |
| [ ] | 1.2 | **Mac browser**, `admin`: `Test truck` → **Setup** → **Meters**; then its **Timeline** | The odometer as before ("Distance · miles · whole"), with no "counts down" or "resets". Every reading on the timeline as before, with no ↺. |
| [ ] | 1.3 | **Maintainer mode** → **Setup** → **+ Meter**: name `Oil life`, **Measures: Percentage** (the unit becomes **%**), **Decimal places: whole**, **Counts: Down**, tick **Can be reset** → **Save** | "to [100] %" appears when ticked. Saved; Meters lists "Oil life · Percentage · % · whole · counts down · resets to 100 %". |
| [ ] | 1.4 | Edit the **odometer**, tick **Can be reset** → **Save** | Refused: "A distance meter that fuel-ups need can't be reset: fuel economy needs its running total." The odometer is unchanged. |
| [ ] | 1.5 | **Phone**, Sam: **+** → **Reading** on `Test truck`: Oil life `80` → **Save**. Then another reading a minute later: Oil life `60` → **Save** | Both saved. The odometer has no **↺ Reset** tick; Oil life has one. The timeline shows "80 %" and "60 %". |
| [ ] | 1.6 | Another reading: Oil life `60` again → **Save** | Saved (staying level is fine). |
| [ ] | 1.7 | Another reading: Oil life `70`, **Reset not ticked** → **Save** | Refused under Oil life: "This meter counts down: higher than 60 % on …", with an **↺ It was a reset** button. |
| [ ] | 1.8 | Tap **↺ It was a reset** → **Save** | The message goes and **↺ Reset** is ticked. Saved; the timeline shows "70 % ↺", and the event's page shows a "70 % ↺" tile. |
| [ ] | 1.9 | **+ Reading**: tick **↺ Reset** on Oil life (leave the value it fills) → **Save** | Ticking fills `100`, and a line says "Oil life was reset: 70 → 100 %." Saved; the timeline shows "100 % ↺". |
| [ ] | 1.10 | Edit that reading (while it's a draft) | **↺ Reset** is still ticked, with 100. Cancel. |
| [ ] | 1.11 | **Mac browser**, Maintainer mode: edit **Oil life**, **Counts: Up** → **Save** | Refused: "Which way a meter counts can't change once it has a reading." |
| [ ] | 1.12 | Edit **Oil life**: reset value `95` → **Save** | Saved; Meters says "resets to 95 %". The readings already logged are unchanged. |
| [ ] | 1.13 | `Test truck` → **+ Log** → **Maintenance**: Oil life at the start `90`, a task → **Save** as a draft | Saved; the job's page shows Oil life "90 %". (A job's *end* readings get their reset with milestone 2.) |
| [ ] | 1.14 | **Desktop:** unlock, Sync now; `Test truck` → **Setup** and **Timeline** | Oil life, with its summary, and every reading of steps 1.5–1.13, the ↺ ones marked. |
| [ ] | 1.15 | **Desktop, Tailscale off** (wait for **OFFLINE**): **+ Reading**: Oil life `95` with **Reset not ticked** → **Save** | Refused on the desktop itself, with **↺ It was a reset**. |
| [ ] | 1.16 | Tick **↺ Reset** (it fills `95`) → **Save**; then **Tailscale on**, Sync now; **phone** (reload) | Saved on the desktop with **↑ 1 to send**; after syncing, on the phone with "95 % ↺". |

**Milestone 1 also checks:** the release fixtures' migration (`v0.2.0`, `v0.3.0`) and the reading rules, in the automated tests.

## Results

# Human Notes
