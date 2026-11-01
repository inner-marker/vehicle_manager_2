//! The message center's messages (decisions #040, #050).
//!
//! **Messages are derived from the current state, not stored:** a message exists while its
//! cause is true ("last night's snapshot failed" while the snapshot job's latest result is a
//! failure) and goes away when it's fixed. Each has a stable [`MessageKey`] built from its
//! cause, so a new failure is a new message. Only each person's **mark** (read, dismissed) is
//! stored, as a [`MessageMarkRecord`] through the command pipeline.
//!
//! Who sees what: Admin messages only in Admin mode; everyone sees that the server is
//! refusing changes, and only Admin mode shows the technical details.
//!
//! # Examples
//!
//! ```
//! use domain::access::ActiveMode;
//! use domain::inbox::{JobReport, JobOutcome, ServerState, inbox, server_messages};
//! use domain::time::UnixMillis;
//!
//! let state = ServerState {
//!     problem: None,
//!     jobs: vec![JobReport {
//!         name: "nightly-snapshot".into(),
//!         outcome: JobOutcome::Failed,
//!         finished: UnixMillis::from_millis(1_000),
//!         detail: "disk full".into(),
//!     }],
//! };
//! let messages = server_messages(&state);
//! // An Admin message: shown in Admin mode, not in Driver mode.
//! assert_eq!(inbox(&messages, &[], ActiveMode::Admin).unread, 1);
//! assert_eq!(inbox(&messages, &[], ActiveMode::Driver).unread, 0);
//! ```

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::access::ActiveMode;
use crate::command::{Patch, Record};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::time::UnixMillis;
use crate::user::UserId;

/// How much a message matters. Ordered: `Info < Warning < Urgent`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Good to know.
    Info,
    /// Needs attention soon.
    Warning,
    /// Needs attention now; also shown as a banner on every screen.
    Urgent,
}

/// Who a message is for.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Audience {
    /// Everyone signed in.
    Everyone,
    /// The household's Admin, in Admin mode only.
    AdminMode,
}

/// A message's identity, built from its cause (e.g. `job-failed:nightly-snapshot:<time>`):
/// the same cause is the same message everywhere; a new failure is a new message.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct MessageKey(String);

impl MessageKey {
    /// The longest key accepted from a client (real keys are far shorter).
    pub const MAX_LENGTH: usize = 200;

    /// A key received from a client; `None` if empty or longer than [`Self::MAX_LENGTH`].
    /// (The server also checks that the person can see a message with this key.)
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        (!text.is_empty() && text.len() <= Self::MAX_LENGTH).then(|| Self(text.to_owned()))
    }

    /// The key as text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl fmt::Display for MessageKey {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.0)
    }
}

/// A scheduled job on the server, by its stable name (decision #044).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "job", content = "name", rename_all = "kebab-case")]
pub enum JobKind {
    /// The nightly database snapshot (`nightly-snapshot`).
    NightlySnapshot,
    /// The hourly database check (`quick-check`).
    QuickCheck,
    /// A job this build doesn't know by name.
    Other(String),
}

impl JobKind {
    /// The job with this name in the server's `jobs` table.
    #[must_use]
    pub fn from_name(name: &str) -> Self {
        match name {
            "nightly-snapshot" => Self::NightlySnapshot,
            "quick-check" => Self::QuickCheck,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The job's stable name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::NightlySnapshot => "nightly-snapshot",
            Self::QuickCheck => "quick-check",
            Self::Other(name) => name,
        }
    }
}

/// What's wrong with an unhealthy server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ServerProblem {
    /// The database couldn't be opened.
    DatabaseUnavailable,
    /// The database's integrity check found problems.
    IntegrityCheckFailed,
}

/// What a message is about.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum MessageKind {
    /// A scheduled job's latest run failed.
    JobFailed {
        /// Which job.
        job: JobKind,
    },
    /// A scheduled job's latest run was skipped (e.g. the snapshot folder is missing).
    JobSkipped {
        /// Which job.
        job: JobKind,
    },
    /// The server is refusing changes because of its database.
    ServerUnhealthy {
        /// What's wrong.
        problem: ServerProblem,
    },
}

