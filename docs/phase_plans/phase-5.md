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
- **One cumulative test plan for the phase** (#081): each milestone's last step adds its section to `docs/test-plans/p5-phase.md`. The whole plan is run at the end of the phase, on the test server (#062) with the web and desktop apps, as `v0.5.0`'s regression pass. A milestone's own plan is optional.
- `cargo clean` at each step's end, once its checks have run (the human's disk is small).
- Status: ⬜ not started · 📝 planning · 🔨 in progress · ☑️ committed on the milestone branch · ✅ in `dev`.

---

## Milestones

| # | Milestone | Status | What it delivers | You can check |
|---|---|---|---|---|
| 1 | **Meters** | ✅ | **Percentage meters** (any meter; 0–100%, stored in hundredths of a percent). **Direction:** counts up or counts down. **Resettable** (either direction), with its reset value (e.g. 100% or 0); never a distance meter used for fuel economy. Readings checked per direction, with a **reset** explaining a jump; resets on the timeline. The meter form and reading inputs. A migration, and new operations for the desktop's outbox | Add **Oil life** to the truck, log 80%, 60%, see 70% refused, reset it to 100% |
| 2 | **What's due** | ✅ | The calculation in `domain`: each rule's next due (meter, calendar, fixed date, and **"when a meter reaches X"**), whichever comes first; dates projected from recent use. **Three steps** (yellow at 20% of the interval left, orange at 10%, red at 5%) and **Overdue** (a darker red, with a chip), as household defaults that Maintainers and Admins can change per requirement. A job's **"Reset Oil life to 100%"**. The Maintenance tab, the Overview's time and distance to each service, and the Garage's urgency order and badges. Document expiries, with the renew flow | The oil change due by Oil life; the colours change as readings come in |
| 3 | **Due messages and snooze** | 🔨 | Due-soon and overdue messages in Messages, worked out from the records (so they also work offline on the desktop); **snooze** by time, distance, or hours, whichever comes first; per-vehicle roles respected; the Garage's **fleet strip** and the **Due & squawks** screen (#083) | Snooze "1 week or 500 mi"; it returns at whichever comes first |
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
- **Older apps: nothing extra; the API stays at version 2** (the human, 2026-11-16, step 1c). The first answer was "leave out what they can't read", but it rested on a wrong premise. A desktop only pulls and pushes on the server's own **schema** version (#075, #076), so migration 0022 already stops an older desktop syncing. It's told to update, and keeps its unsent changes, as in milestone 7's test. Only its online reads remain, and there a vehicle with one of the new meters can't be opened until it's updated. The household's one desktop is built from `dev`.

**The rules, in `domain`:**
- **A meter** gains a **direction** (up, the default for every existing meter, or down) and an optional **reset value** (none: not resettable). The default reset value is 100% for counting down and 0 for counting up.
- **A distance meter required for fuel-ups can't be resettable**, since fuel economy (milestone 5) needs its running total.
- **A reading** gains **reset** (false by default). Only a resettable meter's reading can be a reset.
- **The neighbour check** follows the direction. Counting up, a reading may not be lower than the one before it or higher than the one after; counting down, the mirror image. Equal is always fine. A reset in between breaks the chain: a reset reading has no "before", and a reading before a reset has no "after" beyond it.

**Steps:**

