# Manual test plan: macOS desktop app over Tailscale

*Phase 1, milestone 4 · `docs/phase_plans/phase-1.md` · decisions #024, #027*

## 1. Intended behavior

- **The desktop app runs on the Mac** with `dx serve --platform desktop`. Packages come with the first release (#027).
- **Data folder:** `~/Library/Application Support/vehiclemanager`, named from `DATA_DIR_NAME`, never the display name. It holds:
  - `logs/`: rotating JSON-lines logs (5 × 10 MB, 30 days), plus `last-crash.txt` after a crash;
  - `settings.json`: the server address only.
- **Server address:** Diagnostics → Connection → **Server address**.
  - **Accepted:** `https://<host>[:port]`, or `http://` only for this computer (`localhost`, `127.x.x.x`, `[::1]`).
  - **Refused, with a message for the rule broken:** empty; no `https://`; `http://` to another device; a bad server name or port; a path, query, or `user@`.
  - **Applies at the next launch,** not while running: Dioxus sets the address once per run. The panel says "Using <address>" for the address this run uses.
  - With nothing saved, the app uses the local development server that `dx serve` starts (`http://127.0.0.1:<port>`, usually 8080).
  - A broken or hand-edited `settings.json` is ignored with a warning; the app still starts.
- **Credential:** typed in each run, kept in memory only. It's never saved and never logged.
- **Phase 1 exit criterion (desktop half):** the desktop app on the Mac reaches the NAS over Tailscale and shows the handshake result. A wrong credential is rejected cleanly.
  - *An incompatible API version* can't be produced from the UI. It's covered by the automated tests (`server::auth`, the Connection panel's `version_refusal_passes_auth_and_fails_version`) and was checked with `curl` in `docs/test-plans/p1m2-auth-handshake.md`.

## 2. Setup

- **Mac:** on `milestone/p1-m4-desktop` with steps 10a and 10b. `dx` 0.7.10 installed. Tailscale connected.
- **NAS:** the server running and published with `tailscale serve`, as in `docs/test-plans/p1m3-nas-web.md`. You need the NAS credential (`grep TEST_CREDENTIAL .env` on the NAS; keep it private).
- **Start clean:** quit the app, then move any earlier data folder aside:

  ```sh
  mv ~/Library/Application\ Support/vehiclemanager ~/Library/Application\ Support/vehiclemanager-before-test 2>/dev/null
  ```

- **Run the app** from `crates/app`. "Restart the app" below means: close the window, stop `dx` with Ctrl+C, and run this again (hot reload doesn't restart it):

  ```sh
  dx serve --platform desktop
  ```

## 3. Checklist

| # | Step | Expected result |
|---|---|---|
| 1 | Start the app | The window opens on Garage, in the Dark theme. The terminal shows `writing log files` with `log_dir=…/Library/Application Support/vehiclemanager/logs` and `app starting`. |
| 2 | `ls ~/Library/Application\ Support/vehiclemanager/logs` | `current.jsonl` exists. |
| 3 | Go to Diagnostics | The Connection panel shows a **Server address** field and "Using http://127.0.0.1:<port>" (the local development server `dx` started). |
| 4 | Enter `nas.example.ts.net` (no scheme) → **Save address** | "The address must start with https:// (or http:// for this computer)." No `settings.json` is created. |
| 5 | Enter `http://<your-nas>.<tailnet>.ts.net` → **Save address** | "Use https:// for a server on another device. http:// is only for this computer." |
| 6 | Enter `https://<your-nas>.<tailnet>.ts.net/app` → **Save address** | "Enter only the server's address, without a path, query, or login." |
| 7 | Enter `https://<your-nas>.<tailnet>.ts.net/` (trailing `/`, any capitals) → **Save address** | "Saved. Restart the app to use the new address." The "Using" line still shows the local development server. |
| 8 | `cat ~/Library/Application\ Support/vehiclemanager/settings.json` | Only `server_address`, lowercase and without the trailing `/`. No credential. |
| 9 | Restart the app, then go to Diagnostics | "Using https://<your-nas>.<tailnet>.ts.net". The terminal shows `using the saved server address`. |
| 10 | Enter a **wrong** credential → **Run checks** | Credential **Fail**, API version **Not checked**, "The server didn't accept this credential." |
| 11 | Enter the **right** credential → **Run checks** | Both **Pass**; "Server app 0.1.0, API 1 (accepts 1–1)"; a round-trip time. |
| 12 | Restart the app → Diagnostics → **Run checks** with the field left empty | The address is still the NAS (step 9's line). The credential field is empty. Credential **Fail**, "The server didn't accept this credential." |
| 13 | Turn **Tailscale off** on the Mac → **Run checks** with the right credential | Both **Not checked**, "Couldn't reach the server." The app doesn't hang or crash. Turn Tailscale back on. |
| 14 | Quit the app. Replace `settings.json` with `{ not json` and start the app | It starts normally. The terminal shows `settings file is invalid; using defaults`, and Diagnostics shows the local development server again (as in step 3). |
| 15 | Save the NAS address again (step 7), restart, and repeat step 11 | Both **Pass** again. |
| 16 | `grep -c '<the credential>' ~/Library/Application\ Support/vehiclemanager/logs/*.jsonl ~/Library/Application\ Support/vehiclemanager/settings.json` | `0` for every file. **The credential appears nowhere.** Use your shell's history settings (or clear the line afterwards) if you'd rather not leave it in history. |

**After the test:** to go back to the local development server, delete `settings.json`. To restore the earlier data folder, move `vehiclemanager-before-test` back.

## 4. If a step fails, capture

- **App log:** `~/Library/Application Support/vehiclemanager/logs/current.jsonl`, only the lines from the test's time range, or the Diagnostics log viewer set to **Debug** and the **app** area. Settings and address messages are on the `app` area; the connection checks log on `auth` at `debug`.
- **Terminal output** from `dx serve`, from start-up to the failure.
- **`settings.json`** (it holds only the address).
- **Steps 10–13 and 15:** a screenshot of the Connection panel, and the NAS's container log for the same minute (`sudo docker logs --since 5m vehiclemanager`).
- **Crash:** `~/Library/Application Support/vehiclemanager/logs/last-crash.txt`, if it exists.
- **Check that the credential isn't in anything before sharing it.** Blank out your tailnet name if you'd rather not share it.
