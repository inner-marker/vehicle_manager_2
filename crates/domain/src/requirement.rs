//! Requirements, their interval rules, and service bundles (milestone 7; plan §3 Recurring
//! maintenance, Requirements & compliance; decision #031).
//!
//! A **requirement** is something done to one vehicle, e.g. "Engine oil & filter". It has
//! **interval rules**, and whichever comes due first wins (plan §3):
//! - **every N on a meter** (7,500 mi on the odometer, 250 h on engine hours);
//! - **every N months**, optionally **due by the end of that month** (as aviation inspections
//!   round);
//! - **a fixed next date** (a registration renewal, an inspection), which each sign-off replaces
//!   with the next one from the paperwork (or none).
//!
//! A **service bundle** groups requirements done together, e.g. "10,000-mile service", whose
//! tasks may each have their own intervals. Both belong to one vehicle; **copying a schedule**
//! from another vehicle copies them, never links them (#031).
//!
//! "What's due" is Phase 5. Milestone 7 keeps the rules and shows when each requirement was
//! **last done** (from the finished task that signed it off). The calendar arithmetic is here
//! already, tested, for Phase 5 to use.
//!
//! # Examples
//!
//! ```
//! use domain::requirement::calendar_due;
//!
//! use domain::requirement::CalendarUnit::{Days, Months, Years};
//!
//! // Every 12 months, from 15 March 2026: due 15 March 2027, or 31 March with end-of-month.
//! assert_eq!(calendar_due("2026-03-15", 12, Months, false).unwrap(), "2027-03-15");
//! assert_eq!(calendar_due("2026-03-15", 12, Months, true).unwrap(), "2027-03-31");
//! // A month-end date stays inside the shorter month.
//! assert_eq!(calendar_due("2026-01-31", 1, Months, false).unwrap(), "2026-02-28");
//! // Days and years too.
//! assert_eq!(calendar_due("2026-03-15", 90, Days, false).unwrap(), "2026-06-13");
//! assert_eq!(calendar_due("2028-02-29", 1, Years, false).unwrap(), "2029-02-28");
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Record, record_patch};
use crate::event::{EventId, Reading};
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::label::Label;
use crate::meter::{Meter, MeterId};
use crate::notes::Notes;
use crate::time::UnixMillis;

/// The ID of a requirement.
pub type RequirementId = Id<Requirement>;
/// The ID of a service bundle.
pub type BundleId = Id<Bundle>;

/// The most months a calendar rule may span (50 years).
pub const MAX_MONTHS: u16 = 600;

/// What a calendar rule counts in.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum CalendarUnit {
    /// Days (e.g. every 90 days).
    Days,
    /// Months (where calendar rules started, so rules stored before units read as months).
    #[default]
    Months,
    /// Years.
    Years,
}

impl CalendarUnit {
    /// Every unit, shortest first.
    pub const ALL: [Self; 3] = [Self::Days, Self::Months, Self::Years];

    /// The most a calendar rule may count in this unit: 50 years each way (in days, about 10
    /// years: longer ones are months or years).
    #[must_use]
    pub const fn max(self) -> u16 {
        match self {
            Self::Days => 3_650,
            Self::Months => MAX_MONTHS,
            Self::Years => 50,
        }
    }
}

/// One way a requirement comes due.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "rule", rename_all = "kebab-case")]
pub enum IntervalRule {
    /// Every `every` on `meter`, in the meter's stored unit (meters or seconds).
    Meter {
        /// Which meter.
        meter: MeterId,
        /// How far apart, more than zero.
        every: i64,
    },
    /// Every `every` days, months, or years; with `end_of_month` (months and years), due on
    /// the last day of that month.
    Calendar {
        /// How many (1 up to the unit's [`CalendarUnit::max`]). Stored before units existed as
        /// `months`.
        #[serde(alias = "months")]
        every: u16,
        /// Days, months, or years (months for a rule stored before units existed).
        #[serde(default)]
        unit: CalendarUnit,
        /// Rounded to the end of the calendar month (ignored for days).
        end_of_month: bool,
    },
    /// On a date from the paperwork. `first` is the first one; each sign-off then gives the
    /// next (or none).
    FixedDate {
        /// The first due date, `2026-11-30`, if known when the requirement is set up.
        first: Option<String>,
    },
}

