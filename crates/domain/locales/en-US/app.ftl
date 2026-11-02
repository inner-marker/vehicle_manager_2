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
settings-you-help = Follow the household default, or choose your own. Changes save at once.
settings-household-help = For everyone who hasn't chosen their own.
settings-household-admin-only = Switch to Admin mode to change the household defaults.
settings-use-household = Use household default
settings-this-device = THIS DEVICE
settings-theme = Theme
setting-distance = Distance
setting-volume = Fuel volume
setting-economy = Fuel economy
setting-pressure = Pressure
setting-date-format = Date format
setting-currency = Currency
setting-language = Language
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

settings-sign-in = SIGN-IN
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
# The job's name is in the message's details.
inbox-job-failed = A scheduled job failed
inbox-job-skipped = A scheduled job was skipped
inbox-job-body = See the details below and the server's log.
inbox-snapshot-body = See the details below and the server's log. Until this is fixed, there's no new backup of the database.
inbox-quick-check-body = The hourly check found a problem in the database, so the server is refusing changes. See the details below and the server's log.
inbox-server-unhealthy = The server isn't accepting changes
inbox-place-lookup-unavailable = Place lookup isn't available
inbox-place-lookup-body = The server couldn't open its place data, so it can't suggest a city, state, and country from a location. Saved places and typing a place still work. Restarting the server, or updating it, usually fixes this; the details below say what went wrong.
inbox-database-unavailable = The server couldn't open its database. You can still look around, but nothing can be saved until it's fixed.
inbox-integrity-check-failed = The server found a problem in its database. You can still look around, but nothing can be saved until it's fixed.
inbox-open-diagnostics = Open Diagnostics
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
event-kind-fuel = Fuel-up
event-kind-reading = Reading
event-kind-maintenance = Maintenance
event-kind-repair = Repair
unit-hours = hours
label-empty = Enter a name.
label-too-long = That's too long: at most 40 characters.
meter-wrong-unit = That unit doesn't measure this meter (e.g. hours for a distance).
meter-too-precise = A meter can show at most 2 decimal places.
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
vehicles-card-coming = Readings and due items arrive later.
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
panel-ready-coming = Arrives with what's due (Phase 5).
panel-due = Coming due
panel-due-coming = Next services (Phase 5).
panel-squawks = Open squawks
panel-squawks-coming = Squawks arrive in milestone 7.
panel-fuel = Latest fuel-ups
panel-fuel-coming = Fuel-ups arrive in milestone 6.
panel-meters = Meters
meters-none = No meters yet.
meters-readings-coming = Readings appear here from milestone 6.
# $kinds (text): the event kinds, joined with commas.
meter-required-for = Required for: { $kinds }
meter-required-none = Not required for any event.
precision-0 = whole
precision-1 = tenths
precision-2 = hundredths
dimension-distance = Distance
dimension-duration = Duration
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
field-applies-to = On
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
field-kind-fixed = A field's type and event can't change once it exists.
kind-text = Text
kind-number = Number
kind-date = Date
kind-yes-no = Yes/No
kind-choice = Choice list
kind-distance = Distance
kind-duration = Duration
kind-volume = Volume
kind-fuel-economy = Fuel economy
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
delete-type = Type { $name } to confirm.
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
about-place-data = Place data
about-place-data-unavailable = Not available
about-credits = Credits
about-credits-default = Contains GeoNames data (CC BY 4.0, geonames.org). Boundaries: Natural Earth (public domain, naturalearthdata.com).

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
field-required = Required
field-required-note = Every event of its kind must fill it in, or skip it with a reason. A field that's turned off is never required.
field-required-badge = Required

## + Log and the log forms (milestone 6, step 6g)

log-title = Log
log-vehicle = Vehicle
log-last-time = { $name } · same as last time
log-fuel-detail = Odometer, fuel, and price
log-reading-detail = Just the meters
log-later = Coming in a later milestone
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
