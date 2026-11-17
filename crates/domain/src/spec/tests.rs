use super::*;
use crate::test_support::test_id;

const OIL: u32 = 30;
const FILTER: u32 = 31;
const LUGS: u32 = 32;
const NOTE: u32 = 33;

fn amount(text: &str, unit: SpecUnit) -> SpecAmount {
    SpecAmount::parse(text, unit).unwrap()
}

fn spec(name: &str, amount: Option<SpecAmount>, part_number: Option<&str>) -> Spec {
    Spec {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        amount,
        detail: None,
        part_number: part_number.map(str::to_owned),
        notes: Notes::default(),
    }
}

/// The truck's specs: oil (5.5 qt, 5W-30), its filter (FL-500S), lug nut torque, and one
/// with only a name.
fn specs() -> Vec<(SpecId, Spec)> {
    let mut oil = spec("Engine oil", Some(amount("5.5", SpecUnit::Quarts)), None);
    oil.detail = Some("5W-30 full synthetic".into());
    vec![
        (test_id(OIL), oil),
        (test_id(FILTER), spec("Oil filter", None, Some("FL-500S"))),
        (
            test_id(LUGS),
            spec(
                "Lug nut torque",
                Some(amount("150", SpecUnit::FootPounds)),
                None,
            ),
        ),
        (test_id(NOTE), spec("Wiper blades", None, None)),
    ]
}

#[test]
fn every_unit_has_a_kind_and_converts_to_the_other_system() {
    for unit in SpecUnit::ALL {
        let (other, _) = unit.other();
        assert_eq!(other.kind(), unit.kind(), "{unit:?}");
        assert_ne!(other, unit, "{unit:?}");
        assert!(Translator::english().has(unit.abbr()), "{unit:?}");
        // There and back again.
        let back = unit.in_unit(unit.to_base(12.5));
        assert!((back - 12.5).abs() < 1e-9, "{unit:?}: {back}");
    }
    for kind in SpecKind::ALL {
        assert!(!kind.units().is_empty());
        assert!(Translator::english().has(kind.label()));
    }
}

#[test]
fn amounts_show_as_entered_with_the_other_system_beside_them() {
    let cases = [
        ("5.5", SpecUnit::Quarts, "5.5 qt (5.2 L)"),
        ("6", SpecUnit::Liters, "6 L (6.3 qt)"),
        ("150", SpecUnit::FootPounds, "150 ft-lb (203 N·m)"),
        ("120", SpecUnit::NewtonMeters, "120 N·m (89 ft-lb)"),
        ("35", SpecUnit::Psi, "35 psi (241 kPa)"),
        ("2.4", SpecUnit::Bar, "2.4 bar (35 psi)"),
        ("0.044", SpecUnit::Inches, "0.044 in (1.12 mm)"),
        ("8", SpecUnit::FluidOunces, "8 fl oz (237 mL)"),
    ];
    for (typed, unit, shown) in cases {
        assert_eq!(amount(typed, unit).show_both(), shown, "{typed} {unit:?}");
    }
    assert_eq!(amount("5.500", SpecUnit::Quarts).value_text(), "5.5");
}

#[test]
fn an_amount_must_be_a_number_above_zero_with_three_decimals_at_most() {
    assert_eq!(
        SpecAmount::parse("5.5", SpecUnit::Quarts).map(|amount| amount.milli),
        Ok(5_500)
    );
    assert_eq!(
        SpecAmount::parse("0.0441", SpecUnit::Inches),
        Err(QuantityError::TooPrecise)
    );
    assert!(SpecAmount::parse("0", SpecUnit::Psi).is_err());
    assert!(SpecAmount::parse("-2", SpecUnit::Psi).is_err());
    assert!(SpecAmount::parse("lots", SpecUnit::Psi).is_err());
}

