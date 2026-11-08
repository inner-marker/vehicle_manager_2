//! A vehicle's Maintenance tab (milestone 7 step 7g; choice 15, the human's pick,
//! `docs/ui-sketches/p2m7-schedule-options-2.svg`).
//!
//! One tab, with a segmented control:
//! - **Schedule** (everyone): each requirement as a card, with its rules and when it was last
//!   done (the date, the reading, and the job); turned-off ones and the bundles below. With
//!   nothing set up, Maintainers get **Set it up** and **Copy from…**.
//! - **Jobs** (everyone): the vehicle's maintenance and repairs, drafts first (the Timeline's
//!   rows, see `timeline`).
//! - **Set up** (Maintainers, in Maintainer or Admin mode): the requirements and bundles, with
//!   dialogs to add and change them, and **Copy from…** while there are no requirements.
//!
//! The view is in the address: `/vehicles/:id/tab/maintenance`, `…/tab/jobs`, `…/tab/set-up`.

use dioxus::prelude::*;
use domain::api::{AssetView, AssetsOutcome, ReferenceView, RequirementView, ScheduleOutcome};
use domain::asset::AssetId;
use domain::dates::format_date;
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::label::Label;
use domain::location::TimeZone;
use domain::log_area;
use domain::meter::{Meter, MeterId, MeterUnit};
use domain::notes::Notes;
use domain::preferences::Preferences;
use domain::quantity::{parse_reading, reading_value, show_reading};
use domain::reference::ReferenceDocumentId;
use domain::requirement::{
    Applicability, Bundle, BundleId, CalendarUnit, ComplianceMode, DoneTask, IntervalRule,
    Requirement, RequirementId, check_bundle, check_rules, copy_schedule,
};

use super::compliance::{ComplianceChoices, TypedCompliance, chip_date, is_settled, status_chip};
use super::problem_from_error;
use super::references::{
    ReferenceDialog, ReferenceLine, ReferencePicker, ReferenceSheet, short_name,
    summary as reference_summary, use_references,
};
use super::squawks::use_device_zone;
use super::timeline::{Shown, Timeline};
use super::vehicle_setup::{RowButtons, SectionHeading};
use crate::api;
use crate::ui::routes::Route;
use crate::ui::units::use_units;
use crate::ui::vehicles::use_vehicles;
use crate::ui::{tr, tr_with};

/// The Maintenance tab's views.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum MaintenanceView {
    Schedule,
    Jobs,
    SetUp,
}

impl MaintenanceView {
    /// Its name in an address (`/vehicles/:id/tab/set-up`).
    pub(super) const fn slug(self) -> &'static str {
        match self {
            Self::Schedule => "maintenance",
            Self::Jobs => "jobs",
            Self::SetUp => "set-up",
        }
    }

    /// The view an address names, if it's one of the Maintenance tab's.
    pub(super) fn from_slug(slug: &str) -> Option<Self> {
        [Self::Schedule, Self::Jobs, Self::SetUp]
            .into_iter()
            .find(|view| view.slug() == slug)
    }

    const fn label(self) -> MessageId {
        match self {
            Self::Schedule => messages::MAINTENANCE_SCHEDULE,
            Self::Jobs => messages::MAINTENANCE_JOBS,
            Self::SetUp => messages::MAINTENANCE_SET_UP,
        }
    }
}

/// A vehicle's schedule, once loaded.
#[derive(Debug, Clone, PartialEq)]
struct Schedule {
    requirements: Vec<RequirementView>,
    bundles: Vec<(BundleId, Bundle)>,
    may_configure: bool,
}

/// Loads `asset`'s schedule, again whenever `reloads` changes.
fn use_schedule(
    asset: AssetId,
    reloads: Signal<u32>,
) -> Signal<Option<Result<Schedule, MessageId>>> {
    let mut loaded = use_signal(|| None);
    use_effect(use_reactive!(|asset| {
        let _ = reloads();
        spawn(async move {
            let answer = match api::vehicle_schedule(asset).await {
                Ok(ScheduleOutcome::Schedule {
                    requirements,
                    bundles,
                    may_configure,
                }) => {
                    tracing::debug!(target: log_area::UI, %asset, requirements = requirements.len(), "schedule loaded");
                    Ok(Schedule {
                        requirements,
                        bundles,
                        may_configure,
                    })
                }
                Ok(other) => Err(refusal(&other)),
                Err(error) => Err(problem_from_error(&error).message()),
            };
            loaded.set(Some(answer));
        });
    }));
    loaded
}

/// A refusal's message.
fn refusal(outcome: &ScheduleOutcome) -> MessageId {
    match outcome {
        ScheduleOutcome::Invalid { problem } => problem.message(),
        ScheduleOutcome::NotFound => messages::VEHICLE_NOT_FOUND,
        ScheduleOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        ScheduleOutcome::Schedule { .. }
        | ScheduleOutcome::SavedRequirement { .. }
        | ScheduleOutcome::SavedBundle { .. }
        | ScheduleOutcome::Copied { .. }
        | ScheduleOutcome::Done
        | ScheduleOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// The tab: the segmented control and the chosen view.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn MaintenanceTab(vehicle: AssetView, start: MaintenanceView) -> Element {
    let mut view = use_signal(|| start);
    let reloads = use_signal(|| 0_u32);
    let schedule = use_schedule(vehicle.id, reloads);
    use_effect(use_reactive!(|start| view.set(start)));
    let views: Vec<MaintenanceView> = [
        MaintenanceView::Schedule,
        MaintenanceView::Jobs,
        MaintenanceView::SetUp,
    ]
    .into_iter()
    .filter(|each| *each != MaintenanceView::SetUp || vehicle.can_configure)
    .collect();
    let current = if views.contains(&view()) {
        view()
    } else {
        MaintenanceView::Schedule
    };
    rsx! {
        div { class: "maintenance",
            div { class: "segmented maintenance-views", role: "tablist",
                for each in views {
                    button {
                        key: "{each:?}",
                        class: if each == current { "segment on" } else { "segment" },
                        role: "tab",
                        "aria-selected": each == current,
                        onclick: move |_| view.set(each),
                        {tr(each.label())}
                    }
                }
            }
            match (current, schedule()) {
                (MaintenanceView::Jobs, _) => rsx! { Timeline { view: vehicle, only: Some(Shown::Jobs) } },
                (_, None) => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
                (_, Some(Err(problem))) => rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } },
                (MaintenanceView::Schedule, Some(Ok(loaded))) => rsx! {
                    ScheduleView { vehicle, schedule: loaded, reloads, on_set_up: move |()| view.set(MaintenanceView::SetUp) }
                },
                (MaintenanceView::SetUp, Some(Ok(loaded))) => rsx! {
                    SetUpView { vehicle, schedule: loaded, reloads }
                },
            }
        }
    }
}

// --- Words for rules and last done -------------------------------------------------------

