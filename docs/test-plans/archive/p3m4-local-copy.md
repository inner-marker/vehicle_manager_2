# Manual test plan: Phase 3 milestone 4, local database and pull

*Phase 3 milestone 4 · branch `milestone/p3-m4-local-copy` · `docs/phase_plans/phase-3.md` · decision #075 · sketch `docs/ui-sketches/p3m4-offline-options.svg` (1A, 2B, 3A)*

## 1. Intended behavior

- **The desktop keeps an encrypted copy** of the household's records (SQLCipher), in `servers/<database ID>-test` in its data folder (just `<database ID>` for a main server). Its key is sealed with the sign-in, so the PIN or the password opens it. The file is unreadable without it.
- **What's in it:** the household's records and your own preferences, saved places, garage order, and message marks. Never passwords, sessions, devices, jobs, or other people's private rows.
- **Pull:** the first download right after choosing a PIN, then every minute, at every unlock, before each screen reads, and at **Sync now**.
- **Screens read the copy** on the desktop with the server's own read code, so they show the same as the web. Saving still goes to the server.
- **Offline:** with no connection, unlocking opens the copy in **Driver mode**, with an amber **OFFLINE · copy 3:42 pm** tag in the top bar. Save, Delete, Finish, and Log are dimmed, with "Saving needs the server (offline)." beside each form's buttons. When the server is back, the tag goes and the real session returns.
- **Diagnostics → Offline copy:** status, last pull, records, size, folder, **Sync now**.
- **Signing out deletes the copy.**
- **Files** (photos, PDFs) still come from the server: offline, they don't open. That's milestone 6.
- **The web app is unchanged.**

## 2. Setup

- The "Server image" run for the newest code push to `milestone/p3-m4-local-copy` finished.
- **The test server** (`:8443`): `VEHICLEMANAGER_IMAGE_TAG=milestone-p3-m4-local-copy` in its `.env`, then `sudo docker compose pull && sudo docker compose up -d` in its folder.
- **The desktop app** from this branch, pointed at the test server: `cd crates/app && dx serve --platform desktop`. The first build takes longer than before: it now builds SQLCipher and the server's read code.
- The test server has a vehicle with a few events, a document, and a photo (from earlier tests).

## 3. Checklist

