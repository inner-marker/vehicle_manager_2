//! A vehicle's Timeline tab and Overview's event panels (step 6j; option A of
//! `docs/ui-sketches/p2m6-timeline-options.svg`).
//!
//! **The Timeline** lists the vehicle's events newest first, grouped by month (in each event's
//! own zone), with All / Fuel-ups / Readings filters. Each row opens the event's page. **A
//! corrected original stays, dimmed, beneath its correction:** only the correction's values
//! count. The Fuel tab is the same list, fuel-ups only (fuel economy arrives in Phase 5); the
//! Maintenance tab's Jobs view is maintenance and repairs only, drafts first (step 7g).
//!
//! **Overview** gains three panels: the latest fuel-ups, each meter's latest reading, and the
//! last activity.

use dioxus::prelude::*;
use domain::api::{AssetView, EventView, EventsOutcome, PreferencesOutcome};
use domain::dates::{format_date, format_time, month_heading};
use domain::event::{Event, EventId, EventStatus};
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::meter::{EventKind, Meter, MeterId, MeterUnit};
use domain::preferences::Preferences;
use domain::quantity::{show_reading, show_volume, unit_price};
use domain::time::UnixMillis;

use super::problem_from_error;
use crate::data;
use crate::ui::routes::Route;
use crate::ui::units::volume_abbr;
use crate::ui::{tr, tr_with};

/// A vehicle's events and the person's units, once loaded.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Loaded {
    events: Vec<EventView>,
    units: Preferences,
}

/// Loads a vehicle's events (again when `asset` changes).
pub(super) fn use_vehicle_events(
    asset: domain::asset::AssetId,
) -> Signal<Option<Result<Loaded, MessageId>>> {
    let mut loaded = use_signal(|| None);
    use_effect(use_reactive!(|asset| {
        spawn(async move {
            let units = match data::get_preferences().await {
                Ok(PreferencesOutcome::Preferences { view }) => view.effective,
                _ => Preferences::default(),
            };
            let answer = match data::vehicle_events(asset).await {
                Ok(EventsOutcome::Events { events }) => Ok(Loaded { events, units }),
                Ok(EventsOutcome::NotFound) => Err(messages::VEHICLE_NOT_FOUND),
                Ok(_) => Err(messages::SOMETHING_WENT_WRONG),
                Err(error) => Err(problem_from_error(&error).message()),
            };
            loaded.set(Some(answer));
        });
    }));
    loaded
}

// --- Arranging the list ------------------------------------------------------------------

/// One row of the Timeline: an event, and whether it's a corrected original shown beneath its
/// correction.
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Row {
    pub view: EventView,
    pub corrected: bool,
}

/// Which events a list shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Shown {
    /// Everything (the Timeline tab), or one kind (its filter chips).
    Kind(Option<EventKind>),
    /// Fuel-ups only (the Fuel tab).
    FuelUps,
    /// Maintenance and repairs, drafts first (the Maintenance tab's Jobs).
    Jobs,
}

impl Shown {
    fn shows(self, kind: EventKind) -> bool {
        match self {
            Self::Kind(filter) => filter.is_none_or(|only| only == kind),
            Self::FuelUps => kind == EventKind::Fuel,
            Self::Jobs => !kind.finalizes_by_itself(),
        }
    }
}

/// The events `shown`, newest first, grouped by month: each event that counts, then beneath it
/// whatever it corrects (and what that corrected, in turn). For jobs, the drafts come first,
/// under "In progress".
pub(super) fn arrange(events: &[EventView], shown: Shown) -> Vec<(String, Vec<Row>)> {
    let by_id = |id: EventId| events.iter().find(|view| view.id == id);
    let drafts_first = shown == Shown::Jobs;
    let mut months: Vec<((i16, i8), String, Vec<Row>)> = Vec::new();
    let mut drafts: Vec<Row> = Vec::new();
    // `events` comes newest first; originals that were corrected show under their correction.
    for view in events.iter().filter(|view| view.corrected_by.is_none()) {
        if !shown.shows(view.event.kind) {
            continue;
        }
        if drafts_first && view.event.status == EventStatus::Draft {
            drafts.push(Row {
                view: view.clone(),
                corrected: false,
            });
            continue;
        }
        let zone = &view.event.dates.zone;
        let Ok((heading, key)) = month_heading(view.event.dates.start, zone, Translator::english())
        else {
            continue;
        };
        let mut rows = vec![Row {
            view: view.clone(),
            corrected: false,
        }];
        let mut next = view.event.corrects;
        while let Some(original) = next.and_then(by_id) {
            rows.push(Row {
                view: original.clone(),
                corrected: true,
            });
            next = original.event.corrects;
        }
        match months.iter_mut().find(|(have, _, _)| *have == key) {
            Some((_, _, list)) => list.extend(rows),
            None => months.push((key, heading, rows)),
        }
    }
    let in_progress = (!drafts.is_empty()).then(|| (tr(messages::TIMELINE_IN_PROGRESS), drafts));
    in_progress
        .into_iter()
        .chain(months.into_iter().map(|(_, heading, rows)| (heading, rows)))
        .collect()
}