/// A meter's short unit name.
fn unit_abbr(unit: MeterUnit) -> String {
    tr(match unit {
        MeterUnit::Miles => messages::UNIT_ABBR_MI,
        MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        MeterUnit::Hours => messages::UNIT_ABBR_H,
    })
}

/// A date (`2026-11-30`) in the person's format.
fn civil_text(date: &str, units: &Preferences) -> String {
    let utc = TimeZone::utc();
    domain::dates::local_to_utc(date, "12:00", &utc)
        .ok()
        .and_then(|at| format_date(at, &utc, units.date_format, Translator::english()).ok())
        .unwrap_or_else(|| date.to_owned())
}

/// "6 months", "90 days", "2 years": a calendar rule's count, by its unit.
const fn calendar_count(unit: CalendarUnit) -> MessageId {
    match unit {
        CalendarUnit::Days => messages::SCHEDULE_DAYS,
        CalendarUnit::Months => messages::SCHEDULE_MONTHS,
        CalendarUnit::Years => messages::SCHEDULE_YEARS,
    }
}

/// A calendar unit's name in the rule's menu: "days", "months", "years".
const fn calendar_unit_label(unit: CalendarUnit) -> MessageId {
    match unit {
        CalendarUnit::Days => messages::SCHEDULE_UNIT_DAYS,
        CalendarUnit::Months => messages::SCHEDULE_MONTHS_LABEL,
        CalendarUnit::Years => messages::SCHEDULE_UNIT_YEARS,
    }
}

/// "Every 5,000 mi or 6 months", "When needed", "On a fixed date · next 30 Nov 2026": a
/// requirement's rules, whichever comes first. `next` is a fixed date's next due date, if
/// known.
fn rules_text(
    rules: &[IntervalRule],
    meters: &[(MeterId, Meter)],
    next: Option<&str>,
    units: &Preferences,
) -> String {
    let mut periodic: Vec<String> = Vec::new();
    let mut fixed: Option<String> = None;
    for rule in rules {
        match rule {
            IntervalRule::Meter { meter, every } => {
                if let Some((_, meter)) = meters.iter().find(|(id, _)| id == meter) {
                    periodic.push(format!(
                        "{} {}",
                        show_reading(*every, meter.unit, meter.precision),
                        unit_abbr(meter.unit)
                    ));
                }
            }
            IntervalRule::Calendar {
                every,
                unit,
                end_of_month,
            } => {
                let count = tr_with(
                    calendar_count(*unit),
                    &[("count", MessageArg::from(u32::from(*every)))],
                );
                periodic.push(if *end_of_month && *unit != CalendarUnit::Days {
                    format!("{count}, {}", tr(messages::SCHEDULE_END_OF_MONTH))
                } else {
                    count
                });
            }
            IntervalRule::FixedDate { first } => {
                fixed = Some(match next.or(first.as_deref()) {
                    Some(date) => tr_with(
                        messages::SCHEDULE_FIXED_NEXT,
                        &[("date", MessageArg::from(civil_text(date, units)))],
                    ),
                    None => tr(messages::SCHEDULE_FIXED),
                });
            }
        }
    }
    let every = (!periodic.is_empty()).then(|| {
        tr_with(
            messages::SCHEDULE_EVERY,
            &[(
                "rules",
                MessageArg::from(periodic.join(&format!(" {} ", tr(messages::SCHEDULE_OR)))),
            )],
        )
    });
    match (every, fixed) {
        (Some(every), Some(fixed)) => {
            // "…, or on a fixed date": the fixed date's text, lower-cased at the start only.
            let mut letters = fixed.chars();
            let fixed = letters.next().map_or_else(String::new, |first| {
                first.to_lowercase().chain(letters).collect()
            });
            format!("{every}, {} {fixed}", tr(messages::SCHEDULE_OR))
        }
        (Some(text), None) | (None, Some(text)) => text,
        (None, None) => tr(messages::SCHEDULE_WHEN_NEEDED),
    }
}

/// "3 Nov 2026 · 52,644 mi": when a requirement was last done, with the reading it resets
/// from (its meter rule's meter, or the first reading).
fn last_done_text(
    done: &DoneTask,
    requirement: &Requirement,
    meters: &[(MeterId, Meter)],
    zone: &TimeZone,
    units: &Preferences,
) -> String {
    let date =
        format_date(done.at, zone, units.date_format, Translator::english()).unwrap_or_default();
    let ruled = requirement.rules.iter().find_map(|rule| match rule {
        IntervalRule::Meter { meter, .. } => Some(*meter),
        IntervalRule::Calendar { .. } | IntervalRule::FixedDate { .. } => None,
    });
    let reading = ruled
        .and_then(|meter| done.readings.iter().find(|reading| reading.meter == meter))
        .or_else(|| done.readings.first())
        .and_then(|reading| {
            let (_, meter) = meters.iter().find(|(id, _)| *id == reading.meter)?;
            Some(format!(
                "{} {}",
                show_reading(reading.value, meter.unit, meter.precision),
                unit_abbr(meter.unit)
            ))
        });
    match reading {
        Some(reading) => format!("{date} · {reading}"),
        None => date,
    }
}

// --- Schedule ----------------------------------------------------------------------------

