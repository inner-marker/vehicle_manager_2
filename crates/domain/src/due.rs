//! What's due (Phase 5 milestone 2; decisions #080, #082; plan §3 Recurring maintenance,
//! Official asset documents).
//!
//! Each requirement on a vehicle's schedule, and each document with an expiry, has a
//! [`DueStep`]: **OK**, **Coming up** (yellow), **Due soon** (orange), **Due very soon**
//! (red), or **Overdue** (a darker red). The screens always show the step's word with its
//! colour, so colour is never the only sign.
//!
//! - **Interval rules** (every N on a meter, every N days, months, or years, and "when a meter
//!   reaches X") reach a step when what's left is at or below its share of the interval:
//!   20 / 10 / 5% by default ([`IntervalSteps`]).
//! - **Dates without an interval** (a fixed next date, a document's expiry) reach a step that
//!   many days ahead: 60 / 30 / 14 by default ([`DaySteps`]).
//! - Both are the household's defaults ([`DueDefaults`]); a requirement or a document can have
//!   its own.
//! - **Whichever comes first:** a requirement's step is its most urgent rule's, and its date the
//!   earliest, fixed or projected.
//! - **Counted from** when it was last done (the latest finished task), or else from when it
//!   was **last done before these records** ([`LastDoneBefore`]), or else from the vehicle's
//!   first reading of the meter (or the day the requirement was added): then it's **not done
//!   yet**.
//! - **Projected dates** come from the meter's use over its last 90 days of readings (at least
//!   two, 7 or more days apart), then over all of them, then none ([`usage`]).
//!
//! Pure functions: the server (and the desktop, from its copy) gathers the readings, the
//! finished tasks, and the defaults, and asks.
//!
//! # Examples
//!
//! ```
//! use domain::due::{DaySteps, DueStep, IntervalSteps};
//!
//! let steps = IntervalSteps::default();
//! // 700 mi left of a 7,500 mi interval: 9%, so due soon.
//! assert_eq!(steps.step(700, 7_500), DueStep::DueSoon);
//! assert_eq!(steps.step(-1, 7_500), DueStep::Overdue);
//! assert_eq!(DaySteps::default().step(45), DueStep::ComingUp);
//! ```

use serde::{Deserialize, Serialize};

use crate::dates::utc_to_local;
use crate::document::{AssetDocument, AssetDocumentId, DocumentStatus};
use crate::event_check::PastReading;
use crate::i18n::{MessageId, messages};
use crate::location::TimeZone;
use crate::meter::{Meter, MeterDirection, MeterId};
use crate::requirement::{
    DoneTask, IntervalRule, Requirement, RequirementId, calendar_due, last_done,
};
use crate::time::UnixMillis;

/// One day, in milliseconds.
const DAY_MS: i64 = 24 * 60 * 60 * 1_000;

/// How far back the usage projection looks first (#082).
pub const PROJECTION_DAYS: i64 = 90;

/// The shortest span of readings a projection trusts.
pub const PROJECTION_MIN_DAYS: i64 = 7;

/// Where something stands, least urgent first (so the most urgent is the greatest).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum DueStep {
    /// Not near yet.
    Ok,
    /// Yellow.
    ComingUp,
    /// Orange.
    DueSoon,
    /// Red.
    DueVerySoon,
    /// Past it: a darker red, always with its word.
    Overdue,
}

impl DueStep {
    /// The step's word.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Ok => messages::DUE_OK,
            Self::ComingUp => messages::DUE_COMING_UP,
            Self::DueSoon => messages::DUE_SOON,
            Self::DueVerySoon => messages::DUE_VERY_SOON,
            Self::Overdue => messages::DUE_OVERDUE,
        }
    }
}

/// When an interval rule warns: at or below these percentages of its interval left.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct IntervalSteps {
    /// Yellow (20 by default).
    pub coming_up: u8,
    /// Orange (10).
    pub due_soon: u8,
    /// Red (5).
    pub very_soon: u8,
}

impl Default for IntervalSteps {
    fn default() -> Self {
        Self {
            coming_up: 20,
            due_soon: 10,
            very_soon: 5,
        }
    }
}

