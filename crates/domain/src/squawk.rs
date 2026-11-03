//! Squawks: problems noticed on a vehicle, to deal with later (milestone 7; plan §3 Squawks;
//! decisions #051, #063).
//!
//! A squawk is **opened** by anyone who may drive the vehicle (a Driver or higher), standalone
//! or from an event it was found on. It has a description, notes, and a **severity**: Low,
//! Medium, or High; a new one starts at Low (#063). **High keeps the vehicle out of use**, and
//! each kind's profile names it ("Not drivable", "Not towable", "Out of service").
//!
//! It's then **dispositioned inside a maintenance or repair event:** signed off (resolved),
//! or deferred with a reason. **Deferrals don't carry over:** every later maintenance or
//! repair must disposition it again, and finishing one checks that every outstanding squawk
//! was (the plan's required disposition). A disposition counts once its event is finished;
//! the squawk's status and severity come from the latest one that counts.
//!
//! # Examples
//!
//! ```
//! use domain::squawk::{Severity, SquawkStatus, status};
//!
//! assert_eq!(Severity::default(), Severity::Low);
//! assert!(Severity::High.keeps_out_of_use());
//! assert_eq!(status(&[]), SquawkStatus::Open);
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Clearable, Record, record_patch};
use crate::event::{EventId, Reason};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::notes::Notes;
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of a squawk.
pub type SquawkId = Id<Squawk>;

/// The most characters a squawk's description may have.
pub const DESCRIPTION_MAX_CHARS: usize = 500;

/// What's wrong, in a few words: trimmed, 1–500 characters, e.g. "Brake light out on the
/// left side".
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct SquawkText(String);

impl SquawkText {
    /// A description from what was typed.
    ///
    /// # Errors
    ///
    /// [`SquawkProblem::DescriptionEmpty`] or [`SquawkProblem::DescriptionTooLong`].
    pub fn parse(input: &str) -> Result<Self, SquawkProblem> {
        let text = input.trim();
        match text.chars().count() {
            0 => Err(SquawkProblem::DescriptionEmpty),
            length if length > DESCRIPTION_MAX_CHARS => Err(SquawkProblem::DescriptionTooLong),
            _ => Ok(Self(text.to_owned())),
        }
    }

    /// The description.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for SquawkText {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|problem| format!("{problem:?}"))
    }
}

impl From<SquawkText> for String {
    fn from(text: SquawkText) -> Self {
        text.0
    }
}

/// How serious a squawk is (#063).
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum Severity {
    /// Minor or cosmetic: note it, deal with it sometime. Where every squawk starts.
    #[default]
    Low,
    /// Get it looked at.
    Medium,
    /// Keeps the vehicle out of use (aviation's "grounding" squawk).
    High,
}

impl Severity {
    /// Every severity, lowest first.
    pub const ALL: [Self; 3] = [Self::Low, Self::Medium, Self::High];

    /// The severity's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Low => messages::SEVERITY_LOW,
            Self::Medium => messages::SEVERITY_MEDIUM,
            Self::High => messages::SEVERITY_HIGH,
        }
    }

    /// Whether it keeps the vehicle out of use (Phase 5's readiness counts these).
    #[must_use]
    pub const fn keeps_out_of_use(self) -> bool {
        matches!(self, Self::High)
    }
}

/// One squawk.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Squawk {
    /// Which vehicle.
    pub asset: AssetId,
    /// What's wrong.
    pub description: SquawkText,
    /// More detail, if any (a Markdown subset, #055).
    pub notes: Notes,
    /// How serious it is, as opened (dispositions may change it later).
    pub severity: Severity,
    /// When it was noticed.
    pub opened_at: UnixMillis,
    /// Who opened it.
    pub opened_by: UserId,
    /// The event it was found on, if any (e.g. during a fuel-up or a maintenance job).
    pub found_on: Option<EventId>,
    /// When the server first received it.
    pub received_at: UnixMillis,
}

