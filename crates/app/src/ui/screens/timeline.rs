//! A vehicle's Timeline tab and Overview's event panels (step 6j; option A of
//! `docs/ui-sketches/p2m6-timeline-options.svg`).
//!
//! **The Timeline** lists the vehicle's events newest first, grouped by month (in each event's
//! own zone), with All / Fuel-ups / Readings filters. Each row opens the event's page. **A
//! corrected original stays, dimmed, beneath its correction:** only the correction's values
//! count. The Fuel tab is the same list, fuel-ups only (fuel economy arrives in Phase 5); the
//! Maintenance tab's Jobs view is maintenance and repairs only, drafts first (step 7g).
//! **Each list pages** (phase-6.md 1g): 100 records a page by default (50, 100, or 250,
//! remembered on this device), with ‹ Newer, Older ›, and Jump to a date. A correction and
//! the originals beneath it are one record, so they're never split across pages.
//!
//! **Overview** gains three panels: the latest fuel-ups, each meter's latest reading, and the
//! last activity.

use dioxus::prelude::*;
use domain::api::{AssetView, EventView, EventsOutcome, PreferencesOutcome};
use domain::dates::{format_date, format_time, month_heading, utc_to_local};
use domain::event::{Event, EventId, EventStatus};
use domain::event_check::PastReading;
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

/// One record in a list: an event that counts, with the originals it corrected beneath it
/// (they stay together on a page).
#[derive(Debug, Clone, PartialEq)]
pub(super) struct Entry {
    /// Its month's heading, or "In progress" for a draft job.
    heading: String,
    /// Its local date (`2026-11-02`), for Jump to; empty for a draft job.
    date: String,
    /// Whether it's a draft job, listed first.
    draft: bool,
    rows: Vec<Row>,
}

/// The records `shown`, newest first: each event that counts, then beneath it whatever it
/// corrects (and what that corrected, in turn). For jobs, the drafts come first.
pub(super) fn entries(events: &[EventView], shown: Shown) -> Vec<Entry> {
    let by_id = |id: EventId| events.iter().find(|view| view.id == id);
    let drafts_first = shown == Shown::Jobs;
    let mut drafts: Vec<Entry> = Vec::new();
    let mut dated: Vec<Entry> = Vec::new();
    // `events` comes newest first; originals that were corrected show under their correction.
    for view in events.iter().filter(|view| view.corrected_by.is_none()) {
        if !shown.shows(view.event.kind) {
            continue;
        }
        if drafts_first && view.event.status == EventStatus::Draft {
            drafts.push(Entry {
                heading: tr(messages::TIMELINE_IN_PROGRESS),
                date: String::new(),
                draft: true,
                rows: vec![Row {
                    view: view.clone(),
                    corrected: false,
                }],
            });
            continue;
        }
        let zone = &view.event.dates.zone;
        let Ok((heading, _)) = month_heading(view.event.dates.start, zone, Translator::english())
        else {
            continue;
        };
        let date = utc_to_local(view.event.dates.start, zone)
            .map(|(date, _)| date)
            .unwrap_or_default();
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
        dated.push(Entry {
            heading,
            date,
            draft: false,
            rows,
        });
    }
    drafts.extend(dated);
    drafts
}

/// `entries` grouped under their headings, in order (a month's records together).
pub(super) fn by_month(entries: &[Entry]) -> Vec<(String, Vec<Row>)> {
    let mut months: Vec<(String, Vec<Row>)> = Vec::new();
    for entry in entries {
        match months
            .iter_mut()
            .find(|(heading, _)| *heading == entry.heading)
        {
            Some((_, rows)) => rows.extend(entry.rows.iter().cloned()),
            None => months.push((entry.heading.clone(), entry.rows.clone())),
        }
    }
    months
}

// --- Paging ------------------------------------------------------------------------------

/// The page sizes offered (phase-6.md 1g).
pub(super) const PAGE_SIZES: [usize; 3] = [50, 100, 250];

/// The page size until one is chosen on this device.
pub(super) const PAGE_SIZE: usize = 100;

