//! What the maintenance and repair work log typed, as the event's work (step 7i; choice 16,
//! option A of `docs/ui-sketches/p2m7-work-log-options.svg`). Kept apart from the form's
//! components so it can be tested without a browser.
//!
//! The work log is the log form's extra sections for a job: the tasks (each with its parts,
//! labor, what it resets from, and a fixed date's next due date), the readings at the end, the
//! vehicle's open squawks (each signed off or deferred, or not decided yet: a draft may leave
//! one undecided, but **Finish** won't), and a repair's complaint and cause.

use domain::event::{Part, Reading, ReadingSet, Reason, Task, WorkDetails};
use domain::i18n::{MessageId, messages};
use domain::label::Label;
use domain::meter::{EventKind, Meter, MeterId};
use domain::notes::Notes;
use domain::preferences::CurrencyCode;
use domain::quantity::{
    QuantityError, money_value, parse_duration, parse_money, parse_quantity, parse_reading,
    quantity_value, reading_value, show_duration,
};
use domain::requirement::RequirementId;
use domain::squawk::{Disposition, Severity, SquawkDisposition, SquawkId};

use super::log_input::{Problem, Target};

/// One part as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypedPart {
    pub description: String,
    pub part_number: String,
    pub quantity: String,
    pub unit_price: String,
}

/// One task as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypedTask {
    pub title: String,
    pub requirement: Option<RequirementId>,
    pub notes: String,
    pub parts: Vec<TypedPart>,
    /// Labor time as typed ("0:30"), and its cost.
    pub labor_time: String,
    pub labor_cost: String,
    /// It resets its requirement from the end readings (the start ones otherwise).
    pub from_end: bool,
    /// A fixed-date rule's next due date (`2027-11-30`), as typed.
    pub next_due: String,
}

/// What the job does about one squawk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(super) enum Action {
    /// Not decided yet (a draft may leave it; Finish needs it).
    #[default]
    Undecided,
    /// Fixed.
    SignOff,
    /// Put off, with a reason.
    Defer,
}

/// One squawk's disposition as typed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct TypedDisposition {
    pub squawk: SquawkId,
    pub action: Action,
    /// The task that fixed it, by its place in the list.
    pub task: Option<usize>,
    pub notes: String,
    pub reason: String,
    /// A new severity, if it changed.
    pub severity: Option<Severity>,
}

impl TypedDisposition {
    /// A squawk not decided yet.
    pub(super) const fn undecided(squawk: SquawkId) -> Self {
        Self {
            squawk,
            action: Action::Undecided,
            task: None,
            notes: String::new(),
            reason: String::new(),
            severity: None,
        }
    }
}

/// The work log as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct TypedWork {
    pub tasks: Vec<TypedTask>,
    /// Readings at the end, as typed (shown once asked for).
    pub end_readings: Vec<(MeterId, String)>,
    pub show_end: bool,
    pub dispositions: Vec<TypedDisposition>,
    pub complaint: String,
    pub cause: String,
}

impl TypedWork {
    /// The disposition typed for `squawk`, if any.
    pub(super) fn disposition(&self, squawk: SquawkId) -> Option<&TypedDisposition> {
        self.dispositions
            .iter()
            .find(|disposition| disposition.squawk == squawk)
    }

    /// The squawks of `outstanding` still to sign off or defer.
    pub(super) fn undecided(&self, outstanding: &[SquawkId]) -> Vec<SquawkId> {
        outstanding
            .iter()
            .copied()
            .filter(|squawk| {
                self.disposition(*squawk)
                    .is_none_or(|disposition| disposition.action == Action::Undecided)
            })
            .collect()
    }
}

/// A number problem as a work problem.
fn quantity(error: QuantityError) -> Problem {
    (Target::Work, error.message())
}

/// An optional typed amount.
fn money(text: &str, currency: CurrencyCode) -> Result<Option<i64>, Problem> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    parse_money(text, currency).map(Some).map_err(quantity)
}

/// One part, or the problem with it (an empty row is left out).
fn part(typed: &TypedPart, currency: CurrencyCode) -> Result<Option<Part>, Problem> {
    let blank = [
        &typed.description,
        &typed.part_number,
        &typed.quantity,
        &typed.unit_price,
    ]
    .iter()
    .all(|text| text.trim().is_empty());
    if blank {
        return Ok(None);
    }
    let quantity_milli = if typed.quantity.trim().is_empty() {
        1_000
    } else {
        parse_quantity(&typed.quantity).map_err(quantity)?
    };
    Ok(Some(Part {
        description: typed.description.trim().to_owned(),
        part_number: Some(typed.part_number.trim().to_owned()).filter(|text| !text.is_empty()),
        quantity_milli,
        unit_price_minor: money(&typed.unit_price, currency)?,
    }))
}

