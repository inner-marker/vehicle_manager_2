use proptest::prelude::*;

use super::*;
use crate::label::Label;
use crate::meter::{MeterDirection, MeterUnit};
use crate::test_support::test_id;

const DAY: i64 = 24 * 60 * 60 * 1_000;
const CAMPER: u32 = 70;

/// Fill `n` on day `n`: at `km` on the odometer, `liters` of fuel, full or not.
fn fill(n: u32, km: i64, liters: i64, full: bool) -> Fill {
    Fill {
        event: test_id(n),
        at: UnixMillis::from_millis(i64::from(n) * DAY),
        reading: Some(km * 1_000),
        volume_ml: liters * 1_000,
        filled_full: full,
        missed_previous: false,
        highway_pct: 50,
        towing_pct: 0,
        hauling_pct: 0,
        towed: vec![],
        reported: None,
    }
}

fn now() -> UnixMillis {
    UnixMillis::from_millis(400 * DAY)
}

#[test]
fn the_fuel_odometer_is_the_first_distance_meter_fuel_ups_need() {
    let meter = |name: &str, dimension, required: Vec<EventKind>| Meter {
        asset: test_id(1),
        name: Label::parse(name).unwrap(),
        dimension,
        unit: MeterUnit::Kilometers,
        precision: 0,
        description: None,
        required_for: required,
        direction: MeterDirection::Up,
        reset_to: None,
    };
    let meters = vec![
        (test_id(10), meter("Trip", MeterDimension::Distance, vec![])),
        (
            test_id(11),
            meter("Hours", MeterDimension::Duration, vec![EventKind::Fuel]),
        ),
        (
            test_id(12),
            meter("Odometer", MeterDimension::Distance, vec![EventKind::Fuel]),
        ),
    ];
    assert_eq!(fuel_meter(&meters), Some(test_id(12)));
    assert_eq!(fuel_meter(&meters[..2]), None);
}

#[test]
fn partial_fills_add_into_the_next_full_fill() {
    let fills = [
        fill(1, 900, 10, false),
        fill(2, 1_000, 40, true),
        fill(3, 1_200, 20, false),
        fill(4, 1_500, 30, true),
        fill(5, 1_600, 15, false),
    ];
    let summary = summarize(&fills, now());
    assert_eq!(summary.stretches.len(), 1);
    let stretch = &summary.stretches[0];
    assert_eq!((stretch.distance_m, stretch.fuel_ml), (500_000, 50_000));
    assert_eq!(stretch.economy(), Some(10_000), "10 km/L");
    assert_eq!(
        summary.parts,
        vec![
            (test_id(1), FillPart::BeforeFirstFull),
            (test_id(2), FillPart::Starts),
            (test_id(3), FillPart::Partial { stretch: Some(0) }),
            (test_id(4), FillPart::Ends { stretch: 0 }),
            (test_id(5), FillPart::Partial { stretch: None }),
        ]
    );
}

#[test]
fn order_comes_from_time_not_the_list() {
    let mut fills = vec![
        fill(2, 1_000, 40, true),
        fill(4, 1_500, 30, true),
        fill(3, 1_200, 20, false),
    ];
    fills.reverse();
    assert_eq!(
        summarize(&fills, now()).stretches[0].economy(),
        Some(10_000)
    );
}

#[test]
fn a_missed_fill_up_or_missing_reading_drops_the_stretch() {
    let mut missed = fill(3, 1_200, 20, false);
    missed.missed_previous = true;
    let mut unread = fill(6, 2_000, 30, true);
    unread.reading = None;
    let fills = [
        fill(2, 1_000, 40, true),
        missed,
        fill(4, 1_500, 30, true),
        fill(5, 1_800, 30, true),
        unread,
        fill(7, 2_300, 30, true),
        // The same reading again: no distance.
        fill(8, 2_300, 5, true),
    ];
    let summary = summarize(&fills, now());
    let dropped: Vec<Option<DropReason>> = summary
        .stretches
        .iter()
        .map(|stretch| stretch.dropped)
        .collect();
    assert_eq!(
        dropped,
        vec![
            Some(DropReason::MissedFillUp),
            None,
            Some(DropReason::MissingReading),
            Some(DropReason::MissingReading),
            Some(DropReason::NoDistance),
        ]
    );
    assert_eq!(summary.stretches[0].economy(), None);
    // Only 1,500 → 1,800 km on 30 L counts.
    let all = summary.averages.all.unwrap();
    assert_eq!((all.stretches, all.economy), (1, 10_000));
}

