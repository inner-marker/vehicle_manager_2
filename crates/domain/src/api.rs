//! The client ↔ server API contract: version, headers, handshake, and refusal reasons.
//!
//! The **API version** is separate from the app version (plan §8): it changes only
//! when the contract between app and server changes. Every request to a path under
//! [`API_PATH_PREFIX`] carries the client's API version in [`API_VERSION_HEADER`]
//! and is made by a signed-in session: the session cookie (web) or `Authorization: Bearer
//! <session token>` (desktop).
//! The server checks both before any server function runs (phase-1.md step 7).
//!
//! # Examples
//!
//! ```
//! use domain::api::{API_VERSION, ApiError, check_client_api_version};
//!
//! assert_eq!(check_client_api_version(API_VERSION), Ok(()));
//! assert_eq!(check_client_api_version(API_VERSION + 1), Err(ApiError::ClientTooNew));
//! ```

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, messages};

/// The API version this build speaks.
pub const API_VERSION: u32 = 1;

/// The oldest client API version this build's server still accepts.
pub const MIN_SUPPORTED_API_VERSION: u32 = 1;

/// Header carrying the client's API version (a plain integer, e.g. `1`).
pub const API_VERSION_HEADER: &str = "x-api-version";

/// Every server function lives under this path, and the auth layer guards all of it.
pub const API_PATH_PREFIX: &str = "/api/";

/// Why the server refused a request. Sent as a kebab-case string (e.g.
/// `"invalid-credential"`) so the client can show a translated message.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ApiError {
    /// No session was sent: not signed in.
    MissingCredential,
    /// A session was sent but isn't valid (ended, signed out, or unknown).
    InvalidCredential,
    /// The server can't check sessions (its database isn't open), so it refuses (fail closed).
    AuthNotConfigured,
    /// No API version header.
    MissingApiVersion,
    /// The API version header isn't a whole number.
    MalformedApiVersion,
    /// The client's API version is older than the server still supports: update the app.
    ClientTooOld,
    /// The client's API version is newer than the server's: update the server first.
    ClientTooNew,
}

impl ApiError {
    /// Every reason, for exhaustive tests and the UI.
    pub const ALL: [Self; 7] = [
        Self::MissingCredential,
        Self::InvalidCredential,
        Self::AuthNotConfigured,
        Self::MissingApiVersion,
        Self::MalformedApiVersion,
        Self::ClientTooOld,
        Self::ClientTooNew,
    ];

    /// The HTTP status the server responds with.
    pub const fn http_status(self) -> u16 {
        match self {
            Self::MissingCredential | Self::InvalidCredential => 401,
            Self::AuthNotConfigured => 503,
            Self::MissingApiVersion | Self::MalformedApiVersion | Self::ClientTooNew => 400,
            // 426 Upgrade Required: the client must update to talk to this server.
            Self::ClientTooOld => 426,
        }
    }

    /// The user-facing explanation, through the translation layer.
    pub const fn message(self) -> MessageId {
        match self {
            Self::MissingCredential => messages::API_ERROR_MISSING_CREDENTIAL,
            Self::InvalidCredential => messages::API_ERROR_INVALID_CREDENTIAL,
            Self::AuthNotConfigured => messages::API_ERROR_AUTH_NOT_CONFIGURED,
            Self::MissingApiVersion => messages::API_ERROR_MISSING_API_VERSION,
            Self::MalformedApiVersion => messages::API_ERROR_MALFORMED_API_VERSION,
            Self::ClientTooOld => messages::API_ERROR_CLIENT_TOO_OLD,
            Self::ClientTooNew => messages::API_ERROR_CLIENT_TOO_NEW,
        }
    }

    /// Whether this is a sign-in problem (as opposed to a version problem).
    pub const fn is_auth_failure(self) -> bool {
        matches!(
            self,
            Self::MissingCredential | Self::InvalidCredential | Self::AuthNotConfigured
        )
    }

    /// A short English description for logs and the response's `message` field.
    /// Never shown in the UI, which uses [`ApiError::message`].
    pub const fn description(self) -> &'static str {
        match self {
            Self::MissingCredential => "not signed in",
            Self::InvalidCredential => "session not valid",
            Self::AuthNotConfigured => "server can't check sessions",
            Self::MissingApiVersion => "missing API version header",
            Self::MalformedApiVersion => "malformed API version header",
            Self::ClientTooOld => "client API version is too old",
            Self::ClientTooNew => "client API version is newer than the server",
        }
    }
}

/// Whether a client speaking `client_version` may talk to this server.
///
/// # Errors
///
/// [`ApiError::ClientTooOld`] below [`MIN_SUPPORTED_API_VERSION`];
/// [`ApiError::ClientTooNew`] above [`API_VERSION`] (the server must be upgraded first).
pub const fn check_client_api_version(client_version: u32) -> Result<(), ApiError> {
    if client_version < MIN_SUPPORTED_API_VERSION {
        Err(ApiError::ClientTooOld)
    } else if client_version > API_VERSION {
        Err(ApiError::ClientTooNew)
    } else {
        Ok(())
    }
}

