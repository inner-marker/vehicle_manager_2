//! The Diagnostics "Connection" panel: checks the API version and credential against
//! the server and shows pass/fail for each (Phase 1 milestone 2).
//!
//! The credential is typed in and held in memory only, until Phase 2 adds logins.
//! Checks run only when the button is pressed, never during server rendering (see
//! `crate::api`).

use dioxus::prelude::dioxus_fullstack::{HeaderMap, HeaderValue, set_request_headers};
use dioxus::prelude::*;
use domain::api::{API_VERSION, API_VERSION_HEADER, ApiError, Handshake};
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
    /// Didn't run, e.g. the version isn't checked once the credential fails.
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
}

impl Problem {
    const fn message(self) -> MessageId {
        match self {
            Self::Refused(error) => error.message(),
            Self::Unreachable => messages::CONNECTION_UNREACHABLE,
            Self::Unexpected => messages::CONNECTION_UNEXPECTED,
        }
    }
}

/// Everything the panel shows after a run.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CheckOutcome {
    pub(crate) auth: CheckState,
    pub(crate) version: CheckState,
    pub(crate) problem: Option<Problem>,
    pub(crate) server: Option<Handshake>,
    pub(crate) round_trip_ms: Option<i64>,
}

impl CheckOutcome {
    fn failed(auth: CheckState, version: CheckState, problem: Problem) -> Self {
        Self {
            auth,
            version,
            problem: Some(problem),
            server: None,
            round_trip_ms: None,
        }
    }
}

/// Turns a failed server-function call into pass/fail states.
///
/// The auth layer checks the credential first, so a credential refusal means the
/// version was never checked, and a version refusal means the credential passed.
pub(crate) fn outcome_from_error(error: &ServerFnError) -> CheckOutcome {
    match error {
        ServerFnError::ServerError {
            details: Some(details),
            ..
        } => match serde_json::from_value::<ApiError>(details.clone()) {
            Ok(reason) if reason.is_auth_failure() => CheckOutcome::failed(
                CheckState::Fail,
                CheckState::NotChecked,
                Problem::Refused(reason),
            ),
            Ok(reason) => {
                CheckOutcome::failed(CheckState::Pass, CheckState::Fail, Problem::Refused(reason))
            }
            Err(_) => CheckOutcome::failed(
                CheckState::NotChecked,
                CheckState::NotChecked,
                Problem::Unexpected,
            ),
        },
        ServerFnError::Request(_) => CheckOutcome::failed(
            CheckState::NotChecked,
            CheckState::NotChecked,
            Problem::Unreachable,
        ),
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
        | ServerFnError::Response(_) => CheckOutcome::failed(
            CheckState::NotChecked,
            CheckState::NotChecked,
            Problem::Unexpected,
        ),
    }
}

/// The headers every server-function call carries: credential and API version.
/// A credential with characters headers can't carry is left out (the server then
/// reports it as missing).
fn request_headers(credential: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(API_VERSION_HEADER, HeaderValue::from(API_VERSION));
    if let Ok(value) = HeaderValue::from_str(&format!("Bearer {}", credential.trim())) {
        headers.insert("authorization", value);
    }
    headers
}

/// Runs the handshake, then a ping to time the round trip.
async fn run_checks(credential: &str) -> CheckOutcome {
    set_request_headers(request_headers(credential));
    tracing::debug!(target: log_area::AUTH, "running connection checks");

    let server = match api::handshake().await {
        Ok(server) => server,
        Err(error) => {
            let outcome = outcome_from_error(&error);
            tracing::debug!(target: log_area::AUTH, problem = ?outcome.problem, "handshake failed");
            return outcome;
        }
    };

    let started = SystemClock.now().as_millis();
    let round_trip_ms = match api::ping().await {
        Ok(_) => Some(SystemClock.now().as_millis().saturating_sub(started)),
        Err(error) => return outcome_from_error(&error),
    };
    tracing::debug!(target: log_area::AUTH, ?round_trip_ms, server_api = server.api_version, "connection checks passed");

    CheckOutcome {
        auth: CheckState::Pass,
        version: CheckState::Pass,
        problem: None,
        server: Some(server),
        round_trip_ms,
    }
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
    let mut credential = use_signal(String::new);
    let mut running = use_signal(|| false);
    let mut outcome = use_signal(|| None::<CheckOutcome>);

    rsx! {
        section { class: "connection",
            h2 { {tr(messages::CONNECTION_TITLE)} }
            div { class: "connection-controls",
                label {
                    {tr(messages::CONNECTION_CREDENTIAL)}
                    input {
                        r#type: "password",
                        autocomplete: "off",
                        value: credential(),
                        oninput: move |event| credential.set(event.value()),
                    }
                }
                button {
                    class: "run-checks",
                    disabled: running(),
                    onclick: move |_| async move {
                        running.set(true);
                        let result = run_checks(&credential()).await;
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

/// The pass/fail rows and details of a finished run.
#[component]
fn CheckResults(result: CheckOutcome) -> Element {
    rsx! {
        dl { class: "check-results",
            dt { {tr(messages::CONNECTION_VERSION_CHECK)} }
            dd { class: result.version.class(), {tr(result.version.label())} }
            dt { {tr(messages::CONNECTION_AUTH_CHECK)} }
            dd { class: result.auth.class(), {tr(result.auth.label())} }
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

    #[test]
    fn credential_refusal_fails_auth_and_skips_version() {
        let outcome = outcome_from_error(&refused(ApiError::InvalidCredential));
        assert_eq!(outcome.auth, CheckState::Fail);
        assert_eq!(outcome.version, CheckState::NotChecked);
        assert_eq!(
            outcome.problem,
            Some(Problem::Refused(ApiError::InvalidCredential))
        );
    }

    #[test]
    fn version_refusal_passes_auth_and_fails_version() {
        let outcome = outcome_from_error(&refused(ApiError::ClientTooOld));
        assert_eq!(outcome.auth, CheckState::Pass);
        assert_eq!(outcome.version, CheckState::Fail);
    }

    #[test]
    fn unknown_or_missing_reasons_are_unexpected() {
        let unknown = ServerFnError::ServerError {
            message: String::new(),
            code: 500,
            details: Some(serde_json::json!("something-else")),
        };
        assert_eq!(
            outcome_from_error(&unknown).problem,
            Some(Problem::Unexpected)
        );
        assert_eq!(
            outcome_from_error(&ServerFnError::new("boom")).problem,
            Some(Problem::Unexpected)
        );
    }

    #[test]
    fn requests_carry_api_version_and_bearer_credential() {
        let headers = request_headers("  secret  ");
        assert_eq!(
            headers
                .get(API_VERSION_HEADER)
                .and_then(|v| v.to_str().ok()),
            Some("1")
        );
        assert_eq!(
            headers.get("authorization").and_then(|v| v.to_str().ok()),
            Some("Bearer secret")
        );
    }

    #[test]
    fn every_problem_has_a_translated_message() {
        let english = domain::i18n::Translator::english();
        let problems = ApiError::ALL
            .into_iter()
            .map(Problem::Refused)
            .chain([Problem::Unreachable, Problem::Unexpected]);
        for problem in problems {
            assert!(english.has(problem.message()), "{problem:?}");
        }
    }
}
