//! The checks a maintenance or repair event's work passes (milestone 7; #032, #034).
//!
//! - Only maintenance and repairs have work details, and only repairs a complaint and cause.
//! - Each task's requirement is the vehicle's; its parts have a description and a quantity;
//!   no price, labor time, or labor cost is negative; a next due date only for a requirement
//!   with a fixed-date rule.
//! - A task resetting from the end readings needs some.
//! - Each end reading is at or above the start reading for its meter.
//! - Squawk dispositions are for the vehicle's outstanding squawks, once each, and name real
//!   tasks; when **finishing**, every outstanding squawk must be there.

use crate::event::{
    Event, EventProblem, PART_MAX_CHARS, PART_NUMBER_MAX_CHARS, Part, ReadingSet, Task, WorkDetails,
};
use crate::meter::EventKind;
use crate::requirement::IntervalRule;
use crate::squawk::{Disposition, check_dispositions};

use super::CheckContext;

/// Checks the event's work details (see the module), `finishing` or not.
pub(super) fn check_work(
    event: &Event,
    context: &CheckContext<'_>,
    finishing: bool,
) -> Result<(), EventProblem> {
    let work = match (event.kind, &event.work) {
        (EventKind::Maintenance | EventKind::Repair, Some(work)) => work,
        (EventKind::Maintenance | EventKind::Repair, None)
        | (EventKind::Fuel | EventKind::Reading, Some(_)) => {
            return Err(EventProblem::WorkDetails);
        }
        (EventKind::Fuel | EventKind::Reading, None) => return Ok(()),
    };
    if event.kind != EventKind::Repair && (work.complaint.is_some() || work.cause.is_some()) {
        return Err(EventProblem::RepairOnly);
    }
    for task in &work.tasks {
        check_task(task, work, context)?;
    }
    for end in &work.end_readings {
        if event
            .reading(end.meter)
            .is_some_and(|start| end.value < start)
        {
            return Err(EventProblem::EndBelowStart(end.meter));
        }
    }
    check_dispositions(context.outstanding_squawks, &work.dispositions, finishing)
        .map_err(EventProblem::Squawk)?;
    let task_count = work.tasks.len();
    let names_a_task = |disposition: &&crate::squawk::SquawkDisposition| match &disposition.action {
        Disposition::SignedOff {
            task: Some(task), ..
        } => usize::from(*task) >= task_count,
        Disposition::SignedOff { task: None, .. } | Disposition::Deferred { .. } => false,
    };
    if work.dispositions.iter().any(|d| names_a_task(&d)) {
        return Err(EventProblem::UnknownTask);
    }
    Ok(())
}

fn check_part(part: &Part) -> Result<(), EventProblem> {
    let description = part.description.trim().chars().count();
    let number = part
        .part_number
        .as_deref()
        .map_or(0, |number| number.chars().count());
    if description == 0
        || description > PART_MAX_CHARS
        || number > PART_NUMBER_MAX_CHARS
        || part.quantity_milli <= 0
    {
        return Err(EventProblem::BadPart);
    }
    if part.unit_price_minor.is_some_and(|price| price < 0) {
        return Err(EventProblem::BadCost);
    }
    Ok(())
}

