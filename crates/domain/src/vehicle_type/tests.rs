use super::*;
use crate::catalogue::TireAxle;
use crate::test_support::test_id;

fn kind(id: &str) -> AssetKind {
    AssetKind::parse(id).unwrap()
}

fn label(text: &str) -> Label {
    Label::parse(text).unwrap()
}

fn key(text: &str) -> FieldKey {
    FieldKey::parse(text).unwrap()
}

/// "Track car": a car needing only insurance, trip time off, with the Tires set.
fn track_car() -> VehicleType {
    VehicleType {
        icon: TypeIcon::Flag,
        documents: vec![DocumentType::listed("document-insurance").unwrap()],
        fields_on: vec![],
        catalogue: vec![CatalogueChoice::Tires {
            axles: vec![TireAxle::Single, TireAxle::Single],
        }],
        ..VehicleType::from_kind(label("Track car"), kind("car"))
    }
}

#[test]
fn a_type_starts_as_its_kind() {
    let plain = VehicleType::from_kind(label("Daily"), kind("car"));
    assert_eq!(
        plain.documents,
        [
            DocumentType::listed("document-registration").unwrap(),
            DocumentType::listed("document-insurance").unwrap()
        ]
    );
    assert_eq!(
        plain.fields_on,
        [key("trip-time")],
        "the car's fields that start on"
    );
    let fields = plain
        .new_fields(test_id(1), &[], Translator::english())
        .unwrap();
    let car = crate::profile::profile(&kind("car"))
        .unwrap()
        .new_fields(test_id(1), Translator::english());
    assert_eq!(
        fields, car,
        "a type left as its kind makes the kind's fields"
    );
}

#[test]
fn a_new_vehicle_of_a_type_gets_its_fields_and_documents() {
    let fields = track_car()
        .new_fields(test_id(1), &[], Translator::english())
        .unwrap();
    assert_eq!(
        fields.len(),
        2 + 8,
        "the car's two and four tires' pressure and tread"
    );
    assert!(
        fields.iter().take(2).all(|field| !field.enabled),
        "turned off"
    );
    assert_eq!(fields[2].key.as_str(), "tire-pressure-1l");
    let usage = track_car().default_usage(test_id(1)).unwrap();
    assert!(usage.default);
    assert_eq!(usage.name.as_str(), "Drivable", "the kind's usage name");
    assert_eq!(usage.documents, track_car().documents);
}

#[test]
fn a_set_already_on_from_the_kind_is_used() {
    let hybrid = VehicleType {
        catalogue: vec![CatalogueChoice::Set {
            id: "trip-computer-hybrid".into(),
        }],
        ..VehicleType::from_kind(label("Hybrid"), kind("car"))
    };
    let fields = hybrid
        .new_fields(test_id(1), &[], Translator::english())
        .unwrap();
    assert_eq!(fields.len(), 8, "the car's two, and six more");
    assert!(
        fields.iter().all(|field| field.enabled),
        "EV distance turned on"
    );
}

#[test]
fn types_are_checked() {
    let kept = track_car();
    let others = [(test_id(5), &kept)];
    assert_eq!(
        check_type(&kept, Some(test_id(5)), others),
        Ok(()),
        "itself"
    );
    let renamed = VehicleType {
        name: label("TRACK CAR"),
        ..track_car()
    };
    assert_eq!(
        check_type(&renamed, None, others),
        Err(TypeProblem::NameTaken)
    );
    let spaceship = VehicleType {
        kind: kind("spaceship"),
        ..track_car()
    };
    assert_eq!(
        check_type(&spaceship, None, []),
        Err(TypeProblem::UnknownKind)
    );
    let stranger = VehicleType {
        fields_on: vec![key("oil-life")],
        ..track_car()
    };
    assert_eq!(
        check_type(&stranger, None, []),
        Err(TypeProblem::UnknownField)
    );
    let trailer_trips = VehicleType {
        catalogue: vec![CatalogueChoice::Set {
            id: "trip-computer".into(),
        }],
        ..VehicleType::from_kind(label("Boat trailer"), kind("trailer"))
    };
    assert_eq!(
        check_type(&trailer_trips, None, []),
        Err(TypeProblem::BadCatalogue),
        "a trailer has no fuel-ups"
    );
    let lots = VehicleType {
        catalogue: vec![
            CatalogueChoice::Field {
                key: key("idle-time")
            };
            MAX_CATALOGUE_CHOICES + 1
        ],
        ..track_car()
    };
    assert_eq!(
        check_type(&lots, None, []),
        Err(TypeProblem::TooMuchCatalogue)
    );
}

#[test]
fn a_change_writes_only_what_differs_and_survives_the_trip() {
    let before = track_car();
    let after = VehicleType {
        icon: TypeIcon::Car,
        template: Some(test_id(9)),
        ..track_car()
    };
    let patch = VehicleTypePatch::changes(&before, after);
    assert_eq!(
        crate::command::Patch::changed_fields(&patch),
        ["icon", "template"]
    );
    let all = VehicleTypePatch::all(track_car());
    let json = serde_json::to_string(&all).unwrap();
    assert_eq!(
        serde_json::from_str::<VehicleTypePatch>(&json).unwrap(),
        all
    );
}