/// The device-memory key for the chosen page size.
const PAGE_SIZE_KEY: &str = "timelinePageSize";

/// One page of a list: which records it holds.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) struct Page {
    /// Its number, from 0 (the newest).
    number: usize,
    /// The last page's number.
    last: usize,
    /// The first record on it (from 0), and one past its last.
    start: usize,
    end: usize,
    /// How many records in all.
    total: usize,
}

impl Page {
    const fn has_newer(self) -> bool {
        self.number > 0
    }

    const fn has_older(self) -> bool {
        self.number < self.last
    }
}

/// Page `wanted` of `total` records, `size` a page; past the end, the last page.
pub(super) fn page_at(total: usize, size: usize, wanted: usize) -> Page {
    let size = size.max(1);
    let last = total.saturating_sub(1) / size;
    let number = wanted.min(last);
    let start = number * size;
    Page {
        number,
        last,
        start,
        end: (start + size).min(total),
        total,
    }
}

/// The page size remembered on this device, if it's one offered; otherwise 100.
pub(super) fn page_size_from(saved: Option<&str>) -> usize {
    saved
        .and_then(|text| text.parse().ok())
        .filter(|size| PAGE_SIZES.contains(size))
        .unwrap_or(PAGE_SIZE)
}

/// The page holding `date`'s records (`2026-11-02`): the first record on or before it (the
/// list is newest first). Before every record, the last page.
pub(super) fn page_for_date(entries: &[Entry], size: usize, date: &str) -> usize {
    let index = entries
        .iter()
        .position(|entry| !entry.draft && entry.date.as_str() <= date)
        .unwrap_or_else(|| entries.len().saturating_sub(1));
    index / size.max(1)
}

/// A count with its thousands grouped: 1342 → "1,342".
pub(super) fn grouped(count: usize) -> String {
    let digits = count.to_string();
    let mut out = String::new();
    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            out.push(',');
        }
        out.push(digit);
    }
    out
}

/// A meter's short unit name.
fn meter_abbr(unit: MeterUnit) -> String {
    tr(unit.abbr())
}

/// " ↺" after a reading that was a reset (phase-5.md milestone 1), otherwise nothing.
pub(super) const fn reset_mark(reset: bool) -> &'static str {
    if reset { " ↺" } else { "" }
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
                    "{} {}{}",
                    show_reading(reading.value, meter.unit, meter.precision),
                    meter_abbr(meter.unit),
                    reset_mark(reading.reset)
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
    // The Fuel tab says how each fuel-up counted for economy (#085).
    let economy = super::fuel_view::use_fuel_if(view.id, only == Some(Shown::FuelUps));
    let mut filter = use_signal(|| None::<EventKind>);
    // Paging (phase-6.md 1g): the page shown, and the size remembered on this device.
    let mut page_number = use_signal(|| 0_usize);
    let mut page_size = use_signal(|| PAGE_SIZE);
    use_effect(move || {
        spawn(async move {
            let saved = crate::ui::device_memory::recall(PAGE_SIZE_KEY).await;
            page_size.set(page_size_from(saved.as_deref()));
        });
    });
    // Another vehicle, or another tab's list, starts at its newest page.
    let asset = view.id;
    use_effect(use_reactive!(|asset, only| {
        let _ = (asset, only);
        page_number.set(0);
    }));
    let Some(answer) = loaded() else {
        return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } };
    };
    let data = match answer {
        Ok(data) => data,
        Err(problem) => return rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
    };
    // The Fuel tab and Jobs always show their own (the same component as the Timeline tab, so
    // its own filter may still hold the Timeline's choice).
    let shown = only.unwrap_or(Shown::Kind(filter()));
    let list = entries(&data.events, shown);
    let page = page_at(list.len(), page_size(), page_number());
    let months = by_month(list.get(page.start..page.end).unwrap_or_default());
    // The bar shows once there's more than the smallest page, so its size menu stays in reach.
    let paged = list.len() > PAGE_SIZES[0];
    let on_size = move |size: usize| {
        crate::ui::device_memory::remember(PAGE_SIZE_KEY, &size.to_string());
        page_size.set(size);
        page_number.set(0);
    };
    let on_jump = move |date: String| {
        let Some(Ok(data)) = loaded() else {
            return;
        };
        let list = entries(&data.events, shown);
        page_number.set(page_for_date(&list, page_size(), &date));
    };
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
                            onclick: move |_| {
                                filter.set(choice);
                                page_number.set(0);
                            },
                            {tr(filter_label(choice))}
                        }
                    }
                }
            }
            if paged {
                PageBar { page, size: page_size(), top: true, on_page: move |number| page_number.set(number), on_size, on_jump }
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
                        TimelineRow {
                            key: "{row.view.id}",
                            counted: economy().flatten().and_then(|fuel| super::fuel_view::fill_line(row.view.id, &fuel, &data.units)),
                            row,
                            meters: view.meters.clone(),
                            units: data.units,
                        }
                    }
                }
            }
            if paged {
                PageBar { page, size: page_size(), top: false, on_page: move |number| page_number.set(number), on_size, on_jump }
            }
        }
    }
}

