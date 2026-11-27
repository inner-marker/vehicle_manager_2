# English (en-US) messages for every user-facing string.
#
# Every message here needs a matching line in `define_messages!`
# (crates/domain/src/i18n/message_id.rs); a test checks both directions.
# IDs are kebab-case and grouped by prefix (nav-, error-, ...).
# The UI says "vehicle" for an asset (AGENTS.md, Terminology).

## Navigation (plan §5 UI direction)

nav-garage = Garage
nav-due-and-squawks = Due & { $squawks }
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
        [one] { $count } open { $squawk }
       *[other] { $count } open { $squawks }
    }

## Screens

screen-vehicle-detail = Vehicle
screen-log-event = Log
screen-not-found = Page not found
not-found-back = Back to the garage
placeholder-coming-soon = Coming soon.

## Actions and top bar

action-log = Log
action-refresh = Refresh
menu-open = Open menu
menu-close = Close menu

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
api-error-device-signature = This device's sign-in couldn't be confirmed. If the computer's clock is far off, fix it; otherwise sign in again.

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
# $minutes (number): Admin mode's idle limit (15, or less with test timers).
admin-dialog-text =
    { $minutes ->
        [one] Type your password to confirm. Admin mode ends after { $minutes } minute without activity.
       *[other] Type your password to confirm. Admin mode ends after { $minutes } minutes without activity.
    }
admin-dialog-confirm = Enter Admin mode
wrong-password = That password isn't right.
admin-banner-label = ADMIN MODE
# $minutes (number): Admin mode's idle limit.
admin-banner-text =
    { $minutes ->
        [one] Changes affect the whole household · ends after { $minutes } idle minute
       *[other] Changes affect the whole household · ends after { $minutes } idle minutes
    }
admin-banner-leave = Leave
# $minutes (number): Admin mode's idle limit. The banner's text on phones.
admin-banner-short = ends after { $minutes } idle min
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

## Password recovery (decision #047)

sign-in-forgot-link = Forgot your password?
sign-in-server = SERVER
sign-in-another-server = + Another server…
sign-in-server-address = Server address
sign-in-use-server = Use this server
sign-in-reopen-to-switch = Saved. Quit and reopen the app to switch to { $address }.
server-last-used-today = Last used today
server-last-used-yesterday = Last used yesterday
server-last-used-days = Last used { $days } days ago
recovery-title = Forgot your password?
recovery-subtitle = Enter your username.
recovery-continue = Continue
recovery-ask-admin-title = Ask your Admin
recovery-ask-admin-text = for further instructions.
recovery-have-code = HAVE A RECOVERY CODE?
field-recovery-code = Recovery code
field-new-password-long = New password (12+ characters)
field-new-password-again = New password again
recovery-submit = Set password and sign in
recovery-back = ← Back to sign in
recovery-wrong-code = That code isn't right, or it has expired.

## Settings (decisions #029, #048)

settings-tab-you = You
settings-tab-household = Household
settings-tab-devices = Devices
devices-help = Where you're signed in. Signing out a desktop app means signing in on it again (it pairs again).
devices-this-device = This device
devices-browser = Browser
devices-desktop = Desktop app
devices-desktop-older = Desktop app (older version)
devices-kind-desktop = Desktop app
devices-kind-browser = Browser
devices-kind-desktop-older = Desktop app, unsigned
devices-seen-now = Active now
devices-seen-minutes = { $minutes } min ago
devices-seen-hours = { $hours ->
    [one] 1 hour ago
   *[other] { $hours } hours ago
}
devices-seen-yesterday = Yesterday
devices-seen-days = { $days } days ago
devices-sign-out = Sign out
devices-sign-out-others = Sign out everywhere else
devices-sign-out-others-confirm = Sign out every other device and browser? Desktop apps will need to sign in again.
devices-rename = Rename
devices-name = Name
devices-only-this = You're signed in only here.
devices-unavailable = The devices couldn't be loaded or changed. Try again.
devices-name-empty = Type a name.
devices-name-too-long = Use 64 characters or fewer.
devices-name-control = Keep the name on one line.
users-devices = Devices ({ $count })
users-devices-title = { $name }'s devices
users-devices-sign-out-all = Sign out all of { $name }'s devices
users-devices-none = Not signed in anywhere.
settings-you-help = Follow the household default, or choose your own. Changes save at once.
settings-household-help = For everyone who hasn't chosen their own.
settings-household-admin-only = Switch to Admin mode to change the household defaults.
settings-use-household = Use household default
settings-this-device = This device
settings-theme = Theme
settings-keep-on-top = Keep window on top
settings-keep-on-top-note = Above other windows, on this computer
settings-section-units = Units and formats
settings-section-vehicles = Vehicles and messages
settings-section-photos = Photos
settings-section-appearance = Appearance
settings-section-lock = Lock and PIN
settings-section-household = Defaults for everyone
settings-back = ← Settings
confirm-type = To confirm, type:
pin-not-digits = Use digits only.
pin-too-short = Use at least 6 digits.
pin-too-long = Use 12 digits or fewer.
pin-repeated = Don't repeat one digit.
pin-run = Don't use a run like 123456.
lock-after-5 = 5 minutes
lock-after-15 = 15 minutes
lock-after-30 = 30 minutes
lock-after-60 = 1 hour
lock-after-launch = Only when the app opens
pin-mismatch = The two PINs didn't match. Choose one again.
pin-choose-title = Choose a PIN
pin-choose-help = 6 digits or more. This computer asks for it when the app opens and after it's been idle.
pin-choose-again = Type it again.
pin-saving = Saving…
pin-save-failed = The PIN couldn't be saved. Try again.
lock-title = Locked
lock-unlock = Unlock
lock-wrong-pin = Wrong PIN.
lock-wrong-password = Wrong password.
lock-pin-tries-left = { $count ->
    [one] 1 more wrong PIN and only your password will work.
   *[other] { $count } more wrong PINs and only your password will work.
}
lock-password-only = Use your password. { $count ->
    [one] 1 more wrong try signs this computer out.
   *[other] { $count } more wrong tries sign this computer out.
}
lock-unlocking = Unlocking…
lock-use-password = Use your password instead
lock-use-pin = Use your PIN instead
lock-sign-out = Sign out of this computer
lock-signed-out = Signed out of this computer after too many wrong tries. Sign in to use it again.
settings-lock-after = Lock after
lock-now = Lock now
lock-now-note = Locks at once, as Lock after does.
settings-pin = PIN
pin-change = Change PIN
pin-change-current = Current PIN
pin-change-password = Your password
pin-change-new = New PIN
pin-change-new-again = New PIN again
pin-changed = PIN changed. Use the new one from now on.
offline-tag = OFFLINE · copy { $time }
offline-tag-no-time = OFFLINE
offline-tag-note = Can't reach the server. Showing this computer's copy from { $time }.
offline-saving-note = Saving needs the server (offline).
offline-needs-server = This needs the server, which can't be reached (offline).
connecting-title = Connecting to the server…
# $seconds (number): seconds left before the copy opens offline.
connecting-offline-in =
    { $seconds ->
        [one] No answer yet. Opening this computer's copy (offline) in { $seconds } second.
       *[other] No answer yet. Opening this computer's copy (offline) in { $seconds } seconds.
    }
connecting-offline-now = Start in Offline Mode Now
copy-heading = Offline copy
copy-status = Status
copy-last-pull = Last pull
copy-records = Records
copy-size = Size on disk
copy-folder = Folder
copy-sync-now = Sync now
copy-closed = Not open (locked, or not signed in yet)
copy-downloading = Downloading… { $done } of { $total }
copy-up-to-date = Up to date
copy-offline = Offline: can't reach the server
copy-other-schema = Not updating: the server's database version is { $server } and this app's is { $app }. Update the app or the server.
outbox-chip = ↑ { $count } to send
outbox-chip-note =
    { $count ->
        [one] 1 change waiting to be sent, from { $when }. It goes when the server answers.
       *[other] { $count } changes waiting to be sent; the oldest from { $when }. They go when the server answers.
    }
outbox-long-title = Changes waiting to be sent
outbox-long-body =
    { $count ->
        [one] 1 change on this computer hasn't reached the server since { $when }. Connect to the server to send it.
       *[other] { $count } changes on this computer haven't reached the server; the oldest is from { $when }. Connect to the server to send them.
    }
outbox-stopped-title = Changes can't be sent
outbox-stopped-app = This app is too old for the server: update it to send your changes. They're kept on this computer.
outbox-stopped-server = The server is older than this app: update the server to send your changes. They're kept on this computer.
outbox-stopped-clock = This computer's clock is { $minutes } minutes ahead of the server's: set its date and time right to send your changes.
outbox-stopped-busy = The server can't take changes right now; they'll be sent later.
refused-title = Not saved: { $what }
refused-made = Made on this computer, { $when }.
refused-open = Open
refused-discard = Discard
change-event = { $kind } on { $vehicle }
change-event-edit = A change to an event
change-event-finalize = Finalizing an event
change-event-finish = Finishing a job
change-event-delete = Deleting a draft
change-event-correct = A correction ({ $kind }) on { $vehicle }
change-squawk = “{ $text }” on { $vehicle }
change-squawk-edit = A change to a reported problem
change-squawk-delete = Removing a reported problem
change-preferences = Your settings
change-place = Saved place “{ $name }”
change-place-edit = A change to a saved place
change-place-delete = Removing a saved place
change-garage-order = The Garage order
change-garage-pins = Pinned vehicles
change-attachment-add = A photo or document on { $vehicle }
change-attachment-move = Moving a photo or document
change-attachment-remove = Removing a photo or document
sign-out-waiting-title =
    { $count ->
        [one] 1 change hasn't been sent
       *[other] { $count } changes haven't been sent
    }