| Done | # | Step | Expected result |
|---|---|---|---|
| [x] | 1 | **NAS:** `sudo docker logs --since 10m vehiclemanager-test 2>&1 \| grep -E "app starting\|database open"` | `build=milestone-p3-m4-local-copy · …`; `database open … schema_now=18` (`schema_before=17` on the first start). |
| [x] | 2 | **Desktop:** start it | The **sign-in card** (milestone 3's sealed sign-in is from an older format and is forgotten). The `dx serve` terminal says "…from an older version of the app; it signs in once more". |
| [x] | 3 | Sign in, choose a PIN | The app opens. Within a few seconds, **Diagnostics → Offline copy** says **Up to date**, with a last pull (UTC), some records, a size, and a folder ending in `-test`. |
| [x] | 4 | **Encrypted:** Mac Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/` then `head -c 15 ~/Library/Application\ Support/vehiclemanager/servers/*-test/vehiclemanager.sqlite; echo` | One folder, `<database ID>-test`. The 15 bytes are gibberish, **not** `SQLite format 3`. |
| [x] | 5 | **Desktop:** the Garage, a vehicle's timeline, an event, its squawks, documents, and maintenance schedule | All as on the web. |
| [x] | 6 | **Mac browser:** log a reading on that vehicle. **Desktop:** open the vehicle's timeline (or click away and back) | The reading is there (the desktop catches its copy up before reading). |
| [x] | 7 | **Desktop:** log a fuel-up, then open it | Saved, and it shows straight away. |
| [x] | 8 | **Desktop:** Admin mode (your password), open Users and a vehicle's settings | As before (Users still comes from the server; vehicle pages from the copy, as Admin). Back to Driver mode. |
| [x] | 9 | **Offline at unlock:** quit the desktop (⌘Q). Turn the Mac's **Wi-Fi off**. Start it, type the PIN | Locked, then the app, in **Driver mode**, with **OFFLINE · copy (time)** in the top bar (hover: "Can't reach the server…"). |
| [x] | 10 | Offline: the Garage, a timeline, an event, documents, the schedule | All there, as they were. A photo or PDF doesn't open (files are milestone 6). |
| [x] | 11 | Offline: open the log form (from a vehicle) and an event's Edit | **Save** (and Delete, Finish) are dimmed and can't be clicked; "Saving needs the server (offline)." beside the buttons. The top bar's **Log** is dimmed. |
| [x] | 12 | Offline: **Diagnostics → Offline copy** | Status **Offline: can't reach the server**; the last pull is still shown. |
| [x] | 13 | **Wi-Fi on.** Wait up to a minute (or Diagnostics → **Sync now**) | The OFFLINE tag goes; Status **Up to date**; saving works again. The mode chip shows your real session (Driver). |
| [x] | 14 | **The server goes away while open:** **NAS:** `sudo docker stop vehiclemanager-test`. **Desktop:** open another vehicle (or a timeline) | The OFFLINE tag appears; the screen still shows (from the copy); you're still signed in. |
| [x] | 15 | **NAS:** `sudo docker start vehiclemanager-test`. **Desktop:** Diagnostics → **Sync now** | The tag goes; Up to date. |
| [x] | 16 | **Locked, then unlocked online:** Settings → Lock after → 5 minutes, wait (or quit and start), unlock | Up to date again; anything logged on the web meanwhile shows. Set Lock after back. |
| [x] | 17 | **Signing out deletes the copy:** account menu → Sign out. Terminal: `ls ~/Library/Application\ Support/vehiclemanager/servers/` | The `-test` folder is gone. |
| [x] | 18 | Sign in again, choose a PIN | A new copy downloads (Diagnostics: Up to date), in a new `-test` folder. |
| [x] | 19 | **Mac browser:** the Garage, a timeline, Diagnostics | As before: no OFFLINE tag, no Offline copy section. |
| [x] | 20 | Afterwards: **leave the test server on `:milestone-p3-m4-local-copy`** until this milestone merges, then move it to `:dev` | Its database is now at schema 18; today's `:dev` (schema 17) would refuse to open it. |

## 4. If a step fails, capture

- **Desktop:** the terminal running `dx serve`, and Diagnostics → the log viewer with **Area: Sync** (Minimum level: Debug): "pulled a page", "no pull: can't reach the server", "this computer's copy is open", "the server can be reached again".
- **Container log:** `sudo docker logs --since 30m vehiclemanager-test 2>&1 | grep -i pull`.
- **Never share** a PIN, a password, a token, `.env`, a cookie, `device-credentials.json`, or the copy's file.

# Human Notes
- at some point, the timeline views will become too long to render cleanly and smoothly. Add to am appropriate place later in the plan that we should limit the number of records shown. Maybe pages of 100 (or selectable number) records with "next" and "previous" buttons. The timeline should also have a "jump to date" option.
- Bug: after starting the desktop app offline and entering the correct PIN, the app is stuck on the "loading" screen.

## 5. Findings and fixes (2026-11-12)

- **Stuck on "Loading" after an offline unlock (step 9): a bug, fixed.** The session check had no time limit. With Wi-Fi off and Tailscale still running, the server's address still looks reachable, so its requests (the clock check, then the session) can wait minutes for an answer that never comes, and the app waits on "Loading" the whole time. The desktop now gives the session check **8 seconds** (each pull page 15), then counts the server as unreachable, and the offline copy opens. Expect "Loading" for up to about 8 seconds when offline.
  - **Found with it:** once offline, the app would have gone on to "remember" the server, from a handshake that can't answer, saving it without its database ID or instance. The next offline start then couldn't find the copy's folder (named by both). Offline, it no longer remembers; a failed handshake keeps the ID on file; and the TEST SERVER tag comes from what was remembered.
  - **Retest:** steps 9 onward with a new image.
- **Improvement** When the loading screen is because the app is attempting to contact the server before it goes into offline mode, can we make the loading screen more explicit. Maybe "Loading...\nAttempting to connect to the server. Offline mode in [coundown] seconds". I am open to other wording ideas too. This should only be shown for users logged in and whose PIN is accepted. This should be completed in this phase, not necessarily this milestone.
- Please confirm that it is intented behavior right now that we cannot add a squawk offline. Eventually I want that functionality.
- **Possible Bug** In offline mode, the "+ Squawk" button in a vehicle's Squawks tab is not disabled/greyed out like the "+ Log" button in the top bar is.
- **Idea** Is is possible to get information about the computer using OS-specific APIs to automatically rule out the ability to connect to the server? Like can we see if the computer is connected to wifi or ethernet at all before even trying to contact the server?
- Here was a warning in the diagnostics queue:
```
2026-11-12T22:11:28.878Z	Warning	dioxus_signals::warnings::__copy_value_hoisted	A Copy Value created in ScopeId(3, "root") (at crates/app/src/ui/mod.rs:69:29). It will be dropped when that scope is dropped, but it was used in ScopeId(0, "app") (at crates/app/src/ui/auth.rs:138:15) which is not a descendant of the owning scope. This may cause reads or writes to fail because the value is dropped while it still held. Help: Copy values (like CopyValue, Signal, Memo, and Resource) are owned by the scope they are created in. If you use the value in a scope that may be dropped after the origin scope, it is very easy to use the value after it has been dropped. To fix this, you can move the value to the parent of all of the scopes that it is used in. Broken example ❌: ```rust #[component] fn Counters() -> Element { let counts = use_signal(Vec::new); let mut children = use_signal(|| 0); rsx! { button { onclick: move |_| children += 1, "Add child" } button { onclick: move |_| children -= 1, "Remove child" } // A signal from a child is read or written to in a parent scope "{counts:?}" for _ in 0..children() { Counter { counts } } } } #[component] fn Counter(mut counts: Signal<Vec<Signal<i32>>>) -> Element { let mut signal_owned_by_child = use_signal(|| 0); // Moving the signal up to the parent may cause issues if you read the signal after the child scope is dropped use_hook(|| counts.push(signal_owned_by_child)); rsx! { button { onclick: move |_| signal_owned_by_child += 1, "{signal_owned_by_child}" } } } ``` Fixed example ✅: ```rust #[component] fn Counters() -> Element { let mut counts = use_signal(Vec::new); rsx! { button { onclick: move |_| counts.write().push(0), "Add child" } button { onclick: move |_| { counts.write().pop(); }, "Remove child" } "{counts:?}" // Instead of passing up a signal, we can just write to the signal that lives in the parent for index in 0..counts.len() { Counter { index, counts } } } } #[component] fn Counter(index: usize, mut counts: Signal<Vec<i32>>) -> Element { rsx! { button { onclick: move |_| counts.write()[index] += 1, "{counts.read()[index]}" } } } 
```

- Note for the future, I can make the server inaccessible by simply disabling tailscale rather than disabling wifi. Feel free to include that step in test plans when it makes sense.
## 6. Answers and retest (2026-11-12, after the checklist)

- **The waiting screen (improvement):** done. After the PIN, offline: "Connecting to the server…"; after a second, "No answer yet. Opening this computer's copy (offline) in N seconds." and **Start in Offline Mode Now**. Only with the copy open, so only after an accepted PIN; signing in (and the web) still say "Loading…".
- **Adding a squawk offline:** intended for now. Every change still goes to the server; offline changes (squawks included) are milestone 5.
- **"+ Squawk" not dimmed (bug, fixed):** worse than it looked: the squawk sheet opens outside the screens, so its **Save** wasn't turned off either. Now Save is off in the sheet, and the buttons that only start a change (the top bar's Log, the phone's **+**, **+ Squawk**, a squawk's Edit and Remove) are disabled offline, with a tooltip: "Can't reach the server (offline). Changes need it until offline saving arrives."
- **Asking the operating system about the network (idea):** possible (each system says whether there's any network), but it only helps with no network at all; with Tailscale off or the server down the network is up, so the wait stays. Noted in `docs/plan.md` §9 Phase 6 to weigh later.
- **The Diagnostics warning (fixed):** the sign-in state belonged to the app's root component, but the unlock's session check runs in a task above it. Harmless (that component lives as long as the window), but it now belongs to the very top, so the warning goes.
- **Timeline paging:** noted in `docs/plan.md` §9 Phase 6.

| Done | # | Step | Expected result |
|---|---|---|---|
| [ ] | R1 | Pull the new `:milestone-p3-m4-local-copy` image on the test server; rebuild the desktop (`dx serve --platform desktop`) | Starts as before. |
| [ ] | R2 | Quit the desktop. **Turn Tailscale off.** Start it, type the PIN | "Connecting to the server…"; after a second, the countdown and **Start in Offline Mode Now**; at 0, the app opens offline (OFFLINE tag). |
| [ ] | R3 | Quit, start again (Tailscale still off), type the PIN, click **Start in Offline Mode Now** | The app opens offline at once. |
| [ ] | R4 | Offline: a vehicle's Squawks tab; hover **+ Squawk**, then open a squawk and hover Edit and Remove; hover the top bar's **Log** | All dimmed, with the not-allowed pointer and the tooltip; clicking does nothing. |
| [ ] | R5 | Offline: Diagnostics → the log viewer | No "Copy Value … not a descendant" warning from this run. |
| [ ] | R6 | **Tailscale on.** Wait up to a minute (or Sync now). Open **+ Squawk**, then cancel | Online again: the buttons work, and the sheet's Save is normal. |
| [ ] | R7 | Quit, start with Tailscale on, type the PIN | The app opens straight away (at most a brief "Connecting to the server…"). |
