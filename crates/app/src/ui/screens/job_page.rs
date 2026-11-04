//! A maintenance or repair job's page: what the event page adds for a job (step 7j; option A
//! of `docs/ui-sketches/p2m7-job-page-options.svg`, the human's pick).
//!
//! - **The big numbers:** the total (parts and labor), the tasks, each meter's reading at the
//!   start and the end, and the labor time.
//! - **A repair's** complaint and cause.
//! - **The tasks,** each a card: what it was for (its requirement), what it reset from, its
//!   parts and labor as a table, its notes, and its total. The first is open; the others fold
//!   to one line.
//! - **The squawks it dealt with:** each signed off (by which task) or deferred (why, and a
//!   severity change), linked to its page; then the ones found during the job.

use dioxus::prelude::*;
use domain::api::{AssetView, RequirementView, ScheduleOutcome, SquawkView};
use domain::asset::AssetId;
use domain::attachment::{AttachmentOwner, PhotoSize};
use domain::event::{Event, EventId, Task, WorkDetails};
use domain::i18n::{MessageArg, Translator, messages};
use domain::meter::{Meter, MeterId, MeterUnit};
use domain::preferences::CurrencyCode;
use domain::profile::high_severity_name;
use domain::quantity::{quantity_value, show_duration, show_reading};
use domain::squawk::{Disposition, SquawkDisposition};

use super::attachments::{AttachmentStrip, photo_size_for};
use super::squawks::{SeverityChip, use_vehicle_squawks};
use crate::api;
use crate::ui::routes::Route;
use crate::ui::{tr, tr_with};

/// A meter's unit, short.
fn unit_abbr(unit: MeterUnit) -> String {
    tr(match unit {
        MeterUnit::Miles => messages::UNIT_ABBR_MI,
        MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        MeterUnit::Hours => messages::UNIT_ABBR_H,
    })
}

/// "52,644 → 52,700 mi" (or just the start, without an end reading): one meter's readings.
pub(super) fn readings_text(event: &Event, meter_id: MeterId, meter: &Meter) -> Option<String> {
    let start = event.reading(meter_id)?;
    let shown = |value| show_reading(value, meter.unit, meter.precision);
    let end = event
        .work
        .as_ref()
        .and_then(|work| work.end_reading(meter_id));
    Some(match end {
        Some(end) => format!(
            "{} → {} {}",
            shown(start),
            shown(end),
            unit_abbr(meter.unit)
        ),
        None => format!("{} {}", shown(start), unit_abbr(meter.unit)),
    })
}

/// Every task's labor time together, in seconds.
pub(super) fn labor_seconds(work: &WorkDetails) -> i64 {
    work.tasks
        .iter()
        .filter_map(|task| task.labor_seconds)
        .sum()
}

/// "parts $82.44 · labor $50.00".
pub(super) fn split_text(work: &WorkDetails) -> String {
    let parts: i64 = work.tasks.iter().map(Task::parts_cost_minor).sum();
    let labor: i64 = work
        .tasks
        .iter()
        .filter_map(|task| task.labor_cost_minor)
        .sum();
    tr_with(
        messages::JOB_SPLIT,
        &[
            (
                "parts",
                MessageArg::from(crate::ui::money(parts, work.currency)),
            ),
            (
                "labor",
                MessageArg::from(crate::ui::money(labor, work.currency)),
            ),
        ],
    )
}

/// What the job did about a squawk: "Signed off · by Brake light bulbs · New bulbs",
/// "Deferred · Parts on order · → Medium".
pub(super) fn disposition_text(disposition: &SquawkDisposition, tasks: &[Task]) -> String {
    let mut bits = match &disposition.action {
        Disposition::SignedOff { notes, task } => {
            let mut bits = vec![tr(messages::SQUAWK_HISTORY_SIGNED_OFF)];
            if let Some(task) = task.and_then(|index| tasks.get(usize::from(index))) {
                bits.push(tr_with(
                    messages::JOB_FIXED_BY,
                    &[("task", MessageArg::from(task.title.as_str()))],
                ));
            }
            if !notes.as_str().is_empty() {
                bits.push(notes.as_str().to_owned());
            }
            bits
        }
        Disposition::Deferred { reason } => {
            vec![tr(messages::JOB_DEFERRED), reason.as_str().to_owned()]
        }
    };
    if let Some(severity) = disposition.severity {
        bits.push(format!("→ {}", tr(severity.label())));
    }
    bits.join(" · ")
}

