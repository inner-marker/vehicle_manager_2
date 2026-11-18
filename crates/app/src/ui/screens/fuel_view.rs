//! Fuel economy on screen (Phase 5 milestone 5; decision #085; sketch
//! `docs/ui-sketches/p5m5-fuel-options.svg`, 1A, 2A, 3A, 4A, 5A).
//!
//! - **1A, the tiles:** the last 5 stretches, the last 12 months, and all time, each with how
//!   many stretches it covers.
//! - **2A, the chart:** drawn here as SVG (no chart library, #080). A dot per stretch joined
//!   by a 2px line in the accent colour (one series, so no legend), a gap and a shaded slot for
//!   each dropped stretch, the all-time average as a hairline, only the latest labelled.
//!   Hovering a dot shows its figure; tapping one shows its details under the chart. A table
//!   view lists every stretch.
//! - **3A, the breakdowns:** small bars from zero, a group each (highway, towing, hauling,
//!   towed vehicles), with each band's figure and how many stretches.
//! - **4A:** each fuel-up in the Fuel tab's list says how it counted ([`fill_line`]).
//! - **5A:** the Overview's Fuel panel line ([`overview_line`]).
//!
//! Figures are in each person's economy unit; a lower figure is worse in mpg and km/L, better
//! in L/100 km, and the chart simply plots the person's unit.

use dioxus::prelude::*;
use domain::api::{FuelOutcome, FuelView};
use domain::asset::AssetId;
use domain::event::EventId;
use domain::fuel::{Average, DropReason, FillPart, Stretch};
use domain::i18n::{MessageArg, MessageId, messages};
use domain::location::TimeZone;
use domain::preferences::Preferences;
use domain::quantity::{economy_value, show_distance, show_economy, show_volume};

use crate::data;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

/// Loads a vehicle's fuel economy.
pub(super) fn use_fuel(asset: AssetId) -> Resource<Option<FuelView>> {
    use_fuel_if(asset, true)
}

/// Loads a vehicle's fuel economy if `wanted` (the hook always runs; nothing is fetched
/// otherwise).
pub(super) fn use_fuel_if(asset: AssetId, wanted: bool) -> Resource<Option<FuelView>> {
    use_resource(move || async move {
        if !wanted {
            return None;
        }
        match data::vehicle_fuel(asset).await {
            Ok(FuelOutcome::Fuel { view }) => Some(*view),
            _ => None,
        }
    })
}

/// A count as a Fluent number.
fn count(value: usize) -> MessageArg {
    MessageArg::from(i64::try_from(value).unwrap_or(i64::MAX))
}

/// A figure in the person's unit as a number, for the chart's scale.
fn plotted(meters_per_liter: i64, units: &Preferences) -> f64 {
    economy_value(meters_per_liter, units.economy)
        .replace(',', "")
        .parse()
        .unwrap_or(0.0)
}

/// A local date, as the person writes dates.
fn day(at: domain::time::UnixMillis, zone: &TimeZone, units: &Preferences) -> String {
    domain::dates::utc_to_local(at, zone)
        .ok()
        .and_then(|(date, _)| domain::dates::CalendarDate::parse(&date).ok())
        .map(|date| super::documents::date_text(&date, units))
        .unwrap_or_default()
}

/// Why a stretch doesn't count, in words.
const fn drop_reason(reason: DropReason) -> MessageId {
    match reason {
        DropReason::MissedFillUp => messages::FUEL_DROPPED_MISSED,
        DropReason::MissingReading => messages::FUEL_DROPPED_READING,
        DropReason::NoDistance => messages::FUEL_DROPPED_DISTANCE,
    }
}