/// Parses the [`API_VERSION_HEADER`] value.
///
/// # Errors
///
/// [`ApiError::MalformedApiVersion`] if it isn't a whole, non-negative number.
pub fn parse_api_version(header_value: &str) -> Result<u32, ApiError> {
    header_value
        .trim()
        .parse()
        .map_err(|_| ApiError::MalformedApiVersion)
}

/// What the server reports about itself in the handshake.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Handshake {
    /// The server's app version (e.g. `0.1.0`).
    pub app_version: String,
    /// The API version the server speaks.
    pub api_version: u32,
    /// The oldest client API version the server accepts.
    pub min_supported_api_version: u32,
    /// The server's clock, as Unix epoch milliseconds (UTC).
    pub server_time_ms: i64,
    /// Whether the server's database is open and passed its last check. An unhealthy
    /// server refuses changes. Missing from servers before Phase 2, which counts as healthy.
    #[serde(default = "healthy_by_default")]
    pub healthy: bool,
    /// The server's exact build (phase-3.md 1c). Missing from older servers, which reads as
    /// unknown.
    #[serde(default)]
    pub build: crate::build_info::BuildInfo,
}

/// Who is signed in, as the client sees it (from the session check and sign-in).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionInfo {
    /// Their username.
    pub username: crate::user::Username,
    /// Their display name.
    pub display_name: String,
    /// They must choose a new password before anything else.
    pub must_change_password: bool,
    /// The mode the session is in.
    pub mode: crate::access::ActiveMode,
    /// The modes they may switch to (empty for a Viewer-only user, who works read-only).
    pub available_modes: Vec<crate::access::ActiveMode>,
    /// The server's timer lengths (normal, or test timers on a test build; #054), so every
    /// device times out the same way.
    #[serde(default)]
    pub timers: crate::timers::Timers,
}

/// The session check's answer: who is signed in (if anyone), and whether the server still
/// needs its first Admin.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SessionState {
    /// The signed-in user, or `None`.
    pub signed_in: Option<SessionInfo>,
    /// No users exist yet: the app should offer first-Admin setup instead of sign-in.
    pub setup_needed: bool,
    /// The server's instance name, e.g. `test` (decision #062); `None` on the main server, and
    /// from servers before it existed.
    #[serde(default)]
    pub instance: Option<String>,
}

/// The answer to a first-Admin setup attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SetupOutcome {
    /// The Admin was created and is signed in (as for [`SignInOutcome::SignedIn`]).
    SignedIn {
        /// The new Admin.
        session: SessionInfo,
        /// The session token, if asked for (the desktop app).
        token: Option<String>,
    },
    /// The setup code is wrong.
    WrongCode,
    /// Too many wrong codes; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The username breaks a rule.
    InvalidUsername {
        /// Which rule.
        reason: crate::user::UsernameError,
    },
    /// The display name breaks a rule.
    InvalidDisplayName {
        /// Which rule.
        reason: crate::user::DisplayNameError,
    },
    /// The password breaks a rule.
    InvalidPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// An Admin already exists: sign in instead.
    AlreadySetUp,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to setup step 1, checking the setup code.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum CheckSetupCodeOutcome {
    /// The code is right: go on to step 2.
    Valid,
    /// The code is wrong.
    WrongCode,
    /// Too many wrong codes; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// An Admin already exists: sign in instead.
    AlreadySetUp,
}

/// Whether a user can sign in, and with what.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AccountStatus {
    /// Signs in with their own password.
    Active,
    /// Has a temporary password they must change at their next sign-in.
    TemporaryPassword,
    /// Can't sign in.
    Disabled,
}

/// One user, as the Users screen (Admin mode) shows them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UserSummary {
    /// The user's ID.
    pub id: crate::user::UserId,
    /// Their username.
    pub username: crate::user::Username,
    /// Their display name.
    pub display_name: String,
    /// Whether they can sign in, and with what.
    pub status: AccountStatus,
    /// Their role on all vehicles, if any.
    pub role_on_all: Option<crate::access::Role>,
    /// Every role they have: on all vehicles, a group, or one vehicle (#058), the highest
    /// for each, in that order.
    pub roles: Vec<(crate::access::GrantTarget, crate::access::Role)>,
    /// This is the signed-in Admin.
    pub is_you: bool,
    /// They're the last enabled Admin: they can't be disabled or lose the Admin role.
    pub is_last_admin: bool,
}

/// The answer to a Users screen request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum UserAdminOutcome {
    /// The household's users, by username.
    Users {
        /// Everyone (not deleted).
        users: Vec<UserSummary>,
    },
    /// Done.
    Done,
    /// Done; give this temporary password to the user. Shown once.
    TemporaryPassword {
        /// The password, e.g. `QX7M-K2PD-9WRT`, typed exactly as shown.
        password: String,
    },
    /// Not in Admin mode, or not the household's Admin.
    NotAllowed,
    /// It would leave the household without an enabled Admin.
    LastAdmin,
    /// Someone already has that username.
    UsernameTaken,
    /// The username breaks a rule.
    InvalidUsername {
        /// Which rule.
        reason: crate::user::UsernameError,
    },
    /// The display name breaks a rule.
    InvalidDisplayName {
        /// Which rule.
        reason: crate::user::DisplayNameError,
    },
    /// There's no such user.
    NotFound,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to completing a password recovery (decision #047).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RecoveryOutcome {
    /// The new password is set; signed in (as for [`SignInOutcome::SignedIn`]).
    SignedIn {
        /// Who signed in.
        session: SessionInfo,
        /// The session token, if asked for (the desktop app).
        token: Option<String>,
    },
    /// The code is wrong, used, or expired.
    WrongCode,
    /// Too many attempts; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The new password breaks a rule.
    InvalidPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// Someone's display preferences, as the Settings screen shows them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreferencesView {
    /// What they chose themselves (unchosen settings follow the household).
    pub mine: crate::preferences::PreferenceChoices,
    /// What the household chose (unchosen settings follow the built-in defaults).
    pub household: crate::preferences::PreferenceChoices,
    /// What's in effect for them.
    pub effective: crate::preferences::Preferences,
}

