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
//!     place_lookup_problem: None,
//! };
//! let messages = server_messages(&state);
//! // An Admin message: shown in Admin mode, not in Driver mode.
//! assert_eq!(inbox(&messages, &[], ActiveMode::Admin, UnixMillis::from_millis(0), &[]).unread, 1);
//! assert_eq!(inbox(&messages, &[], ActiveMode::Driver, UnixMillis::from_millis(0), &[]).unread, 0);
//! ```

use core::fmt;

use serde::{Deserialize, Serialize};

use crate::access::ActiveMode;
use crate::command::{Clearable, Patch, Record};
use crate::dates::CalendarDate;
use crate::due::{Due, DueStep};
use crate::event::{Event, EventId};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::meter::MeterId;
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
    /// The nightly check and clean-up of the photo and document files (`file-check`).
    FileCheck,
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
            "file-check" => Self::FileCheck,
            other => Self::Other(other.to_owned()),
        }
    }

    /// The job's stable name.
    #[must_use]
    pub fn name(&self) -> &str {
        match self {
            Self::NightlySnapshot => "nightly-snapshot",
            Self::QuickCheck => "quick-check",
            Self::FileCheck => "file-check",
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
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
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
    /// The server couldn't open its place data, so coordinates can't be looked up (#059).
    PlaceLookupUnavailable,
    /// A maintenance or repair draft left untouched (#034, milestone 7): for whoever logged
    /// it and the vehicle's Maintainers.
    ForgottenJob {
        /// The draft.
        event: EventId,
        /// The vehicle's name, for the text.
        vehicle: String,
    },
    /// A vehicle sent out for service and not back by the day after it was expected (#067,
    /// milestone 9): for whoever sent it out and the vehicle's Maintainers.
    OverdueReturn {
        /// The send-out.
        event: EventId,
        /// The vehicle's name, for the text.
        vehicle: String,
        /// When it was expected back.
        expected: CalendarDate,
    },
    /// A desktop was unlocked after wrong PINs or passwords (phase-3.md 3d, #074): for the
    /// household Admin.
    FailedUnlocks {
        /// The device.
        device: crate::id::DeviceId,
        /// Its name, for the text.
        device_name: String,
        /// Whose it is.
        person: String,
        /// How many wrong tries came before the unlock.
        count: u32,
    },
    /// A desktop sent changes with its clock far ahead of the server's (phase-3.md 5c, #076):
    /// for the household Admin. Its changes wait until its clock is right.
    ClockAhead {
        /// The device.
        device: crate::id::DeviceId,
        /// Its name, for the text.
        device_name: String,
        /// Whose it is.
        person: String,
        /// How far ahead, in whole minutes (at least 1).
        minutes: i64,
    },
    /// Something on a vehicle comes due (Phase 5 milestone 3, #083): for anyone whose role
    /// covers the vehicle.
    Due(DueNotice),
}

/// What a due message is about (#083).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DueNotice {
    /// The vehicle.
    pub asset: crate::asset::AssetId,
    /// Its name, for the text.
    pub vehicle: String,
    /// What comes due.
    pub subject: crate::api::DueSubject,
    /// Where it stands now.
    pub due: Due,
    /// The vehicle's meters its rules count on, for what's left ("in 310 mi").
    #[serde(default)]
    pub meters: Vec<(MeterId, crate::meter::Meter)>,
    /// The meter a snooze counts on (`crate::due::snooze_meter`); `None`: time only.
    #[serde(default)]
    pub snooze_meter: Option<MeterId>,
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
                job: JobKind::FileCheck,
            } => messages::INBOX_FILE_CHECK_FAILED,
            Self::JobFailed {
                job: JobKind::Other(_),
            } => messages::INBOX_JOB_FAILED,
            Self::JobSkipped {
                job: JobKind::NightlySnapshot,
            } => messages::INBOX_SNAPSHOT_SKIPPED,
            Self::JobSkipped { .. } => messages::INBOX_JOB_SKIPPED,
            Self::ServerUnhealthy { .. } => messages::INBOX_SERVER_UNHEALTHY,
            Self::PlaceLookupUnavailable => messages::INBOX_PLACE_LOOKUP_UNAVAILABLE,
            Self::ForgottenJob { .. } => messages::INBOX_FORGOTTEN_JOB,
            Self::OverdueReturn { .. } => messages::INBOX_OVERDUE_RETURN,
            Self::FailedUnlocks { .. } => messages::INBOX_FAILED_UNLOCKS,
            Self::ClockAhead { .. } => messages::INBOX_CLOCK_AHEAD,
            Self::Due(notice) => match notice.due.step {
                DueStep::Ok | DueStep::ComingUp => messages::INBOX_DUE_COMING_UP,
                DueStep::DueSoon => messages::INBOX_DUE_SOON,
                DueStep::DueVerySoon => messages::INBOX_DUE_VERY_SOON,
                DueStep::Overdue => messages::INBOX_DUE_OVERDUE,
            },
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
            Self::JobFailed {
                job: JobKind::FileCheck,
            } => messages::INBOX_FILE_CHECK_BODY,
            Self::JobFailed { .. } | Self::JobSkipped { .. } => messages::INBOX_JOB_BODY,
            Self::ServerUnhealthy {
                problem: ServerProblem::DatabaseUnavailable,
            } => messages::INBOX_DATABASE_UNAVAILABLE,
            Self::ServerUnhealthy {
                problem: ServerProblem::IntegrityCheckFailed,
            } => messages::INBOX_INTEGRITY_CHECK_FAILED,
            Self::PlaceLookupUnavailable => messages::INBOX_PLACE_LOOKUP_BODY,
            Self::ForgottenJob { .. } => messages::INBOX_FORGOTTEN_JOB_BODY,
            Self::OverdueReturn { .. } => messages::INBOX_OVERDUE_RETURN_BODY,
            Self::FailedUnlocks { .. } => messages::INBOX_FAILED_UNLOCKS_BODY,
            Self::ClockAhead { .. } => messages::INBOX_CLOCK_AHEAD_BODY,
            Self::Due(_) => messages::INBOX_DUE_BODY,
        }
    }
}