impl MessageKind {
    /// The message's title.
    #[must_use]
    pub const fn title(&self) -> MessageId {
        match self {
            Self::JobFailed {
                job: JobKind::NightlySnapshot,
            } => messages::INBOX_SNAPSHOT_FAILED,
            Self::JobFailed {
                job: JobKind::QuickCheck,
            } => messages::INBOX_QUICK_CHECK_FAILED,
            Self::JobFailed {
                job: JobKind::Other(_),
            } => messages::INBOX_JOB_FAILED,
            Self::JobSkipped {
                job: JobKind::NightlySnapshot,
            } => messages::INBOX_SNAPSHOT_SKIPPED,
            Self::JobSkipped { .. } => messages::INBOX_JOB_SKIPPED,
            Self::ServerUnhealthy { .. } => messages::INBOX_SERVER_UNHEALTHY,
        }
    }

    /// The message's text (the problem's own text for an unhealthy server).
    #[must_use]
    pub const fn body(&self) -> MessageId {
        match self {
            Self::JobFailed {
                job: JobKind::NightlySnapshot,
            }
            | Self::JobSkipped {
                job: JobKind::NightlySnapshot,
            } => messages::INBOX_SNAPSHOT_BODY,
            Self::JobFailed {
                job: JobKind::QuickCheck,
            } => messages::INBOX_QUICK_CHECK_BODY,
            Self::JobFailed { .. } | Self::JobSkipped { .. } => messages::INBOX_JOB_BODY,
            Self::ServerUnhealthy {
                problem: ServerProblem::DatabaseUnavailable,
            } => messages::INBOX_DATABASE_UNAVAILABLE,
            Self::ServerUnhealthy {
                problem: ServerProblem::IntegrityCheckFailed,
            } => messages::INBOX_INTEGRITY_CHECK_FAILED,
        }
    }
}

/// The one thing a message's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageAction {
    /// Open Diagnostics (the server's health and the log).
    OpenDiagnostics,
}

impl MessageAction {
    /// The button's label.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::OpenDiagnostics => messages::INBOX_OPEN_DIAGNOSTICS,
        }
    }
}

/// One message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Message {
    /// Its stable identity.
    pub key: MessageKey,
    /// What it's about (gives its title and text).
    pub kind: MessageKind,
    /// How much it matters.
    pub severity: Severity,
    /// Who it's for.
    pub audience: Audience,
    /// When its cause happened.
    pub raised_at: UnixMillis,
    /// Technical details from the server (e.g. a job's own message), English only, for the
    /// Admin; empty outside Admin mode.
    pub detail: String,
    /// Its one button, if any.
    pub action: Option<MessageAction>,
}

/// How one scheduled job last ended, as recorded by the server.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum JobOutcome {
    /// It did its work.
    Ok,
    /// It was skipped.
    Skipped,
    /// It failed.
    Failed,
}

/// One job's latest run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobReport {
    /// The job's name in the server's `jobs` table.
    pub name: String,
    /// How it ended.
    pub outcome: JobOutcome,
    /// When it finished.
    pub finished: UnixMillis,
    /// The job's own short message.
    pub detail: String,
}

/// What the server knows about itself, for deriving messages.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct ServerState {
    /// What's wrong, and since when, if the server is unhealthy.
    pub problem: Option<(ServerProblem, UnixMillis)>,
    /// Each job's latest run (jobs that never ran are left out).
    pub jobs: Vec<JobReport>,
}

/// The messages the server's own state gives: one per failed or skipped job, and one while
/// the server is unhealthy.
#[must_use]
pub fn server_messages(state: &ServerState) -> Vec<Message> {
    let unhealthy = state.problem.map(|(problem, since)| Message {
        key: MessageKey(format!(
            "server-unhealthy:{}:{}",
            problem_name(problem),
            since.as_millis()
        )),
        kind: MessageKind::ServerUnhealthy { problem },
        severity: Severity::Urgent,
        audience: Audience::Everyone,
        raised_at: since,
        detail: String::new(),
        action: Some(MessageAction::OpenDiagnostics),
    });
    unhealthy
        .into_iter()
        .chain(state.jobs.iter().filter_map(job_message))
        .collect()
}