/// The Schedule view: each requirement in use, with its rules and when it was last done.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ScheduleView(
    vehicle: AssetView,
    schedule: Schedule,
    reloads: Signal<u32>,
    on_set_up: EventHandler<()>,
) -> Element {
    let mut copying = use_signal(|| false);
    let known = use_references(reloads);
    let mut filter = use_signal(|| ScheduleFilter::Open);
    if schedule.requirements.is_empty() {
        return rsx! {
            section { class: "vehicle-panel schedule-empty",
                h3 { class: "schedule-empty-title", {tr(messages::SCHEDULE_EMPTY_TITLE)} }
                if schedule.may_configure {
                    p { class: "muted-note", {tr(messages::SCHEDULE_EMPTY_HINT)} }
                    div { class: "form-buttons",
                        button { class: "primary-button", r#type: "button", onclick: move |_| on_set_up.call(()), {tr(messages::SCHEDULE_SET_IT_UP)} }
                        button { class: "secondary-button", r#type: "button", onclick: move |_| copying.set(true), {tr(messages::SCHEDULE_COPY_FROM)} }
                    }
                } else {
                    p { class: "muted-note", {tr(messages::SCHEDULE_NOTHING_TRACKED)} }
                }
            }
            if copying() {
                CopyDialog { vehicle, reloads, on_close: move |()| copying.set(false) }
            }
        };
    }
    // Open (the default) hides complied, not-applicable, and turned-off ones; All shows them
    // (the human's note, milestone 9 test).
    let shown: Vec<RequirementView> = schedule
        .requirements
        .iter()
        .filter(|view| filter() == ScheduleFilter::All || is_open(view))
        .cloned()
        .collect();
    let hidden = schedule.requirements.len() - shown.len();
    // Your own schedule, then recalls and bulletins, each in its own section.
    let (recalls, own): (Vec<RequirementView>, Vec<RequirementView>) = shown
        .into_iter()
        .partition(|view| view.requirement.reference.is_some());
    let refs = known();
    let name_of = |id: RequirementId| {
        schedule
            .requirements
            .iter()
            .find(|view| view.id == id)
            .map(|view| view.requirement.name.as_str().to_owned())
            .unwrap_or_default()
    };
    rsx! {
        div { class: "schedule",
            div { class: "chip-row schedule-filter", role: "group",
                for each in [ScheduleFilter::Open, ScheduleFilter::All] {
                    button {
                        key: "{each:?}",
                        class: if filter() == each { "chip-filter on" } else { "chip-filter" },
                        r#type: "button",
                        "aria-pressed": filter() == each,
                        onclick: move |_| filter.set(each),
                        {tr(each.label())}
                    }
                }
            }
            if !own.is_empty() {
                h3 { class: "section-heading", {tr(messages::SCHEDULE_YOUR_SCHEDULE).to_uppercase()} }
                for view in own {
                    ScheduleCard { key: "{view.id}", view, vehicle: vehicle.clone(), refs: refs.clone() }
                }
            }
            if !recalls.is_empty() {
                h3 { class: "section-heading", {tr(messages::SCHEDULE_RECALLS).to_uppercase()} }
                for view in recalls {
                    ScheduleCard { key: "{view.id}", view, vehicle: vehicle.clone(), refs: refs.clone() }
                }
            }
            if filter() == ScheduleFilter::Open && hidden > 0 {
                p { class: "muted-note",
                    {tr_with(messages::SCHEDULE_HIDDEN, &[("count", MessageArg::from(i64::try_from(hidden).unwrap_or(i64::MAX)))])}
                }
            }
            if !schedule.bundles.is_empty() {
                h3 { class: "section-heading", {tr(messages::SCHEDULE_BUNDLES).to_uppercase()} }
                // A card each, like the requirements: its name, and what it covers.
                for (id, bundle) in schedule.bundles.clone() {
                    section { key: "{id}", class: "schedule-card",
                        div { class: "schedule-card-text",
                            span { class: "schedule-card-name", {bundle.name.as_str().to_owned()} }
                            span { class: "schedule-card-rules",
                                {tr_with(messages::SCHEDULE_BUNDLE_COVERS_LIST, &[(
                                    "names",
                                    MessageArg::from(bundle.requirements.iter().map(|id| name_of(*id)).collect::<Vec<_>>().join(", ")),
                                )])}
                            }
                        }
                        div { class: "schedule-card-last",
                            span { class: "schedule-card-label", {tr(messages::SCHEDULE_BUNDLE_LABEL).to_uppercase()} }
                            span { class: "schedule-card-when",
                                {tr_with(messages::SCHEDULE_BUNDLE_COUNT, &[("count", MessageArg::from(i64::try_from(bundle.requirements.len()).unwrap_or(i64::MAX)))])}
                            }
                        }
                    }
                }
            }
        }
    }
}

/// Which requirements the Schedule shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ScheduleFilter {
    /// What still applies: not complied with, applicable, and turned on.
    Open,
    /// Everything.
    All,
}

impl ScheduleFilter {
    const fn label(self) -> MessageId {
        match self {
            Self::Open => messages::SCHEDULE_FILTER_OPEN,
            Self::All => messages::SCHEDULE_FILTER_ALL,
        }
    }
}

/// Whether a requirement still applies: turned on, and neither complied with nor ruled out.
fn is_open(view: &RequirementView) -> bool {
    view.requirement.enabled && !is_settled(view.status)
}

/// What a requirement's card or row says under its name: its rules (a recurring one), or
/// "One-time" and the recall it's for; and why it doesn't apply, if it doesn't.
fn detail_line(
    view: &RequirementView,
    meters: &[(MeterId, Meter)],
    refs: &[ReferenceView],
    units: &Preferences,
) -> String {
    let requirement = &view.requirement;
    let mut parts = Vec::new();
    if requirement.mode == ComplianceMode::OneTime {
        parts.push(tr(ComplianceMode::OneTime.label()));
    } else {
        parts.push(rules_text(
            &requirement.rules,
            meters,
            view.last_done
                .as_ref()
                .and_then(|done| done.next_due.as_deref()),
            units,
        ));
    }
    if let Some(found) = requirement
        .reference
        .and_then(|id| refs.iter().find(|known| known.id == id))
    {
        parts.push(reference_summary(&found.document));
    }
    if let Applicability::NotApplicable { reason } = &requirement.applicability {
        parts.push(reason.as_str().to_owned());
    }
    parts.join(" · ")
}

/// One requirement in the Schedule: its name, status chip, what it says, the recall it's for
/// (opens it), and when it was last done.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ScheduleCard(view: RequirementView, vehicle: AssetView, refs: Vec<ReferenceView>) -> Element {
    let units = use_units();
    let zone = use_device_zone();
    let mut showing = use_signal(|| false);
    let chip = if view.requirement.enabled {
        status_chip(view.status, &view.requirement.applicability, |at| {
            chip_date(at, &zone(), &units())
        })
    } else {
        Some((tr(messages::SCHEDULE_OFF), "status-chip muted"))
    };
    let reference = view.requirement.reference;
    rsx! {
        section { class: "schedule-card",
            div { class: "schedule-card-text",
                span { class: "schedule-card-name",
                    {view.requirement.name.as_str().to_owned()}
                    if let Some((text, class)) = chip {
                        span { class, {text} }
                    }
                }
                span { class: "schedule-card-rules", {detail_line(&view, &vehicle.meters, &refs, &units())} }
                if reference.is_some() {
                    button { class: "link-button", r#type: "button", onclick: move |_| showing.set(true), {format!("{} ›", tr(messages::REFERENCE_OPEN))} }
                }
            }
            div { class: "schedule-card-last",
                span { class: "schedule-card-label", {tr(messages::SCHEDULE_LAST_DONE).to_uppercase()} }
                match &view.last_done {
                    Some(done) => rsx! {
                        span { class: "schedule-card-when", {last_done_text(done, &view.requirement, &vehicle.meters, &zone(), &units())} }
                        Link { class: "schedule-card-job", to: Route::EventDetail { event_id: done.event.to_string() }, {tr(messages::SCHEDULE_THE_JOB)} }
                    },
                    None => rsx! { span { class: "schedule-card-when muted", {tr(messages::SCHEDULE_NOT_DONE)} } },
                }
            }
        }
        if let (Some(id), true) = (reference, showing()) {
            ReferenceSheet { id, vehicle, on_close: move |()| showing.set(false) }
        }
    }
}

// --- Set up ------------------------------------------------------------------------------

/// What a dialog in Set up is doing.
#[derive(Debug, Clone, PartialEq)]
enum Editing {
    NewRequirement,
    Requirement(RequirementId, Requirement),
    NewBundle,
    Bundle(BundleId, Bundle),
    RemoveRequirement(RequirementId, String),
    RemoveBundle(BundleId, String),
    Copy,
    /// **+ Recall or bulletin**: new, or one already recorded (1B).
    PickReference,
    NewReference,
    LinkReference(Box<ReferenceView>),
    /// A recall's own view (after adding one, to attach its PDF).
    ShowReference(ReferenceDocumentId),
}

/// The Set up view: the requirements and bundles, with their dialogs.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SetUpView(vehicle: AssetView, schedule: Schedule, reloads: Signal<u32>) -> Element {
    let units = use_units();
    let mut editing = use_signal(|| None::<Editing>);
    let known = use_references(reloads);
    let linked: Vec<ReferenceDocumentId> = schedule
        .requirements
        .iter()
        .filter_map(|view| view.requirement.reference)
        .collect();
    let refs = known();
    let name_of = |id: RequirementId| {
        schedule
            .requirements
            .iter()
            .find(|view| view.id == id)
            .map(|view| view.requirement.name.as_str().to_owned())
            .unwrap_or_default()
    };
    let close = move |()| editing.set(None);
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::SCHEDULE_REQUIREMENTS,
                div { class: "setup-row-buttons",
                    if schedule.requirements.is_empty() {
                        button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::Copy)), {tr(messages::SCHEDULE_COPY_FROM)} }
                    }
                    button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::NewRequirement)), {tr(messages::SCHEDULE_ADD_REQUIREMENT)} }
                    // Nothing new once the vehicle's disposal is finalized (#067).
                    if vehicle.standing.takes_new_records {
                        button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::PickReference)), {tr(messages::SCHEDULE_ADD_REFERENCE)} }
                    }
                }
            }
            if schedule.requirements.is_empty() {
                p { class: "muted-note", {tr(messages::SCHEDULE_NONE_YET)} }
            }
            for view in schedule.requirements.clone() {
                div { key: "{view.id}", class: if view.requirement.enabled { "setup-row" } else { "setup-row off" },
                    div { class: "setup-row-text",
                        span { class: "setup-row-name", {view.requirement.name.as_str().to_owned()} }
                        span { class: "setup-row-detail", {detail_line(&view, &vehicle.meters, &refs, &units())} }
                        if !view.requirement.enabled {
                            span { class: "setup-row-detail", {tr(messages::SCHEDULE_OFF)} }
                        }
                    }
                    RowButtons {
                        on_edit: {
                            let view = view.clone();
                            move |()| editing.set(Some(Editing::Requirement(view.id, view.requirement.clone())))
                        },
                        on_remove: {
                            let view = view.clone();
                            move |()| editing.set(Some(Editing::RemoveRequirement(view.id, view.requirement.name.as_str().to_owned())))
                        },
                    }
                }
            }
        }
        section { class: "setup-section",
            SectionHeading { title: messages::SCHEDULE_BUNDLES,
                if !schedule.requirements.is_empty() {
                    button { class: "secondary-button", onclick: move |_| editing.set(Some(Editing::NewBundle)), {tr(messages::SCHEDULE_ADD_BUNDLE)} }
                }
            }
            if schedule.bundles.is_empty() {
                p { class: "muted-note", {tr(messages::SCHEDULE_NO_BUNDLES)} }
            }
            for (id, bundle) in schedule.bundles.clone() {
                div { key: "{id}", class: "setup-row",
                    div { class: "setup-row-text",
                        span { class: "setup-row-name", {bundle.name.as_str().to_owned()} }
                        span { class: "setup-row-detail",
                            {bundle.requirements.iter().map(|id| name_of(*id)).collect::<Vec<_>>().join(", ")}
                        }
                    }
                    RowButtons {
                        on_edit: {
                            let bundle = bundle.clone();
                            move |()| editing.set(Some(Editing::Bundle(id, bundle.clone())))
                        },
                        on_remove: {
                            let name = bundle.name.as_str().to_owned();
                            move |()| editing.set(Some(Editing::RemoveBundle(id, name.clone())))
                        },
                    }
                }
            }
        }
        p { class: "muted-note", {tr(messages::SCHEDULE_REMOVE_NOTE)} }
        match editing() {
            None => rsx! {},
            Some(Editing::NewRequirement) => rsx! { RequirementDialog { vehicle, existing: None, reloads, on_close: close } },
            Some(Editing::Requirement(id, requirement)) => rsx! { RequirementDialog { vehicle, existing: Some((id, requirement)), reloads, on_close: close } },
            Some(Editing::NewBundle) => rsx! { BundleDialog { vehicle, requirements: schedule.requirements.clone(), existing: None, reloads, on_close: close } },
            Some(Editing::Bundle(id, bundle)) => rsx! { BundleDialog { vehicle, requirements: schedule.requirements.clone(), existing: Some((id, bundle)), reloads, on_close: close } },
            Some(Editing::RemoveRequirement(id, name)) => rsx! { RemoveDialog { target: Removal::Requirement(id), name, reloads, on_close: close } },
            Some(Editing::RemoveBundle(id, name)) => rsx! { RemoveDialog { target: Removal::Bundle(id), name, reloads, on_close: close } },
            Some(Editing::Copy) => rsx! { CopyDialog { vehicle, reloads, on_close: close } },
            Some(Editing::PickReference) => rsx! {
                ReferencePicker {
                    known: refs,
                    linked,
                    on_new: move |()| editing.set(Some(Editing::NewReference)),
                    on_pick: move |view: ReferenceView| editing.set(Some(Editing::LinkReference(Box::new(view)))),
                    on_close: close,
                }
            },
            Some(Editing::NewReference) => rsx! {
                ReferenceDialog {
                    vehicle,
                    existing: None,
                    on_saved: move |id| {
                        reloads += 1;
                        editing.set(Some(Editing::ShowReference(id)));
                    },
                    on_close: close,
                }
            },
            Some(Editing::LinkReference(view)) => rsx! { RequirementDialog { vehicle, existing: None, link: Some(*view), reloads, on_close: close } },
            Some(Editing::ShowReference(id)) => rsx! { ReferenceSheet { id, vehicle, on_close: close } },
        }
    }
}