/// A stretch in one line: "24.9 mpg over 309 mi · car said 25.4", or why it doesn't count.
pub(super) fn stretch_line(stretch: &Stretch, units: &Preferences) -> String {
    let Some(economy) = stretch.economy() else {
        return tr(drop_reason(
            stretch.dropped.unwrap_or(DropReason::NoDistance),
        ));
    };
    let mut line = tr_with(
        messages::FUEL_STRETCH,
        &[
            (
                "economy",
                MessageArg::from(show_economy(economy, units.economy).as_str()),
            ),
            (
                "distance",
                MessageArg::from(show_distance(stretch.distance_m, units.distance).as_str()),
            ),
        ],
    );
    if let Some(reported) = stretch.reported {
        line.push_str(" · ");
        line.push_str(&tr_with(
            messages::FUEL_CAR_SAID,
            &[(
                "economy",
                MessageArg::from(economy_value(reported, units.economy).as_str()),
            )],
        ));
    }
    line
}

/// How fuel-up `event` counted (4A), for the Fuel tab's list.
pub(super) fn fill_line(event: EventId, view: &FuelView, units: &Preferences) -> Option<String> {
    let (_, part) = view.summary.parts.iter().find(|(id, _)| *id == event)?;
    Some(match part {
        FillPart::BeforeFirstFull => tr(messages::FUEL_PART_BEFORE),
        FillPart::Starts => tr(messages::FUEL_PART_STARTS),
        FillPart::Partial { stretch: None } => tr(messages::FUEL_PART_WAITING),
        FillPart::Partial {
            stretch: Some(index),
        } => {
            let stretch = view.summary.stretches.get(*index)?;
            match stretch.dropped {
                Some(reason) => tr(drop_reason(reason)),
                None => tr(messages::FUEL_PART_ADDED),
            }
        }
        FillPart::Ends { stretch } => stretch_line(view.summary.stretches.get(*stretch)?, units),
    })
}

/// The Overview's line (5A): "24.9 mpg latest · last 5: 24.6", or nothing yet.
pub(super) fn overview_line(view: &FuelView, units: &Preferences) -> Option<String> {
    let latest = view
        .summary
        .stretches
        .iter()
        .rev()
        .find_map(Stretch::economy)?;
    let last = view
        .summary
        .averages
        .last
        .map_or_else(String::new, |average| {
            economy_value(average.economy, units.economy)
        });
    Some(tr_with(
        messages::FUEL_OVERVIEW,
        &[
            (
                "latest",
                MessageArg::from(show_economy(latest, units.economy).as_str()),
            ),
            ("last", MessageArg::from(last.as_str())),
        ],
    ))
}

/// The top of the Fuel tab: the tiles, the chart (or its table), and the breakdowns.
#[component]
pub(super) fn FuelEconomy(asset: AssetId) -> Element {
    let units = use_units();
    let loaded = use_fuel(asset);
    let zone = super::squawks::use_device_zone();
    let Some(Some(view)) = loaded() else {
        return rsx! {};
    };
    if view.summary.stretches.is_empty() {
        return rsx! { p { class: "muted-note", {tr(messages::FUEL_NONE_YET)} } };
    }
    rsx! {
        section { class: "fuel-economy",
            Tiles { averages: view.summary.averages, units: units() }
            StretchChart { stretches: view.summary.stretches.clone(), all: view.summary.averages.all, units: units(), zone: zone() }
            Breakdowns { view, units: units() }
        }
    }
}

/// 1A: three tiles.
#[component]
fn Tiles(averages: domain::fuel::Averages, units: Preferences) -> Element {
    let tiles = [
        (messages::FUEL_LAST_FIVE, averages.last),
        (messages::FUEL_YEAR, averages.year),
        (messages::FUEL_ALL_TIME, averages.all),
    ];
    rsx! {
        div { class: "fuel-tiles",
            for (label, average) in tiles {
                div { key: "{label.as_str()}", class: "fuel-tile",
                    span { class: "fuel-tile-label", {tr(label)} }
                    span { class: "fuel-tile-value",
                        {average.map_or_else(|| "–".to_owned(), |average| show_economy(average.economy, units.economy))}
                    }
                    span { class: "fuel-tile-sub",
                        {tr_with(messages::FUEL_STRETCHES, &[("count", count(average.map_or(0, |average| average.stretches)))])}
                    }
                }
            }
        }
    }
}

