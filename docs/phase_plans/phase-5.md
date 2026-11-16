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
| 1 | **Meters** | ⬜ | **Percentage meters** (any meter; 0–100%, stored in hundredths of a percent). **Direction:** counts up or counts down. **Resettable** (either direction), with its reset value (e.g. 100% or 0); never a distance meter used for fuel economy. Readings checked per direction, with a **reset** explaining a jump; resets on the timeline. The meter form and reading inputs. A migration, and new operations for the desktop's outbox | Add **Oil life** to the truck, log 80%, 60%, see 70% refused, reset it to 100% |
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
