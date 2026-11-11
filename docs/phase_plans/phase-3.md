# Phase 3: Local-first & sync (plan)

*Agreed 2026-11-08 · Roadmap: `docs/plan.md` §9 Phase 3 · Open questions: settled (decisions #071, #072) · Security: `docs/sub-plans/04-security.md`*

**Goal:** the **desktop app works offline.** It keeps an encrypted copy of the household's data on the computer, logs fuel-ups, readings, jobs, and photos with no connection, and syncs when it's back on the tailnet. A lost laptop can be locked and signed out, and a damaged or reset server can never damage the desktop's copy.

**Who it's for:** the desktop app only. The web app stays online-only (#071), and the phone apps are post-MVP (#041); they'll reuse this phase's sync engine unchanged.

**Not in Phase 3:** what's due and fuel economy (Phase 5); custom roles, API-wide rate limits, and the internet-facing guide (Phase 6, #071); server-side encryption (Phase 7, #071); the phone apps (post-MVP).

**Exit criteria:**
- **Offline logging:** on the laptop, with Wi-Fi off, log a fuel-up with a photo. Turn Wi-Fi on: within a minute it's on the web, photo and all.
- **Edits merge:** edit one draft on the web and on the laptop (offline) in different fields; after syncing, both edits are kept. The same field: the later edit wins, by its clock (#043).
- **A rejected change is never lost:** an offline reading lower than one synced meanwhile shows in Messages to resolve; nothing is dropped.
- **A lost laptop:** signed out from **Settings → Devices** on the web, it can't sync, and a copied token is refused. Its local data stays encrypted, and the app is locked by its PIN.
- **Loss detection:** restore the server from an old snapshot; the laptop goes into **protective mode**, loses nothing, and can rebuild the server from its copy.
- **Old and new apps:** an app one API version behind still syncs; two behind is told to update and keeps its unsent changes.

**How this file works:** the same as `phase-2.md` (decision #021).
- One branch and **one PR into `dev` per milestone** (`milestone/p3-mN-<slug>`), merged with a merge commit.
- Each **step** is one Conventional Commit. Steps are planned in detail, and approved, when their milestone starts.
- Screens get **SVG layout options** in `docs/ui-sketches/` before they're built (#040).
- Each milestone has a test plan in `docs/test-plans/`, run on the test server (#062) with the desktop app.
- `cargo clean` at each step's end, once its checks have run (the human's disk is small).
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | What it delivers | You can check | Likely new dependencies (approved per step) |
|---|---|---|---|---|---|
| 1 | **Groundwork** | ✅ | The **app icon** (desktop window and browser tab); the **exact build on Diagnostics** (commit, image tag, build date; GitHub #37); **window title and launch settings** (window size and position remembered, always-on-top option; #19); **secret types and log tiers** with a test that no secret reaches a log (sub-plan 04 §7.4); the **SQLCipher spike**: an encrypted SQLite database through sqlx on macOS, Windows, and Linux in CI, with its effect on build time and size (sub-plan 04 §7.1) | The icon in the Dock and the tab; Diagnostics shows `milestone-p3-m1-…` and its commit; the window reopens where it was | `secrecy`; SQLCipher via `libsqlite3-sys` (spike only) |
| 2 | **Devices & pairing** | 🔨 | **Recent servers:** `settings.json` keeps a list (address, name, TEST tag, database ID, last used), shown as choices at sign-in plus "Another server…"; switching restarts the app (#027). **Signing in pairs the desktop** (#072): it makes a device key (Ed25519) kept in the OS keychain, gets a device token tied to it (kept in the keychain, not memory), and **signs every request** (DPoP-style); the server refuses the token without the signature. **Settings → Devices:** each device and browser, its last use, sign one out or all others; the Admin can do the same from Users | Pair the Mac; quit and reopen without signing in; copy the token to `curl` and see it refused; sign the Mac out from the web | `keyring`, `ed25519-dalek` |
| 3 | **App lock & PIN** | ⬜ | A **PIN per person per device**, set after pairing (6+ digits, no repeats or runs); the app **locks after 15 idle minutes** (a setting); the password also unlocks; **5 wrong PINs** → password needed, **10** → signed out and the local key forgotten; failed unlocks reported to the Admin at the next sync; forgot-PIN by password (sub-plan 04 §4.1) | Lock, unlock with the PIN; five wrong PINs ask for the password | none expected (Argon2 already in use) |
| 4 | **Local database & pull** | ⬜ | The **desktop database:** SQLCipher, one per server, in a folder named by the server's database ID; its key wrapped by the keychain and the PIN. **Pull:** the first full download, then records changed since the device's cursor, with tombstones (kept forever, #072). **One data layer** for screens: the web calls the server as now, the desktop reads its own database, so the desktop works offline **for reading**. Migrations run on the desktop database too | With Wi-Fi off, the desktop still shows the Garage, timelines, and documents; a change on the web appears on the desktop after reconnecting | SQLCipher (per the spike) |
| 5 | **Outbox & push** | ⬜ | **Offline changes:** each change is checked by the same `domain` rules, written locally, and added to an **outbox**; **push** in order, never applied twice, last-write-wins per field by HLC (#043); the server re-checks everything; **rejected changes go to Messages** to resolve; the outbox is pruned once acknowledged. **Versions:** the server accepts the current API version and the one before, upcasting older commands; older apps are told to update and keep their outbox (#072). A clock far in the future is refused and flagged | Log offline, reconnect, see it on the web; make the lower-reading conflict and resolve it from Messages | none expected |
| 6 | **Files** | ⬜ | **Downloads:** thumbnails and PDFs automatically; photos when opened, then kept; originals only on request (#072). **Uploads:** photos and PDFs added offline are queued and uploaded before the change that uses them. **Erased files keep an audit record** on the server: the file's and thumbnail's SHA-256, size, type, name, the record it belonged to, who removed it and when, and when the file was erased (#072) | Open a registration PDF offline; add a photo offline and see it on the web after reconnecting | none expected |
| 7 | **Loss detection** | ⬜ | **Protective mode** when the server looks reset, rolled back, or unhealthy (database ID changed, sequence lower than the device's cursor, or an unhealthy handshake); **suspicious-pull confirmation** (a pull that would delete much more than usual waits for a person); **snapshots before each sync** on the desktop; **"Rebuild the server from this device"**; sync alerts (rejections, protective mode, server unhealthy) in Messages (plan §6, Data integrity & loss detection) | Restore the test server from an old snapshot; the Mac goes into protective mode and keeps everything; rebuild the server from the Mac | none expected |

**Releases:** **`v0.3.0` at the end of Phase 3** (`docs/releasing.md`), with the same regression pass as `v0.2.0` plus the exit criteria above. The first **desktop packages** may come with it, which raises the Phase 6 question of signing them (#041); asked when the release is planned.

---

## Why this order

- **1 first:** the icon and the exact build were asked for before more desktop use, and every later test plan can then confirm its image from the app. Secret types come before the desktop starts storing secrets, and the spike settles the encrypted database before milestone 4 depends on it.
- **2 before 3:** the PIN unlocks the device's key, so the key (pairing) comes first.
- **3 before 4:** the local database's key is wrapped by the PIN, so the lock exists before there's data to lock.
- **4 before 5:** reading locally (pull) is simpler and proves the data layer; writing (push) builds on it.
- **6 after 5:** a file added offline is uploaded before the change that uses it, so it needs the outbox.
- **7 last, but before the release:** loss detection guards the desktop's copy against a damaged server, which only matters once that copy exists; it ships with sync, never after (plan §9).

## To decide when each milestone is planned

- **Milestone 1:** the icon's design (SVG options first, #040); where the build information comes from outside CI (a `build.rs` reading `git`, or "local build").
- **Milestone 2:** how the request signature is formed (DPoP's JWT form, or a smaller one of our own); how long a signature is valid; what Settings → Devices shows for a browser session.
- **Milestone 4:** the shape of the data layer (one trait with a server and a local implementation, or per-screen functions); how a first download shows its progress; how the desktop's storage crate is gated so the web build never pulls in sqlx (plan §3).
- **Milestone 5:** what Messages offers for each kind of rejected change; how long the outbox may grow before the app warns.
- **Milestone 7:** the thresholds for "suspicious"; how "rebuild the server" proves the device's copy is complete.
- **Draft sub-plans that touch Phase 3:** signatures and records (`01`: the device key from milestone 2 is the one it later certifies), and security (`04`, confirmed: §4, §6, §7.1, §7.4 land here). Each is raised before the milestone it affects.
- **Open issues** are reviewed with each milestone's plan (AGENTS.md); #24 and #26 stay open for a later phase.

---

## Step details

*Detailed plans are added here as each milestone is planned and approved. Steps are numbered per milestone (1.0, 1a, …).*

### Milestone 1 plan: groundwork

*Agreed 2026-11-08 · branch `milestone/p3-m1-groundwork` · sketches: `docs/ui-sketches/p3m1-icon-options.svg`, `p3m1-icon-gauge-options.svg`*

**Choices (the human, 2026-11-08):**
- **Icon:** the gauge, as drawn (**A1**): a blue tile, a white dial with three ticks, an orange needle, and an odometer window below.
- **Window (#19):** the desktop reopens at its last size and position (and maximized or full screen), moved back on screen if its monitor is gone; **Settings → You → "Keep window on top"**, desktop only, off by default.
- **Title (#19):** always "Vehicle Manager"; the test server keeps its TEST SERVER prefix.
- **Exact build (#37):** on **Diagnostics → About** only.

| Step | Status | What |
|---|---|---|
| **1.0** `docs:` | ☑️ | This plan; the icon sheets. |
| **1a** `feat(app):` icon | ☑️ | The A1 gauge as one master SVG (`crates/app/assets/`); PNGs at 16, 32, 180, 192, and 512 px made from it by an ignored test using `resvg` (test-only, like `make_fixture`) and committed. **Web:** the favicon (SVG, PNG fallbacks, Apple touch icon). **Desktop:** the window icon (Windows, Linux); the Dock icon on macOS for `dx serve` runs (Dioxus's `with_icon` sets only a window icon, which macOS ignores; a macOS-only call, checked when the step starts); the bundle icon in `Dioxus.toml` for release packages. |
| **1b** `feat(app):` title and window (#19) | ☑️ | The page and window title "Vehicle Manager". The desktop's window size, position, and maximized or full-screen state saved in `settings.json` and restored, back on screen if needed. "Keep window on top" in Settings → You on the desktop, saved per computer in `settings.json` (not synced). |
| **1c** `feat:` exact build (#37) | ☑️ | The image workflows pass the commit, image tag, and build date as Docker build arguments, declared just before the Rust build so dependency layers stay cached; the binary keeps them. Local builds take the commit from `git` (marked "modified" with uncommitted changes), or "local build". The handshake carries the server's build; Diagnostics → About shows it, and on the desktop the app's own build too. Tests for the fallbacks. |
| **1d** `feat:` secret types and log tiers | ☑️ | Passwords, tokens, PINs, and codes held in `secrecy::SecretString` (or the token's own type) from the moment a server function receives them, printing as `[redacted]`. A logging rule in AGENTS.md: what each level may contain. A test captures the logs through setup, sign-in, a failed sign-in, a password change, and a recovery, and checks no secret appears (the setup and recovery codes stay in the log on purpose, #028, #047). Sub-plan 04 §7.4. |
| **1e** `ci:` SQLCipher spike | ☑️ | `spikes/sqlcipher/`, **outside the workspace** so its SQLCipher build never mixes into the server's: creates an encrypted database through sqlx, reopens it with the right key, and checks that a wrong key fails and the file isn't plain SQLite. A manually started workflow runs it on macOS, Windows, and Linux, with the system's crypto and with bundled OpenSSL, recording build time and size. The results are written here for milestone 4. Sub-plan 04 §7.1. |
| **1f** `docs:` | ☑️ | The test plan (`docs/test-plans/archive/p3m1-groundwork.md`), run on the test server with the desktop app. |

**New dependencies (approved with this plan):** `resvg` (dev only), `secrecy`; SQLCipher in the spike only; `objc2-app-kit` only if the Dock icon needs it (already in the build through Dioxus).

**Step 1a notes (2026-11-08):**
- **No Dock icon from code on macOS:** setting it needs `NSApplication::setApplicationIconImage`, which is `unsafe`, and the workspace forbids `unsafe` code. A Mac gets the icon from the app bundle (`dx bundle --platform desktop`); a plain `dx serve` run keeps the generic one. Windows and Linux get it as the window icon. So `objc2-app-kit` wasn't added.
- **`crates/app/Dioxus.toml`** (new): the bundle's identifier (the bundle ID, #016), its icons **largest first** (dx 0.7.10 makes every macOS size from the first one, so a small first icon came out blurry), and a **publisher, "Inner Marker"** (`dx bundle` requires one; taken from the bundle ID's owner). `tests/make_icons.rs` checks the identifier and that each icon exists, and that the committed PNGs still match the SVG (within a shade, since drawing differs slightly between CPUs).
- **For the release packages (later):** the bundle is still named `App.app` after the crate; its display name ("Vehicle Manager") is set when packages are planned. The icon fills its whole square, so in the Mac Dock it looks a little larger than Apple's own icons, which leave a margin; a padded Mac version can come with the packages.

**Step 1b notes (2026-11-08):**
- **Title:** the page and window title is "Vehicle Manager"; on the test server, "test server · Vehicle Manager" (set once the app knows the server's instance; before that, `Dioxus.toml`'s starting title). The title moved from the TEST SERVER tag to the app's root, so it's set in one place.
- **Window:** `settings.json` keeps the window's place (logical pixels, so it survives a screen with another scale) and "keep on top". The place is noted on every move or resize and written at most once a second, and again when the window loses focus, closes, or the app quits. While maximized or full screen, the normal place is kept, so leaving those returns to it. A place that can't be seen (less than a 120 × 40 strip of the title bar on any monitor) moves to the middle of the main monitor.
- **"Keep window on top"** is in Settings → **This device** (beside Theme), not "You": it belongs to the computer, not the person.

**Step 1c notes (2026-11-08):**
- **`domain::build_info::BuildInfo`:** the commit (shortened to 7), "modified", the image tag, and the build date; each optional, and an odd value (not hexadecimal, characters Docker tags don't allow, too long) is dropped rather than shown. One line: `dev · a885ac6 · 2026-11-08 19:02 UTC`, or `local build · a885ac6 · uncommitted changes`.
- **Where the values come from:** `server-image.yml` passes `BUILD_COMMIT` (the workflow's commit), `BUILD_TAG` (metadata-action's `version`: the branch's tag, or a release's version), and `BUILD_DATE`; the `Dockerfile` hands them to `dx bundle` as `VEHICLEMANAGER_BUILD_*`, declared just before that step. A local build's `crates/app/build.rs` asks `git` for the commit and uncommitted changes (rechecked when a commit is made or something is staged). Inside Docker there's no `.git`, so the image relies on the arguments. `deploy_config.rs` checks the workflow, the `Dockerfile`, and the names agree.
- **What shows:** Diagnostics → About gains **Build** (this app's own; the web app and its server share one, being built together) and **Server** (its version and build: always on the desktop, and on the web only when the server has moved on since the page loaded). The handshake carries the server's build (an optional field, so older apps ignore it), and the server's "app starting" log line names it too.

**Step 1d notes (2026-11-08):**
- **`domain::secret::Secret` instead of `secrecy`:** Dioxus sends server-function arguments as JSON both ways, and `secrecy`'s `SecretString` deliberately can't be serialized. `Secret` is a small type of our own: `Debug` prints `[redacted]`, there's no `Display`, the text is reached only through `.expose()`, and on the wire it's a plain string (`serde(transparent)`), so the API didn't change. Unlike `secrecy`, it doesn't wipe its memory when dropped (that needs a dependency or `unsafe`); the risk here is logs. No new dependency.
- **Where it's used:** every password, code, and token parameter of the server functions (sign-in, setup, the setup-code check, password change, Admin mode, recovery), the matching `server` functions down to hashing, the tokens and temporary passwords in the answers (`SignInOutcome`, `SetupOutcome`, `RecoveryOutcome`, `UserAdminOutcome`), and the desktop's in-memory token.
- **The rule** is in AGENTS.md ("What logs may contain"): no secrets or record text at any level; IDs, not contents; the setup and recovery codes only with `console_only = true` (never the log files or the in-app viewer); what each level is for.
- **`crates/server/tests/secrets_in_logs.rs`** runs setup, sign-in, a wrong password, Admin mode, a password change, adding a user, a password reset, and a recovery at trace level, and fails if any secret reaches the log or a code appears in a line that isn't console-only. Checked by logging a password on purpose: the test failed.

**Step 1e notes (2026-11-08):**
- **`spikes/sqlcipher/`** is its own workspace with its own `Cargo.lock` (and ignored by Docker), so its SQLCipher build of `libsqlite3-sys` can never mix into the server's plain SQLite through Cargo's feature unification. It uses the app's sqlx (0.9.0, `sqlite-bundled`), and chooses SQLCipher through the same `libsqlite3-sys` (0.37.0) with one of two features: **system crypto** (`bundled-sqlcipher`: Apple's CommonCrypto on macOS, the system OpenSSL elsewhere) or **bundled OpenSSL** (`bundled-sqlcipher-vendored-openssl`).
- **Seven checks:** it's SQLCipher (`PRAGMA cipher_version`); create and write; the file has no plain SQLite header and the text isn't visible in it; the right key reads; a wrong key and no key are refused ("file is not a database"); `PRAGMA rekey` keeps the data and retires the old key. sqlx sends the `key` pragma before anything else by design (`sqlx-sqlite` 0.9.0, `options/mod.rs`).
- **macOS (local, Apple Silicon, release builds from clean):**

  | Build | Time | Binary | Checks |
  |---|---|---|---|
  | Plain SQLite | 16.6 s | 3.0 MB | (not run) |
  | SQLCipher, Apple's crypto | 18.2 s | 3.1 MB | 7 of 7 |
  | SQLCipher, bundled OpenSSL | 42.7 s | 6.7 MB | 7 of 7 |

- **All three platforms** (`.github/workflows/sqlcipher-spike.yml`, run 2026-11-09 on GitHub's runners; cold builds, including downloading crates; it runs when the spike changes, or by hand once it's on `dev`):

  | | Plain SQLite | SQLCipher, system crypto | SQLCipher, bundled OpenSSL |
  |---|---|---|---|
  | **Linux** | 72 s, 3.4 MB | 74 s, 3.5 MB, 7 of 7 | 124 s, 8.3 MB, 7 of 7 |
  | **macOS** | 78 s, 2.9 MB | 31 s, 2.9 MB, 7 of 7 | 142 s, 6.4 MB, 7 of 7 |
  | **Windows** | 90 s, 2.4 MB | 86 s, 2.5 MB, 7 of 7 | 594 s, 6.5 MB, 7 of 7 |

- **What Windows needed** (the first run's two Windows SQLCipher builds failed on the build machine, not on SQLCipher): system crypto needs OpenSSL installed and named (`OPENSSL_LIB_DIR`, `OPENSSL_INCLUDE_DIR`; the runner's keeps `libcrypto.lib` in `lib\VC\x64\MD`), and the program then needs OpenSSL's DLLs to run, so a Windows package would have to ship them. Bundled OpenSSL needs the full Strawberry Perl (`OPENSSL_SRC_PERL`), not Git's minimal one, and takes about 10 minutes to build in CI.
- **Recommendation for milestone 4** (to settle when it's planned): **macOS: system crypto** (Apple's CommonCrypto, part of the OS, almost no cost); **Windows: bundled OpenSSL** (self-contained: no DLLs to ship or keep patched; the slow build only hits CI and Windows developers); **Linux: system crypto** (OpenSSL is on every desktop distribution, and `.deb` and `.rpm` packages can name it as a dependency), or bundled if a self-contained binary matters more than 5 MB. Cargo can choose per platform: an optional `libsqlite3-sys` with different SQLCipher features in `[target.'cfg(...)'.dependencies]` tables, switched on only by the desktop build, so the server (and its Linux image) keeps plain SQLite until Phase 7.
- **Not tested yet (for Phase 7):** that a SQLCipher build opens an existing plain database with no key (SQLCipher documents that it does), and SQLCipher's `sqlcipher_export` for encrypting an existing database in place.

**Milestone 1 test (2026-11-09):** `docs/test-plans/archive/p3m1-groundwork.md` passed on the test server, except the desktop window's **size** isn't restored (its place and a maximized state are) and the second-monitor step wasn't run; both moved to the Polish phase with the human's request to reorganize Settings (`docs/plan.md` §9 Phase 6).

### Milestone 2 plan: devices and pairing

*Agreed 2026-11-09 · branch `milestone/p3-m2-pairing` · sketch: `docs/ui-sketches/p3m2-devices-options.svg` (1A, 2B, 3A) · decision #073 · sub-plan 04 §4.3, §6*

**Today:** every sign-in makes a session with a new random device ID; the desktop keeps its token in memory only (#027), so it signs in at every launch; nothing lists or names devices.

**Choices (the human, 2026-11-09):**
- **Signature:** our own compact format, shaped like DPoP (RFC 9449), not DPoP's JWTs: only our own apps send it.
- **Debug builds** keep the device key and token in a file in the data folder (readable only by the user; a warning in the log), so `dx serve` rebuilds don't set off Keychain prompts; release builds always use the OS keychain.
- **Switching servers:** choosing another server saves it and says "Quit and reopen to switch" (Dioxus sets the address once per run).
- **Settings → Devices** lists desktops **and** browsers (browsers named from their user agent, e.g. "Safari on iPhone").
- **Sketch:** **1A** (recent servers as a list above the sign-in form), **2B** (Devices as its own Settings tab: You | Devices | Household), **3A** (the Admin's "Devices (3)" dialog from a person's row on the Users screen).
- **One person per desktop per server** (a shared computer is post-MVP, "Shared devices").

| Step | Status | What |
|---|---|---|
| **2.0** `docs:` | ☑️ | This plan, decision #073, the sketch. |
| **2a** `feat(storage):` devices | ☑️ | Migration 0016: a `devices` table (ID, user, name, kind `desktop`/`browser`, public key for desktops, added, last seen, signed out); every session belongs to a device (existing sessions get a browser device each). Storage functions and tests. |
| **2b** `feat:` device keys and signed requests | ☑️ | `domain`: the signature's format and what it covers (method, path and query, time, nonce, SHA-256 of the token), parse and verify; `server`: the auth layer requires a valid signature for a session whose device has a key (within 5 minutes of the server's clock; each nonce once, remembered for that window), and refuses the token without one. Tests: a copied token alone, a replayed signature, a changed path, an old time, another device's key. |
| **2c** `feat(app):` pairing | ☑️ | Desktop sign-in sends a new device's public key and name (the computer's name); the token and key kept in the OS keychain (`keyring`; a file in debug builds), so the desktop stays signed in across restarts; every request signed with the server's time (offset from the handshake); signing out removes them. **API version 2**; version-1 apps (today's desktop) still sign in, without a key, shown as "Desktop app (older version)". |
| **2d** `feat(app):` recent servers | ☑️ | `settings.json`'s list of servers (address, name, TEST tag, database ID, last used); the desktop sign-in shows them (1A) with "+ Another server…"; another server than this run's: "Quit and reopen to switch". |
| **2e** `feat:` Devices | ☑️ | Settings → **Devices** tab (2B): your devices and browsers, "This device", Rename (desktops), Sign out, Sign out everywhere else. Users → **Devices (n)** dialog (3A) in Admin mode: a person's devices, Sign out, sign out all. Signing out a desktop removes its key: it must pair again. |
| **2f** `docs:` | ☑️ | The test plan (`docs/test-plans/p3m2-pairing.md`), on the test server with the desktop app. |

**New dependencies (approved with this plan):** `ed25519-dalek` (signatures), `keyring` (the OS keychain), `gethostname` (the default device name).

**To settle in the steps:** the browser label's wording (from the user agent, kept short); the exact header name and layout (documented in `domain`).

**Step 2a notes (2026-11-09):**
- **Migration 0016:** the `devices` table (its `kind` is plain text checked by `domain::device::DeviceKind`, so a kind can be added without a rebuild; `name` is empty until there is one); each existing session gets a device of its own, recorded as an unnamed browser (today's desktop sessions can't be told apart, and end within 30 idle days); `sessions` is rebuilt so its device must exist. The fixture test now also checks foreign keys, and that every migrated session's device can be read.
- **A device is "signed in" while it has a session that hasn't ended** and hasn't been signed out; the list needs no clean-up job. Signing a device out ends its sessions and stamps it once; the row stays (changes carry its ID). A session's use also updates its device's "last seen".
- **Every sign-in now stores a device** with its session, in one transaction: a browser until 2c pairs desktops.

**Step 2b notes (2026-11-09):**
- **Changed from the plan, with the human's choice:** Dioxus 0.7.10 sends server-function calls with one shared set of extra headers (`set_request_headers`) and has no per-request hook, so those calls can't be signed per request. One format, two scopes (`domain::device_signature`): **request** (method, path and query, time, nonce, token hash; within 5 minutes and accepted once) for the app's own requests (files now, sync from milestone 5), and **session** (no method or path; within 2 minutes, reusable, renewed every 30 seconds) in the shared headers for server-function calls. A copied token alone is refused either way; a captured session signature works only briefly (and capturing it means breaking HTTPS).
- **Header:** `x-device-signature: v1;scope=request;t=<ms>;n=<32 hex>;s=<128 hex>`; Ed25519 (`ed25519-dalek` 3.0.0, same SHA-2 as the workspace, builds for the browser too) over a fixed label, the scope, method, path, time, nonce, and the token's SHA-256 (`domain::device_signature::token_hash`, which the server's `token_hash` now uses).
- **Server:** `sessions::resolve_signed` checks a keyed device's signature **before anything is written** (a copied token can't even keep a session alive); plain `resolve` never lets a keyed device in. The auth layer refuses a bad signature with the new `ApiError::DeviceSignature` (401), except on a public path, which goes on as signed out. Request nonces are remembered in memory for twice the window.
- **Tests** (`crates/server/tests/device_signatures.rs`, through the real auth layer): request and session signatures pass; no signature, a replay, another path, an old time, another device's key, and a malformed header are refused; a public path without one goes on as signed out; a refused request writes nothing.
- **Advisory RUSTSEC-2026-0459** (SQLite in `libsqlite3-sys` 0.37), found by `cargo deny` in this step and not caused by it: ignored in `deny.toml` with its reason (only our own databases are opened), since the fix needs a newer sqlx; tracked in GitHub issue #41.

**Step 2c notes (2026-11-09):**
- **API version 2** (the server still accepts 1). The sign-in server function takes an optional `Pairing` (the public key as hex, and a name); `server::sessions::sign_in_from` stores the device as a **desktop with its key and name**, an **older desktop** (a version-1 app asking for a token: no key, "Desktop app (older version)"), or a **browser**. Plain `sign_in` stays the browser case. First-Admin setup and password recovery on a desktop still sign in unpaired (a token for this run); its next sign-in pairs it. An unusable key is refused as "unavailable".
- **The desktop** (`app::pairing`, `app::device_credential`): signing in makes a key from the OS's randomness and sends its public half with the computer's name (`gethostname`); the key and token are kept per server address in the **OS keychain** (`keyring` 4.2: macOS Keychain, Windows Credential Manager, Linux Secret Service through `zbus`, pure Rust) or, in **debug builds** and where no keychain service runs, in `device-credentials.json` in the data folder (owner-only, 0600). At launch it restores them, so it stays signed in; signing out forgets them.
- **Signing:** server-function calls carry a session signature in Dioxus's shared headers, renewed every 30 seconds by a desktop-only loop (tokio's timer); photo and PDF transfers get a request signature each. Signatures use the server's clock: the desktop measures it with the (public, unsigned) handshake before its first signed call, and again before believing a "signed out", so a computer clock that jumped never costs a pairing. A desktop the server no longer accepts (signed out elsewhere) forgets its pairing.
- **Checked end to end** against a local server: pairing over HTTP stored a desktop device with its key; with the token alone a protected call was refused (`device-signature`) and the session check said signed out; signed, both worked. The debug desktop app, given a saved pairing, restored it at launch and its signed requests were accepted (the device's last use moved; no refusals).

**Step 2d notes (2026-11-09):**
- **`settings.json` keeps `servers`:** each server's address, instance tag, database ID, and last use; newest first, at most 10. A server is matched by its address, or by its database ID when it has moved to a new address (the old entry goes). It's remembered when this desktop signs in to it, and once per run when it starts already signed in.
- **The handshake carries the server's database ID** (an optional field; older servers leave it out). Milestone 4 names the desktop's local data folders after it.
- **The desktop sign-in screen** (option 1A): the servers as cards (address, TEST tag, "Last used today / yesterday / N days ago"), the current one checked; "+ Another server…" opens an address field (open from the start on a computer with no address yet, so a first sign-in is server, username, password). Choosing or typing another server saves it and says "Saved. Quit and reopen the app to switch to …" (Dioxus sets the address once per run). The web shows none of it.
- **Not seen in a screenshot here** (capturing a window needs macOS's Screen Recording permission); the test plan checks it.

**Step 2e notes (2026-11-09):**
- **Browsers are named at sign-in** from their `User-Agent` (`domain::device::browser_label`: "Safari on iPhone", "Chrome on Mac", "Edge on Windows"; only the browser's and system's names are kept, never versions). Sessions from before stay unnamed ("Browser").
- **Server** (`server::devices`): your own list (this device marked), rename one of yours, sign one out (never the one in use: that's the account menu's Sign out), and sign out everywhere else; the household Admin, in Admin mode only, lists and signs out anyone's, one or all, **except the device the Admin is using**. The Users screen's summaries count each person's signed-in devices.
- **Settings → Devices** (option 2B): its own tab for everyone (You | Devices, plus Household in Admin mode); this device first, with Rename; the others with Sign out; "Sign out everywhere else" asks first. **Users → a person → Devices (n)** (option 3A) opens their devices in a dialog. The Users screen opens a person's detail view rather than having buttons on each row, so the button sits in their Account section beside Reset password.
- **Checked in headless Chrome:** three browsers named from their user agents; rename, sign out one, and everywhere else (with its confirmation); the phone layout; the Admin's dialog (Devices (3), one signed out, no button on the Admin's own browser).