/// The paging bar (phase-6.md 1g; part 6 of `docs/ui-sketches/p6m1-settings-options.svg`):
/// "Records 101–200 of 1,342", ‹ Newer and Older ›, and above the list, the page size and
/// Jump to. The bar below the list scrolls back up to the top one.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PageBar(
    page: Page,
    size: usize,
    top: bool,
    on_page: EventHandler<usize>,
    on_size: EventHandler<usize>,
    on_jump: EventHandler<String>,
) -> Element {
    let go = move |number: usize| {
        on_page.call(number);
        if !top {
            let _ = document::eval(
                "document.getElementById('timeline-top')?.scrollIntoView({ block: 'start' });",
            );
        }
    };
    let range = tr_with(
        messages::TIMELINE_RANGE,
        &[
            (
                "first",
                MessageArg::from(grouped((page.start + 1).min(page.total)).as_str()),
            ),
            ("last", MessageArg::from(grouped(page.end).as_str())),
            ("total", MessageArg::from(grouped(page.total).as_str())),
        ],
    );
    rsx! {
        nav {
            class: "page-bar",
            id: if top { "timeline-top" } else { "" },
            "aria-label": tr(messages::TIMELINE_PAGES),
            span { class: "page-range", role: "status", {range} }
            button {
                class: "choice-chip",
                r#type: "button",
                disabled: !page.has_newer(),
                onclick: move |_| go(page.number.saturating_sub(1)),
                {tr(messages::TIMELINE_NEWER)}
            }
            button {
                class: "choice-chip",
                r#type: "button",
                disabled: !page.has_older(),
                onclick: move |_| go(page.number + 1),
                {tr(messages::TIMELINE_OLDER)}
            }
            if top {
                label { class: "page-control",
                    span { {tr(messages::TIMELINE_PER_PAGE)} }
                    select {
                        class: "role-select",
                        onchange: move |event| {
                            if let Ok(chosen) = event.value().parse::<usize>()
                                && PAGE_SIZES.contains(&chosen)
                            {
                                on_size.call(chosen);
                            }
                        },
                        for choice in PAGE_SIZES {
                            option { key: "{choice}", value: "{choice}", selected: choice == size, "{choice}" }
                        }
                    }
                }
                label { class: "page-control",
                    span { {tr(messages::TIMELINE_JUMP)} }
                    input {
                        class: "field-input",
                        r#type: "date",
                        onchange: move |event| {
                            let date = event.value();
                            if !date.is_empty() {
                                on_jump.call(date);
                            }
                        },
                    }
                }
            }
        }
    }
}