/// One task, or the problem with it.
fn task(typed: &TypedTask, currency: CurrencyCode) -> Result<Task, Problem> {
    let parts = typed
        .parts
        .iter()
        .map(|each| part(each, currency))
        .collect::<Result<Vec<_>, Problem>>()?
        .into_iter()
        .flatten()
        .collect();
    let labor_seconds = Some(typed.labor_time.trim())
        .filter(|text| !text.is_empty())
        .map(parse_duration)
        .transpose()
        .map_err(quantity)?;
    Ok(Task {
        title: Label::parse(&typed.title).map_err(|error| (Target::Work, error.message()))?,
        requirement: typed.requirement,
        notes: Notes::parse(&typed.notes).map_err(|error| (Target::Work, error.message()))?,
        parts,
        labor_seconds,
        labor_cost_minor: money(&typed.labor_cost, currency)?,
        resets_from: if typed.from_end {
            ReadingSet::End
        } else {
            ReadingSet::Start
        },
        next_due: Some(typed.next_due.trim().to_owned()).filter(|date| !date.is_empty()),
    })
}

/// One decided disposition (an undecided one is left out of a draft).
fn disposition(typed: &TypedDisposition) -> Result<Option<SquawkDisposition>, Problem> {
    let action = match typed.action {
        Action::Undecided => return Ok(None),
        Action::SignOff => Disposition::SignedOff {
            notes: Notes::parse(&typed.notes)
                .map_err(|error| (Target::Squawks, error.message()))?,
            task: typed.task.and_then(|task| u16::try_from(task).ok()),
        },
        Action::Defer => Disposition::Deferred {
            reason: Reason::parse(&typed.reason)
                .map_err(|error| (Target::Squawks, error.message()))?,
        },
    };
    Ok(Some(SquawkDisposition {
        squawk: typed.squawk,
        action,
        severity: typed.severity,
    }))
}

/// An optional repair note.
fn repair_note(text: &str) -> Result<Option<Notes>, Problem> {
    if text.trim().is_empty() {
        return Ok(None);
    }
    Notes::parse(text.trim())
        .map(Some)
        .map_err(|error| (Target::Work, error.message()))
}

/// The work of a job of `kind`, from what was typed.
///
/// # Errors
///
/// The first problem, and where it shows.
pub(super) fn work_from(
    typed: &TypedWork,
    kind: EventKind,
    meters: &[(MeterId, Meter)],
    currency: CurrencyCode,
) -> Result<WorkDetails, Problem> {
    let end_readings = if typed.show_end {
        typed
            .end_readings
            .iter()
            .filter(|(_, text)| !text.trim().is_empty())
            .map(|(id, text)| {
                let (_, meter) = meters
                    .iter()
                    .find(|(have, _)| have == id)
                    .ok_or((Target::Work, messages::SOMETHING_WENT_WRONG))?;
                Ok(Reading {
                    meter: *id,
                    value: parse_reading(text, meter.unit, meter.precision)
                        .map_err(|error| (Target::EndReading(*id), error.message()))?,
                })
            })
            .collect::<Result<Vec<_>, Problem>>()?
    } else {
        Vec::new()
    };
    let repair = kind == EventKind::Repair;
    Ok(WorkDetails {
        tasks: typed
            .tasks
            .iter()
            .map(|each| task(each, currency))
            .collect::<Result<Vec<_>, Problem>>()?,
        end_readings,
        dispositions: typed
            .dispositions
            .iter()
            .map(disposition)
            .collect::<Result<Vec<_>, Problem>>()?
            .into_iter()
            .flatten()
            .collect(),
        currency,
        complaint: if repair {
            repair_note(&typed.complaint)?
        } else {
            None
        },
        cause: if repair {
            repair_note(&typed.cause)?
        } else {
            None
        },
    })
}