/// The answer to a Settings request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum PreferencesOutcome {
    /// The preferences (after any change).
    Preferences {
        /// Mine, the household's, and the effective ones.
        view: PreferencesView,
    },
    /// Household defaults need the household's Admin in Admin mode.
    NotAllowed,
    /// The server couldn't do it right now.
    Unavailable,
}

/// A group in the Garage, with how many of its vehicles the person can see.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GroupSummary {
    /// Its ID.
    pub id: crate::group::GroupId,
    /// Its name and icon.
    pub group: crate::group::Group,
    /// How many of its vehicles the person can see.
    pub vehicles: usize,
}

/// A vehicle the person can see, with their role on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetSummary {
    /// Its ID.
    pub id: crate::asset::AssetId,
    /// Its details.
    pub asset: crate::asset::Asset,
    /// The person's role on it.
    pub role: crate::access::Role,
    /// Its current license plate, as shown ("NC · NCX 4471"), if it has one.
    #[serde(default)]
    pub plate: Option<String>,
    /// What its lifecycle events say about it now: the status it shows (from them where they
    /// say, out for service or disposed; otherwise what Setup says, `asset.status`), and more
    /// (milestone 9, #067).
    #[serde(default)]
    pub standing: crate::lifecycle::Standing,
}

/// The Garage: the groups and vehicles the person can see, and what they may do.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GarageView {
    /// Groups with at least one visible vehicle (every group, for the household's Admin), by
    /// name.
    pub groups: Vec<GroupSummary>,
    /// The vehicles they can see, archived ones included, by name.
    pub assets: Vec<AssetSummary>,
    /// They may add vehicles (the household's Admin, in Admin mode).
    pub can_add: bool,
    /// They may manage groups and move vehicles (the household's Admin, in Admin mode).
    pub can_manage_groups: bool,
    /// How they sort the Garage (#056).
    pub sort: crate::preferences::GarageSort,
    /// Their custom order, for the Custom sort.
    pub order: Vec<crate::asset::AssetId>,
}

/// One vehicle's page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssetView {
    /// Its ID.
    pub id: crate::asset::AssetId,
    /// Its details.
    pub asset: crate::asset::Asset,
    /// The person's role on it.
    pub role: crate::access::Role,
    /// They may edit, archive, or delete it now (Admin role on it, in Admin mode).
    pub can_manage: bool,
    /// They may change its meters and custom fields now (Maintainer role, in Maintainer or
    /// Admin mode).
    pub can_configure: bool,
    /// They may move it to another group (the household's Admin, in Admin mode).
    pub can_move: bool,
    /// Its meters.
    pub meters: Vec<(crate::meter::MeterId, crate::meter::Meter)>,
    /// Its custom field definitions.
    pub fields: Vec<(
        crate::custom_field::CustomFieldId,
        crate::custom_field::CustomField,
    )>,
    /// Its current license plate, as shown ("NC · NCX 4471"), if it has one.
    #[serde(default)]
    pub plate: Option<String>,
    /// What its lifecycle events say about it now (milestone 9, #067).
    #[serde(default)]
    pub standing: crate::lifecycle::Standing,
}

/// How one person's access changes in a move, for the preview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MoveChange {
    /// Who.
    pub user: crate::user::UserId,
    /// Their display name.
    pub display_name: String,
    /// Their role on the vehicle now (`None`: no access).
    pub before: Option<crate::access::Role>,
    /// Their role on it after the move.
    pub after: Option<crate::access::Role>,
}

/// What the geocoder suggests for a point (decisions #038, #059).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PlaceSuggestion {
    /// The city, state or province, and country.
    pub place: crate::location::Place,
    /// The country's name, e.g. "United States of America".
    pub country_name: String,
    /// The nearest city's time zone, if known.
    pub time_zone: Option<crate::location::TimeZone>,
    /// The point is inside the country's boundary (false: the nearest country, e.g. offshore).
    pub inside_country: bool,
    /// The data version it came from (stored with a derived place, plan §3).
    pub data_version: String,
    /// Up to five places nearby, nearest first, to choose the city from (#061).
    pub nearby: Vec<crate::location::NearbyPlace>,
}

/// What's known about a point: the person's saved place it's in, and the suggested place.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Located {
    /// The person's nearest saved place whose radius contains the point, if any.
    pub saved: Option<(crate::location::SavedPlaceId, crate::location::SavedPlace)>,
    /// The geocoder's suggestion; `None` while place lookup is unavailable.
    pub suggestion: Option<PlaceSuggestion>,
}

