use proptest::prelude::*;

use super::*;
use crate::dates::{CalendarDate, local_to_utc};
use crate::document::DocumentType;
use crate::event::Reading;
use crate::label::Label;
use crate::meter::{MeterDimension, MeterUnit};
use crate::notes::Notes;
use crate::requirement::{Applicability, CalendarUnit, ComplianceMode};
use crate::test_support::test_id;

const ODOMETER: u32 = 10;
const OIL_LIFE: u32 = 11;
const OIL_CHANGE: u32 = 20;
const MILE: i64 = 1_609;

fn utc() -> TimeZone {
    TimeZone::parse("UTC").unwrap()
}

fn on(date: &str) -> UnixMillis {
    local_to_utc(date, "12:00", &utc()).unwrap()
}

fn meters() -> Vec<(MeterId, Meter)> {
    let meter = |name: &str, dimension, unit, direction, reset_to| Meter {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        dimension,
        unit,
        precision: 0,
        description: None,
        required_for: vec![],
        direction,
        reset_to,
    };
    vec![
        (
            test_id(ODOMETER),
            meter(
                "Odometer",
                MeterDimension::Distance,
                MeterUnit::Miles,
                MeterDirection::Up,
                None,
            ),
        ),
        (
            test_id(OIL_LIFE),
            meter(
                "Oil life",
                MeterDimension::Percentage,
                MeterUnit::Percent,
                MeterDirection::Down,
                Some(10_000),
            ),
        ),
    ]
}

fn read(meter: u32, date: &str, value: i64) -> PastReading {
    PastReading {
        meter: test_id(meter),
        at: on(date),
        value,
        reset: false,
    }
}

fn oil_change(rules: Vec<IntervalRule>) -> Requirement {
    Requirement {
        asset: test_id(1),
        name: Label::parse("Engine oil & filter").unwrap(),
        notes: Notes::default(),
        enabled: true,
        rules,
        reference: None,
        mode: ComplianceMode::Recurring,
        applicability: Applicability::Applies,
        before_records: None,
        warnings: None,
    }
}

/// The oil change done on `date` at `miles`, Oil life reset to 100%.
fn done_on(date: &str, miles: i64) -> DoneTask {
    DoneTask {
        requirement: test_id(OIL_CHANGE),
        at: on(date),
        event: test_id(50),
        readings: vec![
            Reading {
                meter: test_id(ODOMETER),
                value: miles * MILE,
                reset: false,
            },
            Reading {
                meter: test_id(OIL_LIFE),
                value: 10_000,
                reset: true,
            },
        ],
        next_due: None,
    }
}

/// `requirement`'s due on `today`, with `readings` and `done`.
fn due(
    requirement: &Requirement,
    today: &str,
    readings: &[PastReading],
    done: &[DoneTask],
) -> Option<Due> {
    let (meters, zone, defaults) = (meters(), utc(), DueDefaults::default());
    let context = DueContext {
        now: on(today),
        zone: &zone,
        meters: &meters,
        readings,
        done,
        defaults: &defaults,
    };
    requirement_due(
        test_id(OIL_CHANGE),
        requirement,
        &context,
        Some("2026-01-01"),
    )
}

fn every_miles(miles: i64) -> IntervalRule {
    IntervalRule::Meter {
        meter: test_id(ODOMETER),
        every: miles * MILE,
    }
}

fn left_on(due: &Due, meter: u32) -> Option<(i64, Option<String>)> {
    due.rules.iter().find_map(|rule| match &rule.left {
        Left::Meter {
            meter: id,
            left,
            projected,
        } if *id == test_id(meter) => Some((*left, projected.clone())),
        Left::Meter { .. } | Left::Date { .. } => None,
    })
}

