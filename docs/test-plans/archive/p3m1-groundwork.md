# Manual test plan: Phase 3 milestone 1, groundwork

*Phase 3 milestone 1 · branch `milestone/p3-m1-groundwork` · `docs/phase_plans/phase-3.md` · GitHub #19, #37 · sub-plan 04 §7.1, §7.4*

**Result (2026-11-09): passed,** on `:milestone-p3-m1-groundwork` (commit `2423703`), with two exceptions moved to the Polish phase (`docs/plan.md` §9 Phase 6): step 8 partly (the window's place is restored, its size isn't, though a maximized window reopens maximized), and step 10 not run (no second monitor). See the human's notes below.

## 1. Intended behavior

- **App icon:** the gauge (blue tile, white dial, orange needle) in the browser tab, on an iPhone's home screen when the web app is added there, and on the desktop window in Windows and Linux. On a Mac, the Dock icon comes from the app bundle (`dx bundle`); a `dx serve` run keeps the generic one (setting it from code would need `unsafe`, which the workspace forbids).
- **Title:** always "Vehicle Manager"; on the test server, "test server · Vehicle Manager".
- **Exact build (#37):** Diagnostics → About shows **Build** (tag · commit · date, or "local build · commit" for a build from your Mac) and, on the desktop, **Server** (the server's version and build). The server's first log line says the same.
- **Desktop window (#19):** reopens at its last size and position (and maximized or full screen if it was); a place that can't be seen moves to the middle of the main screen. **Settings → This device → Keep window on top**, off by default.
- **Secrets:** passwords, codes, and tokens never appear in the logs. The setup and recovery codes still appear, on purpose, in the container's console only.
- **Nothing else changes:** signing in, Admin mode, logging, photos, and documents work as before.

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m1-groundwork` finished successfully (commit `2423703` or later).
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m1-groundwork` in its `.env`, then in its folder:

  ```
  sudo docker compose pull
  sudo docker compose up -d
  ```

- **The desktop app,** on the Mac, from the repository on this branch:

  ```
  cd crates/app
  dx serve --platform desktop
  ```

  Its server address set to the test server (Diagnostics → Connection), then a restart.
- A phone (iPhone) on the tailnet, for step 4.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep "app starting"` | `app starting version=0.2.0 build=milestone-p3-m1-groundwork · <commit> · <date> UTC`. |
| [x] | 2 | **Mac browser:** open the test server | The tab shows the **gauge icon** and the title **"test server · Vehicle Manager"**. Sign in as usual. |
| [x] | 3 | **Diagnostics → About** | **App** `0.2.0`; **Build** the same line as step 1; no **Server** row (the page and the server are one build). |
| [x] | 4 | **iPhone:** open the test server in Safari → Share → **Add to Home Screen** | The home-screen icon is the gauge. (Remove it afterwards if you like.) |
| [x] | 5 | **Main server** (no port), in another tab | Still on `0.2.0`, title **"Vehicle Manager"**. Its Diagnostics has no Build row: that came after `0.2.0`, so the main server shows it from the next release. |
| [x] | 6 | **Desktop app:** look at the window | Title **"test server · Vehicle Manager"** once signed in. (The Dock icon is the generic one under `dx serve`; see section 1.) |
| [x] | 7 | **Desktop:** Diagnostics → About | **Build** `local build · <commit>` (plus "uncommitted changes" if you have any); **Server** `0.2.0 · milestone-p3-m1-groundwork · …`, matching step 1. |
| [~] | 8 | **Desktop:** move the window to another spot and make it a different size; quit (⌘Q); start it again | It opens where and how big it was. |
| [x] | 9 | **Desktop:** maximize it (double-click the title bar, or Window → Zoom); quit; start again; then un-maximize | It opens maximized; un-maximizing returns it to step 8's size and place. |
| N/A | 10 | *(If you have a second monitor)* move the window onto it; quit; unplug the monitor; start again | It opens in the middle of the Mac's own screen. |
| [x] | 11 | **Desktop:** Settings → **This device** → turn on **Keep window on top**; click another app's window | The Vehicle Manager window stays in front. Quit and start again: still on top. Turn it off: it no longer stays in front. |
| [x] | 12 | **Browser:** Settings → This device | No "Keep window on top" row (desktop only). |
| [x] | 13 | **Secrets:** sign out, sign in once with a wrong password, then properly. **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -ic "<the wrong password you typed>"` and the same with your real password | `0` and `0`. |
| [x] | 14 | **NAS:** `grep -c "<your real password>" data/logs/current.jsonl` in the test server's folder | `0`. |
| [x] | 15 | **Everything else:** Admin mode (enter with your password, leave); log a reading; open a photo and a PDF | As before. |
| [x] | 16 | Afterwards: the test server back on `:dev` (`VEHICLEMANAGER_IMAGE_TAG=dev`, `pull`, `up -d`) | Back on its usual image. |

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1`.
- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer (Minimum level: Debug).
- **The desktop's settings file:** `~/Library/Application Support/vehiclemanager/settings.json` (it holds the server address and the window's place; no passwords).
- **Never share** a password, `.env`, or a cookie.

# Human notes

- When re-launching the desktop app with `dx serve --platform desktop`, the location seems to be saved, but the size is not. Except, re-starting when it was quit maximized restarts maximized. Let's make note of that for the polish phase and move on for now; it is a very minor issue.
- #10 is N/A. No second monitor now. We can cintinue with the plan, but make note that second monitor testing should happen before the end of the Polish phase.
- Make a note for the Polish phase that the Settings page should be divided into tabs (column of tabs on the left sode of the page). I am also open to other ideas for organizing and decluttering the settings page as we add more settings. Either way, that should be in the Polish phase.