/// The chart's size, in its own units (it scales to the width it's given).
const WIDTH: f64 = 600.0;
const HEIGHT: f64 = 200.0;
const LEFT: f64 = 40.0;
const TOP: f64 = 16.0;
const BOTTOM: f64 = 24.0;

/// Where stretch `index` of `total` sits across the plot.
fn x_at(index: usize, total: usize) -> f64 {
    let span = WIDTH - LEFT - 16.0;
    let steps = f64::from(u32::try_from(total.saturating_sub(1).max(1)).unwrap_or(u32::MAX));
    LEFT + 8.0 + span * f64::from(u32::try_from(index).unwrap_or(u32::MAX)) / steps
}

/// The plot's range (with a little room), from the counted figures.
fn y_range(values: &[f64]) -> (f64, f64) {
    let low = values.iter().copied().fold(f64::INFINITY, f64::min);
    let high = values.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    if !low.is_finite() || !high.is_finite() {
        return (0.0, 1.0);
    }
    let pad = ((high - low) * 0.15).max(1.0);
    ((low - pad).max(0.0).floor(), (high + pad).ceil())
}

/// Where `value` sits up the plot.
fn y_at(value: f64, (low, high): (f64, f64)) -> f64 {
    let plot = HEIGHT - TOP - BOTTOM;
    TOP + plot - (value - low) / (high - low).max(f64::EPSILON) * plot
}

/// The line's pieces: runs of counted stretches, broken at each dropped one.
pub(super) fn runs(points: &[Option<(f64, f64)>]) -> Vec<Vec<(f64, f64)>> {
    let mut runs = vec![Vec::new()];
    for point in points {
        match point {
            Some(point) => runs
                .last_mut()
                .map_or((), |run: &mut Vec<_>| run.push(*point)),
            None => runs.push(Vec::new()),
        }
    }
    runs.into_iter().filter(|run| run.len() > 1).collect()
}

/// 2A: the chart, a stretch's details when one is tapped, and the table view.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn StretchChart(
    stretches: Vec<Stretch>,
    all: Option<Average>,
    units: Preferences,
    zone: TimeZone,
) -> Element {
    let mut table = use_signal(|| false);
    let mut chosen = use_signal(|| None::<usize>);
    let values: Vec<Option<f64>> = stretches
        .iter()
        .map(|stretch| stretch.economy().map(|economy| plotted(economy, &units)))
        .collect();
    let range = y_range(&values.iter().flatten().copied().collect::<Vec<_>>());
    let total = stretches.len();
    let points: Vec<Option<(f64, f64)>> = values
        .iter()
        .enumerate()
        .map(|(index, value)| value.map(|value| (x_at(index, total), y_at(value, range))))
        .collect();
    let detail = chosen()
        .and_then(|index| stretches.get(index))
        .map(|stretch| {
            format!(
                "{} · {}",
                day(stretch.ended_at, &zone, &units),
                stretch_line(stretch, &units)
            )
        });
    rsx! {
        div { class: "fuel-chart-head",
            h3 { class: "section-heading", {tr(messages::FUEL_CHART).to_uppercase()} }
            button { class: "link-button", r#type: "button", onclick: move |_| table.set(!table()),
                {tr(if table() { messages::FUEL_SHOW_CHART } else { messages::FUEL_SHOW_TABLE })}
            }
        }
        if table() {
            StretchTable { stretches, units, zone }
        } else {
            ChartSvg { points, values, range, all: all.map(|average| plotted(average.economy, &units)), units, on_pick: move |index| chosen.set(Some(index)) }
            if let Some(text) = detail {
                p { class: "fuel-chart-detail", role: "status", {text} }
            } else {
                p { class: "muted-note", {tr(messages::FUEL_CHART_HINT)} }
            }
        }
    }
}