impl IntervalSteps {
    /// Whether they make sense: each at most 100, and none earlier than the one before it.
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.coming_up <= 100 && self.due_soon <= self.coming_up && self.very_soon <= self.due_soon
    }

    /// The step with `left` of an `interval` to go (both in the same unit; `left` below zero
    /// is overdue).
    #[must_use]
    pub fn step(&self, left: i64, interval: i64) -> DueStep {
        if left < 0 {
            return DueStep::Overdue;
        }
        let interval = i128::from(interval.max(1));
        let within = |percent: u8| i128::from(left) * 100 <= i128::from(percent) * interval;
        if within(self.very_soon) {
            DueStep::DueVerySoon
        } else if within(self.due_soon) {
            DueStep::DueSoon
        } else if within(self.coming_up) {
            DueStep::ComingUp
        } else {
            DueStep::Ok
        }
    }
}

/// When a date without an interval warns: this many days ahead, or fewer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DaySteps {
    /// Yellow (60 by default).
    pub coming_up: u16,
    /// Orange (30).
    pub due_soon: u16,
    /// Red (14).
    pub very_soon: u16,
}

impl Default for DaySteps {
    fn default() -> Self {
        Self {
            coming_up: 60,
            due_soon: 30,
            very_soon: 14,
        }
    }
}

impl DaySteps {
    /// The most days ahead a step may warn (about two years).
    pub const MAX: u16 = 730;

    /// Whether they make sense: each at most [`Self::MAX`], and none earlier than the one
    /// before it.
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.coming_up <= Self::MAX
            && self.due_soon <= self.coming_up
            && self.very_soon <= self.due_soon
    }

    /// The step `days_left` days ahead (below zero: past it, overdue; zero: due today).
    #[must_use]
    pub fn step(&self, days_left: i64) -> DueStep {
        if days_left < 0 {
            DueStep::Overdue
        } else if days_left <= i64::from(self.very_soon) {
            DueStep::DueVerySoon
        } else if days_left <= i64::from(self.due_soon) {
            DueStep::DueSoon
        } else if days_left <= i64::from(self.coming_up) {
            DueStep::ComingUp
        } else {
            DueStep::Ok
        }
    }
}

/// When things warn: the household's defaults, or one requirement's own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct DueDefaults {
    /// For interval rules.
    pub interval: IntervalSteps,
    /// For dates without an interval.
    pub days: DaySteps,
}

impl DueDefaults {
    /// Whether both make sense.
    #[must_use]
    pub const fn valid(&self) -> bool {
        self.interval.valid() && self.days.valid()
    }
}

/// When a requirement was last done before the household's records began (#082): a date
/// (`2026-08-02`) and the readings then.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct LastDoneBefore {
    /// The date.
    pub on: String,
    /// The readings then (some or all of its meters).
    pub readings: Vec<crate::event::Reading>,
}

/// What's left on one rule.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Left {
    /// On a meter: how much is left in its stored unit, and when that's projected to run out.
    Meter {
        /// Which meter.
        meter: MeterId,
        /// What's left (below zero: overdue by that much).
        left: i64,
        /// The projected date (`2027-01-03`), an estimate; none without enough readings.
        projected: Option<String>,
    },
    /// On a date.
    Date {
        /// The due date (`2026-12-10`).
        due: String,
    },
}

/// One rule's step and what's left on it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuleDue {
    /// Its step.
    pub step: DueStep,
    /// What's left.
    pub left: Left,
}

/// When something comes due: a date, and whether it's an estimate (projected from use).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DueDate {
    /// The date (`2027-01-03`).
    pub on: String,
    /// Projected from use, not fixed.
    pub estimate: bool,
}

/// Where a requirement or a document stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Due {
    /// The most urgent rule's step.
    pub step: DueStep,
    /// Each rule that could be worked out (a document has one date).
    pub rules: Vec<RuleDue>,
    /// The earliest date, fixed or projected.
    pub date: Option<DueDate>,
    /// Never done, and no "last done before these records".
    pub not_done_yet: bool,
}