/// A meter's short unit name.
fn meter_abbr(unit: MeterUnit) -> String {
    tr(match unit {
        MeterUnit::Miles => messages::UNIT_ABBR_MI,
        MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        MeterUnit::Hours => messages::UNIT_ABBR_H,
    })
}

/// "52,644 mi · 18.004 gal · 62.27 USD": the readings, then a fuel-up's volume and cost.
pub(super) fn numbers(event: &Event, meters: &[(MeterId, Meter)], units: &Preferences) -> String {
    // A lifecycle event: its details first.
    let mut parts: Vec<String> = event
        .lifecycle
        .as_ref()
        .map(|details| super::life_part::life_summary(details, units))
        .unwrap_or_default();
    parts.extend(
        event
            .readings
            .iter()
            .filter_map(|reading| {
                let (_, meter) = meters.iter().find(|(id, _)| *id == reading.meter)?;
                Some(format!(
                    "{} {}",
                    show_reading(reading.value, meter.unit, meter.precision),
                    meter_abbr(meter.unit)
                ))
            })
            .collect::<Vec<_>>(),
    );
    if let Some(fuel) = &event.fuel {
        parts.push(show_volume(fuel.volume_ml, units.volume));
        parts.extend(
            fuel.cost_minor
                .map(|cost| crate::ui::money(cost, fuel.currency)),
        );
    }
    // A job: its first task, how many, and what it cost.
    if let Some(work) = &event.work {
        if let Some(first) = work.tasks.first() {
            parts.push(first.title.as_str().to_owned());
        }
        let count = i64::try_from(work.tasks.len()).unwrap_or(i64::MAX);
        parts.push(tr_with(
            messages::TIMELINE_TASK_COUNT,
            &[("count", MessageArg::from(count))],
        ));
        let total = work.total_cost_minor();
        if total > 0 {
            parts.push(crate::ui::money(total, work.currency));
        }
        // What it did about squawks: "1 signed off · 1 deferred".
        let (signed, deferred) = work
            .dispositions
            .iter()
            .fold((0_i64, 0_i64), |(s, d), each| match each.action {
                domain::squawk::Disposition::SignedOff { .. } => (s + 1, d),
                domain::squawk::Disposition::Deferred { .. } => (s, d + 1),
            });
        if signed > 0 {
            parts.push(tr_with(
                messages::TIMELINE_SIGNED_OFF,
                &[("count", MessageArg::from(signed))],
            ));
        }
        if deferred > 0 {
            parts.push(tr_with(
                messages::TIMELINE_DEFERRED,
                &[("count", MessageArg::from(deferred))],
            ));
        }
    }
    parts.join(" · ")
}

/// "2 Nov 2026 · 3:30 pm", in the event's zone and the person's date format.
fn when_text(at: UnixMillis, event: &Event, units: &Preferences) -> String {
    let zone = &event.dates.zone;
    let english = Translator::english();
    format!(
        "{} · {}",
        format_date(at, zone, units.date_format, english).unwrap_or_default(),
        format_time(at, zone, english).unwrap_or_default()
    )
}

// --- The Timeline ------------------------------------------------------------------------

/// A filter chip's label.
const fn filter_label(filter: Option<EventKind>) -> MessageId {
    match filter {
        None => messages::TIMELINE_ALL,
        Some(EventKind::Fuel) => messages::TIMELINE_FUEL_UPS,
        Some(
            EventKind::Reading
            | EventKind::Maintenance
            | EventKind::Repair
            | EventKind::Acquired
            | EventKind::SentOut
            | EventKind::Returned
            | EventKind::Disposed,
        ) => messages::TIMELINE_READINGS,
    }
}