#[test]
fn steps_come_in_order_and_at_their_shares() {
    assert!(DueStep::Ok < DueStep::ComingUp && DueStep::DueVerySoon < DueStep::Overdue);
    let steps = IntervalSteps::default();
    assert_eq!(steps.step(2_000, 10_000), DueStep::ComingUp, "20% exactly");
    assert_eq!(steps.step(2_001, 10_000), DueStep::Ok);
    assert_eq!(steps.step(1_000, 10_000), DueStep::DueSoon);
    assert_eq!(steps.step(0, 10_000), DueStep::DueVerySoon, "due now");
    assert_eq!(steps.step(-1, 10_000), DueStep::Overdue);
    let days = DaySteps::default();
    assert_eq!(days.step(61), DueStep::Ok);
    assert_eq!(days.step(60), DueStep::ComingUp);
    assert_eq!(days.step(30), DueStep::DueSoon);
    assert_eq!(days.step(14), DueStep::DueVerySoon);
    assert_eq!(days.step(-1), DueStep::Overdue);
    assert!(DueDefaults::default().valid());
    let backwards = IntervalSteps {
        coming_up: 5,
        due_soon: 10,
        very_soon: 20,
    };
    assert!(!backwards.valid());
}

#[test]
fn a_meter_rule_counts_from_when_it_was_last_done() {
    let requirement = oil_change(vec![every_miles(7_500)]);
    let done = [done_on("2026-06-01", 50_000)];
    let at = |miles: i64| {
        let readings = [
            read(ODOMETER, "2026-06-01", 50_000 * MILE),
            read(ODOMETER, "2026-11-01", miles * MILE),
        ];
        due(&requirement, "2026-11-02", &readings, &done).unwrap()
    };
    assert_eq!(at(55_000).step, DueStep::Ok);
    assert_eq!(at(56_800).step, DueStep::DueSoon, "700 mi left: 9%");
    assert_eq!(left_on(&at(56_800), ODOMETER).unwrap().0, 700 * MILE);
    assert_eq!(at(57_200).step, DueStep::DueVerySoon);
    assert_eq!(at(57_600).step, DueStep::Overdue);
    assert!(!at(55_000).not_done_yet);
}

#[test]
fn whichever_rule_comes_first_sets_the_step_and_the_date() {
    let requirement = oil_change(vec![
        every_miles(7_500),
        IntervalRule::Calendar {
            every: 12,
            unit: CalendarUnit::Months,
            end_of_month: false,
        },
    ]);
    let done = [done_on("2026-01-15", 50_000)];
    // Few miles, but 45 days to the yearly date: 12% of the year left, coming up.
    let readings = [
        read(ODOMETER, "2026-01-15", 50_000 * MILE),
        read(ODOMETER, "2026-11-30", 51_000 * MILE),
    ];
    let due = due(&requirement, "2026-12-01", &readings, &done).unwrap();
    assert_eq!(due.step, DueStep::ComingUp);
    assert_eq!(
        due.date,
        Some(DueDate {
            on: "2027-01-15".into(),
            estimate: false
        })
    );
}

#[test]
fn oil_life_comes_due_as_it_falls_to_its_mark() {
    let requirement = oil_change(vec![IntervalRule::Reaches {
        meter: test_id(OIL_LIFE),
        at: 1_500,
    }]);
    let done = [done_on("2026-06-01", 50_000)];
    let at = |percent: i64| {
        let mut reset = read(OIL_LIFE, "2026-06-01", 10_000);
        reset.reset = true;
        let readings = [reset, read(OIL_LIFE, "2026-11-01", percent * 100)];
        due(&requirement, "2026-11-02", &readings, &done).unwrap()
    };
    // Measured over 100% down to 15%: 85 points.
    assert_eq!(at(40).step, DueStep::Ok);
    assert_eq!(at(30).step, DueStep::ComingUp, "15 points left: 17.6%");
    assert_eq!(at(18).step, DueStep::DueVerySoon, "3 points left: 3.5%");
    assert_eq!(left_on(&at(18), OIL_LIFE).unwrap().0, 300);
    assert_eq!(at(10).step, DueStep::Overdue);
}

#[test]
fn never_done_counts_from_before_the_records_or_the_first_reading() {
    let mut requirement = oil_change(vec![every_miles(5_000)]);
    let readings = [
        read(ODOMETER, "2026-03-01", 52_000 * MILE),
        read(ODOMETER, "2026-11-01", 56_500 * MILE),
    ];
    // From the first reading (52,000): 500 mi left of 5,000, not done yet.
    let first = due(&requirement, "2026-11-02", &readings, &[]).unwrap();
    assert!(first.not_done_yet);
    assert_eq!(left_on(&first, ODOMETER).unwrap().0, 500 * MILE);
    // Last done before these records at 49,000: overdue by 2,500 mi.
    requirement.before_records = Some(LastDoneBefore {
        on: "2025-12-01".into(),
        readings: vec![Reading {
            meter: test_id(ODOMETER),
            value: 49_000 * MILE,
            reset: false,
        }],
    });
    let before = due(&requirement, "2026-11-02", &readings, &[]).unwrap();
    assert!(!before.not_done_yet);
    assert_eq!(before.step, DueStep::Overdue);
    assert_eq!(left_on(&before, ODOMETER).unwrap().0, -2_500 * MILE);
}