record_patch! {
    /// The changeable fields of a [`Squawk`]; `Some` means "set to this".
    SquawkPatch
    {
        /// See [`Squawk::asset`] (set when opened).
        asset: AssetId,
        /// See [`Squawk::description`].
        description: SquawkText,
        /// See [`Squawk::notes`].
        notes: Notes,
        /// See [`Squawk::severity`].
        severity: Severity,
        /// See [`Squawk::opened_at`].
        opened_at: UnixMillis,
        /// See [`Squawk::opened_by`] (set when opened).
        opened_by: UserId,
        /// See [`Squawk::found_on`] (set when opened).
        found_on: Clearable<EventId>,
        /// See [`Squawk::received_at`] (set when opened).
        received_at: UnixMillis,
    }
}

impl SquawkPatch {
    /// Every field of `squawk`, for opening it.
    #[must_use]
    pub fn all(squawk: Squawk) -> Self {
        Self {
            asset: Some(squawk.asset),
            description: Some(squawk.description),
            notes: Some(squawk.notes),
            severity: Some(squawk.severity),
            opened_at: Some(squawk.opened_at),
            opened_by: Some(squawk.opened_by),
            found_on: Some(Clearable::from_option(squawk.found_on)),
            received_at: Some(squawk.received_at),
        }
    }
}

impl Record for Squawk {
    const ENTITY: &'static str = "squawk";
    type Patch = SquawkPatch;
}

/// What a maintenance or repair event does about a squawk.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "action", rename_all = "kebab-case")]
pub enum Disposition {
    /// Fixed: the squawk is resolved.
    SignedOff {
        /// How it was resolved, if worth saying.
        notes: Notes,
        /// The task that fixed it, by its place in the event's task list, if one did.
        task: Option<u16>,
    },
    /// Not dealt with this time, and why. The next maintenance or repair asks again.
    Deferred {
        /// Why it's put off.
        reason: Reason,
    },
}

/// One squawk's disposition in a maintenance or repair event.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct SquawkDisposition {
    /// Which squawk.
    pub squawk: SquawkId,
    /// Signed off or deferred.
    pub action: Disposition,
    /// A new severity, if it changed (e.g. a deferred squawk that got worse).
    pub severity: Option<Severity>,
}

/// A disposition that counts: from a finished event, at its time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CountedDisposition {
    /// When the event happened.
    pub at: UnixMillis,
    /// Which event.
    pub event: EventId,
    /// What it did.
    pub disposition: SquawkDisposition,
}

/// Where a squawk stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum SquawkStatus {
    /// Not dealt with yet.
    Open,
    /// Put off, with a reason, at the latest maintenance or repair.
    Deferred,
    /// Signed off.
    Resolved,
}

impl SquawkStatus {
    /// The status's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Open => messages::SQUAWK_STATUS_OPEN,
            Self::Deferred => messages::SQUAWK_STATUS_DEFERRED,
            Self::Resolved => messages::SQUAWK_STATUS_RESOLVED,
        }
    }

    /// Whether a maintenance or repair must disposition it (open or deferred).
    #[must_use]
    pub const fn is_outstanding(self) -> bool {
        matches!(self, Self::Open | Self::Deferred)
    }
}

/// The latest counted disposition in `history` (by its event's time, then its event's ID).
fn latest(history: &[CountedDisposition]) -> Option<&CountedDisposition> {
    history
        .iter()
        .max_by_key(|counted| (counted.at, counted.event))
}

/// A squawk's status from its counted dispositions: open until the first one.
#[must_use]
pub fn status(history: &[CountedDisposition]) -> SquawkStatus {
    match latest(history).map(|counted| &counted.disposition.action) {
        None => SquawkStatus::Open,
        Some(Disposition::Deferred { .. }) => SquawkStatus::Deferred,
        Some(Disposition::SignedOff { .. }) => SquawkStatus::Resolved,
    }
}