// --- The requirement dialog --------------------------------------------------------------

/// One rule as typed.
#[derive(Debug, Clone, PartialEq)]
enum TypedRule {
    Meter {
        meter: MeterId,
        every: String,
    },
    Calendar {
        every: String,
        unit: CalendarUnit,
        end_of_month: bool,
    },
    FixedDate {
        first: String,
    },
}

impl TypedRule {
    /// A stored rule as typed again.
    fn from_rule(rule: &IntervalRule, meters: &[(MeterId, Meter)]) -> Self {
        match rule {
            IntervalRule::Meter { meter, every } => Self::Meter {
                meter: *meter,
                every: meters
                    .iter()
                    .find(|(id, _)| id == meter)
                    .map(|(_, found)| reading_value(*every, found.unit, found.precision))
                    .unwrap_or_default(),
            },
            IntervalRule::Calendar {
                every,
                unit,
                end_of_month,
            } => Self::Calendar {
                every: every.to_string(),
                unit: *unit,
                end_of_month: *end_of_month,
            },
            IntervalRule::FixedDate { first } => Self::FixedDate {
                first: first.clone().unwrap_or_default(),
            },
        }
    }
}

/// The rules as typed, checked against the vehicle's meters.
fn rules_from(
    typed: &[TypedRule],
    meters: &[(MeterId, Meter)],
) -> Result<Vec<IntervalRule>, MessageId> {
    let rules = typed
        .iter()
        .map(|rule| match rule {
            TypedRule::Meter { meter, every } => {
                let (_, found) = meters
                    .iter()
                    .find(|(id, _)| id == meter)
                    .ok_or(messages::SOMETHING_WENT_WRONG)?;
                let every = parse_reading(every, found.unit, found.precision)
                    .map_err(domain::quantity::QuantityError::message)?;
                Ok(IntervalRule::Meter {
                    meter: *meter,
                    every,
                })
            }
            TypedRule::Calendar {
                every,
                unit,
                end_of_month,
            } => Ok(IntervalRule::Calendar {
                every: every
                    .trim()
                    .parse()
                    .map_err(|_| messages::REQUIREMENT_BAD_CALENDAR)?,
                unit: *unit,
                // The month's end is for months and years.
                end_of_month: *end_of_month && *unit != CalendarUnit::Days,
            }),
            TypedRule::FixedDate { first } => Ok(IntervalRule::FixedDate {
                first: Some(first.trim().to_owned()).filter(|date| !date.is_empty()),
            }),
        })
        .collect::<Result<Vec<_>, MessageId>>()?;
    check_rules(&rules, meters).map_err(domain::requirement::RequirementProblem::message)?;
    Ok(rules)
}