#[test]
fn a_calendar_rule_never_done_counts_from_the_day_it_was_added() {
    let requirement = oil_change(vec![IntervalRule::Calendar {
        every: 6,
        unit: CalendarUnit::Months,
        end_of_month: true,
    }]);
    // Added 2026-01-01 (see `due`): due 2026-07-31 with the month's end.
    let due = due(&requirement, "2026-07-20", &[], &[]).unwrap();
    assert!(due.not_done_yet);
    assert_eq!(due.date.unwrap().on, "2026-07-31");
    assert_eq!(due.step, DueStep::DueSoon, "11 days of 211: 5.2%");
}

#[test]
fn a_fixed_date_warns_by_days_and_a_sign_off_gives_the_next() {
    let requirement = oil_change(vec![IntervalRule::FixedDate {
        first: Some("2026-12-10".into()),
    }]);
    let first = due(&requirement, "2026-11-16", &[], &[]).unwrap();
    assert_eq!(first.step, DueStep::DueSoon, "24 days");
    // Signed off with no next date: nothing comes due.
    let mut done = done_on("2026-12-01", 50_000);
    assert_eq!(due(&requirement, "2026-12-02", &[], &[done.clone()]), None);
    done.next_due = Some("2027-12-10".into());
    let next = due(&requirement, "2026-12-02", &[], &[done]).unwrap();
    assert_eq!(
        (next.step, next.date.unwrap().on.as_str()),
        (DueStep::Ok, "2027-12-10")
    );
}

#[test]
fn a_projected_date_comes_from_recent_use() {
    let requirement = oil_change(vec![every_miles(7_500)]);
    let done = [done_on("2026-06-01", 50_000)];
    // 30 mi a day over the last 60 days; 600 mi left: about 20 days.
    let readings = [
        read(ODOMETER, "2026-06-01", 50_000 * MILE),
        read(ODOMETER, "2026-09-02", 55_100 * MILE),
        read(ODOMETER, "2026-11-01", 56_900 * MILE),
    ];
    let due = due(&requirement, "2026-11-01", &readings, &done).unwrap();
    let (left, projected) = left_on(&due, ODOMETER).unwrap();
    assert_eq!(left, 600 * MILE);
    assert_eq!(projected.as_deref(), Some("2026-11-21"));
    assert_eq!(
        due.date,
        Some(DueDate {
            on: "2026-11-21".into(),
            estimate: true
        })
    );
}

#[test]
fn too_few_readings_give_no_date() {
    let readings = [
        read(ODOMETER, "2026-10-28", 1_000),
        read(ODOMETER, "2026-11-01", 2_000),
    ];
    assert_eq!(
        usage(
            &readings,
            test_id(ODOMETER),
            MeterDirection::Up,
            on("2026-11-02")
        ),
        None,
        "only 4 days apart"
    );
    let one = [read(ODOMETER, "2026-11-01", 2_000)];
    assert_eq!(
        usage(
            &one,
            test_id(ODOMETER),
            MeterDirection::Up,
            on("2026-11-02")
        ),
        None
    );
}

#[test]
fn use_across_a_reset_isnt_counted() {
    let mut reset = read(OIL_LIFE, "2026-10-01", 10_000);
    reset.reset = true;
    let readings = [
        read(OIL_LIFE, "2026-09-01", 4_000),
        reset,
        read(OIL_LIFE, "2026-10-31", 7_000),
    ];
    // Only October's fall counts: 30 points in 30 days, not the jump back up.
    assert_eq!(
        usage(
            &readings,
            test_id(OIL_LIFE),
            MeterDirection::Down,
            on("2026-11-01")
        ),
        Some(Usage {
            amount: 3_000,
            millis: 30 * DAY_MS,
        })
    );
}