fn check_task(
    task: &Task,
    work: &WorkDetails,
    context: &CheckContext<'_>,
) -> Result<(), EventProblem> {
    let requirement = match task.requirement {
        None => None,
        Some(id) => Some(
            context
                .requirements
                .iter()
                .find(|(have, _)| *have == id)
                .map(|(_, requirement)| requirement)
                .ok_or(EventProblem::UnknownRequirement)?,
        ),
    };
    for part in &task.parts {
        check_part(part)?;
    }
    if task.labor_seconds.is_some_and(|seconds| seconds < 0)
        || task.labor_cost_minor.is_some_and(|cost| cost < 0)
    {
        return Err(EventProblem::BadCost);
    }
    if let Some(next) = &task.next_due {
        let fixed = requirement.is_some_and(|requirement| {
            requirement
                .rules
                .iter()
                .any(|rule| matches!(rule, IntervalRule::FixedDate { .. }))
        });
        if !fixed || next.trim().parse::<jiff::civil::Date>().is_err() {
            return Err(EventProblem::BadNextDue);
        }
    }
    if task.resets_from == ReadingSet::End && work.end_readings.is_empty() {
        return Err(EventProblem::NoEndReadings);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::command::Patch;
    use crate::dates::EventDates;
    use crate::event::{
        Event, EventLocation, EventProblem, EventSource, EventStatus, Finalizer, Part, Reading,
        ReadingSet, Reason, Task, WorkDetails,
    };
    use crate::event_check::{CheckContext, LocationRule, check, check_finish, finalize};
    use crate::label::Label;
    use crate::location::{LocationSource, Place, TimeZone};
    use crate::meter::{EventKind, Meter, MeterDimension, MeterId, MeterUnit};
    use crate::notes::Notes;
    use crate::preferences::CurrencyCode;
    use crate::requirement::{IntervalRule, Requirement, RequirementId};
    use crate::squawk::{Disposition, SquawkDisposition, SquawkProblem};
    use crate::test_support::test_id;
    use crate::time::UnixMillis;

    const DAY: i64 = 86_400_000;
    const ODOMETER: u32 = 20;
    const OIL: u32 = 40;
    const REGISTRATION: u32 = 41;
    const BRAKES: u32 = 30;

    fn meters() -> Vec<(MeterId, Meter)> {
        vec![(
            test_id(ODOMETER),
            Meter {
                asset: test_id(1),
                name: Label::parse("Odometer").unwrap(),
                dimension: MeterDimension::Distance,
                unit: MeterUnit::Miles,
                precision: 0,
                description: None,
                required_for: vec![EventKind::Fuel, EventKind::Maintenance],
            },
        )]
    }

    fn requirements() -> Vec<(RequirementId, Requirement)> {
        let requirement = |name: &str, rules| Requirement {
            asset: test_id(1),
            name: Label::parse(name).unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules,
        };
        vec![
            (
                test_id(OIL),
                requirement(
                    "Engine oil & filter",
                    vec![IntervalRule::Calendar {
                        every: 12,
                        unit: crate::requirement::CalendarUnit::Months,
                        end_of_month: false,
                    }],
                ),
            ),
            (
                test_id(REGISTRATION),
                requirement(
                    "Registration",
                    vec![IntervalRule::FixedDate { first: None }],
                ),
            ),
        ]
    }

    fn part(description: &str, quantity_milli: i64, unit_price_minor: Option<i64>) -> Part {
        Part {
            description: description.into(),
            part_number: None,
            quantity_milli,
            unit_price_minor,
        }
    }

    fn task(title: &str, requirement: Option<u32>) -> Task {
        Task {
            title: Label::parse(title).unwrap(),
            requirement: requirement.map(test_id),
            notes: Notes::default(),
            parts: vec![],
            labor_seconds: None,
            labor_cost_minor: None,
            resets_from: ReadingSet::Start,
            next_due: None,
        }
    }

    /// An oil change and tire rotation at 1,000 m, ending at 1,010 m after a test drive.
    fn oil_change() -> Event {
        let mut oil = task("Engine oil & filter", Some(OIL));
        oil.parts = vec![
            part("Synthetic oil, quart", 5_500, Some(899)),
            part("Oil filter", 1_000, Some(1_249)),
        ];
        oil.labor_seconds = Some(1_800);
        oil.labor_cost_minor = Some(4_000);
        Event {
            asset: test_id(1),
            kind: EventKind::Maintenance,
            dates: EventDates::new(
                UnixMillis::from_millis(10 * DAY),
                None,
                TimeZone::parse("UTC").unwrap(),
            )
            .unwrap(),
            location: EventLocation {
                place: Some(Place::parse("Asheville", "North Carolina", "US").unwrap()),
                coordinates: None,
                accuracy_m: None,
                fix_time: None,
                source: LocationSource::Manual,
                place_name: None,
                saved_place: None,
                geocoder_version: None,
                missing_reason: None,
            },
            readings: vec![Reading {
                meter: test_id(ODOMETER),
                value: 1_000,
            }],
            skipped: vec![],
            fields: vec![],
            notes: Notes::default(),
            source: EventSource::Typed,
            logged_by: test_id(5),
            performed_by: None,
            fuel: None,
            work: Some(WorkDetails {
                tasks: vec![oil, task("Tire rotation", None)],
                end_readings: vec![Reading {
                    meter: test_id(ODOMETER),
                    value: 1_010,
                }],
                dispositions: vec![],
                currency: CurrencyCode::USD,
                complaint: None,
                cause: None,
            }),
            reading_override: None,
            status: EventStatus::Draft,
            last_edited_at: UnixMillis::from_millis(10 * DAY),
            received_at: UnixMillis::from_millis(10 * DAY),
            finalized: None,
            corrects: None,
            correction_reason: None,
        }
    }

    /// Checks `event` (or finishes it) on a vehicle with `outstanding` squawks.
    fn checked(event: &Event, outstanding: &[u32], finishing: bool) -> Result<(), EventProblem> {
        let (meters, requirements) = (meters(), requirements());
        let outstanding: Vec<_> = outstanding.iter().map(|n| test_id(*n)).collect();
        let context = CheckContext {
            kinds: &EventKind::ALL,
            meters: &meters,
            fields: &[],
            history: &[],
            location_rule: LocationRule::Required,
            may_override_readings: false,
            requirements: &requirements,
            outstanding_squawks: &outstanding,
        };
        let result = if finishing {
            check_finish(event, &context)
        } else {
            check(event, &context)
        };
        result.map(|_| ())
    }

    fn work(event: &mut Event) -> &mut WorkDetails {
        event.work.as_mut().unwrap()
    }

    #[test]
    fn an_oil_change_with_two_tasks_passes_and_adds_up() {
        let event = oil_change();
        assert_eq!(checked(&event, &[], false), Ok(()));
        assert_eq!(
            checked(&event, &[], true),
            Ok(()),
            "nothing outstanding to disposition"
        );
        let details = event.work.as_ref().unwrap();
        // 5.5 × 8.99 = 49.445 → 49.45, plus 12.49, plus 40.00 labor.
        assert_eq!(details.tasks[0].parts_cost_minor(), 4_945 + 1_249);
        assert_eq!(details.total_cost_minor(), 4_945 + 1_249 + 4_000);
        assert_eq!(details.end_reading(test_id(ODOMETER)), Some(1_010));
    }

    #[test]
    fn finishing_needs_every_outstanding_squawk_signed_off_or_deferred() {
        let mut event = oil_change();
        assert_eq!(
            checked(&event, &[BRAKES], false),
            Ok(()),
            "a draft can wait"
        );
        assert_eq!(
            checked(&event, &[BRAKES], true),
            Err(EventProblem::Squawk(SquawkProblem::Undispositioned(
                test_id(BRAKES)
            )))
        );
        work(&mut event).dispositions = vec![SquawkDisposition {
            squawk: test_id(BRAKES),
            action: Disposition::Deferred {
                reason: Reason::parse("Pads ordered").unwrap(),
            },
            severity: None,
        }];
        assert_eq!(checked(&event, &[BRAKES], true), Ok(()));
        work(&mut event).dispositions[0].action = Disposition::SignedOff {
            notes: Notes::default(),
            task: Some(5),
        };
        assert_eq!(
            checked(&event, &[BRAKES], false),
            Err(EventProblem::UnknownTask)
        );
        work(&mut event).dispositions[0].action = Disposition::SignedOff {
            notes: Notes::default(),
            task: Some(1),
        };
        assert_eq!(checked(&event, &[BRAKES], true), Ok(()));
    }

    #[test]
    fn tasks_parts_costs_and_readings_must_make_sense() {
        let refused = |change: &dyn Fn(&mut Event)| {
            let mut event = oil_change();
            change(&mut event);
            checked(&event, &[], false)
        };
        assert_eq!(
            refused(&|e| work(e).tasks[1].requirement = Some(test_id(99))),
            Err(EventProblem::UnknownRequirement)
        );
        assert_eq!(
            refused(&|e| work(e).tasks[0].parts[0].quantity_milli = 0),
            Err(EventProblem::BadPart)
        );
        assert_eq!(
            refused(&|e| work(e).tasks[0].parts[0].description = " ".into()),
            Err(EventProblem::BadPart)
        );
        assert_eq!(
            refused(&|e| work(e).tasks[0].parts[0].unit_price_minor = Some(-1)),
            Err(EventProblem::BadCost)
        );
        assert_eq!(
            refused(&|e| work(e).tasks[0].labor_seconds = Some(-60)),
            Err(EventProblem::BadCost)
        );
        assert_eq!(
            refused(&|e| work(e).tasks[0].next_due = Some("2027-11-30".into())),
            Err(EventProblem::BadNextDue),
            "oil has no fixed-date rule"
        );
        assert_eq!(
            refused(&|e| {
                let mut registration = task("Registration", Some(REGISTRATION));
                registration.next_due = Some("2027-11-30".into());
                work(e).tasks.push(registration);
            }),
            Ok(())
        );
        assert_eq!(
            refused(&|e| {
                work(e).tasks[0].resets_from = ReadingSet::End;
                work(e).end_readings.clear();
            }),
            Err(EventProblem::NoEndReadings)
        );
        assert_eq!(
            refused(&|e| work(e).end_readings[0].value = 990),
            Err(EventProblem::EndBelowStart(test_id(ODOMETER)))
        );
        assert_eq!(
            refused(&|e| work(e).complaint = Some(Notes::parse("Squeal").unwrap())),
            Err(EventProblem::RepairOnly)
        );
        assert_eq!(
            refused(&|e| {
                e.kind = EventKind::Repair;
                work(e).complaint = Some(Notes::parse("Squeal when braking").unwrap());
                work(e).cause = Some(Notes::parse("Worn front pads").unwrap());
            }),
            Ok(())
        );
        assert_eq!(refused(&|e| e.work = None), Err(EventProblem::WorkDetails));
        assert_eq!(
            refused(&|e| e.readings.clear()),
            Err(EventProblem::Required(crate::event::Requirement::Meter(
                test_id(ODOMETER)
            ))),
            "the start odometer is required for maintenance"
        );
    }

    #[test]
    fn maintenance_never_finalizes_by_itself_and_is_forgotten_after_14_days() {
        let mut event = oil_change();
        let later = UnixMillis::from_millis(10 * DAY + 30 * DAY);
        assert!(
            !event.finalize_due(later, DAY),
            "only fuel-ups and readings"
        );
        assert!(!event.forgotten(UnixMillis::from_millis(10 * DAY + 13 * DAY), 14 * DAY));
        assert!(event.forgotten(UnixMillis::from_millis(10 * DAY + 14 * DAY), 14 * DAY));
        event.kind = EventKind::Fuel;
        assert!(
            !event.forgotten(later, 14 * DAY),
            "a fuel-up is never forgotten"
        );
        event.work = None;
        event.fuel = None;
        assert_eq!(
            checked(&event, &[], true).err(),
            Some(EventProblem::NotAJob),
            "fuel-ups are finalized, not finished"
        );
    }

    #[test]
    fn a_finished_task_counts_as_last_done_with_the_readings_it_resets_from() {
        let mut event = oil_change();
        assert!(
            event.done_tasks(test_id(70)).is_empty(),
            "not until it's finished"
        );
        work(&mut event).tasks[0].resets_from = ReadingSet::End;
        let patch = finalize(
            Finalizer::User(test_id(5)),
            UnixMillis::from_millis(11 * DAY),
        );
        assert_eq!(patch.changed_fields(), vec!["status", "finalized"]);
        event.apply(patch);
        let done = event.done_tasks(test_id(70));
        assert_eq!(done.len(), 1, "the tire rotation has no requirement");
        assert_eq!(done[0].requirement, test_id(OIL));
        assert_eq!(done[0].readings[0].value, 1_010, "reset from the end");
        assert_eq!(done[0].at, UnixMillis::from_millis(10 * DAY));
    }
}
