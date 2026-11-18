use super::*;
use crate::due::LastDoneBefore;
use crate::meter::{MeterDimension, MeterDirection, MeterUnit};
use crate::notes::Notes;
use crate::requirement::{Applicability, CalendarUnit, ComplianceMode};
use crate::spec::{SpecAmount, SpecUnit};
use crate::test_support::test_id;

const TRUCK: u32 = 1;
const CAR: u32 = 2;

fn meter(asset: u32, name: &str, dimension: MeterDimension) -> Meter {
    Meter {
        asset: test_id(asset),
        name: Label::parse(name).unwrap(),
        dimension,
        unit: if dimension == MeterDimension::Distance {
            MeterUnit::Miles
        } else {
            MeterUnit::Hours
        },
        precision: 0,
        description: None,
        required_for: vec![],
        direction: MeterDirection::Up,
        reset_to: None,
    }
}

/// The truck's odometer (10), engine hours (11), and a trip meter no rule uses (12).
fn truck_meters() -> Vec<(MeterId, Meter)> {
    vec![
        (
            test_id(10),
            meter(TRUCK, "Odometer", MeterDimension::Distance),
        ),
        (
            test_id(11),
            meter(TRUCK, "Engine hours", MeterDimension::Duration),
        ),
        (test_id(12), meter(TRUCK, "Trip", MeterDimension::Distance)),
    ]
}

fn oil_change() -> Requirement {
    Requirement {
        asset: test_id(TRUCK),
        name: Label::parse("Engine oil & filter").unwrap(),
        notes: Notes::default(),
        enabled: true,
        rules: vec![
            IntervalRule::Meter {
                meter: test_id(10),
                every: 8_046_720,
            },
            IntervalRule::Meter {
                meter: test_id(11),
                every: 900_000,
            },
            IntervalRule::Calendar {
                every: 12,
                unit: CalendarUnit::Months,
                end_of_month: false,
            },
        ],
        reference: Some(test_id(40)),
        mode: ComplianceMode::Recurring,
        applicability: Applicability::Applies,
        before_records: Some(LastDoneBefore {
            on: "2026-01-01".into(),
            readings: vec![],
        }),
        warnings: None,
        specs: vec![test_id(30)],
        usages: None,
    }
}

fn template() -> ScheduleTemplate {
    let oil = Spec {
        asset: test_id(TRUCK),
        name: Label::parse("Engine oil").unwrap(),
        amount: Some(SpecAmount::parse("5.5", SpecUnit::Quarts).unwrap()),
        detail: None,
        part_number: None,
        notes: Notes::default(),
    };
    let service = Bundle {
        asset: test_id(TRUCK),
        name: Label::parse("Annual service").unwrap(),
        requirements: vec![test_id(20)],
    };
    ScheduleTemplate::from_vehicle(
        Label::parse("Pickup basics").unwrap(),
        &truck_meters(),
        &[(test_id(20), oil_change())],
        &[service],
        &[(test_id(30), oil)],
        &[],
    )
}

#[test]
fn a_template_keeps_the_schedule_but_not_the_vehicles_own_past() {
    let template = template();
    assert_eq!(
        template.counts(),
        TemplateCounts {
            requirements: 1,
            bundles: 1,
            specs: 1,
        }
    );
    let (_, oil) = &template.requirements[0];
    assert_eq!(oil.reference, None, "the recall was the truck's");
    assert_eq!(oil.before_records, None);
    assert_eq!(oil.specs, vec![test_id(30)], "its specs stay ticked");
    // Only the meters its rules use.
    let names: Vec<&str> = template
        .meters
        .iter()
        .map(|(_, meter)| meter.name.as_str())
        .collect();
    assert_eq!(names, ["Odometer", "Engine hours"]);
}

#[test]
fn applying_a_template_copies_it_onto_the_vehicle() {
    // The car has an odometer, but no engine hours.
    let car_meters = vec![(
        test_id(50),
        meter(CAR, "odometer", MeterDimension::Distance),
    )];
    let copied = template().apply(&car_meters, test_id(CAR));
    let (old_id, oil) = &copied.requirements[0];
    assert_eq!(
        *old_id,
        test_id(20),
        "so bundles and specs can be re-linked"
    );
    assert_eq!(oil.asset, test_id(CAR));
    assert_eq!(
        oil.rules,
        vec![
            IntervalRule::Meter {
                meter: test_id(50),
                every: 8_046_720,
            },
            IntervalRule::Calendar {
                every: 12,
                unit: CalendarUnit::Months,
                end_of_month: false,
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
    assert_eq!(copied.bundles[0].asset, test_id(CAR));
    assert_eq!(copied.specs[0].0, test_id(30));
    assert_eq!(copied.specs[0].1.asset, test_id(CAR));
}

#[test]
fn a_templates_patch_carries_every_part() {
    let patch = ScheduleTemplatePatch::all(template());
    assert!(
        patch.name.is_some()
            && patch.meters.is_some()
            && patch.requirements.is_some()
            && patch.bundles.is_some()
            && patch.specs.is_some()
    );
}