/// A squawk's severity now: the latest counted change, or as opened.
#[must_use]
pub fn current_severity(squawk: &Squawk, history: &[CountedDisposition]) -> Severity {
    let mut changes: Vec<&CountedDisposition> = history
        .iter()
        .filter(|counted| counted.disposition.severity.is_some())
        .collect();
    changes.sort_by_key(|counted| (counted.at, counted.event));
    changes
        .last()
        .and_then(|counted| counted.disposition.severity)
        .unwrap_or(squawk.severity)
}

/// Why a squawk (or a disposition) was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", content = "squawk", rename_all = "kebab-case")]
pub enum SquawkProblem {
    /// A description with nothing in it.
    DescriptionEmpty,
    /// A description over 500 characters.
    DescriptionTooLong,
    /// Finishing with an outstanding squawk neither signed off nor deferred.
    Undispositioned(SquawkId),
    /// Two dispositions of one squawk in one event.
    DispositionedTwice(SquawkId),
    /// A disposition of a squawk that isn't outstanding on this vehicle.
    NotOutstanding(SquawkId),
}

impl SquawkProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::DescriptionEmpty => messages::SQUAWK_DESCRIPTION_EMPTY,
            Self::DescriptionTooLong => messages::SQUAWK_DESCRIPTION_TOO_LONG,
            Self::Undispositioned(_) => messages::SQUAWK_UNDISPOSITIONED,
            Self::DispositionedTwice(_) => messages::SQUAWK_DISPOSITIONED_TWICE,
            Self::NotOutstanding(_) => messages::SQUAWK_NOT_OUTSTANDING,
        }
    }
}

