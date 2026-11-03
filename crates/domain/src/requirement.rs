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
//! // Every 12 months, from 15 March 2026: due 15 March 2027, or 31 March with end-of-month.
//! assert_eq!(calendar_due("2026-03-15", 12, false).unwrap(), "2027-03-15");
//! assert_eq!(calendar_due("2026-03-15", 12, true).unwrap(), "2027-03-31");
//! // A month-end date stays inside the shorter month.
//! assert_eq!(calendar_due("2026-01-31", 1, false).unwrap(), "2026-02-28");
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
    /// Every `months` months; with `end_of_month`, due on the last day of that month.
    Calendar {
        /// How many months, 1–600.
        months: u16,
        /// Rounded to the end of the calendar month.
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
    /// A calendar rule outside 1–600 months.
    BadMonths,
    /// A fixed date that isn't a date.
    BadDate,
    /// Two rules of the same kind (two calendars, two fixed dates, or one meter twice).
    RepeatedRule,
    /// A bundle with no requirements.
    EmptyBundle,
    /// A bundle naming a requirement twice, or one that isn't this vehicle's.
    BadBundleItem,
}

impl RequirementProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::UnknownMeter => messages::SOMETHING_WENT_WRONG,
            Self::BadInterval => messages::REQUIREMENT_BAD_INTERVAL,
            Self::BadMonths => messages::REQUIREMENT_BAD_MONTHS,
            Self::BadDate => messages::DATE_BAD_DATE,
            Self::RepeatedRule => messages::REQUIREMENT_REPEATED_RULE,
            Self::EmptyBundle => messages::BUNDLE_EMPTY,
            Self::BadBundleItem => messages::BUNDLE_BAD_ITEM,
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
            IntervalRule::Calendar { months, .. } => {
                if !(1..=MAX_MONTHS).contains(months) {
                    return Err(RequirementProblem::BadMonths);
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

/// When a calendar rule of `months` comes due after `done_on` (`2026-03-15`): the same day
/// `months` later (the month's last day if it's shorter), or with `end_of_month`, the last
/// day of that month.
///
/// # Errors
///
/// [`RequirementProblem::BadDate`] if `done_on` isn't a date or the result is out of range.
pub fn calendar_due(
    done_on: &str,
    months: u16,
    end_of_month: bool,
) -> Result<String, RequirementProblem> {
    let done = civil(done_on)?;
    let due = done
        .checked_add(jiff::Span::new().months(i64::from(months)))
        .map_err(|_| RequirementProblem::BadDate)?;
    let due = if end_of_month {
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
                    months: 12,
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
        for months in [0, 601] {
            assert_eq!(
                rule(IntervalRule::Calendar {
                    months,
                    end_of_month: true
                }),
                Err(RequirementProblem::BadMonths)
            );
        }
        assert_eq!(
            rule(IntervalRule::FixedDate {
                first: Some("2026-02-30".into())
            }),
            Err(RequirementProblem::BadDate)
        );
        assert_eq!(rule(IntervalRule::FixedDate { first: None }), Ok(()));
        let twice = [
            IntervalRule::Calendar {
                months: 6,
                end_of_month: false,
            },
            IntervalRule::Calendar {
                months: 12,
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
    fn calendar_rules_add_months_and_round_to_the_month_end() {
        assert_eq!(calendar_due("2026-03-15", 12, false).unwrap(), "2027-03-15");
        assert_eq!(calendar_due("2026-03-15", 12, true).unwrap(), "2027-03-31");
        assert_eq!(calendar_due("2026-01-31", 1, false).unwrap(), "2026-02-28");
        assert_eq!(
            calendar_due("2028-01-31", 1, true).unwrap(),
            "2028-02-29",
            "a leap year"
        );
        assert_eq!(calendar_due("2026-11-30", 3, false).unwrap(), "2027-02-28");
        assert_eq!(
            calendar_due("not a date", 1, false),
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
            let due: jiff::civil::Date = calendar_due(&done.to_string(), months, end_of_month)
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
                    months: 12,
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
