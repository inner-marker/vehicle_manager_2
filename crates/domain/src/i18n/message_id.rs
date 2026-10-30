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
    /// Vehicle detail: which vehicle is shown; takes `id` (text).
    VEHICLE_DETAIL_ID => "vehicle-detail-id",
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
}