#[test]
fn a_specs_detail_and_part_number_are_checked() {
    let mut oil = specs()[0].1.clone();
    assert_eq!(check_spec(&oil), Ok(()));
    oil.detail = Some("x".repeat(DETAIL_MAX_CHARS + 1));
    assert_eq!(check_spec(&oil), Err(SpecProblem::BadDetail));
    oil.detail = Some(String::new());
    assert_eq!(check_spec(&oil), Err(SpecProblem::BadDetail));
    oil.detail = None;
    oil.part_number = Some("x".repeat(PART_NUMBER_MAX_CHARS + 1));
    assert_eq!(check_spec(&oil), Err(SpecProblem::BadPartNumber));
    oil.part_number = None;
    oil.amount = Some(SpecAmount {
        milli: 0,
        unit: SpecUnit::Quarts,
    });
    assert_eq!(check_spec(&oil), Err(SpecProblem::BadAmount));
    for problem in [
        SpecProblem::BadDetail,
        SpecProblem::BadPartNumber,
        SpecProblem::BadAmount,
    ] {
        assert!(Translator::english().has(problem.message()));
    }
}

#[test]
fn ticked_specs_pre_fill_parts_and_reminders() {
    let ticked = [
        test_id(OIL),
        test_id(FILTER),
        test_id(LUGS),
        test_id(NOTE),
        test_id(99),
    ];
    let found = task_specs(&ticked, &specs());
    assert_eq!(
        found.parts,
        vec![
            Part {
                description: "Engine oil 5W-30 full synthetic (qt)".into(),
                part_number: None,
                quantity_milli: 5_500,
                unit_price_minor: None,
                spec: Some(test_id(OIL)),
            },
            Part {
                description: "Oil filter".into(),
                part_number: Some("FL-500S".into()),
                quantity_milli: 1_000,
                unit_price_minor: None,
                spec: Some(test_id(FILTER)),
            },
        ]
    );
    // Torque is a reminder; a spec with only a name puts nothing; an unknown one is skipped.
    assert_eq!(found.reminders, vec![test_id(LUGS)]);
}

#[test]
fn a_long_name_and_detail_still_fit_a_part() {
    let mut long = spec(
        &"n".repeat(crate::label::LABEL_MAX_CHARS),
        Some(amount("1", SpecUnit::Liters)),
        None,
    );
    long.detail = Some("d".repeat(DETAIL_MAX_CHARS));
    let part = long.part(test_id(1)).unwrap();
    assert_eq!(part.description.chars().count(), 100);
}

#[test]
fn a_job_offers_back_what_differs_from_its_specs() {
    let specs = specs();
    let mut parts = task_specs(&[test_id(OIL), test_id(FILTER)], &specs).parts;
    assert!(spec_updates(&parts, &specs).is_empty(), "as pre-filled");
    // 6 quarts this time, and another filter.
    parts[0].quantity_milli = 6_000;
    parts[1].part_number = Some("FL-910S".into());
    // A part typed in by hand isn't a spec's.
    parts.push(Part {
        description: "Drain plug washer".into(),
        part_number: Some("W-1".into()),
        quantity_milli: 1_000,
        unit_price_minor: None,
        spec: None,
    });
    let updates = spec_updates(&parts, &specs);
    assert_eq!(updates.len(), 2);
    assert_eq!(updates[0].spec, test_id(OIL));
    assert_eq!(
        updates[0].amount,
        Some(SpecAmount {
            milli: 6_000,
            unit: SpecUnit::Quarts
        })
    );
    assert_eq!(updates[0].part_number, None, "unchanged");
    assert_eq!(updates[1].part_number, Some(Some("FL-910S".into())));
    assert_eq!(updates[1].amount, None, "the filter has no amount");
    let patch = updates[1].patch();
    assert_eq!(
        patch.part_number,
        Some(Clearable::Set("FL-910S".to_owned()))
    );
    assert_eq!(patch.amount, None);
    // Clearing a part number offers clearing the spec's.
    parts[1].part_number = Some("  ".into());
    assert_eq!(spec_updates(&parts, &specs)[1].part_number, Some(None));
}

#[test]
fn a_patch_carries_only_what_changed() {
    let oil = specs()[0].1.clone();
    let mut changed = oil.clone();
    changed.part_number = Some("M1-110A".into());
    let patch = SpecPatch::changes(&oil, changed);
    assert_eq!(
        patch,
        SpecPatch {
            part_number: Some(Clearable::Set("M1-110A".into())),
            ..SpecPatch::default()
        }
    );
    let all = SpecPatch::all(oil);
    assert!(all.asset.is_some() && all.name.is_some() && all.notes.is_some());
}