/// What the requirement dialog's fields hold.
#[derive(Debug, Clone, PartialEq)]
struct TypedRequirement {
    name: String,
    notes: String,
    enabled: bool,
    rules: Vec<TypedRule>,
}

/// The requirement to save, or the problem with what's typed: a one-time one keeps no
/// intervals (#068), and `reference` is the recall or bulletin it's for.
fn requirement_from(
    typed: &TypedRequirement,
    compliance: &TypedCompliance,
    reference: Option<ReferenceDocumentId>,
    asset: AssetId,
    meters: &[(MeterId, Meter)],
) -> Result<Requirement, MessageId> {
    let one_time = compliance.mode == ComplianceMode::OneTime;
    Ok(Requirement {
        asset,
        name: Label::parse(&typed.name).map_err(domain::label::LabelError::message)?,
        notes: Notes::parse(&typed.notes).map_err(domain::notes::NotesError::message)?,
        enabled: typed.enabled,
        rules: if one_time {
            Vec::new()
        } else {
            rules_from(&typed.rules, meters)?
        },
        reference,
        mode: compliance.mode,
        applicability: compliance.applicability()?,
    })
}

/// Add or change a requirement: its name, notes, whether it's in use, and its rules.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RequirementDialog(
    vehicle: AssetView,
    existing: Option<(RequirementId, Requirement)>,
    /// A new requirement for this recall or bulletin (from **+ Recall or bulletin**).
    #[props(default)]
    link: Option<ReferenceView>,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let meters = vehicle.meters.clone();
    let start = existing.clone();
    let start_link = link.clone();
    let mut typed = use_signal(|| match &start {
        Some((_, requirement)) => TypedRequirement {
            name: requirement.name.as_str().to_owned(),
            notes: requirement.notes.as_str().to_owned(),
            enabled: requirement.enabled,
            rules: requirement
                .rules
                .iter()
                .map(|rule| TypedRule::from_rule(rule, &meters))
                .collect(),
        },
        None => TypedRequirement {
            name: start_link
                .as_ref()
                .map(|view| short_name(&view.document.title))
                .unwrap_or_default(),
            notes: String::new(),
            enabled: true,
            rules: Vec::new(),
        },
    });
    let start = existing.clone();
    let linking = link.is_some();
    let compliance = use_signal(|| match &start {
        Some((_, requirement)) => {
            TypedCompliance::from_stored(requirement.mode, &requirement.applicability)
        }
        // A recall is most often done once.
        None if linking => TypedCompliance {
            mode: ComplianceMode::OneTime,
            ..TypedCompliance::default()
        },
        None => TypedCompliance::default(),
    });
    let reference = existing
        .as_ref()
        .and_then(|(_, requirement)| requirement.reference)
        .or_else(|| link.as_ref().map(|view| view.id));
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let rules = typed().rules;
    let used_meter = |meter: MeterId| {
        rules
            .iter()
            .any(|rule| matches!(rule, TypedRule::Meter { meter: used, .. } if *used == meter))
    };
    let free_meter = vehicle
        .meters
        .iter()
        .map(|(id, _)| *id)
        .find(|id| !used_meter(*id));
    let has_calendar = rules
        .iter()
        .any(|rule| matches!(rule, TypedRule::Calendar { .. }));
    let has_fixed = rules
        .iter()
        .any(|rule| matches!(rule, TypedRule::FixedDate { .. }));
    let save_meters = vehicle.meters.clone();
    let asset = vehicle.id;
    let editing_id = existing.as_ref().map(|(id, _)| *id);
    let save = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        let requirement =
            match requirement_from(&typed(), &compliance(), reference, asset, &save_meters) {
                Ok(requirement) => requirement,
                Err(message) => {
                    problem.set(Some(message));
                    return;
                }
            };
        busy.set(true);
        spawn(async move {
            let result = match editing_id {
                Some(id) => api::edit_requirement(id, requirement).await,
                None => api::add_requirement(requirement).await,
            };
            busy.set(false);
            match result {
                Ok(ScheduleOutcome::SavedRequirement { id }) => {
                    tracing::debug!(target: log_area::UI, requirement = %id, "requirement saved");
                    reloads += 1;
                    on_close.call(());
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let title = existing.as_ref().map_or_else(
        || tr(messages::SCHEDULE_ADD_REQUIREMENT_TITLE),
        |(_, requirement)| requirement.name.as_str().to_owned(),
    );
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            form {
                class: "dialog wide-dialog requirement-dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {title} }
                if let Some(reference) = reference {
                    ReferenceLine { reference, vehicle: vehicle.clone() }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SCHEDULE_NAME)} }
                    input { class: "field-input", value: typed().name, maxlength: "40", oninput: move |event| typed.write().name = event.value() }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SQUAWK_NOTES_LABEL)} }
                    input { class: "field-input", value: typed().notes, oninput: move |event| typed.write().notes = event.value() }
                }
                label { class: "switch",
                    input { r#type: "checkbox", checked: typed().enabled, onchange: move |event| typed.write().enabled = event.checked() }
                    span { {tr(messages::SCHEDULE_IN_USE)} }
                }
                ComplianceChoices { compliance }
                if compliance().mode == ComplianceMode::Recurring {
                h3 { class: "section-heading", {tr(messages::SCHEDULE_DUE_WHEN).to_uppercase()} }
                for (index, rule) in rules.iter().cloned().enumerate() {
                    div { key: "{index}", class: "rule-row",
                        RuleInputs { rule, meters: vehicle.meters.clone(), typed, index }
                        button {
                            class: "link-button rule-remove",
                            r#type: "button",
                            "aria-label": tr(messages::ACTION_REMOVE),
                            onclick: move |_| { typed.write().rules.remove(index); },
                            "✕"
                        }
                    }
                }
                div { class: "check-row",
                    if let Some(meter) = free_meter {
                        button { class: "secondary-button", r#type: "button",
                            onclick: move |_| typed.write().rules.push(TypedRule::Meter { meter, every: String::new() }),
                            {tr(messages::SCHEDULE_ADD_METER_RULE)}
                        }
                    }
                    if !has_calendar {
                        button { class: "secondary-button", r#type: "button",
                            onclick: move |_| typed.write().rules.push(TypedRule::Calendar { every: "12".into(), unit: CalendarUnit::Months, end_of_month: false }),
                            {tr(messages::SCHEDULE_ADD_CALENDAR_RULE)}
                        }
                    }
                    if !has_fixed {
                        button { class: "secondary-button", r#type: "button",
                            onclick: move |_| typed.write().rules.push(TypedRule::FixedDate { first: String::new() }),
                            {tr(messages::SCHEDULE_ADD_FIXED_RULE)}
                        }
                    }
                }
                p { class: "muted-note", {tr(messages::SCHEDULE_RULES_HINT)} }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::ACTION_SAVE)} }
                }
            }
        }
    }
}