#[component]
fn TimelineRow(
    row: Row,
    meters: Vec<(MeterId, Meter)>,
    units: Preferences,
    /// How a fuel-up counted for economy, on the Fuel tab (#085).
    #[props(default)]
    counted: Option<String>,
) -> Element {
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
                if let Some(counted) = counted {
                    span { class: "timeline-numbers timeline-economy", {counted} }
                }
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

/// Each meter's latest value that counts: a typed meter's latest reading, or a derived
/// meter's (#087) worked out from its inputs' readings; with the event it's from.
fn latest_values<'a>(
    events: &'a [EventView],
    meters: &[(MeterId, Meter)],
    meter: MeterId,
) -> Option<(i64, &'a EventView)> {
    let derived = meters
        .iter()
        .any(|(id, each)| *id == meter && each.derived());
    if !derived {
        return latest_readings(events, meter);
    }
    let mut readings: Vec<PastReading> = counting(events)
        .flat_map(|view| {
            view.event.readings.iter().map(move |reading| PastReading {
                meter: reading.meter,
                at: view.event.dates.start,
                value: reading.value,
                reset: reading.reset,
            })
        })
        .collect();
    readings.sort_by_key(|reading| (reading.at, reading.reset));
    let latest = domain::calculated::derived_readings(meters, &readings)
        .into_iter()
        .filter(|reading| reading.meter == meter)
        .max_by_key(|reading| reading.at)?;
    let view = counting(events).find(|view| view.event.dates.start == latest.at)?;
    Some((latest.value, view))
}

/// What a derived meter is worked out from: its inputs' names ("Odometer and Idle hours").
fn derived_from(meter: &Meter, meters: &[(MeterId, Meter)]) -> Option<String> {
    let formula = meter.formula.as_ref()?;
    let used = formula.keys();
    let names: Vec<String> = meters
        .iter()
        .filter(|(_, input)| used.contains(input.key.as_str()))
        .map(|(_, input)| input.name.as_str().to_owned())
        .collect();
    Some(tr_with(
        messages::FORMULA_DERIVED_FROM,
        &[("meters", MessageArg::from(names.join(", ")))],
    ))
}