/// About the place data, for About on Diagnostics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceDataInfo {
    /// The data's version, e.g. `2026-11-02`.
    pub data_version: String,
    /// The credit to show (GeoNames and Natural Earth).
    pub attribution: String,
}

/// The answer to a saved places or place lookup request.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum PlacesOutcome {
    /// The person's saved places, by name.
    Places {
        /// Each place and its ID.
        places: Vec<(crate::location::SavedPlaceId, crate::location::SavedPlace)>,
    },
    /// What's known about a point.
    Located {
        /// The saved place and suggestion (boxed: much larger than the other answers).
        located: Box<Located>,
    },
    /// Every country, by name, for typing a place by hand.
    Countries {
        /// Each country's code and name.
        countries: Vec<(crate::location::CountryCode, String)>,
    },
    /// About the place data (`None` while it's unavailable).
    DataInfo {
        /// Its version and credit.
        info: Option<PlaceDataInfo>,
    },
    /// A saved place was added.
    Added {
        /// Its ID.
        id: crate::location::SavedPlaceId,
    },
    /// Done.
    Done,
    /// No such saved place of this person's.
    NotFound,
    /// The form was refused.
    Invalid {
        /// Why.
        problem: crate::location::LocationError,
    },
    /// The server couldn't do it (no database, or a storage error).
    Unavailable,
}

/// The answer to a Garage, vehicle, or group request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum AssetsOutcome {
    /// The Garage.
    Garage {
        /// What the person can see.
        view: GarageView,
    },
    /// One vehicle.
    Asset {
        /// Its page.
        view: Box<AssetView>,
    },
    /// A vehicle was added.
    Added {
        /// Its ID.
        id: crate::asset::AssetId,
    },
    /// Who gains, loses, or changes access if the vehicle moves.
    MovePreview {
        /// The people affected (empty: nobody).
        changes: Vec<MoveChange>,
    },
    /// Done.
    Done,
    /// There's no such vehicle, meter, field, or group (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The form was refused.
    Invalid {
        /// Why.
        problem: crate::asset_form::Problem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// One event as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventView {
    /// Its ID.
    pub id: crate::event::EventId,
    /// The event.
    pub event: crate::event::Event,
    /// The event that corrects it, if any (then the correction's values count).
    pub corrected_by: Option<crate::event::EventId>,
    /// The person may edit it or **Finalize now** (a draft they may manage).
    pub may_change: bool,
    /// The person may correct it (finalized, not yet corrected, and they may manage it).
    pub may_correct: bool,
    /// Who logged it, by their display name.
    pub logged_by_name: String,
    /// Who finalized it, by their display name (`None`: not yet, or the server did).
    pub finalized_by_name: Option<String>,
}

/// A meter's latest reading that counts.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct LatestReading {
    /// Which meter.
    pub meter: crate::meter::MeterId,
    /// When (the event's start).
    pub at: crate::time::UnixMillis,
    /// The value, in the meter's stored unit.
    pub value: i64,
}

/// The answer to an events request (milestone 6).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum EventsOutcome {
    /// One vehicle's events, newest first.
    Events {
        /// Each event.
        events: Vec<EventView>,
    },
    /// One event.
    Event {
        /// It (boxed: much larger than the other answers).
        view: Box<EventView>,
    },
    /// What the log form needs beyond the vehicle's page.
    LogContext {
        /// Each meter's latest reading.
        latest: Vec<LatestReading>,
        /// The person may keep a reading that doesn't fit, with a reason (the Admin, in Admin
        /// mode).
        may_override_readings: bool,
    },
    /// An event was saved (added, edited, or a correction made).
    Saved {
        /// Its ID.
        id: crate::event::EventId,
        /// What's worth telling the person (it was saved anyway).
        warnings: Vec<crate::event_check::Warning>,
    },
    /// Done.
    Done,
    /// There's no such vehicle or event (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The event was refused.
    Invalid {
        /// Why.
        problem: crate::event::EventProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// What opening or changing a squawk sends (milestone 7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquawkInput {
    /// Which vehicle (fixed once opened).
    pub asset: crate::asset::AssetId,
    /// What's wrong.
    pub description: crate::squawk::SquawkText,
    /// More detail.
    pub notes: crate::notes::Notes,
    /// How serious it is.
    pub severity: crate::squawk::Severity,
    /// When it was noticed.
    pub opened_at: crate::time::UnixMillis,
    /// The event it was found on, if any (fixed once opened).
    pub found_on: Option<crate::event::EventId>,
}

/// One squawk as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SquawkView {
    /// Its ID.
    pub id: crate::squawk::SquawkId,
    /// The squawk, as opened (and edited).
    pub squawk: crate::squawk::Squawk,
    /// Where it stands now.
    pub status: crate::squawk::SquawkStatus,
    /// Its severity now (the latest disposition's change, or as opened).
    pub severity: crate::squawk::Severity,
    /// Its dispositions that count, oldest first.
    pub history: Vec<crate::squawk::CountedDisposition>,
    /// Who opened it, by their display name.
    pub opened_by_name: String,
    /// The person may change or remove it (it's still open, no job has dealt with it, and
    /// they opened it or are a Maintainer).
    pub may_change: bool,
}