#[test]
fn averages_are_totals_over_totals_for_their_spans() {
    // Stretch k: 100 km on (10 + k) L, ending on day 50·k + 10; the first on day 60.
    let mut fills = vec![Fill {
        at: UnixMillis::from_millis(0),
        ..fill(100, 0, 40, true)
    }];
    for k in 1..=7_u32 {
        fills.push(Fill {
            at: UnixMillis::from_millis(i64::from(k) * 50 * DAY + 10 * DAY),
            ..fill(100 + k, i64::from(k) * 100, 10 + i64::from(k), true)
        });
    }
    // Now is day 430, so the year runs from day 65.
    let summary = summarize(&fills, UnixMillis::from_millis(430 * DAY));
    let all = summary.averages.all.unwrap();
    assert_eq!(all.stretches, 7);
    assert_eq!(all.distance_m, 700_000);
    assert_eq!(all.fuel_ml, (11..=17).sum::<i64>() * 1_000);
    assert_eq!(all.economy, per_liter(700_000, 98_000));
    let last = summary.averages.last.unwrap();
    assert_eq!(last.stretches, LAST_STRETCHES);
    assert_eq!(
        last.fuel_ml,
        (13..=17).sum::<i64>() * 1_000,
        "the latest five"
    );
    // The first stretch ended on day 60, more than a year before day 430.
    let year = summary.averages.year.unwrap();
    assert_eq!(year.stretches, 6);
}

#[test]
fn no_fills_or_one_full_fill_give_no_figures() {
    assert_eq!(summarize(&[], now()), FuelSummary::default());
    let summary = summarize(&[fill(1, 1_000, 40, true)], now());
    assert!(summary.stretches.is_empty());
    assert_eq!(summary.averages, Averages::default());
    assert_eq!(summary.parts, vec![(test_id(1), FillPart::Starts)]);
}

#[test]
fn shares_are_weighted_by_fuel_into_bands() {
    // 1,000 → 1,400 km: 10 L at 100% highway, 30 L at 0% (25% overall, city).
    let mut highway = fill(3, 1_100, 10, false);
    highway.highway_pct = 100;
    let mut city = fill(4, 1_400, 30, true);
    city.highway_pct = 0;
    // 1,400 → 2,000 km towing the camper part of the way (40%, "some").
    let mut towing = fill(5, 2_000, 60, true);
    towing.highway_pct = 80;
    towing.towing_pct = 40;
    towing.towed = vec![test_id(CAMPER)];
    let fills = [fill(2, 1_000, 40, true), highway, city, towing];
    let summary = summarize(&fills, now());
    assert_eq!(summary.stretches[0].highway_pct, 25);
    assert_eq!(summary.stretches[1].towing_pct, 40);
    let bands = &summary.breakdowns;
    assert_eq!(bands.highway[0].map(|band| band.stretches), Some(1), "city");
    assert_eq!(bands.highway[1], None, "none mixed");
    assert_eq!(
        bands.highway[2].map(|band| band.stretches),
        Some(1),
        "highway"
    );
    assert_eq!(
        bands.towing[0].map(|band| band.stretches),
        Some(1),
        "no towing"
    );
    assert_eq!(bands.towing[1].map(|band| band.stretches), Some(1), "some");
    assert_eq!(bands.towing[2], None);
    let towed: Vec<Option<AssetId>> = bands.towed.iter().map(|(vehicle, _)| *vehicle).collect();
    assert_eq!(towed, vec![None, Some(test_id(CAMPER))]);
}

