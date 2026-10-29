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

## Connection test (Diagnostics)

connection-title = Connection
connection-credential = Test credential
connection-run-checks = Run checks
connection-running = Checking…
connection-version-check = API version
connection-auth-check = Credential
connection-pass = Pass
connection-fail = Fail
connection-not-checked = Not checked
# $app_version (text), $api_version and $min_api_version (numbers): what the server reported.
connection-server-info = Server app { $app_version }, API { $api_version } (accepts { $min_api_version }–{ $api_version })
# $millis (number): ping round-trip time.
connection-round-trip = Round trip: { $millis } ms
connection-unreachable = Couldn't reach the server.
connection-unexpected = The server sent an unexpected response.
connection-server-address = Server address
connection-save-address = Save address
# $address (text): the server address this run of the app uses.
connection-address-in-use = Using { $address }
connection-address-saved = Saved. Restart the app to use the new address.
connection-address-save-failed = Couldn't save the address. See the log below for details.

## Server address rules (desktop)

server-address-empty = Enter the server's address.
server-address-unsupported-scheme = The address must start with https:// (or http:// for this computer).
server-address-insecure-remote = Use https:// for a server on another device. http:// is only for this computer.
server-address-missing-host = Add the server's name after https://.
server-address-invalid-host = The server name can only have letters, numbers, dots, and hyphens.
server-address-invalid-port = The port must be a number from 1 to 65535.
server-address-extra-parts = Enter only the server's address, without a path, query, or login.

## Why the server refused a request

api-error-missing-credential = No credential was sent.
api-error-invalid-credential = The server didn't accept this credential.
api-error-auth-not-configured = The server has no credential configured, so it refuses every request.
api-error-missing-api-version = The app didn't send its API version.
api-error-malformed-api-version = The server couldn't read the app's API version.
api-error-client-too-old = This app is too old for the server. Update the app.
api-error-client-too-new = This app is newer than the server. Update the server first.
