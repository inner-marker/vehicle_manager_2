//! Typed message IDs, so code can only refer to messages that are declared.

/// The ID of one message in the Fluent catalog, e.g. `nav-garage`.
///
/// Only the constants in [`messages`] exist, so a mistyped ID is a compile error,
/// and a test checks every constant has an English message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct MessageId(&'static str);

impl MessageId {
    /// The message's ID as written in the `.ftl` file.
    pub const fn as_str(self) -> &'static str {
        self.0
    }

    /// An ID outside [`messages`], for tests of undeclared or ad-hoc catalogs.
    #[cfg(test)]
    pub(crate) const fn for_test(id: &'static str) -> Self {
        Self(id)
    }
}

/// Declares each message constant and the [`messages::ALL`] list in one place,
/// so a constant can't be added without being covered by the catalog test.
macro_rules! define_messages {
    ($($(#[$doc:meta])* $name:ident => $id:literal,)+) => {
        /// Every message the app can show. Names mirror the `.ftl` IDs.
        pub mod messages {
            use super::MessageId;

            $(
                $(#[$doc])*
                pub const $name: MessageId = MessageId($id);
            )+

            /// Every declared message, in declaration order.
            pub const ALL: &[MessageId] = &[$($name),+];
        }
    };
}

define_messages! {
    /// Navigation: the home screen with one card per vehicle.
    NAV_GARAGE => "nav-garage",
    /// Navigation: maintenance coming due, and open squawks.
    NAV_DUE_AND_SQUAWKS => "nav-due-and-squawks",
    /// Navigation: the in-app message center.
    NAV_MESSAGES => "nav-messages",
    /// Navigation: every event across vehicles, newest first.
    NAV_TIMELINE => "nav-timeline",
    /// Navigation: history reports and exports.
    NAV_REPORTS => "nav-reports",
    /// Navigation: managing vehicles.
    NAV_VEHICLES => "nav-vehicles",
    /// Navigation: app settings.
    NAV_SETTINGS => "nav-settings",
    /// Navigation: the debug log viewer and export.
    NAV_DIAGNOSTICS => "nav-diagnostics",
    /// "N open squawks"; takes `count` (number).
    OPEN_SQUAWK_COUNT => "open-squawk-count",

    /// Screen title: one vehicle's overview.
    SCREEN_VEHICLE_DETAIL => "screen-vehicle-detail",
    /// Screen title: logging a new event.
    SCREEN_LOG_EVENT => "screen-log-event",
    /// Screen title: no screen at this address.
    SCREEN_NOT_FOUND => "screen-not-found",
    /// Not-found screen: link back to the home screen.
    NOT_FOUND_BACK => "not-found-back",
    /// Placeholder body for screens not built yet.
    PLACEHOLDER_COMING_SOON => "placeholder-coming-soon",

    /// Button: start logging an event (fuel-up, reading, squawk, maintenance).
    ACTION_LOG => "action-log",
    /// Button: reload what's shown.
    ACTION_REFRESH => "action-refresh",
    /// Button label for screen readers: open the navigation menu.
    MENU_OPEN => "menu-open",
    /// Button label for screen readers: close the navigation menu.
    MENU_CLOSE => "menu-close",
    /// Top bar: sync status before sync exists.
    SYNC_STATUS_UNKNOWN => "sync-status-unknown",

    /// Theme switcher label.
    THEME_LABEL => "theme-label",
    /// Theme option: light.
    THEME_LIGHT => "theme-light",
    /// Theme option: dark.
    THEME_DARK => "theme-dark",
    /// Theme option: follow the operating system.
    THEME_SYSTEM => "theme-system",

    /// Log viewer: minimum level filter label.
    LOG_VIEWER_MIN_LEVEL => "log-viewer-min-level",
    /// Log viewer: area filter label.
    LOG_VIEWER_AREA => "log-viewer-area",
    /// Log viewer: area filter option for every area.
    LOG_VIEWER_ALL_AREAS => "log-viewer-all-areas",
    /// Log viewer: column heading for the entry time.
    LOG_VIEWER_TIME => "log-viewer-time",
    /// Log viewer: column heading for the level.
    LOG_VIEWER_LEVEL => "log-viewer-level",
    /// Log viewer: column heading for the message.
    LOG_VIEWER_MESSAGE => "log-viewer-message",
    /// Log viewer: how many entries are shown; takes `count` (number).
    LOG_VIEWER_ENTRY_COUNT => "log-viewer-entry-count",
    /// Log viewer: nothing matches the filters.
    LOG_VIEWER_EMPTY => "log-viewer-empty",
    /// Log viewer: logging didn't start, so there is nothing to show.
    LOG_VIEWER_UNAVAILABLE => "log-viewer-unavailable",

    /// Log area name: app lifecycle.
    LOG_AREA_APP => "log-area-app",
    /// Log area name: user interface.
    LOG_AREA_UI => "log-area-ui",
    /// Log area name: sync.
    LOG_AREA_SYNC => "log-area-sync",
    /// Log area name: database.
    LOG_AREA_DATABASE => "log-area-database",
    /// Log area name: authentication and permissions.
    LOG_AREA_AUTH => "log-area-auth",

    /// Log level name: error.
    LOG_LEVEL_ERROR => "log-level-error",
    /// Log level name: warning.
    LOG_LEVEL_WARN => "log-level-warn",
    /// Log level name: info.
    LOG_LEVEL_INFO => "log-level-info",
    /// Log level name: debug.
    LOG_LEVEL_DEBUG => "log-level-debug",
    /// Log level name: trace.
    LOG_LEVEL_TRACE => "log-level-trace",

    /// Diagnostics: heading of the connection test panel.
    CONNECTION_TITLE => "connection-title",
    /// Connection panel: button that runs the version, sign-in, and health checks.
    CONNECTION_RUN_CHECKS => "connection-run-checks",
    /// Connection panel: shown while the checks run.
    CONNECTION_RUNNING => "connection-running",
    /// Connection panel: name of the API version check.
    CONNECTION_VERSION_CHECK => "connection-version-check",
    /// Connection panel: name of the signed-in check.
    CONNECTION_AUTH_CHECK => "connection-auth-check",
    /// Connection panel: a check passed.
    CONNECTION_PASS => "connection-pass",
    /// Connection panel: a check failed.
    CONNECTION_FAIL => "connection-fail",
    /// Connection panel: a check hasn't run (yet, or because an earlier one failed).
    CONNECTION_NOT_CHECKED => "connection-not-checked",
    /// Connection panel: what the server reported; takes `app_version` (text),
    /// `api_version` and `min_api_version` (numbers).
    CONNECTION_SERVER_INFO => "connection-server-info",
    /// Connection panel: ping time; takes `millis` (number).
    CONNECTION_ROUND_TRIP => "connection-round-trip",
    /// Connection panel: the request never got an answer from the server.
    CONNECTION_UNREACHABLE => "connection-unreachable",
    /// Connection panel: the server answered with something unexpected.
    CONNECTION_UNEXPECTED => "connection-unexpected",
    /// Connection panel: name of the server health check.
    CONNECTION_HEALTH_CHECK => "connection-health-check",
    /// Connection panel: the server reports a problem with its database.
    CONNECTION_SERVER_UNHEALTHY => "connection-server-unhealthy",
    /// Connection panel (desktop): label of the server address field.
    CONNECTION_SERVER_ADDRESS => "connection-server-address",
    /// Connection panel (desktop): button that saves the server address.
    CONNECTION_SAVE_ADDRESS => "connection-save-address",
    /// Connection panel (desktop): the address this run uses; takes `address` (text).
    CONNECTION_ADDRESS_IN_USE => "connection-address-in-use",
    /// Connection panel (desktop): the address was saved and applies after a restart.
    CONNECTION_ADDRESS_SAVED => "connection-address-saved",
    /// Connection panel (desktop): the settings file couldn't be written.
    CONNECTION_ADDRESS_SAVE_FAILED => "connection-address-save-failed",

    /// Server address rule: nothing was entered.
    SERVER_ADDRESS_EMPTY => "server-address-empty",
    /// Server address rule: must start with https:// (or http:// for this computer).
    SERVER_ADDRESS_UNSUPPORTED_SCHEME => "server-address-unsupported-scheme",
    /// Server address rule: http:// is only for this computer.
    SERVER_ADDRESS_INSECURE_REMOTE => "server-address-insecure-remote",
    /// Server address rule: no server name.
    SERVER_ADDRESS_MISSING_HOST => "server-address-missing-host",
    /// Server address rule: the server name has characters it can't have.
    SERVER_ADDRESS_INVALID_HOST => "server-address-invalid-host",
    /// Server address rule: the port isn't 1–65535.
    SERVER_ADDRESS_INVALID_PORT => "server-address-invalid-port",
    /// Server address rule: a path, query, or login after the server name.
    SERVER_ADDRESS_EXTRA_PARTS => "server-address-extra-parts",

    /// Username rule: 3–32 characters.
    USERNAME_WRONG_LENGTH => "username-wrong-length",
    /// Username rule: letters, digits, `.`, `-`, `_` only.
    USERNAME_INVALID_CHARACTER => "username-invalid-character",
    /// Username rule: must start with a letter or digit.
    USERNAME_MUST_START_WITH_LETTER_OR_DIGIT => "username-must-start-with-letter-or-digit",
    /// Password rule: at least 12 characters.
    PASSWORD_TOO_SHORT => "password-too-short",
    /// Password rule: at most 256 characters.
    PASSWORD_TOO_LONG => "password-too-long",
    /// Display name rule: can't be empty.
    DISPLAY_NAME_EMPTY => "display-name-empty",
    /// Display name rule: at most 64 characters.
    DISPLAY_NAME_TOO_LONG => "display-name-too-long",

    /// Refusal reason: not signed in.
    API_ERROR_MISSING_CREDENTIAL => "api-error-missing-credential",
    /// Refusal reason: the session isn't valid any more.
    API_ERROR_INVALID_CREDENTIAL => "api-error-invalid-credential",
    /// Refusal reason: the server can't check sessions.
    API_ERROR_AUTH_NOT_CONFIGURED => "api-error-auth-not-configured",
    /// Refusal reason: the app sent no API version.
    API_ERROR_MISSING_API_VERSION => "api-error-missing-api-version",
    /// Refusal reason: the API version couldn't be read.
    API_ERROR_MALFORMED_API_VERSION => "api-error-malformed-api-version",
    /// Refusal reason: the app is older than the server supports.
    API_ERROR_CLIENT_TOO_OLD => "api-error-client-too-old",
    /// Refusal reason: the app is newer than the server.
    API_ERROR_CLIENT_TOO_NEW => "api-error-client-too-new",

    /// Shown while the app checks who's signed in.
    LOADING => "loading",
    /// A request failed for an unexpected reason.
    SOMETHING_WENT_WRONG => "something-went-wrong",
    /// Button: cancel a dialog.
    ACTION_CANCEL => "action-cancel",
    /// Sign-in screen: heading.
    SIGN_IN_TITLE => "sign-in-title",
    /// Sign-in screen: line under the heading.
    SIGN_IN_SUBTITLE => "sign-in-subtitle",
    /// Form label: username.
    FIELD_USERNAME => "field-username",
    /// Form label: password.
    FIELD_PASSWORD => "field-password",
    /// Sign-in screen: the button.
    SIGN_IN_BUTTON => "sign-in-button",
    /// Sign-in screen: shown on the button while signing in.
    SIGN_IN_BUSY => "sign-in-busy",
    /// Sign-in error: wrong username or password (deliberately not saying which).
    SIGN_IN_WRONG => "sign-in-wrong",
    /// Too many failed attempts; takes `minutes` (number).
    TOO_MANY_ATTEMPTS => "too-many-attempts",
    /// Sign-in error: the server couldn't check.
    SIGN_IN_UNAVAILABLE => "sign-in-unavailable",
    /// Setup: which step; takes `step` (number).
    SETUP_STEP => "setup-step",
    /// Setup step 1: heading.
    SETUP_TITLE => "setup-title",
    /// Setup step 1: line under the heading.
    SETUP_SUBTITLE => "setup-subtitle",
    /// Setup step 1: where to find the code.
    SETUP_CODE_HELP => "setup-code-help",
    /// Form label: the setup code.
    FIELD_SETUP_CODE => "field-setup-code",
    /// Setup step 1: the button.
    SETUP_NEXT => "setup-next",
    /// Setup step 1: shown on the button while checking.
    SETUP_CHECKING => "setup-checking",
    /// Setup error: wrong code.
    SETUP_WRONG_CODE => "setup-wrong-code",
    /// Setup error: an Admin already exists.
    SETUP_ALREADY => "setup-already",
    /// Setup step 2: heading.
    SETUP_ACCOUNT_TITLE => "setup-account-title",
    /// Setup step 2: confirms step 1.
    SETUP_CODE_ACCEPTED => "setup-code-accepted",
    /// Form label: the person's name.
    FIELD_DISPLAY_NAME => "field-display-name",
    /// Form label: a password being chosen.
    FIELD_NEW_PASSWORD => "field-new-password",
    /// Form label: the chosen password, again.
    FIELD_PASSWORD_AGAIN => "field-password-again",
    /// Form error: the two passwords differ.
    PASSWORDS_DIFFER => "passwords-differ",
    /// Setup step 2: the button.
    SETUP_CREATE => "setup-create",
    /// Setup step 2: shown on the button while working.
    SETUP_CREATING => "setup-creating",
    /// Button: go back a step.
    ACTION_BACK => "action-back",
    /// Active mode: Driver.
    MODE_DRIVER => "mode-driver",
    /// Active mode: Maintainer.
    MODE_MAINTAINER => "mode-maintainer",
    /// Active mode: Admin.
    MODE_ADMIN => "mode-admin",
    /// Mode chip for a Viewer-only user.
    MODE_READ_ONLY => "mode-read-only",
    /// Mode menu: Admin asks for the password.
    MODE_NEEDS_PASSWORD => "mode-needs-password",
    /// Accessible label of the mode chip.
    MODE_MENU_LABEL => "mode-menu-label",
    /// Admin password dialog: heading.
    ADMIN_DIALOG_TITLE => "admin-dialog-title",
    /// Admin password dialog: explanation.
    ADMIN_DIALOG_TEXT => "admin-dialog-text",
    /// Admin password dialog: the button.
    ADMIN_DIALOG_CONFIRM => "admin-dialog-confirm",
    /// Error: the password typed isn't right.
    WRONG_PASSWORD => "wrong-password",
    /// Admin banner: the label.
    ADMIN_BANNER_LABEL => "admin-banner-label",
    /// Admin banner: the explanation.
    ADMIN_BANNER_TEXT => "admin-banner-text",
    /// Admin banner: button to leave Admin mode.
    ADMIN_BANNER_LEAVE => "admin-banner-leave",
    /// Admin banner on phones: one short line; takes `minutes` (number).
    ADMIN_BANNER_SHORT => "admin-banner-short",
    /// Accessible label of the account chip.
    ACCOUNT_MENU_LABEL => "account-menu-label",
    /// Account menu: change password.
    ACCOUNT_CHANGE_PASSWORD => "account-change-password",
    /// Account menu: sign out.
    ACCOUNT_SIGN_OUT => "account-sign-out",
    /// Change-password dialog: heading.
    PASSWORD_DIALOG_TITLE => "password-dialog-title",
    /// Form label: the current password.
    FIELD_CURRENT_PASSWORD => "field-current-password",
    /// Change-password dialog: the button.
    PASSWORD_SAVE => "password-save",
    /// Change-password: done.
    PASSWORD_CHANGED => "password-changed",
    /// Forced password change: heading.
    PASSWORD_MUST_CHANGE_TITLE => "password-must-change-title",
    /// Forced password change: explanation.
    PASSWORD_MUST_CHANGE_TEXT => "password-must-change-text",
    /// Change-password error: the current password is wrong.
    PASSWORD_CURRENT_WRONG => "password-current-wrong",

    /// Navigation: managing users (Admins only).
    NAV_USERS => "nav-users",
    /// Role: Viewer.
    ROLE_VIEWER => "role-viewer",
    /// Role: Driver.
    ROLE_DRIVER => "role-driver",
    /// Role: Maintainer.
    ROLE_MAINTAINER => "role-maintainer",
    /// Role: Admin.
    ROLE_ADMIN => "role-admin",
    /// No role on all vehicles.
    ROLE_NONE => "role-none",
    /// Users: button to add a user.
    USERS_ADD => "users-add",
    /// Users: badge on the signed-in Admin.
    USERS_YOU => "users-you",
    /// Users: badge on a disabled user.
    USERS_DISABLED => "users-disabled",
    /// Users: badge on someone with a temporary password.
    USERS_TEMPORARY => "users-temporary",
    /// Users list: a person's role on all vehicles; takes `role` (text).
    USERS_ROLE_SUMMARY => "users-role-summary",
    /// Users detail: the roles heading.
    USERS_ROLES_HEADING => "users-roles-heading",
    /// Users detail: the all-vehicles role row.
    USERS_ALL_VEHICLES => "users-all-vehicles",
    /// Users detail: the account heading.
    USERS_ACCOUNT_HEADING => "users-account-heading",
    /// Users detail: reset password.
    USERS_RESET_PASSWORD => "users-reset-password",
    /// Users detail: disable.
    USERS_DISABLE => "users-disable",
    /// Users detail: enable again.
    USERS_ENABLE => "users-enable",
    /// Users detail: why the last Admin can't be disabled or demoted.
    USERS_LAST_ADMIN => "users-last-admin",
    /// Users detail: what disabling does.
    USERS_DISABLE_NOTE => "users-disable-note",
    /// Confirm dialog: disable; takes `name` (text).
    USERS_CONFIRM_DISABLE_TITLE => "users-confirm-disable-title",
    /// Confirm dialog: what disabling does.
    USERS_CONFIRM_DISABLE_TEXT => "users-confirm-disable-text",
    /// Confirm dialog: the disable button.
    USERS_CONFIRM_DISABLE => "users-confirm-disable",
    /// Confirm dialog: reset; takes `name` (text).
    USERS_CONFIRM_RESET_TITLE => "users-confirm-reset-title",
    /// Confirm dialog: what a reset does.
    USERS_CONFIRM_RESET_TEXT => "users-confirm-reset-text",
    /// Confirm dialog: the reset button.
    USERS_CONFIRM_RESET => "users-confirm-reset",
    /// Add-user dialog: heading.
    USERS_ADD_TITLE => "users-add-title",
    /// Form label: another person's name.
    FIELD_THEIR_NAME => "field-their-name",
    /// Add-user dialog: role label.
    USERS_ROLE_ON_ALL => "users-role-on-all",
    /// Add-user dialog: about the temporary password.
    USERS_ADD_NOTE => "users-add-note",
    /// Add-user dialog: the button.
    USERS_ADD_BUTTON => "users-add-button",
    /// Temporary-password dialog: heading; takes `name` (text).
    USERS_TEMP_TITLE => "users-temp-title",
    /// Temporary-password dialog: what to do with it.
    USERS_TEMP_TEXT => "users-temp-text",
    /// Temporary-password dialog: copy button.
    USERS_TEMP_COPY => "users-temp-copy",
    /// Temporary-password dialog: after copying.
    USERS_TEMP_COPIED => "users-temp-copied",
    /// Temporary-password dialog: close.
    USERS_TEMP_DONE => "users-temp-done",
    /// Temporary-password dialog: what happens next.
    USERS_TEMP_NOTE => "users-temp-note",
    /// Add-user error: the username is taken.
    USERS_USERNAME_TAKEN => "users-username-taken",
    /// Users screen outside Admin mode.
    USERS_NOT_ALLOWED => "users-not-allowed",
    /// Users detail with nobody chosen.
    USERS_SELECT_HINT => "users-select-hint",
    /// Users detail on phones: back to the list.
    USERS_BACK => "users-back",

    /// Sign-in screen: the link to recover a password.
    SIGN_IN_FORGOT_LINK => "sign-in-forgot-link",
    /// Recovery: heading of the username step.
    RECOVERY_TITLE => "recovery-title",
    /// Recovery: the username step's instruction.
    RECOVERY_SUBTITLE => "recovery-subtitle",
    /// Recovery: the username step's button.
    RECOVERY_CONTINUE => "recovery-continue",
    /// Recovery: the answer everyone gets (heading).
    RECOVERY_ASK_ADMIN_TITLE => "recovery-ask-admin-title",
    /// Recovery: the answer everyone gets (line under it).
    RECOVERY_ASK_ADMIN_TEXT => "recovery-ask-admin-text",
    /// Recovery: heading above the code form.
    RECOVERY_HAVE_CODE => "recovery-have-code",
    /// Form label: the recovery code.
    FIELD_RECOVERY_CODE => "field-recovery-code",
    /// Form label: a new password replacing an old one.
    FIELD_NEW_PASSWORD_LONG => "field-new-password-long",
    /// Form label: the new password, again.
    FIELD_NEW_PASSWORD_AGAIN => "field-new-password-again",
    /// Recovery: the code form's button.
    RECOVERY_SUBMIT => "recovery-submit",
    /// Recovery: back to sign-in.
    RECOVERY_BACK => "recovery-back",
    /// Recovery error: wrong, used, or expired code.
    RECOVERY_WRONG_CODE => "recovery-wrong-code",

    /// Settings: the tab with the user's own preferences.
    SETTINGS_TAB_YOU => "settings-tab-you",
    /// Settings: the tab with the household defaults.
    SETTINGS_TAB_HOUSEHOLD => "settings-tab-household",
    /// Settings, You tab: what it's for.
    SETTINGS_YOU_HELP => "settings-you-help",
    /// Settings, Household tab: what it's for.
    SETTINGS_HOUSEHOLD_HELP => "settings-household-help",
    /// Settings, Household tab outside Admin mode.
    SETTINGS_HOUSEHOLD_ADMIN_ONLY => "settings-household-admin-only",
    /// Settings: the switch to follow the household default.
    SETTINGS_USE_HOUSEHOLD => "settings-use-household",
    /// Settings: the per-device section heading.
    SETTINGS_THIS_DEVICE => "settings-this-device",
    /// Settings: the theme row.
    SETTINGS_THEME => "settings-theme",
    /// Setting: distance unit.
    SETTING_DISTANCE => "setting-distance",
    /// Setting: fuel volume unit.
    SETTING_VOLUME => "setting-volume",
    /// Setting: fuel economy unit.
    SETTING_ECONOMY => "setting-economy",
    /// Setting: pressure unit.
    SETTING_PRESSURE => "setting-pressure",
    /// Setting: date format.
    SETTING_DATE_FORMAT => "setting-date-format",
    /// Setting: default currency.
    SETTING_CURRENCY => "setting-currency",
    /// Setting: language.
    SETTING_LANGUAGE => "setting-language",
    /// Choice: miles.
    UNIT_MILES => "unit-miles",
    /// Choice: kilometers.
    UNIT_KILOMETERS => "unit-kilometers",
    /// Choice: US gallons.
    UNIT_US_GALLONS => "unit-us-gallons",
    /// Choice: imperial gallons.
    UNIT_IMPERIAL_GALLONS => "unit-imperial-gallons",
    /// Choice: liters.
    UNIT_LITERS => "unit-liters",
    /// Choice: miles per US gallon.
    UNIT_MPG_US => "unit-mpg-us",
    /// Choice: miles per imperial gallon.
    UNIT_MPG_UK => "unit-mpg-uk",
    /// Choice: liters per 100 km.
    UNIT_L_PER_100_KM => "unit-l-per-100-km",
    /// Choice: kilometers per liter.
    UNIT_KM_PER_L => "unit-km-per-l",
    /// Choice: psi.
    UNIT_PSI => "unit-psi",
    /// Choice: kilopascals.
    UNIT_KPA => "unit-kpa",
    /// Choice: bar.
    UNIT_BAR => "unit-bar",
    /// Date format choice, shown as a sample.
    DATE_SAMPLE_DAY_MONTH_NAME_YEAR => "date-sample-day-month-name-year",
    /// Date format choice, shown as a sample.
    DATE_SAMPLE_MONTH_DAY_YEAR => "date-sample-month-day-year",
    /// Date format choice, shown as a sample.
    DATE_SAMPLE_DAY_MONTH_YEAR => "date-sample-day-month-year",
    /// Date format choice, shown as a sample.
    DATE_SAMPLE_YEAR_MONTH_DAY => "date-sample-year-month-day",
    /// Choice: English (United States).
    LANGUAGE_ENGLISH_US => "language-english-us",
    /// Settings: the sign-in section heading.
    SETTINGS_SIGN_IN => "settings-sign-in",
    /// Setting: when the app signs out by itself.
    SETTING_IDLE_SIGN_OUT => "setting-idle-sign-out",
    /// Idle sign-out choice: 1 hour.
    IDLE_SIGN_OUT_ONE_HOUR => "idle-sign-out-one-hour",
    /// Idle sign-out choice: 4 hours.
    IDLE_SIGN_OUT_FOUR_HOURS => "idle-sign-out-four-hours",
    /// Idle sign-out choice: 8 hours.
    IDLE_SIGN_OUT_EIGHT_HOURS => "idle-sign-out-eight-hours",
    /// Idle sign-out choice: 24 hours.
    IDLE_SIGN_OUT_TWENTY_FOUR_HOURS => "idle-sign-out-twenty-four-hours",
    /// Idle sign-out choice: never.
    IDLE_SIGN_OUT_NEVER => "idle-sign-out-never",
    /// Notice: the app left Admin mode after 15 idle minutes.
    IDLE_ADMIN_ENDED => "idle-admin-ended",
    /// Notice on the sign-in card: signed out after `$hours` idle hours.
    IDLE_SIGNED_OUT => "idle-signed-out",
    /// A notice's dismiss button.
    NOTICE_DISMISS => "notice-dismiss",
    /// Message title: the nightly snapshot failed.
    INBOX_SNAPSHOT_FAILED => "inbox-snapshot-failed",
    /// Message title: the nightly snapshot was skipped.
    INBOX_SNAPSHOT_SKIPPED => "inbox-snapshot-skipped",
    /// Message title: the hourly database check failed.
    INBOX_QUICK_CHECK_FAILED => "inbox-quick-check-failed",
    /// Message title: a job this build doesn't know by name failed.
    INBOX_JOB_FAILED => "inbox-job-failed",
    /// Message title: a job this build doesn't know by name was skipped.
    INBOX_JOB_SKIPPED => "inbox-job-skipped",
    /// Message text for a job this build doesn't know by name.
    INBOX_JOB_BODY => "inbox-job-body",
    /// Message text for a failed or skipped nightly snapshot.
    INBOX_SNAPSHOT_BODY => "inbox-snapshot-body",
    /// Message text for a failed hourly database check.
    INBOX_QUICK_CHECK_BODY => "inbox-quick-check-body",
    /// Message title: the server is refusing changes.
    INBOX_SERVER_UNHEALTHY => "inbox-server-unhealthy",
    /// Inbox title: the server couldn't open its place data (#059).
    INBOX_PLACE_LOOKUP_UNAVAILABLE => "inbox-place-lookup-unavailable",
    /// Inbox text: place lookup unavailable, and what still works.
    INBOX_PLACE_LOOKUP_BODY => "inbox-place-lookup-body",
    /// Message text: the database couldn't be opened.
    INBOX_DATABASE_UNAVAILABLE => "inbox-database-unavailable",
    /// Message text: the integrity check found problems.
    INBOX_INTEGRITY_CHECK_FAILED => "inbox-integrity-check-failed",
    /// Message button: open Diagnostics.
    INBOX_OPEN_DIAGNOSTICS => "inbox-open-diagnostics",
    /// Messages filter: everything.
    MESSAGES_FILTER_ALL => "messages-filter-all",
    /// Severity word: urgent.
    MESSAGES_SEVERITY_URGENT => "messages-severity-urgent",
    /// Severity word: warning.
    MESSAGES_SEVERITY_WARNING => "messages-severity-warning",
    /// Severity word: info.
    MESSAGES_SEVERITY_INFO => "messages-severity-info",
    /// Messages filter: urgent only.
    MESSAGES_FILTER_URGENT => "messages-filter-urgent",
    /// Messages filter: warnings only.
    MESSAGES_FILTER_WARNING => "messages-filter-warning",
    /// Messages filter: info only.
    MESSAGES_FILTER_INFO => "messages-filter-info",
    /// How many messages are unread (`$count`).
    MESSAGES_UNREAD => "messages-unread",
    /// Messages: nothing chosen yet.
    MESSAGES_SELECT_HINT => "messages-select-hint",
    /// Messages: there are none.
    MESSAGES_EMPTY => "messages-empty",
    /// Messages on phones: back to the list.
    MESSAGES_BACK => "messages-back",
    /// Messages: the technical details heading (Admin mode).
    MESSAGES_DETAILS => "messages-details",
    /// Messages: the dismiss button.
    MESSAGES_DISMISS => "messages-dismiss",
    /// A time shown in UTC (`$time`).
    MESSAGES_TIME_UTC => "messages-time-utc",
    /// The urgent banner's label.
    URGENT_BANNER_LABEL => "urgent-banner-label",
    /// The urgent banner's link to the message.
    URGENT_BANNER_VIEW => "urgent-banner-view",
    /// Notice on the sign-in card: signed out after `$minutes` idle minutes (test timers).
    IDLE_SIGNED_OUT_MINUTES => "idle-signed-out-minutes",
    /// The top bar's tag while test timers are on (`$minutes`).
    TEST_TIMERS_TAG => "test-timers-tag",
    /// A sign-out choice with its test length (`$label`, `$minutes`).
    IDLE_SIGN_OUT_TEST => "idle-sign-out-test",
    /// Notes longer than the limit.
    NOTES_TOO_LONG => "notes-too-long",
    /// A group with no name.
    GROUP_NAME_EMPTY => "group-name-empty",
    /// A group name longer than the limit.
    GROUP_NAME_TOO_LONG => "group-name-too-long",
    /// A vehicle with no name.
    ASSET_NAME_EMPTY => "asset-name-empty",
    /// A vehicle name longer than the limit.
    ASSET_NAME_TOO_LONG => "asset-name-too-long",
    /// Make, model, colour, or an identifier longer than the limit.
    ASSET_TEXT_TOO_LONG => "asset-text-too-long",
    /// A model year outside the plausible range.
    ASSET_YEAR_OUT_OF_RANGE => "asset-year-out-of-range",
    /// An identifier with no value.
    ASSET_IDENTIFIER_EMPTY => "asset-identifier-empty",
    /// More identifiers than allowed.
    ASSET_TOO_MANY_IDENTIFIERS => "asset-too-many-identifiers",
    /// Event kind: a fuel-up.
    EVENT_KIND_FUEL => "event-kind-fuel",
    /// Event kind: a reading.
    EVENT_KIND_READING => "event-kind-reading",
    /// Event kind: maintenance.
    EVENT_KIND_MAINTENANCE => "event-kind-maintenance",
    /// Event kind: a repair.
    EVENT_KIND_REPAIR => "event-kind-repair",
    /// Meter unit: hours.
    UNIT_HOURS => "unit-hours",
    /// A label with nothing in it.
    LABEL_EMPTY => "label-empty",
    /// A label longer than the limit.
    LABEL_TOO_LONG => "label-too-long",
    /// A meter unit that doesn't fit its dimension.
    METER_WRONG_UNIT => "meter-wrong-unit",
    /// Too many decimal places on a meter.
    METER_TOO_PRECISE => "meter-too-precise",
    /// Too many decimal places on a number field.
    FIELD_TOO_MANY_DECIMALS => "field-too-many-decimals",
    /// A choice list with no choices or too many.
    FIELD_BAD_CHOICES => "field-bad-choices",
    /// A range that doesn't fit its field.
    FIELD_BAD_RANGE => "field-bad-range",
    /// Removing a group that still has vehicles.
    GROUP_NOT_EMPTY => "group-not-empty",
    /// Removing the last group.
    GROUP_LAST => "group-last",
    /// The typed name doesn't match the vehicle's.
    NAME_MISMATCH => "name-mismatch",
    /// The default group's name, made with the server.
    GROUP_DEFAULT_NAME => "group-default-name",
    /// Sidebar and title: every group's vehicles.
    NAV_ALL_VEHICLES => "nav-all-vehicles",
    /// Button: add a vehicle.
    VEHICLES_ADD => "vehicles-add",
    /// The sort menu's label (for screen readers).
    VEHICLES_SORT_LABEL => "vehicles-sort-label",
    /// Sort choice: newest activity.
    SORT_NEWEST_ACTIVITY => "sort-newest-activity",
    /// Sort choice: by name.
    SORT_NAME => "sort-name",
    /// Sort choice: the person's own order.
    SORT_CUSTOM => "sort-custom",
    /// Button: arrange the custom order.
    VEHICLES_ARRANGE => "vehicles-arrange",
    /// Button: finish arranging.
    VEHICLES_ARRANGE_DONE => "vehicles-arrange-done",
    /// Button: show archived vehicles (`$count`).
    VEHICLES_SHOW_ARCHIVED => "vehicles-show-archived",
    /// Button: hide archived vehicles.
    VEHICLES_HIDE_ARCHIVED => "vehicles-hide-archived",
    /// How many vehicles a group has (`$count`).
    VEHICLES_COUNT => "vehicles-count",
    /// No vehicles at all.
    VEHICLES_EMPTY => "vehicles-empty",
    /// No vehicles: how to add one.
    VEHICLES_EMPTY_ADMIN => "vehicles-empty-admin",
    /// A group with no vehicles.
    VEHICLES_EMPTY_GROUP => "vehicles-empty-group",
    /// The phone's group menu label.
    VEHICLES_GROUP_MENU => "vehicles-group-menu",
    /// A card's note until readings arrive.
    VEHICLES_CARD_COMING => "vehicles-card-coming",
    /// Arranging: move a vehicle up.
    VEHICLES_MOVE_UP => "vehicles-move-up",
    /// Arranging: move a vehicle down.
    VEHICLES_MOVE_DOWN => "vehicles-move-down",
    /// The add-vehicle dialog's title.
    ADD_VEHICLE_TITLE => "add-vehicle-title",
    /// Add vehicle, step 1: pick a kind.
    ADD_VEHICLE_KIND => "add-vehicle-kind",
    /// Add vehicle: the add button.
    ADD_VEHICLE_ADD => "add-vehicle-add",
    /// Add vehicle: while adding.
    ADD_VEHICLE_ADDING => "add-vehicle-adding",
    /// Field: the vehicle's name.
    ADD_VEHICLE_NAME => "add-vehicle-name",
    /// Add vehicle: what it starts with.
    ADD_VEHICLE_NOTE => "add-vehicle-note",
    /// Field: make.
    FIELD_MAKE => "field-make",
    /// Field: model.
    FIELD_MODEL => "field-model",
    /// Field: model year.
    FIELD_YEAR => "field-year",
    /// Field: colour.
    FIELD_COLOUR => "field-colour",
    /// Field: group.
    FIELD_GROUP => "field-group",
    /// Lifecycle status: in service.
    STATUS_IN_SERVICE => "status-in-service",
    /// Lifecycle status: spare.
    STATUS_SPARE => "status-spare",
    /// Lifecycle status: out for service.
    STATUS_OUT_FOR_SERVICE => "status-out-for-service",
    /// Lifecycle status: retired.
    STATUS_RETIRED => "status-retired",
    /// Lifecycle status: disposed.
    STATUS_DISPOSED => "status-disposed",
    /// A vehicle action the person's role or mode doesn't allow.
    VEHICLES_NOT_ALLOWED => "vehicles-not-allowed",
    /// Profile name: car. Used by profile files (data), so code rarely names it.
    PROFILE_CAR => "profile-car",
    /// Profile name: truck. Used by profile files (data), so code rarely names it.
    PROFILE_TRUCK => "profile-truck",
    /// Profile name: motorcycle. Used by profile files (data), so code rarely names it.
    PROFILE_MOTORCYCLE => "profile-motorcycle",
    /// Profile name: trailer. Used by profile files (data), so code rarely names it.
    PROFILE_TRAILER => "profile-trailer",
    /// Profile name: generator. Used by profile files (data), so code rarely names it.
    PROFILE_GENERATOR => "profile-generator",
    /// Profile name: other equipment. Used by profile files (data), so code rarely names it.
    PROFILE_OTHER_EQUIPMENT => "profile-other-equipment",
    /// Identifier: VIN. Used by profile files (data), so code rarely names it.
    IDENTIFIER_VIN => "identifier-vin",
    /// Identifier: license plate. Used by profile files (data), so code rarely names it.
    IDENTIFIER_PLATE => "identifier-plate",
    /// Identifier: serial number. Used by profile files (data), so code rarely names it.
    IDENTIFIER_SERIAL => "identifier-serial",
    /// Expected document: title. Used by profile files (data), so code rarely names it.
    DOCUMENT_TITLE => "document-title",
    /// Expected document: registration. Used by profile files (data), so code rarely names it.
    DOCUMENT_REGISTRATION => "document-registration",
    /// Expected document: insurance. Used by profile files (data), so code rarely names it.
    DOCUMENT_INSURANCE => "document-insurance",
    /// A profile's meter: the odometer. Used by profile files (data), so code rarely names it.
    METER_ODOMETER => "meter-odometer",
    /// A profile's meter: run hours. Used by profile files (data), so code rarely names it.
    METER_RUN_HOURS => "meter-run-hours",
    /// A profile's custom field: trip time. Used by profile files (data), so code rarely names it.
    FIELD_TRIP_TIME => "field-trip-time",
    /// A profile's custom field: trip EV distance. Used by profile files (data), so code rarely names it.
    FIELD_TRIP_EV_DISTANCE => "field-trip-ev-distance",
    /// Vehicle page: no such vehicle, or not visible.
    VEHICLE_NOT_FOUND => "vehicle-not-found",
    /// Vehicle page: back to All vehicles.
    VEHICLE_BACK_ALL => "vehicle-back-all",
    /// Vehicle page: back to its group; takes `group` (text).
    VEHICLE_BACK_GROUP => "vehicle-back-group",
    /// Vehicle page: the tab bar's accessible name.
    VEHICLE_TABS => "vehicle-tabs",
    /// Vehicle tab: Overview.
    TAB_OVERVIEW => "tab-overview",
    /// Vehicle tab: Timeline.
    TAB_TIMELINE => "tab-timeline",
    /// Vehicle tab: Maintenance.
    TAB_MAINTENANCE => "tab-maintenance",
    /// Vehicle tab: Fuel.
    TAB_FUEL => "tab-fuel",
    /// Vehicle tab: Documents.
    TAB_DOCUMENTS => "tab-documents",
    /// Vehicle tab: Setup.
    TAB_SETUP => "tab-setup",
    /// Vehicle tab without content yet; takes `milestone` (number).
    TAB_COMING => "tab-coming",
    /// Overview panel: readiness.
    PANEL_READY => "panel-ready",
    /// Overview panel: readiness, not built yet.
    PANEL_READY_COMING => "panel-ready-coming",
    /// Overview panel: coming due.
    PANEL_DUE => "panel-due",
    /// Overview panel: coming due, not built yet.
    PANEL_DUE_COMING => "panel-due-coming",
    /// Overview panel: open squawks.
    PANEL_SQUAWKS => "panel-squawks",
    /// Overview panel: latest fuel-ups.
    PANEL_FUEL => "panel-fuel",
    /// Overview panel and Setup section: meters.
    PANEL_METERS => "panel-meters",
    /// A vehicle with no meters.
    METERS_NONE => "meters-none",
    /// A meter's required event kinds; takes `kinds` (text, a list).
    METER_REQUIRED_FOR => "meter-required-for",
    /// A meter no event kind requires.
    METER_REQUIRED_NONE => "meter-required-none",
    /// Meter decimal places: none.
    PRECISION_0 => "precision-0",
    /// Meter decimal places: one.
    PRECISION_1 => "precision-1",
    /// Meter decimal places: two.
    PRECISION_2 => "precision-2",
    /// What a meter measures: distance.
    DIMENSION_DISTANCE => "dimension-distance",
    /// What a meter measures: duration.
    DIMENSION_DURATION => "dimension-duration",
    /// Setup section: the vehicle's details.
    SETUP_DETAILS => "setup-details",
    /// Setup: open the details form.
    SETUP_EDIT_DETAILS => "setup-edit-details",
    /// Setup: the vehicle has no description.
    SETUP_NO_DESCRIPTION => "setup-no-description",
    /// Setup: add a meter.
    SETUP_ADD_METER => "setup-add-meter",
    /// Setup section: custom fields.
    SETUP_FIELDS => "setup-fields",
    /// Setup: add a custom field.
    SETUP_ADD_FIELD => "setup-add-field",
    /// Setup: a vehicle with no custom fields.
    SETUP_FIELDS_NONE => "setup-fields-none",
    /// Setup section: group, status, and deleting.
    SETUP_GROUP_STATUS => "setup-group-status",
    /// Setup: open the move dialog.
    SETUP_MOVE => "setup-move",
    /// Setup: the lifecycle status menu.
    SETUP_STATUS => "setup-status",
    /// Setup: open the delete dialog.
    SETUP_DELETE => "setup-delete",
    /// Setup: the person can look but not change anything now.
    SETUP_VIEW_ONLY => "setup-view-only",
    /// Button: edit.
    ACTION_EDIT => "action-edit",
    /// Button: remove.
    ACTION_REMOVE => "action-remove",
    /// Button: save.
    ACTION_SAVE => "action-save",
    /// Button while saving.
    ACTION_SAVING => "action-saving",
    /// Form field: a vehicle's description.
    FIELD_DESCRIPTION => "field-description",
    /// Form note: the description is a Markdown subset, shown as plain text.
    FIELD_DESCRIPTION_NOTE => "field-description-note",
    /// Dialog title: add a meter.
    METER_ADD_TITLE => "meter-add-title",
    /// Dialog title: edit a meter.
    METER_EDIT_TITLE => "meter-edit-title",
    /// Form field: a meter's name.
    FIELD_NAME => "field-name",
    /// Form field: what a meter measures.
    FIELD_MEASURES => "field-measures",
    /// Form field: a meter's unit.
    FIELD_UNIT => "field-unit",
    /// Form field: a meter's or number's decimal places.
    FIELD_PRECISION => "field-precision",
    /// Form field: a meter's description.
    FIELD_METER_DESCRIPTION => "field-meter-description",
    /// Form note: a meter's dimension is fixed.
    METER_DIMENSION_FIXED => "meter-dimension-fixed",
    /// Confirm dialog title: remove a meter or field; takes `name` (text).
    REMOVE_TITLE => "remove-title",
    /// Confirm text: removing a meter.
    METER_REMOVE_TEXT => "meter-remove-text",
    /// Confirm text: removing a custom field.
    FIELD_REMOVE_TEXT => "field-remove-text",
    /// Dialog title: add a custom field.
    FIELD_ADD_TITLE => "field-add-title",
    /// Dialog title: edit a custom field.
    FIELD_EDIT_TITLE => "field-edit-title",
    /// Form field: a custom field's label.
    FIELD_LABEL => "field-label",
    /// Form field: what a custom field holds.
    FIELD_TYPE => "field-type",
    /// Form field: a choice list's choices.
    FIELD_CHOICES => "field-choices",
    /// Form field: a custom field is on.
    FIELD_ENABLED => "field-enabled",
    /// Badge: a custom field that's turned off.
    FIELD_OFF => "field-off",
    /// Form field: a number's sensible lowest value.
    FIELD_RANGE_MIN => "field-range-min",
    /// Form field: a number's sensible highest value.
    FIELD_RANGE_MAX => "field-range-max",
    /// Form note: what a range does.
    FIELD_RANGE_NOTE => "field-range-note",
    /// Form error: a range end that isn't a number.
    FIELD_RANGE_BAD_NUMBER => "field-range-bad-number",
    /// Form field: a custom field's stable key.
    FIELD_KEY => "field-key",
    /// Form note: what a field's key is for.
    FIELD_KEY_NOTE => "field-key-note",
    /// Form note: a new field's key comes from its label.
    FIELD_KEY_LATER => "field-key-later",
    /// Form note: a custom field's type and event are fixed.
    FIELD_KIND_FIXED => "field-kind-fixed",
    /// Custom field type: text.
    KIND_TEXT => "kind-text",
    /// Custom field type: number.
    KIND_NUMBER => "kind-number",
    /// Custom field type: date.
    KIND_DATE => "kind-date",
    /// Custom field type: yes or no.
    KIND_YES_NO => "kind-yes-no",
    /// Custom field type: one of a list.
    KIND_CHOICE => "kind-choice",
    /// Custom field type: distance.
    KIND_DISTANCE => "kind-distance",
    /// Custom field type: duration.
    KIND_DURATION => "kind-duration",
    /// Custom field type: volume.
    KIND_VOLUME => "kind-volume",
    /// Custom field type: fuel economy.
    KIND_FUEL_ECONOMY => "kind-fuel-economy",
    /// Dialog title: move a vehicle; takes `name` (text).
    MOVE_TITLE => "move-title",
    /// Form field: the group to move to.
    MOVE_TO => "move-to",
    /// Move dialog: the access changes.
    MOVE_AFFECTED => "move-affected",
    /// Move dialog: no access changes.
    MOVE_NOBODY => "move-nobody",
    /// Move dialog: one person's change; takes `name`, `before`, `after` (text).
    MOVE_CHANGE => "move-change",
    /// A person with no access to a vehicle.
    ACCESS_NONE => "access-none",
    /// Button: move the vehicle.
    MOVE_MOVE => "move-move",
    /// Button while moving.
    MOVE_MOVING => "move-moving",
    /// Move dialog: only one group exists.
    MOVE_NO_OTHER_GROUP => "move-no-other-group",
    /// Dialog title: delete a vehicle; takes `name` (text).
    DELETE_TITLE => "delete-title",
    /// Delete dialog: when to delete.
    DELETE_TEXT => "delete-text",
    /// Delete dialog: type the name; takes `name` (text).
    DELETE_TYPE => "delete-type",
    /// Button: delete the vehicle.
    DELETE_DELETE => "delete-delete",
    /// Button while deleting.
    DELETE_DELETING => "delete-deleting",
    /// Navigation: the Groups screen (Admin mode).
    NAV_GROUPS => "nav-groups",
    /// A person's one role, in the list; takes `role` and `target` (text).
    USERS_ROLE_ON => "users-role-on",
    /// A person with several roles, in the list; takes `count` (number).
    USERS_ROLES_COUNT => "users-roles-count",
    /// Users: add a role on all vehicles, a group, or one vehicle.
    ROLES_ADD => "roles-add",
    /// Dialog title: add a role; takes `name` (text).
    ROLES_ADD_TITLE => "roles-add-title",
    /// Form field: what a role is on.
    ROLES_ON => "roles-on",
    /// Form field: the role.
    ROLES_ROLE => "roles-role",
    /// Button: add the role.
    ROLES_ADD_BUTTON => "roles-add-button",
    /// A role's target: a group; takes `name` (text).
    ROLES_ON_GROUP => "roles-on-group",
    /// A role's target: one vehicle; takes `name` (text).
    ROLES_ON_VEHICLE => "roles-on-vehicle",
    /// Menu heading: groups.
    ROLES_GROUPS => "roles-groups",
    /// Menu heading: vehicles.
    ROLES_VEHICLES => "roles-vehicles",
    /// A role on a group or vehicle that no longer exists.
    ROLES_REMOVED => "roles-removed",
    /// Button label (✕): remove a role.
    ROLES_REMOVE => "roles-remove",
    /// Users: a person with no roles.
    ROLES_NONE => "roles-none",
    /// Users: how roles combine.
    ROLES_NOTE => "roles-note",
    /// Groups: add a group.
    GROUPS_ADD => "groups-add",
    /// Dialog title: add a group.
    GROUPS_ADD_TITLE => "groups-add-title",
    /// Form field: a group's icon.
    GROUPS_ICON => "groups-icon",
    /// Groups: nothing chosen yet.
    GROUPS_SELECT_HINT => "groups-select-hint",
    /// Groups: the chosen group's vehicles.
    GROUPS_VEHICLES => "groups-vehicles",
    /// Groups: an empty group.
    GROUPS_NO_VEHICLES => "groups-no-vehicles",
    /// Groups: open a vehicle's page.
    GROUPS_OPEN => "groups-open",
    /// Groups: each person's access to the group.
    GROUPS_WHO => "groups-who",
    /// Who sees it: a role on all vehicles; takes `role` (text).
    GROUPS_FROM_ALL => "groups-from-all",
    /// Who sees it: a role on the group; takes `role` (text).
    GROUPS_FROM_GROUP => "groups-from-group",
    /// Who sees it: a role on one of its vehicles; takes `role` and `vehicle` (text).
    GROUPS_FROM_VEHICLE => "groups-from-vehicle",
    /// Who sees it: no role here.
    GROUPS_NO_ACCESS => "groups-no-access",
    /// Groups: delete the chosen group.
    GROUPS_DELETE => "groups-delete",
    /// Dialog title: delete a group; takes `name` (text).
    GROUPS_DELETE_TITLE => "groups-delete-title",
    /// Delete group dialog.
    GROUPS_DELETE_TEXT => "groups-delete-text",
    /// Groups: when Delete is possible.
    GROUPS_DELETE_NOTE => "groups-delete-note",
    /// Groups: how changes save.
    GROUPS_SAVES_NOTE => "groups-saves-note",
    /// Groups: a change was saved.
    GROUPS_SAVED => "groups-saved",
    /// Groups: not in Admin mode.
    GROUPS_NOT_ALLOWED => "groups-not-allowed",
    /// Groups on phones: back to the list.
    GROUPS_BACK => "groups-back",
    /// Group icon: garage.
    GROUP_ICON_GARAGE => "group-icon-garage",
    /// Group icon: hangar.
    GROUP_ICON_HANGAR => "group-icon-hangar",
    /// Group icon: boathouse.
    GROUP_ICON_BOATHOUSE => "group-icon-boathouse",
    /// Group icon: storage.
    GROUP_ICON_STORAGE => "group-icon-storage",
    /// Group icon: warehouse.
    GROUP_ICON_WAREHOUSE => "group-icon-warehouse",
    /// Group icon: shop.
    GROUP_ICON_SHOP => "group-icon-shop",
    /// Group icon: farm.
    GROUP_ICON_FARM => "group-icon-farm",
    /// Group icon: house.
    GROUP_ICON_HOUSE => "group-icon-house",
    /// Location: latitude out of range.
    LOCATION_LATITUDE_RANGE => "location-latitude-range",
    /// Location: longitude out of range.
    LOCATION_LONGITUDE_RANGE => "location-longitude-range",
    /// Location: typed coordinates aren't numbers.
    LOCATION_NOT_COORDINATES => "location-not-coordinates",
    /// Location: a saved place's radius out of range.
    LOCATION_RADIUS_RANGE => "location-radius-range",
    /// Location: no city.
    LOCATION_CITY_EMPTY => "location-city-empty",
    /// Location: no state or province.
    LOCATION_REGION_EMPTY => "location-region-empty",
    /// Location: a city or region name too long.
    LOCATION_PLACE_TOO_LONG => "location-place-too-long",
    /// Location: not a country code.
    LOCATION_COUNTRY_INVALID => "location-country-invalid",
    /// Location: not an IANA time zone name.
    LOCATION_TIME_ZONE_INVALID => "location-time-zone-invalid",
    /// Navigation: each person's saved places (#060).
    NAV_PLACES => "nav-places",
    /// Places: add a saved place.
    PLACES_ADD => "places-add",
    /// Places: what saved places are.
    PLACES_INTRO => "places-intro",
    /// Places: none yet.
    PLACES_EMPTY => "places-empty",
    /// Places: removing is safe for past events.
    PLACES_REMOVE_NOTE => "places-remove-note",
    /// A distance in meters; takes `meters` (number).
    PLACES_METERS => "places-meters",
    /// A distance in kilometers; takes `km` (number).
    PLACES_KILOMETERS => "places-kilometers",
    /// Dialog title: add a saved place.
    PLACE_ADD_TITLE => "place-add-title",
    /// Dialog title: edit a saved place.
    PLACE_EDIT_TITLE => "place-edit-title",
    /// Place dialog: the location part.
    PLACE_STEP_WHERE => "place-step-where",
    /// Place dialog: the city, state, and country part.
    PLACE_STEP_PLACE => "place-step-place",
    /// Place dialog: the name and radius part.
    PLACE_STEP_NAME => "place-step-name",
    /// Button: ask the device where it is.
    PLACE_USE_LOCATION => "place-use-location",
    /// Waiting for the device's location.
    PLACE_LOCATING => "place-locating",
    /// Button: type latitude and longitude instead.
    PLACE_TYPE_COORDINATES => "place-type-coordinates",
    /// Form field: typed or pasted coordinates.
    PLACE_COORDINATES => "place-coordinates",
    /// Form note: how to type coordinates.
    PLACE_COORDINATES_HINT => "place-coordinates-hint",
    /// Button: use the typed coordinates.
    PLACE_COORDINATES_APPLY => "place-coordinates-apply",
    /// A device location's accuracy, just taken; takes `meters` (number).
    PLACE_FIX_NOW => "place-fix-now",
    /// A device location's accuracy and age; takes `meters` and `minutes` (numbers).
    PLACE_FIX_MINUTES => "place-fix-minutes",
    /// The coordinates were typed, not from the device.
    PLACE_FIX_TYPED => "place-fix-typed",
    /// Warning: the device location is stale.
    PLACE_FIX_STALE => "place-fix-stale",
    /// Place dialog: nothing chosen yet.
    PLACE_NO_LOCATION => "place-no-location",
    /// The device location was refused.
    PLACE_LOCATION_DENIED => "place-location-denied",
    /// The device location couldn't be found.
    PLACE_LOCATION_UNAVAILABLE => "place-location-unavailable",
    /// The geocoder's suggestion; takes `place` (text).
    PLACE_SUGGESTED => "place-suggested",
    /// The suggestion's time zone and data version; takes `zone` and `version` (text).
    PLACE_SUGGESTED_DETAIL => "place-suggested-detail",
    /// The point is outside every country; takes `place` (text).
    PLACE_OFFSHORE => "place-offshore",
    /// Inside a saved place; takes `name` (text) and `meters` (number).
    PLACE_AT_SAVED => "place-at-saved",
    /// No suggestion: the server's place data is unavailable.
    PLACE_LOOKUP_UNAVAILABLE => "place-lookup-unavailable",
    /// Button: fill the place fields from the suggestion.
    PLACE_USE_SUGGESTION => "place-use-suggestion",
    /// Form field: the city.
    PLACE_CITY => "place-city",
    /// Form field: the state or province.
    PLACE_REGION => "place-region",
    /// Form field: the country.
    PLACE_COUNTRY => "place-country",
    /// Form field: a two-letter country code, when there's no country list.
    PLACE_COUNTRY_CODE => "place-country-code",
    /// Form note: the place can be typed.
    PLACE_TYPED_NOTE => "place-typed-note",
    /// Form field: the saved place's name.
    PLACE_NAME => "place-name",
    /// Form field: how close counts as being there.
    PLACE_RADIUS => "place-radius",
    /// Form error: no coordinates yet.
    PLACE_NEEDS_LOCATION => "place-needs-location",
    /// Confirm dialog title; takes `name` (text).
    PLACE_REMOVE_TITLE => "place-remove-title",
    /// Confirm dialog text: removing a place.
    PLACE_REMOVE_TEXT => "place-remove-text",
    /// Diagnostics section: the app and its data.
    ABOUT_HEADING => "about-heading",
    /// About: the app's version.
    ABOUT_APP => "about-app",
    /// About: the place data's version.
    ABOUT_PLACE_DATA => "about-place-data",
    /// About: the server couldn't open its place data.
    ABOUT_PLACE_DATA_UNAVAILABLE => "about-place-data-unavailable",
    /// About: data credits.
    ABOUT_CREDITS => "about-credits",
    /// About: the data credits when the server doesn't give them.
    ABOUT_CREDITS_DEFAULT => "about-credits-default",
    /// Picker: the nearby places list's heading (#061).
    PLACE_NEARBY => "place-nearby",
    /// Picker: typing is still possible.
    PLACE_NEARBY_NOTE => "place-nearby-note",
    /// A distance in kilometers, one decimal; takes `km` (text, already formatted).
    PLACE_DISTANCE_KM => "place-distance-km",
    /// A distance in miles, one decimal; takes `miles` (text, already formatted).
    PLACE_DISTANCE_MI => "place-distance-mi",
    /// Place type: a national capital.
    PLACE_KIND_CAPITAL => "place-kind-capital",
    /// Place type: a section of a bigger place.
    PLACE_KIND_DISTRICT => "place-kind-district",
    /// Place type: a small named locality.
    PLACE_KIND_LOCALITY => "place-kind-locality",
    /// Place type: 100,000 people or more.
    PLACE_KIND_CITY => "place-kind-city",
    /// Place type: 10,000 people or more.
    PLACE_KIND_TOWN => "place-kind-town",
    /// Place type: fewer than 10,000 people.
    PLACE_KIND_VILLAGE => "place-kind-village",
    /// Place role: seat of a state or province.
    PLACE_SEAT_REGION => "place-seat-region",
    /// Place role: seat of a county.
    PLACE_SEAT_COUNTY => "place-seat-county",
    /// Place role: seat of a smaller division.
    PLACE_SEAT_LOCAL => "place-seat-local",
    /// Compass: north.
    COMPASS_N => "compass-n",
    /// Compass: northeast.
    COMPASS_NE => "compass-ne",
    /// Compass: east.
    COMPASS_E => "compass-e",
    /// Compass: southeast.
    COMPASS_SE => "compass-se",
    /// Compass: south.
    COMPASS_S => "compass-s",
    /// Compass: southwest.
    COMPASS_SW => "compass-sw",
    /// Compass: west.
    COMPASS_W => "compass-w",
    /// Compass: northwest.
    COMPASS_NW => "compass-nw",
    /// Date: not a date.
    DATE_BAD_DATE => "date-bad-date",
    /// Date: not a time.
    DATE_BAD_TIME => "date-bad-time",
    /// Date: the zone isn't in the database.
    DATE_UNKNOWN_ZONE => "date-unknown-zone",
    /// Date: a multi-day event ending before it starts.
    DATE_END_BEFORE_START => "date-end-before-start",
    /// Date: outside 1900–2200.
    DATE_OUT_OF_RANGE => "date-out-of-range",
    /// Time: before noon.
    TIME_AM => "time-am",
    /// Time: noon and after.
    TIME_PM => "time-pm",
    /// A month's short name: Jan.
    MONTH_1 => "month-1",
    /// A month's short name: Feb.
    MONTH_2 => "month-2",
    /// A month's short name: Mar.
    MONTH_3 => "month-3",
    /// A month's short name: Apr.
    MONTH_4 => "month-4",
    /// A month's short name: May.
    MONTH_5 => "month-5",
    /// A month's short name: Jun.
    MONTH_6 => "month-6",
    /// A month's short name: Jul.
    MONTH_7 => "month-7",
    /// A month's short name: Aug.
    MONTH_8 => "month-8",
    /// A month's short name: Sep.
    MONTH_9 => "month-9",
    /// A month's short name: Oct.
    MONTH_10 => "month-10",
    /// A month's short name: Nov.
    MONTH_11 => "month-11",
    /// A month's short name: Dec.
    MONTH_12 => "month-12",
    /// Quantity: not a number.
    QUANTITY_NOT_A_NUMBER => "quantity-not-a-number",
    /// Quantity: negative.
    QUANTITY_NEGATIVE => "quantity-negative",
    /// Quantity: too large to store (or zero L/100 km).
    QUANTITY_TOO_LARGE => "quantity-too-large",
    /// Quantity: more decimals than allowed.
    QUANTITY_TOO_PRECISE => "quantity-too-precise",
    /// Quantity: not a duration.
    QUANTITY_NOT_A_DURATION => "quantity-not-a-duration",
    /// Unit abbreviation: miles.
    UNIT_ABBR_MI => "unit-abbr-mi",
    /// Unit abbreviation: kilometers.
    UNIT_ABBR_KM => "unit-abbr-km",
    /// Unit abbreviation: US gallons.
    UNIT_ABBR_GAL => "unit-abbr-gal",
    /// Unit abbreviation: imperial gallons.
    UNIT_ABBR_GAL_UK => "unit-abbr-gal-uk",
    /// Unit abbreviation: liters.
    UNIT_ABBR_L => "unit-abbr-l",
    /// Unit abbreviation: miles per US gallon.
    UNIT_ABBR_MPG => "unit-abbr-mpg",
    /// Unit abbreviation: miles per imperial gallon.
    UNIT_ABBR_MPG_UK => "unit-abbr-mpg-uk",
    /// Unit abbreviation: kilometers per liter.
    UNIT_ABBR_KM_PER_L => "unit-abbr-km-per-l",
    /// Unit abbreviation: liters per 100 kilometers.
    UNIT_ABBR_L_PER_100_KM => "unit-abbr-l-per-100-km",
    /// Unit abbreviation: hours.
    UNIT_ABBR_H => "unit-abbr-h",
    /// Event: a reason is needed (skip, override, correction).
    EVENT_REASON_EMPTY => "event-reason-empty",
    /// Event: a reason over 500 characters.
    EVENT_REASON_TOO_LONG => "event-reason-too-long",
    /// Event: the performer's name is empty or too long.
    EVENT_BAD_PERFORMER => "event-bad-performer",
    /// Event: no place and no reason.
    EVENT_LOCATION_REQUIRED => "event-location-required",
    /// Event: a required reading or field is missing.
    EVENT_REQUIRED => "event-required",
    /// Event: a reading that doesn't fit its neighbours in time.
    EVENT_READING_OUT_OF_ORDER => "event-reading-out-of-order",
    /// Event: a field value of the wrong type or choice.
    EVENT_WRONG_VALUE => "event-wrong-value",
    /// Event: a fuel-up of zero.
    EVENT_NO_VOLUME => "event-no-volume",
    /// Event: a negative cost or zero fuel economy.
    EVENT_BAD_AMOUNT => "event-bad-amount",
    /// Event: a percentage over 100.
    EVENT_BAD_PERCENT => "event-bad-percent",
    /// Event: the vehicle is among its own towed vehicles.
    EVENT_TOWS_ITSELF => "event-tows-itself",
    /// Event: e.g. a fuel-up for a trailer.
    EVENT_WRONG_KIND => "event-wrong-kind",
    /// Event: a change to a finalized event.
    EVENT_FINALIZED => "event-finalized",
    /// Event: a correction of a draft.
    EVENT_NOT_FINALIZED => "event-not-finalized",
    /// Event: a correction of an event that was already corrected.
    EVENT_ALREADY_CORRECTED => "event-already-corrected",
    /// Event flag: no place could be given.
    EVENT_FLAG_MISSING_LOCATION => "event-flag-missing-location",
    /// Event flag: a required reading or field was skipped.
    EVENT_FLAG_SKIPPED => "event-flag-skipped",
    /// Event flag: a reading out of order was kept.
    EVENT_FLAG_READING_OVERRIDE => "event-flag-reading-override",
    /// Event flag: it corrects another event.
    EVENT_FLAG_CORRECTION => "event-flag-correction",
    /// Setup: the meter dialog's "Required for" heading.
    METER_REQUIRED_LABEL => "meter-required-label",
    /// Setup: what checking a kind means.
    METER_REQUIRED_NOTE => "meter-required-note",
    /// The + Log chooser's title.
    LOG_TITLE => "log-title",
    /// The + Log chooser: the vehicle dropdown.
    LOG_VEHICLE => "log-vehicle",
    /// The + Log chooser: last time's vehicle.
    LOG_LAST_TIME => "log-last-time",
    /// The + Log chooser: under Fuel-up.
    LOG_FUEL_DETAIL => "log-fuel-detail",
    /// The + Log chooser: under Reading.
    LOG_READING_DETAIL => "log-reading-detail",
    /// The + Log chooser: nothing to log for.
    LOG_NO_VEHICLES => "log-no-vehicles",
    /// Log form: the badge on a required item.
    LOG_REQUIRED => "log-required",
    /// Log form: a meter's latest reading.
    LOG_LAST_READING => "log-last-reading",
    /// Log form: skipping a required reading.
    LOG_SKIP => "log-skip",
    /// Log form: skipping a required field.
    LOG_SKIP_FIELD => "log-skip-field",
    /// Log form: a skipped item.
    LOG_SKIPPED => "log-skipped",
    /// Log form: the skip reason.
    LOG_SKIP_REASON => "log-skip-reason",
    /// Log form: stop skipping.
    LOG_UNSKIP => "log-unskip",
    /// Log form: the Admin's override box.
    LOG_OVERRIDE_TITLE => "log-override-title",
    /// Log form: the override reason.
    LOG_OVERRIDE_REASON => "log-override-reason",
    /// Log form: what the override does.
    LOG_OVERRIDE_NOTE => "log-override-note",
    /// Log form: a yes-or-no field's yes.
    LOG_YES => "log-yes",
    /// Log form: a yes-or-no field's no.
    LOG_NO => "log-no",
    /// Log form: the fuel section.
    LOG_SECTION_FUEL => "log-section-fuel",
    /// Log form: the fuel volume.
    LOG_VOLUME => "log-volume",
    /// Log form: the fuel's total price.
    LOG_PRICE => "log-price",
    /// Log form: the price per gallon or liter.
    LOG_PER_UNIT => "log-per-unit",
    /// Log form: full fill.
    LOG_FILLED_FULL => "log-filled-full",
    /// Log form: an earlier fill-up wasn't logged.
    LOG_MISSED => "log-missed",
    /// Log form: the when and where section.
    LOG_SECTION_WHEN_WHERE => "log-section-when-where",
    /// Log form: logged as now.
    LOG_NOW => "log-now",
    /// Log form: change the date or place.
    LOG_CHANGE => "log-change",
    /// Log form: the date.
    LOG_DATE => "log-date",
    /// Log form: the time.
    LOG_TIME => "log-time",
    /// Log form: the event's time zone.
    LOG_ZONE => "log-zone",
    /// Log form: choose one of your saved places.
    LOG_SAVED_PLACE => "log-saved-place",
    /// Log form: done choosing the place.
    LOG_PLACE_DONE => "log-place-done",
    /// Log form: no place at all.
    LOG_NO_PLACE => "log-no-place",
    /// Log form: the no-place reason.
    LOG_NO_PLACE_REASON => "log-no-place-reason",
    /// Log form: stop saying there's no place.
    LOG_PLACE_AFTER_ALL => "log-place-after-all",
    /// Log form: the driving section.
    LOG_FOLD_DRIVING => "log-fold-driving",
    /// Log form: the driving summary.
    LOG_PCT_HIGHWAY => "log-pct-highway",
    /// Log form: the driving summary.
    LOG_PCT_TOWING => "log-pct-towing",
    /// Log form: the driving summary.
    LOG_PCT_HAULING => "log-pct-hauling",
    /// Log form: the highway slider.
    LOG_HIGHWAY => "log-highway",
    /// Log form: the towing slider.
    LOG_TOWING => "log-towing",
    /// Log form: the hauling slider.
    LOG_HAULING => "log-hauling",
    /// Log form: the vehicles towed.
    LOG_TOWED => "log-towed",
    /// Log form: the other meters and fields.
    LOG_FOLD_MORE => "log-fold-more",
    /// Log form: nothing in a folded section.
    LOG_FOLD_MORE_EMPTY => "log-fold-more-empty",
    /// Log form: the fuel economy the vehicle shows.
    LOG_REPORTED_ECONOMY => "log-reported-economy",
    /// Log form: the who and notes section.
    LOG_FOLD_WHO => "log-fold-who",
    /// Log form: done by the person logging it (summary).
    LOG_WHO_ME => "log-who-me",
    /// Log form: the who summary without notes.
    LOG_NOTES_NONE => "log-notes-none",
    /// Log form: the who summary with notes.
    LOG_NOTES_SOME => "log-notes-some",
    /// Log form: who did it.
    LOG_DONE_BY => "log-done-by",
    /// Log form: done by the person logging it.
    LOG_DONE_BY_ME => "log-done-by-me",
    /// Log form: done by someone else.
    LOG_DONE_BY_SOMEONE => "log-done-by-someone",
    /// Log form: someone else's name.
    LOG_THEIR_NAME => "log-their-name",
    /// Log form: someone else's contact.
    LOG_THEIR_CONTACT => "log-their-contact",
    /// Log form: notes.
    LOG_NOTES => "log-notes",
    /// Log form: by Save.
    LOG_DRAFT_NOTE => "log-draft-note",
    /// Log form: save.
    LOG_SAVE => "log-save",
    /// Log form: while saving.
    LOG_SAVING => "log-saving",
    /// Log form: saved, with warnings.
    LOG_SAVED => "log-saved",
    /// Log form: a field above its range.
    LOG_WARNING_ABOVE => "log-warning-above",
    /// Log form: a field below its range.
    LOG_WARNING_BELOW => "log-warning-below",
    /// Log form: the Admin kept a reading.
    LOG_WARNING_KEPT => "log-warning-kept",
    /// Log form: leave after saving.
    LOG_DONE => "log-done",
    /// Screen title: one event.
    SCREEN_EVENT => "screen-event",
    /// Screen title: editing a draft.
    SCREEN_EDIT_EVENT => "screen-edit-event",
    /// Screen title: correcting an event.
    SCREEN_CORRECT_EVENT => "screen-correct-event",
    /// An event that isn't there.
    EVENT_NOT_FOUND => "event-not-found",
    /// Log form: a reading below the one before.
    LOG_CLASH_BELOW => "log-clash-below",
    /// Log form: a reading above the one after.
    LOG_CLASH_ABOVE => "log-clash-above",
    /// Correct form: what correcting does.
    LOG_CORRECT_NOTE => "log-correct-note",
    /// Correct form: the reason.
    LOG_CORRECT_REASON => "log-correct-reason",
    /// Event page: a draft's status.
    EVENT_STATUS_DRAFT => "event-status-draft",
    /// Event page: a finalized event's status.
    EVENT_STATUS_FINALIZED => "event-status-finalized",
    /// Event page: hours until a draft locks.
    EVENT_LOCKS_HOURS => "event-locks-hours",
    /// Event page: minutes until a draft locks.
    EVENT_LOCKS_MINUTES => "event-locks-minutes",
    /// Event page: a draft due to lock now.
    EVENT_LOCKS_SOON => "event-locks-soon",
    /// Event page: finalize a draft.
    EVENT_FINALIZE_NOW => "event-finalize-now",
    /// Event page: delete a draft.
    EVENT_DELETE => "event-delete",
    /// Event page: correct a finalized event.
    EVENT_CORRECT => "event-correct",
    /// Event page: on an original that was corrected.
    EVENT_CORRECTED_BY => "event-corrected-by",
    /// Event page: link to the correction.
    EVENT_SEE_CORRECTION => "event-see-correction",
    /// Event page: on a correction.
    EVENT_CORRECTS => "event-corrects",
    /// Event page: link to the original.
    EVENT_SEE_ORIGINAL => "event-see-original",
    /// Event page: a full fill.
    EVENT_FILLED_FULL => "event-filled-full",
    /// Event page: not a full fill.
    EVENT_PARTIAL_FILL => "event-partial-fill",
    /// Event page: the place's name.
    EVENT_PLACE_NAME => "event-place-name",
    /// Event page: the city, state, and country.
    EVENT_PLACE => "event-place",
    /// Event page: why there's no place.
    EVENT_NO_PLACE => "event-no-place",
    /// Event page: where the location came from.
    EVENT_FROM => "event-from",
    /// Event page: from the device.
    EVENT_SOURCE_GPS => "event-source-gps",
    /// Event page: from a saved place.
    EVENT_SOURCE_SAVED => "event-source-saved",
    /// Event page: picked on a map.
    EVENT_SOURCE_MAP => "event-source-map",
    /// Event page: typed in.
    EVENT_SOURCE_MANUAL => "event-source-manual",
    /// Event page: the event's zone.
    EVENT_ZONE => "event-zone",
    /// Event page: a skipped item.
    EVENT_SKIPPED_BECAUSE => "event-skipped-because",
    /// Event page: the economy the vehicle showed.
    EVENT_REPORTED_ECONOMY => "event-reported-economy",
    /// Event page: done by the person who logged it.
    EVENT_DONE_BY_LOGGER => "event-done-by-logger",
    /// Event page: the Admin's override reason.
    EVENT_OVERRIDE => "event-override",
    /// Event page: the event was typed.
    EVENT_TYPED => "event-typed",
    /// Event page: the event was spoken.
    EVENT_VOICE => "event-voice",
    /// Event page: the event was imported.
    EVENT_IMPORTED => "event-imported",
    /// Event page: the record section.
    EVENT_RECORD => "event-record",
    /// Event page: who logged it and when.
    EVENT_LOGGED => "event-logged",
    /// Event page: when it last changed.
    EVENT_LAST_EDITED => "event-last-edited",
    /// Event page: who finalized it and when.
    EVENT_FINALIZED_ROW => "event-finalized-row",
    /// Event page: finalized automatically.
    EVENT_BY_SERVER => "event-by-server",
    /// Finalize now's confirmation.
    EVENT_FINALIZE_TITLE => "event-finalize-title",
    /// Finalize now's confirmation.
    EVENT_FINALIZE_TEXT => "event-finalize-text",
    /// Finalize now's confirmation: the button.
    EVENT_FINALIZE => "event-finalize",
    /// Delete's confirmation.
    EVENT_DELETE_TITLE => "event-delete-title",
    /// Delete's confirmation.
    EVENT_DELETE_TEXT => "event-delete-text",
    /// Setup: a field shown on no kind of event.
    FIELD_NO_KINDS => "field-no-kinds",
    /// Setup: the field dialog's "Shown on" checkboxes.
    FIELD_SHOWN_ON => "field-shown-on",
    /// Setup: what "Shown on" means for a field.
    FIELD_KIND_NOTE => "field-kind-note",
    /// Setup: what "Required for" means for a field.
    FIELD_REQUIRED_FOR_NOTE => "field-required-for-note",
    /// A month's full name, for headings.
    MONTH_LONG_1 => "month-long-1",
    /// A month's full name, for headings.
    MONTH_LONG_2 => "month-long-2",
    /// A month's full name, for headings.
    MONTH_LONG_3 => "month-long-3",
    /// A month's full name, for headings.
    MONTH_LONG_4 => "month-long-4",
    /// A month's full name, for headings.
    MONTH_LONG_5 => "month-long-5",
    /// A month's full name, for headings.
    MONTH_LONG_6 => "month-long-6",
    /// A month's full name, for headings.
    MONTH_LONG_7 => "month-long-7",
    /// A month's full name, for headings.
    MONTH_LONG_8 => "month-long-8",
    /// A month's full name, for headings.
    MONTH_LONG_9 => "month-long-9",
    /// A month's full name, for headings.
    MONTH_LONG_10 => "month-long-10",
    /// A month's full name, for headings.
    MONTH_LONG_11 => "month-long-11",
    /// A month's full name, for headings.
    MONTH_LONG_12 => "month-long-12",
    /// Timeline: show every event.
    TIMELINE_ALL => "timeline-all",
    /// Timeline: only fuel-ups.
    TIMELINE_FUEL_UPS => "timeline-fuel-ups",
    /// Timeline: only readings.
    TIMELINE_READINGS => "timeline-readings",
    /// Timeline: no events.
    TIMELINE_EMPTY => "timeline-empty",
    /// Timeline: an original that was corrected.
    TIMELINE_CORRECTED => "timeline-corrected",
    /// Timeline: a draft.
    TIMELINE_DRAFT => "timeline-draft",
    /// Overview: no fuel-ups.
    PANEL_FUEL_NONE => "panel-fuel-none",
    /// Overview: open the Timeline.
    PANEL_SEE_TIMELINE => "panel-see-timeline",
    /// Overview: economy comes later.
    PANEL_ECONOMY_LATER => "panel-economy-later",
    /// Overview: the newest event.
    PANEL_LAST_ACTIVITY => "panel-last-activity",
    /// Overview: no events.
    PANEL_NO_ACTIVITY => "panel-no-activity",
    /// Overview: the newest event is finalized.
    PANEL_FINALIZED => "panel-finalized",
    /// Overview: when a meter's latest reading was.
    PANEL_READING_WHEN => "panel-reading-when",
    /// Overview: a meter with no readings.
    PANEL_NO_READINGS => "panel-no-readings",
    /// The tag on a server that isn't the household's main one, e.g. "test server".
    INSTANCE_TAG => "instance-tag",
    /// The instance tag's tooltip.
    INSTANCE_TAG_NOTE => "instance-tag-note",
    /// Squawk severity: minor.
    SEVERITY_LOW => "severity-low",
    /// Squawk severity: get it looked at.
    SEVERITY_MEDIUM => "severity-medium",
    /// Squawk severity: keeps it out of use.
    SEVERITY_HIGH => "severity-high",
    /// What High means for a car, truck, or motorcycle.
    SQUAWK_HIGH_NOT_DRIVABLE => "squawk-high-not-drivable",
    /// What High means for a trailer.
    SQUAWK_HIGH_NOT_TOWABLE => "squawk-high-not-towable",
    /// What High means for equipment, and kinds without a profile.
    SQUAWK_HIGH_OUT_OF_SERVICE => "squawk-high-out-of-service",
    /// Squawk status: not dealt with yet.
    SQUAWK_STATUS_OPEN => "squawk-status-open",
    /// Squawk status: put off at the latest maintenance.
    SQUAWK_STATUS_DEFERRED => "squawk-status-deferred",
    /// Squawk status: signed off.
    SQUAWK_STATUS_RESOLVED => "squawk-status-resolved",
    /// Squawk: no description.
    SQUAWK_DESCRIPTION_EMPTY => "squawk-description-empty",
    /// Squawk: a description over 500 characters.
    SQUAWK_DESCRIPTION_TOO_LONG => "squawk-description-too-long",
    /// Finish: an outstanding squawk left as it was.
    SQUAWK_UNDISPOSITIONED => "squawk-undispositioned",
    /// A squawk dispositioned twice in one event.
    SQUAWK_DISPOSITIONED_TWICE => "squawk-dispositioned-twice",
    /// A disposition of a squawk that isn't outstanding.
    SQUAWK_NOT_OUTSTANDING => "squawk-not-outstanding",
    /// Requirement: a meter rule of zero or less.
    REQUIREMENT_BAD_INTERVAL => "requirement-bad-interval",
    /// Requirement: a calendar rule out of its unit's range.
    REQUIREMENT_BAD_CALENDAR => "requirement-bad-calendar",
    /// Requirement: two rules of one kind.
    REQUIREMENT_REPEATED_RULE => "requirement-repeated-rule",
    /// Bundle: no requirements.
    BUNDLE_EMPTY => "bundle-empty",
    /// Bundle: a repeated or unknown requirement.
    BUNDLE_BAD_ITEM => "bundle-bad-item",
    /// Maintenance: a part that doesn't fit.
    EVENT_BAD_PART => "event-bad-part",
    /// Maintenance: a misplaced or invalid next due date.
    EVENT_BAD_NEXT_DUE => "event-bad-next-due",
    /// Maintenance: resetting from end readings that don't exist.
    EVENT_NO_END_READINGS => "event-no-end-readings",
    /// Maintenance: an end reading below the start.
    EVENT_END_BELOW_START => "event-end-below-start",
    /// Maintenance: Finish on a fuel-up or reading, or Finalize now on a job.
    EVENT_FINISH_JOBS_ONLY => "event-finish-jobs-only",
    /// Squawk: changing or removing one that a job has already dealt with.
    SQUAWK_DISPOSITIONED => "squawk-dispositioned",
    /// Requirement: removing one a task has done.
    REQUIREMENT_IN_USE => "requirement-in-use",
    /// Copy schedule: the vehicle already has requirements.
    SCHEDULE_NOT_EMPTY => "schedule-not-empty",
    /// Message title: a forgotten maintenance or repair draft.
    INBOX_FORGOTTEN_JOB => "inbox-forgotten-job",
    /// Message text: a forgotten maintenance or repair draft.
    INBOX_FORGOTTEN_JOB_BODY => "inbox-forgotten-job-body",
    /// Message button: open the forgotten job.
    INBOX_OPEN_JOB => "inbox-open-job",
    /// What squawks are called (#064): "issue", with its plural, capitalized forms, and article
    /// as attributes.
    SQUAWK_WORD_ISSUE => "squawk-word-issue",
    /// What squawks are called: "squawk".
    SQUAWK_WORD_SQUAWK => "squawk-word-squawk",
    /// What squawks are called: "fault".
    SQUAWK_WORD_FAULT => "squawk-word-fault",
    /// What squawks are called: "write-up".
    SQUAWK_WORD_WRITE_UP => "squawk-word-write-up",
    /// Settings: what squawks are called.
    SETTING_SQUAWK_WORD => "setting-squawk-word",
    /// Settings: one choice of what squawks are called, e.g. "Issues".
    SQUAWK_WORD_CHOICE => "squawk-word-choice",
    /// Vehicle tab: its squawks.
    TAB_SQUAWKS => "tab-squawks",
    /// Vehicle tab: its squawks, with how many are outstanding; takes `count`.
    TAB_SQUAWKS_COUNT => "tab-squawks-count",
    /// Top bar title: one squawk's page.
    SCREEN_SQUAWK => "screen-squawk",
    /// Button: open a new squawk.
    SQUAWK_NEW => "squawk-new",
    /// Event page button: open a squawk found on this event.
    SQUAWK_REPORT => "squawk-report",
    /// Sheet title for a new squawk; takes `vehicle`.
    SQUAWK_SHEET_TITLE => "squawk-sheet-title",
    /// Sheet title when changing a squawk.
    SQUAWK_SHEET_EDIT_TITLE => "squawk-sheet-edit-title",
    /// Sheet: the description's label.
    SQUAWK_DESCRIPTION_LABEL => "squawk-description-label",
    /// Sheet: the severity's label.
    SQUAWK_SEVERITY_LABEL => "squawk-severity-label",
    /// Sheet: what High means; takes `word` (e.g. Not drivable).
    SQUAWK_HIGH_NOTE => "squawk-high-note",
    /// Sheet: unfold the notes and when it was noticed.
    SQUAWK_MORE => "squawk-more",
    /// Sheet: the notes' label.
    SQUAWK_NOTES_LABEL => "squawk-notes-label",
    /// Sheet: the date it was noticed.
    SQUAWK_NOTICED_DATE => "squawk-noticed-date",
    /// A squawk that's gone, or can't be seen.
    SQUAWK_NOT_FOUND => "squawk-not-found",
    /// Overview card: nothing outstanding.
    SQUAWKS_NONE_OUTSTANDING => "squawks-none-outstanding",
    /// Overview card: to the Squawks tab.
    SQUAWKS_ALL => "squawks-all",
    /// Squawks tab filter; takes `count`.
    SQUAWKS_FILTER_OUTSTANDING => "squawks-filter-outstanding",
    /// Squawks tab filter; takes `count`.
    SQUAWKS_FILTER_RESOLVED => "squawks-filter-resolved",
    /// Squawks tab filter: everything.
    SQUAWKS_FILTER_ALL => "squawks-filter-all",
    /// Squawks tab: nothing under this filter.
    SQUAWKS_NONE_HERE => "squawks-none-here",
    /// Squawks tab: nothing chosen yet.
    SQUAWKS_SELECT_HINT => "squawks-select-hint",
    /// A squawk's page: who opened it; takes `name` and `date`.
    SQUAWK_OPENED_BY => "squawk-opened-by",
    /// A squawk's page: the event it was found on; takes `event`.
    SQUAWK_FOUND_ON => "squawk-found-on",
    /// The found-on event, until it's loaded.
    SQUAWK_FOUND_ON_EVENT => "squawk-found-on-event",
    /// A squawk's page: its notes.
    SQUAWK_NOTES_HEADING => "squawk-notes-heading",
    /// A squawk's page: its history.
    SQUAWK_HISTORY_HEADING => "squawk-history-heading",
    /// History: opened; takes `severity`.
    SQUAWK_HISTORY_OPENED => "squawk-history-opened",
    /// History: a job signed it off.
    SQUAWK_HISTORY_SIGNED_OFF => "squawk-history-signed-off",
    /// History: a job deferred it; takes `reason`.
    SQUAWK_HISTORY_DEFERRED => "squawk-history-deferred",
    /// History: the job, linked; takes `date`.
    SQUAWK_HISTORY_JOB => "squawk-history-job",
    /// A squawk a job has dealt with.
    SQUAWK_LOCKED_NOTE => "squawk-locked-note",
    /// Button: remove a squawk opened by mistake.
    SQUAWK_REMOVE => "squawk-remove",
    /// Remove dialog: its title.
    SQUAWK_REMOVE_TITLE => "squawk-remove-title",
    /// Remove dialog: what it does.
    SQUAWK_REMOVE_TEXT => "squawk-remove-text",
    /// + Log: the squawk tile's line.
    LOG_SQUAWK_DETAIL => "log-squawk-detail",
    /// Setup: where the maintenance schedule is.
    SETUP_SCHEDULE_MOVED => "setup-schedule-moved",
    /// Maintenance tab view: what's tracked.
    MAINTENANCE_SCHEDULE => "maintenance-schedule",
    /// Maintenance tab view: the maintenance and repairs.
    MAINTENANCE_JOBS => "maintenance-jobs",
    /// Maintenance tab view: setting the schedule up.
    MAINTENANCE_SET_UP => "maintenance-set-up",
    /// Jobs: the drafts' heading.
    TIMELINE_IN_PROGRESS => "timeline-in-progress",
    /// A job's row: how many tasks; takes `count`.
    TIMELINE_TASK_COUNT => "timeline-task-count",
    /// Jobs: none yet.
    JOBS_EMPTY => "jobs-empty",
    /// Rules: periodic ones; takes `rules` (e.g. 5,000 mi or 6 months).
    SCHEDULE_EVERY => "schedule-every",
    /// Rules: between two rules.
    SCHEDULE_OR => "schedule-or",
    /// Rules: a calendar rule; takes `count`.
    SCHEDULE_MONTHS => "schedule-months",
    /// Rules: a calendar rule in days; takes `count`.
    SCHEDULE_DAYS => "schedule-days",
    /// Rules: a calendar rule in years; takes `count`.
    SCHEDULE_YEARS => "schedule-years",
    /// Rules: a fixed date with no date yet.
    SCHEDULE_FIXED => "schedule-fixed",
    /// Rules: a fixed date; takes `date`.
    SCHEDULE_FIXED_NEXT => "schedule-fixed-next",
    /// Rules: none.
    SCHEDULE_WHEN_NEEDED => "schedule-when-needed",
    /// Schedule card: when it was last done.
    SCHEDULE_LAST_DONE => "schedule-last-done",
    /// Schedule card: never done.
    SCHEDULE_NOT_DONE => "schedule-not-done",
    /// Schedule card: the job that last did it.
    SCHEDULE_THE_JOB => "schedule-the-job",
    /// Schedule: turned-off requirements; takes `names`.
    SCHEDULE_TURNED_OFF => "schedule-turned-off",
    /// Schedule and Set up: the bundles.
    SCHEDULE_BUNDLES => "schedule-bundles",
    /// Set up: the requirements.
    SCHEDULE_REQUIREMENTS => "schedule-requirements",
    /// Schedule: nothing set up.
    SCHEDULE_EMPTY_TITLE => "schedule-empty-title",
    /// Schedule: what to set up.
    SCHEDULE_EMPTY_HINT => "schedule-empty-hint",
    /// Schedule: nothing set up, for those who can't.
    SCHEDULE_NOTHING_TRACKED => "schedule-nothing-tracked",
    /// Schedule: to Set up.
    SCHEDULE_SET_IT_UP => "schedule-set-it-up",
    /// Button: copy a schedule from another vehicle.
    SCHEDULE_COPY_FROM => "schedule-copy-from",
    /// Button: add a requirement.
    SCHEDULE_ADD_REQUIREMENT => "schedule-add-requirement",
    /// Button: add a bundle.
    SCHEDULE_ADD_BUNDLE => "schedule-add-bundle",
    /// Set up: no requirements.
    SCHEDULE_NONE_YET => "schedule-none-yet",
    /// Set up: no bundles.
    SCHEDULE_NO_BUNDLES => "schedule-no-bundles",
    /// Set up: a requirement turned off.
    SCHEDULE_OFF => "schedule-off",
    /// Set up: about removing.
    SCHEDULE_REMOVE_NOTE => "schedule-remove-note",
    /// Requirement dialog: adding one.
    SCHEDULE_ADD_REQUIREMENT_TITLE => "schedule-add-requirement-title",
    /// Requirement and bundle dialogs: the name.
    SCHEDULE_NAME => "schedule-name",
    /// Requirement dialog: whether it's on.
    SCHEDULE_IN_USE => "schedule-in-use",
    /// Requirement dialog: the rules' heading.
    SCHEDULE_DUE_WHEN => "schedule-due-when",
    /// Requirement dialog: add a meter rule.
    SCHEDULE_ADD_METER_RULE => "schedule-add-meter-rule",
    /// Requirement dialog: add a calendar rule.
    SCHEDULE_ADD_CALENDAR_RULE => "schedule-add-calendar-rule",
    /// Requirement dialog: add a fixed date.
    SCHEDULE_ADD_FIXED_RULE => "schedule-add-fixed-rule",
    /// Requirement dialog: about the rules.
    SCHEDULE_RULES_HINT => "schedule-rules-hint",
    /// A rule's inputs: every.
    SCHEDULE_EVERY_LABEL => "schedule-every-label",
    /// A rule's kind: the calendar.
    SCHEDULE_CALENDAR => "schedule-calendar",
    /// A calendar rule's unit: months.
    SCHEDULE_MONTHS_LABEL => "schedule-months-label",
    /// A calendar rule's unit: days.
    SCHEDULE_UNIT_DAYS => "schedule-unit-days",
    /// A calendar rule's unit: years.
    SCHEDULE_UNIT_YEARS => "schedule-unit-years",
    /// A calendar rule's unit menu, for screen readers.
    SCHEDULE_CALENDAR_UNIT => "schedule-calendar-unit",
    /// A calendar rule: due by the month's end.
    SCHEDULE_END_OF_MONTH => "schedule-end-of-month",
    /// A rule's kind: a fixed date.
    SCHEDULE_FIXED_LABEL => "schedule-fixed-label",
    /// A fixed date's first date.
    SCHEDULE_FIRST_DUE => "schedule-first-due",
    /// Bundle dialog: adding one.
    SCHEDULE_ADD_BUNDLE_TITLE => "schedule-add-bundle-title",
    /// Bundle dialog: changing one.
    SCHEDULE_EDIT_BUNDLE_TITLE => "schedule-edit-bundle-title",
    /// Bundle dialog: its requirements.
    SCHEDULE_BUNDLE_COVERS => "schedule-bundle-covers",
    /// Schedule: a bundle card's requirements; takes `names`.
    SCHEDULE_BUNDLE_COVERS_LIST => "schedule-bundle-covers-list",
    /// Schedule: a bundle card's label.
    SCHEDULE_BUNDLE_LABEL => "schedule-bundle-label",
    /// Schedule: how many requirements a bundle covers; takes `count`.
    SCHEDULE_BUNDLE_COUNT => "schedule-bundle-count",
    /// Remove dialog; takes `name`.
    SCHEDULE_REMOVE_TITLE => "schedule-remove-title",
    /// Copy dialog; takes `vehicle`.
    SCHEDULE_COPY_TITLE => "schedule-copy-title",
    /// Copy dialog: the vehicle to copy from.
    SCHEDULE_COPY_FROM_LABEL => "schedule-copy-from-label",
    /// Copy dialog: no other vehicles.
    SCHEDULE_COPY_NO_VEHICLES => "schedule-copy-no-vehicles",
    /// Copy dialog: what comes; takes `requirements` and `bundles`.
    SCHEDULE_COPY_SUMMARY => "schedule-copy-summary",
    /// Copy dialog: meter rules left out; takes `vehicle`.
    SCHEDULE_COPY_LEFT_OUT => "schedule-copy-left-out",
    /// Copy dialog's button.
    SCHEDULE_COPY => "schedule-copy",
    /// + Log: the maintenance tile's line.
    LOG_MAINTENANCE_DETAIL => "log-maintenance-detail",
    /// + Log: the repair tile's line.
    LOG_REPAIR_DETAIL => "log-repair-detail",
    /// A job's draft status (it never locks by itself).
    EVENT_STATUS_JOB_DRAFT => "event-status-job-draft",
    /// A squawk's page: start a repair of it.
    SQUAWK_REPAIR_THIS => "squawk-repair-this",
    /// Work log: the readings at the end.
    WORK_END_READINGS => "work-end-readings",
    /// Work log: ask for readings at the end.
    WORK_ADD_END_READINGS => "work-add-end-readings",
    /// Work log: one end reading's label; takes `meter` and `unit`.
    WORK_END_READING => "work-end-reading",
    /// Work log: the tasks.
    WORK_TASKS => "work-tasks",
    /// Work log: add a task from a requirement.
    WORK_FROM_REQUIREMENT => "work-from-requirement",
    /// Work log: add a bundle's tasks.
    WORK_FROM_BUNDLE => "work-from-bundle",
    /// Work log: add a typed task.
    WORK_ADD_TASK => "work-add-task",
    /// Work log: no tasks yet.
    WORK_NO_TASKS => "work-no-tasks",
    /// A task with no title yet.
    WORK_UNTITLED_TASK => "work-untitled-task",
    /// The open task's fold button, for screen readers.
    WORK_FOLD_TASK => "work-fold-task",
    /// A folded task: how many parts; takes `count`.
    WORK_PART_COUNT => "work-part-count",
    /// A task's title, as a placeholder.
    WORK_TASK_TITLE => "work-task-title",
    /// A task's requirement; takes `name`.
    WORK_TASK_FOR => "work-task-for",
    /// A task's parts.
    WORK_PARTS => "work-parts",
    /// A part: what it is.
    WORK_PART_WHAT => "work-part-what",
    /// A part: its number.
    WORK_PART_NUMBER => "work-part-number",
    /// A part: how many.
    WORK_PART_QUANTITY => "work-part-quantity",
    /// A part: the price of one.
    WORK_PART_PRICE => "work-part-price",
    /// Add a part.
    WORK_ADD_PART => "work-add-part",
    /// A task's labor time.
    WORK_LABOR_TIME => "work-labor-time",
    /// A task's labor cost; takes `currency`.
    WORK_LABOR_COST => "work-labor-cost",
    /// Which readings reset the requirement.
    WORK_RESETS_FROM => "work-resets-from",
    /// Resets from the start readings.
    WORK_RESETS_START => "work-resets-start",
    /// Resets from the end readings.
    WORK_RESETS_END => "work-resets-end",
    /// A fixed-date rule's next due date.
    WORK_NEXT_DUE => "work-next-due",
    /// A task's total; takes `total`.
    WORK_TASK_TOTAL => "work-task-total",
    /// Work log: the squawks to deal with; takes `count`.
    WORK_SQUAWKS => "work-squawks",
    /// Work log: no open squawks.
    WORK_NO_SQUAWKS => "work-no-squawks",
    /// Work log: open a squawk found during the job.
    WORK_OPEN_SQUAWK => "work-open-squawk",
    /// A squawk: fixed.
    WORK_SIGN_OFF => "work-sign-off",
    /// A squawk: put off.
    WORK_DEFER => "work-defer",
    /// A sign-off: the task that fixed it.
    WORK_FIXED_BY => "work-fixed-by",
    /// A sign-off with no task.
    WORK_FIXED_BY_NONE => "work-fixed-by-none",
    /// A deferral's reason.
    WORK_DEFER_WHY => "work-defer-why",
    /// A disposition keeping the severity.
    WORK_SEVERITY_SAME => "work-severity-same",
    /// Work log: a repair's own section.
    WORK_REPAIR => "work-repair",
    /// A repair's complaint.
    WORK_COMPLAINT => "work-complaint",
    /// A repair's cause.
    WORK_CAUSE => "work-cause",
    /// About a repair's tasks.
    WORK_REPAIR_NOTE => "work-repair-note",
    /// Work log: the total.
    WORK_TOTAL => "work-total",
    /// Work log: save without finishing.
    WORK_SAVE_DRAFT => "work-save-draft",
    /// Finish a job.
    WORK_FINISH => "work-finish",
    /// Finish dialog: its title.
    WORK_FINISH_TITLE => "work-finish-title",
    /// Finish check: readings given.
    WORK_CHECK_READINGS_OK => "work-check-readings-ok",
    /// Finish check: readings missing.
    WORK_CHECK_READINGS_MISSING => "work-check-readings-missing",
    /// Finish check: a place.
    WORK_CHECK_PLACE_OK => "work-check-place-ok",
    /// Finish check: no place.
    WORK_CHECK_PLACE_MISSING => "work-check-place-missing",
    /// Finish check: a squawk not dealt with; takes `description`.
    WORK_CHECK_UNDECIDED => "work-check-undecided",
    /// Finish dialog: what it does.
    WORK_FINISH_LOCKS => "work-finish-locks",
    /// Finish dialog: back to the form.
    WORK_FINISH_BACK => "work-finish-back",
    /// Finish dialog: finish.
    WORK_FINISH_LOCK => "work-finish-lock",
    /// A finished job's status; takes `date`.
    EVENT_STATUS_JOB_FINISHED => "event-status-job-finished",
    /// A job's tile: the total.
    JOB_TOTAL => "job-total",
    /// A job's total, split; takes `parts` and `labor`.
    JOB_SPLIT => "job-split",
    /// A job's labor.
    JOB_LABOR => "job-labor",
    /// A part's or labor's cost, as a column.
    JOB_COST => "job-cost",
    /// A job with no tasks.
    JOB_NO_TASKS => "job-no-tasks",
    /// A task reset its requirement from the start readings.
    JOB_RESETS_START => "job-resets-start",
    /// A task reset its requirement from the end readings.
    JOB_RESETS_END => "job-resets-end",
    /// A task's next due date; takes `date`.
    JOB_NEXT_DUE => "job-next-due",
    /// A task's notes; takes `notes`.
    JOB_NOTES => "job-notes",
    /// A job's page: the squawks it dealt with.
    JOB_SQUAWKS => "job-squawks",
    /// A sign-off's task; takes `task`.
    JOB_FIXED_BY => "job-fixed-by",
    /// A deferral, on a job's page.
    JOB_DEFERRED => "job-deferred",
    /// A squawk found during the job; takes `description` and `severity`.
    JOB_FOUND => "job-found",
    /// A job's row: squawks signed off; takes `count`.
    TIMELINE_SIGNED_OFF => "timeline-signed-off",
    /// A job's row: squawks deferred; takes `count`.
    TIMELINE_DEFERRED => "timeline-deferred",
    /// Settings: the word is the household's, set by its Admin.
    SETTING_SQUAWK_WORD_HOUSEHOLD => "setting-squawk-word-household",
}