/// Overview's Latest fuel-ups, Meters, and Last activity panels.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn OverviewPanels(view: AssetView, on_timeline: EventHandler<()>) -> Element {
    let loaded = use_vehicle_events(view.id);
    let economy = super::fuel_view::use_fuel_if(view.id, super::vehicle::has_fuel(&view.asset));
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
                // 5A (#085): the latest stretch's figure and the last-5 average.
                if let Some(line) = economy().flatten().and_then(|fuel| super::fuel_view::overview_line(&fuel, &units)) {
                    p { class: "fuel-overview", {line} }
                }
                button { class: "link-button", r#type: "button", onclick: move |_| on_timeline.call(()), {tr(messages::PANEL_SEE_TIMELINE)} }
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
                    MeterTile { key: "{meter_id}", meter: meter.clone(), latest: latest_values(&events, &view.meters, *meter_id).map(|(value, view)| (value, view.clone())), units, derived_from: derived_from(meter, &view.meters) }
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
fn MeterTile(
    meter: Meter,
    latest: Option<(i64, EventView)>,
    units: Preferences,
    /// A derived meter's inputs, in words (#087).
    #[props(default)]
    derived_from: Option<String>,
) -> Element {
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
            span { class: "meter-name",
                if derived_from.is_some() {
                    super::formula_view::FxBadge {}
                }
                {meter.name.as_str().to_owned()}
            }
            match (reading, derived_from.clone()) {
                (Some((value, _)), Some(from)) => rsx! {
                    strong { class: "meter-reading", {value} }
                    span { class: "meter-unit", {from} }
                },
                (Some((value, when)), None) => rsx! {
                    strong { class: "meter-reading", {value} }
                    span { class: "meter-unit", {when} }
                },
                (None, _) => rsx! { span { class: "meter-unit", {tr(messages::PANEL_NO_READINGS)} } },
            }
            if derived_from.is_none() {
                span { class: "meter-required", {super::vehicle::required_text(&meter)} }
            }
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
            calculated: vec![],
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
                    reset: false,
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

    /// The whole list `shown`, grouped by month, as one page.
    fn arrange(events: &[EventView], shown: Shown) -> Vec<(String, Vec<Row>)> {
        by_month(&entries(events, shown))
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
    fn a_correction_and_its_original_are_one_record_with_its_date() {
        let list = entries(&events(), Shown::Kind(None));
        let shape: Vec<(usize, &str)> = list
            .iter()
            .map(|entry| (entry.rows.len(), entry.date.as_str()))
            .collect();
        assert_eq!(
            shape,
            vec![(1, "2026-11-02"), (2, "2026-10-28"), (1, "2026-10-20")]
        );
        // A page of the records, grouped, is the same as the whole list grouped.
        assert_eq!(
            ids(&by_month(&list)),
            ids(&arrange(&events(), Shown::Kind(None)))
        );
        // The second record starts a page: its month heading shows again there.
        assert_eq!(
            ids(&by_month(&list[1..])),
            vec![(
                "October 2026".to_owned(),
                vec![(11, false), (12, true), (13, false)]
            )]
        );
    }

    #[test]
    fn pages_hold_their_share_and_stay_in_range() {
        let first = page_at(1342, 100, 0);
        assert_eq!((first.start, first.end, first.last), (0, 100, 13));
        assert!(!first.has_newer() && first.has_older());
        let second = page_at(1342, 100, 1);
        assert_eq!((second.start + 1, second.end), (101, 200));
        // Past the end: the last page, which holds what's left.
        let last = page_at(1342, 100, 99);
        assert_eq!((last.number, last.start, last.end), (13, 1300, 1342));
        assert!(last.has_newer() && !last.has_older());
        // Nothing to page, and a zero size, are still one page.
        let empty = page_at(0, 100, 3);
        assert_eq!((empty.number, empty.start, empty.end), (0, 0, 0));
        assert!(!empty.has_newer() && !empty.has_older());
        assert_eq!(page_at(5, 0, 2).end, 3);
    }

    #[test]
    fn only_an_offered_page_size_is_remembered() {
        assert_eq!(page_size_from(Some("250")), 250);
        assert_eq!(page_size_from(Some("50")), 50);
        assert_eq!(page_size_from(Some("75")), PAGE_SIZE);
        assert_eq!(page_size_from(Some("lots")), PAGE_SIZE);
        assert_eq!(page_size_from(None), PAGE_SIZE);
    }

    #[test]
    fn jump_to_opens_the_page_with_that_dates_records() {
        let list = entries(&events(), Shown::Kind(None));
        // One record a page: 2 Nov is page 0, 28 Oct page 1, 20 Oct page 2.
        assert_eq!(page_for_date(&list, 1, "2026-12-25"), 0);
        assert_eq!(page_for_date(&list, 1, "2026-11-02"), 0);
        assert_eq!(page_for_date(&list, 1, "2026-11-01"), 1);
        assert_eq!(page_for_date(&list, 1, "2026-10-21"), 2);
        // Before every record: the last page.
        assert_eq!(page_for_date(&list, 1, "2020-01-01"), 2);
        assert_eq!(page_for_date(&list, 2, "2026-10-20"), 1);
        assert_eq!(page_for_date(&[], 100, "2026-10-20"), 0);
    }

    #[test]
    fn counts_are_grouped_in_thousands() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1342), "1,342");
        assert_eq!(grouped(1_234_567), "1,234,567");
    }

    #[test]
    fn the_latest_reading_skips_corrected_originals() {
        let mut events = events();
        // The corrected original is the newest reading of all, but doesn't count.
        events[2].event.dates.start = UnixMillis::from_millis(NOV_2 + DAY);
        let (value, latest) = latest_readings(&events, test_id(20)).unwrap();
        assert_eq!((value, latest.id), (52_644, test_id(10)));
        let meter = Meter {
            key: domain::custom_field::FieldKey::parse("odometer").unwrap(),
            formula: None,
            asset: test_id(1),
            name: Label::parse("Odometer").unwrap(),
            dimension: MeterDimension::Distance,
            unit: MeterUnit::Miles,
            precision: 0,
            description: None,
            required_for: vec![],
            direction: domain::meter::MeterDirection::Up,
            reset_to: None,
        };
        let meters = vec![(test_id(20), meter)];
        assert_eq!(
            numbers(&events[0].event, &meters, &Preferences::default()),
            "33 mi"
        );
    }
}
