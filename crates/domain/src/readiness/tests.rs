use super::*;
use crate::dates::CalendarDate;
use crate::due::{DueDate, Left, RuleDue};
use crate::notes::Notes;
use crate::requirement::{Applicability, ComplianceMode};
use crate::test_support::test_id;

const DRIVABLE: u32 = 10;
const TOWING: u32 = 11;

fn usage(name: &str, default: bool, documents: &[&str]) -> Usage {
    Usage {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        default,
        documents: documents
            .iter()
            .map(|key| DocumentType::listed(key).unwrap())
            .collect(),
    }
}

fn requirement(name: &str, usages: Vec<UsageId>) -> Requirement {
    Requirement {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        notes: Notes::default(),
        enabled: true,
        rules: vec![],
        reference: None,
        mode: ComplianceMode::Recurring,
        applicability: Applicability::Applies,
        before_records: None,
        warnings: None,
        specs: vec![],
        usages,
    }
}

fn due(step: DueStep) -> Due {
    Due {
        step,
        rules: vec![RuleDue {
            step,
            left: Left::Date {
                due: "2026-12-01".into(),
            },
            since: None,
        }],
        date: Some(DueDate {
            on: "2026-12-01".into(),
            estimate: false,
        }),
        not_done_yet: false,
    }
}

fn document(key: &str, expires: Option<&str>) -> AssetDocument {
    AssetDocument {
        asset: test_id(1),
        doc_type: DocumentType::listed(key).unwrap(),
        number: None,
        issuer: None,
        issued_on: None,
        effective_on: None,
        expires_on: expires.map(|date| CalendarDate::parse(date).unwrap()),
        supersedes: None,
        plate: None,
        notes: Notes::default(),
        added_at: crate::time::UnixMillis::from_millis(0),
        added_by: test_id(5),
        received_at: crate::time::UnixMillis::from_millis(0),
        warn_days: None,
    }
}

/// The truck: an oil change (overdue, ticking nothing: the default), a tire rotation (due
/// soon, for Towing only), a hitch inspection (fine, for Towing); registration on file
/// (expiring soon), no insurance.
struct Truck {
    requirements: Vec<(RequirementId, Requirement)>,
    due: Vec<DueEntry>,
    documents: Vec<(AssetDocumentId, AssetDocument)>,
    today: CalendarDate,
}

fn truck() -> Truck {
    let requirements = vec![
        (test_id(20), requirement("Engine oil & filter", vec![])),
        (
            test_id(21),
            requirement("Tire rotation", vec![test_id(TOWING)]),
        ),
        (
            test_id(22),
            requirement("Hitch inspection", vec![test_id(TOWING)]),
        ),
        (test_id(23), requirement("Wipers", vec![test_id(DRIVABLE)])),
    ];
    let entry = |id: u32, name: &str, step| DueEntry {
        subject: DueSubject::Requirement {
            id: test_id(id),
            name: Label::parse(name).unwrap(),
        },
        due: due(step),
    };
    let registration = document("document-registration", Some("2026-12-01"));
    let due = vec![
        entry(20, "Engine oil & filter", DueStep::Overdue),
        entry(21, "Tire rotation", DueStep::DueSoon),
        entry(22, "Hitch inspection", DueStep::Ok),
        // Wipers: not on the schedule, so not listed.
        DueEntry {
            subject: DueSubject::Document {
                id: test_id(30),
                doc_type: registration.doc_type.clone(),
            },
            due: due(DueStep::DueVerySoon),
        },
    ];
    Truck {
        requirements,
        due,
        documents: vec![(test_id(30), registration)],
        today: CalendarDate::parse("2026-11-18").unwrap(),
    }
}

fn records<'a>(truck: &'a Truck, high: &'a [HighIssue]) -> VehicleRecords<'a> {
    VehicleRecords {
        requirements: &truck.requirements,
        due: &truck.due,
        documents: &truck.documents,
        high,
        today: &truck.today,
    }
}

fn marks(readiness: &Readiness) -> Vec<(Mark, String)> {
    readiness
        .items
        .iter()
        .map(|item| {
            let what = match &item.reason {
                ItemReason::Requirement { name, .. } => name.as_str().to_owned(),
                ItemReason::Document { doc_type, .. }
                | ItemReason::MissingDocument { doc_type } => match doc_type {
                    DocumentType::Listed { key } => key.clone(),
                    DocumentType::Custom { name } => name.as_str().to_owned(),
                },
                ItemReason::HighIssue { description, .. } => description.clone(),
            };
            (item.mark, what)
        })
        .collect()
}