/// A job's big numbers: the total, the tasks, each reading (start → end), and the labor time.
#[component]
pub(super) fn JobTiles(event: Event, vehicle: AssetView) -> Element {
    let Some(work) = event.work.clone() else {
        return rsx! {};
    };
    let titles: Vec<String> = work
        .tasks
        .iter()
        .map(|task| task.title.as_str().to_owned())
        .collect();
    let mut tiles: Vec<(String, String, String)> = vec![
        (
            tr(messages::JOB_TOTAL).to_uppercase(),
            crate::ui::money(work.total_cost_minor(), work.currency),
            split_text(&work),
        ),
        (
            tr(messages::WORK_TASKS).to_uppercase(),
            work.tasks.len().to_string(),
            titles.join(", "),
        ),
    ];
    tiles.extend(vehicle.meters.iter().filter_map(|(id, meter)| {
        Some((
            meter.name.as_str().to_uppercase(),
            readings_text(&event, *id, meter)?,
            String::new(),
        ))
    }));
    let labor = labor_seconds(&work);
    if labor > 0 {
        tiles.push((
            tr(messages::JOB_LABOR).to_uppercase(),
            show_duration(labor),
            String::new(),
        ));
    }
    rsx! {
        div { class: "event-tiles job-tiles",
            for (label, big, small) in tiles {
                div { key: "{label}", class: "event-tile",
                    span { class: "event-tile-label", {label} }
                    strong { {big} }
                    if !small.is_empty() {
                        span { {small} }
                    }
                }
            }
        }
    }
}

/// A repair's complaint and cause, the tasks, and the squawks the job dealt with.
#[component]
pub(super) fn JobSections(id: EventId, event: Event, vehicle: AssetView) -> Element {
    let squawks = use_vehicle_squawks(vehicle.id);
    let mut schedule = use_signal(Vec::<RequirementView>::new);
    let asset = vehicle.id;
    use_effect(use_reactive!(|asset| {
        spawn(async move {
            if let Ok(ScheduleOutcome::Schedule { requirements, .. }) =
                api::vehicle_schedule(asset).await
            {
                schedule.set(requirements);
            }
        });
    }));
    let Some(work) = event.work else {
        return rsx! {};
    };
    let all: Vec<SquawkView> = squawks().map(|loaded| loaded.squawks).unwrap_or_default();
    let found: Vec<SquawkView> = all
        .iter()
        .filter(|view| view.squawk.found_on == Some(id))
        .cloned()
        .collect();
    let high_word = high_severity_name(&vehicle.asset.kind, Translator::english());
    let currency = work.currency;
    rsx! {
        if work.complaint.is_some() || work.cause.is_some() {
            section { class: "event-section",
                h3 { class: "section-heading", {tr(messages::WORK_REPAIR).to_uppercase()} }
                dl { class: "event-rows",
                    if let Some(complaint) = &work.complaint {
                        div { class: "event-row", dt { {tr(messages::WORK_COMPLAINT)} } dd { {complaint.as_str().to_owned()} } }
                    }
                    if let Some(cause) = &work.cause {
                        div { class: "event-row", dt { {tr(messages::WORK_CAUSE)} } dd { {cause.as_str().to_owned()} } }
                    }
                }
            }
        }
        section { class: "event-section",
            h3 { class: "section-heading", {tr(messages::WORK_TASKS).to_uppercase()} }
            if work.tasks.is_empty() {
                p { class: "muted-note", {tr(messages::JOB_NO_TASKS)} }
            }
            for (index, task) in work.tasks.iter().cloned().enumerate() {
                TaskView {
                    key: "{index}",
                    task,
                    open: index == 0,
                    asset: vehicle.id,
                    owner: AttachmentOwner::Event { event: id, task: u16::try_from(index).ok() },
                    photo_size: photo_size_for(&vehicle.asset.kind, event.kind),
                    requirement: schedule().iter().find(|view| Some(view.id) == work.tasks[index].requirement).map(|view| view.requirement.name.as_str().to_owned()),
                    asset_id: vehicle.id.to_string(),
                    currency,
                }
            }
        }
        AttachmentStrip {
            asset: vehicle.id,
            owner: AttachmentOwner::Event { event: id, task: None },
            heading: messages::ATTACHMENT_JOB_PHOTOS,
            default_size: photo_size_for(&vehicle.asset.kind, event.kind),
        }
        if !work.dispositions.is_empty() || !found.is_empty() {
            section { class: "event-section",
                h3 { class: "section-heading", {tr(messages::JOB_SQUAWKS).to_uppercase()} }
                for disposition in work.dispositions.clone() {
                    div { key: "{disposition.squawk}", class: "job-squawk",
                        div { class: "squawk-row-top",
                            if let Some(view) = all.iter().find(|view| view.id == disposition.squawk) {
                                SeverityChip { severity: view.severity, high_word: high_word.clone() }
                                Link { class: "job-squawk-link", to: Route::SquawkDetail { squawk_id: view.id.to_string() },
                                    {format!("{} ›", view.squawk.description.as_str())}
                                }
                            }
                        }
                        span {
                            class: match disposition.action { Disposition::SignedOff { .. } => "job-squawk-what signed-off", Disposition::Deferred { .. } => "job-squawk-what deferred" },
                            {disposition_text(&disposition, &work.tasks)}
                        }
                    }
                }
                for view in found {
                    Link { key: "{view.id}", class: "job-squawk-link", to: Route::SquawkDetail { squawk_id: view.id.to_string() },
                        {tr_with(messages::JOB_FOUND, &[
                            ("description", MessageArg::from(view.squawk.description.as_str())),
                            ("severity", MessageArg::from(tr(view.severity.label()))),
                        ])}
                    }
                }
            }
        }
    }
}