/// The one thing a message's button does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum MessageAction {
    /// Open Diagnostics (the server's health and the log).
    OpenDiagnostics,
    /// Open an event's page (a forgotten job).
    OpenEvent {
        /// Which event.
        event: EventId,
    },
    /// Open a send-out's page (an overdue return).
    OpenSendOut {
        /// Which event.
        event: EventId,
    },
    /// Open a vehicle's Maintenance tab (a requirement coming due).
    OpenMaintenance {
        /// Which vehicle.
        asset: crate::asset::AssetId,
    },
    /// Open a vehicle's Documents tab (a document expiring).
    OpenDocuments {
        /// Which vehicle.
        asset: crate::asset::AssetId,
    },
}

impl MessageAction {
    /// The button's label.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::OpenDiagnostics => messages::INBOX_OPEN_DIAGNOSTICS,
            Self::OpenEvent { .. } => messages::INBOX_OPEN_JOB,
            Self::OpenSendOut { .. } => messages::INBOX_OPEN_SEND_OUT,
            Self::OpenMaintenance { .. } => messages::INBOX_OPEN_MAINTENANCE,
            Self::OpenDocuments { .. } => messages::INBOX_OPEN_DOCUMENTS,
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
    /// Since when, and why, the place data couldn't be opened, if it couldn't (#059).
    pub place_lookup_problem: Option<(UnixMillis, String)>,
}

/// The messages the server's own state gives: one per failed or skipped job, one while the
/// server is unhealthy, and one while place lookup is unavailable.
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
    // A warning for the Admin: saved places and typing a place still work (#059).
    let place_lookup = state
        .place_lookup_problem
        .as_ref()
        .map(|(since, detail)| Message {
            key: MessageKey(format!("place-lookup-unavailable:{}", since.as_millis())),
            kind: MessageKind::PlaceLookupUnavailable,
            severity: Severity::Warning,
            audience: Audience::AdminMode,
            raised_at: *since,
            detail: detail.clone(),
            action: Some(MessageAction::OpenDiagnostics),
        });
    unhealthy
        .into_iter()
        .chain(state.jobs.iter().filter_map(job_message))
        .chain(place_lookup)
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