/// The message for a job's latest run, if it didn't go well.
fn job_message(report: &JobReport) -> Option<Message> {
    let job = JobKind::from_name(&report.name);
    let (prefix, kind) = match report.outcome {
        JobOutcome::Ok => return None,
        JobOutcome::Failed => ("job-failed", MessageKind::JobFailed { job: job.clone() }),
        JobOutcome::Skipped => ("job-skipped", MessageKind::JobSkipped { job: job.clone() }),
    };
    // A failed database check is as serious as an unhealthy server; anything else can wait
    // for the Admin's next look.
    let severity = if matches!(
        kind,
        MessageKind::JobFailed {
            job: JobKind::QuickCheck
        }
    ) {
        Severity::Urgent
    } else {
        Severity::Warning
    };
    Some(Message {
        key: MessageKey(format!(
            "{prefix}:{}:{}",
            job.name(),
            report.finished.as_millis()
        )),
        kind,
        severity,
        audience: Audience::AdminMode,
        raised_at: report.finished,
        detail: report.detail.clone(),
        action: Some(MessageAction::OpenDiagnostics),
    })
}

/// A problem's stable name, for keys.
const fn problem_name(problem: ServerProblem) -> &'static str {
    match problem {
        ServerProblem::DatabaseUnavailable => "database-unavailable",
        ServerProblem::IntegrityCheckFailed => "integrity-check-failed",
    }
}

/// A message read and dismissed record: one per person and message key.
#[derive(Debug)]
pub enum MessageMarkRecord {}

/// The ID of a message mark.
pub type MessageMarkId = Id<MessageMarkRecord>;

/// One person's mark on one message.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageMark {
    /// Whose mark.
    pub user: UserId,
    /// Which message.
    pub key: MessageKey,
    /// They've seen it.
    pub read: bool,
    /// They've put it away; it no longer shows.
    pub dismissed: bool,
}

/// Changed fields of a message mark; `None` means unchanged.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MessageMarkPatch {
    /// See [`MessageMark::user`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user: Option<UserId>,
    /// See [`MessageMark::key`] (set when created).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<MessageKey>,
    /// See [`MessageMark::read`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read: Option<bool>,
    /// See [`MessageMark::dismissed`].
    #[serde(skip_serializing_if = "Option::is_none")]
    pub dismissed: Option<bool>,
}

impl Patch for MessageMarkPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("user", self.user.is_some()),
            ("key", self.key.is_some()),
            ("read", self.read.is_some()),
            ("dismissed", self.dismissed.is_some()),
        ]
        .into_iter()
        .filter_map(|(field, set)| set.then_some(field))
        .collect()
    }

    fn keep_only(&mut self, fields: &[&str]) {
        let keep = |name: &str| fields.contains(&name);
        if !keep("user") {
            self.user = None;
        }
        if !keep("key") {
            self.key = None;
        }
        if !keep("read") {
            self.read = None;
        }
        if !keep("dismissed") {
            self.dismissed = None;
        }
    }
}

impl Record for MessageMarkRecord {
    const ENTITY: &'static str = "message_mark";
    type Patch = MessageMarkPatch;
}

/// A message as one person sees it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct InboxItem {
    /// The message.
    pub message: Message,
    /// They've read it.
    pub read: bool,
}

/// One person's inbox: what they can see and haven't dismissed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inbox {
    /// Unread first, then newest first.
    pub items: Vec<InboxItem>,
    /// How many are unread (the badge).
    pub unread: usize,
}

