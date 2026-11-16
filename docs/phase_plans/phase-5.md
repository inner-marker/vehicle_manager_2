# Phase 5: Maintenance intelligence (plan)

*Agreed 2026-11-15 · Roadmap: `docs/plan.md` §9 Phase 5 · Open questions: settled (decision #080) · Builds on #031, #051, #057, #063*

**Goal:** the app **tells the household what's due.** Every requirement knows when it's next due, by distance, hours, time, or a meter such as **Oil life remaining**. Warnings start early and grow more urgent. Messages arrive when something is due, and they can be snoozed. A job pre-fills the parts it uses, a scheduled service is signed off in one go, fuel economy is honest about partial and missed fill-ups, and each vehicle answers **"Ready to use?"** with its reasons.

**Who it's for:** the web app and the desktop app alike. Everything is worked out in `domain` from the records, so the desktop does it offline, from its own copy (#071, #072).

**Not in Phase 5:** import and export, reports, standalone mode, and the Settings reorganization (Phase 6); aircraft items and operation types (the aircraft phase); the phone apps (post-MVP, #041).

**Exit criteria:**
- **Oil life:** the truck has a counting-down percentage meter, **Oil life**. Its oil change is due at 7,500 mi, at 12 months, or at Oil life 15%, whichever comes first. As readings are logged, the oil change turns yellow, then orange, then red. Logging the oil change with **Reset Oil life to 100%** moves its next due on.
- **What's due:** the Maintenance tab and the Overview show the time and distance to each service, with projected dates. The Garage puts overdue vehicles first, with an **Overdue** chip.
- **Messages:** a due-soon message appears on the web, and on the desktop with Wi-Fi off. Snoozed for "1 week or 500 mi", it comes back when either is passed.
- **Specs and bundles:** an oil change pre-fills its oil and filter from the truck's Specs. A "10,000 mile service" signs off three tasks in one event, and each task's own clock resets.
- **Fuel economy:** a partial fill adds into the next full fill's figure. A missed fill-up leaves a gap, not a wrong average. The figures can be broken down by driving condition and by towed vehicle.
- **Readiness:** expired insurance makes the car **not ready**, saying why. A deferred High issue still blocks. On the truck, **Towing** adds its own items.
- **Calculated values:** a derived meter drives an interval rule, and a calculated field shows on a fuel-up.
- **Sync:** each new kind of change works offline on the desktop and syncs. An app one API version behind still syncs.

**How this file works:** the same as `phase-3.md` (decision #021).
- One branch and **one PR into `dev` per milestone** (`milestone/p5-mN-<slug>`), merged with a merge commit.
- Each **step** is one Conventional Commit. Steps are planned in detail, and approved, when their milestone starts.
- Screens get **SVG layout options** in `docs/ui-sketches/` before they're built (#040).
- Each milestone has a test plan in `docs/test-plans/`, run on the test server (#062) with the web and desktop apps.
- `cargo clean` at each step's end, once its checks have run (the human's disk is small).
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | What it delivers | You can check |
|---|---|---|---|---|
| 1 | **Meters** | 🔨 | **Percentage meters** (any meter; 0–100%, stored in hundredths of a percent). **Direction:** counts up or counts down. **Resettable** (either direction), with its reset value (e.g. 100% or 0); never a distance meter used for fuel economy. Readings checked per direction, with a **reset** explaining a jump; resets on the timeline. The meter form and reading inputs. A migration, and new operations for the desktop's outbox | Add **Oil life** to the truck, log 80%, 60%, see 70% refused, reset it to 100% |
| 2 | **What's due** | ⬜ | The calculation in `domain`: each rule's next due (meter, calendar, fixed date, and **"when a meter reaches X"**), whichever comes first; dates projected from recent use. **Three steps** (yellow at 20% of the interval left, orange at 10%, red at 5%) and **Overdue** (a darker red, with a chip), as household defaults that Maintainers and Admins can change per requirement. A job's **"Reset Oil life to 100%"**. The Maintenance tab, the Overview's time and distance to each service, and the Garage's urgency order and badges. Document expiries, with the renew flow | The oil change due by Oil life; the colours change as readings come in |
| 3 | **Due messages and snooze** | ⬜ | Due-soon and overdue messages in Messages, worked out from the records (so they also work offline on the desktop); **snooze** by time, distance, or hours, whichever comes first; per-vehicle roles respected | Snooze "1 week or 500 mi"; it returns at whichever comes first |
| 4 | **Specs and service bundles** | ⬜ | The vehicle's **Specs** list (a name, a value with units or a part number, a note); requirements link to specs; a logged task's **parts pre-filled** from them (editable; "update the spec too?"); **bundled sign-off** (one event, each task's clock reset); named household **templates**, copied onto a vehicle (#031) | The oil change pre-fills oil and filter; the 10k service signs off three tasks |
| 5 | **Fuel economy** | ⬜ | **Stretches** from one full fill to the next, with partials added in; a missed fill-up drops its stretch; **ⓘ** on **Filled full** and **Missed previous fill-up**; averages, a chart with gaps (drawn by the app, no chart library), breakdowns by condition and towed vehicle; the Overview's latest figures; the vehicle-reported figure beside the app's (#033) | Partial and missed fills behave as the ⓘ says |
| 6 | **Readiness** | ⬜ | **Operation types** in the profiles (**Normal use** on each ground profile; **Towing** on the truck); **needed for use** on requirements and documents; **"Ready to use?"** on the Overview, each item with ✓ or ✗ and its reason; the Garage badge; who may sign off or defer an issue, enforced (#063) | Expired insurance: not ready; deferred High: not ready |
| 7 | **Calculated fields and derived meters** | ⬜ | **Formulas** over built-in and custom fields by their stable keys, unit-aware, worked out when shown and never stored (#057). **Derived meters** worked out from other meters (e.g. odometer + idle hours × 33), usable in interval rules; a percentage can be derived too | A derived meter drives a rule; a calculated field on a fuel-up |

**Releases:** **`v0.5.0` at the end of Phase 5** (`docs/releasing.md`), with a migration fixture and a regression pass, as `v0.3.0` had. The version follows the phase's number, since Phase 4 moved after the MVP (#041).

---

## Why this order

- **1 first:** what's due needs every kind of meter and rule, and the new meters change how readings are checked, which is safest done before anything else builds on it.
- **2 before 3, 4, and 6:** messages, bundles, and readiness all ask "what's due?"
- **5 is independent:** fuel economy needs only fuel-ups, and could move earlier if wanted.
- **7 last:** derived meters need the formula engine of calculated fields. Milestone 2 reads meters through one interface, so a derived meter fits in later without rework.

## To decide when each milestone is planned

- **Milestone 1:** where a reset is recorded (a reading flagged as a reset, or a meter-adjustment record of its own); whether rollover and replacement of an odometer (plan §3 "Meter replacement & rollover", never built) come with it, since they're the same mechanism; whether a meter's direction can change once it has readings; the meter form's layout (SVG options).
- **Milestone 2:** how far back the usage projection looks (90 days, or the last N readings), and what it shows with too few readings; how a "reaches X" rule's steps are measured (from the reset value to X); where the household defaults live (Settings, Admin); the screens (SVG options).
- **Milestone 3:** when a message first appears (at yellow, or a household choice); how snooze is offered (quick choices plus a custom one).
- **Milestone 4:** what a spec holds (quantity types from custom fields, #033); how templates are named and applied.
- **Milestone 5:** the chart's form (SVG options), and how breakdowns are shown.
- **Milestone 6:** the readiness card and the Garage badge (SVG options); how "safety-related" is marked on a requirement.
- **Milestone 7:** the formula language (a small one of our own, or a crate), and how units are checked.
- **API version:** new operations raise it once per milestone that adds them. The server keeps accepting the one before (#072).
- **Open issues** are reviewed with each milestone's plan (AGENTS.md).

---

## Step details

*Detailed plans are added here as each milestone is planned and approved. Steps are numbered per milestone (1.0, 1a, …).*

### Milestone 1 plan: meters

*Agreed 2026-11-16 · branch `milestone/p5-m1-meters` · sketch: `docs/ui-sketches/p5m1-meters-options.svg` · Decision #080*

**Choices (the human, 2026-11-16):**
- **A reset is a reading marked "reset"**, on any event with readings (a Reading, a fuel-up, a job). Its value is the new start, it syncs with its event, it shows ↺ on the timeline, and the neighbour check starts afresh from it. Milestone 2's "Reset Oil life to 100%" on a job adds such a reading.
- **Odometer rollover and replacement: later** (Phase 6). They need corrected totals everywhere, and no household vehicle needs them yet.
- **A meter's direction is fixed once it has a reading**, like its dimension. "Resettable" and the reset value can change at any time.
- **Percentages:** any value (some gauges read over 100%), with 0–2 decimal places, stored in hundredths of a percent. **Repeated values are fine** (Oil life can sit at 0%); a counting-down meter just never rises, except at a reset.
- **The sketch: 1A, 2A, 3A.**
  - **1A:** the meter dialog keeps every setting in view, adding "Counts: Up / Down" and "Can be reset to [100] %".
  - **2A:** a resettable meter's reading has a **↺ Reset** tick; ticking it fills the reset value (still editable), and a line confirms "18% → 100%".
  - **3A:** a reading that goes the wrong way is refused with why. On a resettable meter, the refusal has an **↺ It was a reset** button.
- **Older apps: leave out what they can't read.** The API version goes to 3. A version-2 desktop is sent everything except percentage and counting-down meters and their readings, and when it edits an event, the server keeps the readings it never saw. It keeps syncing (#072); it just doesn't show those meters until it's updated.

**The rules, in `domain`:**
- **A meter** gains a **direction** (up, the default for every existing meter, or down) and an optional **reset value** (none: not resettable). The default reset value is 100% for counting down and 0 for counting up.
- **A distance meter required for fuel-ups can't be resettable**, since fuel economy (milestone 5) needs its running total.
- **A reading** gains **reset** (false by default). Only a resettable meter's reading can be a reset.
- **The neighbour check** follows the direction. Counting up, a reading may not be lower than the one before it or higher than the one after; counting down, the mirror image. Equal is always fine. A reset in between breaks the chain: a reset reading has no "before", and a reading before a reset has no "after" beyond it.

**Steps:**

| Step | Status | What |
|---|---|---|
| **1.0** `docs:` | ☑️ | This plan; the sketch. |
| **1a** `feat(domain):` meter kinds | ☑️ | `MeterDimension::Percentage` and `MeterUnit::Percent`; `MeterDirection` (up, down); `Meter::reset_to` (an optional reset value) on the meter and its patch; `check_meter` refuses a reset value of the wrong kind; `Reading::reset`; `neighbours` and `clash` aware of direction and resets (the history carries each reading's reset flag); parsing and showing percentages; the new messages. Unit tests first, including a back-dated reading either side of a reset. |
| **1b** `feat(storage):` migration 0022 | ☑️ | The `meters` table rebuilt to allow `percentage` and `percent` and to add `direction` and `reset_to`. SQLite can't change a `CHECK` in place, and `event_readings` and the tasks' readings point at `meters`, so the rebuild keeps every row and ID and is checked by `foreign_key_check`. `event_readings` gains `reset`. The `v0.2.0` and `v0.3.0` fixtures migrate. The desktop's copy runs the same migration. |
| **1c** `feat(server):` saving and syncing | ⬜ | Meter and event commands carry the new fields. The server refuses a direction change once the meter has readings, a reset on a meter that isn't resettable, and a resettable fuel-up odometer. The desktop's outbox and pull carry them. **API version 3:** a version-2 app's pull leaves out the meters it can't read and their readings (and the reset flag, which it ignores anyway), and its event edits keep those readings on the server. Tests for both. |
| **1d** `feat(app):` the screens | ⬜ | The meter dialog (the chosen row 1), the Reset choice on readings (row 2), the refusal (row 3), ↺ on the timeline and the event's page, and the meter's summary in Setup ("% · counts down · resets to 100%"). The same on the desktop, offline. |
| **1e** `docs:` | ⬜ | The test plan, `docs/test-plans/p5m1-meters.md`, on the test server with the web and desktop apps. |

**New dependencies:** none expected.

**Open issues** (reviewed 2026-11-16): none touch meters. #41 (sqlx) still waits upstream; the migration in 1b doesn't change that. #44, #31, #29, #27, #26, #24, and #15 belong to later phases.

**Step 1a notes (2026-11-16):**
- **`domain::meter`:**
  - `MeterDimension::Percentage` and `MeterUnit::Percent`, stored in hundredths of a percent (62% is 6,200).
  - `MeterDirection` (`Up`, `Down`), with `allows(earlier, next)`: equal is always allowed.
  - `Meter::direction` and `Meter::reset_to` (and the same in `MeterPatch`). Both have serde defaults, and `reset_to` is left out when empty, so a meter stored before Phase 5 reads as counting up and not resettable.
  - `check_reset` refuses a negative reset value and a resettable distance meter that fuel-ups require. `MeterError::DirectionFixed` is ready for the server's check in 1c. `default_reset` gives 100% for a percentage counting down, otherwise 0.
  - `MeterUnit::abbr()` gives the short unit name ("mi", "km", "h", "%"). It replaces six copies of the same `match` in the app's screens.
- **`domain::event`:** `Reading::reset` (false by default, left out of the JSON when false, as older apps expect). `ClashSide` gains `AboveEarlier` and `BelowLater` for meters counting down, each with its own text. `EventProblem::NotResettable` refuses a reset on a meter that can't be reset; not even the Admin can keep one. `EndBelowStart` now means "the wrong way from the start reading", and an end reading that's a reset is exempt.
- **`domain::event_check`:** the history is a list of `PastReading`s (meter, when, value, reset) instead of tuples.
  - `neighbours` treats a reset as coming after other readings at the same moment, and a reset after the new reading doesn't bind it.
  - A new reset reading has no "before".
  - `clash` and `reading_fits` take the meter's direction.
  - The server passes `reset: false` until 1b stores the flag.
- **Tests:** Oil life falling, sitting at 0%, and refused when it rises. Readings either side of a reset, including back-dated ones and one at the reset's own moment. A reset on a meter that can't be reset. A job's end reading counting down, or reset. Percent parsing and showing (also in the round-trip property test). A property test that counting down is counting up mirrored. A meter stored before Phase 5 still reads.
- **Not yet:** the meter dialog doesn't offer Percentage yet (1d), and nothing stores the new fields (1b).

**Step 1b notes (2026-11-16):**
- **`0022_meter_kinds.sql`** rebuilds `meters` by 0015's procedure:
  - it runs outside sqlx's transaction, with foreign keys off;
  - in one transaction it creates the new table, copies every row, drops the old one, renames the new one, and recreates both indexes (`meters_by_asset`, `meters_by_sequence`, the only ones);
  - it checks every foreign key before committing.

  Following 0015's rule, `dimension` and `unit` no longer list their values. New columns: `direction` (default `'up'`), `reset_to` (NULL, or zero and up), and `event_readings.reset` (0 or 1).
- **Storage:** meters write and read their direction and reset value (a patch can clear the reset value). Readings write and read the flag. `CountedReading` carries it to the server's checks.
- **Readings at the same moment** now come in the order they happened: resets last, a job's start before its end, then in the meter's direction. So the last of them is the latest, which `latest_readings` relies on. Before, they were simply sorted by value.
- **Tests:**
  - The release fixture test now also counts every meter and reading before and after migrating. `v0.2.0`'s and `v0.3.0`'s all come through, counting up, not resettable, and with no resets.
  - A meter's direction and reset value round-trip, and the reset value can be cleared.
  - A reset is kept, and comes last at its moment, after two readings counting down.
- **0015's own test** builds a database at schema 14 and fills it with today's writers, which now need 0022's columns. Its meter is written in SQL instead. The `reset` column is added only while its events are written, and dropped before 0015 runs, so its snapshot is still the real schema 14.

**Not in this milestone:** "when a meter reaches X" rules, due dates, and a job's "Reset Oil life to 100%" (milestone 2); derived meters (milestone 7); rollover and replacement (Phase 6).