/// One rule's inputs.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RuleInputs(
    rule: TypedRule,
    meters: Vec<(MeterId, Meter)>,
    typed: Signal<TypedRequirement>,
    index: usize,
) -> Element {
    let mut typed = typed;
    let mut set = move |next: TypedRule| {
        if let Some(slot) = typed.write().rules.get_mut(index) {
            *slot = next;
        }
    };
    match rule {
        TypedRule::Meter { meter, every } => {
            let unit = meters
                .iter()
                .find(|(id, _)| *id == meter)
                .map(|(_, found)| unit_abbr(found.unit))
                .unwrap_or_default();
            let every_now = every.clone();
            rsx! {
                div { class: "rule-inputs",
                    select {
                        class: "field-input",
                        onchange: move |event| {
                            if let Ok(chosen) = event.value().parse() {
                                set(TypedRule::Meter { meter: chosen, every: every_now.clone() });
                            }
                        },
                        for (id, found) in meters {
                            option { key: "{id}", value: "{id}", selected: id == meter, {found.name.as_str().to_owned()} }
                        }
                    }
                    span { {tr(messages::SCHEDULE_EVERY_LABEL)} }
                    input {
                        class: "field-input rule-number",
                        inputmode: "decimal",
                        value: every,
                        oninput: move |event| set(TypedRule::Meter { meter, every: event.value() }),
                    }
                    span { {unit} }
                }
            }
        }
        TypedRule::Calendar {
            every,
            unit,
            end_of_month,
        } => {
            let (every_unit, every_month) = (every.clone(), every.clone());
            rsx! {
                div { class: "rule-inputs",
                    span { class: "rule-kind", {tr(messages::SCHEDULE_CALENDAR)} }
                    span { {tr(messages::SCHEDULE_EVERY_LABEL)} }
                    input {
                        class: "field-input rule-number",
                        inputmode: "numeric",
                        value: every,
                        oninput: move |event| set(TypedRule::Calendar { every: event.value(), unit, end_of_month }),
                    }
                    select {
                        class: "field-input",
                        "aria-label": tr(messages::SCHEDULE_CALENDAR_UNIT),
                        onchange: move |event| {
                            let picked = CalendarUnit::ALL.into_iter().find(|each| format!("{each:?}") == event.value()).unwrap_or(unit);
                            set(TypedRule::Calendar { every: every_unit.clone(), unit: picked, end_of_month });
                        },
                        for each in CalendarUnit::ALL {
                            option { key: "{each:?}", value: "{each:?}", selected: each == unit, {tr(calendar_unit_label(each))} }
                        }
                    }
                    if unit != CalendarUnit::Days {
                        label { class: "check",
                            input {
                                r#type: "checkbox",
                                checked: end_of_month,
                                onchange: move |event| set(TypedRule::Calendar { every: every_month.clone(), unit, end_of_month: event.checked() }),
                            }
                            span { {tr(messages::SCHEDULE_END_OF_MONTH)} }
                        }
                    }
                }
            }
        }
        TypedRule::FixedDate { first } => rsx! {
            div { class: "rule-inputs",
                span { class: "rule-kind", {tr(messages::SCHEDULE_FIXED_LABEL)} }
                span { {tr(messages::SCHEDULE_FIRST_DUE)} }
                input {
                    class: "field-input",
                    r#type: "date",
                    value: first,
                    oninput: move |event| set(TypedRule::FixedDate { first: event.value() }),
                }
            }
        },
    }
}

// --- The bundle dialog -------------------------------------------------------------------