/// The Timeline tab (or the Fuel tab, or the Maintenance tab's Jobs, with `only`).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn Timeline(view: AssetView, only: Option<Shown>) -> Element {
    let loaded = use_vehicle_events(view.id);
    let mut filter = use_signal(|| None::<EventKind>);
    let Some(answer) = loaded() else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let data = match answer {
        Ok(data) => data,
        Err(problem) => return rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
    };
    // The Fuel tab and Jobs always show their own (the same component as the Timeline tab, so
    // its own filter may still hold the Timeline's choice).
    let months = arrange(&data.events, only.unwrap_or(Shown::Kind(filter())));
    let choices: Vec<Option<EventKind>> = if only.is_some() {
        Vec::new()
    } else {
        let fuel = super::vehicle::has_fuel(&view.asset);
        [None, Some(EventKind::Fuel), Some(EventKind::Reading)]
            .into_iter()
            .filter(|choice| fuel || *choice != Some(EventKind::Fuel))
            .collect()
    };
    rsx! {
        div { class: "timeline",
            if !choices.is_empty() {
                div { class: "chip-row", role: "group",
                    for choice in choices {
                        button {
                            key: "{choice:?}",
                            r#type: "button",
                            class: if filter() == choice { "choice-chip chosen" } else { "choice-chip" },
                            "aria-pressed": if filter() == choice { "true" } else { "false" },
                            onclick: move |_| filter.set(choice),
                            {tr(filter_label(choice))}
                        }
                    }
                }
            }
            if months.is_empty() {
                p { class: "placeholder",
                    if only == Some(Shown::Jobs) { {tr(messages::JOBS_EMPTY)} } else { {tr(messages::TIMELINE_EMPTY)} }
                }
            }
            for (heading, rows) in months {
                section { key: "{heading}", class: "timeline-month",
                    h3 { class: "section-heading", {heading.to_uppercase()} }
                    for row in rows {
                        TimelineRow { key: "{row.view.id}", row, meters: view.meters.clone(), units: data.units }
                    }
                }
            }
        }
    }
}