#[test]
fn only_whats_on_the_schedule_comes_due() {
    let mut requirement = oil_change(vec![every_miles(7_500)]);
    requirement.enabled = false;
    let readings = [read(ODOMETER, "2026-11-01", 50_000 * MILE)];
    assert_eq!(due(&requirement, "2026-11-02", &readings, &[]), None);
    let none = oil_change(vec![]);
    assert_eq!(due(&none, "2026-11-02", &readings, &[]), None, "no rules");
}

#[test]
fn a_requirements_own_warnings_replace_the_households() {
    let mut requirement = oil_change(vec![every_miles(10_000)]);
    let done = [done_on("2026-06-01", 50_000)];
    let readings = [
        read(ODOMETER, "2026-06-01", 50_000 * MILE),
        read(ODOMETER, "2026-11-01", 57_000 * MILE),
    ];
    assert_eq!(
        due(&requirement, "2026-11-02", &readings, &done)
            .unwrap()
            .step,
        DueStep::Ok,
        "30% left"
    );
    requirement.warnings = Some(DueDefaults {
        interval: IntervalSteps {
            coming_up: 40,
            due_soon: 20,
            very_soon: 10,
        },
        days: DaySteps::default(),
    });
    assert_eq!(
        due(&requirement, "2026-11-02", &readings, &done)
            .unwrap()
            .step,
        DueStep::ComingUp
    );
}

fn insurance(expires: &str) -> AssetDocument {
    AssetDocument {
        asset: test_id(1),
        doc_type: DocumentType::Listed {
            key: "document-insurance".into(),
        },
        number: None,
        issuer: None,
        issued_on: None,
        effective_on: None,
        expires_on: Some(CalendarDate::parse(expires).unwrap()),
        supersedes: None,
        plate: None,
        notes: Notes::default(),
        added_at: on("2026-01-01"),
        added_by: test_id(5),
        received_at: on("2026-01-01"),
        warn_days: None,
    }
}

#[test]
fn a_document_warns_by_days_until_renewed() {
    let today = CalendarDate::parse("2026-11-16").unwrap();
    let defaults = DueDefaults::default();
    let current = insurance("2026-12-06");
    let documents = vec![(test_id(30), current.clone())];
    let due = document_due(test_id(30), &current, &documents, &today, &utc(), &defaults).unwrap();
    assert_eq!(due.step, DueStep::DueSoon, "20 days");
    let expired = insurance("2026-11-15");
    let gone = vec![(test_id(31), expired.clone())];
    assert_eq!(
        document_due(test_id(31), &expired, &gone, &today, &utc(), &defaults)
            .unwrap()
            .step,
        DueStep::Overdue
    );
    // Renewed: the old one doesn't come due.
    let mut renewal = insurance("2027-12-06");
    renewal.supersedes = Some(test_id(31));
    let both = vec![(test_id(31), expired.clone()), (test_id(32), renewal)];
    assert_eq!(
        document_due(test_id(31), &expired, &both, &today, &utc(), &defaults),
        None
    );
    let mut title = insurance("2026-12-06");
    title.expires_on = None;
    assert_eq!(
        document_due(test_id(33), &title, &[], &today, &utc(), &defaults),
        None
    );
}

#[test]
fn the_most_urgent_come_first_then_the_soonest() {
    let item = |n: u32, step, date: Option<&str>| {
        (
            DueItem::Requirement(test_id(n)),
            Due {
                step,
                rules: vec![],
                date: date.map(|on| DueDate {
                    on: on.into(),
                    estimate: false,
                }),
                not_done_yet: false,
            },
        )
    };
    let mut items = vec![
        item(1, DueStep::Ok, Some("2027-01-01")),
        item(2, DueStep::DueSoon, None),
        item(3, DueStep::DueSoon, Some("2026-12-01")),
        item(4, DueStep::Overdue, Some("2026-10-01")),
    ];
    most_urgent_first(&mut items);
    let order: Vec<DueItem> = items.iter().map(|(item, _)| *item).collect();
    assert_eq!(
        order,
        [4, 3, 2, 1].map(|n| DueItem::Requirement(test_id(n)))
    );
}

