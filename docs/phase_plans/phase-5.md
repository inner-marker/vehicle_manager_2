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
| 3 | **Due messages and snooze** | ✅ | Due-soon and overdue messages in Messages, worked out from the records (so they also work offline on the desktop); **snooze** by time, distance, or hours, whichever comes first; per-vehicle roles respected; the Garage's **fleet strip** and the **Due & squawks** screen (#083) | Snooze "1 week or 500 mi"; it returns at whichever comes first |
| 4 | **Specs and service bundles** | ✅ | The vehicle's **Specs** list (a name, a value with units or a part number, a note); requirements link to specs; a logged task's **parts pre-filled** from them (editable; "update the spec too?"); **bundled sign-off** (one event, each task's clock reset); named household **templates**, copied onto a vehicle (#031) | The oil change pre-fills oil and filter; the 10k service signs off three tasks |
| 5 | **Fuel economy** | ✅ | **Stretches** from one full fill to the next, with partials added in; a missed fill-up drops its stretch; **ⓘ** on **Filled full** and **Missed previous fill-up**; averages, a chart with gaps (drawn by the app, no chart library), breakdowns by condition and towed vehicle; the Overview's latest figures; the vehicle-reported figure beside the app's (#033) | Partial and missed fills behave as the ⓘ says |
| 6 | **Readiness** | ✅ | **Usages** per vehicle (#086): a default named for its kind (Drivable, Towable, Usable) needing its profile's documents, and more added by Maintainers; each requirement **needed for** chosen usages; **"Ready to use?"** on the Overview, each usage with every item's ✓, !, or ✗ and its reason; the Garage chip; who may sign off or defer an issue, enforced (#063) | Expired insurance: not ready; an open or deferred High: not ready |
| 7 | **Calculated fields and derived meters** | ☑️ | **Formulas** over built-in and custom fields by their stable keys, unit-aware, worked out when shown and never stored (#057). **Derived meters** worked out from other meters (e.g. odometer + idle hours × 33), usable in interval rules; a percentage can be derived too | A derived meter drives a rule; a calculated field on a fuel-up |

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
| **3b** `feat(storage):` migration 0024 | ☑️ | `message_marks.snooze` and `preferences.due_messages_from`, nullable JSON, so existing rows read as before. The release fixtures migrate. |
| **3c** `feat(server):` | ☑️ | Due messages in each person's inbox, for the vehicles their role covers (the inbox takes the device's time zone, as the Garage does); **Snooze** and **Wake now**; the fleet counts on the Garage; the Due & squawks list across vehicles; the Admin's starting step. The desktop gets the same, offline, from its copy, and its marks sync as before. |
| **3d** `feat(app):` the screens | ☑️ | The **Snooze ▾** menu and **Custom…** (row 1), the fleet strip (row 2), the Due & squawks screen in place of its placeholder (row 3), the Household setting and the **Snoozed** filter (row 4). |
| **3e** `docs:` | ☑️ | Milestone 3's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

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

**Step 3b notes (2026-11-16):**
- **`0024_due_messages.sql`** only adds nullable columns, so every existing row reads as before:
  - `message_marks.snooze`, as JSON (`until`, and `meter` with its value);
  - `preferences.due_messages_from`, by name like the other settings (`due-soon`).
- **Storage:** a mark's writer stores its snooze, and a patch can set or clear it (Wake now) while read and dismissed changes leave it alone. A snooze this build can't read counts as none, so the message shows. Sync copies whole rows, so marks' snoozes reach the desktop with no change there.
- **Ahead of 3c, so the gap never existed:** the server refuses `due_messages_from` in a person's own preferences, and the Admin's must be Coming up, Due soon, or Due very soon. Anything else gets the same answer as backwards warnings (`BadDueWarnings`), since the screen only offers those three.
- **Tests:**
  - storage: a snooze kept through a dismiss, cleared, and an unreadable one ignored; the starting step stored by name and put back to the default;
  - the release fixtures migrate with no snoozes and no starting step;
  - server: a person can't choose their own starting step; OK and Overdue are refused; the Admin's choice is everyone's, and goes back to Due soon.

**Step 3c notes (2026-11-16):**
- **Which vehicles:** due messages, the fleet strip, and Due & squawks cover the vehicles the Garage shows the person: their role covers it, and it isn't retired or disposed (`due::shown_vehicles`). The Garage's own badges still show on archived cards.
- **`server::due`:**
  - `vehicle_due_in_full` also gives the vehicle's finished tasks (each message's cycle) and each meter's latest value (for snoozes); `vehicle_due` and the Garage use it.
  - `due_messages`: each item at or past the household's starting step, as `domain::inbox::due_message` makes it.
  - `snooze_meter_of`: the item's counting-up meter and its latest value.
  - `fleet`: Due & squawks. What comes due (Coming up or more urgent), most urgent first and then by vehicle; open and deferred squawks, as the vehicle's page shows them, most severe first and then oldest; and the counts.
- **Messages:**
  - The inbox and marks take the device's time zone, so `GET /api/v1/messages` became `POST`. The mark carries the zone too.
  - `MarkMessage::Snooze` (with a `SnoozeRequest`, marking it read) and `MarkMessage::Wake`.
  - A snooze on anything but a due message, or one that can't be used, is `InboxOutcome::BadSnooze`; the app says "That snooze can't be used".
  - **Overdue replaces the first message,** not joins it: the first message (and its snooze) is gone once the item is overdue, so Wake on it is Not found. Decision #083's wording now says so.
- **The Garage:** `GarageView::fleet`, the strip's counts: items over the vehicles it shows, and their outstanding squawks.
- **Endpoints:** `POST /api/v1/due/fleet` is new (`server_side::with_fleet`). On the desktop, the same server functions run on its copy, offline included, and marks sync as before.
- **Tests** (server, `tests/due.rs`):
  - Sam's message at Due soon; a snooze needing days or an amount; "1 week or 500 mi" asleep until the odometer passes, then back, read, at Due very soon; snoozed again, then Overdue as a new unread message; Wake;
  - Due & squawks and the Garage's counts agree; a retired car's expired insurance isn't counted or sent.

  The older message tests take the zone.
- **Tidying:** the fleet counts pushed `assets::garage_view` over 80 lines, so filling in each card (its due step, plate, and standing) and counting the fleet moved into `fill_in_summaries`.

**Step 3d notes (2026-11-16):**
- **What a due message carries:** `MessageKind::Due` now holds a `DueNotice`: the vehicle, the item, where it stands, the vehicle's meters its rules count on (`due::meters_in`), and its snooze meter. The screens need no other request to say what's left or to offer "500 mi". `FleetDue` carries its meters the same way. `MessageKind` no longer derives `Hash` (nothing used it, and a meter isn't hashable).
- **Messages (1A, 4A):**
  - a due message is titled by vehicle and item ("Outback: Engine oil & filter"), with the step's chip, and what's left in the step's colour;
  - **Snooze ▾** (`ui::screens::snooze`): 1 day, 1 week, 1 month, an amount fitted to the item's meter (500 mi or km, 10 h; none for a percentage), the two together, and **Custom…** (days and/or an amount in the meter's unit, whichever comes first). An item without a counting-up meter offers time only;
  - snoozed messages wait under **Snoozed (n)**, each saying "Snoozed until … or 85,210 mi", with **Wake now**.
- **The desktop, offline:** the data layer asks the server for the inbox first (so the server's own messages show), and when it can't be reached works it out from the copy: due messages, forgotten jobs, and overdue returns. **Marking needs the server:** there's no operation for marks, and adding one would raise the API version, which this milestone keeps at 2. Offline, Read, Dismiss, and Snooze say the server can't be reached. A follow-up could add a mark operation.
- **The fleet strip (2A)** on All vehicles (its counts are the household's, so not on a group's page): **⚠ Overdue**, **Due soon**, **Open** squawks (in the household's word), each opening Due & squawks with that filter at `/due/<filter>`, and on the desktop app **Synced**, how many changes wait to be sent, or **Offline**, opening Messages.
- **Due & squawks (3A)** replaces the placeholder: chips All, Overdue, Due soon, and the squawks; due items with their vehicle, what's left, and chip, opening the vehicle's Maintenance or Documents tab; then open squawks with their vehicle, severity, and status, opening the squawk. On the desktop it's read from the copy, offline too.
- **Settings (4A):** "Messages about what's due start at" on the Household tab, a menu of Coming up, Due soon, and Due very soon; the You tab shows it in words.
- **Tests:** the quick choices for a distance, an hours, and a percentage meter; a custom snooze's checks; "Snoozed until … or 85,210 mi"; a due message's title and text; each Due & squawks filter, and its name in the address; the new route.
- **Tidying:** carrying each meter's snooze pushed `server::due::vehicle_due_in_full` over 80 lines, so its latest values moved into `latest_values`. The sync tile is a small `SyncTile` enum rather than a nested `Option`.
- **Not seen running yet:** all of these screens. They're in the phase's test plan (3e).

**Step 3e notes (2026-11-17):**
- **The test plan:** milestone 3's section of `docs/test-plans/p5-phase.md`, 21 steps, run at the end of the phase (#081) after milestone 2's, whose last steps leave the oil change overdue. They cover:
  - the migration;
  - the fleet strip and Due & squawks, with their filters;
  - Sam's due message, opening Maintenance, and **Snooze ▾**'s choices;
  - "1 week or 500 mi" asleep, Jordan still seeing it, and the odometer waking it;
  - Custom's refusals, and **Wake now**;
  - the desktop online (the **Synced** tile) and offline (messages from the copy; snoozing needs the server);
  - a sign-off ending the messages; Due soon snoozed for a month, then Overdue getting past it;
  - the household's starting step, with a document's Coming up message (time-only snooze), and the You tab.
- **Checked against the code while writing it:** whether the sidebar marks **Due & squawks** at `/due/overdue` isn't certain (the link matches its own address), so the plan doesn't promise it; an offline snooze's words are quoted as the app has them.

**API version:** stays at 2. Snooze is a new field on an existing record, not a new operation, and older desktops are already stopped by the schema check (as in milestone 2).

**New dependencies:** none expected.

**Open issues** (reviewed 2026-11-16): none touch messages or snooze.

**Not in this milestone:** squawk messages (plan §5 lists them; not planned into a milestone yet), notifications outside the app and email (post-MVP, idea 13), specs and bundles (milestone 4), readiness (milestone 6).

### Milestone 4 plan: specs and service bundles

*Agreed 2026-11-17 · branch `milestone/p5-m4-specs` · sketch: `docs/ui-sketches/p5m4-specs-options.svg` · Decision #084*

**Choices (the human, 2026-11-17):**
- **A spec's value is typed, with units:** volume (qt, L, US and imperial gal, fl oz), torque (ft-lb, in-lb, N·m), pressure (psi, kPa, bar), and length (in, mm). A spec can also be just a name and a part number.
- **Shown as entered, with the other system beside it:** "5.5 qt (5.2 L)".
- **A pre-filled part's quantity** is the spec's amount, with its unit in the text ("Engine oil 5W-30 (qt)", 5.5). Torque, pressure, and length specs aren't parts: they show on the task as reminders.
- **"Update the spec too?"** is asked when the job is saved.
- **Templates:** Maintainers save and apply them, and they hold requirements, bundles, and specs. The Admin renames and deletes them.
- **Log this service** on each bundle card, with parts pre-filled.
- **The sketch: 1A, 2A, 3A, 4B, 5A.**
  - **1A:** Specs is a third view on the Maintenance tab: Schedule · Specs · Set up. Everyone sees it; Maintainers get **+ Spec**.
  - **2A:** the spec dialog (name, kind, amount and unit with the conversion beside it, detail, part number, note), and a **Specs** checklist in the requirement dialog.
  - **3A:** saving a job asks once, listing each changed part, ticked.
  - **4B:** a **Templates** screen under Manage, each template with what it holds and **Apply to…**; new ones come from a vehicle's Set up (**Save as template…**).
  - **5A:** **Log this service** on the Schedule's bundle card.

**The rules, in `domain` (test-first):**
- **A spec:** a name (a label, up to 40 characters), an optional amount (more than zero, at most 3 decimals) in one of the units, an optional detail (up to 100), part number (up to 40), and note. Its kind comes from its unit.
- **Conversion** (for show only) with `uom`, as fuel volumes already use: volume to the other system (qt ↔ L, gal ↔ L, fl oz ↔ mL), torque (ft-lb and in-lb ↔ N·m), pressure (psi ↔ kPa; bar ↔ psi), length (in ↔ mm).
- **A requirement's specs:** the vehicle's own, each once. Removing a spec takes it off its requirements.
- **Pre-filling a task:** a part for each ticked volume spec, and each ticked spec with a part number and no amount; a reminder for each torque, pressure, or length spec. A pre-filled part remembers its spec, so saving can tell what differs.
- **What differs:** a part from a spec whose part number or quantity was changed. Updating the spec takes the part's.
- **Templates:** a household record holding requirements (with meters named, not by ID), bundles, and specs. Applying copies onto a vehicle as Copy schedule does, with specs, re-linked; what can't be matched is left out and listed. Copy schedule copies specs too.

**Steps:**

| Step | Status | What |
|---|---|---|
| **4.0** `docs:` | ☑️ | This plan; the sketch; decision #084; milestone 3 marked ✅. |
| **4a** `feat(domain):` specs and templates | ☑️ | `domain::spec` (the spec, its units and conversions, its checks); `Requirement::specs`; a part's spec; pre-filling a task's parts and reminders; what differs from the specs; `ScheduleTemplate`, saved from a vehicle and applied as a copy; Copy schedule with specs. Unit tests first. |
| **4b** `feat(storage):` migration 0025 | ☑️ | A `specs` table, `requirements.specs`, a job's part's spec, and a `schedule_templates` table; both new tables reach the desktop's copy. The release fixtures migrate. |
| **4c** `feat(server):` | ☑️ | Specs (everyone whose role covers the vehicle sees them; Maintainers change them); updating specs after a job; templates (save from a vehicle, apply to one, rename and delete by the Admin); Copy schedule with specs. |
| **4d** `feat(app):` the screens | ☑️ | The Specs view and dialog (rows 1, 2), the requirement's Specs checklist, pre-filled parts and reminders on a job's task, "Update the spec too?" (row 3), the Templates screen and **Save as template…** (row 4), and **Log this service** (row 5). |
| **4e** `docs:` | ☑️ | Milestone 4's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**Step 4a notes (2026-11-17):**
- **`domain::spec`:**
  - `SpecUnit` (14 units) and `SpecKind` (volume, torque, pressure, length). Each unit knows its kind, its short name, and the other system's unit with the decimals that suit it (qt and gal to L, L to qt, fl oz to mL, ft-lb and in-lb to N·m, psi to kPa, kPa and bar to psi, in to mm, mm to in).
  - `SpecAmount`: thousandths of the unit as entered (5.5 qt is 5,500), up to three decimals, above zero. `show` gives "5.5 qt"; `show_both` gives "5.5 qt (5.2 L)", converted with `uom` for show only.
  - `Spec` (a name, amount, detail up to 100 characters, part number up to 40, note), `SpecId`, `SpecPatch` (with `all` and `changes`), and `check_spec`. The name is a label, so up to 40 characters (the plan said 60; corrected).
  - `Spec::use_on_task`: a **part** for a volume, or a part number with no amount; a **reminder** for a torque, pressure, or length; nothing for a name alone. `task_specs` gives a requirement's parts (description "Engine oil 5W-30 full synthetic (qt)", cut to a part's 100 characters; the amount as the quantity, 1 without one) and reminders.
  - `spec_updates`: the parts from specs whose part number, or for a volume whose quantity, now differs, one per spec, each with its `SpecPatch`. A cleared part number offers clearing the spec's.
- **`domain::template`:** `ScheduleTemplate` (a name, the meters its rules use, requirements, bundles, specs), `TemplateId`, `ScheduleTemplatePatch`, and `TemplateCounts`. `from_vehicle` leaves each requirement's recall and "last done before" behind; `apply` is Copy schedule from the template's meters.
- **Elsewhere in `domain`:**
  - `Requirement::specs` (the ticked specs, in order) and its patch field; `check_specs` (the vehicle's, each once; `RequirementProblem::BadSpecs`).
  - `copy_schedule` takes and returns specs (with their old IDs, for re-linking). Until 4c, the server and the copy preview pass none.
  - `Part::spec`: the spec a part was pre-filled from. Storage reads it as none until 4b; parts are stored as rows, so 4b adds a column.
  - The four number helpers in `quantity` are shared within `domain`.
- **Tidying:** clippy asked for the four base units' conversions to share one arm each way, and a part's description built without `format!`. Adding `spec: None` pushed a storage test's oil change over 80 lines, so its parts moved into `oil_parts`.
- **Tests:** every unit's kind, short name, and round trip; eight conversions as shown; amounts' checks; a spec's checks; parts and reminders from ticked specs, an unknown one skipped; a long name and detail fitting a part; what a job offers back (a changed quantity and part number, a hand-typed part ignored, a cleared number); a spec patch's changes; a template leaving the vehicle's past behind and keeping only used meters; applying one (a meter matched ignoring case, one left out and listed, specs moved); a requirement's specs checked.

**Step 4b notes (2026-11-17):**
- **`0025_specs.sql`:**
  - `specs`: a vehicle's specs. An amount is kept as entered, in two columns that are set together: thousandths (`amount_milli`) and the unit's name (`amount_unit`, e.g. `quarts`).
  - `requirements.specs`: the ticked specs, a JSON list, empty for every existing requirement.
  - `task_parts.spec_id`: the spec a part was pre-filled from. It has no foreign key, so a job keeps what it recorded if the spec is removed later.
  - `schedule_templates`: each template's meters, requirements, bundles, and specs as JSON.
- **Storage:**
  - `storage::specs` (`SpecWriter`, `spec`, `specs_of`, by name) and `storage::templates` (`TemplateWriter`, `template`, `templates`, by name).
  - The requirement writer stores `specs`, and a job's parts their spec.
- **The desktop's copy:** `specs` and `schedule_templates` join the synced record tables (after `assets`, for the foreign key). Sync copies whole rows, so the new columns come with them. Templates are the household's, so everyone's copy has them.
- **Tests:**
  - a spec's amount stored as entered, changed to another unit, its detail cleared, a part number set, the amount cleared, and the spec removed;
  - a requirement's specs kept in order and changed;
  - a template's round trip, renamed with the rest unchanged, then removed;
  - a job's part keeping its spec (the maintenance round trip);
  - the release fixtures migrate with no specs, templates, ticked specs, or parts from specs.
- **Tidying:** the test of migration 0015 writes events with today's writer on a version-14 database, so it now adds `task_parts.spec_id` while it does (as it already did for readings' `reset`) and drops it before 0015 runs. The release fixture's oil change moved into its own function, keeping `everyday` under 80 lines.

**Step 4c notes (2026-11-17):**
- **Specs on the server (`server::schedule`):**
  - The schedule's answer carries the vehicle's specs, by name, so everyone who sees the schedule sees them.
  - `add_spec`, `edit_spec`, and `delete_spec` need the same permission as changing the schedule (a Maintainer on the vehicle, in Maintainer mode or above). A spec stays on its vehicle; a bad one is `ScheduleOutcome::InvalidSpec`.
  - **"Update the spec too?"** is just `edit_spec`: the screen offers it only to someone who may change the schedule.
  - Removing a spec unticks it on the vehicle's requirements; jobs keep their parts.
  - A requirement's ticked specs must be the vehicle's own, each once (`RequirementProblem::BadSpecs`), and editing a requirement can change them.
- **Copying:** Copy schedule and applying a template share `schedule::write_copied`: specs first, then requirements with their specs re-linked, then bundles. `ScheduleOutcome::Copied` now says how many specs.
- **Templates (`server::templates`):**
  - **List:** anyone signed in, with what each holds, the vehicles the person may apply one to (they may change its schedule, and it has none yet), and whether they may manage them.
  - **Save** from a vehicle: whoever may change its schedule; it needs at least one requirement (`RequirementProblem::NothingToSave`).
  - **Apply** to a vehicle with no requirements yet, as Copy schedule (`ScheduleNotEmpty` otherwise).
  - **Rename** and **remove:** the household's Admin, in Admin mode (`AdminAction::ManageServerSettings`). Vehicles keep their copies.
- **Endpoints:** `/api/v1/specs/add`, `/edit`, `/delete`; `GET /api/v1/templates`; `/api/v1/templates/save`, `/apply`, `/rename`, `/delete`. On the desktop, the Templates list is read from its copy (offline too; it has no caller until 4d). Changing specs and templates goes to the server, like other schedule changes.
- **Tests** (server, `tests/specs.rs`):
  - a Driver can't add a spec and a Maintainer can; a part number that's too long is refused; a requirement can't tick another vehicle's spec; Sam sees the specs; an edit keeps a spec on its vehicle; removing one unticks it; a Driver can't remove one;
  - Copy schedule brings the specs, re-linked;
  - a template: nothing to save at first; a Driver can't save one; its counts; the vehicles it can go on; applied as a copy, not twice; renamed and removed only by the Admin in Admin mode; the van keeps its copy.

**Step 4d notes (2026-11-17):**
- **Specs (1A, 2A; `ui::screens::specs_view`):** the Maintenance tab's views are now Schedule · Specs · Jobs · Set up. Each spec shows its name, "5.5 qt (5.2 L) · 5W-30 full synthetic · Part FL-500S", and its note. Whoever may change the schedule gets **+ Spec** and opens a spec to change or remove it. The dialog takes the name, a kind (none, volume, torque, pressure, or length), the amount and its unit with "= 5.2 L" shown as it's typed, the detail, the part number, and a note.
- **The requirement dialog:** **Specs its job uses**, a checklist of the vehicle's specs ("Engine oil · 5.5 qt", "Oil filter · FL-500S"), when it has any.
- **A job's task:** a task from a requirement (from the menu, a bundle, or **Log this service**) gets its ticked specs' parts pre-filled, each remembering its spec; with none, an empty row as before. Torque, pressure, and length specs show on the task as "Reminder: Drain plug torque 30 ft-lb (41 N·m)".
- **"Update the specs too?" (3A):** saving a job, when the person may change the schedule and a pre-filled part's number or a volume's quantity differs from its spec, opens a dialog listing each ("Oil filter: FL-500S → FL-910S"), ticked. **Save job** saves it, then the ticked specs (`SpecUpdate::applied`, new in `domain::spec`). The job is saved either way; a spec that can't be changed then (offline, say) is only logged.
- **Log this service (5A):** on each bundle card on the Schedule, for whoever may change it, while the vehicle takes new records. It opens `/vehicles/:id/service/:bundle`: a new maintenance job with a task for each of the bundle's requirements, parts pre-filled and resets ticked (`work_log::service_work`).
- **Templates (4B; `ui::screens::templates`):** a **Templates** screen under Manage (the wrench icon, so as not to share one with Groups or Reports). Each template shows what it holds ("9 requirements · 2 bundles · 6 specs") and **Apply to…** for the vehicles the person may give it to; applying says what was copied and any meter rules left out. The Admin, in Admin mode, gets **Rename** and **Remove**. Set up offers **Save as template…** once a vehicle has requirements, and **From a template…** (to the Templates screen) while it has none. On the desktop, the list is read from its copy; saving and applying need the server.
- **Tests:** a spec in one line, typed and checked; a template's counts and what applying one did; a task pre-filled from specs, its reminder, and a bare task's empty row; the new routes and the sidebar's 11 entries.
- **Tidying:** clippy's style findings (clones it didn't need, a shared arm for the wrench icon, a struct built in one go).
- **Not seen running yet:** all of these screens. They're in the phase's test plan (4e).

**Step 4e notes (2026-11-17):**
- **The test plan:** milestone 4's section of `docs/test-plans/p5-phase.md`, 18 steps, run at the end of the phase (#081). They cover:
  - the migration;
  - the Specs view and dialog (the conversion as it's typed, a unit changed and back, a bad amount);
  - ticking specs on a requirement; a task's pre-filled parts and reminder; "Update the specs too?" with one ticked and one not, and asked again on an edit;
  - a Driver's read-only Specs;
  - a bundle's **Log this service**, finished, each requirement's clock reset;
  - a template saved, applied as a copy (a later spec change not reaching the copy), renamed and removed by the Admin only;
  - a new vehicle's Copy schedule and From a template; the desktop online and offline.
- **Checked against the code while writing it:** a spec's summary is its own line under the name; a service job whose parts match their specs asks nothing.

**API version:** stays at 2. Specs and templates are Maintainer and Admin changes, which go to the server like other schedule changes, so the desktop gains no operations. A pre-filled part's spec is a new field inside a job, read as none by older records.

**New dependencies:** none expected (`uom` is already used).

**Open issues** (reviewed 2026-11-17): none touch specs or templates.

**Not in this milestone:** fuel economy (milestone 5), readiness (milestone 6), formulas and derived meters (milestone 7); factory schedules from reference data packs (post-MVP, #031).

### Milestone 5 plan: fuel economy

*Agreed 2026-11-17 · branch `milestone/p5-m5-fuel` · sketch: `docs/ui-sketches/p5m5-fuel-options.svg` · Decision #085*

**Choices (the human, 2026-11-17):**
- **Averages:** the last 5 stretches, the last 12 months, and all time.
- **Conditions:** three bands each (highway: 30% or less, mixed, 70% or more; towing and hauling: none, under 50%, 50% or more), shares weighted by fuel.
- **Towed vehicles:** a stretch counts for a vehicle if any fill in it names it.
- **The Overview:** the latest stretch, the last-5 average, and the last fill-up.
- **The sketch: 1A, 2A, 3A, 4A, 5A.**
  - **1A:** three tiles at the top of the Fuel tab, each with its stretch count.
  - **2A:** a line chart: a dot per stretch, a gap for each dropped one, the average as a hairline, only the latest labelled, details on hover or tap, and a table view. Drawn by the app, one series in the accent colour (its contrast checked: 7.1 on dark, 4.9 on light).
  - **3A:** the breakdowns as small bars from zero, a group each, with the average and the stretch count.
  - **4A:** each fuel-up in the list says how it counted ("24.9 mpg over 309 mi · car said 25.4", "counts in the next full fill-up", "not counted: an earlier fill-up wasn't logged"); **ⓘ** on **Filled full** and **Missed previous fill-up**.
  - **5A:** the Overview's Fuel panel.

**The rules, in `domain::fuel` (test-first):**
- **The fuel odometer:** the vehicle's first distance meter that fuel-ups require.
- **Fill-ups in order** of their time (counted events only: a corrected one by its correction).
- **A stretch** from a full fill to the next full fill: distance from their readings, fuel the sum of every fill after the first up to and including the last.
- **Dropped** (in no figure, a gap in the chart): a missed fill-up anywhere in it, a missing reading, or a distance of zero or less.
- **Each fill's part:** it starts the first stretch, ends one (with its figure), is a partial waiting for the next full fill, or is in a dropped stretch (and why).
- **Averages** as total distance over total fuel: the last 5 counted stretches, those ending in the last 12 months, and all.
- **Bands:** a stretch's shares are its fills' (after the first) weighted by fuel; then the bands above, and by towed vehicle.
- **The car's figure:** the reported economy on the fill that ends the stretch.

**Steps:**

| Step | Status | What |
|---|---|---|
| **5.0** `docs:` | ☑️ | This plan; the sketch; decision #085; milestone 4 marked ✅. |
| **5a** `feat(domain):` fuel economy | ☑️ | `domain::fuel`: the fuel odometer, stretches, each fill's part, averages, bands, towed vehicles, and the car's figure. Unit and property tests first (partials, a missed fill-up, a missing reading, a correction, the first full fill, an average never using a dropped stretch). |
| **5b** `feat(server):` | ☑️ | A vehicle's fuel summary (for the Fuel tab and the Overview), from its counted fuel-ups, for anyone whose role covers it. The desktop works it out from its copy, offline too. |
| **5c** `feat(app):` the screens | ☑️ | The tiles (row 1), the chart with its details and table view (row 2), the breakdowns (row 3), each fuel-up's line and the **ⓘ** popovers (row 4), and the Overview's Fuel panel (row 5). |
| **5d** `docs:` | ☑️ | Milestone 5's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**Step 5a notes (2026-11-17):**
- **`domain::fuel`:**
  - `fuel_meter`: the vehicle's first distance meter that fuel-ups require.
  - `Fill`: a fuel-up as economy needs it (`Fill::of` reads one from its event and the fuel odometer).
  - `summarize` sorts the fills by time and walks them: a full fill starts a stretch, the next full fill ends it, and partials between add their fuel. Each `Stretch` has its distance, fuel, shares weighted by fuel, towed vehicles, the car's figure from the fill it ends at, and why it's dropped, if it is (`DropReason`: a missed fill-up in it, a missing reading at either end, or no distance).
  - `FillPart` says what each fill did: before the first full fill, starts, a partial (in a stretch, or waiting for the next full fill), or ends a stretch.
  - `Averages` (the last 5 counted stretches, those ending in the last 365 days, and all) and `Breakdowns` (`Band::highway` and `Band::load`, three bands each; and each towed vehicle, with "not towing" first) are totals over totals (`Average`), in meters per liter, rounded.
- **Which fills:** the server passes the vehicle's counted fuel-ups (5b), so a correction stands in for what it corrects.
- **Tests:** the fuel odometer; partials adding in, and each fill's part; order by time, not by list; a missed fill-up, a missing reading, and no distance each dropping a stretch; the three averages' spans; no fills, or one; shares weighted by fuel into bands, and a towed vehicle; the bands' edges; the car's figure; a fill read from a fuel-up; and a property test that a dropped stretch never moves an average.

**Step 5b notes (2026-11-17):**
- **`server::fuel::summary`:** a vehicle's fuel economy (`FuelView`: the summary, the towed vehicles' names, and when the latest fill-up was), for anyone whose role covers it; **Not found** otherwise.
- **Which fuel-ups:** every one not deleted, **drafts included**. A fuel-up is a draft for its first day, and today's should count. A finalized correction stands in for what it corrects; a correction still in draft waits, and its original counts until then. (The plan said "counted events", which are finalized only; that would have left out every fuel-up from the last day.)
- **Towed vehicles** the person can't see aren't named; the screen will say "another vehicle".
- **Endpoint:** `POST /api/v1/fuel`. On the desktop, the data layer reads it from the copy, offline too; it has no caller until 5c.
- **Tests** (server, `tests/fuel.rs`): a truck's five fuel-ups, logged by a Driver (so drafts): a partial added in (20 mpg), a stretch towing the camper (12 mpg), a missed fill-up dropped, the all-time average as distance over fuel (15 mpg), the camper named, the latest fill-up's time, and a vehicle Sam can't see.

**Step 5c notes (2026-11-17):**
- **The Fuel tab (`ui::screens::fuel_view`)** starts with the figures, then the fuel-ups as before:
  - **1A:** three tiles (Last 5, 12 months, All time), each with its stretch count, "–" until there's one.
  - **2A:** the chart, an SVG drawn by the app: gridlines and axis labels, a shaded slot for each dropped stretch, the all-time average as a hairline ("average 24.0"), the line in runs broken at each gap (a lone point is a dot), a dot per counted stretch (its figure on hover, its details under the chart on tap), and only the latest labelled. **Show as a table** lists every stretch, newest first: when it ended, its fuel, and its figure or why it doesn't count.
  - **3A:** Highway, Towing, Hauling, and Towed vehicles, each band a bar from zero (scaled to the longest), with its figure and stretch count, or "not enough yet". A group with nothing in it isn't shown. A towed vehicle the person can't see is "Another vehicle".
  - With fewer than two full fill-ups: "Fuel economy shows once there are two full fill-ups."
- **4A:** each fuel-up in the Fuel tab's list adds how it counted ("20.0 mpg over 300.0 mi · car said 21.0", "Partial: added into the next full fill-up's figure", "Not counted: an earlier fill-up wasn't logged", …). The Timeline and Jobs lists don't fetch it.
- **The fuel-up form:** an **ⓘ** beside **Filled full** and **Missed previous fill-up** (outside the checkbox, so tapping it doesn't tick it) opens its explanation under the row; tapping it again closes it.
- **5A:** the Overview's Fuel panel shows "20.0 mpg latest · last 5: 20.0" above the latest fuel-ups, in place of "Fuel economy arrives with Phase 5's reports" (that text is gone). The panel's existing list of the latest fuel-ups already gives the last fill-up's date.
- **The chart's colour** is the app's accent, one series (no legend); its contrast was checked by hand (7.1 on dark, 4.9 on light), since the palette checker needs Node, which isn't installed here.
- **Tests:** each fill-up's line; the Overview's line; the line breaking at dropped stretches; the scale's room and floor.
- **Tidying:** clippy's style findings (two clones it didn't need, a `BandRow` type for the breakdown rows, and float comparisons in a test done with a tolerance). The time zone hook is used through its own module, since its re-export is desktop-only.
- **Not seen running yet:** all of these screens. They're in the phase's test plan (5d).

**Step 5d notes (2026-11-17):**
- **The test plan:** milestone 5's section of `docs/test-plans/p5-phase.md`, 16 steps, run at the end of the phase (#081), on a new car with round numbers so every figure can be checked: a full fill, a partial, a full fill with the car's own figure (32.0 mpg), a towing stretch (30.0), a missed fill-up (a gap), and a stretch after it (33.0); the tiles, the line broken at the gap, the breakdowns, a dot's details and the table, the Overview, L/100 km, an edited draft, the desktop offline, and a trailer with no Fuel tab.
- **Checked against the code while writing it:** the figures are shown to one decimal; the car's own figure is under the form's **More**.

**Nothing new stored:** the figures come from the fuel-ups already recorded, so there's no migration, and the API stays at 2.

**New dependencies:** none (the chart is drawn by the app, #080).

**Open issues** (reviewed 2026-11-17): #26 (a fuel-up's saved location) is about logging, not economy; not now.

**Not in this milestone:** readiness (milestone 6), formulas and derived meters (milestone 7), reports and exports (Phase 6).

### Milestone 6 plan: readiness

*Agreed 2026-11-18 · branch `milestone/p5-m6-readiness` · sketch: `docs/ui-sketches/p5m6-readiness-options.svg` · Decision #086*

**Choices (the human, 2026-11-18):**
- **Usages per vehicle**, made by Admins and Maintainers, in place of operation types in the profiles: groundwork for aviation's VFR, IFR, oceanic, and ETOPS; ground vehicles might have Local, Highway, Towing.
- **The default usage** is named for the kind: Drivable, Towable, or Usable.
- **Each usage stands alone**; overlap is ticking an item for several.
- **One "needed for" switch** (per usage) for requirements, not a separate "safety-related" flag.
- **Existing requirements** all count for the default usage.
- **Documents:** the default usage needs the profile's registration and insurance (not title); existing vehicles too. **A needed document not on file** makes the vehicle not ready.
- **Copying** keeps usages, matched by name.
- **The sketch: 1A, 2A, 3A, 4A.**
  - **1A:** the Overview's **Ready to use?**: usage chips (the default first), a headline ("✗ Not drivable: 2 things"), then every item with ✓, !, or ✗, its reason, and a link; "Advisory: from the records; it certifies nothing."
  - **2A:** a chip on each Garage card for the default usage: "✗ Not drivable", "! Drivable", "✓ Towable".
  - **3A:** a **Usages** section in Maintenance → Set up: each usage's name and the document types it needs; **+ Usage**.
  - **4A:** **Needed for** ticks in the requirement dialog; a new requirement starts with the default usage ticked.

**The rules, in `domain` (test-first):**
- **A usage:** a vehicle's, with a name (a label), whether it's the default, and the document types it needs. One default per vehicle; names unique per vehicle (ignoring case).
- **The profiles** name each kind's default usage and its needed document types.
- **A requirement's usages:** the vehicle's, each once; **none listed** means the default usage (every existing requirement).
- **Readiness for a usage** from the vehicle's due items (milestone 2), documents, and squawks:
  - a requirement ticked for it: Overdue ✗; Due soon or Due very soon !; otherwise ✓ (not on the schedule: not listed);
  - a document type it needs: the current one expired ✗, due soon !, current ✓; **none on file ✗**;
  - each open or deferred **High** issue ✗ (every usage);
  - the answer: ✗ if any ✗, else ! if any !, else ✓.
- **Copying:** a requirement's usages move to the new vehicle's usages of the same name (ignoring case); missing ones are created.
- **Dispositions:** a job's sign-offs and deferrals need `AssetAction::DispositionSquawk` on the vehicle.

**Steps:**

| Step | Status | What |
|---|---|---|
| **6.0** `docs:` | ☑️ | This plan; the sketch; decision #086; milestone 5 marked ✅. |
| **6a** `feat(domain):` readiness | ☑️ | `domain::usage` (the record, its checks, the default from the profile); `Requirement::usages`; the readiness calculation; copying usages by name; the disposition check. Unit tests first (each mark, a missing document, High blocking every usage, a deferred High, standing alone, the default for existing requirements). |
| **6b** `feat(storage):` migration 0026 | ☑️ | A `usages` table and `requirements.usages` (empty for every existing requirement, so the default). Each vehicle's default usage made at startup if it has none, and when a vehicle is added. Both reach the desktop's copy. The release fixtures migrate. |
| **6c** `feat(server):` | ☑️ | Usages (Admins and Maintainers change them; everyone who sees the vehicle sees them); readiness for each of a vehicle's usages (the Overview); the default usage's answer on each Garage card; Copy schedule and templates with usages; sign-offs and deferrals checked directly. |
| **6d** `feat(app):` the screens | ☑️ | **Ready to use?** on the Overview (row 1), the Garage chip (row 2), the Usages section (row 3), and **Needed for** (row 4). |
| **6e** `docs:` | ☑️ | Milestone 6's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**Step 6a notes (2026-11-18):**
- **The profiles** each gained `default_usage` (Drivable, Towable, or Usable) and `usage_documents` (registration and insurance on cars, trucks, motorcycles, and trailers; none on generators and other equipment); every profile's version went up by one.
- **`domain::usage`:** `Usage` (a vehicle, a name, whether it's the default, the document types it needs), `UsageId`, `UsagePatch`, and `check_usage` (a name of its own, ignoring case; each document type once; `UsageProblem`). `default_usage` makes a vehicle's from its kind's profile ("Usable" with none for a kind without one). `match_usages` maps a copied schedule's usages to the new vehicle's by name, the default to its default, and says which to create.
- **`Requirement::usages`** (and its patch field), with `check_usages` and `Requirement::needed_for`: ticking none means the default usage, so every existing requirement counts for it with no change to its data.
- **`domain::readiness`:** `readiness` for one usage from a vehicle's `VehicleRecords` (its requirements, what's due, its documents, its open and deferred High issues, and today). Each `ReadinessItem` has a `Mark` (✓, !, ✗) and an `ItemReason`: a requirement and where it stands, a needed document type's current document (the latest to expire among those not renewed; one that never expires counts as current) or **none on file**, or a High issue (deferred or not). Items run worst first; the answer is the worst mark.
- **Templates** carry the vehicle's usages too (`ScheduleTemplate::usages`, read as none by templates saved before).
- **Not here after all:** the disposition check. It's the server's (6c), where a job's permissions are already checked; there's nothing for `domain` to work out.
- **Tests:** a usage's checks; each kind's default (title left out; a kind without a profile); matching usages by name and the default by being the default; the default usage counting requirements that tick none and needing its documents (one missing, one due soon); each usage standing alone; a High issue blocking a usage that needs nothing, deferred too; a renewed document giving way to its renewal; each due step's mark; `needed_for`.
- **Tidying:** the fuel example in `domain::fuel`'s docs made its IDs with `test_support`, which only a full workspace build turns on; it now makes them with `Id::generate`.

**Step 6b notes (2026-11-18):**
- **`0026_usages.sql`:**
  - `usages`: each vehicle's usages (a name, whether it's the default, and the document types it needs as JSON);
  - `requirements.usages`: a JSON list, empty for every existing requirement, which means the default usage;
  - `schedule_templates.usages`.
- **Storage:** `storage::usages` (`UsageWriter`, `usage`, and `usages_of`, the default first, then by name). Once made, a usage's vehicle and whether it's the default don't change. The requirement and template writers store their usages.
- **The desktop's copy:** `usages` joins the synced record tables.
- **Moved to 6c:** making each vehicle's default usage (at startup for those without one, and when a vehicle is added). That's the server's, since its name and documents come from the vehicle's profile.
- **Tidying:** the release fixtures' checks for migrations 0024 to 0026 moved into `phase_five_starts_empty`, keeping the meters' check under clippy's complexity limit.
- **Tests:** a vehicle's usages (the default first), one renamed and needing nothing more while staying the default, one removed; a requirement's usages; a template's usages; the release fixtures migrating with no usages and no requirement or template listing any.

**Step 6c notes (2026-11-18):**
- **"Never chosen" and "none" are told apart:** `Requirement::usages` became an `Option`. Not set (every existing requirement) means the default usage; a list, even an empty one, is what was chosen. Without this, removing a requirement's only usage, or unticking every box, would have quietly made it count for the default. Migration 0026's `requirements.usages` became nullable (it hadn't merged, so it could still change), and the requirement's patch can set it back to "the default".
- **Default usages (`server::usages`):** `ensure_default_usages` runs at startup after the default group and makes one for every vehicle without (every vehicle from before usages); adding a vehicle makes its own (`make_default`). (Moved here from 6b, since the name and documents come from the profile.)
- **Usages:** `add_usage` (never a second default), `edit_usage` (name and documents; the vehicle and the default flag stay), and `delete_usage` (not the default, `UsageProblem::DefaultStays`; a requirement that chose it loses it, perhaps leaving it needed for none). Whoever may change the schedule; the schedule's answer lists them, the default first. A requirement's chosen usages are checked against the vehicle's (`RequirementProblem::BadUsages`), and editing can change them.
- **Readiness (`server::readiness`):** `view` gives every usage's readiness for the Overview (`POST /api/v1/readiness`); the Garage puts the default usage's name and answer on each card (`AssetSummary::ready`), reusing the due items it already works out. High issues come from each squawk's current severity and status.
- **Copying:** Copy schedule and applying a template make the usages the copied requirements chose on the new vehicle, matched by name (its default for the default) or made there, and re-link them. Templates save the vehicle's usages.
- **Dispositions:** adding, editing, or correcting a job with sign-offs or deferrals needs `AssetAction::DispositionSquawk` (a Maintainer), checked directly. Logging a job already needs a Maintainer, so this can't be shown failing yet; it guards against that ever changing.
- **Endpoints:** `/api/v1/usages/add`, `/edit`, `/delete`, and `POST /api/v1/readiness` (read from the copy on the desktop; no caller until 6d).
- **Tests** (server, `tests/readiness.rs`): a new truck's default usage (Drivable, two documents) and nothing more to make at startup; not drivable with neither document on file, and with an open High issue; the Garage's chip; a vehicle Sam can't see; a Driver can't add a usage; never a second default; a name taken; an unknown usage refused; the default not removable; removing Towing leaving the hitch needed for none; Copy schedule making Towing on the van and re-linking it.

**Step 6d notes (2026-11-18):**
- **`ui::screens::readiness_view`**, all four rows of the sketch:
  - **Ready to use?** (1A) replaces the Overview's placeholder: a chip per usage (the default first, shown when there's more than one), the chosen usage's headline ("✗ Not drivable: 2 things", "! Drivable: 1 thing due soon", "✓ Drivable"), then every item with ✓, !, or ✗, its name, why, and a link (Maintenance, Documents, or the issue), and that it's advice only.
  - **The Garage chip** (2A): the default usage's answer in words beside the due chip; none on an archived vehicle.
  - **Usages** (3A) in Maintenance → Set up, after the bundles: each usage with the documents it needs, the default marked; **+ Usage** and **Edit** open a dialog for the name and the document types (the kind's profile's, and any already chosen). **Remove** is in the dialog, never for the default.
  - **Needed for** (4A) in the requirement dialog: a requirement never chosen shows the default ticked; the first change makes its own list, which can be empty.
- **Colour is never alone:** each mark has its symbol and words; the colours are the existing OK, warning, and error ones.
- **Gone:** the Overview's "arrives with what's due" note and its placeholder panel.
- **Tests:** a usage's answer in words; the headline counting only what sets the answer.

**API version:** stays at 2. Usages are schedule changes, which go to the server; the desktop gains no operations.

**New dependencies:** none expected.

**Open issues** (reviewed 2026-11-18): #27 (custom roles and permissions) would one day change who may sign off; not now.

**Not in this milestone:** a person's currency (post-MVP, #051), aviation's usages and items (the aircraft phase), formulas and derived meters (milestone 7).

### Milestone 7 plan: calculated fields and derived meters

*Agreed 2026-11-18 · branch `milestone/p5-m7-formulas` · sketch: `docs/ui-sketches/p5m7-formulas-options.svg` · Decision #087*

**Choices (the human, 2026-11-18):**
- **Our own small formula language:** numbers (with units), `+ − × ÷`, parentheses, values by their stable key, and functions. **Meters get stable keys** too, like fields (#057).
- **Functions:** `mean`, `median`, `geometric_mean`, `harmonic_mean`, `min`, `max`, `sum`, `count`, `stddev`, `round`, and `abs`. They work over **values listed in the formula** and over **past events**, with an **Over:** cumulative, or the last X events, days, weeks, months, or distance or hours.
- **Units checked by dimension:** `+` and `−` need the same dimension; `×` and `÷` combine them; the result must match what the field shows. A formula that doesn't is refused when saved, with the reason.
- **A calculated field** on an event uses that event's values (its fields, its readings, a fuel-up's volume and cost) and **since the last event of its kind** (`distance-since-last(odometer)`, `time-since-last`).
- **A derived meter** gets a value **at every reading of one of its inputs**, from each input's latest reading. It's never stored, and nobody types a reading of it.
- **The sketch: 1 (A and B combined), 2A, 3B, 4A, 5A**, and a stylized **ƒ badge** wherever a value is worked out.
  - **1:** a typed formula box with **Functions ▾** and **Values ▾** menus that insert at the cursor; an **Over** picker for past-events functions; checked as you type ("✓ Money ÷ Distance = money per distance"); a preview from the latest event.
  - **2A:** calculated values as read-only ƒ rows under the fields on the log form (changing as values are typed) and on the event's page.
  - **3B:** **+ Derived meter** beside **+ Meter** in Setup, with its own dialog (name, unit, formula, its check, and its value now).
  - **4A:** a derived meter beside the typed ones, on the Overview and in Setup, ƒ-marked, "worked out from …"; never a reading field on a form.
  - **5A:** **ⓘ How to write one** beside every formula box: a short popup (the rules in four lines, three examples) and **Full guide →**, which opens `docs/help/formulas.md` (on GitHub, for now). An in-app guide comes later (idea 22).

**The rules, in `domain` (test-first):**
- **Writing:** a formula is text, kept as typed, and parsed whenever it's checked or used. Values are keys (`cost`, `trip-time`, `odometer`); numbers can carry a unit (`33 mi/h`, `1000 mi`, `5 gal`, `90 days`); `×` and `*`, `÷` and `/` are the same. A function takes values separated by commas, or one expression and an **Over** (`mean(trip-time, last 5)`, `sum(volume, last 3 months)`, `mean(cost ÷ volume, last 1000 mi)`, `max(trip-time, cumulative)`).
- **Dimensions:** each value has powers of distance, volume, time, and money, or none (a plain number or a percentage). A field's kind gives its dimension; a fuel-up's cost is money in its currency (mixing currencies is refused). `round(x, places)` keeps x's dimension; `count` is a plain number; `geometric_mean` and the rest keep their values' dimension.
- **What a calculated field shows:** a number (with its decimals), a percentage, a distance, a duration, a volume, fuel economy, money, money per distance, money per volume, or distance per time, each in the person's units.
- **Over:** the past events of the same kind on the same vehicle, up to and including this one: the last N; those in the last N days, weeks, or months before it; those within the last N mi or km on the vehicle's main odometer, or N h on a named hour meter (`last 50 h on engine-hours`); or **cumulative** (all of them). The expression is worked out on each, and events where it has no value are left out.
- **No value** ("—", with why) when something it needs is missing: a field left empty, no earlier event for "since the last", nothing in the Over, or a division by zero.
- **Keys:** every meter gets a key from its name, unique on the vehicle and fixed once made (existing meters too). A field or meter that a formula uses can't be removed or turned off (it says which formula uses it).
- **A derived meter:** its formula uses only the vehicle's typed meters (not resettable ones, and not other derived meters). It has the formula's dimension (distance, duration, or percentage) and a direction, and it can't be reset. Its values come at each moment one of its inputs was read, once every input has a reading. It works in interval rules, projections, and "last done" like any other meter, with its value at the job worked out the same way.

**Steps:**

| Step | Status | What |
|---|---|---|
| **7.0** `docs:` | ☑️ | This plan; the sketch; decision #087; idea 22 (an in-app guide); milestone 6 marked ✅. |
| **7a** `feat(domain):` the formula language | ☑️ | `domain::formula`: the parser (keys, numbers with units, operators, parentheses, functions, Over), the dimension check against what the result shows, working it out (values from a lookup, past events from a list), the functions, and every refusal and "no value" in words. Unit tests first, and property tests (the dimension rules; a formula printed and read back is the same). |
| **7b** `feat(domain):` fields and meters | ☑️ | `FieldKind::Calculated` (formula, what it shows); `Meter::key` and `Meter::formula`; keys from names; the values an event offers (its fields, readings, volume, cost, and since the last); a derived meter's readings for what's due; the "used by a formula" checks. Unit tests first. |
| **7c** `feat(storage):` migration 0027 | ☑️ | `meters.key` (made from each existing meter's name, unique on its vehicle) and `meters.formula`. Calculated fields need no change (a field's kind is JSON). The desktop's copy runs the same migration; the release fixtures migrate. |
| **7d** `feat(server):` | ☑️ | Saving calculated fields and derived meters (checked; Maintainers, as now); calculated values on each event the person sees (the event's page, the timeline); derived meters' values on the vehicle and in what's due (Maintenance, the Overview, the Garage, messages). The desktop does the same from its copy, offline too. |
| **7e** `feat(app):` the screens | ☑️ | The calculated field dialog (row 1), ƒ rows on the log form and the event's page (row 2), **+ Derived meter** and its dialog (row 3), derived meters on the Overview and in Setup (row 4), the help popup (row 5), the ƒ badge; `docs/help/formulas.md`. |
| **7f** `docs:` | ☑️ | Milestone 7's section of the phase's test plan (`docs/test-plans/p5-phase.md`). |

**Step 7a notes (2026-11-18):**
- **`domain::formula`:** `Formula::parse` keeps the text as typed (stored as that text) and reads it; `check` (with what each key is, `KeyInfo`) and `check_shows` (against `ShowsAs`) refuse what doesn't fit; `evaluate` works it out on an event (`Scope`, within a `Context`: earlier events of its kind, the main odometer's key, the zone, the person's units); `keys` lists what it uses (for "used by a formula"), and `uses_history` says if it needs earlier events.
- **Reading:** a key may contain `-`, so a minus between keys needs spaces (or `−`). `/` inside a unit (`33 mi/h`) is the unit's, while `÷` is always a division, so `5 mi ÷ h` divides by a value keyed `h`. A number's unit is any of mi, km, m, h, min, s, days, weeks, gal, imp-gal, L, mL, qt, or `%`, alone or one over another.
- **Dimensions:** powers of distance, volume, time, and money (`Dimension`); a percentage is a plain number that remembers it's a percentage (80% is 0.8, shown as 80 %) until it's multiplied or divided. Money keeps its currency, and mixing two gives no value.
- **`round`** rounds as the value is shown in the person's units (miles, gallons, hours, mpg or L/100 km, money per mile), so `round(cost ÷ distance-since-last(odometer), 2)` is to the cent per mile. `in_units` gives any value as shown.
- **Over** picks the past events of the kind up to and including this one; months are calendar months in the household's zone; distance spans count back on the main odometer, or `on` a named meter; an hour span must name its meter.
- **`stddev`** is the sample standard deviation (at least two values); `geometric_mean` and `harmonic_mean` need every value above zero.
- **Refusals and "no value"** each have a message: e.g. "trip-time + volume: can't add a time to a volume." and "No value: there's no earlier one to count from."
- **Tests:** precedence, keys with hyphens, units, every refusal (with where), each Over, the keys used, the stored text; dimensions combining and refused; meters needed; nested Overs; the result against what's shown; refusals in words; cost per mile, rounded; each "no value"; every function; rounding in the person's units (L/100 km inverted); Over by events, days, weeks, months, miles, and kilometers, leaving out events without a value. Properties: dividing undoes multiplying, and a formula printed reads back the same.

**Step 7b notes (2026-11-18):**
- **`FieldKind::Calculated`** (a formula, and what it shows as): never required, never typed (a value given for one is refused), and its decimals checked like a number's. Formulas compare by their text, so fields and meters keep their equality.
- **`Meter::key` and `Meter::formula`** (and their patch fields; both set when made). The profiles' meters gained keys (`odometer`, `run-hours`), each profile's version up by one. A reading of a derived meter is refused as for a meter that isn't the vehicle's.
- **`domain::calculated`:**
  - **Keys:** `new_key` makes one from a name, different from every field's and meter's key on the vehicle and from every word formulas use (`is_reserved`: functions, units, Over's words, and a fuel-up's values).
  - **Checks:** `check_calculated` checks a calculated field's formula on each kind of event it's shown on: the event's numeric fields (not other calculated ones, `FormulaError::Calculated`), its meters (derived ones too), and a fuel-up's `volume`, `cost`, `highway`, `towing`, `hauling`, and `reported-economy`. `check_derived` refuses a derived meter that's resettable or required (`MeterError::DerivedResettable`), uses anything but typed meters that aren't reset (`FormulaError::NotAnInput`), uses `round`, past events, or "since the last" (`FormulaError::NotInDerived`), or doesn't measure what the meter does.
  - **Values:** `scopes` gives each of a vehicle's events of one kind its values (fields, readings, derived meters at that moment, a fuel-up's) and since the one before; `calculate` works a calculated field out on one of them; `main_odometer` is the typed distance meter fuel-ups require.
  - **Derived readings:** `derived_readings` gives a derived meter a reading at every moment one of its inputs was read, once each has one; `with_derived` adds them to the readings what's due counts from; `add_derived_to_done` adds each derived meter's value when a job was done (its own readings first, else the latest before it).
  - **In use:** `used_by` names the calculated field or derived meter whose formula uses a key.
- **Until 7c:** storage makes each meter's key from its name (the column comes with migration 0027), and the server gives a new meter a key from `new_key`. The log form leaves out derived meters and calculated fields (nothing to type); the field kind reads "Calculated" in Setup. The screens for them come in 7e.
- **Tests:** keys never taken or reserved; a calculated field checked on each kind it's shown on, refused for an unknown value, the wrong result, or another calculated field; every derived-meter refusal; what's in use; derived readings at every input's reading (none until each input has one, none at another meter's); a job counting a derived meter from its own readings; each event's values and since the last; cost per mile and an average worked out, and none on the first; a calculated field never required, its decimals checked.

**Step 7c notes (2026-11-19):**
- **`0027_meter_keys.sql`:** `meters.key` (unique on its vehicle among meters that aren't deleted, so a removed meter's key can be used again) and `meters.formula` (a derived meter's, as typed). The meter writer stores both and an update can change the formula; reading parses the formula.
- **Changed from the plan: existing meters' keys come from the server, not the migration.** A key is made from the meter's name, different from the vehicle's other meters' and fields' keys and from the words formulas use; SQL can't make that from any name reliably. So the column starts empty, and the server gives each older meter its key at startup (step 7d), saved like any change so desktops get it by syncing, as milestone 6's default usages were. Until then, a meter without a stored key reads with one made from its name.
- **Calculated fields need no change:** a field's kind is already JSON.
- **The desktop's copy** runs the same migration; meters already sync.
- **Tests:** a derived meter's key and formula round-trip, a changed formula keeps its key, a key taken on the vehicle is refused until that meter is deleted, and a meter without a stored key reads with one from its name. The release fixtures migrate with no stored keys or formulas, each meter typed and keyed from its name.

**Step 7d notes (2026-11-19):**
- **Older meters' keys (`server::formulas::ensure_meter_keys`):** at startup, after the default usages, every meter without a stored key gets one from its name (`storage`'s `unkeyed_meters`), saved like any change so desktops get it by syncing.
- **Saving (Maintainers, as before):**
  - A meter form can carry a formula (`MeterForm::formula`): a derived meter, checked when added and when changed (`check_derived`). A typed meter stays typed.
  - A calculated field is checked when added and when its formula, what it shows, its kinds, or on/off change (`check_calculated`). Its formula and what it shows can change, since it holds no values; other kinds stay fixed.
  - Refusals come back as `AssetsOutcome::InvalidFormula`, with the reason in words.
- **Kept while in use:** a meter or field a formula uses can't be removed, turned off, or taken off a kind it's shown on. A typed meter a derived one uses can't be made resettable. Each comes back as `AssetsOutcome::InUse`, naming the formula's field or meter ("Service miles uses this in its formula …").
- **Keys:** a new meter's and field's keys come from `domain::calculated`, so they never match each other's or a word formulas use; a field's never matches a removed field's either (`new_key_among`).
- **Values on events:** the events list and one event's page carry each calculated field's value (`EventView::calculated`), rounded in the person's units, or why there's none. Events a correction replaces don't count; months count in the event's own zone. The log form's context carries each kind's earlier values (`EventsOutcome::LogContext::history`), so the form can work them out as values are typed (7e).
- **What's due** counts derived meters' readings (Maintenance, the Overview, the Garage, Due & squawks, and messages all read it there), a job's derived value included, and so does a snooze by meter.
- **The desktop** does all of this from its copy, through the same server code.
- **Tests** (server, `tests/formulas.rs`): a derived meter refused for `round`, an unknown key, or being required; what it uses kept from removal and from being made resettable; what's due coming up on Service miles from two readings; a typed reading of a derived meter refused; a calculated field refused for adding money to a volume; the odometer kept while cost per mile uses it; $0.15 a mile on the second fuel-up, none on the first, the same on the event's page; the log form's history; older meters given their keys once, and nothing more to do after.

**Step 7e notes (2026-11-19):**
- **`ui::screens::formula_view`**, every row of the sketch:
  - **The ƒ badge** (a blue italic serif ƒ in a rounded square) on every worked-out value: calculated rows, derived meters in Setup and on the Overview.
  - **The formula box** (row 1): typed, with **Functions ▾** (the statistics functions, `round`, `abs`) and **Values ▾** (this event's numeric fields and a fuel-up's values, meters, since the last one, numbers with units). Both insert at the cursor: a short script puts the text in place of the selection, with the text quoted as JSON so nothing in it can break out. The **Over** picker inserts `, cumulative`, `, last 5`, `, last 3 months`, or `, last 1000 mi`, and the numbers can then be changed. The check updates as you type ("✓ Gives money per distance", or the reason in words).
  - **ⓘ How to write one** (row 5): a short popup (values, operators and units, Over, spaces around a minus; three examples) and **Full guide →** to `docs/help/formulas.md` on GitHub (`domain::formula::GUIDE_URL`).
  - **+ Calculated field** (Setup → Custom fields): label, **Shows as** (and decimals for a number or percentage), the formula box, the kinds it's shown on, and a preview worked out on the latest event of the first kind ("On the latest fuel-up: $0.14 per mi").
  - **+ Derived meter** (row 3B, Setup → Meters, its own dialog): name, what it measures (fixed once made), unit, precision, the formula box (typed meters that aren't reset, numbers, no past-events functions), and its value now from each input's latest reading.
  - **ƒ rows** (row 2A): on the event's page, and on the log form as values are typed, worked out after the vehicle's earlier events of the kind; until the form is complete enough to build the event, "Shows once the form is filled in."
  - **Derived meters on the Overview** (row 4A): the value worked out from its inputs' readings, and "Worked out from Odometer, Engine hours".
- **Setup rows:** calculated fields and derived meters show their formula; each meter's key shows, as fields' do. Edit opens the right dialog.
- **The log form's history** covers every kind of event the vehicle has (not only kinds with a calculated field yet), so a new calculated field's preview works before it's saved.
- **`domain`:** `Logged` (an event, or one being typed) and `scope_of` give one event's values after the one before it; `check_calculated_formula` and `check_derived_formula` check a formula alone; `derived_now` gives a derived meter's value now.
- **`docs/help/formulas.md`:** the full guide: writing, units, values, every function and Over, derived meters, no value, and in use.
- **Tests:** values shown in each unit (money per mile, a time, a percentage, mpg and L/100 km, mph), the check line, no-value words, the insert script quoting what it inserts, and each "shows as" with its own option.

**API version:** stays at 2. Migration 0027 changes the schema, so an older desktop is told to update before it syncs (#075, #076).

**New dependencies:** none expected (units through `uom`, as with specs).

**Open issues** (reviewed 2026-11-18): none touch formulas or meters; #27 (custom roles) would one day change who may write formulas.

**Not in this milestone:** an in-app help guide (idea 22); formulas across vehicles, and in reports (Phase 6); a catalogue of ready-made fields (idea 16); copying derived meters and calculated fields with a schedule or template.