/// The reminder for a forgotten job (#034): `event` (with ID `id`) on the vehicle named
/// `vehicle`, a maintenance or repair draft untouched for `delay_millis`. Editing the draft
/// ends it; a later lapse is a new message.
#[must_use]
pub fn forgotten_job_message(
    id: EventId,
    event: &Event,
    vehicle: &str,
    delay_millis: i64,
) -> Message {
    let edited = event.last_edited_at.as_millis();
    Message {
        key: MessageKey(format!("forgotten-job:{id}:{edited}")),
        kind: MessageKind::ForgottenJob {
            event: id,
            vehicle: vehicle.to_owned(),
        },
        severity: Severity::Warning,
        audience: Audience::Everyone,
        raised_at: UnixMillis::from_millis(edited + delay_millis),
        detail: String::new(),
        action: Some(MessageAction::OpenEvent { event: id }),
    }
}

/// The message for an overdue return (#067): the send-out `id` of the vehicle named `vehicle`,
/// expected back on `expected`, raised at `raised_at` (the start of the next day, as the server
/// counts it). Recording the return ends it; a new expected date is a new message.
#[must_use]
pub fn overdue_return_message(
    id: EventId,
    vehicle: &str,
    expected: &CalendarDate,
    raised_at: UnixMillis,
) -> Message {
    Message {
        key: MessageKey(format!("overdue-return:{id}:{}", expected.as_str())),
        kind: MessageKind::OverdueReturn {
            event: id,
            vehicle: vehicle.to_owned(),
            expected: expected.clone(),
        },
        severity: Severity::Warning,
        audience: Audience::Everyone,
        raised_at,
        detail: String::new(),
        action: Some(MessageAction::OpenSendOut { event: id }),
    }
}

/// The message for a desktop unlocked after `count` wrong tries, the last at `last_at`
/// (phase-3.md 3d, #074): for the household Admin, in Admin mode. A later report is a new
/// message.
#[must_use]
pub fn failed_unlocks_message(
    device: crate::id::DeviceId,
    device_name: &str,
    person: &str,
    count: u32,
    last_at: UnixMillis,
) -> Message {
    Message {
        key: MessageKey(format!("failed-unlocks:{device}:{}", last_at.as_millis())),
        kind: MessageKind::FailedUnlocks {
            device,
            device_name: device_name.to_owned(),
            person: person.to_owned(),
            count,
        },
        severity: Severity::Warning,
        audience: Audience::AdminMode,
        raised_at: last_at,
        detail: String::new(),
        action: None,
    }
}

/// The Admin's message that `device`'s clock was found `ahead_millis` ahead of the server's at
/// `at`, while it sent changes (one message per time it's flagged).
#[must_use]
pub fn clock_ahead_message(
    device: crate::id::DeviceId,
    device_name: &str,
    person: &str,
    ahead_millis: i64,
    at: UnixMillis,
) -> Message {
    Message {
        key: MessageKey(format!("clock-ahead:{device}:{}", at.as_millis())),
        kind: MessageKind::ClockAhead {
            device,
            device_name: device_name.to_owned(),
            person: person.to_owned(),
            minutes: (ahead_millis / 60_000).max(1),
        },
        severity: Severity::Warning,
        audience: Audience::AdminMode,
        raised_at: at,
        detail: String::new(),
        action: None,
    }
}

/// The message for `notice`'s item coming due (#083), if it has reached `from` (the
/// household's starting step) or is overdue. `cycle` is the finished task its count runs from
/// (`None`: never done, or a document).
///
/// Its key names the item and its cycle, so a sign-off starts a new message. **Overdue has a
/// key of its own,** so it comes back unread whatever was done with the first (#083). It's
/// raised when the item reached its step ([`Due::since`]), else at `now`.
#[must_use]
pub fn due_message(
    notice: DueNotice,
    cycle: Option<EventId>,
    from: DueStep,
    now: UnixMillis,
) -> Option<Message> {
    use crate::api::DueSubject;
    let (asset, due) = (notice.asset, &notice.due);
    if due.step == DueStep::Ok || (due.step < from && due.step != DueStep::Overdue) {
        return None;
    }
    let (item, action) = match &notice.subject {
        DueSubject::Requirement { id, .. } => (
            format!("requirement:{id}"),
            MessageAction::OpenMaintenance { asset },
        ),
        DueSubject::Document { id, .. } => (
            format!("document:{id}"),
            MessageAction::OpenDocuments { asset },
        ),
    };
    let cycle = cycle.map_or_else(|| "first".to_owned(), |event| event.to_string());
    let prefix = if due.step == DueStep::Overdue {
        "due-overdue"
    } else {
        "due"
    };
    let severity = if due.step == DueStep::ComingUp {
        Severity::Info
    } else {
        Severity::Warning
    };
    let raised_at = due.since().unwrap_or(now);
    Some(Message {
        key: MessageKey(format!("{prefix}:{item}:{cycle}")),
        kind: MessageKind::Due(notice),
        severity,
        audience: Audience::Everyone,
        raised_at,
        detail: String::new(),
        action: Some(action),
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
    /// Snoozed: hidden until it wakes (#083).
    #[serde(default)]
    pub snooze: Option<Snooze>,
}

/// One day, in milliseconds.
const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

/// A meter value a snooze waits for: it wakes once the meter's latest reading reaches `at`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnoozeMeter {
    /// Which meter (one that counts up).
    pub meter: MeterId,
    /// The value, in its stored unit.
    pub at: i64,
}

/// How long a message sleeps (#083): until a time, until a meter reaches a value, or both,
/// whichever comes first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Snooze {
    /// When it wakes, if by time.
    #[serde(default)]
    pub until: Option<UnixMillis>,
    /// What it waits for on a meter, if by distance or hours.
    #[serde(default)]
    pub meter: Option<SnoozeMeter>,
}

