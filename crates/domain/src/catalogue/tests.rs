use super::*;
use crate::calculated::check_calculated;
use crate::custom_field::check_field;
use crate::test_support::test_id;

const CAR: [EventKind; 4] = [
    EventKind::Fuel,
    EventKind::Reading,
    EventKind::Maintenance,
    EventKind::Repair,
];
const TRAILER: [EventKind; 3] = [
    EventKind::Reading,
    EventKind::Maintenance,
    EventKind::Repair,
];

fn english() -> &'static Translator {
    Translator::english()
}

fn key(text: &str) -> FieldKey {
    FieldKey::parse(text).unwrap()
}

fn set(id: &str) -> CatalogueChoice {
    CatalogueChoice::Set { id: id.into() }
}

fn add(choice: &CatalogueChoice, current: &[CustomField]) -> Result<Addition, CatalogueError> {
    let taken: Vec<FieldKey> = current.iter().map(|field| field.key.clone()).collect();
    fields_to_add(choice, test_id(1), &CAR, current, &taken, english())
}

fn keys(fields: &[CustomField]) -> Vec<&str> {
    fields.iter().map(|field| field.key.as_str()).collect()
}

fn labels(fields: &[CustomField]) -> Vec<&str> {
    fields.iter().map(|field| field.label.as_str()).collect()
}

#[test]
fn the_catalogue_loads_and_every_key_is_unique_and_valid() {
    let catalogue = catalogue().unwrap();
    let mut all: Vec<&str> = catalogue
        .fields
        .iter()
        .chain(&catalogue.tire_fields)
        .map(|field| field.key.as_str())
        .collect();
    let count = all.len();
    all.sort_unstable();
    all.dedup();
    assert_eq!(all.len(), count, "keys repeat");
    let mut sets: Vec<&str> = catalogue.sets.iter().map(|set| set.id.as_str()).collect();
    sets.sort_unstable();
    sets.dedup();
    assert_eq!(sets.len(), catalogue.sets.len(), "set IDs repeat");
    for field in catalogue.fields.iter().chain(&catalogue.tire_fields) {
        assert!(
            check_field(&field.kind, field.range).is_ok(),
            "{}",
            field.label
        );
        assert!(!field.shown_on.is_empty(), "{}", field.label);
        assert!(
            !crate::calculated::is_reserved(field.key.as_str()),
            "{}",
            field.label
        );
        assert!(english().has_key(&field.label), "{}", field.label);
    }
    for set in &catalogue.sets {
        assert!(english().has_key(&set.name), "{}", set.name);
        for key in &set.fields {
            assert!(
                catalogue.field(key).is_some(),
                "{}: {}",
                set.id,
                key.as_str()
            );
        }
    }
}

#[test]
fn every_set_makes_fields_whose_formulas_check() {
    let catalogue = catalogue().unwrap();
    for each in &catalogue.sets {
        let added = add(&set(&each.id), &[]).unwrap();
        assert_eq!(added.new.len(), each.fields.len(), "{}", each.id);
        let fields: Vec<_> = added
            .new
            .iter()
            .enumerate()
            .map(|(n, field)| (test_id(100 + u32::try_from(n).unwrap()), field.clone()))
            .collect();
        for (_, field) in &fields {
            assert!(
                check_calculated(field, &fields, &[]).is_ok(),
                "{}: {}",
                each.id,
                field.key.as_str()
            );
        }
    }
}

#[test]
fn a_set_copies_its_fields_on_and_again_with_b() {
    let first = add(&set("trip-computer"), &[]).unwrap();
    assert_eq!(
        keys(&first.new),
        [
            "trip-distance",
            "trip-time",
            "trip-average-speed",
            "trip-fuel-economy",
            "trip-fuel-used"
        ]
    );
    assert!(first.new.iter().all(|field| field.enabled));
    let again = add(&set("trip-computer"), &first.new).unwrap();
    assert_eq!(
        keys(&again.new),
        [
            "trip-distance-b",
            "trip-time-b",
            "trip-average-speed-b",
            "trip-fuel-economy-b",
            "trip-fuel-used-b"
        ]
    );
    assert_eq!(labels(&again.new)[0], "Trip distance B");
    let FieldKind::Calculated { formula, .. } = &again.new[2].kind else {
        panic!("average speed is calculated");
    };
    assert_eq!(formula.as_str(), "trip-distance-b ÷ trip-time-b");
    let mut both = first.new;
    both.extend(again.new);
    let third = add(&set("trip-computer"), &both).unwrap();
    assert_eq!(labels(&third.new)[4], "Trip fuel used C");
}

