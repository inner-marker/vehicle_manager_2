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

## Screens

screen-vehicle-detail = Vehicle
# $id (text): the vehicle's internal ID, until vehicles have names.
vehicle-detail-id = ID: { $id }
screen-log-event = Log
screen-not-found = Page not found
not-found-back = Back to the garage
placeholder-coming-soon = Coming soon.

## Actions and top bar

action-log = Log
action-refresh = Refresh
menu-open = Open menu
menu-close = Close menu
sync-status-unknown = Sync not set up

## Theme

theme-label = Theme
theme-light = Light
theme-dark = Dark
theme-system = System

## Log viewer (Diagnostics)

log-viewer-min-level = Minimum level
log-viewer-area = Area
log-viewer-all-areas = All areas
log-viewer-time = Time (UTC)
log-viewer-level = Level
log-viewer-message = Message
# $count (number): entries currently shown.
log-viewer-entry-count =
    { $count ->
        [one] { $count } entry
       *[other] { $count } entries
    }
log-viewer-empty = No log entries match these filters.
log-viewer-unavailable = The debug log isn't running, so there's nothing to show.

## Log areas and levels (shown in the log viewer)

log-area-app = App
log-area-ui = Interface
log-area-sync = Sync
log-area-database = Database
log-area-auth = Sign-in & permissions

log-level-error = Error
log-level-warn = Warning
log-level-info = Info
log-level-debug = Debug
log-level-trace = Trace