| Step | Status | What |
|---|---|---|
| **1.0** `docs:` | ✅ | This plan; the sketch. |
| **1a** `feat(domain):` meter kinds | ✅ | `MeterDimension::Percentage` and `MeterUnit::Percent`; `MeterDirection` (up, down); `Meter::reset_to` (an optional reset value) on the meter and its patch; `check_meter` refuses a reset value of the wrong kind; `Reading::reset`; `neighbours` and `clash` aware of direction and resets (the history carries each reading's reset flag); parsing and showing percentages; the new messages. Unit tests first, including a back-dated reading either side of a reset. |
| **1b** `feat(storage):` migration 0022 | ✅ | The `meters` table rebuilt to allow `percentage` and `percent` and to add `direction` and `reset_to`. SQLite can't change a `CHECK` in place, and `event_readings` and the tasks' readings point at `meters`, so the rebuild keeps every row and ID and is checked by `foreign_key_check`. `event_readings` gains `reset`. The `v0.2.0` and `v0.3.0` fixtures migrate. The desktop's copy runs the same migration. |
| **1c** `feat(server):` saving and syncing | ✅ | Meter and event commands carry the new fields. The server refuses a direction change once the meter has readings, a reset on a meter that isn't resettable, and a resettable fuel-up odometer. The desktop's outbox and pull carry them. The API stays at version 2 (see the choices). |
| **1d** `feat(app):` the screens | ✅ | The meter dialog (the chosen row 1), the Reset choice on readings (row 2), the refusal (row 3), ↺ on the timeline and the event's page, and the meter's summary in Setup ("% · counts down · resets to 100%"). The same on the desktop, offline. |
| **1e** `docs:` | ✅ | Milestone 1's section of the phase's test plan, `docs/test-plans/p5-phase.md` (#081: run at the end of the phase, not now). |

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

**Step 1c notes (2026-11-16):**
- **Meters are set up online only** (Maintainer work, `MeterForm`); the desktop's outbox carries events, issues, places, and attachments, not meters. So the meter checks live in the form and the server.
  - `MeterForm` gains `direction` and `reset_to` (with serde defaults), and `parse` runs `check_reset`. `check_reset` now takes the meter's dimension, required events, and reset value, so the form and a stored meter use the same check.
  - The server adds a meter with its direction and reset value. `edit_meter` changes either, but refuses a direction change once the meter has a reading that counts (`MeterError::DirectionFixed`).
  - Until 1d, the app's meter dialog sends a meter's existing direction and reset value unchanged, so editing a meter can't clear them.
- **Readings with a reset** travel inside their events, both online and through the outbox. The pull copies every column of `meters` and `event_readings` by name, checked against the copy's schema (`storage::sync`), so the new columns need no change there.
- **Older desktops:** see the choices. A desktop on schema 21 is refused pull and push by the schema check that's already there (`PullOutcome::OtherSchema`), and told to update. The API version stays at 2.
- **Server tests** (`tests/events.rs`, two tests): Oil life added, counting down and resettable. A resettable fuel-up odometer refused. 18%, then 62% refused (`AboveEarlier`), then accepted as a reset and kept. Staying at 62% is fine. A reset on the odometer refused (`NotResettable`). Oil life's direction is fixed once it has readings, but its reset value changes. A meter with no readings can still change direction. Also a domain test of the form's reset check.

**Step 1d notes (2026-11-16):**
- **The meter dialog (1A):**
  - **Measures** offers Percentage.
  - **Counts: Up / Down** uses the segmented control the compliance choices use. It notes that it's fixed once the meter has a reading; the server refuses a late change.
  - **Can be reset** opens "to [value] unit", typed in the meter's own unit and decimals, and filled with 100% for a percentage counting down (otherwise 0). A reset value left at its default follows a change of direction.
  - A bad value is refused in the dialog with the usual number messages. `TypedCounting` holds these rows.
- **Readings (2A):** a resettable meter's reading has a **↺ Reset** tick. Ticking it fills the meter's reset value, which stays editable. With a reading before it, a line confirms "Oil life was reset: 18 → 100 %". The form's `Typed` carries `resets` (the meters ticked), and editing an event ticks them again.
- **The refusal (3A):** when the server refuses a reading of a resettable meter for going the wrong way, that meter offers **↺ It was a reset**. It ticks Reset and clears the message, ready to save again. Meters counting down have their own refusal texts ("This meter counts down: higher than 18 % on 3 Nov…").
- **Where resets show:** ↺ after the value on the timeline, on the event page's tiles, and in a job's "start → end" line. **Setup → Meters** says "Percentage · % · whole · counts down · resets to 100 %".
- **Only a job's start readings** have the tick in this milestone. Its end readings get "Reset Oil life to 100%" with milestone 2's tasks.
- **Tests:** the form builds a ticked reading as a reset, and an event typed back into the form is ticked again and builds the same event. The dialog types the reset value in the meter's unit, and an unticked value isn't checked. The meter summary for Oil life.
- **Not seen running yet:** the dialog and the form, on the web and the desktop. They're in the phase's test plan (1e), run at the end of the phase (#081).

**Step 1e notes (2026-11-16):** the human changed the practice (decision #081): one cumulative test plan per phase, run at its end, and a milestone's own plan only when needed. So 1e starts `docs/test-plans/p5-phase.md` (its setup, and what to capture on a failure) with milestone 1's section: 16 steps. They cover the migration on the test server's data, Oil life set up, readings falling, staying level, refused when rising, "It was a reset", the Reset tick, the direction fixed, the reset value changed, a job's start reading, and the desktop online and offline. Nothing is run until the end of Phase 5. AGENTS.md, `plan.md` ("Keeping test plans"), and this file's "How this file works" say so.

**Not in this milestone:** "when a meter reaches X" rules, due dates, and a job's "Reset Oil life to 100%" (milestone 2); derived meters (milestone 7); rollover and replacement (Phase 6).

### Milestone 2 plan: what's due

*Agreed 2026-11-16 · branch `milestone/p5-m2-due` · sketch: `docs/ui-sketches/p5m2-due-options.svg` · Decisions #080, #082*

**Choices (the human, 2026-11-16):**
- **Never done: "Last done before these records."** Setting up a requirement (and Copy from…) offers an optional date and the readings then, and the first due counts from them. Without it, the requirement says **Not done yet** and counts from the vehicle's first reading of that meter (or, for months, the day the requirement was added).
- **Projected dates** come from the meter's average use over its **last 90 days** of readings (at least two, 7 or more days apart). Failing that, they use all its readings. Failing that, there's no date, only what's left ("in 1,200 mi"). An estimate is shown with "~".
- **Dates without an interval** (a fixed "next due on" date, and document expiries) warn **60 / 30 / 14 days** before. Like the 20/10/5% for intervals, these are household defaults, changeable per requirement and per document.
- **The two airworthiness compliance modes** ("initial deadline, then recurring" and "recurring until a terminating action") and **supersession** move to the **aircraft phase**. Recalls are one-time, which already exists.
- **The household's defaults** are changed in **Admin mode → Settings → Household** ("Due soon warnings"). Maintainers change them per requirement and per document.
- **The Overview** shows the **5 most urgent** items (requirements and expiring documents), each with its step and what's left, and **See all** to the Maintenance tab.
- **The Garage:** each card gets a **badge** with its vehicle's most urgent step, and **Most urgent** becomes the default sort (#040). The fleet strip comes with milestone 3's messages.
- **A job's reset:** a task whose requirement has a rule on a resettable meter shows **↺ Reset Oil life to 100%**, ticked by default. It adds a reset reading to the job's end readings.
- **The sketch: 1A, 2A, 3A, 4A.**
  - **1A:** the schedule cards get the step's colour on their left edge, a chip, and what's left in that colour.
  - **2A:** the Overview's **Coming up** is a list: the name, what's left, and a chip, then **See all on Maintenance**.
  - **3A:** the Garage card has a chip in its corner, and nothing when everything is OK.
  - **4A:** a requirement's warnings are folded ("Household default: 20% / 10% / 5% left · Change").

**The rules, in `domain::due` (test-first):**
- **Steps:** OK, **Coming up** (yellow), **Due soon** (orange), **Due very soon** (red), and **⚠ Overdue** (a darker red, always with its word). Colour is never the only sign. A step is reached when what's left is at or below its share: of the interval for an interval rule (20/10/5%), and in days for a date (60/30/14).
- **Each rule's next due:**
  - every N on a meter, from the reading it was last done at;
  - every N days, months, or years (end of month as before), from when it was last done;
  - a fixed date, from the last task's next date or the rule's first;
  - **when a meter reaches X** (new, for a counting-down or counting-up meter), from its latest reading, with its steps measured over the span from its reset value to X.
- **A requirement's step** is its most urgent rule's ("whichever comes first"), with the earliest date (projected or fixed) and what's left on each meter rule ("in 310 mi or Oil life 9%").
- **Only what's on the schedule** comes due (`Requirement::on_schedule`): turned on, applies, and not a one-time one already done.
- **A document's step** comes from its expiry and its own (or the household's) day steps. A renewed (superseded) document doesn't come due.

**Steps:**

| Step | Status | What |
|---|---|---|
| **2.0** `docs:` | ☑️ | This plan; the sketch; decision #082; the airworthiness modes moved to the aircraft phase in `plan.md`. |
| **2a** `feat(domain):` what's due | ☑️ | `domain::due`: the steps and their defaults; each rule's next due; `IntervalRule::Reaches`; "last done before these records"; the 90-day projection; a requirement's and a document's step; a vehicle's items in urgency order. Unit and property tests first (whichever comes first; end of month; never done; a reset meter; too few readings). |
| **2b** `feat(storage):` migration 0023 | ☑️ | `requirements` gains "last done before" (a date and readings) and its warning steps, `asset_documents` its warning days, and the household's preferences the default steps. The release fixtures migrate. |
| **2c** `feat(server):` due on the server | ☑️ | The vehicle's due items (Maintenance, Overview), each Garage card's most urgent step and the **Most urgent** sort, the household's due defaults (the Admin), and the requirement and document forms' new fields. The desktop gets the same answers offline, from its own copy. |
| **2d** `feat(app):` the screens | ☑️ | The schedule cards (row 1), the Overview's **Coming up** (row 2), the Garage badge and sort (row 3), the requirement dialog (row 4: last done before, its warnings, and the **reaches X** rule), the document form's warning days, the task's **↺ Reset** in a job, and **Due soon warnings** on the Household tab. |
| **2e** `docs:` | ☑️ | Milestone 2's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**New dependencies:** none expected.

**Open issues** (reviewed 2026-11-16): none touch what's due. #24 (a vehicle's main photo) could join the Garage card later; not now.

**Step 2a notes (2026-11-16):**
- **`domain::due`:**
  - `DueStep`, with OK < Coming up < Due soon < Due very soon < Overdue, so the most urgent is the greatest. Its words are "OK", "Coming up", "Due soon", "Due very soon", "⚠ Overdue".
  - `IntervalSteps` (20/10/5% of the interval left) and `DaySteps` (60/30/14 days), together the household's `DueDefaults`, with a check that each step comes no earlier than the one before it. Exactly due is **Due very soon**; one past it is **Overdue**.
  - `requirement_due` gives a requirement's step, each rule's step and what's left (`Left::Meter`, with a projected date, or `Left::Date`), the earliest date (`DueDate`, with `estimate` for a projected one), and **not done yet**. It's `None` for what isn't on the schedule (#068), or with no rule it can work out.
  - `document_due` gives a document's step by days; it's `None` for one renewed or that never expires.
  - `most_urgent_first` sorts by step, then the soonest date (none last).
- **Counting from:** the latest finished task. Else `Requirement::before_records` (`LastDoneBefore`: a date and readings). Else the meter's first reading, or the day the requirement was added (`added_on`, which 2c passes in).
- **"When a meter reaches X":** `IntervalRule::Reaches { meter, at }`, checked like a meter rule (the vehicle's meter, the mark zero or more, once per meter). It can sit beside an "every" rule on the same meter. Its steps are measured over the span from the meter's reset value (or the reading when last done) to the mark.
- **Projection (`usage`):** the meter's movement in its own direction over consecutive readings in the last 90 days, never across a reset, from at least two readings 7 or more days apart. Else all its readings; else none. Integer arithmetic throughout.
- **New fields** (serde defaults, so older records read as before):
  - `Requirement::before_records` and `Requirement::warnings` (its own `DueDefaults`);
  - `AssetDocument::warn_days`;
  - their patches.

  Storage writes them from 2b; until then they read as empty.
- **Copying a schedule** moves a "reaches" rule to the matching meter, like an "every" rule (`IntervalRule::on_meter`). It leaves "last done before" behind, since that's the other vehicle's past.
- **Ahead of 2d, because the rule needed them:**
  - the schedule card says "when Oil life reaches 15 %";
  - the requirement dialog has **+ When a meter reaches** (offering a resettable meter first) with its meter and mark, and its round trip.
- **Tests:**
  - the steps at their shares, and a property test that less left is never less urgent;
  - a meter rule;
  - whichever comes first (a date);
  - Oil life falling to its mark;
  - never done, from the first reading or from before the records;
  - a calendar rule from the day added, with end of month;
  - fixed dates before and after a sign-off;
  - a projected date;
  - too few readings;
  - use across a reset;
  - only what's on the schedule;
  - a requirement's own warnings;
  - documents (due soon, expired, renewed, never expiring);
  - the urgency order;
  - the reaches rule's checks and `on_meter`; a copy leaving "last done before" behind.
- **Tidying:** adding the two fields pushed `server::references::add_reference` over the 80-line limit, so its permission checks moved into `may_add_reference`.

**Step 2b notes (2026-11-16):**
- **`0023_due.sql`** only adds nullable columns, so every existing row reads as before:
  - `requirements.before_records` and `requirements.warnings`;
  - `asset_documents.warn_days`;
  - `preferences.due_warnings` (the household's record only).

  Each is JSON text. "When a meter reaches X" lives inside the existing `requirements.rules` JSON, so it needs no change.
- **Storage:** the requirement and document writers store the new fields, and a patch can set or clear each. New helpers in `storage::convert`: `optional_json`, `optional_from_json`, and `clearable_json`. The household's due warnings are the one structured preference: `ColumnChange::json` writes them, and a value this build can't read counts as "not chosen", like an unknown name.
- **`domain::preferences`:** `due_warnings` is household-only, like what issues are called. `Preferences::resolve` takes it from the household alone. The server refuses it in a person's own preferences, ahead of 2c, so the gap never existed.
- **Tests:**
  - a requirement keeps its date before the records and its own warnings, and both clear;
  - a document's warning days set and clear;
  - only the Admin sets the household's due warnings, everyone sees them, and they go back to the defaults;
  - the release fixtures migrate with all the new columns empty.

**Step 2c notes (2026-11-16):**
- **Today is the person's:** each call takes the device's time zone, as the documents' status already does. The data layer asks the browser or webview itself (`ui::device_zone`, which `use_device_zone` now uses too), so screens don't pass it. An unknown zone falls back to UTC.
- **`server::due`:**
  - `vehicle_due` gathers one vehicle's meters, readings, finished tasks, requirements, documents, and the household's warnings, and asks `domain::due`. It returns `DueEntry`s (a requirement with its name, or a document with its type), most urgent first, by `due::urgency_order`.
  - `list` serves the Overview to anyone whose role covers the vehicle; it's **Not found** otherwise.
  - When a requirement was added comes from the clock of its `asset` field, which is set once and never changes (`Database::requirements_added`).
- **The schedule view** (`schedule::view`) gives each requirement its `due`.
- **The Garage** gives each card its most urgent step (`AssetSummary::due`), and **Most urgent** is a new sort and the default (#040). Ties go by name.
- **Checks:**
  - a requirement's own warnings, and "last done before" (a date; the vehicle's meters, each once; none negative), via `check_due_settings`;
  - a document's warning days (`DocumentProblem::BadWarnDays`);
  - the household's warnings when the Admin sets them (`PreferencesOutcome::BadDueWarnings`).

  Edits carry the new fields.
- **Endpoints:**
  - The Garage moved from `GET /api/v1/assets` to `POST /api/v1/assets/garage`, now that it takes the zone.
  - `POST /api/v1/due` is new, with its own `server_side::with_due` wrapper.
  - `POST /api/v1/requirements` takes the zone.
  - On the desktop, the same server functions run on its copy, offline included. The due list has no caller until the Overview in 2d.
- **The Garage's sort menu** offers **Most urgent**, since it's now the default.
- **Tests:**
  - server (`tests/due.rs`): a truck's oil change counted from before the records, and its insurance, both **Due soon**, on the list, the schedule, and the Garage; a vehicle Sam can't see; refusals for backwards warnings, an unknown meter, a bad date, and backwards document days;
  - the household's backwards warnings refused;
  - domain: `check_due_settings`, and the Most urgent order.

**Step 2d notes (2026-11-16):**
- **`ui::screens::due_view`:**
  - `DueChip`, the step's word in its colour, so colour is never the only sign;
  - `step_class`;
  - `left_text`: "in 310 mi or by 12 Dec · ~19 Nov", "1 mi over", "was due 2 Nov", with "Not done yet" first when it applies. A date reads "was due" when its own rule is overdue.
- **Colours:** set on the app's theme selectors (light, dark, and the system's choice). Yellow, orange, red, and a darker red on a tinted background for Overdue.
- **1A, the schedule cards:** a coloured left edge, the chip beside the name, and what's left in the step's colour above the rules.
- **2A, the Overview:** the placeholder "Coming due" panel is now **Coming up**, the 5 most urgent items (requirements by name, documents by type) with what's left and the chip, then **See all on Maintenance →**. With nothing set up, it says so.
- **3A, the Garage card:** the chip replaces "Readings and due items arrive later", and shows nothing when everything is OK. The lifecycle badges stay.
- **4A, the requirement dialog** (`DueSettings`), below the rules of a recurring one:
  - **"Last done before these records"**: a date, and a reading for each meter its rules count on.
  - **"Warn me"**, folded to "Household default: 20 / 10 / 5% left · 60 / 30 / 14 days before a date · Change". Change opens the steps, pre-filled with the household's (percent of the interval, and days before a date). **Use the household's warnings** folds them again.
- **The document dialog:** "Warn me", once it has an expiry, folded the same way in days. A renewal keeps the old document's choice.
- **A job's task:** **↺ Reset Oil life to 100 % at the end of the job**, for each resettable meter its requirement counts on. It's ticked when the task is added, from the menu or a bundle. `TypedWork.resets` turns it into a reset end reading at the meter's reset value, and editing the job ticks it again rather than showing it as a typed end reading.
- **Settings:** **Due soon warnings** on the Household tab (the steps, **Save**, and **Use the built-in warnings**). The You tab shows them in words, set by the Admin.
- **Shared pieces:** `TypedWarnings`, `StepInputs`, `days_typed`, and `parse_days` are used by the requirement, the document, and the Household tab alike.
- **Tests:**
  - every step has its word and colour;
  - what's left reads as one line, including overdue and not done yet;
  - a ticked reset becomes a reset end reading at the reset value (and a non-resettable meter's tick does nothing), and comes back ticked.
- **Tidying:** clippy asked for the day steps by value (`DaySteps` is 6 bytes), and `typed_work` grew past 80 lines, so a task's part moved into `typed_task`.
- **Not seen running yet:** all of these screens. They're in the phase's test plan (2e).

**Step 2e notes (2026-11-16):**
- **The test plan:** milestone 2's section of `docs/test-plans/p5-phase.md`, 17 steps, run at the end of the phase (#081). They cover:
  - the migration;
  - the Garage's sort and chips, and the Household tab's warnings;
  - an oil change counted from before the records, with three rules, going **Due soon** then **⚠ Overdue**;
  - a requirement's own warnings, and backwards ones refused;
  - insurance by days, and its own days;
  - a job resetting Oil life and setting the oil change back to OK;
  - the desktop offline, and a Driver's view.
- **A wording fix found while writing it:** a meter other than distance now names itself in "what's left": "Oil life: 80 % to go", "Engine hours: 12 h over". A distance still reads "in 300 mi". Tested.

**Not in this milestone:** due and overdue messages and snooze, and the fleet strip (milestone 3); service bundles' sign-off and household templates (milestone 4); readiness (milestone 6); the airworthiness modes and supersession (the aircraft phase).

### Milestone 3 plan: due messages and snooze

*Agreed 2026-11-16 · branch `milestone/p5-m3-messages` · sketch: `docs/ui-sketches/p5m3-messages-options.svg` · Decision #083*

**Choices (the human, 2026-11-16):**
- **When a message first appears:** at **Due soon**, a household choice (Coming up, Due soon, or Due very soon) on the Household tab beside the due warnings. Overdue always sends one.
- **Snooze and urgency:** **Overdue breaks through.** A snooze holds while the item moves from Due soon to Due very soon; going Overdue brings it back, unread.
- **Snooze is offered** as quick choices plus **Custom…**.
- **Due & squawks**, the sidebar's placeholder, is built now, and the fleet strip's counts open it.
- **Who:** anyone with a role on the vehicle.
- **Snooze and dismiss are per person.**
- **Overdue is a Warning**, with no banner.
- **The sketch: 1A, 2A, 3A, 4A.**
  - **1A:** a **Snooze ▾** button beside **Open on Maintenance** and **Dismiss** opens 1 day, 1 week, 1 month, a meter amount (500 mi), the two together, and **Custom…**.
  - **2A:** the fleet strip is four tiles above the groups: **⚠ Overdue**, **Due soon**, **Open squawks**, and **Synced** (the desktop app only).
  - **3A:** Due & squawks is one list, most urgent first, the vehicle on each row, then the open squawks; filter chips All, Overdue, Due soon, Squawks.
  - **4A:** "Messages about what's due start at" on the Household tab; a **Snoozed** filter in Messages, each with when it wakes and **Wake now**.

**The rules (test-first):**
- **A due message** is worked out like every other (#050): it exists while its item is at or past the household's starting step, for each person whose role covers the vehicle, and goes when the item is signed off or renewed.
- **Its key** names the item and its **cycle** (the last finished task, or "first" before one; a document is its own cycle). **Overdue has a key of its own**, so it's a new, unread message whatever happened to the first. Both show the step and what's left now.
- **Severity:** Coming up is Info; Due soon, Due very soon, and Overdue are Warnings.
- **Its button** opens the vehicle's Maintenance tab (a document: its Documents tab).
- **Snooze** is kept on the person's mark: until a time, until the item's meter reaches a value, or both, whichever comes first. The meter is the first counting-up meter the item's rules use; an item with none snoozes by time only. A snoozed message isn't in the inbox or the unread count, but is under **Snoozed**. **Wake now** clears it.
- **The fleet counts** are items, not vehicles: Overdue, Due soon (Due soon and Due very soon), and open squawks (deferred ones included), over the vehicles the person can see. Coming up isn't counted.

**Steps:**

| Step | Status | What |
|---|---|---|
| **3.0** `docs:` | ☑️ | This plan; the sketch; decision #083; milestone 2 marked ✅. |
| **3a** `feat(domain):` due messages | ☑️ | `MessageKind::Due`, their keys (cycle, and Overdue apart), severity, and button; `Snooze` on `MessageMark` and its patch, and the inbox leaving out snoozed messages until they wake; the household's `due_messages_from`; the fleet counts. Unit tests first (the starting step; Overdue past a dismiss and a snooze; a sign-off starting a new cycle; waking by time and by meter; role filtering). |
| **3b** `feat(storage):` migration 0024 | ⬜ | `message_marks.snooze` and `preferences.due_messages_from`, nullable JSON, so existing rows read as before. The release fixtures migrate. |
| **3c** `feat(server):` | ⬜ | Due messages in each person's inbox, for the vehicles their role covers (the inbox takes the device's time zone, as the Garage does); **Snooze** and **Wake now**; the fleet counts on the Garage; the Due & squawks list across vehicles; the Admin's starting step. The desktop gets the same, offline, from its copy, and its marks sync as before. |
| **3d** `feat(app):` the screens | ⬜ | The **Snooze ▾** menu and **Custom…** (row 1), the fleet strip (row 2), the Due & squawks screen in place of its placeholder (row 3), the Household setting and the **Snoozed** filter (row 4). |
| **3e** `docs:` | ⬜ | Milestone 3's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**Step 3a notes (2026-11-16):**
- **`domain::inbox`:**
  - `MessageKind::Due` carries the vehicle, its name, what comes due (`DueSubject`), and where it stands (`Due`). Its titles are the step ("Due soon", "⚠ Overdue"), and its text names the vehicle, the item, and what's left.
  - `due_message` makes it, or nothing: OK, or before the household's starting step, gives none, and Overdue always gives one.
    - **Keys:** `due:<item>:<cycle>`, where the cycle is the finished task its count runs from, or `first`. Overdue is `due-overdue:<item>:<cycle>`, so it's a new, unread message whatever was done with the first.
    - **Severity:** Coming up is Info; the rest are Warnings.
    - **Its button:** `MessageAction::OpenMaintenance` (a requirement) or `OpenDocuments` (a document).
  - **Snooze:**
    - `Snooze` (until a time, until a meter reaches a value, or both) sits on `MessageMark`, and its patch can set or clear it.
    - `SnoozeRequest` (days, from 1 to 366, and/or an amount more than zero) turns into one at a moment, from the meter's latest value.
    - `inbox` now takes now and each meter's latest value. It sets asleep messages aside in `Inbox::snoozed` (timed ones soonest first, then those waiting on a meter), out of the list and the unread count. A woken one comes back with its read state.
- **When a message is raised:** `RuleDue::since` and `Due::since` give when an item reached its step: the reading that took it there, or the start of the day a date's step began. `document_due` takes the zone for that. The inbox orders by it, and it doesn't change with each new reading.
- **`domain::due`:**
  - `snooze_meter`: the first counting-up meter a requirement's rules use (Oil life counts down, so it isn't used).
  - `FleetCounts::of`: the counts for the fleet strip; Coming up isn't counted.
  - `DueStep::starts_messages`: Coming up, Due soon, or Due very soon.
- **Preferences:** `due_messages_from`, the household's alone (default Due soon), resolved like the due warnings. Older stored preferences read as Due soon. `Preferences` now spells out its `Default`, since a step has no sensible one of its own.
- **Until 3b and 3c:**
  - storage reads the new fields as empty;
  - the server passes no meter values to `inbox`, since nothing can snooze yet;
  - the Messages screen names a due message's vehicle, item, and step, and its button opens the vehicle's tab. What's left in full comes in 3d.
- **Tests:**
  - domain: a step reached by the reading that took it there, and by the start of a date's day; the snooze meter; the fleet counts; which steps can start messages; the household's starting step;
  - inbox: the starting step, severity, and button; one message per cycle and one for Overdue; Overdue after a dismiss and a snooze; waking by time or by meter; snooze requests; the mark's new field.

**API version:** stays at 2. Snooze is a new field on an existing record, not a new operation, and older desktops are already stopped by the schema check (as in milestone 2).

**New dependencies:** none expected.

**Open issues** (reviewed 2026-11-16): none touch messages or snooze.

**Not in this milestone:** squawk messages (plan §5 lists them; not planned into a milestone yet), notifications outside the app and email (post-MVP, idea 13), specs and bundles (milestone 4), readiness (milestone 6).