/// A job's work as typed again (to edit or correct it; the reverse of [`work_from`]).
pub(super) fn typed_work(work: &WorkDetails, meters: &[(MeterId, Meter)]) -> TypedWork {
    let currency = work.currency;
    let money_text = |minor: Option<i64>| {
        minor
            .map(|minor| money_value(minor, currency))
            .unwrap_or_default()
    };
    TypedWork {
        tasks: work
            .tasks
            .iter()
            .map(|task| TypedTask {
                title: task.title.as_str().to_owned(),
                requirement: task.requirement,
                notes: task.notes.as_str().to_owned(),
                parts: task
                    .parts
                    .iter()
                    .map(|part| TypedPart {
                        description: part.description.clone(),
                        part_number: part.part_number.clone().unwrap_or_default(),
                        quantity: quantity_value(part.quantity_milli),
                        unit_price: money_text(part.unit_price_minor),
                    })
                    .collect(),
                labor_time: task.labor_seconds.map(show_duration).unwrap_or_default(),
                labor_cost: money_text(task.labor_cost_minor),
                from_end: task.resets_from == ReadingSet::End,
                next_due: task.next_due.clone().unwrap_or_default(),
            })
            .collect(),
        end_readings: work
            .end_readings
            .iter()
            .filter_map(|reading| {
                let (_, meter) = meters.iter().find(|(id, _)| *id == reading.meter)?;
                Some((
                    reading.meter,
                    reading_value(reading.value, meter.unit, meter.precision),
                ))
            })
            .collect(),
        show_end: !work.end_readings.is_empty(),
        dispositions: work
            .dispositions
            .iter()
            .map(|disposition| {
                let base = TypedDisposition::undecided(disposition.squawk);
                let typed = match &disposition.action {
                    Disposition::SignedOff { notes, task } => TypedDisposition {
                        action: Action::SignOff,
                        task: task.map(usize::from),
                        notes: notes.as_str().to_owned(),
                        ..base
                    },
                    Disposition::Deferred { reason } => TypedDisposition {
                        action: Action::Defer,
                        reason: reason.as_str().to_owned(),
                        ..base
                    },
                };
                TypedDisposition {
                    severity: disposition.severity,
                    ..typed
                }
            })
            .collect(),
        complaint: work
            .complaint
            .as_ref()
            .map(|notes| notes.as_str().to_owned())
            .unwrap_or_default(),
        cause: work
            .cause
            .as_ref()
            .map(|notes| notes.as_str().to_owned())
            .unwrap_or_default(),
    }
}

/// What the typed work costs so far: every part and labor cost that can be read, whatever
/// else is still missing (a task's title, say).
pub(super) fn total_minor(typed: &TypedWork, currency: CurrencyCode) -> i64 {
    typed
        .tasks
        .iter()
        .map(|each| {
            let parts: i64 = each
                .parts
                .iter()
                .filter_map(|typed| part(typed, currency).ok().flatten())
                .filter_map(|part| part.cost_minor())
                .sum();
            parts
                + money(&each.labor_cost, currency)
                    .ok()
                    .flatten()
                    .unwrap_or(0)
        })
        .sum()
}

/// One of Finish's checks, before it's sent (the server checks everything again).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum FinishCheck {
    /// The required readings are given (or skipped).
    Readings(bool),
    /// There's a place, or a reason why not.
    Place(bool),
    /// A squawk still to sign off or defer, by its description.
    Undecided(String),
}

impl FinishCheck {
    /// Whether it passes.
    pub(super) const fn passes(&self) -> bool {
        match self {
            Self::Readings(ok) | Self::Place(ok) => *ok,
            Self::Undecided(_) => false,
        }
    }

