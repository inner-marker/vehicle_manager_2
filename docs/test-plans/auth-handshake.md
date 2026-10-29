# Manual test plan: auth stub and version handshake

*Phase 1, milestone 2, step 7 · `docs/phase_plans/phase-1.md` · decision #024*

## 1. Intended behavior

- Every request to a path under `/api/` must carry:
  - `Authorization: Bearer <credential>` matching the server's `VEHICLEMANAGER_TEST_CREDENTIAL`;
  - `x-api-version: <n>` within what the server accepts (currently exactly `1`).
- **The credential is checked first, then the version.** Refusals carry a reason code:

  | Situation | Status | Reason |
  |---|---|---|
  | Server has no credential set | 503 | `auth-not-configured` |
  | No `Authorization` header | 401 | `missing-credential` |
  | Wrong credential, or not `Bearer` | 401 | `invalid-credential` |
  | No version header | 400 | `missing-api-version` |
  | Version not a whole number | 400 | `malformed-api-version` |
  | Version older than accepted | 426 | `client-too-old` |
  | Version newer than the server's | 400 | `client-too-new` |

- **Pages and assets** (anything not under `/api/`) always load, even with no credential configured.
- **Diagnostics → Connection panel:** type the credential and press **Run checks** to see:
  - **API version** and **Credential**, each *Pass*, *Fail*, or *Not checked*. Version shows *Not checked* when the credential fails, because it's checked second;
  - a translated reason when something fails;
  - on success, the server's app and API versions and the ping round-trip time.
- **The credential is never logged or shown.** The field is a password input, and the value isn't kept after a reload.

## 2. Setup

- On `milestone/p1-m2-auth-handshake` with step 7 in place. Pick any test credential (e.g. `garage-test-1`).
- From `crates/app`, in a terminal:

  ```sh
  VEHICLEMANAGER_TEST_CREDENTIAL=garage-test-1 dx serve --platform web
  ```

  Open the printed URL (usually `http://127.0.0.1:8080`) and go to **Diagnostics**.
- For steps 8–11 you also need `curl` in a second terminal. Use the port `dx serve` printed.

## 3. Checklist

| # | Step | Expected result |
|---|---|---|
| 1 | Look at the `dx serve` terminal output at startup | An `INFO auth: API credential configured env_var=VEHICLEMANAGER_TEST_CREDENTIAL` line. **The credential value appears nowhere.** |
| 2 | Open Diagnostics | A **Connection** panel above the log viewer: a "Test credential" field and a **Run checks** button. No results yet. |
| 3 | Leave the field empty and press **Run checks** | Credential: **Fail**; API version: **Not checked**; "The server didn't accept this credential." (the app sends an empty Bearer value). |
| 4 | Type `wrong` and press **Run checks** | Credential: **Fail**; API version: **Not checked**; "The server didn't accept this credential." |
| 5 | Type `garage-test-1` and press **Run checks** | Both **Pass**. Shows "Server app 0.1.0, API 1 (accepts 1–1)" and "Round trip: N ms". |
| 6 | While the checks run (press, then watch the button) | The button shows "Checking…" and is disabled until results appear. |
| 7 | Reload the page | The field is empty again; nothing was remembered. |
| 8 | `curl -i http://127.0.0.1:8080/api/v1/handshake` | `401`, body `{"code":401,"data":"missing-credential",...}` |
| 9 | `curl -i -H 'authorization: Bearer garage-test-1' -H 'x-api-version: 0' http://127.0.0.1:8080/api/v1/handshake` | `426`, `client-too-old` |
| 10 | Same as 9 with `x-api-version: 2` | `400`, `client-too-new` |
| 11 | Same as 9 with `x-api-version: 1` | `200` and JSON with `app_version`, `api_version`, `min_supported_api_version`, `server_time_ms` |
| 12 | Stop `dx serve` and restart it **without** the variable (`dx serve --platform web`), then repeat step 5 | Startup shows `WARN auth: no API credential configured; every API request will be refused`. The page loads, but Run checks shows Credential **Fail** with "The server has no credential configured, so it refuses every request." |

## 4. If a step fails, capture

- **Server terminal:** the `dx serve` output from startup to the failure, especially the `auth:` lines. Refusal reasons are logged on the `auth` area at `debug`, which is off by default and has no UI switch yet; the reason code in the response body (curl or the Network tab) tells the same story.
- **Browser devtools:** the console and the **Network** tab entry for `/api/v1/handshake`: its status, response body, and request headers. **Blank out the `authorization` value before sharing.**
- **curl steps:** the full `curl -i` output.
- **Screenshots** of the Connection panel results.
- **Don't share the real credential** in any of the above.
