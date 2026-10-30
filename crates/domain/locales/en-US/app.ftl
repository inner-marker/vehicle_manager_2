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
connection-run-checks = Run checks
connection-running = Checking…
connection-version-check = API version
connection-auth-check = Signed in
connection-pass = Pass
connection-fail = Fail
connection-not-checked = Not checked
# $app_version (text), $api_version and $min_api_version (numbers): what the server reported.
connection-server-info = Server app { $app_version }, API { $api_version } (accepts { $min_api_version }–{ $api_version })
# $millis (number): ping round-trip time.
connection-round-trip = Round trip: { $millis } ms
connection-unreachable = Couldn't reach the server.
connection-unexpected = The server sent an unexpected response.
connection-health-check = Server health
connection-server-unhealthy = The server has a problem with its database and isn't accepting changes. Check the server's log.
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

## Usernames, passwords, and names

username-wrong-length = A username needs 3 to 32 characters.
username-invalid-character = A username can only have letters, numbers, dots, hyphens, and underscores.
username-must-start-with-letter-or-digit = A username must start with a letter or a number.
password-too-short = A password needs at least 12 characters. A short sentence works well.
password-too-long = A password can have at most 256 characters.
display-name-empty = Enter a name.
display-name-too-long = A name can have at most 64 characters.

## Why the server refused a request

api-error-missing-credential = You're not signed in.
api-error-invalid-credential = Your session has ended. Sign in again.
api-error-auth-not-configured = The server can't check sign-ins right now. Check the server's log.
api-error-missing-api-version = The app didn't send its API version.
api-error-malformed-api-version = The server couldn't read the app's API version.
api-error-client-too-old = This app is too old for the server. Update the app.
api-error-client-too-new = This app is newer than the server. Update the server first.

## Signing in, first-Admin setup, modes, and the account menu

loading = Loading…
something-went-wrong = Something went wrong. Try again.
action-cancel = Cancel
sign-in-title = Sign in
sign-in-subtitle = to see your household's vehicles
field-username = Username
field-password = Password
sign-in-button = Sign in
sign-in-busy = Signing in…
sign-in-forgot = Forgot it? Ask your household's Admin.
sign-in-wrong = Wrong username or password.
# $minutes (number): how long to wait.
too-many-attempts =
    { $minutes ->
        [one] Too many attempts. Try again in { $minutes } minute.
       *[other] Too many attempts. Try again in { $minutes } minutes.
    }
sign-in-unavailable = The server can't sign you in right now. Try again in a moment.
# $step (number): 1 or 2.
setup-step = STEP { $step } OF 2
setup-title = Set up this server
setup-subtitle = No one has an account yet.
setup-code-help = Enter the setup code from the server's log (see the guide).
field-setup-code = Setup code
setup-next = Next
setup-checking = Checking…
setup-wrong-code = That code isn't right.
setup-already = This server already has an Admin. Sign in instead.
setup-account-title = Your account
setup-code-accepted = ✓ Code accepted. You'll be the Admin.
field-display-name = Your name
field-new-password = Password (12+ characters)
field-password-again = Password again
passwords-differ = The two passwords aren't the same.
setup-create = Create Admin and sign in
setup-creating = Creating…
action-back = ← Back
mode-driver = Driver
mode-maintainer = Maintainer
mode-admin = Admin
mode-read-only = Read-only
mode-needs-password = password
mode-menu-label = Change mode
admin-dialog-title = Enter Admin mode
admin-dialog-text = Type your password to confirm. Admin mode ends after 15 minutes without activity.
admin-dialog-confirm = Enter Admin mode
wrong-password = That password isn't right.
admin-banner-label = ADMIN MODE
admin-banner-text = Changes affect the whole household · ends after 15 idle minutes
admin-banner-leave = Leave
account-menu-label = Account
account-change-password = Change password
account-sign-out = Sign out
password-dialog-title = Change password
field-current-password = Current password
password-save = Change password
password-changed = Password changed. You were signed out on your other devices.
password-must-change-title = Choose a new password
password-must-change-text = Your Admin gave you a temporary password. Choose your own to continue.
password-current-wrong = Your current password isn't right.

## Users (Admin mode)

nav-users = Users
role-viewer = Viewer
role-driver = Driver
role-maintainer = Maintainer
role-admin = Admin
role-none = No role
users-add = + Add user
users-you = You
users-disabled = Disabled
users-temporary = Temporary password
# $role (text): the role's name.
users-role-summary = { $role } · all vehicles
users-roles-heading = ROLES
users-all-vehicles = All vehicles
users-one-vehicle-later = One vehicle: available once vehicles exist (milestone 4).
users-account-heading = ACCOUNT
users-reset-password = Reset password…
users-disable = Disable…
users-enable = Enable
users-last-admin = The last Admin can't be disabled or lose the Admin role. Make someone else an Admin first.
users-disable-note = Disabling signs them out everywhere; it can be undone.
# $name (text): the person's display name.
users-confirm-disable-title = Disable { $name }?
users-confirm-disable-text = They'll be signed out everywhere and can't sign in until you enable them again.
users-confirm-disable = Disable
# $name (text): the person's display name.
users-confirm-reset-title = Reset { $name }'s password?
users-confirm-reset-text = They'll be signed out everywhere and get a temporary password to change at their next sign-in.
users-confirm-reset = Reset password
users-add-title = Add a user
field-their-name = Their name
users-role-on-all = Role on all vehicles
users-add-note = A temporary password is made for them; they choose their own at their first sign-in.
users-add-button = Add user
# $name (text): the person's display name.
users-temp-title = { $name }'s temporary password
users-temp-text = Give it to them, typed exactly as shown. It's shown only this once.
users-temp-copy = Copy
users-temp-copied = Copied
users-temp-done = Done
users-temp-note = They must choose a new password at their next sign-in.
users-username-taken = Someone already has that username.
users-not-allowed = Switch to Admin mode to manage users.
users-select-hint = Choose someone to see their roles and account.
users-back = ← Users
