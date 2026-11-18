use super::*;
use crate::test_support::test_id;

fn usage(name: &str, default: bool) -> Usage {
    Usage {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        default,
        documents: vec![],
    }
}

#[test]
fn a_usages_name_is_its_own_and_documents_are_listed_once() {
    let others = vec![
        (test_id(10), usage("Drivable", true)),
        (test_id(11), usage("Towing", false)),
    ];
    assert_eq!(
        check_usage(None, &usage("towing", false), &others),
        Err(UsageProblem::NameTaken)
    );
    assert_eq!(
        check_usage(Some(test_id(11)), &usage("Towing", false), &others),
        Ok(()),
        "its own name, unchanged"
    );
    let insurance = DocumentType::listed("document-insurance").unwrap();
    let twice = Usage {
        documents: vec![insurance.clone(), insurance],
        ..usage("Highway", false)
    };
    assert_eq!(
        check_usage(None, &twice, &others),
        Err(UsageProblem::RepeatedDocument)
    );
    for problem in [UsageProblem::NameTaken, UsageProblem::DefaultStays] {
        assert!(Translator::english().has(problem.message()));
    }
}

#[test]
fn each_kind_has_its_default_usage() {
    let kind = |id: &str| AssetKind::parse(id).unwrap();
    let truck = default_usage(test_id(1), &kind("truck")).unwrap();
    assert_eq!(truck.name.as_str(), "Drivable");
    assert!(truck.default);
    let keys: Vec<String> = truck
        .documents
        .iter()
        .map(|document| match document {
            DocumentType::Listed { key } => key.clone(),
            DocumentType::Custom { name } => name.as_str().to_owned(),
        })
        .collect();
    assert_eq!(
        keys,
        ["document-registration", "document-insurance"],
        "not title"
    );
    assert_eq!(
        default_usage(test_id(1), &kind("trailer"))
            .unwrap()
            .name
            .as_str(),
        "Towable"
    );
    let generator = default_usage(test_id(1), &kind("generator")).unwrap();
    assert_eq!(generator.name.as_str(), "Usable");
    assert!(generator.documents.is_empty());
    assert_eq!(
        default_usage(test_id(1), &kind("boat"))
            .unwrap()
            .name
            .as_str(),
        "Usable",
        "a kind without a profile"
    );
}

#[test]
fn copied_usages_match_by_name_and_the_default_by_being_the_default() {
    let from = vec![
        (test_id(10), usage("Drivable", true)),
        (test_id(11), usage("Towing", false)),
        (test_id(12), usage("Highway", false)),
    ];
    let to = vec![
        (test_id(20), usage("Ready to drive", true)),
        (test_id(21), usage("TOWING", false)),
    ];
    assert_eq!(
        match_usages(
            &[test_id(10), test_id(11), test_id(12), test_id(11)],
            &from,
            &to
        ),
        vec![
            (test_id(10), Some(test_id(20))),
            (test_id(11), Some(test_id(21))),
            (test_id(12), None),
        ]
    );
    assert_eq!(default_of(&to), Some(test_id(20)));
}