/// Add or change a bundle: its name, and which requirements it covers (in the schedule's
/// order).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn BundleDialog(
    vehicle: AssetView,
    requirements: Vec<RequirementView>,
    existing: Option<(BundleId, Bundle)>,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let start = existing.clone();
    let mut name = use_signal(|| {
        start
            .as_ref()
            .map(|(_, bundle)| bundle.name.as_str().to_owned())
            .unwrap_or_default()
    });
    let mut chosen = use_signal(|| {
        start
            .as_ref()
            .map(|(_, bundle)| bundle.requirements.clone())
            .unwrap_or_default()
    });
    let mut problem = use_signal(|| None::<MessageId>);
    let order: Vec<RequirementId> = requirements.iter().map(|view| view.id).collect();
    let editing_id = existing.as_ref().map(|(id, _)| *id);
    let asset = vehicle.id;
    let save = move |event: FormEvent| {
        event.prevent_default();
        let label = match Label::parse(&name()) {
            Ok(label) => label,
            Err(error) => {
                problem.set(Some(error.message()));
                return;
            }
        };
        // In the schedule's order, whatever order they were ticked in.
        let picked: Vec<RequirementId> = order
            .iter()
            .copied()
            .filter(|id| chosen().contains(id))
            .collect();
        if let Err(refused) = check_bundle(&picked, &order) {
            problem.set(Some(refused.message()));
            return;
        }
        let bundle = Bundle {
            asset,
            name: label,
            requirements: picked,
        };
        spawn(async move {
            let result = match editing_id {
                Some(id) => api::edit_bundle(id, bundle).await,
                None => api::add_bundle(bundle).await,
            };
            match result {
                Ok(ScheduleOutcome::SavedBundle { id }) => {
                    tracing::debug!(target: log_area::UI, bundle = %id, "bundle saved");
                    reloads += 1;
                    on_close.call(());
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            form {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {tr(if existing.is_some() { messages::SCHEDULE_EDIT_BUNDLE_TITLE } else { messages::SCHEDULE_ADD_BUNDLE_TITLE })} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::SCHEDULE_NAME)} }
                    input { class: "field-input", value: name(), maxlength: "40", oninput: move |event| name.set(event.value()) }
                }
                span { class: "field-label", {tr(messages::SCHEDULE_BUNDLE_COVERS)} }
                for view in requirements {
                    label { key: "{view.id}", class: "check",
                        input {
                            r#type: "checkbox",
                            checked: chosen().contains(&view.id),
                            onchange: move |event| {
                                let mut list = chosen.write();
                                list.retain(|id| *id != view.id);
                                if event.checked() {
                                    list.push(view.id);
                                }
                            },
                        }
                        span { {view.requirement.name.as_str().to_owned()} }
                    }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "submit", {tr(messages::ACTION_SAVE)} }
                }
            }
        }
    }
}

// --- Removing ----------------------------------------------------------------------------

/// What's being removed.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Removal {
    Requirement(RequirementId),
    Bundle(BundleId),
}

/// "Remove Engine oil & filter?"; a requirement a task has done is refused (turn it off).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RemoveDialog(
    target: Removal,
    name: String,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let mut problem = use_signal(|| None::<MessageId>);
    let mut reloads = reloads;
    let remove = move |_| {
        spawn(async move {
            let result = match target {
                Removal::Requirement(id) => api::delete_requirement(id).await,
                Removal::Bundle(id) => api::delete_bundle(id).await,
            };
            match result {
                Ok(ScheduleOutcome::Done) => {
                    tracing::debug!(target: log_area::UI, ?target, "removed from the schedule");
                    reloads += 1;
                    on_close.call(());
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr_with(messages::SCHEDULE_REMOVE_TITLE, &[("name", MessageArg::from(name.as_str()))])} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button", r#type: "button", onclick: remove, {tr(messages::ACTION_REMOVE)} }
                }
            }
        }
    }
}

// --- Copying a schedule ------------------------------------------------------------------

/// What copying from a vehicle would bring.
#[derive(Debug, Clone, PartialEq)]
struct Preview {
    requirements: usize,
    bundles: usize,
    /// "Engine oil & filter: Engine hours": meter rules with no matching meter here.
    left_out: Vec<String>,
}

/// What copying `from`'s schedule onto `to` would bring.
async fn preview(from: AssetId, to: &AssetView) -> Result<Preview, MessageId> {
    let failed = |error: &ServerFnError| problem_from_error(error).message();
    let ScheduleOutcome::Schedule {
        requirements,
        bundles,
        ..
    } = api::vehicle_schedule(from).await.map_err(|e| failed(&e))?
    else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let AssetsOutcome::Asset { view: source } =
        api::vehicle_page(from).await.map_err(|e| failed(&e))?
    else {
        return Err(messages::SOMETHING_WENT_WRONG);
    };
    let listed: Vec<(RequirementId, Requirement)> = requirements
        .into_iter()
        .map(|view| (view.id, view.requirement))
        .collect();
    let bundles: Vec<Bundle> = bundles.into_iter().map(|(_, bundle)| bundle).collect();
    let copied = copy_schedule(&listed, &bundles, &source.meters, &to.meters, to.id);
    Ok(Preview {
        requirements: copied.requirements.len(),
        bundles: copied.bundles.len(),
        left_out: copied
            .left_out
            .iter()
            .map(|(requirement, meter)| format!("{}: {}", requirement.as_str(), meter.as_str()))
            .collect(),
    })
}