/// The answer to a squawk request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SquawksOutcome {
    /// One vehicle's squawks, newest first.
    Squawks {
        /// Each squawk.
        squawks: Vec<SquawkView>,
        /// The person may open one on this vehicle (a Driver or higher, in that mode).
        may_open: bool,
    },
    /// One squawk.
    Squawk {
        /// It.
        view: Box<SquawkView>,
    },
    /// A squawk was opened or changed.
    Saved {
        /// Its ID.
        id: crate::squawk::SquawkId,
    },
    /// Done.
    Done,
    /// There's no such vehicle, squawk, or event (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The squawk was refused.
    Invalid {
        /// Why.
        problem: crate::squawk::SquawkProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

// --- Attachments (milestone 8) ------------------------------------------------------------

/// The path files are uploaded to (a plain route, not a server function: its body limit is
/// raised to fit a PDF, #037). The body is the file; the query says what it's for.
pub const UPLOAD_PATH: &str = "/api/v1/files/upload";

/// What an upload is for, as its query (`?for=photo&size=medium`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "for", rename_all = "kebab-case")]
pub enum UploadPurpose {
    /// A photo, kept at `size` (the server converts or shrinks it if needed, #036).
    Photo {
        /// The size picked.
        size: crate::attachment::PhotoSize,
    },
    /// A photo's original, as taken (#036).
    Original,
    /// A photo's metadata as read (EXIF).
    PhotoMetadata,
    /// A PDF (#037).
    Document,
    /// A PDF's first page as a JPEG, which the app draws.
    DocumentThumbnail,
}

impl UploadPurpose {
    /// The query string, without the `?`.
    #[must_use]
    pub fn to_query(self) -> String {
        match self {
            Self::Photo { size } => format!(
                "for=photo&size={}",
                match size {
                    crate::attachment::PhotoSize::Small => "small",
                    crate::attachment::PhotoSize::Medium => "medium",
                    crate::attachment::PhotoSize::Large => "large",
                }
            ),
            Self::Original => "for=original".to_owned(),
            Self::PhotoMetadata => "for=photo-metadata".to_owned(),
            Self::Document => "for=document".to_owned(),
            Self::DocumentThumbnail => "for=document-thumbnail".to_owned(),
        }
    }

    /// The purpose from a query string (without the `?`), if it's one.
    #[must_use]
    pub fn from_query(query: &str) -> Option<Self> {
        let value = |key: &str| {
            query
                .split('&')
                .filter_map(|pair| pair.split_once('='))
                .find(|(name, _)| *name == key)
                .map(|(_, value)| value)
        };
        match value("for")? {
            "photo" => Some(Self::Photo {
                size: match value("size")? {
                    "small" => crate::attachment::PhotoSize::Small,
                    "medium" => crate::attachment::PhotoSize::Medium,
                    "large" => crate::attachment::PhotoSize::Large,
                    _ => return None,
                },
            }),
            "original" => Some(Self::Original),
            "photo-metadata" => Some(Self::PhotoMetadata),
            "document" => Some(Self::Document),
            "document-thumbnail" => Some(Self::DocumentThumbnail),
            _ => None,
        }
    }
}

/// A file the server stored from an upload, with what it found out about it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UploadedFile {
    /// The file as stored (a photo as the server kept it: a correctly sized JPEG).
    pub file: crate::attachment::StoredFile,
    /// What it is (none for a photo's metadata).
    pub file_type: Option<crate::attachment::FileType>,
    /// The thumbnail the server made (photos and PDF thumbnails).
    pub thumbnail: Option<crate::attachment::StoredFile>,
    /// A photo's width and height, in pixels.
    pub pixels: Option<(u32, u32)>,
    /// A PDF's page count.
    pub pages: Option<u32>,
}

/// The answer to an upload.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum UploadOutcome {
    /// Stored.
    Uploaded {
        /// What was stored.
        uploaded: UploadedFile,
    },
    /// The person may not upload (nobody signed in, or no role that adds attachments).
    NotAllowed,
    /// The file was refused.
    Invalid {
        /// Why.
        problem: crate::attachment::AttachmentProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// Adding an attachment from files already uploaded. The server works out a photo's pixels
/// and a PDF's pages from the stored files, whatever `content` says.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentInput {
    /// Which vehicle.
    pub asset: crate::asset::AssetId,
    /// What it goes on.
    pub owner: crate::attachment::AttachmentOwner,
    /// The name it was uploaded with.
    pub name: crate::attachment::FileName,
    /// The file (from [`UploadedFile::file`]).
    pub file: crate::attachment::StoredFile,
    /// Its thumbnail (from [`UploadedFile::thumbnail`]).
    pub thumbnail: Option<crate::attachment::StoredFile>,
    /// A photo's or a PDF's details.
    pub content: crate::attachment::Content,
}

/// One attachment as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AttachmentView {
    /// Its ID.
    pub id: crate::attachment::AttachmentId,
    /// The attachment.
    pub attachment: crate::attachment::Attachment,
    /// Who added it, by name.
    pub added_by_name: String,
    /// The person may move or remove it.
    pub may_change: bool,
}