/// The SVG itself: gridlines, gaps, the average, the line, and the dots.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ChartSvg(
    points: Vec<Option<(f64, f64)>>,
    values: Vec<Option<f64>>,
    range: (f64, f64),
    all: Option<f64>,
    units: Preferences,
    on_pick: EventHandler<usize>,
) -> Element {
    let (low, high) = range;
    let step = ((high - low) / 4.0).max(1.0).round();
    let mut ticks = Vec::new();
    let mut tick = low;
    while tick <= high + f64::EPSILON {
        ticks.push(tick);
        tick += step;
    }
    let total = points.len();
    let line_runs = runs(&points);
    let latest = points
        .iter()
        .enumerate()
        .rev()
        .find_map(|(index, point)| point.map(|point| (index, point)));
    rsx! {
        svg {
            class: "fuel-chart",
            view_box: "0 0 {WIDTH} {HEIGHT}",
            role: "img",
            "aria-label": tr(messages::FUEL_CHART),
            for value in ticks {
                g { key: "t{value}",
                    line { class: "chart-grid", x1: "{LEFT}", x2: "{WIDTH}", y1: "{y_at(value, range)}", y2: "{y_at(value, range)}" }
                    text { class: "chart-axis", x: "{LEFT - 6.0}", y: "{y_at(value, range) + 4.0}", text_anchor: "end", "{value}" }
                }
            }
            for (index, point) in points.iter().enumerate() {
                if point.is_none() {
                    rect { key: "g{index}", class: "chart-gap", x: "{x_at(index, total) - 6.0}", y: "{TOP}", width: "12", height: "{HEIGHT - TOP - BOTTOM}" }
                }
            }
            if let Some(average) = all {
                line { class: "chart-average", x1: "{LEFT}", x2: "{WIDTH}", y1: "{y_at(average, range)}", y2: "{y_at(average, range)}" }
                text { class: "chart-axis", x: "{LEFT + 6.0}", y: "{y_at(average, range) - 5.0}", {tr_with(messages::FUEL_AVERAGE_LINE, &[("value", MessageArg::from(format!("{average:.1}").as_str()))])} }
            }
            for (number, run) in line_runs.iter().enumerate() {
                polyline { key: "r{number}", class: "chart-line", points: run.iter().map(|(x, y)| format!("{x},{y}")).collect::<Vec<_>>().join(" ") }
            }
            for (index, point) in points.iter().enumerate() {
                if let Some((x, y)) = point {
                    circle { key: "d{index}", class: "chart-dot", cx: "{x}", cy: "{y}", r: "5", onclick: move |_| on_pick.call(index),
                        title { {values.get(index).copied().flatten().map(|value| format!("{value:.1} {}", crate::ui::units::economy_abbr(units.economy))).unwrap_or_default()} }
                    }
                }
            }
            if let Some((index, (x, y))) = latest {
                text { class: "chart-label", x: "{x}", y: "{y - 10.0}", text_anchor: "middle",
                    {values.get(index).copied().flatten().map(|value| format!("{value:.1}")).unwrap_or_default()}
                }
            }
        }
    }
}

/// The chart as a table: every stretch, newest first.
#[component]
fn StretchTable(stretches: Vec<Stretch>, units: Preferences, zone: TimeZone) -> Element {
    rsx! {
        table { class: "fuel-table",
            thead {
                tr {
                    th { {tr(messages::FUEL_TABLE_ENDED)} }
                    th { {tr(messages::FUEL_TABLE_FUEL)} }
                    th { {tr(messages::FUEL_TABLE_FIGURE)} }
                }
            }
            tbody {
                for (index, stretch) in stretches.iter().enumerate().rev() {
                    tr { key: "{index}",
                        td { {day(stretch.ended_at, &zone, &units)} }
                        td { {show_volume(stretch.fuel_ml, units.volume)} }
                        td { {stretch_line(stretch, &units)} }
                    }
                }
            }
        }
    }
}

