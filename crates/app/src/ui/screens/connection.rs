//! The Diagnostics "Connection" panel: checks the API version, whether this session is
//! signed in, and the server's health, and shows pass/fail for each.
//!
//! 1. The **handshake** is public: it checks the API version and reports the server's health.
//! 2. A **ping** needs a signed-in session: it's the "Signed in" check, and times the round trip.
//!
//! Checks run only when the button is pressed, never during server rendering (see
//! `crate::api`).

use dioxus::prelude::*;
use domain::api::{ApiError, Handshake};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::log_area;
use domain::time::Clock;

use crate::api;
use crate::logging::SystemClock;
use crate::ui::{tr, tr_with};

/// The result of one check.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum CheckState {
    Pass,
    Fail,
    /// Didn't run, e.g. sign-in isn't checked when the server can't be reached.
    NotChecked,
}

impl CheckState {
    const fn label(self) -> MessageId {
        match self {
            Self::Pass => messages::CONNECTION_PASS,
            Self::Fail => messages::CONNECTION_FAIL,
            Self::NotChecked => messages::CONNECTION_NOT_CHECKED,
        }
    }

    const fn class(self) -> &'static str {
        match self {
            Self::Pass => "check-pass",
            Self::Fail => "check-fail",
            Self::NotChecked => "check-not-checked",
        }
    }
}

/// Why the checks didn't all pass.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Problem {
    /// The server refused the request, with a reason.
    Refused(ApiError),
    /// No answer from the server (down, wrong address, network).
    Unreachable,
    /// An answer we couldn't interpret.
    Unexpected,
    /// The server answered, but reports a problem with its database.
    ServerUnhealthy,
}

impl Problem {
    pub(crate) const fn message(self) -> MessageId {
        match self {
            Self::Refused(error) => error.message(),
            Self::Unreachable => messages::CONNECTION_UNREACHABLE,
            Self::Unexpected => messages::CONNECTION_UNEXPECTED,
            Self::ServerUnhealthy => messages::CONNECTION_SERVER_UNHEALTHY,
        }
    }
}

/// Everything the panel shows after a run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CheckOutcome {
    pub(crate) auth: CheckState,
    pub(crate) version: CheckState,
    /// The server's own health, from the handshake; not checked if the handshake failed.
    pub(crate) health: CheckState,
    pub(crate) problem: Option<Problem>,
    pub(crate) server: Option<Handshake>,
    pub(crate) round_trip_ms: Option<i64>,
}

impl CheckOutcome {
    /// Nothing passed: the handshake itself failed.
    const fn handshake_failed(version: CheckState, problem: Problem) -> Self {
        Self {
            auth: CheckState::NotChecked,
            version,
            health: CheckState::NotChecked,
            problem: Some(problem),
            server: None,
            round_trip_ms: None,
        }
    }
}

/// What went wrong with a server-function call.
pub(crate) fn problem_from_error(error: &ServerFnError) -> Problem {
    match error {
        ServerFnError::ServerError {
            details: Some(details),
            ..
        } => serde_json::from_value::<ApiError>(details.clone())
            .map_or(Problem::Unexpected, Problem::Refused),
        ServerFnError::Request(_) => Problem::Unreachable,
        // A server function's own failure, or a decoding problem: not a refusal we understand.
        ServerFnError::ServerError { details: None, .. }
        | ServerFnError::StreamError(_)
        | ServerFnError::Registration(_)
        | ServerFnError::UnsupportedRequestMethod(_)
        | ServerFnError::MiddlewareError(_)
        | ServerFnError::Deserialization(_)
        | ServerFnError::Serialization(_)
        | ServerFnError::Args(_)
        | ServerFnError::MissingArg(_)
        | ServerFnError::Response(_) => Problem::Unexpected,
    }
}

/// The handshake failed: a version refusal fails the version check; anything else (server
/// unreachable, unexpected answer) leaves every check unchecked.
pub(crate) fn outcome_from_handshake_error(error: &ServerFnError) -> CheckOutcome {
    let problem = problem_from_error(error);
    let version = match problem {
        Problem::Refused(reason) if !reason.is_auth_failure() => CheckState::Fail,
        Problem::Refused(_)
        | Problem::Unreachable
        | Problem::Unexpected
        | Problem::ServerUnhealthy => CheckState::NotChecked,
    };
    CheckOutcome::handshake_failed(version, problem)
}