/// Which file of an attachment to download.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentPart {
    /// The photo or the PDF.
    File,
    /// Its thumbnail.
    Thumbnail,
    /// A photo's original (Maintainers and Admins only, #036).
    Original,
}

impl AttachmentPart {
    /// Its name in the download path.
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::File => "file",
            Self::Thumbnail => "thumbnail",
            Self::Original => "original",
        }
    }

    /// The part from its name in the path.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        [Self::File, Self::Thumbnail, Self::Original]
            .into_iter()
            .find(|part| part.as_str() == text)
    }
}

/// The start of every attachment download's path.
pub const DOWNLOAD_PATH_PREFIX: &str = "/api/v1/attachments/";

/// Where one part of an attachment is downloaded: `/api/v1/attachments/<id>/<part>`.
#[must_use]
pub fn download_path(id: crate::attachment::AttachmentId, part: AttachmentPart) -> String {
    format!("{DOWNLOAD_PATH_PREFIX}{id}/{}", part.as_str())
}

/// The answer to an attachment request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum AttachmentsOutcome {
    /// What's attached to something, oldest first.
    Attachments {
        /// Each attachment.
        attachments: Vec<AttachmentView>,
        /// The person may add one here.
        may_add: bool,
        /// The person may keep a photo's original here (#036).
        may_keep_originals: bool,
    },
    /// An attachment was added or moved.
    Saved {
        /// Its ID.
        id: crate::attachment::AttachmentId,
    },
    /// Done.
    Done,
    /// There's no such vehicle, attachment, or owner (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The attachment was refused.
    Invalid {
        /// Why.
        problem: crate::attachment::AttachmentProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

// --- Official documents and license plates (milestone 8) --------------------------------

/// An official document as typed in (renewals name what they renew separately).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentInput {
    /// Which vehicle.
    pub asset: crate::asset::AssetId,
    /// What kind.
    pub doc_type: crate::document::DocumentType,
    /// Its number.
    pub number: Option<String>,
    /// Who issued it.
    pub issuer: Option<String>,
    /// When it was issued.
    pub issued_on: Option<crate::dates::CalendarDate>,
    /// When it takes effect, if not when issued.
    pub effective_on: Option<crate::dates::CalendarDate>,
    /// When it expires (none: never).
    pub expires_on: Option<crate::dates::CalendarDate>,
    /// A registration's plate.
    pub plate: Option<crate::plate::LicensePlateId>,
    /// Notes.
    pub notes: crate::notes::Notes,
}

/// A license plate as typed in.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlateInput {
    /// Which vehicle.
    pub asset: crate::asset::AssetId,
    /// Who issued it.
    pub authority: Option<crate::plate::Authority>,
    /// Its number.
    pub number: crate::plate::PlateNumber,
    /// When it was issued.
    pub issued_on: Option<crate::dates::CalendarDate>,
    /// When it expires.
    pub expires_on: Option<crate::dates::CalendarDate>,
    /// When it stopped being used.
    pub retired_on: Option<crate::dates::CalendarDate>,
    /// Notes.
    pub notes: crate::notes::Notes,
}

/// One official document as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DocumentView {
    /// Its ID.
    pub id: crate::document::AssetDocumentId,
    /// The document.
    pub document: crate::document::AssetDocument,
    /// Where it stands on the day asked about.
    pub status: crate::document::DocumentStatus,
    /// The document that renews it, if any.
    pub renewed_by: Option<crate::document::AssetDocumentId>,
}

/// One license plate as the app shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlateView {
    /// Its ID.
    pub id: crate::plate::LicensePlateId,
    /// The plate.
    pub plate: crate::plate::LicensePlate,
    /// It's the vehicle's current plate.
    pub current: bool,
}

/// The answer to a documents or plates request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum DocumentsOutcome {
    /// One vehicle's documents (oldest first) and plates (the current one last).
    Documents {
        /// Each document.
        documents: Vec<DocumentView>,
        /// Each plate.
        plates: Vec<PlateView>,
        /// The document types the vehicle's profile lists (message keys).
        listed_types: Vec<String>,
        /// The person may add, renew, change, and remove them (a Maintainer, in that mode).
        may_change: bool,
    },
    /// A document was added, renewed, or changed.
    SavedDocument {
        /// Its ID.
        id: crate::document::AssetDocumentId,
    },
    /// A plate was added or changed.
    SavedPlate {
        /// Its ID.
        id: crate::plate::LicensePlateId,
    },
    /// Done.
    Done,
    /// There's no such vehicle, document, or plate (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The document was refused.
    InvalidDocument {
        /// Why.
        problem: crate::document::DocumentProblem,
    },
    /// The plate was refused.
    InvalidPlate {
        /// Why.
        problem: crate::plate::PlateProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// One requirement as the app shows it, with when it was last done.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RequirementView {
    /// Its ID.
    pub id: crate::requirement::RequirementId,
    /// The requirement.
    pub requirement: crate::requirement::Requirement,
    /// The latest finished task that did it, if any.
    pub last_done: Option<crate::requirement::DoneTask>,
    /// Where it stands (open, complied, active, not applicable).
    pub status: crate::requirement::RequirementStatus,
}

