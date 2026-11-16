# Manual test plan: skeleton UI and navigation

*Phase 1, milestone 1, step 6 · `docs/phase_plans/phase-1.md`*

## 1. Intended behavior

- **One adaptive layout** for every screen (decision #011):
  - **Wide (≥ 1024 px):** full sidebar with icons and labels. Top bar: page title, sync status, theme toggle, **+ Log** button.
  - **Medium (640–1023 px):** icon-only sidebar. Hovering an icon shows its label as a tooltip.
  - **Narrow (< 640 px):** no sidebar. Instead:
    - a ☰ button opens it as an overlay, which also holds the theme toggle;
    - bottom tabs for Garage, Due & squawks, and Messages;
    - a round floating **+** button for Log.
- **Navigation:**
  - Sidebar main section: Garage, Due & squawks, Messages, Timeline, Reports. Below the divider: Vehicles, Settings, Diagnostics.
  - The current screen is highlighted, and its name is the top-bar title.
- **Placeholders:** every screen except Diagnostics shows "Coming soon." The vehicle detail screen (`/vehicles/<id>`) also shows "ID: <id>".
- **Unknown addresses** show "Page not found" with a "Back to the garage" link. They must not crash or show a blank page.
- **Theme:**
  - Light / Dark / System, defaulting to **Dark** on every start. It isn't remembered yet; that's expected.
  - System follows the operating system's appearance setting.
- **Diagnostics (log viewer):**
  - Shows this browser's own log entries, newest first. It always includes the "app starting" line.
  - The minimum level filter defaults to **Info**, and the area filter defaults to **All areas**.
  - **Refresh** loads new entries.
  - Times are UTC.
- **Expected limitations, not failures:** no copy/export, search, or live auto-refresh; sync status always says "Sync not set up"; no app name anywhere.

## 2. Setup

- On the `milestone/p1-m1-local-skeleton` branch with step 6 in place, from `crates/app` run:

  ```sh
  dx serve --platform web
  ```

  Open the URL it prints (usually `http://127.0.0.1:8080`) in a desktop browser.
- Use the browser's responsive or device mode (devtools) to set exact widths. Steps 1–8 need no devtools console.
- Optional: a phone on the same network for the narrow layout on a real device.

## 3. Checklist

| # | Step | Expected result |
|---|---|---|
| 1 | Open the app at a width of 1280 px | Garage screen. Full sidebar with 8 entries in two groups; Garage highlighted. Title "Garage". Top bar shows "Sync not set up", the theme toggle, and **+ Log**. Dark colors. |
| 2 | Click each sidebar entry in turn | Each click changes the title to that entry's name, highlights that entry, and shows "Coming soon." (Diagnostics shows the log viewer instead). No full page reload. |
| 3 | Click **+ Log** | Title "Log", "Coming soon." |
| 4 | Go to `/vehicles/test-123` in the address bar | Title "Vehicle", "ID: test-123", "Coming soon." |
| 5 | Go to `/does/not/exist` | Title "Page not found" and a "Back to the garage" link. Clicking it opens Garage. |
| 6 | Click the sun icon (Light) in the top bar | Whole app switches to light colors; the sun button is highlighted. |
| 7 | Click the monitor icon (System), then change the OS appearance between light and dark | The app follows the OS setting each time. |
| 8 | Reload the page | Theme is back to **Dark** (not saved yet, by design). |
| 9 | Resize to 800 px | Sidebar shrinks to icons only; hovering an icon shows its name. Everything else still works. |
| 10 | Resize to 375 px | Sidebar gone. ☰ button at the left of the top bar; bottom tabs (Garage, Due & squawks, Messages); round **+** button above the tabs, bottom right; the top-bar theme toggle and + Log are hidden. |
| 11 | At 375 px, tap a bottom tab | That screen opens and its tab is highlighted. |
| 12 | At 375 px, tap ☰ | Sidebar slides in as an overlay with labels, a close (✕) button, and the theme toggle at the bottom; the rest of the screen is dimmed. The toggle's three options (Light, Dark, System) fill the menu width equally, each label **under** its icon, none cut off. Tapping one shows no white outline. Check at 320 px too. |
| 13 | Tap a sidebar entry, then reopen ☰ and tap the dimmed area | The entry's screen opens and the menu closes; tapping the dimmed area also closes it. |
| 14 | At 375 px, tap the round **+** | Title "Log". |
| 15 | At 1280 px, open Diagnostics | Filters "Minimum level: Info" and "Area: All areas", a Refresh button, an entry count, and a table with at least the "app starting" line (area "App", level "Info"). |
| 16 | Set Minimum level to **Debug**, then click **Refresh** | No error. Debug entries appear only if a debug-level area is enabled; at the default levels the list may look the same. |
| 17 | Set Area to **Interface**, then change the theme twice and click **Refresh** | Only Interface entries are listed (none at the default Info level is fine). The count matches the rows. |
| 18 | Set Area back to **All areas** and Minimum level to **Error** | "No log entries match these filters." (unless something actually failed). |
| 19 | Open the browser devtools console and reload any screen | The "app starting" line is in the console. No red errors, and **no hydration mismatch warnings**. |

## 4. If a step fails, capture

- **Logs:** there's no runtime level switch in the UI yet. For a UI failure, note what you clicked and in what order.
- **Browser console:** copy the devtools console output (all levels) from page load up to the failure. Hydration warnings are especially useful for steps 15–19.
- **Terminal:** the `dx serve` output around the failure.
- **Screenshots:** the failed screen, with the browser width visible (devtools responsive mode shows it).
- **Environment:** browser and version; OS; for step 7, the OS appearance setting.