/// What [`requirement_due`] needs beyond the requirement: the vehicle's meters, their readings
/// that count (in time order), the finished tasks that did requirements, now and the zone the
/// dates are in, and the household's defaults.
#[derive(Debug, Clone, Copy)]
pub struct DueContext<'a> {
    /// Now.
    pub now: UnixMillis,
    /// The household's time zone, for dates.
    pub zone: &'a TimeZone,
    /// The vehicle's meters.
    pub meters: &'a [(MeterId, Meter)],
    /// Its readings that count, in time order.
    pub readings: &'a [PastReading],
    /// Its finished tasks that did requirements.
    pub done: &'a [DoneTask],
    /// The household's defaults.
    pub defaults: &'a DueDefaults,
}

impl DueContext<'_> {
    /// Today's date in the zone.
    fn today(&self) -> Option<jiff::civil::Date> {
        date_of(self.now, self.zone)
    }

    /// A meter's latest reading that counts (a reset counts after others at its moment).
    fn latest(&self, meter: MeterId) -> Option<&PastReading> {
        self.readings
            .iter()
            .filter(|reading| reading.meter == meter)
            .max_by_key(|reading| (reading.at, reading.reset))
    }

    /// A meter's first reading.
    fn first(&self, meter: MeterId) -> Option<&PastReading> {
        self.readings
            .iter()
            .filter(|reading| reading.meter == meter)
            .min_by_key(|reading| (reading.at, !reading.reset))
    }
}

/// The local date of `at` in `zone`.
fn date_of(at: UnixMillis, zone: &TimeZone) -> Option<jiff::civil::Date> {
    let (date, _) = utc_to_local(at, zone).ok()?;
    date.parse().ok()
}

/// Days from `from` to `to` (negative if `to` is earlier).
fn days_between(from: jiff::civil::Date, to: jiff::civil::Date) -> Option<i64> {
    let span = from.until(to).ok()?;
    Some(i64::from(span.get_days()))
}

/// A meter's use over a span: how far it went (in its direction) and over how many
/// milliseconds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Usage {
    /// How far, in the meter's stored unit, more than zero.
    pub amount: i64,
    /// Over how long, in milliseconds, more than zero.
    pub millis: i64,
}

/// `meter`'s use, counting `direction`, over its readings in the last 90 days, else all of
/// them: the sum over each pair of readings in a row (not across a reset), from at least two
/// readings 7 or more days apart. `None` if neither has enough, or it didn't move.
#[must_use]
pub fn usage(
    readings: &[PastReading],
    meter: MeterId,
    direction: MeterDirection,
    now: UnixMillis,
) -> Option<Usage> {
    let since = now.as_millis() - PROJECTION_DAYS * DAY_MS;
    usage_since(readings, meter, direction, Some(since))
        .or_else(|| usage_since(readings, meter, direction, None))
}

fn usage_since(
    readings: &[PastReading],
    meter: MeterId,
    direction: MeterDirection,
    since: Option<i64>,
) -> Option<Usage> {
    let mut of_meter: Vec<&PastReading> = readings
        .iter()
        .filter(|reading| reading.meter == meter)
        .filter(|reading| since.is_none_or(|since| reading.at.as_millis() >= since))
        .collect();
    of_meter.sort_by_key(|reading| (reading.at, reading.reset));
    let (first, last) = (of_meter.first()?, of_meter.last()?);
    if last.at.as_millis() - first.at.as_millis() < PROJECTION_MIN_DAYS * DAY_MS {
        return None;
    }
    let (mut amount, mut millis) = (0_i64, 0_i64);
    for pair in of_meter.windows(2) {
        let [before, after] = pair else { continue };
        if after.reset {
            continue;
        }
        let moved = match direction {
            MeterDirection::Up => after.value - before.value,
            MeterDirection::Down => before.value - after.value,
        };
        amount += moved.max(0);
        millis += after.at.as_millis() - before.at.as_millis();
    }
    (amount > 0 && millis > 0).then_some(Usage { amount, millis })
}