impl Snooze {
    /// Whether it has woken at `now`, given each meter's latest value: either limit passed
    /// (one with neither is awake).
    #[must_use]
    pub fn awake(&self, now: UnixMillis, latest: &[(MeterId, i64)]) -> bool {
        let by_time = self.until.is_some_and(|until| now >= until);
        let by_meter = self.meter.is_some_and(|wait| {
            latest
                .iter()
                .any(|(meter, value)| *meter == wait.meter && *value >= wait.at)
        });
        by_time || by_meter || (self.until.is_none() && self.meter.is_none())
    }
}

/// The longest snooze, in days.
pub const MAX_SNOOZE_DAYS: u16 = 366;

/// What someone asks for when they snooze (#083): a number of days, an amount on the item's
/// meter, or both. The quick choices are these too ("1 week or 500 mi" is 7 days and 500 mi).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SnoozeRequest {
    /// For this many days (1 to [`MAX_SNOOZE_DAYS`]).
    #[serde(default)]
    pub days: Option<u16>,
    /// For this much more on the item's meter, in its stored unit (more than zero).
    #[serde(default)]
    pub amount: Option<i64>,
}

impl SnoozeRequest {
    /// The snooze it makes at `now`, with the item's `meter` and its latest value (if it has
    /// one that counts up). `None` if it asks for nothing usable: no days and no amount, days
    /// out of range, an amount of zero or less, or an amount with no meter.
    #[must_use]
    pub fn snooze(self, now: UnixMillis, meter: Option<(MeterId, i64)>) -> Option<Snooze> {
        let until = match self.days {
            None => None,
            Some(days) if (1..=MAX_SNOOZE_DAYS).contains(&days) => Some(UnixMillis::from_millis(
                now.as_millis().checked_add(i64::from(days) * DAY_MS)?,
            )),
            Some(_) => return None,
        };
        let meter = match self.amount {
            None => None,
            Some(amount) if amount > 0 => {
                let (meter, latest) = meter?;
                Some(SnoozeMeter {
                    meter,
                    at: latest.checked_add(amount)?,
                })
            }
            Some(_) => return None,
        };
        (until.is_some() || meter.is_some()).then_some(Snooze { until, meter })
    }
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
    /// See [`MessageMark::snooze`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub snooze: Option<Clearable<Snooze>>,
}