#[test]
fn a_step_is_reached_by_the_reading_that_took_it_there() {
    let requirement = oil_change(vec![every_miles(7_500)]);
    let done = [done_on("2026-06-01", 50_000)];
    let mut readings = vec![
        read(ODOMETER, "2026-06-01", 50_000 * MILE),
        read(ODOMETER, "2026-08-01", 55_000 * MILE),
        // 700 mi left (9%): due soon from here.
        read(ODOMETER, "2026-09-01", 56_800 * MILE),
        read(ODOMETER, "2026-10-01", 57_000 * MILE),
    ];
    let soon = due(&requirement, "2026-10-15", &readings, &done).unwrap();
    assert_eq!(soon.step, DueStep::DueSoon);
    assert_eq!(soon.since(), Some(on("2026-09-01")));
    // 100 mi over: overdue from the reading that went past.
    readings.push(read(ODOMETER, "2026-11-01", 57_600 * MILE));
    let over = due(&requirement, "2026-11-10", &readings, &done).unwrap();
    assert_eq!(over.step, DueStep::Overdue);
    assert_eq!(over.since(), Some(on("2026-11-01")));
    // OK: no time.
    let ok = due(&requirement, "2026-08-02", &readings[..2], &done).unwrap();
    assert_eq!((ok.step, ok.since()), (DueStep::Ok, None));
}

#[test]
fn a_dates_step_began_at_the_start_of_its_day() {
    let today = CalendarDate::parse("2026-12-10").unwrap();
    let document = insurance("2026-12-31");
    let documents = vec![(test_id(30), document.clone())];
    let due = document_due(
        test_id(30),
        &document,
        &documents,
        &today,
        &utc(),
        &DueDefaults::default(),
    )
    .unwrap();
    // 21 days left: due soon since 30 days before.
    assert_eq!(due.step, DueStep::DueSoon);
    assert_eq!(
        due.since(),
        Some(local_to_utc("2026-12-01", "00:00", &utc()).unwrap())
    );
    // Expired: overdue from the day after.
    let late = CalendarDate::parse("2027-01-05").unwrap();
    let over = document_due(
        test_id(30),
        &document,
        &documents,
        &late,
        &utc(),
        &DueDefaults::default(),
    )
    .unwrap();
    assert_eq!(
        over.since(),
        Some(local_to_utc("2027-01-01", "00:00", &utc()).unwrap())
    );
}

#[test]
fn a_snooze_counts_on_a_meter_that_counts_up() {
    let reaches = IntervalRule::Reaches {
        meter: test_id(OIL_LIFE),
        at: 1_500,
    };
    let months = IntervalRule::Calendar {
        every: 12,
        unit: CalendarUnit::Months,
        end_of_month: false,
    };
    assert_eq!(
        snooze_meter(
            &oil_change(vec![reaches.clone(), months.clone(), every_miles(7_500)]),
            &meters()
        ),
        Some(test_id(ODOMETER)),
        "Oil life counts down"
    );
    assert_eq!(
        snooze_meter(&oil_change(vec![reaches, months]), &meters()),
        None,
        "time only"
    );
}

#[test]
fn the_fleet_counts_overdue_and_due_soon_items() {
    let counts = FleetCounts::of(
        [
            DueStep::Ok,
            DueStep::ComingUp,
            DueStep::DueSoon,
            DueStep::DueVerySoon,
            DueStep::Overdue,
        ],
        3,
    );
    assert_eq!(
        counts,
        FleetCounts {
            overdue: 1,
            due_soon: 2,
            open_squawks: 3,
        }
    );
}

#[test]
fn messages_start_at_coming_up_due_soon_or_due_very_soon() {
    let starts: Vec<bool> = [
        DueStep::Ok,
        DueStep::ComingUp,
        DueStep::DueSoon,
        DueStep::DueVerySoon,
        DueStep::Overdue,
    ]
    .map(DueStep::starts_messages)
    .to_vec();
    assert_eq!(starts, [false, true, true, true, false]);
}

proptest! {
    #[test]
    fn less_left_is_never_less_urgent(
        interval in 1i64..1_000_000,
        a in -1_000_000i64..1_000_000,
        b in -1_000_000i64..1_000_000,
    ) {
        let steps = IntervalSteps::default();
        let (less, more) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(steps.step(less, interval) >= steps.step(more, interval));
    }
}
