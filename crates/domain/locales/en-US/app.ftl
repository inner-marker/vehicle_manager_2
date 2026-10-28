# English (en-US) messages for every user-facing string.
#
# Every message here needs a matching line in `define_messages!`
# (crates/domain/src/i18n/message_id.rs); a test checks both directions.
# IDs are kebab-case and grouped by prefix (nav-, error-, ...).
# The UI says "vehicle" for an asset (AGENTS.md, Terminology).

## Navigation (plan §5 UI direction)

nav-garage = Garage
nav-due-and-squawks = Due & squawks
nav-messages = Messages
nav-timeline = Timeline
nav-reports = Reports
nav-vehicles = Vehicles
nav-settings = Settings
nav-diagnostics = Diagnostics

## Counts

# $count (number): open squawks across the shown vehicles.
open-squawk-count =
    { $count ->
        [one] { $count } open squawk
       *[other] { $count } open squawks
    }