/// 3A: each condition's bands, and the towed vehicles, as small bars from zero.
#[component]
fn Breakdowns(view: FuelView, units: Preferences) -> Element {
    let bands = &view.summary.breakdowns;
    let groups: Vec<(MessageId, Vec<BandRow>)> = vec![
        (
            messages::FUEL_HIGHWAY,
            labelled(
                &[
                    messages::FUEL_BAND_CITY,
                    messages::FUEL_BAND_MIXED,
                    messages::FUEL_BAND_HIGHWAY,
                ],
                &bands.highway,
            ),
        ),
        (
            messages::FUEL_TOWING,
            labelled(
                &[
                    messages::FUEL_BAND_NONE,
                    messages::FUEL_BAND_SOME,
                    messages::FUEL_BAND_MOSTLY,
                ],
                &bands.towing,
            ),
        ),
        (
            messages::FUEL_HAULING,
            labelled(
                &[
                    messages::FUEL_BAND_NONE,
                    messages::FUEL_BAND_SOME,
                    messages::FUEL_BAND_MOSTLY,
                ],
                &bands.hauling,
            ),
        ),
        (messages::FUEL_TOWED, towed_rows(&view)),
    ];
    let longest = groups
        .iter()
        .flat_map(|(_, rows)| rows.iter().filter_map(|(_, average)| *average))
        .map(|average| plotted(average.economy, &units))
        .fold(0.0_f64, f64::max);
    rsx! {
        div { class: "fuel-breakdowns",
            for (heading, rows) in groups {
                if rows.iter().any(|(_, average)| average.is_some()) {
                    div { key: "{heading.as_str()}", class: "fuel-group",
                        h4 { class: "section-heading", {tr(heading).to_uppercase()} }
                        for (label, average) in rows {
                            div { key: "{label}", class: "fuel-band",
                                span { class: "fuel-band-label", {label} }
                                BandBar { average, longest, units }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// A band's name and average (`None`: no stretches in it).
type BandRow = (String, Option<Average>);

/// One band's bar and figure ("not enough yet" without one).
#[component]
fn BandBar(average: Option<Average>, longest: f64, units: Preferences) -> Element {
    let Some(average) = average else {
        return rsx! { span { class: "muted-note", {tr(messages::FUEL_NOT_ENOUGH)} } };
    };
    let share = plotted(average.economy, &units) / longest.max(f64::EPSILON) * 100.0;
    rsx! {
        span { class: "fuel-band-track",
            span { class: "fuel-band-bar", style: "width: {share:.0}%" }
        }
        span { class: "fuel-band-value",
            {format!("{} · {}", show_economy(average.economy, units.economy), tr_with(messages::FUEL_STRETCHES, &[("count", count(average.stretches))]))}
        }
    }
}

/// A condition's three bands with their names.
fn labelled(names: &[MessageId; 3], bands: &[Option<Average>; 3]) -> Vec<BandRow> {
    names
        .iter()
        .zip(bands)
        .map(|(name, band)| (tr(*name), *band))
        .collect()
}

/// The towed vehicles' rows: "Not towing", then each by name ("Another vehicle" if unseen).
fn towed_rows(view: &FuelView) -> Vec<BandRow> {
    view.summary
        .breakdowns
        .towed
        .iter()
        .map(|(vehicle, average)| {
            let name = match vehicle {
                None => tr(messages::FUEL_NOT_TOWING),
                Some(id) => view
                    .towed_names
                    .iter()
                    .find_map(|(named, name)| (named == id).then(|| name.clone()))
                    .unwrap_or_else(|| tr(messages::FUEL_ANOTHER_VEHICLE)),
            };
            (name, Some(*average))
        })
        .collect()
}

#[cfg(test)]
mod tests;