/// Something done to one vehicle, and how often.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Requirement {
    /// Which vehicle.
    pub asset: AssetId,
    /// Its name, e.g. "Engine oil & filter".
    pub name: Label,
    /// Notes (e.g. the oil to use).
    pub notes: Notes,
    /// In use; one turned off stays in the history.
    pub enabled: bool,
    /// When it comes due: whichever rule first. None: done when needed.
    pub rules: Vec<IntervalRule>,
}

record_patch! {
    /// The changeable fields of a [`Requirement`]; `Some` means "set to this".
    RequirementPatch
    {
        /// See [`Requirement::asset`] (set when created).
        asset: AssetId,
        /// See [`Requirement::name`].
        name: Label,
        /// See [`Requirement::notes`].
        notes: Notes,
        /// See [`Requirement::enabled`].
        enabled: bool,
        /// See [`Requirement::rules`] (the whole list).
        rules: Vec<IntervalRule>,
    }
}

impl RequirementPatch {
    /// Every field of `requirement`, for creating it.
    #[must_use]
    pub fn all(requirement: Requirement) -> Self {
        Self {
            asset: Some(requirement.asset),
            name: Some(requirement.name),
            notes: Some(requirement.notes),
            enabled: Some(requirement.enabled),
            rules: Some(requirement.rules),
        }
    }
}

impl Record for Requirement {
    const ENTITY: &'static str = "requirement";
    type Patch = RequirementPatch;
}

/// A service done as one: the requirements it covers, e.g. "10,000-mile service".
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Bundle {
    /// Which vehicle.
    pub asset: AssetId,
    /// Its name.
    pub name: Label,
    /// The requirements it covers, in order (at least one).
    pub requirements: Vec<RequirementId>,
}

record_patch! {
    /// The changeable fields of a [`Bundle`]; `Some` means "set to this".
    BundlePatch
    {
        /// See [`Bundle::asset`] (set when created).
        asset: AssetId,
        /// See [`Bundle::name`].
        name: Label,
        /// See [`Bundle::requirements`] (the whole list).
        requirements: Vec<RequirementId>,
    }
}

impl BundlePatch {
    /// Every field of `bundle`, for creating it.
    #[must_use]
    pub fn all(bundle: Bundle) -> Self {
        Self {
            asset: Some(bundle.asset),
            name: Some(bundle.name),
            requirements: Some(bundle.requirements),
        }
    }
}

impl Record for Bundle {
    const ENTITY: &'static str = "bundle";
    type Patch = BundlePatch;
}

/// Why a requirement or bundle was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RequirementProblem {
    /// A meter rule for a meter that isn't this vehicle's.
    UnknownMeter,
    /// A meter rule of zero or less.
    BadInterval,
    /// A calendar rule outside its unit's range (1–3,650 days, 1–600 months, 1–50 years).
    BadCalendar,
    /// A fixed date that isn't a date.
    BadDate,
    /// Two rules of the same kind (two calendars, two fixed dates, or one meter twice).
    RepeatedRule,
    /// A bundle with no requirements.
    EmptyBundle,
    /// A bundle naming a requirement twice, or one that isn't this vehicle's.
    BadBundleItem,
    /// Removing a requirement a task has done (turn it off instead; it stays in the history).
    InUse,
    /// Copying a schedule onto a vehicle that already has requirements.
    ScheduleNotEmpty,
}

impl RequirementProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::UnknownMeter => messages::SOMETHING_WENT_WRONG,
            Self::BadInterval => messages::REQUIREMENT_BAD_INTERVAL,
            Self::BadCalendar => messages::REQUIREMENT_BAD_CALENDAR,
            Self::BadDate => messages::DATE_BAD_DATE,
            Self::RepeatedRule => messages::REQUIREMENT_REPEATED_RULE,
            Self::EmptyBundle => messages::BUNDLE_EMPTY,
            Self::BadBundleItem => messages::BUNDLE_BAD_ITEM,
            Self::InUse => messages::REQUIREMENT_IN_USE,
            Self::ScheduleNotEmpty => messages::SCHEDULE_NOT_EMPTY,
        }
    }
}

/// A date typed or stored as `2026-11-30`, checked.
fn civil(date: &str) -> Result<jiff::civil::Date, RequirementProblem> {
    date.trim().parse().map_err(|_| RequirementProblem::BadDate)
}