sign-out-waiting-body = Signing out deletes this computer's copy, and these changes with it.
sign-out-not-saved = { $what } (not saved)
sign-out-and-more = and { $count } more
sign-out-send-now = Send now
sign-out-anyway = Sign out anyway
copy-waiting = Waiting to send
copy-refused = Not saved
copy-oldest = Oldest waiting
copy-sending = Sending
photo-limit-1-gb = 1 GB
photo-limit-2-gb = 2 GB
photo-limit-5-gb = 5 GB
photo-limit-10-gb = 10 GB
photo-limit-none = No limit
photo-preview-note = Full size downloads when this computer can reach the server (a small preview).
file-not-here = Not on this computer yet. It downloads when the server can be reached.
upload-waiting = Waiting to upload (offline).
copy-files = Files
copy-files-value = { $count } · { $size }
copy-photos-kept = Photos kept
copy-photos-kept-value = { $used } of { $limit }
copy-photo-limit = Keep photos up to
copy-unwell = Waiting: the server isn't well (its database failed a check)
protected-tag = PROTECTED
protected-server-changed = Syncing stopped: the server's database changed (it was reset, or it's another server). This computer's copy is kept.
protected-went-back = Syncing stopped: the server went back in time (it was restored from an older backup). This computer's copy is kept.
protected-forgotten = Syncing stopped: the server no longer knows this computer. Its copy and unsent changes are kept.
protected-pull-held = Syncing stopped: the server would delete { $count } records. This computer's copy is kept.
protected-details = Details
protected-title = Protecting this computer's copy
protected-keep = Keep protected
protected-use-server = Use this server
protected-use-server-note = A snapshot of this copy is kept first. Changes not yet sent go to this server and are checked there.
protected-rebuild = Rebuild the server from this computer
protected-sign-in-note = The server no longer knows this computer. Its copy and { $count } unsent changes are kept: sign in again to carry on, or delete the copy.
protected-delete-copy = Delete this computer's copy
protected-delete-confirm = Delete this computer's copy and its unsent changes? This can't be undone.
held-title = The server would delete { $count } records
held-vehicle = { $vehicle }: { $count }
held-note = If that's not right, keep this computer's copy.
held-keep = Keep this computer's copy
held-apply = Apply them
held-apply-note = Apply takes a snapshot first.
rebuild-title = Rebuild the server · step { $step } of 3
rebuild-check-complete = This copy was complete at its last pull
rebuild-check-nothing-waiting = Nothing waiting to be sent or refused
rebuild-check-fresh = The server is new (it's asking for setup)
rebuild-what-records = A new database, with this computer's records
rebuild-what-passwords = Everyone else gets a new temporary password, shown once, to give out
rebuild-what-devices = Other computers pair again; people's own settings and saved places start empty
rebuild-what-photos = Photos this computer never opened are left out
rebuild-code = The server's setup code (from its log)
rebuild-password = Your password (from now on)
rebuild-button = Rebuild
rebuild-busy = Rebuilding…
rebuild-next = Next
rebuild-back = Back
rebuild-done = The server is rebuilt.
rebuild-passwords = Give each person their temporary password; they choose a new one when they sign in:
rebuild-left-out = Left out (not on this computer): { $names }
rebuild-unsettled = The server is rebuilt, but this computer's photos and documents aren't sent yet, and the photos it doesn't have aren't left out yet. Send them when the server can be reached.
rebuild-settle = Send the files
rebuild-settle-failed = The files couldn't be sent yet (the log says why). Try again when the server can be reached and you're signed in.
rebuild-wrong-code = That isn't the server's setup code.
rebuild-not-fresh = This server isn't new: only a new server can be rebuilt.
rebuild-not-allowed = Only the household's Admin can rebuild the server, with a password that follows the usual rules.
rebuild-not-ready = This computer can't rebuild the server yet (see the checks).
rebuild-failed = The rebuild didn't finish (the log says why). This computer's copy is unchanged.
rebuild-offer = This computer has a copy of the household's records. Rebuild this server from it?
snapshots-heading = Snapshots of this copy
snapshot-restore = Restore
snapshot-restore-confirm = Put this snapshot back as this computer's copy? Changes since then come back from the server at the next pull; changes not yet sent are kept.
snapshot-none = None yet
snapshot-take = Take one now
sync-unwell-title = The server isn't well
sync-unwell-body = Its database failed a check, so it isn't taking or sending changes. This computer waits, and carries on when it's well.
copy-failed = Not updating: something went wrong (see the log below)
copy-never = Never
setting-distance = Distance
setting-volume = Fuel volume
setting-economy = Fuel economy
setting-pressure = Pressure
setting-date-format = Date format
setting-currency = Currency
setting-language = Language
setting-squawk-word = Call problems
setting-squawk-word-household = The household's Admin chooses this, for everyone.
setting-originals = Keep photo originals
setting-originals-allowed = Allowed
setting-originals-off = Off
setting-originals-note = Maintainers and Admins may keep a photo's file as taken (with its location and camera details) beside the one shown. Originals already kept stay.
squawk-word-choice = { $Squawks }

## Squawk screens (milestone 7 step 7f)

tab-squawks = { $Squawks }
tab-squawks-count = { $Squawks } · { $count }
screen-squawk = { $Squawk }
squawk-new = + { $Squawk }
squawk-report = Report { $a-squawk }
squawk-sheet-title = New { $squawk } · { $vehicle }
squawk-sheet-edit-title = Change this { $squawk }
squawk-description-label = What's wrong?
squawk-severity-label = Severity
squawk-high-note = High keeps it out of use: shown as { $word }.
squawk-more = Notes, when noticed
squawk-notes-label = Notes (optional)
squawk-noticed-date = Noticed
squawk-not-found = That { $squawk } isn't there any more.
squawks-none-outstanding = No open { $squawks }.
squawks-all = All { $squawks } ›
squawks-filter-outstanding = Outstanding · { $count }
squawks-filter-resolved = Resolved · { $count }
squawks-filter-all = All
squawks-none-here = Nothing here.
squawks-select-hint = Choose { $a-squawk } to see its history.
squawk-opened-by = Opened by { $name } · { $date }
squawk-found-on = Found on: { $event } ›
squawk-found-on-event = an event
squawk-notes-heading = Notes
squawk-history-heading = History
squawk-history-opened = Opened · { $severity }
squawk-history-signed-off = Signed off
squawk-history-deferred = Deferred: { $reason }
squawk-history-job = Job · { $date } ›
squawk-locked-note = A job has dealt with it, so it changes only in the next job.
squawk-remove = Remove
squawk-remove-title = Remove this { $squawk }?
squawk-remove-text = Only for one opened by mistake: it's gone for everyone.
log-squawk-detail = Something's wrong

## The Maintenance tab (milestone 7 step 7g)

setup-schedule-moved = The maintenance schedule is under Maintenance › Set up ›
maintenance-schedule = Schedule
maintenance-jobs = Jobs
maintenance-set-up = Set up
timeline-in-progress = In progress
timeline-task-count = { $count ->
        [one] 1 task
       *[other] { $count } tasks
    }
jobs-empty = No maintenance or repairs logged yet.
schedule-every = Every { $rules }
schedule-or = or
schedule-months = { $count ->
        [one] month
       *[other] { $count } months
    }
schedule-days = { $count ->
        [one] day
       *[other] { $count } days
    }
schedule-years = { $count ->
        [one] year
       *[other] { $count } years
    }
schedule-fixed = On a fixed date
schedule-fixed-next = On a fixed date · next { $date }
schedule-when-needed = When needed
schedule-last-done = Last done
schedule-not-done = Not done yet
schedule-the-job = The job ›
schedule-bundles = Service bundles
schedule-requirements = Requirements
schedule-empty-title = No schedule yet
schedule-empty-hint = Track oil, tires, inspections, and anything else done to it.
schedule-nothing-tracked = Nothing is tracked yet.
schedule-set-it-up = Set it up
schedule-copy-from = Copy from…
schedule-add-requirement = + Requirement
schedule-add-bundle = + Bundle
schedule-none-yet = No requirements yet.
schedule-no-bundles = No bundles. A bundle is a service done as one, e.g. a 5,000-mile service.
schedule-off = Off
schedule-remove-note = A requirement a job has done can't be removed (turn it off instead). Removing one takes it out of its bundles.
schedule-add-requirement-title = New requirement
schedule-name = Name
schedule-in-use = In use
schedule-due-when = Due when, whichever comes first
schedule-add-meter-rule = + Meter rule
schedule-add-calendar-rule = + Calendar rule
schedule-add-fixed-rule = + Fixed date
schedule-add-reaches-rule = + When a meter reaches
schedule-when-label = When
schedule-reaches-label = reaches
# $meter (text): the meter's name; $value (text): the mark with its unit, e.g. "15 %".
schedule-reaches = when { $meter } reaches { $value }
schedule-rules-hint = No rules: done when needed. A fixed date's first date is optional; each job gives the next.
schedule-every-label = every
schedule-calendar = Calendar
schedule-months-label = months
schedule-unit-days = days
schedule-unit-years = years
schedule-calendar-unit = Days, months, or years
schedule-end-of-month = by the month's end
schedule-fixed-label = Fixed date
schedule-first-due = first due
schedule-add-bundle-title = New bundle
schedule-edit-bundle-title = Change this bundle
schedule-bundle-covers = What it covers
schedule-bundle-covers-list = Covers { $names }
schedule-bundle-label = Service bundle
schedule-bundle-count = { $count ->
        [one] 1 requirement
       *[other] { $count } requirements
    }
schedule-remove-title = Remove { $name }?
schedule-copy-title = Copy a schedule to { $vehicle }
schedule-copy-from-label = From
schedule-copy-no-vehicles = There's no other vehicle to copy from.
schedule-copy-summary = { $requirements ->
        [one] 1 requirement
       *[other] { $requirements } requirements
    } and { $bundles ->
        [one] 1 bundle
       *[other] { $bundles } bundles
    }. Copies, never links.
schedule-copy-left-out = Left out (no matching meter on { $vehicle }):
schedule-copy = Copy

## The work log (milestone 7 step 7i)

log-maintenance-detail = Service, inspections, the schedule
log-repair-detail = Fixing something wrong
event-status-job-draft = Draft · finish it when the work is done
squawk-repair-this = Repair this
work-end-readings = Readings at the end
work-add-end-readings = + Readings at the end (after a test drive)
work-end-reading = { $meter } at the end ({ $unit })
work-tasks = Tasks
work-from-requirement = + From a requirement…
work-from-bundle = + From a bundle…
work-add-task = + Task
work-no-tasks = No tasks yet: add one from a requirement, a bundle, or your own.
work-untitled-task = (no title)
work-fold-task = Fold this task
work-part-count = { $count ->
        [one] 1 part
       *[other] { $count } parts
    }
work-task-title = What was done
work-task-for = For: { $name }
work-parts = Parts
work-part-what = What
work-part-number = Part number
work-part-quantity = Quantity
work-part-price = Unit price
work-add-part = + Part
work-labor-time = Labor (h:mm)
work-labor-cost = Labor cost ({ $currency })
work-resets-from = Resets the requirement from
work-resets-start = Start
work-resets-end = End readings
work-next-due = Next due (from the paperwork)
work-task-total = Task: { $total }
work-squawks = { $Squawks } · { $count } to sign off or defer
work-no-squawks = None open on this vehicle.
work-open-squawk = + Found something new? Report { $a-squawk }
work-sign-off = Sign off
work-defer = Defer
work-fixed-by = Fixed by
work-fixed-by-none = No task
work-defer-why = Why
work-severity-same = No change
work-repair = Repair
work-complaint = Complaint: what was wrong
work-cause = Cause: the diagnosis
work-repair-note = The tasks are the fix.
work-total = Total
work-save-draft = Save draft
work-finish = Finish…
work-finish-title = Finish this job?
work-check-readings-ok = The required readings
work-check-readings-missing = A required reading is missing: give it, or skip it with a reason
work-check-place-ok = Where it was done
work-check-place-missing = Where it was done: set it, or say why you can't
work-check-undecided = { $description }: sign it off or defer it
work-finish-locks = Finishing locks it: a mistake is fixed with a correction.
work-finish-back = Back
work-finish-lock = Finish and lock

## A job's page (milestone 7 step 7j)

event-status-job-finished = Finished · { $date }
job-total = Total
job-split = parts { $parts } · labor { $labor }
job-labor = Labor
job-cost = Cost
job-no-tasks = No tasks.
job-resets-start = resets from the start readings
job-resets-end = resets from the end readings
job-next-due = Next due: { $date }
job-notes = Notes: { $notes }
job-squawks = { $Squawks } dealt with
job-fixed-by = by { $task }
job-deferred = Deferred
job-found = Found during this job: { $description } ({ $severity }) ›
timeline-signed-off = { $count } signed off
timeline-deferred = { $count } deferred
unit-miles = Miles
unit-kilometers = Kilometers
unit-us-gallons = US gal
unit-imperial-gallons = Imp gal
unit-liters = Liters
unit-mpg-us = mpg (US)
unit-mpg-uk = mpg (UK)
unit-l-per-100-km = L/100 km
unit-km-per-l = km/L
unit-psi = psi
unit-kpa = kPa
unit-bar = bar
date-sample-day-month-name-year = 30 Oct 2026
date-sample-month-day-year = 10/30/2026
date-sample-day-month-year = 30/10/2026
date-sample-year-month-day = 2026-10-30
language-english-us = English (US)

## Idle sign-out and Admin timeout (decision #049)

settings-sign-in = Sign-in
setting-idle-sign-out = Sign out when idle
idle-sign-out-one-hour = After 1 hour
idle-sign-out-four-hours = After 4 hours
idle-sign-out-eight-hours = After 8 hours
idle-sign-out-twenty-four-hours = After 24 hours
idle-sign-out-never = Never
# $minutes (number): Admin mode's idle limit.
idle-admin-ended =
    { $minutes ->
        [one] Admin mode ended after { $minutes } idle minute.
       *[other] Admin mode ended after { $minutes } idle minutes.
    }
# $hours (number): how long the app was idle.
idle-signed-out =
    { $hours ->
        [one] You were signed out after { $hours } idle hour.
       *[other] You were signed out after { $hours } idle hours.
    }
notice-dismiss = Dismiss

## Message center (decisions #040, #050)

inbox-snapshot-failed = The nightly snapshot failed
inbox-snapshot-skipped = The nightly snapshot was skipped
inbox-quick-check-failed = The hourly database check failed
inbox-file-check-failed = Some photos or documents are missing or damaged
# The job's name is in the message's details.
inbox-job-failed = A scheduled job failed
inbox-job-skipped = A scheduled job was skipped
inbox-job-body = See the details below and the server's log.
inbox-snapshot-body = See the details below and the server's log. Until this is fixed, there's no new backup of the database.
inbox-quick-check-body = The hourly check found a problem in the database, so the server is refusing changes. See the details below and the server's log.
inbox-file-check-body = The nightly check of the photo and document files in data/files/ found a problem. See the details below and the server's log. A missing or damaged file can be copied back from a backup of data/files/.
inbox-server-unhealthy = The server isn't accepting changes
inbox-place-lookup-unavailable = Place lookup isn't available
inbox-place-lookup-body = The server couldn't open its place data, so it can't suggest a city, state, and country from a location. Saved places and typing a place still work. Restarting the server, or updating it, usually fixes this; the details below say what went wrong.
inbox-database-unavailable = The server couldn't open its database. You can still look around, but nothing can be saved until it's fixed.
inbox-integrity-check-failed = The server found a problem in its database. You can still look around, but nothing can be saved until it's fixed.
inbox-open-diagnostics = Open Diagnostics
inbox-forgotten-job = A job is still a draft
inbox-forgotten-job-body = A maintenance or repair job on { $vehicle } hasn't been touched for a while. Finish it when the work is done, or delete it if it was a mistake.
inbox-open-job = Open the job
messages-filter-all = All
messages-severity-urgent = URGENT
messages-severity-warning = WARNING
messages-severity-info = INFO
messages-filter-urgent = Urgent
messages-filter-warning = Warning
messages-filter-info = Info
# $count (number): unread messages.
messages-unread =
    { $count ->
        [0] All read
        [one] { $count } unread
       *[other] { $count } unread
    }
messages-select-hint = Choose a message to read it.
messages-empty = No messages. You're all caught up.
messages-back = ← Messages
messages-details = DETAILS
messages-dismiss = Dismiss
# $time (text): e.g. 2026-10-31 07:00.
messages-time-utc = { $time } UTC
urgent-banner-label = URGENT
urgent-banner-view = View
# $minutes (number): how long the app was idle, when it isn't whole hours (test timers).
idle-signed-out-minutes =
    { $minutes ->
        [one] You were signed out after { $minutes } idle minute.
       *[other] You were signed out after { $minutes } idle minutes.
    }

## Test timers (decision #054)

# $minutes (number): the test minute setting.
test-timers-tag = TEST TIMERS · { $minutes } min
# $label (text): the normal choice, e.g. "After 1 hour"; $minutes (number): its test length.
idle-sign-out-test = { $label } (test: { $minutes } min)


## Assets, groups, and notes (decisions #052, #055)

notes-too-long = That's too long: notes can have at most 10,000 characters.
group-name-empty = Give the group a name.
group-name-too-long = A group's name can have at most 40 characters.
asset-name-empty = Give the vehicle a name.
asset-name-too-long = A vehicle's name can have at most 40 characters.
asset-text-too-long = That's too long: at most 64 characters.
asset-year-out-of-range = That year doesn't look right.
asset-identifier-empty = Enter the identifier, or remove it.
asset-too-many-identifiers = A vehicle can have at most 10 identifiers.
asset-bad-in-service = In service since must be a date, no later than today.
field-in-service = In service since
field-in-service-optional = In service since (optional)
field-in-service-estimated = Estimated
field-in-service-note = Built, or first registered. For a used vehicle, backdate it, estimated if need be. A schedule's "first at 3 years" counts from it; without it, from the day the vehicle was acquired.
# $date (string): the date, already formatted.
setup-in-service-estimated = { $date } (est.)
# $first, $every (string): the first interval and the repeating one, e.g. "100,000 mi".
schedule-first-then = First at { $first }, then every { $every }
schedule-first-different = A different first interval
schedule-first-at-label = First at
# $unit (string): the meter's unit, e.g. "mi".
schedule-first-from-zero = { $unit } on the meter (from zero)
# $unit (string): days, months, or years.
schedule-first-from-start = { $unit } from In service since (else Acquired)

## Profiles, meters, and custom fields (decision #030). Profile names, identifiers, documents,
## meters, and fields below are keys used by crates/domain/profiles/*.ron.

profile-car = Car
profile-truck = Truck
profile-motorcycle = Motorcycle
profile-trailer = Trailer
profile-generator = Generator
profile-other-equipment = Other equipment
identifier-vin = VIN
identifier-plate = License plate
identifier-serial = Serial number
document-title = Title
document-registration = Registration
document-insurance = Insurance
meter-odometer = Odometer
meter-run-hours = Run hours
field-trip-time = Trip time
field-trip-ev-distance = Trip EV distance
field-trip-distance = Trip distance
field-trip-average-speed = Trip average speed
field-trip-fuel-economy = Trip fuel economy
field-trip-fuel-used = Trip fuel used
field-trip-ev-share = Trip EV share
field-trip-ev-time = Trip EV time
field-idle-time = Idle time
field-engine-run-time = Engine run time
field-oil-life = Oil life %
field-def-level = DEF level %
field-tire-pressure = Tire pressure
field-tread-depth = Tread depth
catalogue-set-trip-computer = Trip computer
catalogue-set-trip-computer-hybrid = Trip computer (hybrid)
catalogue-set-engine-idle = Engine and idle
event-kind-fuel = Fuel-up
event-kind-reading = Reading
event-kind-maintenance = Maintenance
event-kind-repair = Repair
unit-hours = hours
due-ok = OK
due-coming-up = Coming up
due-soon = Due soon
due-very-soon = Due very soon
due-overdue = ⚠ Overdue
due-bad-warnings = Each warning must come no earlier than the one before it: yellow, then orange, then red (at most 100%, or 730 days).
requirement-bad-before-records = Check "Last done before these records": a date, and each meter's reading once, none below zero.
sort-most-urgent = Most urgent
# $amount (text): e.g. "310 mi".
due-in = in { $amount }
due-over-by = { $amount } over
# $meter (text): e.g. "Oil life"; $amount (text): e.g. "80 %".
due-to-go = { $meter }: { $amount } to go
due-meter-over = { $meter }: { $amount } over
# $date (text): a date as the person writes dates.
due-by = by { $date }
due-was = was due { $date }
due-about = ~{ $date }
due-or = or
due-not-done-yet = Not done yet
# $interval (text): e.g. "20 / 10 / 5"; $days (text): e.g. "60 / 30 / 14".
due-warnings-text = { $interval }% left · { $days } days before a date
due-before-records = Last done before these records (optional)
due-before-date = Date
due-warn-me = Warn me
# $warnings (text): the household's warnings in words.
due-household-default = Household default: { $warnings }
# $days (text): e.g. "60 / 30 / 14".
due-household-days = Household default: { $days } days before it expires
due-change = Change
due-use-household = Use the household's warnings
due-percent-left = Of the interval left
due-days-before = Before a date (a fixed date, an expiry)
due-percent = %
due-days = days
# $meter (text): the meter, e.g. "Oil life"; $value (text): its reset value, e.g. "100 %".
work-reset-meter = ↺ Reset { $meter } to { $value } at the end of the job
setting-due-warnings = Due soon warnings
setting-due-warnings-household = The household's Admin sets these for everyone.
due-built-in = Use the built-in warnings (20 / 10 / 5% · 60 / 30 / 14 days)
unit-percent = %
label-empty = Enter a name.
label-too-long = That's too long: at most 40 characters.
meter-wrong-unit = That unit doesn't measure this meter (e.g. hours for a distance).
meter-too-precise = A meter can show at most 2 decimal places.
meter-negative-reset = A reset value can't be below zero.
meter-resettable-fuel = A distance meter that fuel-ups need can't be reset: fuel economy needs its running total.
meter-direction-fixed = Which way a meter counts can't change once it has a reading.
meter-counts-up = Up
meter-counts-down = Down
meter-counts = Counts
meter-counts-note = Down: e.g. Oil life, from 100% toward 0%. Fixed once the meter has a reading.
meter-resettable = Can be reset
meter-summary-down = counts down
# $value (text): the reset value with its unit, e.g. "100 %".
meter-summary-resets = resets to { $value }
meter-reset-to = to
meter-reset-note = A reading can then be marked ↺ Reset, e.g. when the oil is changed.
field-too-many-decimals = A number can have at most 4 decimal places.
field-bad-choices = A choice list needs 1 to 30 choices.
field-bad-range = The range doesn't fit this field: its lowest value must not be above its highest, and only numbers have one.
group-not-empty = Move this group's vehicles to another group first.
group-last = Every vehicle needs a group, so the last one can't be removed.
name-mismatch = Type the vehicle's name exactly to delete it.
group-default-name = Garage

## Vehicles: All vehicles, a group's page, and adding a vehicle (decisions #052, #056)

nav-all-vehicles = All vehicles
vehicles-add = + Add vehicle
vehicles-sort-label = Sort
sort-newest-activity = Sort: Newest activity
sort-name = Sort: Name (A–Z)
sort-custom = Sort: Custom
vehicles-arrange = Arrange
vehicles-arrange-done = Done
# $name (string): the vehicle's name.
vehicles-card-menu = More for { $name }
vehicles-open = Open
vehicles-pin = Pin
vehicles-unpin = Unpin
vehicles-pinned = Pinned
# $count (number): archived vehicles not shown.
vehicles-show-archived = Show archived ({ $count })
vehicles-hide-archived = Hide archived
# $count (number): vehicles in a group.
vehicles-count =
    { $count ->
        [one] { $count } vehicle
       *[other] { $count } vehicles
    }
vehicles-empty = No vehicles yet.
vehicles-empty-admin = Use + Add vehicle to add the first one.
vehicles-empty-group = No vehicles in this group yet.
vehicles-group-menu = Show
vehicles-move-up = Move up
vehicles-move-down = Move down
add-vehicle-title = Add a vehicle
add-vehicle-kind = What kind?
add-vehicle-add = Add
add-vehicle-adding = Adding…
add-vehicle-name = Name (on cards)
add-vehicle-note = It starts with its kind's meters and custom fields; change them later on its Setup tab.
field-make = Make
field-model = Model
field-year = Year
field-colour = Colour
field-group = Group
status-in-service = In service
status-spare = Spare
status-out-for-service = Out for service
status-retired = Retired
status-disposed = Disposed
vehicles-not-allowed = Your role or mode doesn't allow that. Switch mode, or ask your Admin.

## A vehicle's page: tabs, Overview, and Setup (decision #057)

vehicle-not-found = That vehicle isn't here, or you can't see it.
vehicle-back-all = ← All vehicles
# $group (text): the vehicle's group.
vehicle-back-group = ← { $group }
vehicle-tabs = Vehicle sections
tab-overview = Overview
tab-timeline = Timeline
tab-maintenance = Maintenance
tab-fuel = Fuel
tab-documents = Documents
tab-setup = Setup
# $milestone (number): the Phase 2 milestone that fills the tab.
tab-coming = This tab fills in with milestone { $milestone }.
panel-ready = Ready to use?
panel-due = Coming up
panel-due-none = Nothing comes due yet. Set up its schedule, or add documents with an expiry.
panel-due-see-all = See all on Maintenance →
panel-squawks = Open { $squawks }
panel-fuel = Latest fuel-ups
panel-meters = Meters
meters-none = No meters yet.
# $kinds (text): the event kinds, joined with commas.
meter-required-for = Required for: { $kinds }
meter-required-none = Not required for any event.
precision-0 = whole
precision-1 = tenths
precision-2 = hundredths
dimension-distance = Distance
dimension-duration = Duration
dimension-percentage = Percentage
setup-details = Details
setup-edit-details = Edit details
setup-no-description = No description.
setup-add-meter = + Add meter
setup-fields = Custom fields
setup-add-field = + Add field
setup-fields-none = No custom fields yet.
setup-group-status = Group and status
setup-move = Move to another group…
setup-status = Status
setup-delete = Delete…
setup-view-only = Changing this vehicle's setup needs Maintainer or Admin mode.
action-edit = Edit
action-remove = Remove
action-save = Save
action-saving = Saving…
field-description = Description
field-description-note = Plain text for now; **bold**, lists, and links show as typed (#055).
meter-add-title = Add a meter
meter-edit-title = Edit meter
field-name = Name
field-measures = Measures
field-unit = Unit
field-precision = Decimal places
field-meter-description = What drives it (optional)
meter-dimension-fixed = What a meter measures can't change once it exists.
# $name (text): the meter's or field's name.
remove-title = Remove { $name }?
meter-remove-text = Its readings (from milestone 6) go with it.
field-remove-text = To stop asking for it but keep its values, turn it off instead.
field-add-title = Add a custom field
field-edit-title = Edit field
field-label = Label
field-type = Type
field-choices = Choices (one per line)
field-enabled = Shown on forms
field-off = Off
field-range-min = Lowest likely
field-range-max = Highest likely
field-range-note = Values outside the range get a warning, never a refusal.
field-range-bad-number = Enter a number for the range, or leave it empty.
field-key = Key
field-key-note = Formulas and reports use the key, so it never changes (#057).
field-key-later = Made from the label when the field is added.
field-kind-fixed = A field's type can't change once it exists.
kind-text = Text
kind-number = Number
kind-date = Date
kind-yes-no = Yes/No
kind-choice = Choice list
kind-distance = Distance
kind-duration = Duration
kind-volume = Volume
kind-fuel-economy = Fuel economy
kind-calculated = Calculated
kind-pressure = Pressure
kind-tread-depth = Tread depth
# $name (text): the vehicle's name.
move-title = Move { $name }
move-to = To
move-affected = Who's affected
move-nobody = Nobody's access changes.
# $name (text): a person; $before and $after (text): their role, or "no access".
move-change = { $name }: { $before } → { $after }
access-none = no access
move-move = Move
move-moving = Moving…
move-no-other-group = There's no other group yet. The household's Admin adds groups.
# $name (text): the vehicle's name.
delete-title = Delete { $name }?
delete-text = Only for a vehicle added by mistake: its records go with it. To keep its history, set its status to Retired or Disposed instead.
# $name (text): the vehicle's name.
delete-delete = Delete
delete-deleting = Deleting…

## Groups and roles on a group or one vehicle (decision #058)

nav-groups = Groups
# $role (text): a role; $target (text): what it's on.
users-role-on = { $role } · { $target }
# $count (number): how many roles.
users-roles-count = { $count } roles
roles-add = + Add role
# $name (text): the person.
roles-add-title = Add a role for { $name }
roles-on = On
roles-role = Role
roles-add-button = Add
# $name (text): the group.
roles-on-group = { $name } · group
# $name (text): the vehicle.
roles-on-vehicle = { $name } · vehicle
roles-groups = Groups
roles-vehicles = Vehicles
roles-removed = (removed)
roles-remove = Remove this role
roles-none = No roles: they can't see any vehicles yet.
roles-note = Roles add up: on each vehicle, the highest one applies. Adding a role where they already have one replaces it.
groups-add = + Add group
groups-add-title = Add a group
groups-icon = Icon
groups-select-hint = Choose a group to see its vehicles and who sees it.
groups-vehicles = Vehicles
groups-no-vehicles = No vehicles in this group yet.
groups-open = Open ›
groups-who = Who sees it
# $role (text): the role.
groups-from-all = { $role } · all vehicles
# $role (text): the role.
groups-from-group = { $role } · this group
# $role (text): the role; $vehicle (text): the vehicle.
groups-from-vehicle = { $role } · { $vehicle } only
groups-no-access = no access
groups-delete = Delete group…
# $name (text): the group.
groups-delete-title = Delete { $name }?
groups-delete-text = Roles on this group are removed with it.
groups-delete-note = Only an empty group can be deleted, and never the last one.
groups-saves-note = The name and icon save when changed. Roles are set on the Users screen.
groups-saved = Saved.
groups-not-allowed = Switch to Admin mode to manage groups.
groups-back = ← Groups
group-icon-garage = Garage
group-icon-hangar = Hangar
group-icon-boathouse = Boathouse
group-icon-storage = Storage
group-icon-warehouse = Warehouse
group-icon-shop = Shop
group-icon-farm = Farm
group-icon-house = House

## Location (decisions #038, #059)

location-latitude-range = Latitude must be between -90 and 90.
location-longitude-range = Longitude must be between -180 and 180.
location-not-coordinates = Enter decimal degrees, e.g. 35.5951 and -82.5515.
location-radius-range = The radius must be between 25 m and 2 km.
location-city-empty = Enter the city or town.
location-region-empty = Enter the state or province.
location-place-too-long = That's too long: at most 100 characters.
location-country-invalid = Choose a country.
location-time-zone-invalid = That time zone isn't recognised.

## Places, the location picker, and About (decisions #059, #060)

nav-places = Places
places-add = + Add place
places-intro = Your places: only you see them. When you're inside one, a new event uses it.
places-empty = No places yet. Add the ones you go to often, like home or your usual fuel stop.
places-remove-note = Removing a place never changes past events: each event keeps its own copy.
# $meters: `meters` (number).
places-meters = { $meters } m
# $km: `km` (number).
places-kilometers = { $km } km
place-add-title = Add a place
place-edit-title = Edit place
place-step-where = 1 · Where
place-step-place = 2 · Place
place-step-name = 3 · Name
place-use-location = Use my location
place-locating = Finding your location…
place-type-coordinates = Type coordinates
place-coordinates = Latitude, longitude
place-coordinates-hint = For example 35.5951, -82.5515 (as map apps copy them).
place-coordinates-apply = Use these
# $meters: `meters` (number).
place-fix-now = ± { $meters } m · just now
# $meters, $minutes: `meters` and `minutes` (numbers).
place-fix-minutes = ± { $meters } m · { $minutes } min ago
place-fix-typed = Typed coordinates
place-fix-stale = This location is over 20 minutes old.
place-no-location = No location yet: use yours, or type coordinates.
place-location-denied = Location permission was refused. Allow it in the browser's settings, or type coordinates.
place-location-unavailable = The location isn't available here. Type coordinates instead.
# $place: `place` (text).
place-suggested = Suggested: { $place }
# $zone, $version: `zone` and `version` (text).
place-suggested-detail = Time zone { $zone } · from place data { $version }
# $place: `place` (text).
place-offshore = Outside the map's coastline (at sea, or on a wide river or bay): the nearest place is { $place }. Check it.
# $name, $meters: `name` (text) and `meters` (number).
place-at-saved = You're at { $name } ({ $meters } m away). Its city, state, and country are used.
place-lookup-unavailable = Place lookup isn't available: type the city, state, and country.
place-use-suggestion = Use the suggestion
place-city = City or town
place-region = State or province
place-country = Country
place-country-code = Country code (e.g. US)
place-typed-note = Type any city or state, e.g. a base that isn't in the list.
place-name = Name
place-radius = Radius
place-needs-location = Set where it is first: use your location, or type coordinates.
# $name: `name` (text).
place-remove-title = Remove { $name }?
place-remove-text = Past events keep their own copy of it.
about-heading = About
about-app = App
about-build = Build
about-server = Server
about-build-local = local build
about-build-modified = uncommitted changes
about-place-data = Place data
about-place-data-unavailable = Not available
about-credits = Credits
about-credits-default = Contains GeoNames data (CC BY 4.0, geonames.org). Boundaries: Natural Earth (public domain, naturalearthdata.com).
about-licenses = Licenses
about-licenses-open = Third-party notices
licenses-intro = Vehicle Manager is built with the work of others. These are their licenses and notices.

## Nearby places in the picker (decision #061)

place-nearby = Nearby: choose the city or town
place-nearby-note = Not in the list? Type the city or town below.
# $km: `km` (text, already formatted).
place-distance-km = { $km } km
# $miles: `miles` (text, already formatted).
place-distance-mi = { $miles } mi
place-kind-capital = Capital
place-kind-district = District
place-kind-locality = Locality
place-kind-city = City
place-kind-town = Town
place-kind-village = Village
place-seat-region = state or province capital
place-seat-county = county seat
place-seat-local = local seat
compass-n = N
compass-ne = NE
compass-e = E
compass-se = SE
compass-s = S
compass-sw = SW
compass-w = W
compass-nw = NW

## Dates and times (decisions #008, #048)

date-bad-date = Enter a date, e.g. 2026-11-02.
date-bad-time = Enter a time, e.g. 15:30.
date-unknown-zone = That time zone isn't recognised.
date-end-before-start = The end date can't be before the start.
date-out-of-range = That date is too far in the past or future.
time-am = am
time-pm = pm
month-1 = Jan
month-2 = Feb
month-3 = Mar
month-4 = Apr
month-5 = May
month-6 = Jun
month-7 = Jul
month-8 = Aug
month-9 = Sep
month-10 = Oct
month-11 = Nov
month-12 = Dec

## Quantities in each person's units (decisions #007, #029)

quantity-not-a-number = Enter a number, e.g. 12.5.
quantity-negative = That can't be below zero.
quantity-too-large = That's too large.
quantity-too-precise = That has too many decimal places.
quantity-not-a-duration = Enter hours and minutes, e.g. 1:30 or 1:30:15.
unit-abbr-mi = mi
unit-abbr-km = km
unit-abbr-gal = gal
unit-abbr-gal-uk = gal (UK)
unit-abbr-l = L
unit-abbr-mpg = mpg
unit-abbr-mpg-uk = mpg (UK)
unit-abbr-km-per-l = km/L
unit-abbr-l-per-100-km = L/100 km
unit-abbr-h = h
unit-abbr-percent = %
unit-abbr-qt = qt
unit-abbr-fl-oz = fl oz
unit-abbr-ml = mL
unit-abbr-ft-lb = ft-lb
unit-abbr-in-lb = in-lb
unit-abbr-n-m = N·m
unit-abbr-psi = psi
unit-abbr-kpa = kPa
unit-abbr-bar = bar
unit-abbr-in = in
unit-abbr-mm = mm
unit-abbr-32nds = /32 in
spec-kind-volume = Volume
spec-kind-torque = Torque
spec-kind-pressure = Pressure
spec-kind-length = Length or gap
spec-bad-detail = The detail can be up to 100 characters.
spec-bad-part-number = The part number can be up to 40 characters.
spec-bad-amount = The amount must be more than zero.
template-nothing-to-save = This vehicle has no requirements to save as a template yet.
spec-reminder = Reminder: { $name } { $amount }
spec-update-title = Update the specs too?
spec-update-note = The job records what was used either way.
spec-update-save = Save job
spec-update-back = Back
spec-update-none = none
maintenance-specs = Specs
spec-add = + Spec
spec-add-title = New spec
spec-none-yet = No specs yet: the oil it takes, its filter's part number, tire pressures.
spec-name = Name
spec-kind = Kind
spec-kind-none = None (a part number only)
spec-amount = Amount
spec-unit = Unit
spec-detail = Detail
spec-part-number = Part number
spec-part-number-shown = Part { $number }
spec-remove = Remove
spec-checklist = Specs its job uses
spec-checklist-note = Ticked specs pre-fill a task's parts; torque, pressure, and length show on the task as reminders.
schedule-log-service = Log this service
nav-templates = Templates
templates-intro = A template is a vehicle's requirements, bundles, and specs, saved under a name. Applying one copies it: later changes to the template don't reach the vehicle.
templates-none = No templates yet. Save one from a vehicle's Maintenance → Set up.
template-counts = { $requirements ->
    [one] 1 requirement
   *[other] { $requirements } requirements
} · { $bundles ->
    [one] 1 bundle
   *[other] { $bundles } bundles
} · { $specs ->
    [one] 1 spec
   *[other] { $specs } specs
}
template-applied = Copied { $requirements ->
    [one] 1 requirement
   *[other] { $requirements } requirements
}, { $bundles ->
    [one] 1 bundle
   *[other] { $bundles } bundles
}, and { $specs ->
    [one] 1 spec
   *[other] { $specs } specs
} onto { $vehicle }.
template-left-out = Left out (no matching meter): { $list }.
template-apply-to = Apply to…
template-apply = Apply
template-rename = Rename
template-name = Name
template-remove-title = Remove the template { $name }?
template-remove-note = Vehicles it was applied to keep their copy.
template-not-found = That template is gone.
template-save-as = Save as template…
template-save-title = Save this schedule as a template
template-from = From a template…
fuel-dropped-missed = Not counted: an earlier fill-up wasn't logged
fuel-dropped-reading = Not counted: the odometer wasn't read
fuel-dropped-distance = Not counted: the odometer didn't move
fuel-stretch = { $economy } over { $distance }
fuel-car-said = car said { $economy }
fuel-part-before = Before the first full fill-up: nothing to measure from yet
fuel-part-starts = Economy starts here
fuel-part-waiting = Partial: counts in the next full fill-up
fuel-part-added = Partial: added into the next full fill-up's figure
fuel-overview = { $latest } latest · last 5: { $last }
fuel-none-yet = Fuel economy shows once there are two full fill-ups.
fuel-last-five = Last 5
fuel-year = 12 months
fuel-all-time = All time
fuel-stretches = { $count ->
    [one] 1 stretch
   *[other] { $count } stretches
}
fuel-chart = Economy by stretch
fuel-show-chart = Show as a chart
fuel-show-table = Show as a table
fuel-chart-hint = Tap a dot for its details. A gap is a stretch that doesn't count.
fuel-average-line = average { $value }
fuel-table-ended = Ended
fuel-table-fuel = Fuel
fuel-table-figure = Economy
fuel-highway = Highway
fuel-towing = Towing
fuel-hauling = Hauling
fuel-towed = Towed vehicles
fuel-band-city = Mostly city
fuel-band-mixed = Mixed
fuel-band-highway = Mostly highway
fuel-band-none = None
fuel-band-some = Some
fuel-band-mostly = Mostly
fuel-not-enough = not enough yet
fuel-not-towing = Not towing
fuel-another-vehicle = Another vehicle
fuel-info-full = Filled full: the tank was filled until the pump stopped. Economy runs from one full fill-up to the next; a partial fill-up's fuel is added into the next full one, and it has no figure of its own.
fuel-info-missed = Missed previous fill-up: a fill-up since the last one here wasn't logged. The stretch it ends can't be measured, so it's left out of the averages and shows as a gap.
fuel-info-label = What this means

## Events: fuel-ups and readings (milestone 6)

event-reason-empty = Give a reason.
event-reason-too-long = That's too long: at most 500 characters.
event-bad-performer = Enter who did it (at most 100 characters).
event-location-required = Set where it was, or say why you can't.
event-required = Fill in the required items, or skip one with a reason.
event-reading-out-of-order = That reading doesn't fit: it's lower than an earlier one or higher than a later one. Check it, or correct the other event.
event-wrong-value = That value doesn't fit this field.
event-no-volume = Enter how much fuel.
event-bad-amount = That amount doesn't look right.
event-bad-percent = Percentages go up to 100.
event-tows-itself = A vehicle can't tow itself.
event-wrong-kind = This vehicle doesn't have that kind of event.
event-finalized = This event is finalized: correct it instead.
event-not-finalized = This event is still a draft: edit it instead.
event-already-corrected = This event was already corrected: correct the correction instead.
event-flag-missing-location = No location
event-flag-skipped = Skipped items
event-flag-reading-override = Reading kept by the Admin
event-flag-correction = Correction

## Required for (milestone 6, step 6d)

meter-required-label = Required for
meter-required-note = Events of a checked kind must record this meter, or skip it with a reason.

## + Log and the log forms (milestone 6, step 6g)

log-title = Log
log-vehicle = Vehicle
log-last-time = { $name } · same as last time
log-fuel-detail = Odometer, fuel, and price
log-reading-detail = Just the meters
log-no-vehicles = There are no vehicles you can log for.
log-required = Required
log-last-reading = last { $value } · { $date }
log-skip = Can't read it? Skip with a reason
log-skip-field = Can't give it? Skip with a reason
log-skipped = Skipped
log-skip-reason = Why can't you give it?
log-unskip = Enter it after all
log-override-title = Admin · keep it anyway
log-override-reason = Why keep it?
log-override-note = The event is flagged.
log-yes = Yes
log-no = No
log-section-fuel = Fuel
log-volume = Volume ({ $unit })
log-price = Total price ({ $currency })
log-per-unit = { $price } { $currency } per { $unit }
log-filled-full = Filled to full
log-missed = Missed a fill-up
log-section-when-where = When and where
log-now = Now
log-place-needed = Place needed
log-no-place-summary = No place (reason given)
log-change = Change
log-date = Date
log-time = Time
log-zone = Time zone: { $zone }
log-saved-place = Saved place
log-place-done = Use this place
log-no-place = Can't give a place? Say why
log-no-place-reason = Why is there no place?
log-place-after-all = Give a place after all
log-fold-driving = Driving
log-pct-highway = { $pct }% highway
log-pct-towing = { $pct }% towing
log-pct-hauling = { $pct }% hauling
log-highway = Highway
log-towing = Towing
log-hauling = Hauling
log-towed = Towed
log-fold-more = Trip and readings
log-fold-more-empty = Nothing entered
log-reported-economy = The vehicle's own fuel economy ({ $unit })
log-fold-who = Who and notes
log-who-me = Me
log-notes-none = no notes
log-notes-some = notes
log-done-by = Done by
log-done-by-me = Me (the person logging it)
log-done-by-someone = Someone else
log-their-name = Their name or business
log-their-contact = How to reach them (optional)
log-notes = Notes
log-draft-note = Saved as a draft: it can change for 24 hours, then it locks.
log-save = Save
log-saving = Saving…
log-saved = Saved as a draft.
log-warning-above = { $name } is above its likely range.
log-warning-below = { $name } is below its likely range.
log-warning-kept = The { $name } reading was kept, out of order.
log-done = Done

## An event's page and corrections (milestone 6, step 6h)

screen-event = Event
screen-edit-event = Edit event
screen-correct-event = Correct event
event-not-found = There's no such event, or you can't see it.
log-clash-below = Lower than { $value } { $unit } on { $date }. Check it, or correct that event.
log-clash-above = Higher than { $value } { $unit } on { $date }. Check it, or correct that event.
log-reset = ↺ Reset
# $name (text): the meter; $from: its last reading; $to: the new one as typed; $unit: e.g. "%".
log-reset-done = { $name } was reset: { $from } → { $to } { $unit }.
log-it-was-a-reset = ↺ It was a reset
log-clash-above-earlier = This meter counts down: higher than { $value } { $unit } on { $date }. Check it, or correct that event.
log-clash-below-later = This meter counts down: lower than { $value } { $unit } on { $date }. Check it, or correct that event.
log-correct-note = A correction replaces this event's values; the original stays, linked to it.
log-correct-reason = Why? (required)
event-status-draft = Draft · locks in { $when }
event-status-finalized = Finalized · { $date }
event-locks-hours = { $hours } h
event-locks-minutes = { $minutes } min
event-locks-soon = a moment
event-finalize-now = Finalize now
event-delete = Delete
event-correct = Correct…
event-corrected-by = Corrected on { $date } by { $name }: "{ $reason }"
event-see-correction = See the correction ›
event-corrects = Correction of the { $kind } on { $date }: "{ $reason }"
event-see-original = See the original ›
event-filled-full = filled to full
event-partial-fill = partial fill
event-place-name = Name
event-place = Place
event-no-place = No place
event-from = From
event-source-gps = Device location
event-source-saved = Saved place
event-source-map = Map
event-source-manual = Typed
event-zone = Time zone
event-skipped-because = Skipped: "{ $reason }"
event-reported-economy = Vehicle's own fuel economy
event-done-by-logger = { $name } (logged it)
event-override = Reading kept because
event-typed = typed
event-voice = spoken
event-imported = imported
event-record = Record
event-logged = Logged
event-last-edited = Last edited
event-finalized-row = Finalized
event-by-server = the server, 24 hours after the last edit
event-finalize-title = Finalize this event?
event-finalize-text = Afterwards it can't be edited or deleted; a mistake is fixed with a correction.
event-finalize = Finalize
event-delete-title = Delete this draft?
event-delete-text = It's removed for good, and its readings no longer count.

## Custom fields shown on several kinds (milestone 6, after the checkpoint)

field-no-kinds = Choose at least one kind of event to show it on.
field-shown-on = Shown on
field-kind-note = The kinds of events whose forms show it.
field-required-for-note = Events of a checked kind must fill it in, or skip it with a reason. A field that's turned off is never required.

## The Timeline and Overview (milestone 6, step 6j)

month-long-1 = January
month-long-2 = February
month-long-3 = March
month-long-4 = April
month-long-5 = May
month-long-6 = June
month-long-7 = July
month-long-8 = August
month-long-9 = September
month-long-10 = October
month-long-11 = November
month-long-12 = December
timeline-all = All
timeline-fuel-ups = Fuel-ups
timeline-readings = Readings
timeline-empty = Nothing logged yet. Use + Log to add a fuel-up or reading.
# $first, $last, $total (text, thousands already grouped): "Records 101–200 of 1,342".
timeline-range = Records { $first }–{ $last } of { $total }
timeline-newer = ‹ Newer
timeline-older = Older ›
timeline-per-page = Per page
timeline-jump = Jump to
timeline-pages = Pages
attachment-added-on-save = Added when you save. ✕ removes one first.
attachment-too-many-pages = A scan can have up to 100 pages.
scan-choice = Scan pages
scan-choice-hint = photos of each page, made into one PDF
scan-title = Scan pages
# $count (number): pages taken so far.
scan-count = { $count ->
    [0] No pages yet. Take the first one.
    [one] 1 page
   *[other] { $count } pages
}
# $number (number): the page's place in the document.
scan-page = Page { $number }
scan-remove-page = Remove this page
scan-earlier = Move earlier
scan-later = Move later
scan-first-page = 📷 First page
scan-next-page = 📷 Next page
scan-page-hint = or choose photos of the pages
scan-working = Working…
log-save-place = ☆ Save as a place…
place-save-here-title = Save as a place
# $where (text): "44.0462, -123.0220 · Springfield, OR".
place-save-here-at = Here: { $where }
shop-name-taken = There's already a shop with that name.
shop-bad-contact = A contact can have up to 100 characters.
shop-bad-rate = A labor rate can't be below zero or above 100,000 an hour.
settings-section-labor-rates = Labor rates
shop-rates-intro = For everyone in the household, in { $currency }. A job keeps the labor cost it was saved with.
shop-household-rate = Household rate
shop-household-rate-help = For work done by someone in the household, or by a shop without its own rate. Empty for none.
shop-per-hour = / hour
shop-rate-none = None set
shop-rate-household = household rate
shop-rate-per-hour = { $amount } / h
shops-heading = Shops
shops-empty = No shops yet. Add the places that work on your vehicles, each with its own rate if it differs.
shop-add = + Shop
shop-add-title = Add a shop
shop-edit-title = Change a shop
shop-name = Name
shop-contact = Contact (phone or email)
shop-own-rate = Its own labor rate
shop-own-rate-help = Empty: the household's rate.
shop-remove-title = Remove { $name }?
shop-remove-text = Jobs it did keep its name, contact, and costs.
shop-not-found = That shop isn't there any more.
shop-suggestions = Shops
work-use-rate = ↺ Use the rate
work-rate-fills = { $time } × { $rate } ({ $source }). Change it for a discount or a fixed price; then it stays as typed.
work-rate-would-be = At the rate: { $time } × { $rate } ({ $source }) is { $cost }.
main-photo-make = Make Main Photo
main-photo-clear = Main Photo: click for none
main-photo-is = Main Photo
# $count (number): how many pages the PDF will have.
scan-make-pdf = { $count ->
    [one] Make PDF (1 page)
   *[other] Make PDF ({ $count } pages)
}
timeline-corrected = Corrected
timeline-draft = Draft
panel-fuel-none = No fuel-ups yet.
panel-see-timeline = See all on the Timeline ›
panel-last-activity = Last activity
panel-no-activity = Nothing logged yet.
panel-finalized = Finalized
panel-reading-when = { $date } · { $kind }
panel-no-readings = No readings yet

## A test server beside the main one (decision #062)

instance-tag = { $name } server
instance-tag-note = Not the household's main server: for testing. What's saved here stays here.

## Squawks (milestone 7)

# What the household calls squawks (#064). Every message gets these as { $squawk },
# { $squawks }, { $Squawk }, { $Squawks }, and { $a-squawk }: write "squawk" through them,
# never as a word.
squawk-word-issue = issue
    .plural = issues
    .title = Issue
    .title-plural = Issues
    .article = an issue
squawk-word-squawk = squawk
    .plural = squawks
    .title = Squawk
    .title-plural = Squawks
    .article = a squawk
squawk-word-fault = fault
    .plural = faults
    .title = Fault
    .title-plural = Faults
    .article = a fault
squawk-word-write-up = write-up
    .plural = write-ups
    .title = Write-up
    .title-plural = Write-ups
    .article = a write-up

severity-low = Low
severity-medium = Medium
severity-high = High
squawk-high-not-drivable = Not drivable
usage-drivable = Drivable
usage-towable = Towable
usage-usable = Usable
usage-name-taken = This vehicle already has a usage with that name.
usage-default-stays = The default usage can't be removed; rename it instead.
formula-word-distance = distance
formula-word-volume = volume
formula-word-time = time
formula-word-money = money
# $unit (string): its parts, e.g. "distance² ÷ time".
formula-dim-other = an unusual unit ({ $unit })
formula-dim-number = a number
formula-dim-distance = a distance
formula-dim-volume = a volume
formula-dim-time = a time
formula-dim-money = money
formula-dim-economy = fuel economy
formula-dim-money-per-distance = money per distance
formula-dim-money-per-volume = money per volume
formula-dim-speed = a speed
formula-empty = Type a formula.
# $max (number): the most characters.
formula-too-long = A formula can be at most { $max } characters.
formula-unexpected-end = The formula stops too soon: something is missing at the end.
# $found (string): what was found. $position (number): its character position.
formula-unexpected = "{ $found }" doesn't fit there (character { $position }).
# $position (number): the bracket's character position.
formula-unclosed = The bracket at character { $position } is never closed.
# $name (string): the name typed.
formula-unknown-function = There's no function called "{ $name }".
# $max (number): the most places.
formula-bad-places = round takes a whole number of places, from 0 to { $max }.
# $key (string): the key typed.
formula-unknown-key = There's no value called "{ $key }" here.
# $key (string): the key typed.
formula-not-a-meter = "{ $key }" isn't a meter; this needs one of the vehicle's meters.
# $key (string): the meter's key. $wanted (string): what it should measure.
formula-wrong-meter = "{ $key }" doesn't measure { $wanted }.
formula-hours-need-meter = Say which hour meter: last 50 h on engine-hours.
# $expression (string): the sum, as written. $left, $right (string): what each side measures.
formula-mismatch-plus = { $expression }: can't add { $left } to { $right }.
# $expression (string): the difference, as written. $left, $right (string): what each side measures.
formula-mismatch-minus = { $expression }: can't take { $right } from { $left }.
# $function (string): the function's name.
formula-mixed-values = Everything inside { $function } must measure the same thing.
formula-nested-over = A function over past events can't go inside another one.
# $found (string): what the formula gives. $wanted (string): what the field shows.
formula-wrong-result = This gives { $found }, but the field shows { $wanted }.
# $key (string): the value's key.
formula-no-value-missing = No value: { $key } was left out.
formula-no-value-no-earlier = No value: there's no earlier one to count from.
formula-no-value-nothing-in-over = No value: none of those events has one.
formula-no-value-divide-by-zero = No value: it divides by zero.
formula-no-value-mixed-currencies = No value: it mixes currencies.
formula-no-value-not-positive = No value: this mean needs every value above zero.
formula-no-value-too-few = No value: stddev needs at least two values.
formula-no-value-no-odometer = No value: this vehicle has no odometer to count back on.
formula-no-value-not-finite = No value: the result is too big to show.
formula-no-value-mismatch = No value: the formula no longer fits its values; check it in Setup.
# $by (string): the calculated field or derived meter.
formula-in-use = { $by } uses this in its formula, so it can't be removed, turned off, or taken off that kind of event. Change { $by } first.
formula-worked-out = Worked out from a formula
# $value (string): the value. $unit (string): the unit.
formula-per = { $value } per { $unit }
shows-number = Number
shows-percent = Percentage
shows-distance = Distance
shows-duration = Time
shows-volume = Volume
shows-economy = Fuel economy
shows-money = Money
shows-money-per-distance = Money per distance
shows-money-per-volume = Money per volume
shows-speed = Speed
# $what (string): what it gives, e.g. "money per distance".
formula-check-ok = ✓ Gives { $what }
formula-rows = Worked out
formula-label = Formula
formula-how-to = ⓘ How to write one
formula-functions = Functions ▾
formula-functions-over = Over listed values or past events
formula-functions-listed = Over listed values
formula-functions-one = On one value
formula-values = Values ▾
formula-values-note = Each by its key, which never changes when it's renamed.
formula-values-event = This event
formula-values-meters = Meters
formula-values-since = Since the last one
formula-values-numbers = Numbers with units
formula-over = Over (for a function over past events)
formula-over-cumulative = Cumulative
formula-over-events = Last 5 events
formula-over-months = Last 3 months
formula-over-distance = Last 1,000 mi
formula-over-note = Last X events, days, weeks, months, or mi or km; or last X h on an hour meter. Change the number as you like.
formula-help-title = Writing a formula
formula-help-values = Values by their key; numbers with a unit (33 mi/h).
formula-help-operators = + − × ÷ and ( ). Units must agree: miles + hours is refused.
formula-help-over = A function over past events takes an Over: last 5, last 3 months, last 1000 mi, or cumulative.
formula-help-minus = Keys can have hyphens, so put spaces around a minus: a - b.
formula-help-examples = Examples
formula-help-guide = Full guide →
# $kind (string): the kind of event, in lower case. $value (string): the value, or why there's none.
formula-preview = On the latest { $kind }: { $value }
formula-shows-as = Shows as
formula-field-add-title = Add a calculated field
formula-field-edit-title = Change calculated field
formula-shown-on-note = It shows, worked out, on these kinds of events: never typed, never required.
# $value (string): the value with its unit.
formula-now = Now: { $value }
formula-meter-add-title = Add a derived meter
formula-meter-edit-title = Change derived meter
formula-meter-note = Worked out at every reading of its meters. Interval rules can use it; nobody types its readings, and it can't be reset.
formula-fill-in = Shows once the form is filled in.
# $meters (string): the meters it's worked out from, by name.
formula-derived-from = Worked out from { $meters }
setup-add-calculated = + Calculated field
tire-left = L
tire-right = R
tire-outer = outer
tire-inner = inner
catalogue-already-there = This field is already on the vehicle.
catalogue-not-for-this-kind = These fields are for kinds of events this vehicle doesn't have.
catalogue-bad-axles = Choose 1 to 4 axles.
setup-add-from-catalogue = + From the catalogue
catalogue-title = From the catalogue · { $name }
catalogue-intro = Each is copied into the vehicle's own fields, then can be changed like any field. A set added again gets " B" on its labels and keys.
catalogue-sets = Sets
catalogue-fields = Fields
catalogue-set-summary = { $count } fields: { $fields }
catalogue-set-tires = Tires
catalogue-set-tires-note = Pressure and tread depth for each tire, by axle
catalogue-on-vehicle = already on this vehicle
catalogue-not-for-vehicle = not for this kind of vehicle
catalogue-add = Add
tires-title = Tires · set up
tires-axles = Axles
tires-on-each = Tires on each axle
tires-axle = Axle { $number }
tires-single = 2
tires-dual = 4 (dual)
tires-summary = { $tires } tires · { $fields } fields: a tire pressure and a tread depth for each ({ $names })
tires-units-note = Pressure in your pressure unit; tread depth in 32nds of an inch (miles) or mm (kilometers).
tires-add = Add { $fields } fields
type-name-taken = Another vehicle type has that name.
type-bad-catalogue = Something from the catalogue doesn't fit this kind, or there's too much (20 at most).
type-template-gone = That template has been removed. Choose another, or none.
type-wrong-kind = A vehicle's type must be built on its kind.
nav-vehicle-types = Vehicle types
types-intro = Your household's own types, each built on one of the app's kinds. A change applies only to vehicles added afterwards.
types-kinds-note = The app's own kinds (car, truck, …) stay as they are.
types-add = + Add type
types-none = No vehicle types yet.
types-select-hint = Choose a type to see it, or add one.
types-back = ‹ Vehicle types
types-new = New type
types-built-on = Built on
types-keeps = Keeps its kind's rules: fuel economy, its meters, and what a High { $squawk } means.
types-documents = Documents its default usage ({ $usage }) needs
types-no-documents = None ticked: it needs no documents.
types-starts-with = Starts with
types-meters = Meters (its kind's): { $list }
types-kind-fields = Its kind's fields, on or off
types-from-catalogue = From the catalogue
types-catalogue-none = Nothing from the catalogue.
types-catalogue-pick = + Add from the catalogue…
types-tires = Tires: { $axles } axles, { $tires } tires
types-template = Template (optional)
types-template-none = None
types-template-note = Copied onto each new vehicle, as Apply template does.
types-saved = Saved. Vehicles added from now on start this way.
types-delete = Delete type
types-delete-note = Its vehicles keep everything; they show as their kind.
types-delete-title = Delete the type { $name }?
types-read-only = Only the household's Admin, in Admin mode, changes types.
types-not-found = That type has been removed.
type-icon-car = Car
type-icon-truck = Truck
type-icon-motorcycle = Motorcycle
type-icon-trailer = Trailer
type-icon-generator = Generator
type-icon-tractor = Tractor
type-icon-boat = Boat
type-icon-aircraft = Aircraft
type-icon-bus = Bus
type-icon-van = Van
type-icon-bicycle = Bicycle
type-icon-flag = Race
type-icon-tool = Tool
type-summary = Needs { $documents } · { $meters } meters · { $fields } fields · { $template }
type-needs-nothing = no documents
type-no-template = no template
type-with-template = the { $name } schedule
type-of-kind = { $type } ({ $kind })
add-vehicle-your-types = Your household's types
add-vehicle-left-out = Added. Its template's meter rules without a matching meter were left out: { $list }.
setup-type = Type
setup-type-none = None: shows as its kind
setup-type-note = Changes only its label and icon, never its records or fields.
setup-add-derived = + Derived meter
# $key (string): the key typed.
formula-not-an-input = "{ $key }" can't be used here: a derived meter is worked out from typed meters that aren't reset.
# $key (string): the field's key.
formula-calculated = "{ $key }" is worked out itself, so another formula can't use it.
formula-not-in-derived = A derived meter uses only meters, numbers, + − × ÷, and functions of listed values: no round, past events, or since the last.
meter-derived-resettable = A derived meter is worked out, so it can't be reset or required on events.
usage-section = Usages
usage-section-note = Each usage stands alone: what's ticked "Needed for" it, and the documents it needs. A High issue blocks every usage.
usage-add = + Usage
usage-add-title = Add a usage
usage-edit-title = Change usage
usage-name = Name
# $name (string): the default usage's name.
usage-default-name = { $name } (default)
usage-documents = Documents it needs
usage-documents-note = Not on file, or expired: not ready. Due for renewal soon: a warning.
# $documents (string): document type names, joined by commas.
usage-needs = Needs: { $documents }
usage-needs-nothing = Needs no documents
usage-needed-for = Needed for
usage-needed-for-note = Overdue: not ready for the ticked usages. Due soon: a warning.
# $usage (string): the usage's name, in lower case.
ready-not = Not { $usage }
# $usage (string): the answer ("Not drivable"). $count (number): how many things block it.
ready-blockers = { $usage }: { $count ->
        [one] { $count } thing
       *[other] { $count } things
    }
# $usage (string): the usage's name. $count (number): how many things are due soon.
ready-warnings = { $usage }: { $count ->
        [one] { $count } thing due soon
       *[other] { $count } things due soon
    }
ready-document-current = On file and current
ready-document-missing = Not on file
ready-issue-open = High issue, open
ready-issue-deferred = High issue, deferred
ready-nothing = Nothing is needed for this usage yet.
ready-advisory = Advice only: nothing stops anyone from using it.
squawk-high-not-towable = Not towable
squawk-high-out-of-service = Out of service
squawk-status-open = Open
squawk-status-deferred = Deferred
squawk-status-resolved = Resolved
squawk-description-empty = Say what's wrong.
squawk-description-too-long = That's too long: at most 500 characters.
squawk-undispositioned = Sign off or defer every open { $squawk } before finishing.
squawk-dispositioned-twice = That { $squawk } is already signed off or deferred in this job.
squawk-not-outstanding = That { $squawk } isn't open or deferred on this vehicle.
squawk-dispositioned = A job has already dealt with this { $squawk }, so it can't be changed or removed. Its next job can change its severity.

## Requirements and bundles (milestone 7)

requirement-bad-interval = Enter how far apart, more than zero.
requirement-bad-calendar = Every 1 to 3,650 days, 1 to 600 months, or 1 to 50 years.
requirement-repeated-rule = Each meter, the calendar, and the fixed date can be used once.
bundle-empty = Choose at least one requirement.
bundle-bad-item = Each requirement can be in a bundle once.
requirement-in-use = A task has done this requirement, so it stays in the history. Turn it off instead.
schedule-not-empty = This vehicle already has requirements. Copying a schedule is only for a vehicle without one.

## Maintenance and repairs (milestone 7)

event-bad-part = Each part needs a description (up to 100 characters), a part number of up to 40, and a quantity above zero.
event-bad-next-due = A next due date goes only on a task whose requirement has a fixed-date rule, and must be a date.
event-no-end-readings = This task resets from the end readings, but there aren't any. Add them, or reset from the start.
event-end-below-start = The end reading goes the wrong way from the start one. Check them.
event-not-resettable = This meter can't be reset. Untick Reset, or make the meter resettable in Setup.
event-finish-jobs-only = Only maintenance and repairs are finished; fuel-ups and readings are finalized.

## Attachments (milestone 8)

photo-size-small = Small
photo-size-medium = Medium
photo-size-large = Large
attachment-empty = That file is empty.
attachment-not-a-photo = That isn't a photo the app can use. Choose a JPEG or PNG.
attachment-heic-photo = That photo is HEIC, which the server doesn't convert. Take it again from the app, or export it as a JPEG first.
attachment-not-a-pdf = Documents must be PDFs.
attachment-photo-too-large = Photos can be up to 25 MB.
attachment-pdf-too-large = PDFs can be up to 50 MB. Very large manuals are better kept outside the app.
attachment-name-empty = The file needs a name.
attachment-name-too-long = The file's name can be up to 255 characters.
attachment-no-size = That photo has no size. Try it again.
attachment-camera-too-long = The camera's name can be up to 100 characters.
attachment-no-pages = That PDF has no pages.
attachment-no-such-task = That task isn't on this job any more.
attachment-bad-hash = That file reference isn't valid.
attachment-unchangeable = An attachment can only be moved; to change it, remove it and add it again.
attachment-unreadable = The server couldn't read that file. It may be damaged, or a PDF it can't open.
attachment-not-uploaded = That file didn't finish uploading. Try adding it again.
attachment-originals-off = The household doesn't keep originals. The photo can still be added without it.
attachment-event-finished = This event is locked. To add or remove a photo or document, correct the event.

## Official documents and license plates (milestone 8)

document-status-current = Current
document-status-expired = Expired
document-status-superseded = Superseded
document-unknown-type = That isn't a document type the app knows.
document-number-too-long = The number can be up to 60 characters.
document-issuer-too-long = Who issued it can be up to 100 characters.
document-expires-before-issued = It expires before it's issued or takes effect. Check the dates.
document-renews-unknown = The document it renews isn't on this vehicle.
document-renews-other-type = A renewal must be of the same kind of document.
document-already-renewed = That document has already been renewed. Renew the newer one.
document-plate-not-registration = Only a registration is linked to a license plate.
document-unknown-plate = That license plate isn't on this vehicle.
document-renewed = A later document renews this one. Remove that one first.
plate-number-empty = Enter the plate's number.
plate-number-too-long = A plate's number can be up to 15 characters.
plate-needs-authority = Choose who issued the plate: a state or province, a country, or another authority.
plate-unknown-region = Choose a state or province from the list.
plate-bad-country = A country is its two-letter code, e.g. DE or MX.
plate-authority-empty = Enter who issued the plate.
plate-authority-too-long = Who issued the plate can be up to 60 characters.
plate-retired-before-issued = The plate stopped being used before it was issued. Check the dates.
plate-in-use = A registration names this plate. Change the registration first.

## Photos in the app (milestone 8, step 8e)

attachment-photos = Photos
attachment-task-photos = Photos & receipts
attachment-job-photos = Photos & documents
attachment-add = + Add
attachment-pdf-pages = { $count } p
attachment-add-photo = Add a photo or PDF
attachment-take-photo = Take photo
attachment-take-photo-hint = with the camera
attachment-choose-photos = Choose photos
attachment-choose-photos-hint = from the library or files
attachment-size = Size
photo-size-small-hint = about 200 KB
photo-size-medium-hint = about 1 MB
photo-size-large-hint = about 3 MB
attachment-keep-original = Keep the original, as taken
attachment-adding = Adding { $at } of { $count }…
attachment-failed = { $name }: { $problem }
attachment-done = Done
attachment-back = ‹ Back
attachment-position = { $at } of { $count }
attachment-previous = Previous
attachment-next = Next
attachment-taken = Taken
attachment-where = Where
attachment-camera = Camera
attachment-added = Added
attachment-added-by = by { $name }, { $date }
attachment-download = Download
attachment-original = Original
attachment-saved-to = Saved to { $path }
attachment-remove-title = Remove this file?
attachment-remove-text = It's taken off here for everyone.
attachment-save-first = Save the draft to add photos and documents.
attachment-save-to-see = Save the draft to see each task's photos again.

## PDFs in the app (milestone 8, step 8f)

attachment-choose-pdf = Choose a PDF
attachment-choose-pdf-hint = up to 50 MB
attachment-pdf-cant-show = This PDF can't be shown here. Use Download to open it.
pdf-zoom-out = Zoom out
pdf-zoom-in = Zoom in
pdf-fit-width = Fit width
pdf-page-of = Page { $page } of { $pages }
work-copy-from-start = Copy from start
log-currency = Currency

## The Documents tab and license plates (milestone 8, step 8h)

documents-heading = Documents
documents-add = + Document
documents-none = No documents yet.
documents-plate = License plate
documents-add-plate = + Plate
documents-no-plate = No license plate yet.
documents-earlier-plates = Earlier plates ({ $count })
documents-earlier = Earlier ({ $count })
documents-scans = Scans
documents-renew = Renew…
document-issued-on = Issued { $date }
document-effective-on = from { $date }
document-expires-on = expires { $date }
document-never-expires = never expires
document-expires-in = { $days ->
    [0] Expires today
    [one] Expires tomorrow
   *[other] Expires in { $days } days
}
documents-add-title = Add a document
documents-renew-title = Renew it
documents-edit-title = Change the document
documents-renew-note = A new record; the one it renews is kept as history.
documents-renew-save = Renew
documents-type = Type
documents-type-other = Other…
documents-type-name = What it is
documents-name-needed = Say what it is (up to 40 characters).
documents-number = Number
documents-issuer = Issued by
documents-issued = Issued
documents-effective = Takes effect
documents-expires = Expires
documents-plate-field = License plate
documents-no-plate-choice = None
documents-plate-follows = Its expiry becomes the plate's.
documents-notes = Notes
documents-notes-too-long = The notes are too long.
documents-scans-later = Add its scans on its card once it's saved.
documents-remove-confirm = Remove it and its scans
plate-add-title = Add a license plate
plate-edit-title = Change the license plate
plate-add-note = The current plate (if any) is retired on the new one's issue date, or today.
plate-add-save = Add
plate-issued-by = Issued by
plate-by-state = US state
plate-by-province = Canadian province
plate-by-country = Country
plate-by-other = Other
plate-state = State
plate-province = Province or territory
plate-country = Country
plate-other = Who issued it
plate-number = Number
plate-retired = Retired
plate-since = Since { $date }
plate-retired-on = until { $date }
plate-from-registration = (from its registration)
plate-choose-authority = Choose who issued it: Edit.
plate-remove-confirm = Remove the plate
attachment-kind-photo = Photo
attachment-kind-pdf = PDF
attachment-unpick = Don't add { $name }
attachment-add-count = { $count ->
    [0] Add
    [one] Add 1
   *[other] Add { $count }
}
event-kind-acquired = Acquired
event-kind-sent-out = Sent out
event-kind-returned = Returned
event-kind-disposed = Disposed
disposal-sold = Sold
disposal-traded-in = Traded in
disposal-scrapped = Scrapped
disposal-given-away = Given away
disposal-returned-to-vendor = Returned to the vendor
disposal-lost-or-stolen = Lost or stolen
disposal-other = Other
event-text-too-long = That's too long: at most 100 characters.
event-already-acquired = This vehicle's acquisition is already recorded: correct that one instead.
event-already-out = This vehicle is already out for service: record its return first.
event-not-out = This vehicle isn't out for service.
event-return-before-send-out = It can't come back before it went out: check the date.
event-already-disposed = This vehicle's disposal is already recorded: correct that one instead.
event-vehicle-disposed = This vehicle was disposed of: its records can be corrected, but nothing new added.
inbox-overdue-return = A vehicle isn't back yet
inbox-overdue-return-body = { $vehicle } was expected back from service on { $date }. Record its return when it's back, or update the expected date.
inbox-failed-unlocks = Wrong PINs on a computer
inbox-failed-unlocks-body = { $count ->
    [one] Someone typed 1 wrong PIN or password on { $device } ({ $person }) before it was unlocked.
   *[other] Someone typed { $count } wrong PINs or passwords on { $device } ({ $person }) before it was unlocked.
} If that wasn't { $person }, sign the computer out from their Devices on the Users screen.
inbox-clock-ahead = A computer's clock is wrong
inbox-clock-ahead-body = { $minutes ->
    [one] { $device } ({ $person }) sent changes with its clock 1 minute ahead of the server's.
   *[other] { $device } ({ $person }) sent changes with its clock { $minutes } minutes ahead of the server's.
} Its changes wait on that computer until its date and time are set right.
inbox-open-send-out = Open the send-out
inbox-due-coming-up = Coming up
inbox-due-soon = Due soon
inbox-due-very-soon = Due very soon
inbox-due-overdue = ⚠ Overdue
inbox-due-body = { $vehicle }: { $item }, { $left }.
inbox-open-maintenance = Open on Maintenance
inbox-open-documents = Open Documents
messages-bad-snooze = That snooze can't be used. Choose days, or an amount more than zero.
inbox-due-row = { $vehicle }: { $item }
snooze = Snooze
snooze-day = 1 day
snooze-week = 1 week
snooze-month = 1 month
snooze-either = { $time } or { $amount }
snooze-custom = Custom…
snooze-days = Days
snooze-or-meter = or { $meter } ({ $unit })
snooze-hint = Leave either empty; whichever comes first.
messages-snoozed-until = Snoozed until { $when }
messages-wake = Wake now
messages-filter-snoozed = Snoozed ({ $count })
due-filter-all = All
due-filter-overdue = Overdue
due-filter-soon = Due soon
due-filter-squawks = { $Squawks }
due-list-squawks = Open { $squawks }
due-list-nothing = Nothing coming due, and no open { $squawks }.
fleet-label = Across your vehicles
fleet-overdue = ⚠ Overdue
fleet-due-soon = Due soon
fleet-squawks = Open { $squawks }
fleet-synced = Synced
fleet-waiting = { $count ->
    [one] change waiting to send
   *[other] changes waiting to send
}
fleet-offline = Offline
setting-due-messages = Messages about what's due start at
setting-due-messages-note = Overdue always sends one. Each person can snooze or dismiss their own.
log-acquired-detail = When and how it was bought
log-sent-out-detail = Off to a shop, and when it's due back
log-returned-detail = Back from the shop, and what it cost
log-disposed-detail = Sold, traded in, or scrapped
reference-bad-title = Enter a title (at most 200 characters).
reference-authority-too-long = That's too long: at most 100 characters.
reference-bad-number = Each number needs 1 to 60 characters, once each, and at most 10 numbers.
reference-revision-too-long = That's too long: at most 20 characters.
reference-bad-url = Enter a web address starting with https:// or http://.
reference-effective-before-issued = It can't take effect before it was issued.
compliance-recurring = Recurring
compliance-one-time = One-time
requirement-status-open = Open
requirement-status-complied = Complied
requirement-status-active = Active
requirement-status-not-applicable = Not applicable
requirement-one-time-with-rules = A one-time item has no intervals: remove them, or make it recurring.
reference-recall = Recall
reference-technical-service-bulletin = Technical service bulletin
reference-service-bulletin = Service bulletin
reference-manufacturer-schedule = Manufacturer schedule
reference-other = Other
log-vehicle-life = Vehicle's life
life-shop-required = Enter the shop or vendor.
life-method-required = Choose how it was disposed of.
life-acquired-heading = How it was acquired
life-sent-out-heading = The shop
life-returned-heading = The return
life-disposed-heading = How it left
life-from = From (seller, dealer, or maker)
life-shop = Shop or vendor
life-to-whom = To whom
life-contact = Contact (optional)
life-price = Price
life-estimate = Estimate
life-final-cost = Final cost
life-work-order = Work order
life-due-back = Due back
life-shipping = Shipping
life-shipping-none = none
life-carrier = Carrier
life-tracking = Tracking number
life-how = How
life-choose = Choose…
life-withdrawn = It wasn't disposed of after all
life-return-of = Back from { $shop }, sent out on { $date }.
life-dispose-title = Record this disposal?
life-dispose-text = Once it's finalized (in 24 hours, or with Finalize now), nothing new can be added to { $vehicle }. Its records can still be corrected.
life-dispose-save = Save the disposal
life-summary-from = From { $name }
life-summary-to = to { $name }
life-summary-due-back = due back { $date }
life-banner-out-due = At { $shop } since { $date } · due back { $due }
life-banner-out = At { $shop } since { $date }
life-banner-late = { $days ->
    [one] 1 day late
   *[other] { $days } days late
}
life-banner-estimate = estimate { $amount }
life-record-return = Record return
life-banner-disposed = { $how } on { $date }
life-banner-disposed-to = { $how } on { $date } to { $name }
life-banner-disposed-note = Its records stay; they can be corrected, but nothing new can be added.
life-banner-disposed-draft = The disposal is a draft: it can still be changed or deleted. Once it's finalized, nothing new can be added.
compliance-applies = Applies
compliance-under-review = Under review
compliance-reason-needed = Say why it doesn't apply (at most 500 characters).
compliance-how = How it's complied with
compliance-one-time-note = Done once: the first finished job that does it closes it.
compliance-applies-question = Does it apply?
compliance-why-not = Why not?
reference-not-found = That recall or bulletin isn't there any more.
reference-in-use = A vehicle's requirement still links it: remove those first.
reference-picker-title = Recall or bulletin
reference-new-one = + A new one
reference-already-recorded = Already recorded (on your other vehicles)
reference-new-title = New recall or bulletin
reference-edit-title = Change recall or bulletin
reference-type = Type
reference-authority = Issued by
reference-numbers = Reference numbers
reference-scheme = Whose
reference-number = Number
reference-another-number = + Another number
reference-title = Title
reference-description = What it's about
reference-remedy = Remedy
reference-issued = Issued
reference-effective = Effective
reference-revision = Revision
reference-applies-to = Applies to, as written
reference-url = Web address
reference-on-vehicle = On { $vehicle }
reference-requirement-name = Requirement's name
reference-attach-note = Add its PDF or photos once it's saved, from its own view: they're the recall's, so every vehicle that links it sees them.
reference-edit-note = A change shows on every vehicle that links it.
reference-linked = On
reference-open-source = Open the source ↗
reference-files = PDF and photos
reference-open = Open it
schedule-add-reference = + Recall or bulletin
action-close = Close
setup-view-only-role = You can see this vehicle's setup; a Maintainer or the Admin changes it.
schedule-filter-open = Open
schedule-filter-all = All
schedule-your-schedule = Your schedule
schedule-recalls = Recalls and bulletins
schedule-hidden = { $count ->
    [one] 1 more is done, not applicable, or turned off: All shows it.
   *[other] { $count } more are done, not applicable, or turned off: All shows them.
}
work-from-recall = + From a recall or bulletin…
