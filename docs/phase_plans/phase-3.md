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
| 1 | **Groundwork** | ⬜ | The **app icon** (desktop window and browser tab); the **exact build on Diagnostics** (commit, image tag, build date; GitHub #37); **window title and launch settings** (window size and position remembered, always-on-top option; #19); **secret types and log tiers** with a test that no secret reaches a log (sub-plan 04 §7.4); the **SQLCipher spike**: an encrypted SQLite database through sqlx on macOS, Windows, and Linux in CI, with its effect on build time and size (sub-plan 04 §7.1) | The icon in the Dock and the tab; Diagnostics shows `milestone-p3-m1-…` and its commit; the window reopens where it was | `secrecy`; SQLCipher via `libsqlite3-sys` (spike only) |
| 2 | **Devices & pairing** | ⬜ | **Recent servers:** `settings.json` keeps a list (address, name, TEST tag, database ID, last used), shown as choices at sign-in plus "Another server…"; switching restarts the app (#027). **Signing in pairs the desktop** (#072): it makes a device key (Ed25519) kept in the OS keychain, gets a device token tied to it (kept in the keychain, not memory), and **signs every request** (DPoP-style); the server refuses the token without the signature. **Settings → Devices:** each device and browser, its last use, sign one out or all others; the Admin can do the same from Users | Pair the Mac; quit and reopen without signing in; copy the token to `curl` and see it refused; sign the Mac out from the web | `keyring`, `ed25519-dalek` |
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