/// A reference document as typed in (everything but who added it and when).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceInput {
    /// See [`crate::reference::ReferenceDocument::doc_type`].
    pub doc_type: crate::reference::ReferenceType,
    /// See [`crate::reference::ReferenceDocument::authority`].
    pub authority: Option<String>,
    /// See [`crate::reference::ReferenceDocument::numbers`].
    pub numbers: Vec<crate::reference::ReferenceNumber>,
    /// See [`crate::reference::ReferenceDocument::title`].
    pub title: String,
    /// See [`crate::reference::ReferenceDocument::description`].
    pub description: crate::notes::Notes,
    /// See [`crate::reference::ReferenceDocument::remedy`].
    pub remedy: crate::notes::Notes,
    /// See [`crate::reference::ReferenceDocument::issued_on`].
    pub issued_on: Option<crate::dates::CalendarDate>,
    /// See [`crate::reference::ReferenceDocument::effective_on`].
    pub effective_on: Option<crate::dates::CalendarDate>,
    /// See [`crate::reference::ReferenceDocument::revision`].
    pub revision: Option<String>,
    /// See [`crate::reference::ReferenceDocument::supersedes`].
    pub supersedes: Option<crate::reference::ReferenceDocumentId>,
    /// See [`crate::reference::ReferenceDocument::applicability`].
    pub applicability: crate::notes::Notes,
    /// See [`crate::reference::ReferenceDocument::url`].
    pub url: Option<String>,
}

/// The requirement a new reference document starts with, on one vehicle.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceLink {
    /// Which vehicle.
    pub asset: crate::asset::AssetId,
    /// The requirement's name (e.g. a shortened title).
    pub name: crate::label::Label,
    /// One-time (most recalls) or recurring.
    pub mode: crate::requirement::ComplianceMode,
    /// Whether it applies.
    pub applicability: crate::requirement::Applicability,
}

/// One reference document as the person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReferenceView {
    /// Its ID.
    pub id: crate::reference::ReferenceDocumentId,
    /// The document.
    pub document: crate::reference::ReferenceDocument,
    /// The vehicles linking it that the person can see, with their names.
    pub vehicles: Vec<(crate::asset::AssetId, String)>,
    /// The person may change it or its attachments (a Maintainer of every linked vehicle, or
    /// the household's Admin in Admin mode).
    pub may_change: bool,
}

/// The answer to a reference document request (#068).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum ReferencesOutcome {
    /// The reference documents the person can read, by title.
    References {
        /// Each one.
        references: Vec<ReferenceView>,
    },
    /// One reference document.
    Reference {
        /// It.
        view: Box<ReferenceView>,
    },
    /// A reference document was added (with its first requirement) or changed.
    Saved {
        /// Its ID.
        id: crate::reference::ReferenceDocumentId,
        /// The requirement it started with, when it was added.
        requirement: Option<crate::requirement::RequirementId>,
    },
    /// Done.
    Done,
    /// There's no such reference document or vehicle (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The reference document was refused.
    Invalid {
        /// Why.
        problem: crate::reference::ReferenceProblem,
    },
    /// The requirement it starts with was refused.
    InvalidRequirement {
        /// Why.
        problem: crate::requirement::RequirementProblem,
    },
    /// It can't be removed while a requirement links it.
    InUse,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to a requirements or bundles request (a vehicle's maintenance schedule).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum ScheduleOutcome {
    /// One vehicle's schedule.
    Schedule {
        /// Its requirements (turned-off ones too), by name.
        requirements: Vec<RequirementView>,
        /// Its bundles, by name.
        bundles: Vec<(crate::requirement::BundleId, crate::requirement::Bundle)>,
        /// The person may change it (a Maintainer or higher, in that mode).
        may_configure: bool,
    },
    /// A requirement was added or changed.
    SavedRequirement {
        /// Its ID.
        id: crate::requirement::RequirementId,
    },
    /// A bundle was added or changed.
    SavedBundle {
        /// Its ID.
        id: crate::requirement::BundleId,
    },
    /// A schedule was copied from another vehicle.
    Copied {
        /// How many requirements were copied.
        requirements: usize,
        /// How many bundles were copied.
        bundles: usize,
        /// Meter rules left out, as "requirement: meter" (no matching meter).
        left_out: Vec<(crate::label::Label, crate::label::Label)>,
    },
    /// Done.
    Done,
    /// There's no such vehicle, requirement, or bundle (or the person can't see it).
    NotFound,
    /// The person's roles or mode don't allow it.
    NotAllowed,
    /// The requirement or bundle was refused.
    Invalid {
        /// Why.
        problem: crate::requirement::RequirementProblem,
    },
    /// The server couldn't do it right now.
    Unavailable,
}

/// What to do with a message (decision #050).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MarkMessage {
    /// Mark it read.
    Read,
    /// Put it away (it's also marked read).
    Dismiss,
}

/// The answer to a message center request.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum InboxOutcome {
    /// The person's inbox (after any change).
    Inbox {
        /// What they can see, unread first.
        inbox: crate::inbox::Inbox,
    },
    /// There's no message with that key for this person (gone, or not theirs to see).
    NotFound,
    /// The server couldn't do it right now (e.g. it's refusing changes).
    Unavailable,
}