/// The handshake passed; `ping` is the signed-in check's result (its round trip in
/// milliseconds, or its error). A sign-in problem is reported before an unhealthy server.
pub(crate) fn outcome_from_handshake(
    server: Handshake,
    ping: Result<i64, &ServerFnError>,
) -> CheckOutcome {
    let health = if server.healthy {
        CheckState::Pass
    } else {
        CheckState::Fail
    };
    let unhealthy = (!server.healthy).then_some(Problem::ServerUnhealthy);
    let (auth, auth_problem, round_trip_ms) = match ping {
        Ok(millis) => (CheckState::Pass, None, Some(millis)),
        Err(error) => match problem_from_error(error) {
            Problem::Refused(reason) if reason.is_auth_failure() => {
                (CheckState::Fail, Some(Problem::Refused(reason)), None)
            }
            other @ (Problem::Refused(_)
            | Problem::Unreachable
            | Problem::Unexpected
            | Problem::ServerUnhealthy) => (CheckState::NotChecked, Some(other), None),
        },
    };
    CheckOutcome {
        auth,
        version: CheckState::Pass,
        health,
        problem: auth_problem.or(unhealthy),
        server: Some(server),
        round_trip_ms,
    }
}

/// Runs the handshake, then a ping to check the session and time the round trip.
async fn run_checks() -> CheckOutcome {
    // Headers (API version, desktop session token) are kept current by `crate::client`.
    crate::client::apply_request_headers();
    tracing::debug!(target: log_area::AUTH, "running connection checks");

    let server = match api::handshake().await {
        Ok(server) => server,
        Err(error) => {
            let outcome = outcome_from_handshake_error(&error);
            tracing::debug!(target: log_area::AUTH, problem = ?outcome.problem, "handshake failed");
            return outcome;
        }
    };
    let started = SystemClock.now().as_millis();
    let ping = api::ping()
        .await
        .map(|_| SystemClock.now().as_millis().saturating_sub(started));
    let outcome = outcome_from_handshake(server, ping.as_ref().map(|millis| *millis));
    tracing::debug!(target: log_area::AUTH, auth = ?outcome.auth, healthy = ?outcome.health, round_trip_ms = ?outcome.round_trip_ms, "connection checks finished");
    outcome
}

/// The panel.
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn ConnectionPanel() -> Element {
    let mut running = use_signal(|| false);
    let mut outcome = use_signal(|| None::<CheckOutcome>);

    rsx! {
        section { class: "connection",
            h2 { {tr(messages::CONNECTION_TITLE)} }
            ServerAddressField {}
            div { class: "connection-controls",
                button {
                    class: "run-checks",
                    disabled: running(),
                    onclick: move |_| async move {
                        running.set(true);
                        let result = run_checks().await;
                        outcome.set(Some(result));
                        running.set(false);
                    },
                    if running() {
                        {tr(messages::CONNECTION_RUNNING)}
                    } else {
                        {tr(messages::CONNECTION_RUN_CHECKS)}
                    }
                }
            }
            if let Some(result) = outcome() {
                CheckResults { result }
            }
        }
    }
}

/// The result of pressing "Save address": a message, and whether it's a problem.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct SaveStatus {
    message: MessageId,
    is_problem: bool,
}

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
impl SaveStatus {
    fn from_result(
        result: &Result<domain::server_address::ServerAddress, crate::settings::SaveAddressError>,
    ) -> Self {
        use crate::settings::SaveAddressError;

        let (message, is_problem) = match result {
            Ok(_) => (messages::CONNECTION_ADDRESS_SAVED, false),
            Err(SaveAddressError::Invalid(error)) => (error.message(), true),
            Err(SaveAddressError::NoDataFolder | SaveAddressError::Io(_)) => {
                (messages::CONNECTION_ADDRESS_SAVE_FAILED, true)
            }
        };
        Self {
            message,
            is_problem,
        }
    }
}

/// Desktop: the server address field. Shows the address this run uses, and saves a new
/// one for the next launch (Dioxus can't change it while running; decision #027).
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ServerAddressField() -> Element {
    use dioxus::prelude::dioxus_fullstack::get_server_url;

    let in_use = get_server_url();
    let mut address = use_signal(|| in_use.to_owned());
    let mut status = use_signal(|| None::<SaveStatus>);

    rsx! {
        div { class: "connection-controls server-address",
            label {
                {tr(messages::CONNECTION_SERVER_ADDRESS)}
                input {
                    r#type: "url",
                    autocomplete: "off",
                    spellcheck: "false",
                    value: address(),
                    oninput: move |event| address.set(event.value()),
                }
            }
            button {
                class: "run-checks",
                onclick: move |_| {
                    let data_folder = crate::data_folder::platform_data_folder();
                    let result = crate::settings::save_server_address(data_folder.as_deref(), &address());
                    if let Err(error) = &result {
                        tracing::warn!(target: log_area::APP, ?error, "server address not saved");
                    }
                    status.set(Some(SaveStatus::from_result(&result)));
                },
                {tr(messages::CONNECTION_SAVE_ADDRESS)}
            }
        }
        p { class: "check-detail", {tr_with(messages::CONNECTION_ADDRESS_IN_USE, &[("address", MessageArg::from(in_use))])} }
        if let Some(status) = status() {
            p { class: if status.is_problem { "check-problem" } else { "check-detail" }, {tr(status.message)} }
        }
    }
}