#[test]
fn the_default_usage_counts_requirements_ticking_none_and_needs_its_documents() {
    let truck = truck();
    let drivable = usage(
        "Drivable",
        true,
        &["document-registration", "document-insurance"],
    );
    let ready = readiness(test_id(DRIVABLE), &drivable, &records(&truck, &[]));
    assert_eq!(ready.mark, Mark::NotReady);
    assert_eq!(
        marks(&ready),
        vec![
            (Mark::NotReady, "Engine oil & filter".to_owned()),
            (Mark::NotReady, "document-insurance".to_owned()),
            (Mark::Warning, "document-registration".to_owned()),
        ],
        "worst first; insurance none on file; the wipers aren't on the schedule"
    );
    assert!(matches!(
        ready.items[1].reason,
        ItemReason::MissingDocument { .. }
    ));
}

#[test]
fn each_usage_stands_alone() {
    let truck = truck();
    let towing = usage("Towing", false, &[]);
    let ready = readiness(test_id(TOWING), &towing, &records(&truck, &[]));
    // Only what's ticked for Towing: not the oil change (the default's), no documents.
    assert_eq!(
        marks(&ready),
        vec![
            (Mark::Warning, "Tire rotation".to_owned()),
            (Mark::Ready, "Hitch inspection".to_owned()),
        ]
    );
    assert_eq!(ready.mark, Mark::Warning);
    assert!(!ready.default);
}

#[test]
fn a_high_issue_blocks_every_usage_even_deferred() {
    let truck = truck();
    let high = [HighIssue {
        id: test_id(40),
        description: "Brakes grinding".into(),
        deferred: true,
    }];
    let nothing_needed = usage("Local", false, &[]);
    let ready = readiness(test_id(12), &nothing_needed, &records(&truck, &high));
    assert_eq!(ready.mark, Mark::NotReady);
    assert_eq!(
        marks(&ready),
        vec![(Mark::NotReady, "Brakes grinding".to_owned())]
    );
    let alone = readiness(test_id(12), &nothing_needed, &records(&truck, &[]));
    assert_eq!(
        (alone.mark, alone.items.len()),
        (Mark::Ready, 0),
        "✓ with nothing"
    );
}

#[test]
fn a_renewed_document_gives_way_to_its_renewal() {
    let mut truck = truck();
    let mut renewal = document("document-registration", Some("2027-12-01"));
    renewal.supersedes = Some(test_id(30));
    truck.documents.push((test_id(31), renewal));
    let needs = usage("Drivable", true, &["document-registration"]);
    let ready = readiness(test_id(DRIVABLE), &needs, &records(&truck, &[]));
    let document = ready
        .items
        .iter()
        .find_map(|item| match &item.reason {
            ItemReason::Document { id, due, .. } => Some((*id, due.clone())),
            ItemReason::Requirement { .. }
            | ItemReason::MissingDocument { .. }
            | ItemReason::HighIssue { .. } => None,
        })
        .unwrap();
    // The renewal is current; nothing due for it in these records, so ✓.
    assert_eq!(document, (test_id(31), None));
}

#[test]
fn a_due_step_gives_its_mark() {
    assert_eq!(
        [
            DueStep::Ok,
            DueStep::ComingUp,
            DueStep::DueSoon,
            DueStep::DueVerySoon,
            DueStep::Overdue
        ]
        .map(Mark::of_step),
        [
            Mark::Ready,
            Mark::Ready,
            Mark::Warning,
            Mark::Warning,
            Mark::NotReady
        ]
    );
}

#[test]
fn a_requirement_ticking_none_is_the_defaults() {
    let none = requirement("Oil", vec![]);
    assert!(none.needed_for(test_id(DRIVABLE), true));
    assert!(!none.needed_for(test_id(TOWING), false));
    let towing = requirement("Hitch", vec![test_id(TOWING)]);
    assert!(!towing.needed_for(test_id(DRIVABLE), true));
    assert!(towing.needed_for(test_id(TOWING), false));
}