/// The answer to a password change.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum ChangePasswordOutcome {
    /// Changed. Other sessions of this user were signed out.
    Changed,
    /// The current password is wrong.
    WrongPassword,
    /// The new password breaks a rule.
    InvalidNewPassword {
        /// Which rule.
        reason: crate::user::PasswordError,
    },
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't do it right now.
    Unavailable,
}

/// The answer to a sign-in attempt.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SignInOutcome {
    /// Signed in. The web app gets a session cookie; `token` is set only when the caller
    /// asked for it (the desktop app, which sends it as `Authorization: Bearer`), so a
    /// web page's scripts never see it.
    SignedIn {
        /// Who signed in.
        session: SessionInfo,
        /// The session token, for the desktop app.
        token: Option<String>,
    },
    /// Unknown username or wrong password (deliberately not saying which).
    InvalidCredentials,
    /// Too many failed attempts; try again after this many seconds.
    TooManyAttempts {
        /// How long to wait.
        retry_after_seconds: i64,
    },
    /// The server couldn't check right now.
    Unavailable,
}

/// The answer to a mode switch.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum SwitchModeOutcome {
    /// Now in this mode.
    Switched {
        /// The new mode.
        mode: crate::access::ActiveMode,
    },
    /// The user's roles don't allow that mode.
    NotAllowed,
    /// Admin mode needs the password, and it was wrong.
    WrongPassword,
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't switch right now.
    Unavailable,
}

/// Servers from before the `healthy` field had no health check, so treat them as healthy.
const fn healthy_by_default() -> bool {
    true
}

/// The server's reply to a ping.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Pong {
    /// The server's clock, as Unix epoch milliseconds (UTC).
    pub server_time_ms: i64,
}

#[cfg(test)]
mod tests {
    #[test]
    fn handshakes_from_older_servers_count_as_healthy() {
        let older = r#"{"app_version":"0.1.0","api_version":1,"min_supported_api_version":1,"server_time_ms":0}"#;
        let handshake: Handshake = serde_json::from_str(older).unwrap();
        assert!(handshake.healthy);
    }

    use super::*;
    use crate::i18n::Translator;

    #[test]
    fn api_version_check_accepts_supported_range() {
        for version in MIN_SUPPORTED_API_VERSION..=API_VERSION {
            assert_eq!(check_client_api_version(version), Ok(()));
        }
    }

    #[test]
    fn client_older_than_minimum_is_too_old() {
        assert_eq!(
            check_client_api_version(MIN_SUPPORTED_API_VERSION - 1),
            Err(ApiError::ClientTooOld)
        );
    }

    #[test]
    fn client_newer_than_server_is_too_new() {
        assert_eq!(
            check_client_api_version(API_VERSION + 1),
            Err(ApiError::ClientTooNew)
        );
    }

    #[test]
    fn api_version_header_parses_whole_numbers_only() {
        assert_eq!(parse_api_version("1"), Ok(1));
        assert_eq!(parse_api_version(" 2 "), Ok(2));
        for bad in ["", "v1", "1.0", "-1", "one"] {
            assert_eq!(
                parse_api_version(bad),
                Err(ApiError::MalformedApiVersion),
                "{bad:?}"
            );
        }
    }

    #[test]
    fn every_api_error_has_a_status_and_message() {
        let english = Translator::english();
        for error in ApiError::ALL {
            assert!((400..600).contains(&error.http_status()), "{error:?}");
            assert!(english.has(error.message()), "{error:?}");
            assert!(!error.description().is_empty());
        }
    }

    #[test]
    fn api_errors_serialize_as_kebab_case_codes() {
        assert_eq!(
            serde_json::to_string(&ApiError::InvalidCredential)
                .ok()
                .as_deref(),
            Some(r#""invalid-credential""#)
        );
        assert_eq!(
            serde_json::from_str::<ApiError>(r#""client-too-old""#).ok(),
            Some(ApiError::ClientTooOld)
        );
    }

    #[test]
    fn only_credential_problems_count_as_auth_failures() {
        let auth: Vec<ApiError> = ApiError::ALL
            .into_iter()
            .filter(|e| e.is_auth_failure())
            .collect();
        assert_eq!(
            auth,
            [
                ApiError::MissingCredential,
                ApiError::InvalidCredential,
                ApiError::AuthNotConfigured
            ]
        );
    }

    #[test]
    fn upload_purposes_round_trip_through_their_query() {
        use crate::attachment::PhotoSize;
        for purpose in [
            UploadPurpose::Photo {
                size: PhotoSize::Small,
            },
            UploadPurpose::Photo {
                size: PhotoSize::Large,
            },
            UploadPurpose::Original,
            UploadPurpose::PhotoMetadata,
            UploadPurpose::Document,
            UploadPurpose::DocumentThumbnail,
        ] {
            assert_eq!(
                UploadPurpose::from_query(&purpose.to_query()),
                Some(purpose)
            );
        }
        assert_eq!(
            UploadPurpose::from_query("for=photo"),
            None,
            "a photo needs a size"
        );
        assert_eq!(UploadPurpose::from_query("for=video"), None);
        assert_eq!(
            AttachmentPart::parse("thumbnail"),
            Some(AttachmentPart::Thumbnail)
        );
        assert_eq!(AttachmentPart::parse("raw"), None);
    }
}
