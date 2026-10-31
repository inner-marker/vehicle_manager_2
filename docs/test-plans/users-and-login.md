# Manual test plan: users, sign-in, and roles

*Phase 2, milestone 2 · `docs/phase_plans/phase-2.md` · `docs/deploying-nas.md` "Create the first Admin" · decisions #028, #029, #045–#048*

## 1. Intended behavior

- **First Admin:** a server with no users writes a one-time **setup code** to its log at every start. Setup has two steps: the code alone, then the account. The new user is the household's **Admin**, and the code stops working.
- **Signing in:**
  - Every page except sign-in and setup needs a session. Sessions last 30 days without use; signing out ends one.
  - **Web:** an `HttpOnly` cookie. **Desktop:** a token kept in memory only, so each launch signs in again.
- **Modes:** every sign-in starts in **Driver** mode.
  - **Admin** mode asks for the password again. It shows an orange chip and banner (with **Leave**), and drops back to Driver after 15 minutes without activity.
  - Someone with only the Viewer role sees **Read-only**.
- **Attempt limit:** after 5 wrong attempts in 15 minutes, for one username or from one device, sign-in shows "Too many attempts. Try again in N minutes." Wrong setup and recovery codes count too. The limit lives in memory, so restarting the server clears it.
- **Users** (household Admin, in Admin mode):
  - Add a user with a role on all vehicles; the server makes a 12-character **temporary password**, shown once. They must choose their own at their next sign-in.
  - Change roles, disable and enable, and reset passwords. Disabling and resetting sign the user out everywhere.
  - **The last enabled Admin** can't be disabled or lose the Admin role.
- **Password recovery:** **Forgot your password?** always answers "Ask your Admin for further instructions." For an Admin account only, a one-time **recovery code** goes to the server log (valid 15 minutes, used once). The code and a new password sign them in and end their other sessions.
- **Settings:**
  - **You** tab: each setting follows the household default until you choose your own; **Use household default** goes back to following it.
  - **Household** tab: needs Admin mode.
  - Changes save at once and survive restarts. Values aren't formatted with them yet: units come in milestone 4, dates in milestone 6.