/// The date `left` more is projected to take at `usage`'s pace, from `now`.
fn projected(left: i64, usage: Usage, now: UnixMillis, zone: &TimeZone) -> Option<String> {
    if left < 0 {
        return None;
    }
    let millis = i128::from(left) * i128::from(usage.millis) / i128::from(usage.amount);
    let at = i64::try_from(i128::from(now.as_millis()) + millis).ok()?;
    date_of(UnixMillis::from_millis(at), zone).map(|date| date.to_string())
}

/// Where requirement `id` stands, or `None` if it isn't on the schedule (#068) or has no
/// rule that can be worked out. `added_on` is the day it was added (`2026-11-16`), for a
/// calendar rule never done.
#[must_use]
pub fn requirement_due(
    id: RequirementId,
    requirement: &Requirement,
    context: &DueContext<'_>,
    added_on: Option<&str>,
) -> Option<Due> {
    if !requirement.on_schedule(id, context.done) {
        return None;
    }
    let last = last_done(id, context.done);
    let before = requirement.before_records.as_ref();
    let steps = requirement.warnings.unwrap_or(*context.defaults);
    let rules: Vec<RuleDue> = requirement
        .rules
        .iter()
        .filter_map(|rule| rule_due(rule, last, before, added_on, &steps, context))
        .collect();
    let step = rules.iter().map(|rule| rule.step).max()?;
    let date = rules
        .iter()
        .filter_map(|rule| match &rule.left {
            Left::Date { due } => Some(DueDate {
                on: due.clone(),
                estimate: false,
            }),
            Left::Meter { projected, .. } => {
                projected.clone().map(|on| DueDate { on, estimate: true })
            }
        })
        .min_by(|a, b| a.on.cmp(&b.on));
    Some(Due {
        step,
        rules,
        date,
        not_done_yet: last.is_none() && before.is_none(),
    })
}

/// The value a meter rule counts from: the reading when it was last done, or before these
/// records, or (never done) the meter's first reading.
fn base_reading(
    meter: MeterId,
    last: Option<&DoneTask>,
    before: Option<&LastDoneBefore>,
    context: &DueContext<'_>,
) -> Option<i64> {
    let from = |readings: &[crate::event::Reading]| {
        readings
            .iter()
            .find(|reading| reading.meter == meter)
            .map(|reading| reading.value)
    };
    match (last, before) {
        (Some(task), _) => from(&task.readings),
        (None, Some(before)) => from(&before.readings),
        (None, None) => context.first(meter).map(|reading| reading.value),
    }
}

/// The date a calendar rule counts from: when it was last done, or before these records, or
/// the day the requirement was added.
fn base_date(
    last: Option<&DoneTask>,
    before: Option<&LastDoneBefore>,
    added_on: Option<&str>,
    context: &DueContext<'_>,
) -> Option<jiff::civil::Date> {
    match (last, before) {
        (Some(task), _) => date_of(task.at, context.zone),
        (None, Some(before)) => before.on.parse().ok(),
        (None, None) => added_on?.parse().ok(),
    }
}