/// The inbox for someone working in `mode`, with their `marks` applied: messages they can't
/// see and ones they've dismissed are left out; details only show in Admin mode.
#[must_use]
pub fn inbox(messages: &[Message], marks: &[MessageMark], mode: ActiveMode) -> Inbox {
    let in_admin = mode == ActiveMode::Admin;
    let mark_for = |key: &MessageKey| marks.iter().find(|mark| &mark.key == key);

    // Visible, not dismissed, with read state and details settled for this person.
    let mut items: Vec<InboxItem> = messages
        .iter()
        .filter(|message| in_admin || message.audience == Audience::Everyone)
        .filter(|message| !mark_for(&message.key).is_some_and(|mark| mark.dismissed))
        .map(|message| {
            let mut message = message.clone();
            if !in_admin {
                message.detail.clear();
            }
            let read = mark_for(&message.key).is_some_and(|mark| mark.read);
            InboxItem { message, read }
        })
        .collect();
    // Unread first (false sorts before true), then newest first.
    items.sort_by(|a, b| {
        a.read
            .cmp(&b.read)
            .then(b.message.raised_at.cmp(&a.message.raised_at))
    });
    let unread = items.iter().filter(|item| !item.read).count();
    Inbox { items, unread }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::test_support::test_id;

    fn job(name: &str, outcome: JobOutcome, finished: i64) -> JobReport {
        JobReport {
            name: name.to_owned(),
            outcome,
            finished: UnixMillis::from_millis(finished),
            detail: format!("{name} detail"),
        }
    }

    fn mark(message: &Message, read: bool, dismissed: bool) -> MessageMark {
        MessageMark {
            user: test_id(1),
            key: message.key.clone(),
            read,
            dismissed,
        }
    }

    #[test]
    fn a_healthy_server_with_good_jobs_has_no_messages() {
        let state = ServerState {
            problem: None,
            jobs: vec![
                job("nightly-snapshot", JobOutcome::Ok, 1),
                job("quick-check", JobOutcome::Ok, 2),
            ],
        };
        assert!(server_messages(&state).is_empty());
    }

    #[test]
    fn a_failed_snapshot_is_an_admin_warning_keyed_by_its_run() {
        let state = ServerState {
            problem: None,
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 1_000)],
        };
        let messages = server_messages(&state);
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.key.as_str(), "job-failed:nightly-snapshot:1000");
        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.audience, Audience::AdminMode);
        assert_eq!(message.kind.title(), messages::INBOX_SNAPSHOT_FAILED);
        assert_eq!(message.detail, "nightly-snapshot detail");
    }

    #[test]
    fn a_later_failure_is_a_new_message() {
        let first = server_messages(&ServerState {
            problem: None,
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 1_000)],
        });
        let second = server_messages(&ServerState {
            problem: None,
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 2_000)],
        });
        assert_ne!(first[0].key, second[0].key);
    }

    #[test]
    fn a_skipped_snapshot_and_a_failed_check_have_their_own_titles() {
        let messages = server_messages(&ServerState {
            problem: None,
            jobs: vec![
                job("nightly-snapshot", JobOutcome::Skipped, 1),
                job("quick-check", JobOutcome::Failed, 2),
                job("something-new", JobOutcome::Failed, 3),
            ],
        });
        let titles: Vec<MessageId> = messages.iter().map(|m| m.kind.title()).collect();
        assert_eq!(
            titles,
            vec![
                messages::INBOX_SNAPSHOT_SKIPPED,
                messages::INBOX_QUICK_CHECK_FAILED,
                messages::INBOX_JOB_FAILED,
            ]
        );
        assert_eq!(messages[1].severity, Severity::Urgent, "a failed check");
        let bodies: Vec<MessageId> = messages.iter().map(|m| m.kind.body()).collect();
        assert_eq!(
            bodies,
            vec![
                messages::INBOX_SNAPSHOT_BODY,
                messages::INBOX_QUICK_CHECK_BODY,
                messages::INBOX_JOB_BODY,
            ],
            "only a snapshot's text mentions backups"
        );
    }

    #[test]
    fn an_unhealthy_server_is_urgent_for_everyone() {
        let since = UnixMillis::from_millis(5);
        let messages = server_messages(&ServerState {
            problem: Some((ServerProblem::IntegrityCheckFailed, since)),
            jobs: vec![],
        });
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0].severity, Severity::Urgent);
        assert_eq!(messages[0].audience, Audience::Everyone);
        assert_eq!(
            messages[0].key.as_str(),
            "server-unhealthy:integrity-check-failed:5"
        );
        assert_eq!(
            messages[0].kind.body(),
            messages::INBOX_INTEGRITY_CHECK_FAILED
        );
    }

    #[test]
    fn admin_messages_show_only_in_admin_mode() {
        let messages = server_messages(&ServerState {
            problem: Some((
                ServerProblem::DatabaseUnavailable,
                UnixMillis::from_millis(1),
            )),
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 2)],
        });
        assert_eq!(inbox(&messages, &[], ActiveMode::Admin).items.len(), 2);
        for mode in [ActiveMode::Driver, ActiveMode::Maintainer] {
            let seen = inbox(&messages, &[], mode);
            assert_eq!(seen.items.len(), 1, "{mode:?}");
            assert_eq!(seen.items[0].message.audience, Audience::Everyone);
        }
    }

    #[test]
    fn details_show_only_in_admin_mode() {
        let mut messages = server_messages(&ServerState {
            problem: Some((
                ServerProblem::DatabaseUnavailable,
                UnixMillis::from_millis(1),
            )),
            jobs: vec![],
        });
        messages[0].detail = "file is not a database".into();
        assert_eq!(
            inbox(&messages, &[], ActiveMode::Admin).items[0]
                .message
                .detail,
            "file is not a database"
        );
        assert!(
            inbox(&messages, &[], ActiveMode::Driver).items[0]
                .message
                .detail
                .is_empty()
        );
    }

    #[test]
    fn marks_hide_dismissed_messages_and_count_only_unread_ones() {
        let messages = server_messages(&ServerState {
            problem: None,
            jobs: vec![
                job("a", JobOutcome::Failed, 1),
                job("b", JobOutcome::Failed, 2),
                job("c", JobOutcome::Failed, 3),
            ],
        });
        let marks = [
            mark(&messages[0], true, false),
            mark(&messages[1], false, true),
        ];
        let seen = inbox(&messages, &marks, ActiveMode::Admin);
        let keys: Vec<&str> = seen.items.iter().map(|i| i.message.key.as_str()).collect();
        assert_eq!(
            keys,
            vec!["job-failed:c:3", "job-failed:a:1"],
            "b is dismissed"
        );
        assert_eq!(seen.unread, 1);
    }

    #[test]
    fn unread_come_first_then_newest() {
        let messages = server_messages(&ServerState {
            problem: None,
            jobs: vec![
                job("old-unread", JobOutcome::Failed, 1),
                job("new-read", JobOutcome::Failed, 3),
                job("new-unread", JobOutcome::Failed, 2),
            ],
        });
        let marks = [mark(&messages[1], true, false)];
        let seen = inbox(&messages, &marks, ActiveMode::Admin);
        let names: Vec<&str> = seen
            .items
            .iter()
            .map(|item| match &item.message.kind {
                MessageKind::JobFailed { job } => job.name(),
                MessageKind::JobSkipped { .. } | MessageKind::ServerUnhealthy { .. } => "",
            })
            .collect();
        assert_eq!(names, vec!["new-unread", "old-unread", "new-read"]);
    }

    #[test]
    fn message_keys_from_clients_are_checked_for_length() {
        assert!(MessageKey::parse("job-failed:nightly-snapshot:1").is_some());
        assert!(MessageKey::parse("").is_none());
        assert!(MessageKey::parse(&"x".repeat(MessageKey::MAX_LENGTH + 1)).is_none());
    }

    #[test]
    fn a_mark_patch_lists_and_keeps_its_fields() {
        let mut patch = MessageMarkPatch {
            read: Some(true),
            dismissed: Some(true),
            ..MessageMarkPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["read", "dismissed"]);
        patch.keep_only(&["dismissed"]);
        assert_eq!(patch.changed_fields(), vec!["dismissed"]);
    }

    #[test]
    fn messages_survive_the_trip_to_the_app() {
        let messages = server_messages(&ServerState {
            problem: Some((
                ServerProblem::DatabaseUnavailable,
                UnixMillis::from_millis(1),
            )),
            jobs: vec![job("nightly-snapshot", JobOutcome::Skipped, 2)],
        });
        let seen = inbox(&messages, &[], ActiveMode::Admin);
        let json = serde_json::to_string(&seen).unwrap();
        assert_eq!(serde_json::from_str::<Inbox>(&json).unwrap(), seen);
    }
}
