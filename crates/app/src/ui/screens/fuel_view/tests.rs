use domain::fuel::{Fill, summarize};
use domain::preferences::{DistanceUnit, EconomyUnit};
use domain::test_support::test_id;
use domain::time::UnixMillis;

use super::*;

const DAY: i64 = 24 * 60 * 60 * 1_000;

/// Fill `n` on day `n`: at `miles`, `gallons` bought, full or not.
fn fill(n: u32, miles: i64, gallons: i64, full: bool) -> Fill {
    Fill {
        event: test_id(n),
        at: UnixMillis::from_millis(i64::from(n) * DAY),
        reading: Some(miles * 1_609_344 / 1_000),
        volume_ml: gallons * 3_785_412 / 1_000,
        filled_full: full,
        missed_previous: false,
        highway_pct: 50,
        towing_pct: 0,
        hauling_pct: 0,
        towed: vec![],
        reported: None,
    }
}

fn miles() -> Preferences {
    Preferences {
        distance: DistanceUnit::Miles,
        economy: EconomyUnit::MpgUs,
        ..Preferences::default()
    }
}

/// Full at 1,000 mi; a 5-gallon partial; full at 1,300 mi with 10 more (20 mpg, the car said
/// 21); a missed fill-up at 1,600; a partial waiting.
fn view() -> FuelView {
    let mut ending = fill(3, 1_300, 10, true);
    ending.reported = Some(8_928);
    let mut missed = fill(4, 1_600, 12, true);
    missed.missed_previous = true;
    let fills = [
        fill(1, 1_000, 14, true),
        fill(2, 1_150, 5, false),
        ending,
        missed,
        fill(5, 1_700, 4, false),
    ];
    FuelView {
        summary: summarize(&fills, UnixMillis::from_millis(10 * DAY)),
        towed_names: vec![],
        last_fill: None,
    }
}

#[test]
fn each_fill_up_says_how_it_counted() {
    let view = view();
    let lines: Vec<String> = (1..=5)
        .map(|n| fill_line(test_id(n), &view, &miles()).unwrap())
        .collect();
    assert_eq!(lines[0], "Economy starts here");
    assert_eq!(
        lines[1],
        "Partial: added into the next full fill-up's figure"
    );
    assert!(
        lines[2].starts_with("20.0 mpg over 300.0 mi"),
        "{}",
        lines[2]
    );
    assert!(lines[2].ends_with("car said 21.0"), "{}", lines[2]);
    assert_eq!(lines[3], "Not counted: an earlier fill-up wasn't logged");
    assert_eq!(lines[4], "Partial: counts in the next full fill-up");
    assert_eq!(fill_line(test_id(99), &view, &miles()), None);
}

#[test]
fn the_overview_shows_the_latest_and_the_last_five() {
    let line = overview_line(&view(), &miles()).unwrap();
    assert!(line.starts_with("20.0 mpg latest · last 5: 20.0"), "{line}");
    assert_eq!(overview_line(&FuelView::default(), &miles()), None);
}

#[test]
fn the_line_breaks_at_each_dropped_stretch() {
    let points = [
        Some((1.0, 1.0)),
        Some((2.0, 2.0)),
        None,
        Some((4.0, 4.0)),
        None,
        Some((6.0, 6.0)),
        Some((7.0, 7.0)),
    ];
    assert_eq!(
        runs(&points),
        vec![vec![(1.0, 1.0), (2.0, 2.0)], vec![(6.0, 6.0), (7.0, 7.0)]],
        "a lone point is a dot, not a line"
    );
}

#[test]
fn the_scale_leaves_room_and_never_goes_below_zero() {
    let (low, high) = y_range(&[20.0, 25.0]);
    assert!(low < 20.0 && high > 25.0);
    assert!(y_range(&[0.5]).0.abs() < f64::EPSILON, "never below zero");
    let (empty_low, empty_high) = y_range(&[]);
    assert!(empty_low.abs() < f64::EPSILON && (empty_high - 1.0).abs() < f64::EPSILON);
    assert!(
        y_at(low, (low, high)) > y_at(high, (low, high)),
        "up is more"
    );
}