/// Checks a requirement's rules against its vehicle's `meters`.
///
/// # Errors
///
/// The first [`RequirementProblem`] found.
pub fn check_rules(
    rules: &[IntervalRule],
    meters: &[(MeterId, Meter)],
) -> Result<(), RequirementProblem> {
    for (index, rule) in rules.iter().enumerate() {
        let same_kind = rules[..index].iter().any(|earlier| match (earlier, rule) {
            (IntervalRule::Meter { meter: a, .. }, IntervalRule::Meter { meter: b, .. }) => a == b,
            (IntervalRule::Calendar { .. }, IntervalRule::Calendar { .. })
            | (IntervalRule::FixedDate { .. }, IntervalRule::FixedDate { .. }) => true,
            _ => false,
        });
        if same_kind {
            return Err(RequirementProblem::RepeatedRule);
        }
        match rule {
            IntervalRule::Meter { meter, every } => {
                if !meters.iter().any(|(id, _)| id == meter) {
                    return Err(RequirementProblem::UnknownMeter);
                }
                if *every <= 0 {
                    return Err(RequirementProblem::BadInterval);
                }
            }
            IntervalRule::Calendar { every, unit, .. } => {
                if !(1..=unit.max()).contains(every) {
                    return Err(RequirementProblem::BadCalendar);
                }
            }
            IntervalRule::FixedDate { first } => {
                if let Some(first) = first {
                    civil(first)?;
                }
            }
        }
    }
    Ok(())
}

/// Checks a bundle's requirements: at least one, each the vehicle's (`known`), none twice.
///
/// # Errors
///
/// [`RequirementProblem::EmptyBundle`] or [`RequirementProblem::BadBundleItem`].
pub fn check_bundle(
    requirements: &[RequirementId],
    known: &[RequirementId],
) -> Result<(), RequirementProblem> {
    if requirements.is_empty() {
        return Err(RequirementProblem::EmptyBundle);
    }
    for (index, requirement) in requirements.iter().enumerate() {
        if !known.contains(requirement) || requirements[..index].contains(requirement) {
            return Err(RequirementProblem::BadBundleItem);
        }
    }
    Ok(())
}

/// When a calendar rule of `every` `unit`s comes due after `done_on` (`2026-03-15`): the same
/// day that much later (the month's last day if it's shorter), or with `end_of_month` (months
/// and years only), the last day of that month.
///
/// # Errors
///
/// [`RequirementProblem::BadDate`] if `done_on` isn't a date or the result is out of range.
pub fn calendar_due(
    done_on: &str,
    every: u16,
    unit: CalendarUnit,
    end_of_month: bool,
) -> Result<String, RequirementProblem> {
    let done = civil(done_on)?;
    let count = i64::from(every);
    let span = match unit {
        CalendarUnit::Days => jiff::Span::new().days(count),
        CalendarUnit::Months => jiff::Span::new().months(count),
        CalendarUnit::Years => jiff::Span::new().years(count),
    };
    let due = done
        .checked_add(span)
        .map_err(|_| RequirementProblem::BadDate)?;
    let due = if end_of_month && unit != CalendarUnit::Days {
        due.last_of_month()
    } else {
        due
    };
    Ok(due.to_string())
}

/// A finished task that did a requirement: when, on which event, the readings it resets from
/// (the event's start or end set), and the next due date it gave for a fixed-date rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DoneTask {
    /// Which requirement.
    pub requirement: RequirementId,
    /// When (the event's start).
    pub at: UnixMillis,
    /// Which event.
    pub event: EventId,
    /// The readings it resets from.
    pub readings: Vec<Reading>,
    /// The next due date it gave (a fixed-date rule), `2027-11-30`; `None`: no next date.
    pub next_due: Option<String>,
}

/// When `requirement` was last done: the latest finished task for it (by the event's time).
#[must_use]
pub fn last_done(requirement: RequirementId, done: &[DoneTask]) -> Option<&DoneTask> {
    done.iter()
        .filter(|task| task.requirement == requirement)
        .max_by_key(|task| (task.at, task.event))
}

/// A schedule copied onto another vehicle: the requirements (with the IDs they had, so the
/// bundles' items can be mapped to their new ones), the bundles, and the meter rules left out
/// because the new vehicle has no matching meter.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CopiedSchedule {
    /// Each requirement, with its ID on the vehicle it came from.
    pub requirements: Vec<(RequirementId, Requirement)>,
    /// Each bundle, still naming the requirements by their old IDs.
    pub bundles: Vec<Bundle>,
    /// "Engine oil & filter: Engine hours": a requirement and a meter it couldn't keep.
    pub left_out: Vec<(Label, Label)>,
}