#[test]
fn a_set_uses_the_fields_the_vehicle_has_and_turns_them_on() {
    let car = crate::profile::profile(&crate::asset::AssetKind::parse("car").unwrap())
        .unwrap()
        .new_fields(test_id(1), english());
    // The car starts with trip time (on) and trip EV distance (off).
    let hybrid = add(&set("trip-computer-hybrid"), &car).unwrap();
    assert_eq!(hybrid.new.len(), 6, "{:?}", keys(&hybrid.new));
    assert!(!keys(&hybrid.new).contains(&"trip-time"));
    assert_eq!(hybrid.turn_on, [key("trip-ev-distance")]);
}

#[test]
fn a_removed_fields_key_is_never_reused() {
    let taken = [key("trip-distance")];
    let added = fields_to_add(
        &CatalogueChoice::Field {
            key: key("trip-distance"),
        },
        test_id(1),
        &CAR,
        &[],
        &taken,
        english(),
    )
    .unwrap();
    assert_eq!(keys(&added.new), ["trip-distance-2"]);
}

#[test]
fn a_single_field_already_there_is_refused() {
    let added = add(
        &CatalogueChoice::Field {
            key: key("idle-time"),
        },
        &[],
    )
    .unwrap();
    assert_eq!(
        add(
            &CatalogueChoice::Field {
                key: key("idle-time")
            },
            &added.new
        ),
        Err(CatalogueError::AlreadyThere)
    );
}

#[test]
fn fields_follow_the_vehicles_kinds_of_events() {
    let trailer =
        |choice: &CatalogueChoice| fields_to_add(choice, test_id(1), &TRAILER, &[], &[], english());
    assert_eq!(
        trailer(&set("trip-computer")),
        Err(CatalogueError::NotForThisKind)
    );
    let engine = trailer(&set("engine-idle")).unwrap();
    assert!(
        engine
            .new
            .iter()
            .all(|field| field.shown_on == [EventKind::Reading])
    );
}

#[test]
fn tires_are_named_by_axle_for_one_to_four_axles() {
    let names = |axles: &[TireAxle]| -> Vec<String> {
        tire_positions(axles)
            .iter()
            .map(|tire| tire.name(english()))
            .collect()
    };
    assert_eq!(names(&[TireAxle::Single]), ["1L", "1R"]);
    assert_eq!(
        names(&[TireAxle::Single, TireAxle::Dual]),
        ["1L", "1R", "2L outer", "2L inner", "2R inner", "2R outer"]
    );
    assert_eq!(names(&[TireAxle::Single; 3]).len(), 6);
    let truck = [
        TireAxle::Single,
        TireAxle::Dual,
        TireAxle::Dual,
        TireAxle::Dual,
    ];
    assert_eq!(names(&truck).len(), 14);
    assert_eq!(names(&truck)[13], "4R outer");
    let added = add(
        &CatalogueChoice::Tires {
            axles: truck.to_vec(),
        },
        &[],
    )
    .unwrap();
    assert_eq!(added.new.len(), 28, "a pressure and a tread depth each");
    assert_eq!(
        labels(&added.new)[..2],
        ["Tire pressure 1L", "Tread depth 1L"]
    );
    assert_eq!(keys(&added.new)[4], "tire-pressure-2l-outer");
    assert_eq!(added.new[0].kind, FieldKind::Pressure);
    assert_eq!(added.new[1].kind, FieldKind::TreadDepth);
    assert_eq!(
        added.new[0].shown_on,
        [EventKind::Reading, EventKind::Maintenance]
    );
    for axles in [vec![], vec![TireAxle::Single; 5]] {
        assert_eq!(
            add(&CatalogueChoice::Tires { axles }, &[]),
            Err(CatalogueError::BadAxles)
        );
    }
}

#[test]
fn tires_again_get_b() {
    let axles = CatalogueChoice::Tires {
        axles: vec![TireAxle::Single, TireAxle::Single],
    };
    let first = add(&axles, &[]).unwrap();
    let again = add(&axles, &first.new).unwrap();
    assert_eq!(labels(&again.new)[0], "Tire pressure 1L B");
    assert_eq!(keys(&again.new)[0], "tire-pressure-1l-b");
}

#[test]
fn a_choice_survives_the_trip() {
    for choice in [
        set("engine-idle"),
        CatalogueChoice::Field {
            key: key("oil-life"),
        },
        CatalogueChoice::Tires {
            axles: vec![TireAxle::Single, TireAxle::Dual],
        },
    ] {
        let json = serde_json::to_string(&choice).unwrap();
        assert_eq!(
            serde_json::from_str::<CatalogueChoice>(&json).unwrap(),
            choice
        );
    }
}