/// Checks an event's dispositions against the vehicle's outstanding squawks: each is for an
/// outstanding one, at most once. With `finishing`, every outstanding one must be there too
/// (the required disposition).
///
/// # Errors
///
/// The first [`SquawkProblem`] found.
pub fn check_dispositions(
    outstanding: &[SquawkId],
    dispositions: &[SquawkDisposition],
    finishing: bool,
) -> Result<(), SquawkProblem> {
    for (index, disposition) in dispositions.iter().enumerate() {
        if !outstanding.contains(&disposition.squawk) {
            return Err(SquawkProblem::NotOutstanding(disposition.squawk));
        }
        if dispositions[..index]
            .iter()
            .any(|earlier| earlier.squawk == disposition.squawk)
        {
            return Err(SquawkProblem::DispositionedTwice(disposition.squawk));
        }
    }
    if finishing
        && let Some(missing) = outstanding
            .iter()
            .find(|squawk| !dispositions.iter().any(|d| d.squawk == **squawk))
    {
        return Err(SquawkProblem::Undispositioned(*missing));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Patch;
    use crate::test_support::test_id;

    const DAY: i64 = 86_400_000;

    fn reason(text: &str) -> Reason {
        Reason::parse(text).unwrap()
    }

    fn squawk() -> Squawk {
        Squawk {
            asset: test_id(1),
            description: SquawkText::parse("Brake light out, left side").unwrap(),
            notes: Notes::default(),
            severity: Severity::default(),
            opened_at: UnixMillis::from_millis(0),
            opened_by: test_id(5),
            found_on: None,
            received_at: UnixMillis::from_millis(0),
        }
    }

    fn counted(
        n: u32,
        days: i64,
        action: Disposition,
        severity: Option<Severity>,
    ) -> CountedDisposition {
        CountedDisposition {
            at: UnixMillis::from_millis(days * DAY),
            event: test_id(n),
            disposition: SquawkDisposition {
                squawk: test_id(30),
                action,
                severity,
            },
        }
    }

    fn deferred(text: &str) -> Disposition {
        Disposition::Deferred {
            reason: reason(text),
        }
    }

    fn signed_off() -> Disposition {
        Disposition::SignedOff {
            notes: Notes::default(),
            task: Some(0),
        }
    }

    #[test]
    fn a_description_is_one_to_five_hundred_characters() {
        assert_eq!(
            SquawkText::parse("  Wiper chatters  ").unwrap().as_str(),
            "Wiper chatters"
        );
        assert_eq!(SquawkText::parse(" "), Err(SquawkProblem::DescriptionEmpty));
        assert_eq!(
            SquawkText::parse(&"x".repeat(501)),
            Err(SquawkProblem::DescriptionTooLong)
        );
        assert!(
            SquawkText::parse(&"é".repeat(500)).is_ok(),
            "characters, not bytes"
        );
    }

    #[test]
    fn severity_starts_low_and_only_high_keeps_it_out_of_use() {
        assert_eq!(Severity::default(), Severity::Low);
        let out: Vec<Severity> = Severity::ALL
            .into_iter()
            .filter(|severity| severity.keeps_out_of_use())
            .collect();
        assert_eq!(out, vec![Severity::High]);
        assert!(Severity::Low < Severity::Medium && Severity::Medium < Severity::High);
    }

    #[test]
    fn status_and_severity_follow_the_latest_counted_disposition() {
        let squawk = squawk();
        assert_eq!(status(&[]), SquawkStatus::Open);
        assert_eq!(current_severity(&squawk, &[]), Severity::Low);

        // Deferred twice, the second time raised to High; then signed off.
        let history = vec![
            counted(10, 1, deferred("Waiting for the part"), None),
            counted(11, 20, deferred("Part back-ordered"), Some(Severity::High)),
        ];
        assert_eq!(status(&history), SquawkStatus::Deferred);
        assert_eq!(current_severity(&squawk, &history), Severity::High);
        assert!(status(&history).is_outstanding());

        let mut fixed = history;
        fixed.push(counted(12, 30, signed_off(), None));
        assert_eq!(status(&fixed), SquawkStatus::Resolved);
        assert!(!status(&fixed).is_outstanding());
        assert_eq!(
            current_severity(&squawk, &fixed),
            Severity::High,
            "unchanged"
        );

        // Order in the list doesn't matter; the events' times do.
        fixed.reverse();
        assert_eq!(status(&fixed), SquawkStatus::Resolved);
    }

    #[test]
    fn finishing_needs_every_outstanding_squawk_dispositioned_once() {
        let (brake, wiper, other) = (test_id(30), test_id(31), test_id(32));
        let outstanding = [brake, wiper];
        let disposition = |squawk, action| SquawkDisposition {
            squawk,
            action,
            severity: None,
        };
        let one = vec![disposition(brake, signed_off())];
        assert_eq!(
            check_dispositions(&outstanding, &one, false),
            Ok(()),
            "a draft"
        );
        assert_eq!(
            check_dispositions(&outstanding, &one, true),
            Err(SquawkProblem::Undispositioned(wiper))
        );
        let both = vec![
            disposition(brake, signed_off()),
            disposition(wiper, deferred("Next oil change")),
        ];
        assert_eq!(check_dispositions(&outstanding, &both, true), Ok(()));
        let twice = vec![
            disposition(brake, signed_off()),
            disposition(brake, deferred("Changed my mind")),
        ];
        assert_eq!(
            check_dispositions(&outstanding, &twice, false),
            Err(SquawkProblem::DispositionedTwice(brake))
        );
        let stranger = vec![disposition(other, signed_off())];
        assert_eq!(
            check_dispositions(&outstanding, &stranger, false),
            Err(SquawkProblem::NotOutstanding(other))
        );
        assert_eq!(
            check_dispositions(&[], &[], true),
            Ok(()),
            "nothing outstanding"
        );
    }

    #[test]
    fn opening_sets_every_field_and_survives_the_trip() {
        let patch = SquawkPatch::all(squawk());
        assert_eq!(patch.changed_fields().len(), 8);
        let json = serde_json::to_string(&patch).unwrap();
        assert_eq!(serde_json::from_str::<SquawkPatch>(&json).unwrap(), patch);
        let disposition = SquawkDisposition {
            squawk: test_id(30),
            action: deferred("Waiting for the part"),
            severity: Some(Severity::Medium),
        };
        let json = serde_json::to_string(&disposition).unwrap();
        assert!(json.contains("\"action\":\"deferred\""), "{json}");
        assert_eq!(
            serde_json::from_str::<SquawkDisposition>(&json).unwrap(),
            disposition
        );
    }
}