- **Secrets:** passwords, temporary passwords, and session tokens never appear in any log. The setup and recovery codes do, on purpose, and only those.
- **The test credential is gone** (#024 retired). The Connection panel's second check is now **Signed in**.

## 2. Setup

- **The milestone image:** after the branch is pushed, the "Dev image" workflow builds `milestone-p2-m2-users`. Check Actions → it finished successfully.
- **On the NAS over SSH:** get the milestone's `compose.yaml` (it no longer passes the credential) and edit `.env`:

  ```sh
  cd /volume1/docker/vehicle_manager
  curl -fsSLo compose.yaml https://raw.githubusercontent.com/inner-marker/vehicle_manager_2/milestone/p2-m2-users/deploy/compose.yaml
  vi .env
  ```

  In `.env`:
  - set `VEHICLEMANAGER_IMAGE_TAG=milestone-p2-m2-users`;
  - **delete** the `VEHICLEMANAGER_TEST_CREDENTIAL=` line.

  Then `Esc` and `:wq`.
- **Two browsers on the tailnet:** your usual one (the Admin, "stuart" below) and a **private window** or a second device (the second user, "sam" below).
- **The Mac** for the desktop steps (25–28): `dx` 0.7.10, Tailscale connected, and the NAS address saved from `docs/test-plans/desktop-macos.md` (Diagnostics → Server address). Run the app from `crates/app` with `dx serve --platform desktop`. "Restart the app" means close the window, Ctrl+C, and run it again.
- **Write down** each password as you choose it. **Don't paste any password into a shell command**; step 29 checks the log without one.
- **Timing:** step 12 needs 16 idle minutes, and step 14 locks out one device for 15 minutes (or until a restart). Steps 30–31 need a server restart.

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | `sudo docker compose pull && sudo docker compose up -d`, then `sudo docker logs --tail 40 vehiclemanager` | `backed up before migrating` with `schema_before=1`, then `database open` with `created=false`, `schema_before=1 schema_now=3`, and the **same database ID** as milestone 1. `database integrity check passed`. A `WARN` line ending `setup_code=XXXX-XXXX-XXXX-XXXX`. No `ERROR` lines and nothing about a credential. |
| [x] | 2 | `ls -ln data/backups/` | One `pre-migration-<UTC time>-v1.sqlite`, owned by `1030 100`. |
| [x] | 3 | Open the app's address in the browser | **Set up this server**, STEP 1 OF 2, asking only for the setup code. No other page is reachable. |
| [x] | 4 | Enter `AAAA-AAAA-AAAA-AAAA` → **Next** | "That code isn't right." Still step 1. |
| [x] | 5 | Enter the real code **in lowercase** → **Next** | STEP 2 OF 2: "✓ Code accepted. You'll be the Admin." Fields for username, name, and the password twice. |
| [x] | 6 | Choose username `stuart`, your name, and a password; first try one of 11 characters, then two that differ, then a real one → **Create Admin and sign in** | First: "A password needs at least 12 characters…". Then: "The two passwords aren't the same." Then you're in the **Garage**, with chips **Driver** and your name. The sidebar shows **Users**. |
| [x] | 7 | `sudo docker compose restart`, then `sudo docker logs --tail 20 vehiclemanager \| grep -i "setup code"` | Nothing: setup is closed once an Admin exists. Reload the browser: you're still signed in. |
| [x] | 8 | Diagnostics → **Run checks** | API version, **Signed in**, and Server health all **Pass**. |
| [x] | 9 | **Driver ▾** → **Admin** → a wrong password | "That password isn't right." Still Driver. |
| [x] | 10 | **Driver ▾** → **Admin** → your password | Orange **Admin** chip and the banner "ADMIN MODE · Changes affect the whole household · ends after 15 idle minutes" with **Leave**. |
| [x] | 11 | **Leave** | Back to **Driver**; the banner is gone. |
| [x] | 12 | Enter Admin mode again, leave the tab alone for **16 minutes**, then click **Garage** | Back in **Driver**; no banner. |
| [x] | 13 | In Admin mode: **Users** → **+ Add user**: username `sam`, a name, role **Maintainer** → **Add user** | "Sam's temporary password" in groups of 4 (`XXXX-XXXX-XXXX`), with **Copy**. "It's shown only this once." Sam's row shows **Temporary password**. |
| [x] | 14 | In the **private window**: sign in as `sam` with a wrong password **6 times** | Tries 1–5: "Wrong username or password." Try 6: "Too many attempts. Try again in 15 minutes." Then even the **right** temporary password is refused with the same message. |
| [x] | 15 | `sudo docker compose restart` (clears the limit). In the private window, sign in as `sam` with the temporary password | **Choose a new password**: "Your Admin gave you a temporary password…". Nothing else is reachable until Sam chooses one. |
| [x] | 16 | Choose Sam's new password | Sam is in the Garage in **Driver** mode. The mode menu offers **Driver** and **Maintainer**, **not Admin**. There's **no Users** item in the sidebar. |
| [x] | 17 | As Sam, open `/users` by typing it in the address bar | No user list: "Switch to Admin mode to manage users." |
| [x] | 18 | As stuart in **Driver** mode, open **Users** | "Switch to Admin mode to manage users." No list. |
| [x] | 19 | As stuart in Admin mode: open your own page in Users | Role shown greyed out with "The last Admin can't be disabled or lose the Admin role…". No **Disable…** button. |
| [x] | 20 | Open Sam's page → change the role to **Viewer**. Then, as Sam, click **Garage** | Sam's mode chip shows **Read-only**. |
| [x] | 21 | As stuart: Sam's page → **Disable…** → "Disable Sam?" → **Disable**. Then, as Sam, click any page | Sam's row shows **Disabled**. Sam is sent to **Sign in**; signing in again gives "Wrong username or password." |
| [x] | 22 | As stuart: **Enable**, then **Reset password…** → **Reset password** | A new temporary password is shown once. Sam signs in with it and must choose a new password again (as in 15–16). |
| [x] | 23 | As Sam: account chip → **Change password** with a wrong current password, then the right one | First: "Your current password isn't right." Then: "Password changed. You were signed out on your other devices." |
| [x] | 24 | As Sam: account chip → **Sign out**. Then use the browser's **Back** button | **Sign in** page; Back doesn't show Sam's pages. Close the private window. |
| [x] | 25 | **Mac:** start the desktop app | **Sign in**, not setup (the NAS already has an Admin). |
| [x] | 26 | Sign in as stuart | Garage in **Driver** mode. Diagnostics → **Run checks**: all **Pass**. |
| [x] | 27 | Enter Admin mode in the desktop app → **Settings** → **Household** → **Kilometers** | Kilometers is selected. In the browser (still stuart), Settings → **You**: Kilometers shows as the household's value, with **Use household default** ticked. |
| [x] | 28 | Restart the desktop app | **Sign in** again (the token was in memory only). `grep -ril "token\|password" ~/Library/Application\ Support/vehiclemanager/settings.json` prints nothing. |
| [x] | 29 | **Browser, stuart:** account chip → **Sign out** → **Forgot your password?** → username `nobody` → **Continue**. Then `sudo docker logs --since 5m vehiclemanager \| grep -i "recovery code"` | "Ask your Admin for further instructions." with a code form. **No** recovery code in the log. |
| [x] | 30 | ← Back → **Forgot your password?** → `stuart` → **Continue**, then the same `grep` | The **same** answer on screen. One `WARN` line with `recovery_code=XXXX-XXXX-XXXX-XXXX`. |
| [x] | 31 | Enter `AAAA-AAAA-AAAA-AAAA` and a new password, then the real code (lowercase) and a new password → **Set password and sign in** | First: "That code isn't right, or it has expired." Then you're signed in to the Garage. The desktop app's session is gone (its next click goes to Sign in). |
| [x] | 32 | Try the same recovery code once more (sign out first) | "That code isn't right, or it has expired." |
| [x] | 33 | **Settings → You:** choose **Kilometers**, **Liters**, **bar**, and **2026-10-30** | Each is your own choice (filled), and its **Use household default** is unticked. |
| [x] | 34 | Tick **Use household default** for Distance | Distance goes back to following the household (Kilometers since step 27, outlined). |
| [x] | 35 | Settings → **Household** in Driver mode | "Switch to Admin mode to change the household defaults." Only the theme row below. |
| [x] | 36 | `sudo docker compose restart`, sign in, open Settings → You | Everything from 33–34 is as you left it. |
| [x] | 37 | **Secrets check:** `sudo docker logs vehiclemanager 2>&1 \| grep -ic "password="` and `sudo docker logs vehiclemanager 2>&1 \| grep -ic "token="` | `0` both times. Then look through `sudo docker logs --tail 200 vehiclemanager` for anything that looks like a password or temporary password: none. Only `setup_code=` and `recovery_code=` lines carry codes. |

## 4. If a step fails, capture

- **Container log:** `sudo docker logs --since 30m vehiclemanager`. Sign-in, sessions, setup, and recovery are on the `auth` area; database and migration messages on `database`. Before sharing it, blank out any setup or recovery code that's still valid.
- **Log files:** `data/logs/current.jsonl` for the test's time range.
- **Browser:** a screenshot of the page, and the failing request in DevTools → Network (status and response body). **Don't share cookie values.**
- **Desktop:** `~/Library/Application Support/vehiclemanager/logs/current.jsonl` for the time range, and the `dx serve` terminal output.
- **Never share** a password, a temporary password, `.env`, or a cookie.

**After the test:** Sam can stay (disabled, if you like) or be used for later milestones. To start over completely, stop the container, move `data/vehiclemanager.sqlite*` aside, and start it: setup begins again with a new code.