fn rule_due(
    rule: &IntervalRule,
    last: Option<&DoneTask>,
    before: Option<&LastDoneBefore>,
    added_on: Option<&str>,
    steps: &DueDefaults,
    context: &DueContext<'_>,
) -> Option<RuleDue> {
    match rule {
        IntervalRule::Meter { meter, every } => {
            let direction = direction_of(*meter, context)?;
            let base = base_reading(*meter, last, before, context)?;
            let now = context.latest(*meter)?.value;
            let due_at = match direction {
                MeterDirection::Up => base + every,
                MeterDirection::Down => base - every,
            };
            let left = match direction {
                MeterDirection::Up => due_at - now,
                MeterDirection::Down => now - due_at,
            };
            Some(meter_left(*meter, left, *every, direction, steps, context))
        }
        IntervalRule::Reaches { meter, at } => {
            let direction = direction_of(*meter, context)?;
            let now = context.latest(*meter)?.value;
            let left = match direction {
                MeterDirection::Up => at - now,
                MeterDirection::Down => now - at,
            };
            // The span it's measured over: from where it started (its reset value, or the
            // reading when last done) to the mark.
            let start = context
                .meters
                .iter()
                .find(|(id, _)| id == meter)
                .and_then(|(_, meter)| meter.reset_to)
                .or_else(|| base_reading(*meter, last, before, context))?;
            let interval = (start - at).abs();
            Some(meter_left(
                *meter, left, interval, direction, steps, context,
            ))
        }
        IntervalRule::Calendar {
            every,
            unit,
            end_of_month,
        } => {
            let from = base_date(last, before, added_on, context)?;
            let due: jiff::civil::Date =
                calendar_due(&from.to_string(), *every, *unit, *end_of_month)
                    .ok()?
                    .parse()
                    .ok()?;
            let left = days_between(context.today()?, due)?;
            let interval = days_between(from, due)?;
            Some(RuleDue {
                step: steps.interval.step(left, interval),
                left: Left::Date {
                    due: due.to_string(),
                },
            })
        }
        IntervalRule::FixedDate { first } => {
            // After a sign-off, the date it gave (or none); before any, the rule's first.
            let next = match last {
                Some(task) => task.next_due.clone()?,
                None => first.clone()?,
            };
            let due: jiff::civil::Date = next.parse().ok()?;
            let left = days_between(context.today()?, due)?;
            Some(RuleDue {
                step: steps.days.step(left),
                left: Left::Date { due: next },
            })
        }
    }
}

fn direction_of(meter: MeterId, context: &DueContext<'_>) -> Option<MeterDirection> {
    context
        .meters
        .iter()
        .find(|(id, _)| *id == meter)
        .map(|(_, meter)| meter.direction)
}

fn meter_left(
    meter: MeterId,
    left: i64,
    interval: i64,
    direction: MeterDirection,
    steps: &DueDefaults,
    context: &DueContext<'_>,
) -> RuleDue {
    let projected = usage(context.readings, meter, direction, context.now)
        .and_then(|usage| projected(left, usage, context.now, context.zone));
    RuleDue {
        step: steps.interval.step(left, interval),
        left: Left::Meter {
            meter,
            left,
            projected,
        },
    }
}

/// Where document `id` stands on `today` (`2026-11-16`) among its vehicle's `documents`, or
/// `None` if it never expires or has been renewed.
#[must_use]
pub fn document_due(
    id: AssetDocumentId,
    document: &AssetDocument,
    documents: &[(AssetDocumentId, AssetDocument)],
    today: &crate::dates::CalendarDate,
    defaults: &DueDefaults,
) -> Option<Due> {
    if crate::document::status(id, document, documents, today) == DocumentStatus::Superseded {
        return None;
    }
    let expires = document.expires_on.as_ref()?;
    let (from, to): (jiff::civil::Date, jiff::civil::Date) =
        (today.as_str().parse().ok()?, expires.as_str().parse().ok()?);
    let left = days_between(from, to)?;
    let steps = document.warn_days.unwrap_or(defaults.days);
    let step = steps.step(left);
    Some(Due {
        step,
        rules: vec![RuleDue {
            step,
            left: Left::Date {
                due: expires.as_str().to_owned(),
            },
        }],
        date: Some(DueDate {
            on: expires.as_str().to_owned(),
            estimate: false,
        }),
        not_done_yet: false,
    })
}

/// What a due item is.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", content = "id", rename_all = "kebab-case")]
pub enum DueItem {
    /// A requirement.
    Requirement(RequirementId),
    /// An official document.
    Document(AssetDocumentId),
}

/// Sorts `items` most urgent first: by step, then the earliest date (none last).
pub fn most_urgent_first(items: &mut [(DueItem, Due)]) {
    items.sort_by(|(_, a), (_, b)| {
        b.step.cmp(&a.step).then_with(|| match (&a.date, &b.date) {
            (Some(a), Some(b)) => a.on.cmp(&b.on),
            (Some(_), None) => core::cmp::Ordering::Less,
            (None, Some(_)) => core::cmp::Ordering::Greater,
            (None, None) => core::cmp::Ordering::Equal,
        })
    });
}

#[cfg(test)]
mod tests;