#[component]
fn TimelineRow(row: Row, meters: Vec<(MeterId, Meter)>, units: Preferences) -> Element {
    let event = &row.view.event;
    let icon = super::log_chooser::kind_icon(event.kind);
    let title = format!(
        "{} · {}",
        tr(event.kind.label()),
        when_text(event.dates.start, event, &units)
    );
    let mut chips: Vec<(String, &'static str)> = Vec::new();
    if row.corrected {
        chips.push((tr(messages::TIMELINE_CORRECTED), "status-chip"));
    } else if event.status == EventStatus::Draft {
        chips.push((tr(messages::TIMELINE_DRAFT), "status-chip draft"));
    }
    if !row.corrected {
        for flag in event.flags() {
            chips.push((format!("⚑ {}", tr(flag.label())), "status-chip flag"));
        }
    }
    rsx! {
        Link {
            class: if row.corrected { "timeline-row corrected" } else { "timeline-row" },
            to: Route::EventDetail { event_id: row.view.id.to_string() },
            span { class: "timeline-icon", {icon} }
            span { class: "timeline-text",
                span { class: "timeline-title", {title} }
                span { class: "timeline-numbers", {numbers(event, &meters, &units)} }
            }
            span { class: "timeline-chips",
                for (label, class) in chips {
                    span { key: "{label}", class, {label} }
                }
            }
        }
    }
}

// --- Overview's panels -------------------------------------------------------------------

/// The events whose values count: not corrected originals.
fn counting(events: &[EventView]) -> impl Iterator<Item = &EventView> {
    events.iter().filter(|view| view.corrected_by.is_none())
}

/// Each meter's latest reading that counts: its value and the event it's on.
pub(super) fn latest_readings(events: &[EventView], meter: MeterId) -> Option<(i64, &EventView)> {
    counting(events)
        .filter_map(|view| view.event.reading(meter).map(|value| (value, view)))
        .max_by_key(|(_, view)| view.event.dates.start)
}

/// Overview's Latest fuel-ups, Meters, and Last activity panels.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn OverviewPanels(view: AssetView, on_timeline: EventHandler<()>) -> Element {
    let loaded = use_vehicle_events(view.id);
    let Some(Ok(data)) = loaded() else {
        return rsx! {};
    };
    let Loaded { events, units } = data;
    let fuel_ups: Vec<EventView> = counting(&events)
        .filter(|view| view.event.kind == EventKind::Fuel)
        .take(3)
        .cloned()
        .collect();
    let last = counting(&events).next().cloned();
    rsx! {
        if super::vehicle::has_fuel(&view.asset) {
            section { class: "vehicle-panel",
                h3 { class: "section-heading", {tr(messages::PANEL_FUEL).to_uppercase()} }
                if fuel_ups.is_empty() {
                    p { class: "muted-note", {tr(messages::PANEL_FUEL_NONE)} }
                }
                dl { class: "event-rows",
                    for fuel_up in fuel_ups {
                        FuelUpRow { key: "{fuel_up.id}", view: fuel_up, units }
                    }
                }
                button { class: "link-button", r#type: "button", onclick: move |_| on_timeline.call(()), {tr(messages::PANEL_SEE_TIMELINE)} }
                p { class: "muted-note", {tr(messages::PANEL_ECONOMY_LATER)} }
            }
        }
        section { class: "vehicle-panel",
            h3 { class: "section-heading", {tr(messages::PANEL_LAST_ACTIVITY).to_uppercase()} }
            match last {
                None => rsx! { p { class: "muted-note", {tr(messages::PANEL_NO_ACTIVITY)} } },
                Some(last) => rsx! {
                    Link { class: "panel-link", to: Route::EventDetail { event_id: last.id.to_string() },
                        strong { {format!("{} · {}", tr(last.event.kind.label()), when_text(last.event.dates.start, &last.event, &units))} }
                    }
                    p { class: "muted-note", {tr(if last.event.status == EventStatus::Draft { messages::TIMELINE_DRAFT } else { messages::PANEL_FINALIZED })} }
                },
            }
        }
        section { class: "vehicle-panel wide",
            h3 { class: "section-heading", {tr(messages::PANEL_METERS).to_uppercase()} }
            if view.meters.is_empty() {
                p { class: "muted-note", {tr(messages::METERS_NONE)} }
            }
            div { class: "meter-tiles",
                for (meter_id, meter) in view.meters.iter() {
                    MeterTile { key: "{meter_id}", meter: meter.clone(), latest: latest_readings(&events, *meter_id).map(|(value, view)| (value, view.clone())), units }
                }
            }
        }
    }
}

#[component]
fn FuelUpRow(view: EventView, units: Preferences) -> Element {
    let event = &view.event;
    let Some(fuel) = &event.fuel else {
        return rsx! {};
    };
    let date = format_date(
        event.dates.start,
        &event.dates.zone,
        units.date_format,
        Translator::english(),
    )
    .unwrap_or_default();
    let mut amount = vec![show_volume(fuel.volume_ml, units.volume)];
    amount.extend(
        fuel.cost_minor
            .map(|cost| crate::ui::money(cost, fuel.currency)),
    );
    let per = fuel.cost_minor.and_then(|cost| {
        unit_price(cost, fuel.volume_ml, fuel.currency, units.volume)
            .map(|price| format!("{price} /{}", volume_abbr(units.volume)))
    });
    rsx! {
        Link { class: "event-row panel-link", to: Route::EventDetail { event_id: view.id.to_string() },
            dt { {date} }
            dd {
                span { {amount.join(" · ")} }
                if let Some(per) = per {
                    span { class: "muted-note", {per} }
                }
            }
        }
    }
}

#[component]
fn MeterTile(meter: Meter, latest: Option<(i64, EventView)>, units: Preferences) -> Element {
    let reading = latest.as_ref().map(|(value, view)| {
        let event = &view.event;
        let date = format_date(
            event.dates.start,
            &event.dates.zone,
            units.date_format,
            Translator::english(),
        )
        .unwrap_or_default();
        (
            format!(
                "{} {}",
                show_reading(*value, meter.unit, meter.precision),
                meter_abbr(meter.unit)
            ),
            tr_with(
                messages::PANEL_READING_WHEN,
                &[
                    ("date", MessageArg::from(date)),
                    (
                        "kind",
                        MessageArg::from(tr(event.kind.label()).to_lowercase()),
                    ),
                ],
            ),
        )
    });
    rsx! {
        div { class: "meter-tile",
            span { class: "meter-name", {meter.name.as_str().to_owned()} }
            match reading {
                Some((value, when)) => rsx! {
                    strong { class: "meter-reading", {value} }
                    span { class: "meter-unit", {when} }
                },
                None => rsx! { span { class: "meter-unit", {tr(messages::PANEL_NO_READINGS)} } },
            }
            span { class: "meter-required", {super::vehicle::required_text(&meter)} }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::dates::EventDates;
    use domain::event::{EventLocation, EventSource, Reading};
    use domain::label::Label;
    use domain::location::{LocationSource, TimeZone};
    use domain::meter::MeterDimension;
    use domain::notes::Notes;
    use domain::test_support::test_id;

    use super::*;

    const DAY: i64 = 86_400_000;
    /// 2 November 2026, noon UTC.
    const NOV_2: i64 = 1_793_620_800_000;

    fn view(n: u32, kind: EventKind, days_before: i64, odometer: i64) -> EventView {
        let at = UnixMillis::from_millis(NOV_2 - days_before * DAY);
        EventView {
            id: test_id(n),
            event: Event {
                asset: test_id(1),
                kind,
                dates: EventDates::new(at, None, TimeZone::parse("UTC").unwrap()).unwrap(),
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
                    meter: test_id(20),
                    value: odometer,
                }],
                skipped: vec![],
                fields: vec![],
                notes: Notes::default(),
                source: EventSource::Typed,
                logged_by: test_id(5),
                performed_by: None,
                fuel: None,
                work: None,
                lifecycle: None,
                reading_override: None,
                status: EventStatus::Finalized,
                last_edited_at: at,
                received_at: at,
                finalized: None,
                corrects: None,
                correction_reason: None,
            },
            corrected_by: None,
            may_change: false,
            may_correct: false,
            logged_by_name: "Jordan".into(),
            finalized_by_name: None,
        }
    }

    /// Newest first, as the server sends them: a fuel-up in November, then in October a
    /// correction (logged later, so it comes first), what it corrects, and a reading.
    fn events() -> Vec<EventView> {
        let november = view(10, EventKind::Fuel, 0, 52_644);
        let mut correction = view(11, EventKind::Fuel, 5, 52_301);
        correction.event.corrects = Some(test_id(12));
        let mut original = view(12, EventKind::Fuel, 5, 52_310);
        original.corrected_by = Some(test_id(11));
        let reading = view(13, EventKind::Reading, 13, 52_118);
        vec![november, correction, original, reading]
    }

    fn ids(months: &[(String, Vec<Row>)]) -> Vec<(String, Vec<(u32, bool)>)> {
        let number = |id: EventId| (10..20).find(|n| test_id::<Event>(*n) == id).unwrap_or(0);
        months
            .iter()
            .map(|(heading, rows)| {
                (
                    heading.clone(),
                    rows.iter()
                        .map(|row| (number(row.view.id), row.corrected))
                        .collect(),
                )
            })
            .collect()
    }

    #[test]
    fn the_timeline_groups_by_month_and_puts_originals_under_their_corrections() {
        assert_eq!(
            ids(&arrange(&events(), Shown::Kind(None))),
            vec![
                ("November 2026".to_owned(), vec![(10, false)]),
                (
                    "October 2026".to_owned(),
                    vec![(11, false), (12, true), (13, false)]
                ),
            ]
        );
        assert_eq!(
            ids(&arrange(&events(), Shown::Kind(Some(EventKind::Reading)))),
            vec![("October 2026".to_owned(), vec![(13, false)])]
        );
    }

    #[test]
    fn jobs_show_only_maintenance_and_repairs_with_drafts_first() {
        let mut events = events();
        let mut draft = view(14, EventKind::Repair, 20, 52_000);
        draft.event.status = EventStatus::Draft;
        events.push(draft);
        events.push(view(15, EventKind::Maintenance, 1, 52_600));
        assert_eq!(
            ids(&arrange(&events, Shown::Jobs)),
            vec![
                ("In progress".to_owned(), vec![(14, false)]),
                ("November 2026".to_owned(), vec![(15, false)]),
            ]
        );
    }

    #[test]
    fn the_latest_reading_skips_corrected_originals() {
        let mut events = events();
        // The corrected original is the newest reading of all, but doesn't count.
        events[2].event.dates.start = UnixMillis::from_millis(NOV_2 + DAY);
        let (value, latest) = latest_readings(&events, test_id(20)).unwrap();
        assert_eq!((value, latest.id), (52_644, test_id(10)));
        let meter = Meter {
            asset: test_id(1),
            name: Label::parse("Odometer").unwrap(),
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            precision: 0,
            description: None,
            required_for: vec![],
        };
        let meters = vec![(test_id(20), meter)];
        assert_eq!(
            numbers(&events[0].event, &meters, &Preferences::default()),
            "33 mi"
        );
    }
}