#[test]
fn bands_have_their_edges() {
    assert_eq!(
        [0, 30, 31, 69, 70, 100].map(Band::highway),
        [
            Band::Low,
            Band::Low,
            Band::Middle,
            Band::Middle,
            Band::High,
            Band::High
        ]
    );
    assert_eq!(
        [0, 1, 49, 50, 100].map(Band::load),
        [
            Band::Low,
            Band::Middle,
            Band::Middle,
            Band::High,
            Band::High
        ]
    );
}

#[test]
fn the_cars_figure_is_the_ending_fills() {
    let mut ending = fill(3, 1_500, 50, true);
    ending.reported = Some(10_500);
    let summary = summarize(&[fill(2, 1_000, 40, true), ending], now());
    assert_eq!(summary.stretches[0].reported, Some(10_500));
}

#[test]
fn a_fill_from_a_fuel_up_reads_its_odometer() {
    use crate::dates::EventDates;
    use crate::event::{EventLocation, EventSource, EventStatus, FuelDetails, Reading};
    use crate::location::{LocationSource, TimeZone};
    use crate::notes::Notes;
    use crate::preferences::CurrencyCode;
    let event = Event {
        asset: test_id(1),
        kind: EventKind::Fuel,
        dates: EventDates {
            start: UnixMillis::from_millis(DAY),
            end: None,
            zone: TimeZone::parse("UTC").unwrap(),
        },
        location: EventLocation {
            place: None,
            coordinates: None,
            accuracy_m: None,
            fix_time: None,
            source: LocationSource::Manual,
            place_name: None,
            saved_place: None,
            geocoder_version: None,
            missing_reason: None,
        },
        readings: vec![Reading {
            meter: test_id(12),
            value: 1_000_000,
            reset: false,
        }],
        skipped: vec![],
        fields: vec![],
        notes: Notes::default(),
        source: EventSource::Typed,
        logged_by: test_id(5),
        performed_by: None,
        fuel: Some(FuelDetails {
            volume_ml: 40_000,
            cost_minor: None,
            currency: CurrencyCode::USD,
            filled_full: true,
            missed_previous: false,
            highway_pct: 60,
            towing_pct: 0,
            hauling_pct: 0,
            towed: vec![],
            reported_economy: Some(9_000),
        }),
        work: None,
        lifecycle: None,
        reading_override: None,
        status: EventStatus::Finalized,
        last_edited_at: UnixMillis::from_millis(DAY),
        received_at: UnixMillis::from_millis(DAY),
        finalized: None,
        corrects: None,
        correction_reason: None,
    };
    let made = Fill::of(test_id(50), &event, Some(test_id(12))).unwrap();
    assert_eq!(made.reading, Some(1_000_000));
    assert_eq!(made.reported, Some(9_000));
    assert_eq!(Fill::of(test_id(50), &event, None).unwrap().reading, None);
    let reading = Event {
        fuel: None,
        ..event
    };
    assert_eq!(Fill::of(test_id(51), &reading, Some(test_id(12))), None);
}

proptest! {
    /// A dropped stretch never moves an average: dropping it by a missed fill-up gives the
    /// same figure as leaving it out of the counted ones.
    #[test]
    fn a_dropped_stretch_never_counts(
        legs in proptest::collection::vec((1_i64..2_000, 1_i64..200, any::<bool>()), 1..12),
    ) {
        let mut fills = vec![fill(1, 0, 40, true)];
        let mut km = 0;
        for (index, (distance, liters, missed)) in legs.iter().enumerate() {
            km += distance;
            let mut next = fill(u32::try_from(index).unwrap() + 2, km, *liters, true);
            next.missed_previous = *missed;
            fills.push(next);
        }
        let summary = summarize(&fills, now());
        let (count, distance, fuel) = legs
            .iter()
            .filter(|(_, _, missed)| !missed)
            .fold((0, 0, 0), |(count, distance, fuel), (d, l, _)| {
                (count + 1, distance + d * 1_000, fuel + l * 1_000)
            });
        match summary.averages.all {
            None => prop_assert_eq!(count, 0),
            Some(all) => {
                prop_assert_eq!(all.stretches, count);
                prop_assert_eq!((all.distance_m, all.fuel_ml), (distance, fuel));
            }
        }
    }
}
