# Security Sub-Plan

*Confirmed 2026-11-07 (decision #071; the open questions in §10 settled as proposed). Gathers GitHub issues #28 (Login and PIN), #29 (internet-facing hardening), #30 (device-bound session tokens), and #31 (encryption at rest), and places each item in a phase. Details `docs/plan.md` §6, Security (defense in depth), and Phase 7.*

## 1. Goals

- **Keep today's model:** a household server behind Tailscale, with the server as the authority on who may do what.
- **Be ready for what Phase 3 brings:** data stored on devices, and devices that stay signed in. A lost laptop or phone shouldn't expose the household's records or let someone sign in as its owner.
- **Allow an internet-facing server** (for example, a VPS) for those who choose it, with the app's own defenses plus a deployment guide.
- **Build each piece when it's cheapest:** what's hard to add later (the device database's format, the pairing protocol) goes in Phase 3; the rest waits for Phase 6 or Phase 7.

## 2. Where things stand (`v0.2.0`)

| Area | Today |
|---|---|
| Network | `tailscale serve` only; the server listens on loopback; no database port. |
| Sign-in | Username and password (argon2), HttpOnly Secure SameSite=Strict cookie (#028). The same generic error for a wrong username or password. |
| Failed sign-ins | `server::login_limiter`: 5 failures in 15 minutes per username and per client, in memory. |
| Sessions | Ends after 30 idle days on the server; the app signs out after a chosen idle time (default 8 hours, #049). A password change ends the person's other sessions. No list of sessions or devices. |
| Admin mode | Asks for the password; ends after 15 idle minutes (#049). |
| Recovery | An Admin resets anyone's password (#046); an Admin's own recovery uses a one-time code in the server log (#047). |
| Desktop credential | Typed in, kept in memory only (#027). |
| Hashing on the server | Argon2 runs on `spawn_blocking`, with no limit on how many run at once. *(Now at most 2 at once: §5.1, done.)* |
| Data at rest | Plain SQLite on the NAS; attachments stored as files named by their SHA-256 (#065); snapshots and backups unencrypted. |
| Logs | `tracing`, with no rule or type that keeps secrets out. |

**One gap found while reading:** the "per client" key for failed sign-ins is the **first `X-Forwarded-For` entry** (`app::api::server_side::client_key`). A client can send its own header, so on an internet-facing server it could change that key on every try and get past the per-client limit. (The per-username limit still holds.) **Fixed** in `fix/sign-in-hardening` (§5.1).

## 3. Who we defend against

| Threat | Where it matters | Main defenses |
|---|---|---|
| Someone on the home network | Every install | Tailscale only; nothing on the LAN (already done). |
| A lost or stolen device | Phase 3 onward (data on devices) | App lock (PIN), an encrypted device database, a token bound to the device, revoking a device. |
| A copied token | Desktop and mobile | The token kept in OS secure storage and bound to a device key (§6). |
| Password guessing and floods | Internet-facing servers mostly | Limits per account and per client, a cap on concurrent hashing, size and time limits, logs fail2ban can read (§5). |
| A stolen disk, backup, or snapshot | Every install | Encryption at rest on the server, and encrypted backups (§7). |
| Secrets leaking through logs | Every install | Secret types that can't be printed, and log tiers (§7.4). |

## 4. Sign-in, PIN, and lock (#28)

### 4.1 App lock on desktop and mobile (Phase 3)
- Once a device keeps a session and data (Phase 3), the app **locks after 15 idle minutes** (a setting, like "Sign out when idle"). Unlocking asks for a **PIN**; the password also works.
- **The PIN is per person and per device,** set when the device is paired. It works offline: it unlocks a key kept in OS secure storage (§6.1), and is never sent to the server.
- **PIN rules:** 6 digits or more; refuses repeats and runs (`111111`, `123456`) and the person's birth year if known. Proposed: **no shuffled keypad** on desktop (a keyboard is used); revisit for mobile.
- **Failed unlocks:** after 5 wrong PINs, the PIN stops working and the password is needed; after 10 wrong tries in all, the device signs out and forgets its local key (data can be synced again after signing in). Failed unlocks are kept and reported to the server at the next sync, for the Admin.
- **Forgot your PIN:** sign in with the password, then choose a new PIN.
- **The web app:** no PIN. It stays as now (cookie, idle sign-out), since a browser can't keep a key safely. See open question 1.
- **Biometrics** (Touch ID, Windows Hello): post-MVP, as the issue says.

### 4.2 Admin mode re-authentication (Phase 6)
- **Per Admin, a choice:** password (the default, as now) or PIN. The issue also lists "none"; proposed to **leave it out**, since Admin mode guards deleting records and changing roles. See open question 2.

### 4.3 Sessions and devices (Phase 3, with pairing)
- **Settings → Account → Devices:** each signed-in device and browser, its last use, and **Sign out** for one or **Sign out everywhere else**. An Admin can do the same for anyone from the Users screen.
- Built on the existing session table (`storage::users`, which can already end a person's sessions).

## 5. Internet-facing servers (#29)

### 5.1 In the app
| Item | Phase | Notes |
|---|---|---|
| **Use only the `X-Forwarded-For` entry the proxies wrote** | **Done** (`fix/sign-in-hardening`) | `VEHICLEMANAGER_PROXY_HOPS` (default 1, for `tailscale serve`): the entry that many places from the right is the client; too few entries is `direct` (`server::client_address`). Counting replaced the first idea, a list of trusted proxy addresses, because the server never sees the connection's own address (decision #071, amended). |
| **Cap concurrent password hashes** | **Done**, with the fix above | At most 2 at once, each waiting up to 10 seconds; then sign-in answers "try again in 5 seconds" (`server::passwords`). |
| **Request size and time limits** | **Done**, with the fix above | 2 MB bodies (uploads 51 MB) → 413; 1 minute to answer (uploads and downloads 10 minutes) → 408 (`server::limits`). |
| Rate limits on the whole API, per client and per account | Phase 6 | `tower_governor` or a small limiter of our own; sync traffic sets the numbers, so it waits for Phase 3. |
| Growing delays after repeated failures | Phase 6 | Extends `login_limiter`: 5 failures → 15 minutes as now, then doubling up to a day. Kept in the database so a restart doesn't clear it. |
| Failed sign-ins logged in a fixed format | Phase 6 | One line per failure with the client address, for fail2ban or CrowdSec; the deployment guide gives the filter. |
| Generic error for a wrong username or password | Done | Kept, and covered by a test. |
| Optional proof-of-work before the web app loads (Anubis or ALTCHA) | Post-MVP, off by default | Only protects the web app; the API limits above are still needed. |

### 5.2 In the deployment guide (Phase 6)
A new `docs/deploying-internet.md`: a reverse proxy with TLS (a sample Compose file with Caddy), a firewall that opens only HTTPS, fail2ban or CrowdSec with the filter from §5.1, upstream DDoS protection (Cloudflare or the VPS provider), and keeping the host and image updated. It says plainly that Tailscale is the recommended setup.

## 6. Devices and tokens (#30)

### 6.1 Secure storage (Phase 3, with pairing)
- The device token and the device's keys go in **OS secure storage** through the `keyring` crate (macOS Keychain, Windows Credential Manager, Linux Secret Service). Today the desktop keeps the session in memory only (#027), so nothing is on disk to protect yet.
- Where no secure storage exists (some Linux desktops), the key is wrapped with a key derived from the PIN (Argon2id), and the app says that's weaker. The same fallback as sub-plan 01 §5.

### 6.2 Tokens bound to the device (Phase 3, with pairing)
- **At pairing, the device makes a key pair** (Ed25519; in secure hardware where the platform allows it, otherwise in secure storage). The server stores the public key with the device's session.
- **Each request is signed** (DPoP-style, RFC 9449: the method, the path, the time, and a hash of the token). The server refuses a token without a valid signature from its device, and refuses signatures more than a few minutes old. A copied token alone is useless.
- **One device key for both uses:** the same per-device key (and the PIN that unlocks it) serves the signatures and records plan (sub-plan 01 §5, "Key cert, one per user+device"). Pairing makes the key once; sub-plan 01 later certifies it.
- **The web app keeps the cookie** (HttpOnly, Secure, SameSite=Strict), as the issue says.
- **Why in Phase 3:** pairing is designed there (an open Phase 3 question), and adding signing to a sync protocol after devices already use it would mean supporting both for a while.

## 7. Encryption at rest and secrets (#31)

### 7.1 Device databases (Phase 3)
- **The device database is SQLCipher from the start** (its first local database). Encrypting an existing plain database later is a migration on every device; starting encrypted costs little.
- **Two layers of keys:** a random database key, wrapped by a key in OS secure storage (and unlocked by the PIN, §4.1). Changing the PIN re-wraps the key; the database isn't rewritten.
- **To check first (a spike before the Phase 3 storage work):** sqlx with `libsqlite3-sys`'s bundled SQLCipher on macOS, Windows, and Linux, in CI, and its effect on build time and the binary's size.
- **Attachments on devices:** encrypted per file (§7.2).
- **The web app is online-only** with no stored data: `Cache-Control: no-store` on API responses, and anything kept in the browser is cleared at sign-out. (This also answers part of the Phase 3 question "Web app: local-first or online-only?"; see open question 3.)

### 7.2 The server (Phase 7)
- **The server database in SQLCipher,** its key from a Docker secret (`/run/secrets/db_key`) or, if none is set, a passphrase typed in at startup through the web app (the server waits, locked, until then). A wrong or missing key never starts the server with an empty database.
- **Attachments encrypted per file** (`age`), named on disk by an HMAC of their SHA-256 instead of the plain SHA-256. The **SHA-256 stays the attachment's ID in the database and in sync** (#065: duplicates and integrity checks), so this is a change to the file store only, and doesn't need to be decided before Phase 3's blob sync.
- **Snapshots and backups encrypted** with the same key, and restorable only with it.
- **A recovery key,** shown once at setup for the Admin to print or store (like sub-plan 01's root recovery code), and **key rotation** from Admin mode.
- **Moving an existing server** to encryption: a one-time, guided step in Admin mode, after a snapshot.

### 7.3 Deployment guide (Phase 7)
How to create the Docker secret on Synology, where to keep the recovery key, what happens if it's lost (the data can't be read), and how to rotate.

### 7.4 Secrets in logs (Phase 3, early)
- **Secret types:** passwords, tokens, PINs, codes, and keys are held in `secrecy::SecretString` (or a small `Redacted<T>`), which print as `[redacted]` in `Debug` and logs.
- **Log tiers:** normal logs never hold secrets, record contents, or full tokens (a token's first 8 characters of its hash at most). The one-time setup and recovery codes stay in the server log on purpose (#028, #047), marked as such.
- **A test** that signs in, recovers, and pairs with a capturing subscriber, and checks no secret appears in the output.
- Cheap and useful before devices start storing anything, so proposed early in Phase 3.

## 8. Custom roles (#27)

Custom roles aren't really a security feature: they change **who may do what**, not how it's protected. Proposed: **their own sub-plan later** (`05-custom-roles.md`), placed in **Phase 6**, for these reasons:
- Today's roles are fixed (`domain::access`: Driver, Maintainer, Admin, with modes and per-vehicle grants). Custom roles turn them into named sets of permissions, which then sync to devices as data. Doing that after sync works (Phase 3) avoids designing sync twice.
- The permission checks are already gathered in `AssetAction` and the "show only what the role allows" rule (#069), so the change is contained.
- The issue's own rules carry over: **Admin stays fixed with every permission**, a person can hold several roles, and the mode picker switches between them.

See open question 4.

## 9. Phasing

| When | What |
|---|---|
| **Now** (one `fix/` branch, before Phase 3 code) | **Done** (`fix/sign-in-hardening`): the client address from counted proxies; a cap on concurrent hashing; request size and time limits (§5.1). |
| **Phase 3, early** | Secret types and log tiers, with the test (§7.4). The SQLCipher spike (§7.1). |
| **Phase 3, with the device database and pairing** | SQLCipher device database; OS secure storage; the device key pair and signed requests; app lock with a PIN; the Devices list and revoking (§4.1, §4.3, §6, §7.1). Web online-only with `no-store`. |
| **Phase 6** | Admin-mode re-auth choice; API-wide rate limits; growing delays kept in the database; the fail2ban log format; the internet-facing deployment guide (§4.2, §5). Custom roles, in their own sub-plan (§8). |
| **Phase 7** | Server database and attachment encryption, encrypted snapshots and backups, the recovery key, rotation, and their guide (§7.2, §7.3); the audit, review, and penetration test the plan already lists. |
| **Post-MVP** | Biometrics; a proof-of-work gate; hardware keys (with sub-plan 01). |

## 10. Settled questions

Settled by the human on 2026-11-07, all as proposed (decision #071):

1. **No PIN on the web app.** The web keeps idle sign-out (#049); a shorter time can be chosen.
2. **No "none" for Admin-mode re-auth:** password (the default) or PIN.
3. **The web app is online-only** (§7.1). This also settles Phase 3's first open question.
4. **Custom roles (#27) get their own sub-plan, in Phase 6** (§8).
5. **The "now" fix goes first,** before the app icon (§5.1).
6. **After 10 wrong PINs** in all, the device signs out and forgets its local key (§4.1).
7. **Server encryption waits for Phase 7** (§7.2). Until then, the NAS deployment guide recommends a Synology encrypted shared folder for the data folder.