/// "Copy a schedule to Van": choose the vehicle to copy from, see what comes and what's left
/// out, then copy (#031: copies, never links).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn CopyDialog(vehicle: AssetView, reloads: Signal<u32>, on_close: EventHandler<()>) -> Element {
    let vehicles = use_vehicles();
    let others: Vec<(AssetId, String)> = (vehicles.garage)()
        .map(|garage| garage.assets)
        .unwrap_or_default()
        .into_iter()
        .filter(|summary| summary.id != vehicle.id)
        .map(|summary| (summary.id, summary.asset.name.as_str().to_owned()))
        .collect();
    let first = others.first().map(|(id, _)| *id);
    let mut from = use_signal(|| first);
    let mut shown = use_signal(|| None::<Result<Preview, MessageId>>);
    let mut problem = use_signal(|| None::<MessageId>);
    let mut reloads = reloads;
    let to = vehicle.clone();
    use_effect(move || {
        let Some(source) = from() else {
            return;
        };
        let to = to.clone();
        spawn(async move {
            shown.set(Some(preview(source, &to).await));
        });
    });
    let target = vehicle.id;
    let copy = move |_| {
        let Some(source) = from() else {
            return;
        };
        spawn(async move {
            match api::copy_schedule(source, target).await {
                Ok(ScheduleOutcome::Copied {
                    requirements,
                    bundles,
                    ..
                }) => {
                    tracing::debug!(target: log_area::UI, %source, %target, requirements, bundles, "schedule copied");
                    reloads += 1;
                    on_close.call(());
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let can_copy = matches!(shown(), Some(Ok(ref preview)) if preview.requirements > 0);
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog copy-dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr_with(messages::SCHEDULE_COPY_TITLE, &[("vehicle", MessageArg::from(vehicle.asset.name.as_str()))])} }
                if others.is_empty() {
                    p { class: "muted-note", {tr(messages::SCHEDULE_COPY_NO_VEHICLES)} }
                } else {
                    label { class: "field",
                        span { class: "field-label", {tr(messages::SCHEDULE_COPY_FROM_LABEL)} }
                        select {
                            class: "field-input",
                            onchange: move |event| from.set(event.value().parse().ok()),
                            for (id, name) in others {
                                option { key: "{id}", value: "{id}", selected: Some(id) == from(), {name} }
                            }
                        }
                    }
                    match shown() {
                        None => rsx! { p { class: "muted-note", {tr(messages::LOADING)} } },
                        Some(Err(message)) => rsx! { p { class: "form-error", {tr(message)} } },
                        Some(Ok(preview)) => rsx! {
                            p {
                                {tr_with(messages::SCHEDULE_COPY_SUMMARY, &[
                                    ("requirements", MessageArg::from(i64::try_from(preview.requirements).unwrap_or(i64::MAX))),
                                    ("bundles", MessageArg::from(i64::try_from(preview.bundles).unwrap_or(i64::MAX))),
                                ])}
                            }
                            if !preview.left_out.is_empty() {
                                div { class: "copy-left-out",
                                    strong { {tr_with(messages::SCHEDULE_COPY_LEFT_OUT, &[("vehicle", MessageArg::from(vehicle.asset.name.as_str()))])} }
                                    for line in preview.left_out {
                                        span { key: "{line}", {line} }
                                    }
                                }
                            }
                        },
                    }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "button", disabled: !can_copy, onclick: copy, {tr(messages::SCHEDULE_COPY)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::meter::{EventKind, MeterDimension};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;

    use super::*;

    const MILE: i64 = 1_609_344;

    fn meters() -> Vec<(MeterId, Meter)> {
        [
            (
                20,
                "Odometer",
                MeterDimension::Distance,
                MeterUnit::Miles,
                0,
            ),
            (
                21,
                "Engine hours",
                MeterDimension::Duration,
                MeterUnit::Hours,
                1,
            ),
        ]
        .into_iter()
        .map(|(n, name, dimension, unit, precision)| {
            (
                test_id(n),
                Meter {
                    asset: test_id(10),
                    name: Label::parse(name).unwrap(),
                    dimension,
                    unit,
                    precision,
                    description: None,
                    required_for: vec![EventKind::Maintenance],
                },
            )
        })
        .collect()
    }

    #[test]
    fn rules_read_as_one_line_whichever_comes_first() {
        let units = Preferences::default();
        let oil = [
            IntervalRule::Meter {
                meter: test_id(20),
                every: 5_000 * MILE / 1000,
            },
            IntervalRule::Calendar {
                every: 6,
                unit: CalendarUnit::Months,
                end_of_month: false,
            },
        ];
        assert_eq!(
            rules_text(&oil, &meters(), None, &units),
            "Every 5,000 mi or 6 months"
        );
        let calendar = |every, unit, end_of_month| {
            rules_text(
                &[IntervalRule::Calendar {
                    every,
                    unit,
                    end_of_month,
                }],
                &meters(),
                None,
                &units,
            )
        };
        assert_eq!(calendar(90, CalendarUnit::Days, false), "Every 90 days");
        assert_eq!(calendar(1, CalendarUnit::Years, false), "Every year");
        assert_eq!(
            calendar(2, CalendarUnit::Years, true),
            "Every 2 years, by the month's end"
        );
        assert_eq!(rules_text(&[], &meters(), None, &units), "When needed");
        let inspection = [IntervalRule::FixedDate {
            first: Some("2026-11-30".into()),
        }];
        assert_eq!(
            rules_text(&inspection, &meters(), None, &units),
            "On a fixed date · next 30 Nov 2026"
        );
        assert_eq!(
            rules_text(&inspection, &meters(), Some("2027-11-30"), &units),
            "On a fixed date · next 30 Nov 2027",
            "the last job's next due date wins"
        );
        let yearly = [IntervalRule::Calendar {
            every: 1,
            unit: CalendarUnit::Months,
            end_of_month: true,
        }];
        assert_eq!(
            rules_text(&yearly, &meters(), None, &units),
            "Every month, by the month's end"
        );
    }

    #[test]
    fn last_done_shows_the_reading_its_meter_rule_uses() {
        let requirement = Requirement {
            asset: test_id(10),
            name: Label::parse("Engine oil & filter").unwrap(),
            notes: Notes::default(),
            enabled: true,
            rules: vec![IntervalRule::Meter {
                meter: test_id(21),
                every: 360_000,
            }],
            reference: None,
            mode: ComplianceMode::Recurring,
            applicability: Applicability::Applies,
        };
        let done = DoneTask {
            requirement: test_id(40),
            at: UnixMillis::from_millis(1_793_620_800_000),
            event: test_id(100),
            readings: vec![
                domain::event::Reading {
                    meter: test_id(20),
                    value: 52_644 * MILE / 1000,
                },
                domain::event::Reading {
                    meter: test_id(21),
                    value: 4_446_000,
                },
            ],
            next_due: None,
        };
        assert_eq!(
            last_done_text(
                &done,
                &requirement,
                &meters(),
                &TimeZone::utc(),
                &Preferences::default()
            ),
            "2 Nov 2026 · 1,235.0 h"
        );
    }

    #[test]
    fn typed_rules_become_checked_rules() {
        let typed = [
            TypedRule::Meter {
                meter: test_id(20),
                every: "5000".into(),
            },
            TypedRule::Calendar {
                every: " 6 ".into(),
                unit: CalendarUnit::Months,
                end_of_month: true,
            },
            TypedRule::FixedDate {
                first: String::new(),
            },
        ];
        assert_eq!(
            rules_from(&typed, &meters()),
            Ok(vec![
                IntervalRule::Meter {
                    meter: test_id(20),
                    every: parse_reading("5000", MeterUnit::Miles, 0).unwrap(),
                },
                IntervalRule::Calendar {
                    every: 6,
                    unit: CalendarUnit::Months,
                    end_of_month: true,
                },
                IntervalRule::FixedDate { first: None },
            ])
        );
        let zero = [TypedRule::Meter {
            meter: test_id(20),
            every: "0".into(),
        }];
        assert_eq!(
            rules_from(&zero, &meters()),
            Err(messages::REQUIREMENT_BAD_INTERVAL)
        );
        let months = [TypedRule::Calendar {
            every: "lots".into(),
            unit: CalendarUnit::Months,
            end_of_month: false,
        }];
        assert_eq!(
            rules_from(&months, &meters()),
            Err(messages::REQUIREMENT_BAD_CALENDAR)
        );
        let days = [TypedRule::Calendar {
            every: "90".into(),
            unit: CalendarUnit::Days,
            end_of_month: true,
        }];
        assert_eq!(
            rules_from(&days, &meters()),
            Ok(vec![IntervalRule::Calendar {
                every: 90,
                unit: CalendarUnit::Days,
                end_of_month: false
            }]),
            "the month's end is dropped for days"
        );
        // A stored rule types back into the same text.
        let rule = IntervalRule::Meter {
            meter: test_id(21),
            every: 360_000,
        };
        assert_eq!(
            TypedRule::from_rule(&rule, &meters()),
            TypedRule::Meter {
                meter: test_id(21),
                every: "100.0".into()
            }
        );
    }

    #[test]
    fn each_view_has_an_address() {
        for view in [
            MaintenanceView::Schedule,
            MaintenanceView::Jobs,
            MaintenanceView::SetUp,
        ] {
            assert_eq!(MaintenanceView::from_slug(view.slug()), Some(view));
        }
        assert_eq!(MaintenanceView::from_slug("fuel"), None);
    }
}