/// Copies `requirements` and `bundles` onto `to_asset` (#031): copies, never links. A meter
/// rule moves to the new vehicle's meter with the same name (ignoring case) and dimension;
/// with none, it's left out and listed.
#[must_use]
pub fn copy_schedule(
    requirements: &[(RequirementId, Requirement)],
    bundles: &[Bundle],
    from_meters: &[(MeterId, Meter)],
    to_meters: &[(MeterId, Meter)],
    to_asset: AssetId,
) -> CopiedSchedule {
    let matching = |meter: MeterId| -> Option<MeterId> {
        let (_, from) = from_meters.iter().find(|(id, _)| *id == meter)?;
        to_meters
            .iter()
            .find(|(_, to)| {
                to.dimension == from.dimension
                    && to.name.as_str().to_lowercase() == from.name.as_str().to_lowercase()
            })
            .map(|(id, _)| *id)
    };
    let meter_name = |meter: MeterId| {
        from_meters
            .iter()
            .find(|(id, _)| *id == meter)
            .map(|(_, meter)| meter.name.clone())
    };
    let mut left_out = Vec::new();
    let copied = requirements
        .iter()
        .map(|(id, requirement)| {
            let rules = requirement
                .rules
                .iter()
                .filter_map(|rule| match rule {
                    IntervalRule::Meter { meter, every } => {
                        let moved = matching(*meter).map(|to| IntervalRule::Meter {
                            meter: to,
                            every: *every,
                        });
                        if moved.is_none() {
                            left_out.extend(
                                meter_name(*meter).map(|name| (requirement.name.clone(), name)),
                            );
                        }
                        moved
                    }
                    other @ (IntervalRule::Calendar { .. } | IntervalRule::FixedDate { .. }) => {
                        Some(other.clone())
                    }
                })
                .collect();
            (
                *id,
                Requirement {
                    asset: to_asset,
                    rules,
                    ..requirement.clone()
                },
            )
        })
        .collect();
    CopiedSchedule {
        requirements: copied,
        bundles: bundles
            .iter()
            .map(|bundle| Bundle {
                asset: to_asset,
                ..bundle.clone()
            })
            .collect(),
        left_out,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::command::Patch;
    use crate::meter::{EventKind, MeterDimension, MeterUnit};
    use crate::test_support::test_id;

    const ODOMETER: u32 = 20;
    const HOURS: u32 = 21;

    fn meter(name: &str, dimension: MeterDimension) -> Meter {
        Meter {
            asset: test_id(1),
            name: Label::parse(name).unwrap(),
            dimension,
            unit: if dimension == MeterDimension::Distance {
                MeterUnit::Miles
            } else {
                MeterUnit::Hours
            },
            precision: 0,
            description: None,
            required_for: vec![EventKind::Fuel],
        }
    }

    fn meters() -> Vec<(MeterId, Meter)> {
        vec![
            (
                test_id(ODOMETER),
                meter("Odometer", MeterDimension::Distance),
            ),
            (
                test_id(HOURS),
                meter("Engine hours", MeterDimension::Duration),
            ),
        ]
    }

    fn oil() -> Requirement {
        Requirement {
            asset: test_id(1),
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![
                IntervalRule::Meter {
                    meter: test_id(ODOMETER),
                    every: 12_070_080,
                },
                IntervalRule::Meter {
                    meter: test_id(HOURS),
                    every: 900_000,
                },
                IntervalRule::Calendar {
                    every: 12,
                    unit: CalendarUnit::Months,
                    end_of_month: false,
                },
            ],
        }
    }

    #[test]
    fn rules_name_the_vehicles_meters_and_each_kind_once() {
        let meters = meters();
        assert_eq!(check_rules(&oil().rules, &meters), Ok(()));
        assert_eq!(check_rules(&[], &meters), Ok(()), "done when needed");
        let rule = |r: IntervalRule| check_rules(&[r], &meters);
        assert_eq!(
            rule(IntervalRule::Meter {
                meter: test_id(99),
                every: 1
            }),
            Err(RequirementProblem::UnknownMeter)
        );
        assert_eq!(
            rule(IntervalRule::Meter {
                meter: test_id(ODOMETER),
                every: 0
            }),
            Err(RequirementProblem::BadInterval)
        );
        for (every, unit) in [
            (0, CalendarUnit::Months),
            (601, CalendarUnit::Months),
            (3_651, CalendarUnit::Days),
            (51, CalendarUnit::Years),
        ] {
            assert_eq!(
                rule(IntervalRule::Calendar {
                    every,
                    unit,
                    end_of_month: true
                }),
                Err(RequirementProblem::BadCalendar)
            );
        }
        assert_eq!(
            rule(IntervalRule::Calendar {
                every: 90,
                unit: CalendarUnit::Days,
                end_of_month: false
            }),
            Ok(())
        );
        assert_eq!(
            rule(IntervalRule::FixedDate {
                first: Some("2026-02-30".into())
            }),
            Err(RequirementProblem::BadDate)
        );
        assert_eq!(rule(IntervalRule::FixedDate { first: None }), Ok(()));
        let twice = [
            IntervalRule::Calendar {
                every: 6,
                unit: CalendarUnit::Months,
                end_of_month: false,
            },
            IntervalRule::Calendar {
                every: 12,
                unit: CalendarUnit::Months,
                end_of_month: false,
            },
        ];
        assert_eq!(
            check_rules(&twice, &meters),
            Err(RequirementProblem::RepeatedRule)
        );
        let same_meter = [
            IntervalRule::Meter {
                meter: test_id(ODOMETER),
                every: 1,
            },
            IntervalRule::Meter {
                meter: test_id(ODOMETER),
                every: 2,
            },
        ];
        assert_eq!(
            check_rules(&same_meter, &meters),
            Err(RequirementProblem::RepeatedRule)
        );
    }

    #[test]
    fn a_bundle_names_the_vehicles_requirements_once_each() {
        let known = [test_id(40), test_id(41)];
        assert_eq!(check_bundle(&[test_id(40), test_id(41)], &known), Ok(()));
        assert_eq!(
            check_bundle(&[], &known),
            Err(RequirementProblem::EmptyBundle)
        );
        assert_eq!(
            check_bundle(&[test_id(40), test_id(40)], &known),
            Err(RequirementProblem::BadBundleItem)
        );
        assert_eq!(
            check_bundle(&[test_id(42)], &known),
            Err(RequirementProblem::BadBundleItem)
        );
    }

    #[test]
    fn calendar_rules_count_days_and_years_too() {
        assert_eq!(
            calendar_due("2026-03-15", 90, CalendarUnit::Days, false).unwrap(),
            "2026-06-13"
        );
        assert_eq!(
            calendar_due("2026-03-15", 90, CalendarUnit::Days, true).unwrap(),
            "2026-06-13",
            "the month's end is for months and years"
        );
        assert_eq!(
            calendar_due("2026-03-15", 2, CalendarUnit::Years, true).unwrap(),
            "2028-03-31"
        );
        assert_eq!(
            calendar_due("2028-02-29", 1, CalendarUnit::Years, false).unwrap(),
            "2029-02-28",
            "a leap day a year on"
        );
    }

    #[test]
    fn a_rule_stored_before_units_reads_as_months() {
        let stored = r#"{"rule":"calendar","months":6,"end_of_month":false}"#;
        assert_eq!(
            serde_json::from_str::<IntervalRule>(stored).unwrap(),
            IntervalRule::Calendar {
                every: 6,
                unit: CalendarUnit::Months,
                end_of_month: false
            }
        );
    }

    #[test]
    fn calendar_rules_add_months_and_round_to_the_month_end() {
        assert_eq!(
            calendar_due("2026-03-15", 12, CalendarUnit::Months, false).unwrap(),
            "2027-03-15"
        );
        assert_eq!(
            calendar_due("2026-03-15", 12, CalendarUnit::Months, true).unwrap(),
            "2027-03-31"
        );
        assert_eq!(
            calendar_due("2026-01-31", 1, CalendarUnit::Months, false).unwrap(),
            "2026-02-28"
        );
        assert_eq!(
            calendar_due("2028-01-31", 1, CalendarUnit::Months, true).unwrap(),
            "2028-02-29",
            "a leap year"
        );
        assert_eq!(
            calendar_due("2026-11-30", 3, CalendarUnit::Months, false).unwrap(),
            "2027-02-28"
        );
        assert_eq!(
            calendar_due("not a date", 1, CalendarUnit::Months, false),
            Err(RequirementProblem::BadDate)
        );
    }

    proptest! {
        #[test]
        fn a_calendar_due_date_is_months_later_and_end_of_month_is_the_last_day(
            day in 0i64..60_000,
            months in 1u16..=MAX_MONTHS,
            end_of_month: bool,
        ) {
            let done = jiff::civil::date(1950, 1, 1)
                .checked_add(jiff::Span::new().days(day))
                .unwrap();
            let due: jiff::civil::Date = calendar_due(&done.to_string(), months, CalendarUnit::Months, end_of_month)
                .unwrap()
                .parse()
                .unwrap();
            // Exactly `months` calendar months later.
            let month_count = |date: jiff::civil::Date| i64::from(date.year()) * 12 + i64::from(date.month());
            prop_assert_eq!(month_count(due) - month_count(done), i64::from(months));
            prop_assert!(due > done);
            if end_of_month {
                prop_assert_eq!(due, due.last_of_month());
            } else {
                // The same day, or the month's last day when it's shorter.
                prop_assert!(due.day() == done.day() || (due == due.last_of_month() && due.day() < done.day()));
            }
        }
    }

    #[test]
    fn last_done_is_the_latest_finished_task_for_it() {
        let task = |n: u32, requirement: u32, at: i64| DoneTask {
            requirement: test_id(requirement),
            at: UnixMillis::from_millis(at),
            event: test_id(n),
            readings: vec![],
            next_due: None,
        };
        let done = vec![
            task(10, 40, 100),
            task(11, 40, 300),
            task(12, 41, 500),
            task(13, 40, 200),
        ];
        assert_eq!(
            last_done(test_id(40), &done).map(|t| t.event),
            Some(test_id(11))
        );
        assert_eq!(
            last_done(test_id(41), &done).map(|t| t.event),
            Some(test_id(12))
        );
        assert_eq!(last_done(test_id(42), &done), None);
    }

    #[test]
    fn copying_a_schedule_moves_meter_rules_by_name_and_lists_what_it_left_out() {
        // The new vehicle has an odometer ("ODOMETER", another ID) but no engine hours.
        let to_meters = vec![(test_id(50), meter("ODOMETER", MeterDimension::Distance))];
        let requirements = vec![(test_id(40), oil())];
        let bundles = vec![Bundle {
            asset: test_id(1),
            name: Label::parse("10,000-mile service").unwrap(),
            requirements: vec![test_id(40)],
        }];
        let copied = copy_schedule(&requirements, &bundles, &meters(), &to_meters, test_id(2));
        let (old_id, copy) = &copied.requirements[0];
        assert_eq!(*old_id, test_id(40));
        assert_eq!(copy.asset, test_id(2));
        assert_eq!(
            copy.rules,
            vec![
                IntervalRule::Meter {
                    meter: test_id(50),
                    every: 12_070_080
                },
                IntervalRule::Calendar {
                    every: 12,
                    unit: CalendarUnit::Months,
                    end_of_month: false
                },
            ]
        );
        assert_eq!(
            copied.left_out,
            vec![(
                Label::parse("Engine oil & filter").unwrap(),
                Label::parse("Engine hours").unwrap()
            )]
        );
        assert_eq!(copied.bundles[0].asset, test_id(2));
        assert_eq!(
            copied.bundles[0].requirements,
            vec![test_id(40)],
            "mapped by the server"
        );
        assert_eq!(check_rules(&copy.rules, &to_meters), Ok(()));
    }

    #[test]
    fn patches_set_every_field_and_survive_the_trip() {
        let patch = RequirementPatch::all(oil());
        assert_eq!(patch.changed_fields().len(), 5);
        let json = serde_json::to_string(&patch).unwrap();
        assert!(json.contains("\"rule\":\"calendar\""), "{json}");
        assert_eq!(
            serde_json::from_str::<RequirementPatch>(&json).unwrap(),
            patch
        );
        let bundle = BundlePatch::all(Bundle {
            asset: test_id(1),
            name: Label::parse("Annual").unwrap(),
            requirements: vec![test_id(40)],
        });
        assert_eq!(bundle.changed_fields().len(), 3);
    }
}