    /// Its line in the Finish dialog.
    pub(super) const fn message(&self) -> MessageId {
        match self {
            Self::Readings(true) => messages::WORK_CHECK_READINGS_OK,
            Self::Readings(false) => messages::WORK_CHECK_READINGS_MISSING,
            Self::Place(true) => messages::WORK_CHECK_PLACE_OK,
            Self::Place(false) => messages::WORK_CHECK_PLACE_MISSING,
            Self::Undecided(_) => messages::WORK_CHECK_UNDECIDED,
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::label::Label;
    use domain::meter::{MeterDimension, MeterUnit};
    use domain::test_support::test_id;

    use super::*;

    fn meters() -> Vec<(MeterId, Meter)> {
        vec![(
            test_id(20),
            Meter {
                asset: test_id(10),
                name: Label::parse("Odometer").unwrap(),
                dimension: MeterDimension::Distance,
                unit: MeterUnit::Miles,
                precision: 0,
                description: None,
                required_for: vec![EventKind::Maintenance],
            },
        )]
    }

    fn oil_change() -> TypedWork {
        TypedWork {
            tasks: vec![TypedTask {
                title: "Engine oil & filter".into(),
                requirement: Some(test_id(40)),
                notes: String::new(),
                parts: vec![
                    TypedPart {
                        description: "Oil filter".into(),
                        part_number: "FL-500S".into(),
                        quantity: "1".into(),
                        unit_price: "12.99".into(),
                    },
                    TypedPart {
                        description: "Engine oil".into(),
                        part_number: String::new(),
                        quantity: "5.5".into(),
                        unit_price: "8.99".into(),
                    },
                    TypedPart::default(),
                ],
                labor_time: "0:30".into(),
                labor_cost: "45".into(),
                from_end: true,
                next_due: String::new(),
            }],
            end_readings: vec![(test_id(20), "52700".into())],
            show_end: true,
            dispositions: vec![
                TypedDisposition {
                    action: Action::SignOff,
                    task: Some(0),
                    notes: "New bulbs".into(),
                    ..TypedDisposition::undecided(test_id(60))
                },
                TypedDisposition {
                    action: Action::Defer,
                    reason: "Parts on order".into(),
                    severity: Some(Severity::Medium),
                    ..TypedDisposition::undecided(test_id(61))
                },
                TypedDisposition::undecided(test_id(62)),
            ],
            complaint: "Squeal".into(),
            cause: String::new(),
        }
    }

    #[test]
    fn an_oil_change_becomes_its_work() {
        let work = work_from(
            &oil_change(),
            EventKind::Maintenance,
            &meters(),
            CurrencyCode::USD,
        )
        .unwrap();
        let task = &work.tasks[0];
        assert_eq!(task.parts.len(), 2, "the empty row is left out");
        assert_eq!(task.parts[1].quantity_milli, 5_500);
        assert_eq!(task.parts[0].part_number.as_deref(), Some("FL-500S"));
        assert_eq!(task.parts[1].part_number, None);
        assert_eq!(
            (task.labor_seconds, task.labor_cost_minor),
            (Some(1_800), Some(4_500))
        );
        assert_eq!(task.resets_from, ReadingSet::End);
        assert_eq!(work.total_cost_minor(), 1_299 + 4_945 + 4_500);
        assert_eq!(work.end_readings.len(), 1);
        assert_eq!(work.dispositions.len(), 2, "the undecided one is left out");
        assert_eq!(work.complaint, None, "only a repair has a complaint");
        assert_eq!(
            total_minor(&oil_change(), CurrencyCode::USD),
            work.total_cost_minor()
        );
        let mut untitled = oil_change();
        untitled.tasks[0].title.clear();
        assert_eq!(
            total_minor(&untitled, CurrencyCode::USD),
            work.total_cost_minor(),
            "a task without a title yet still counts"
        );
    }

    #[test]
    fn a_jobs_work_types_back_into_the_same_work() {
        let work = work_from(
            &oil_change(),
            EventKind::Repair,
            &meters(),
            CurrencyCode::USD,
        )
        .unwrap();
        let again = typed_work(&work, &meters());
        assert_eq!(
            work_from(&again, EventKind::Repair, &meters(), CurrencyCode::USD),
            Ok(work)
        );
    }

    #[test]
    fn problems_show_in_the_work_log() {
        let mut typed = oil_change();
        typed.tasks[0].labor_time = "half an hour".into();
        assert_eq!(
            work_from(&typed, EventKind::Maintenance, &meters(), CurrencyCode::USD),
            Err((Target::Work, messages::QUANTITY_NOT_A_DURATION))
        );
        let mut typed = oil_change();
        typed.dispositions[1].reason = " ".into();
        assert_eq!(
            work_from(&typed, EventKind::Maintenance, &meters(), CurrencyCode::USD)
                .map(|_| ())
                .unwrap_err()
                .0,
            Target::Squawks
        );
        let mut typed = oil_change();
        typed.end_readings[0].1 = "lots".into();
        assert_eq!(
            work_from(&typed, EventKind::Maintenance, &meters(), CurrencyCode::USD)
                .unwrap_err()
                .0,
            Target::EndReading(test_id(20))
        );
    }

    #[test]
    fn finish_needs_every_outstanding_squawk_decided() {
        let typed = oil_change();
        assert_eq!(
            typed.undecided(&[test_id(60), test_id(61), test_id(62), test_id(63)]),
            vec![test_id(62), test_id(63)]
        );
        assert!(!FinishCheck::Undecided("Rattle".into()).passes());
        assert!(FinishCheck::Place(true).passes());
    }
}