/// Web and mobile: no address field. The web app always talks to the server it was
/// loaded from; mobile gets its own setting in Phase 4.
#[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
#[component]
fn ServerAddressField() -> Element {
    rsx! {}
}

/// The pass/fail rows and details of a finished run.
#[component]
fn CheckResults(result: CheckOutcome) -> Element {
    rsx! {
        dl { class: "check-results",
            dt { {tr(messages::CONNECTION_VERSION_CHECK)} }
            dd { class: result.version.class(), {tr(result.version.label())} }
            dt { {tr(messages::CONNECTION_AUTH_CHECK)} }
            dd { class: result.auth.class(), {tr(result.auth.label())} }
            dt { {tr(messages::CONNECTION_HEALTH_CHECK)} }
            dd { class: result.health.class(), {tr(result.health.label())} }
        }
        if let Some(problem) = result.problem {
            p { class: "check-problem", {tr(problem.message())} }
        }
        if let Some(server) = &result.server {
            p { class: "check-detail", {server_info(server)} }
        }
        if let Some(millis) = result.round_trip_ms {
            p { class: "check-detail", {tr_with(messages::CONNECTION_ROUND_TRIP, &[("millis", MessageArg::from(millis))])} }
        }
    }
}

fn server_info(server: &Handshake) -> String {
    tr_with(
        messages::CONNECTION_SERVER_INFO,
        &[
            ("app_version", MessageArg::from(server.app_version.as_str())),
            ("api_version", MessageArg::from(server.api_version)),
            (
                "min_api_version",
                MessageArg::from(server.min_supported_api_version),
            ),
        ],
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn refused(reason: ApiError) -> ServerFnError {
        ServerFnError::ServerError {
            message: reason.description().to_owned(),
            code: reason.http_status(),
            details: serde_json::to_value(reason).ok(),
        }
    }

    fn handshake(healthy: bool) -> Handshake {
        Handshake {
            app_version: "0.1.0".to_owned(),
            api_version: domain::api::API_VERSION,
            min_supported_api_version: domain::api::API_VERSION,
            server_time_ms: 0,
            healthy,
        }
    }

    #[test]
    fn signed_in_to_a_healthy_server_passes_every_check() {
        let outcome = outcome_from_handshake(handshake(true), Ok(12));
        assert_eq!(
            (outcome.version, outcome.auth, outcome.health),
            (CheckState::Pass, CheckState::Pass, CheckState::Pass)
        );
        assert_eq!((outcome.problem, outcome.round_trip_ms), (None, Some(12)));
    }

    #[test]
    fn not_signed_in_fails_only_the_sign_in_check() {
        let error = refused(ApiError::MissingCredential);
        let outcome = outcome_from_handshake(handshake(true), Err(&error));
        assert_eq!(
            (outcome.version, outcome.auth, outcome.health),
            (CheckState::Pass, CheckState::Fail, CheckState::Pass)
        );
        assert_eq!(
            outcome.problem,
            Some(Problem::Refused(ApiError::MissingCredential))
        );
    }

    #[test]
    fn an_unhealthy_server_fails_the_health_check_with_its_reason() {
        let outcome = outcome_from_handshake(handshake(false), Ok(12));
        assert_eq!(
            (outcome.auth, outcome.health),
            (CheckState::Pass, CheckState::Fail)
        );
        assert_eq!(outcome.problem, Some(Problem::ServerUnhealthy));
    }

    #[test]
    fn a_sign_in_problem_is_reported_before_an_unhealthy_server() {
        let error = refused(ApiError::InvalidCredential);
        let outcome = outcome_from_handshake(handshake(false), Err(&error));
        assert_eq!(
            outcome.problem,
            Some(Problem::Refused(ApiError::InvalidCredential))
        );
    }

    #[test]
    fn a_version_refusal_fails_the_version_and_checks_nothing_else() {
        let outcome = outcome_from_handshake_error(&refused(ApiError::ClientTooOld));
        assert_eq!(
            (outcome.version, outcome.auth, outcome.health),
            (
                CheckState::Fail,
                CheckState::NotChecked,
                CheckState::NotChecked
            )
        );
    }

    #[test]
    fn unknown_or_missing_reasons_are_unexpected() {
        let unknown = ServerFnError::ServerError {
            message: String::new(),
            code: 500,
            details: Some(serde_json::json!("something-else")),
        };
        assert_eq!(problem_from_error(&unknown), Problem::Unexpected);
        assert_eq!(
            problem_from_error(&ServerFnError::new("boom")),
            Problem::Unexpected
        );
    }

    #[test]
    fn every_problem_has_a_translated_message() {
        let english = domain::i18n::Translator::english();
        let problems = ApiError::ALL.into_iter().map(Problem::Refused).chain([
            Problem::Unreachable,
            Problem::Unexpected,
            Problem::ServerUnhealthy,
        ]);
        for problem in problems {
            assert!(english.has(problem.message()), "{problem:?}");
        }
    }
}