/// One task: open, as a card with its parts and labor; or folded to one line.
#[component]
fn TaskView(
    task: Task,
    open: bool,
    asset: AssetId,
    owner: AttachmentOwner,
    photo_size: PhotoSize,
    requirement: Option<String>,
    asset_id: String,
    currency: CurrencyCode,
) -> Element {
    let total = crate::ui::money(
        task.parts_cost_minor() + task.labor_cost_minor.unwrap_or(0),
        currency,
    );
    let parts = i64::try_from(task.parts.len()).unwrap_or(i64::MAX);
    let mut summary = vec![tr_with(
        messages::WORK_PART_COUNT,
        &[("count", MessageArg::from(parts))],
    )];
    summary.extend(task.labor_seconds.map(show_duration));
    summary.push(total.clone());
    let resets = tr(match task.resets_from {
        domain::event::ReadingSet::Start => messages::JOB_RESETS_START,
        domain::event::ReadingSet::End => messages::JOB_RESETS_END,
    });
    rsx! {
        details { class: "job-task", open,
            summary { class: "job-task-head",
                span { class: "job-task-title", {task.title.as_str().to_owned()} }
                span { class: "job-task-summary", {summary.join(" · ")} }
            }
            div { class: "job-task-body",
                if let Some(name) = requirement {
                    p { class: "muted-note",
                        Link { to: Route::VehicleTab { asset_id, tab: "maintenance".to_owned() },
                            {tr_with(messages::WORK_TASK_FOR, &[("name", MessageArg::from(name))])}
                        }
                        {format!(" · {resets}")}
                    }
                }
                if let Some(next) = &task.next_due {
                    p { class: "muted-note", {tr_with(messages::JOB_NEXT_DUE, &[("date", MessageArg::from(next.as_str()))])} }
                }
                if !task.parts.is_empty() || task.labor_seconds.is_some() || task.labor_cost_minor.is_some() {
                    table { class: "job-parts",
                        thead {
                            tr {
                                th { {tr(messages::WORK_PART_WHAT)} }
                                th { {tr(messages::WORK_PART_NUMBER)} }
                                th { class: "number", {tr(messages::WORK_PART_QUANTITY)} }
                                th { class: "number", {tr(messages::WORK_PART_PRICE)} }
                                th { class: "number", {tr(messages::JOB_COST)} }
                            }
                        }
                        tbody {
                            for (row, part) in task.parts.iter().enumerate() {
                                tr { key: "{row}",
                                    td { {part.description.clone()} }
                                    td { class: "muted", {part.part_number.clone().unwrap_or_default()} }
                                    td { class: "number", {quantity_value(part.quantity_milli)} }
                                    td { class: "number", {part.unit_price_minor.map(|price| crate::ui::money(price, currency)).unwrap_or_default()} }
                                    td { class: "number", {part.cost_minor().map(|cost| crate::ui::money(cost, currency)).unwrap_or_default()} }
                                }
                            }
                            if task.labor_seconds.is_some() || task.labor_cost_minor.is_some() {
                                tr {
                                    td { {tr(messages::JOB_LABOR)} }
                                    td {}
                                    td { class: "number", {task.labor_seconds.map(show_duration).unwrap_or_default()} }
                                    td {}
                                    td { class: "number", {task.labor_cost_minor.map(|cost| crate::ui::money(cost, currency)).unwrap_or_default()} }
                                }
                            }
                        }
                    }
                }
                if !task.notes.as_str().is_empty() {
                    p { class: "muted-note", {tr_with(messages::JOB_NOTES, &[("notes", MessageArg::from(task.notes.as_str()))])} }
                }
                p { class: "task-card-total", {tr_with(messages::WORK_TASK_TOTAL, &[("total", MessageArg::from(total))])} }
                AttachmentStrip { asset, owner, heading: messages::ATTACHMENT_TASK_PHOTOS, default_size: photo_size }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::event::{Part, ReadingSet};
    use domain::label::Label;
    use domain::notes::Notes;
    use domain::test_support::test_id;

    use super::*;

    fn task(title: &str, labor: Option<i64>, labor_cost: Option<i64>, parts: Vec<Part>) -> Task {
        Task {
            title: Label::parse(title).unwrap(),
            requirement: None,
            notes: Notes::default(),
            parts,
            labor_seconds: labor,
            labor_cost_minor: labor_cost,
            resets_from: ReadingSet::Start,
            next_due: None,
        }
    }

    fn work() -> WorkDetails {
        WorkDetails {
            tasks: vec![
                task(
                    "Engine oil & filter",
                    Some(1_800),
                    Some(4_500),
                    vec![Part {
                        description: "Oil filter".into(),
                        part_number: None,
                        quantity_milli: 1_000,
                        unit_price_minor: Some(1_299),
                    }],
                ),
                task("Brake light bulbs", Some(600), None, vec![]),
            ],
            end_readings: vec![],
            dispositions: vec![],
            currency: CurrencyCode::USD,
            complaint: None,
            cause: None,
        }
    }

    #[test]
    fn the_big_numbers_add_up() {
        let work = work();
        assert_eq!(labor_seconds(&work), 2_400);
        assert_eq!(split_text(&work), "parts $12.99 · labor $45.00");
    }

    #[test]
    fn a_disposition_says_what_the_job_did() {
        let work = work();
        let signed = SquawkDisposition {
            squawk: test_id(60),
            action: Disposition::SignedOff {
                notes: Notes::parse("New bulbs").unwrap(),
                task: Some(1),
            },
            severity: None,
        };
        assert_eq!(
            disposition_text(&signed, &work.tasks),
            "Signed off · by Brake light bulbs · New bulbs"
        );
        let deferred = SquawkDisposition {
            squawk: test_id(61),
            action: Disposition::Deferred {
                reason: domain::event::Reason::parse("Parts on order").unwrap(),
            },
            severity: Some(domain::squawk::Severity::Medium),
        };
        assert_eq!(
            disposition_text(&deferred, &work.tasks),
            "Deferred · Parts on order · → Medium"
        );
    }
}