impl Patch for MessageMarkPatch {
    fn changed_fields(&self) -> Vec<&'static str> {
        [
            ("user", self.user.is_some()),
            ("key", self.key.is_some()),
            ("read", self.read.is_some()),
            ("dismissed", self.dismissed.is_some()),
            ("snooze", self.snooze.is_some()),
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
        if !keep("snooze") {
            self.snooze = None;
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
    /// Its snooze, while it sleeps (only on [`Inbox::snoozed`]).
    #[serde(default)]
    pub snooze: Option<Snooze>,
}

/// One person's inbox: what they can see and haven't dismissed.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Inbox {
    /// Unread first, then newest first.
    pub items: Vec<InboxItem>,
    /// How many are unread (the badge).
    pub unread: usize,
    /// Snoozed and still asleep (not in `items` or `unread`), by when they wake, soonest first, then those waiting on a meter (#083).
    #[serde(default)]
    pub snoozed: Vec<InboxItem>,
}

/// The inbox for someone working in `mode`, with their `marks` applied: messages they can't
/// see and ones they've dismissed are left out; details only show in Admin mode. Snoozed ones
/// still asleep at `now`, given each meter's `latest` value, are set aside in
/// [`Inbox::snoozed`] (#083).
#[must_use]
pub fn inbox(
    messages: &[Message],
    marks: &[MessageMark],
    mode: ActiveMode,
    now: UnixMillis,
    latest: &[(MeterId, i64)],
) -> Inbox {
    let in_admin = mode == ActiveMode::Admin;
    let mark_for = |key: &MessageKey| marks.iter().find(|mark| &mark.key == key);

    // Visible, not dismissed, with read state, snooze, and details settled for this person.
    let (asleep, mut items): (Vec<InboxItem>, Vec<InboxItem>) = messages
        .iter()
        .filter(|message| in_admin || message.audience == Audience::Everyone)
        .filter(|message| !mark_for(&message.key).is_some_and(|mark| mark.dismissed))
        .map(|message| {
            let mut message = message.clone();
            if !in_admin {
                message.detail.clear();
            }
            let mark = mark_for(&message.key);
            let read = mark.is_some_and(|mark| mark.read);
            let snooze = mark
                .and_then(|mark| mark.snooze)
                .filter(|snooze| !snooze.awake(now, latest));
            InboxItem {
                message,
                read,
                snooze,
            }
        })
        .partition(|item| item.snooze.is_some());
    let mut snoozed = asleep;
    // By time first (soonest first), then those waiting only on a meter.
    snoozed.sort_by_key(|item| {
        let until = item.snooze.and_then(|snooze| snooze.until);
        (until.is_none(), until)
    });
    // Unread first (false sorts before true), then newest first.
    items.sort_by(|a, b| {
        a.read
            .cmp(&b.read)
            .then(b.message.raised_at.cmp(&a.message.raised_at))
    });
    let unread = items.iter().filter(|item| !item.read).count();
    Inbox {
        items,
        unread,
        snoozed,
    }
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
            snooze: None,
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
            place_lookup_problem: None,
        };
        assert!(server_messages(&state).is_empty());
    }

    #[test]
    fn a_failed_snapshot_is_an_admin_warning_keyed_by_its_run() {
        let state = ServerState {
            problem: None,
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 1_000)],
            place_lookup_problem: None,
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
            place_lookup_problem: None,
        });
        let second = server_messages(&ServerState {
            problem: None,
            jobs: vec![job("nightly-snapshot", JobOutcome::Failed, 2_000)],
            place_lookup_problem: None,
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
            place_lookup_problem: None,
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
            place_lookup_problem: None,
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
            place_lookup_problem: None,
        });
        assert_eq!(
            inbox(
                &messages,
                &[],
                ActiveMode::Admin,
                UnixMillis::from_millis(0),
                &[]
            )
            .items
            .len(),
            2
        );
        for mode in [ActiveMode::Driver, ActiveMode::Maintainer] {
            let seen = inbox(&messages, &[], mode, UnixMillis::from_millis(0), &[]);
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
            place_lookup_problem: None,
        });
        messages[0].detail = "file is not a database".into();
        assert_eq!(
            inbox(
                &messages,
                &[],
                ActiveMode::Admin,
                UnixMillis::from_millis(0),
                &[]
            )
            .items[0]
                .message
                .detail,
            "file is not a database"
        );
        assert!(
            inbox(
                &messages,
                &[],
                ActiveMode::Driver,
                UnixMillis::from_millis(0),
                &[]
            )
            .items[0]
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
            place_lookup_problem: None,
        });
        let marks = [
            mark(&messages[0], true, false),
            mark(&messages[1], false, true),
        ];
        let seen = inbox(
            &messages,
            &marks,
            ActiveMode::Admin,
            UnixMillis::from_millis(0),
            &[],
        );
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
            place_lookup_problem: None,
        });
        let marks = [mark(&messages[1], true, false)];
        let seen = inbox(
            &messages,
            &marks,
            ActiveMode::Admin,
            UnixMillis::from_millis(0),
            &[],
        );
        let names: Vec<&str> = seen
            .items
            .iter()
            .map(|item| match &item.message.kind {
                MessageKind::JobFailed { job } => job.name(),
                MessageKind::JobSkipped { .. }
                | MessageKind::ServerUnhealthy { .. }
                | MessageKind::PlaceLookupUnavailable
                | MessageKind::ForgottenJob { .. }
                | MessageKind::OverdueReturn { .. }
                | MessageKind::FailedUnlocks { .. }
                | MessageKind::ClockAhead { .. }
                | MessageKind::Due(_) => "",
            })
            .collect();
        assert_eq!(names, vec!["new-unread", "old-unread", "new-read"]);
    }

    #[test]
    fn an_overdue_return_is_one_message_per_expected_date() {
        let date = |text| CalendarDate::parse(text).unwrap();
        let at = UnixMillis::from_millis(5_000);
        let first = overdue_return_message(test_id(7), "Tacoma", &date("2026-11-10"), at);
        assert_eq!(
            first.key.as_str(),
            format!("overdue-return:{}:2026-11-10", test_id::<Event>(7))
        );
        assert_eq!(
            first.action,
            Some(MessageAction::OpenSendOut { event: test_id(7) })
        );
        assert_eq!(first.audience, Audience::Everyone);
        let later = overdue_return_message(test_id(7), "Tacoma", &date("2026-11-20"), at);
        assert_ne!(first.key, later.key, "a new expected date is a new message");
    }

    #[test]
    fn a_forgotten_job_is_raised_once_per_lapse_and_opens_the_job() {
        use crate::dates::EventDates;
        use crate::event::{EventLocation, EventSource, EventStatus};
        use crate::location::{LocationSource, TimeZone};
        use crate::meter::EventKind;

        let mut event = Event {
            asset: test_id(10),
            kind: EventKind::Maintenance,
            dates: EventDates {
                start: UnixMillis::from_millis(1_000),
                end: None,
                zone: TimeZone::parse("UTC").unwrap(),
            },
            location: EventLocation {
                place: None,
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![],
            skipped: vec![],
            fields: vec![],
            notes: crate::notes::Notes::default(),
            source: EventSource::Typed,
            logged_by: test_id(1),
            performed_by: None,
            fuel: None,
            work: None,
            lifecycle: None,
            reading_override: None,
            status: EventStatus::Draft,
            last_edited_at: UnixMillis::from_millis(5_000),
            received_at: UnixMillis::from_millis(5_000),
            finalized: None,
            corrects: None,
            correction_reason: None,
        };
        let id = test_id(100);
        let message = forgotten_job_message(id, &event, "Truck", 60_000);
        assert_eq!(message.raised_at, UnixMillis::from_millis(65_000));
        assert_eq!(message.audience, Audience::Everyone);
        assert_eq!(message.action, Some(MessageAction::OpenEvent { event: id }));
        assert_eq!(
            message.kind,
            MessageKind::ForgottenJob {
                event: id,
                vehicle: "Truck".into()
            }
        );
        assert!(MessageKey::parse(message.key.as_str()).is_some());

        event.last_edited_at = UnixMillis::from_millis(9_000);
        let later = forgotten_job_message(id, &event, "Truck", 60_000);
        assert_ne!(later.key, message.key, "a new lapse is a new message");
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
            place_lookup_problem: None,
        });
        let seen = inbox(
            &messages,
            &[],
            ActiveMode::Admin,
            UnixMillis::from_millis(0),
            &[],
        );
        let json = serde_json::to_string(&seen).unwrap();
        assert_eq!(serde_json::from_str::<Inbox>(&json).unwrap(), seen);
    }

    #[test]
    fn missing_place_data_is_a_warning_for_the_admin() {
        let messages = server_messages(&ServerState {
            place_lookup_problem: Some((UnixMillis::from_millis(5), "no such file".into())),
            ..ServerState::default()
        });
        assert_eq!(messages.len(), 1);
        let message = &messages[0];
        assert_eq!(message.key.as_str(), "place-lookup-unavailable:5");
        assert_eq!(message.severity, Severity::Warning);
        assert_eq!(message.audience, Audience::AdminMode);
        assert!(
            inbox(
                &messages,
                &[],
                ActiveMode::Driver,
                UnixMillis::from_millis(0),
                &[]
            )
            .items
            .is_empty()
        );
        assert_eq!(
            inbox(
                &messages,
                &[],
                ActiveMode::Admin,
                UnixMillis::from_millis(0),
                &[]
            )
            .items
            .len(),
            1
        );
    }

    #[test]
    fn failed_unlocks_are_for_the_admin_and_each_report_is_new() {
        let device = test_id(5);
        let first = failed_unlocks_message(
            device,
            "Garage Mac",
            "Sam",
            3,
            UnixMillis::from_millis(1_000),
        );
        let later = failed_unlocks_message(
            device,
            "Garage Mac",
            "Sam",
            2,
            UnixMillis::from_millis(9_000),
        );
        assert_eq!(first.audience, Audience::AdminMode);
        assert_ne!(first.key, later.key);
        let messages = [first];
        assert_eq!(
            inbox(
                &messages,
                &[],
                ActiveMode::Admin,
                UnixMillis::from_millis(0),
                &[]
            )
            .unread,
            1
        );
        assert_eq!(
            inbox(
                &messages,
                &[],
                ActiveMode::Maintainer,
                UnixMillis::from_millis(0),
                &[]
            )
            .unread,
            0
        );
    }

    fn oil_due(step: DueStep, since: Option<i64>) -> Due {
        Due {
            step,
            rules: vec![crate::due::RuleDue {
                step,
                left: crate::due::Left::Date {
                    due: "2026-12-01".into(),
                },
                since: since.map(UnixMillis::from_millis),
            }],
            date: None,
            not_done_yet: false,
        }
    }

    fn notice(subject: crate::api::DueSubject, due: Due) -> DueNotice {
        DueNotice {
            asset: test_id(1),
            vehicle: "Outback".into(),
            subject,
            due,
            meters: vec![],
            snooze_meter: None,
        }
    }

    fn oil(step: DueStep, cycle: Option<EventId>) -> Option<Message> {
        due_message(
            notice(
                crate::api::DueSubject::Requirement {
                    id: test_id(20),
                    name: crate::label::Label::parse("Engine oil & filter").unwrap(),
                },
                oil_due(step, Some(4_000)),
            ),
            cycle,
            DueStep::DueSoon,
            UnixMillis::from_millis(9_000),
        )
    }

    #[test]
    fn a_due_message_starts_at_the_households_step() {
        assert_eq!(oil(DueStep::Ok, None), None);
        assert_eq!(oil(DueStep::ComingUp, None), None, "before Due soon");
        let soon = oil(DueStep::DueSoon, None).unwrap();
        assert_eq!(soon.severity, Severity::Warning);
        assert_eq!(soon.audience, Audience::Everyone);
        assert_eq!(
            soon.raised_at,
            UnixMillis::from_millis(4_000),
            "when it got there"
        );
        assert_eq!(soon.kind.title(), messages::INBOX_DUE_SOON);
        assert_eq!(
            soon.action,
            Some(MessageAction::OpenMaintenance { asset: test_id(1) })
        );
        // From Coming up, an Info.
        let early = due_message(
            notice(
                crate::api::DueSubject::Document {
                    id: test_id(30),
                    doc_type: crate::document::DocumentType::listed("document-insurance").unwrap(),
                },
                oil_due(DueStep::ComingUp, None),
            ),
            None,
            DueStep::ComingUp,
            UnixMillis::from_millis(9_000),
        )
        .unwrap();
        assert_eq!(early.severity, Severity::Info);
        assert_eq!(early.raised_at, UnixMillis::from_millis(9_000), "now");
        assert_eq!(
            early.action,
            Some(MessageAction::OpenDocuments { asset: test_id(1) })
        );
    }

    #[test]
    fn one_message_per_cycle_and_one_for_overdue() {
        let soon = oil(DueStep::DueSoon, None).unwrap();
        let very_soon = oil(DueStep::DueVerySoon, None).unwrap();
        let overdue = oil(DueStep::Overdue, None).unwrap();
        assert_eq!(soon.key, very_soon.key, "the same message, more urgent");
        assert_ne!(soon.key, overdue.key, "Overdue is its own");
        assert_eq!(overdue.kind.title(), messages::INBOX_DUE_OVERDUE);
        let next_cycle = oil(DueStep::DueSoon, Some(test_id(50))).unwrap();
        assert_ne!(soon.key, next_cycle.key, "a sign-off starts a new one");
        assert!(
            next_cycle
                .key
                .as_str()
                .ends_with(&test_id::<Event>(50).to_string())
        );
    }

    #[test]
    fn overdue_comes_back_after_a_dismiss_or_a_snooze() {
        let soon = oil(DueStep::DueSoon, None).unwrap();
        let snoozed = MessageMark {
            snooze: Some(Snooze {
                until: Some(UnixMillis::from_millis(100_000)),
                meter: None,
            }),
            ..mark(&soon, true, false)
        };
        let now = UnixMillis::from_millis(10_000);
        let asleep = inbox(
            std::slice::from_ref(&soon),
            std::slice::from_ref(&snoozed),
            ActiveMode::Driver,
            now,
            &[],
        );
        assert!(asleep.items.is_empty());
        assert_eq!(asleep.unread, 0);
        assert_eq!(asleep.snoozed.len(), 1);
        assert_eq!(asleep.snoozed[0].snooze, snoozed.snooze);
        // Overdue: a new message, unread, though the first was snoozed and another dismissed.
        let overdue = oil(DueStep::Overdue, None).unwrap();
        let dismissed = mark(
            &oil(DueStep::DueSoon, Some(test_id(50))).unwrap(),
            true,
            true,
        );
        let seen = inbox(
            std::slice::from_ref(&overdue),
            &[snoozed, dismissed],
            ActiveMode::Driver,
            now,
            &[],
        );
        assert_eq!(seen.unread, 1);
        assert_eq!(seen.items[0].message.key, overdue.key);
    }

    #[test]
    fn a_snooze_wakes_by_time_or_by_meter_whichever_first() {
        let odometer: MeterId = test_id(10);
        let snooze = Snooze {
            until: Some(UnixMillis::from_millis(100_000)),
            meter: Some(SnoozeMeter {
                meter: odometer,
                at: 5_000,
            }),
        };
        let early = UnixMillis::from_millis(99_999);
        assert!(!snooze.awake(early, &[(odometer, 4_999)]));
        assert!(snooze.awake(early, &[(odometer, 5_000)]), "the meter");
        assert!(
            snooze.awake(UnixMillis::from_millis(100_000), &[(odometer, 10)]),
            "the time"
        );
        assert!(
            !snooze.awake(early, &[(test_id(11), 9_999)]),
            "another meter"
        );
        // In the inbox: back, keeping its read state.
        let soon = oil(DueStep::DueSoon, None).unwrap();
        let marked = MessageMark {
            snooze: Some(snooze),
            ..mark(&soon, true, false)
        };
        let woken = inbox(
            &[soon],
            &[marked],
            ActiveMode::Driver,
            early,
            &[(odometer, 6_000)],
        );
        assert_eq!(woken.items.len(), 1);
        assert!(woken.items[0].read);
        assert_eq!(woken.items[0].snooze, None);
        assert!(woken.snoozed.is_empty());
    }

    #[test]
    fn a_snooze_request_makes_a_snooze() {
        let now = UnixMillis::from_millis(1_000);
        let odometer: MeterId = test_id(10);
        let both = SnoozeRequest {
            days: Some(7),
            amount: Some(500),
        };
        assert_eq!(
            both.snooze(now, Some((odometer, 80_000))),
            Some(Snooze {
                until: Some(UnixMillis::from_millis(1_000 + 7 * DAY_MS)),
                meter: Some(SnoozeMeter {
                    meter: odometer,
                    at: 80_500,
                }),
            })
        );
        assert_eq!(both.snooze(now, None), None, "an amount needs a meter");
        let days = |days| SnoozeRequest {
            days: Some(days),
            amount: None,
        };
        assert!(days(1).snooze(now, None).is_some());
        assert!(days(MAX_SNOOZE_DAYS).snooze(now, None).is_some());
        assert_eq!(days(0).snooze(now, None), None);
        assert_eq!(days(MAX_SNOOZE_DAYS + 1).snooze(now, None), None);
        let nothing = SnoozeRequest::default();
        assert_eq!(nothing.snooze(now, Some((odometer, 1))), None);
        let backwards = SnoozeRequest {
            days: None,
            amount: Some(-5),
        };
        assert_eq!(backwards.snooze(now, Some((odometer, 1))), None);
    }

    #[test]
    fn a_marks_snooze_is_a_field_of_its_own() {
        let patch = MessageMarkPatch {
            snooze: Some(Clearable::Clear),
            ..MessageMarkPatch::default()
        };
        assert_eq!(patch.changed_fields(), vec!["snooze"]);
        let mut kept = patch;
        kept.keep_only(&["read"]);
        assert_eq!(kept, MessageMarkPatch::default());
    }
}
