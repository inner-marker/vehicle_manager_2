//! The maintenance and repair work log: the log form's sections for a job (step 7i; choice 16,
//! option A of `docs/ui-sketches/p2m7-work-log-options.svg`).
//!
//! - **Readings at the end** (after a test drive), once asked for.
//! - **Tasks**, from a requirement, a bundle (a task for each of its requirements), or typed.
//!   The open task is a card with its parts table, labor time and cost, what it resets its
//!   requirement from, a fixed-date rule's next due date, and notes; the others fold to one line.
//! - **Squawks:** each open one on the vehicle, signed off (with the task that fixed it, and
//!   notes) or deferred (with a reason), and a new severity if it changed. One found during the
//!   job is opened with the squawk sheet, and joins the list.
//! - **A repair's complaint and cause.**
//!
//! The typed values live in one signal ([`TypedWork`]); `work_input` builds the work from them.

use dioxus::prelude::*;
use domain::api::{AssetView, RequirementView, SquawkView};
use domain::asset::AssetId;
use domain::attachment::{AttachmentOwner, PhotoSize};
use domain::event::EventId;
use domain::i18n::{MessageArg, messages};
use domain::meter::{EventKind, Meter, MeterId, MeterUnit};
use domain::preferences::CurrencyCode;
use domain::profile::high_severity_name;
use domain::quantity::duration_as_typed;
use domain::requirement::{Bundle, BundleId, IntervalRule};
use domain::squawk::{CountedDisposition, Severity, SquawkId, status};

use super::FormError;
use super::attachments::{AttachmentStrip, Held, HeldStrip, photo_size_for};
use super::log_form::MoneyInput;
use super::squawks::{SeverityChip, use_squawk_sheet};
use super::work_input::{
    Action, TypedDisposition, TypedPart, TypedTask, TypedWork, prefilled_part, total_minor,
};
use crate::ui::{tr, tr_with};
use domain::spec::{Spec, SpecAmount, SpecId, task_specs};

/// A meter's unit, short.
fn unit_abbr(unit: MeterUnit) -> String {
    tr(unit.abbr())
}

/// The tasks for `bundle`'s requirements, in its order (those already in the list are left
/// out), parts pre-filled from the vehicle's `specs`.
pub(super) fn bundle_tasks(
    bundle: &Bundle,
    requirements: &[RequirementView],
    already: &[TypedTask],
    specs: &[(SpecId, Spec)],
) -> Vec<TypedTask> {
    bundle
        .requirements
        .iter()
        .filter(|id| !already.iter().any(|task| task.requirement == Some(**id)))
        .filter_map(|id| requirements.iter().find(|view| view.id == *id))
        .map(|view| requirement_task(view, specs))
        .collect()
}

/// The work a job starts with for **Log this service** (#084): a task for each of `bundle`'s
/// requirements, parts pre-filled from `specs`, each one's resets ticked.
pub(super) fn service_work(
    bundle: &Bundle,
    requirements: &[RequirementView],
    specs: &[(SpecId, Spec)],
    meters: &[(MeterId, Meter)],
) -> TypedWork {
    let mut work = TypedWork {
        tasks: bundle_tasks(bundle, requirements, &[], specs),
        ..TypedWork::default()
    };
    for view in requirements
        .iter()
        .filter(|view| bundle.requirements.contains(&view.id))
    {
        tick_resets(&mut work, view, meters);
    }
    work
}

/// The resettable meters `view`'s rules count on: a task for it offers to reset each (#082).
pub(super) fn reset_meters(
    view: &RequirementView,
    meters: &[(MeterId, Meter)],
) -> Vec<(MeterId, Meter)> {
    meters
        .iter()
        .filter(|(id, meter)| {
            meter.resettable()
                && view.requirement.rules.iter().any(|rule| match rule {
                    IntervalRule::Meter { meter, .. } | IntervalRule::Reaches { meter, .. } => {
                        meter == id
                    }
                    IntervalRule::Calendar { .. } | IntervalRule::FixedDate { .. } => false,
                })
        })
        .cloned()
        .collect()
}

/// "↺ Reset Oil life to 100 %".
fn reset_label(meter: &Meter) -> String {
    let value = meter.reset_to.map_or_else(String::new, |value| {
        format!(
            "{} {}",
            domain::quantity::show_reading(value, meter.unit, meter.precision),
            tr(meter.unit.abbr())
        )
    });
    tr_with(
        messages::WORK_RESET_METER,
        &[
            ("meter", MessageArg::from(meter.name.as_str().to_owned())),
            ("value", MessageArg::from(value)),
        ],
    )
}

/// Ticks the resets a task for `view` offers (ticked by default, #082).
fn tick_resets(work: &mut TypedWork, view: &RequirementView, meters: &[(MeterId, Meter)]) {
    for (id, _) in reset_meters(view, meters) {
        if !work.resets.contains(&id) {
            work.resets.push(id);
        }
    }
}

/// A new task doing `view`'s requirement, titled with its name.
pub(super) fn requirement_task(view: &RequirementView, specs: &[(SpecId, Spec)]) -> TypedTask {
    // The specs it ticks pre-fill its parts (#084); with none, an empty row to type in.
    let mut parts: Vec<TypedPart> = task_specs(&view.requirement.specs, specs)
        .parts
        .iter()
        .map(prefilled_part)
        .collect();
    if parts.is_empty() {
        parts.push(TypedPart::default());
    }
    TypedTask {
        title: view.requirement.name.as_str().to_owned(),
        requirement: Some(view.id),
        parts,
        ..TypedTask::default()
    }
}

/// "Lug nut torque: 150 ft-lb (203 N·m)": the specs a task for `view` reminds of (#084).
pub(super) fn reminder_lines(view: &RequirementView, specs: &[(SpecId, Spec)]) -> Vec<String> {
    task_specs(&view.requirement.specs, specs)
        .reminders
        .iter()
        .filter_map(|id| specs.iter().find(|(spec, _)| spec == id))
        .map(|(_, spec)| {
            let amount = spec.amount.map(SpecAmount::show_both).unwrap_or_default();
            tr_with(
                messages::SPEC_REMINDER,
                &[
                    ("name", MessageArg::from(spec.name.as_str())),
                    ("amount", MessageArg::from(amount.as_str())),
                ],
            )
        })
        .collect()
}

/// Removes task `index`, and keeps the sign-offs naming the tasks after it right.
pub(super) fn remove_task(work: &mut TypedWork, index: usize) {
    if index >= work.tasks.len() {
        return;
    }
    work.tasks.remove(index);
    work.removed_tasks
        .push(u16::try_from(index).unwrap_or(u16::MAX));
    for disposition in &mut work.dispositions {
        disposition.task = match disposition.task {
            Some(task) if task == index => None,
            Some(task) if task > index => Some(task - 1),
            other => other,
        };
    }
}

/// A folded task's title, or that it has none yet.
fn folded_title(task: &TypedTask) -> String {
    if task.title.trim().is_empty() {
        tr(messages::WORK_UNTITLED_TASK)
    } else {
        task.title.clone()
    }
}

/// A task folded to one line: "2 parts · 0:30 · $107.44".
fn task_summary(task: &TypedTask, currency: CurrencyCode) -> String {
    let parts = task
        .parts
        .iter()
        .filter(|part| !part.description.trim().is_empty())
        .count();
    let mut bits = vec![tr_with(
        messages::WORK_PART_COUNT,
        &[(
            "count",
            MessageArg::from(i64::try_from(parts).unwrap_or(i64::MAX)),
        )],
    )];
    if !task.labor_time.trim().is_empty() {
        bits.push(task.labor_time.clone());
    }
    let one = TypedWork {
        tasks: vec![task.clone()],
        ..TypedWork::default()
    };
    bits.push(crate::ui::money(total_minor(&one, currency), currency));
    bits.join(" · ")
}

/// The work log's sections.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn WorkLog(
    kind: EventKind,
    work: Signal<TypedWork>,
    vehicle: AssetView,
    requirements: Vec<RequirementView>,
    bundles: Vec<(BundleId, Bundle)>,
    /// The vehicle's specs, to pre-fill parts and remind of (#084).
    specs: Vec<(SpecId, Spec)>,
    squawks: Vec<SquawkView>,
    currency: CurrencyCode,
    work_problem: Option<String>,
    squawk_problem: Option<String>,
    end_problem: Option<(MeterId, String)>,
    editing: Option<EventId>,
    /// A new job's photos and documents, held until it's saved (phase-6.md 2b).
    held: Option<Signal<Vec<Held>>>,
    saved_tasks: usize,
    start_readings: Vec<(MeterId, String)>,
) -> Element {
    let photo_size = photo_size_for(&vehicle.asset.kind, kind);
    // A new issue from the job: whoever may open one, and not once the vehicle's disposal is
    // finalized (#067, #069).
    let auth = crate::ui::auth::use_auth();
    let may_open = vehicle.standing.takes_new_records
        && super::squawks::may_open_squawks(vehicle.role, super::squawks::current_mode(auth()));
    let mut work = work;
    let mut open = use_signal(|| Some(0_usize));
    let sheet = use_squawk_sheet();
    let typed = work();
    let free: Vec<RequirementView> = requirements
        .iter()
        .filter(|view| view.requirement.enabled)
        .filter(|view| {
            !typed
                .tasks
                .iter()
                .any(|task| task.requirement == Some(view.id))
        })
        .cloned()
        .collect();
    let (recall_free, own_free): (Vec<RequirementView>, Vec<RequirementView>) = free
        .iter()
        .cloned()
        .partition(|view| view.requirement.reference.is_some());
    let high_word = high_severity_name(&vehicle.asset.kind, domain::i18n::Translator::english());
    let titles: Vec<String> = typed.tasks.iter().map(|task| task.title.clone()).collect();
    let asset = vehicle.asset.clone();
    let asset_id = vehicle.id;
    let meters = vehicle.meters;
    let (menu_meters, bundle_meters, card_meters) =
        (meters.clone(), meters.clone(), meters.clone());
    // Each menu's closure keeps its own copy.
    let for_menu = requirements.clone();
    let for_bundles = requirements.clone();
    let (menu_specs, bundle_specs) = (specs.clone(), specs.clone());
    let menu_bundles = bundles.clone();
    rsx! {
        // Readings at the end.
        div { class: "log-item",
            if typed.show_end {
                h3 { class: "section-heading", {tr(messages::WORK_END_READINGS).to_uppercase()} }
                for (id, meter) in meters {
                    EndReading {
                        key: "{id}",
                        work,
                        id,
                        meter,
                        start: start_readings.iter().find(|(at, _)| *at == id).map(|(_, text)| text.trim().to_owned()).filter(|text| !text.is_empty()),
                        problem: end_problem.clone().filter(|(at, _)| *at == id).map(|(_, text)| text),
                    }
                }
            } else {
                button { class: "link-button", r#type: "button", onclick: move |_| work.write().show_end = true,
                    {tr(messages::WORK_ADD_END_READINGS)}
                }
            }
        }
        // Tasks.
        section { class: "log-item work-tasks",
            h3 { class: "section-heading", {tr(messages::WORK_TASKS).to_uppercase()} }
            if typed.tasks.is_empty() {
                p { class: "muted-note", {tr(messages::WORK_NO_TASKS)} }
            }
            for (index, task) in typed.tasks.iter().cloned().enumerate() {
                if open() == Some(index) {
                    TaskCard {
                        key: "{index}",
                        work,
                        index,
                        task,
                        requirements: requirements.clone(),
                        meters: card_meters.clone(),
                        specs: specs.clone(),
                        currency,
                        on_collapse: move |()| open.set(None),
                        photos: task_photos(editing, &typed, saved_tasks, index),
                        asset: vehicle.id,
                        photo_size,
                    }
                } else {
                    button {
                        key: "{index}",
                        class: "task-folded",
                        r#type: "button",
                        "aria-expanded": "false",
                        onclick: move |_| open.set(Some(index)),
                        span { class: "task-folded-title",
                            span { class: "task-toggle", "aria-hidden": "true", "▸" }
                            {folded_title(&task)}
                        }
                        span { class: "task-folded-summary", {task_summary(&task, currency)} }
                    }
                }
            }
            // New tasks go at the bottom, so these sit below the last one.
            div { class: "check-row work-adds",
                // Your own schedule's, and recalls' and bulletins', each in its own menu (the
                // human's note, milestone 9 test).
                for (items, prompt) in [(own_free, messages::WORK_FROM_REQUIREMENT), (recall_free, messages::WORK_FROM_RECALL)] {
                    if !items.is_empty() {
                        select {
                            key: "{prompt.as_str()}",
                            class: "field-input work-add",
                            onchange: {
                                let for_menu = for_menu.clone();
                                let menu_meters = menu_meters.clone();
                                let menu_specs = menu_specs.clone();
                                move |event: FormEvent| {
                                    let picked = for_menu.iter().find(|view| view.id.to_string() == event.value()).cloned();
                                    if let Some(view) = picked {
                                        let mut typed = work.write();
                                        typed.tasks.push(requirement_task(&view, &menu_specs));
                                        tick_resets(&mut typed, &view, &menu_meters);
                                        open.set(Some(typed.tasks.len() - 1));
                                    }
                                }
                            },
                            option { value: "", selected: true, {tr(prompt)} }
                            for view in items {
                                option { key: "{view.id}", value: "{view.id}", {view.requirement.name.as_str().to_owned()} }
                            }
                        }
                    }
                }
                if !bundles.is_empty() {
                    select {
                        class: "field-input work-add",
                        onchange: move |event: FormEvent| {
                            let picked = menu_bundles.iter().find(|(id, _)| id.to_string() == event.value()).map(|(_, bundle)| bundle.clone());
                            if let Some(bundle) = picked {
                                let mut typed = work.write();
                                let added = bundle_tasks(&bundle, &for_bundles, &typed.tasks, &bundle_specs);
                                for view in for_bundles.iter().filter(|view| added.iter().any(|task| task.requirement == Some(view.id))) {
                                    tick_resets(&mut typed, view, &bundle_meters);
                                }
                                typed.tasks.extend(added);
                                open.set(None);
                            }
                        },
                        option { value: "", selected: true, {tr(messages::WORK_FROM_BUNDLE)} }
                        for (id, bundle) in bundles {
                            option { key: "{id}", value: "{id}", {bundle.name.as_str().to_owned()} }
                        }
                    }
                }
                button {
                    class: "secondary-button",
                    r#type: "button",
                    onclick: move |_| {
                        let mut typed = work.write();
                        typed.tasks.push(TypedTask { parts: vec![TypedPart::default()], ..TypedTask::default() });
                        open.set(Some(typed.tasks.len() - 1));
                    },
                    {tr(messages::WORK_ADD_TASK)}
                }
            }
            FormError { message: work_problem }
            match editing {
                Some(event) => rsx! {
                    if !typed.removed_tasks.is_empty() {
                        p { class: "muted-note", {tr(messages::ATTACHMENT_SAVE_TO_SEE)} }
                    }
                    AttachmentStrip {
                        asset: vehicle.id,
                        owner: AttachmentOwner::Event { event, task: None },
                        heading: messages::ATTACHMENT_JOB_PHOTOS,
                        default_size: photo_size,
                    }
                },
                None => match held {
                    Some(held) => rsx! {
                        HeldStrip {
                            asset: vehicle.id,
                            held,
                            heading: messages::ATTACHMENT_JOB_PHOTOS,
                            default_size: photo_size,
                        }
                    },
                    None => rsx! { p { class: "muted-note", {tr(messages::ATTACHMENT_SAVE_FIRST)} } },
                },
            }
        }
        // Squawks.
        section { class: "log-item work-squawks",
            h3 { class: "section-heading",
                {tr_with(messages::WORK_SQUAWKS, &[("count", MessageArg::from(i64::try_from(squawks.len()).unwrap_or(i64::MAX)))]).to_uppercase()}
            }
            if squawks.is_empty() {
                p { class: "muted-note", {tr(messages::WORK_NO_SQUAWKS)} }
            }
            for view in squawks {
                SquawkChoice {
                    key: "{view.id}",
                    work,
                    disposition: typed.disposition(view.id).cloned().unwrap_or_else(|| TypedDisposition::undecided(view.id)),
                    view,
                    high_word: high_word.clone(),
                    tasks: titles.clone(),
                }
            }
            if may_open {
                button { class: "link-button", r#type: "button", onclick: move |_| sheet.open_new(asset_id, &asset, None),
                    {tr(messages::WORK_OPEN_SQUAWK)}
                }
            }
            FormError { message: squawk_problem }
        }
        // A repair's complaint and cause.
        if kind == EventKind::Repair {
            RepairPart { work }
        }
    }
}

/// A repair's complaint and cause.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RepairPart(work: Signal<TypedWork>) -> Element {
    let mut work = work;
    let typed = work();
    rsx! {
        section { class: "log-item",
            h3 { class: "section-heading", {tr(messages::WORK_REPAIR).to_uppercase()} }
            label { class: "field",
                span { class: "field-label", {tr(messages::WORK_COMPLAINT)} }
                textarea { class: "field-input", rows: "2", value: typed.complaint, oninput: move |event| work.write().complaint = event.value() }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::WORK_CAUSE)} }
                textarea { class: "field-input", rows: "2", value: typed.cause, oninput: move |event| work.write().cause = event.value() }
            }
            p { class: "muted-note", {tr(messages::WORK_REPAIR_NOTE)} }
        }
    }
}

/// One meter's reading at the end.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn EndReading(
    work: Signal<TypedWork>,
    id: MeterId,
    meter: Meter,
    start: Option<String>,
    problem: Option<String>,
) -> Element {
    let mut work = work;
    let text = work()
        .end_readings
        .iter()
        .find(|(have, _)| *have == id)
        .map(|(_, text)| text.clone())
        .unwrap_or_default();
    let class = if problem.is_some() {
        "field-input invalid"
    } else {
        "field-input"
    };
    rsx! {
        label { class: "field",
            span { class: "field-label",
                {tr_with(messages::WORK_END_READING, &[("meter", MessageArg::from(meter.name.as_str())), ("unit", MessageArg::from(unit_abbr(meter.unit)))])}
            }
            input {
                class,
                inputmode: "decimal",
                autocomplete: "off",
                value: text,
                oninput: move |event| set_end_reading(&mut work.write(), id, event.value()),
            }
            if let Some(start) = start {
                button {
                    class: "link-button copy-start",
                    r#type: "button",
                    onclick: move |_| set_end_reading(&mut work.write(), id, start.clone()),
                    {tr(messages::WORK_COPY_FROM_START)}
                }
            }
        }
        FormError { message: problem }
    }
}

/// Sets meter `id`'s end reading to `text`.
fn set_end_reading(work: &mut TypedWork, id: MeterId, text: String) {
    match work.end_readings.iter_mut().find(|(have, _)| *have == id) {
        Some(entry) => entry.1 = text,
        None => work.end_readings.push((id, text)),
    }
}

/// The open task: its title and requirement, parts, labor, what it resets from, its next due
/// date (a fixed-date rule), and notes.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn TaskCard(
    work: Signal<TypedWork>,
    index: usize,
    task: TypedTask,
    requirements: Vec<RequirementView>,
    /// The vehicle's meters, for the resets a task offers.
    meters: Vec<(MeterId, Meter)>,
    /// The vehicle's specs, for the reminders a task shows (#084).
    specs: Vec<(SpecId, Spec)>,
    currency: CurrencyCode,
    on_collapse: EventHandler<()>,
    photos: Option<AttachmentOwner>,
    asset: AssetId,
    photo_size: PhotoSize,
) -> Element {
    let mut work = work;
    let mut edit = move |change: &dyn Fn(&mut TypedTask)| {
        if let Some(task) = work.write().tasks.get_mut(index) {
            change(task);
        }
    };
    let requirement = task
        .requirement
        .and_then(|id| requirements.iter().find(|view| view.id == id))
        .cloned();
    // Its torque, pressure, and length specs, as reminders (#084).
    let reminders = requirement
        .as_ref()
        .map(|view| reminder_lines(view, &specs))
        .unwrap_or_default();
    // The resettable meters its requirement counts on, e.g. Oil life (#082).
    let resets: Vec<(MeterId, Meter, bool)> = requirement
        .as_ref()
        .map(|view| reset_meters(view, &meters))
        .unwrap_or_default()
        .into_iter()
        .map(|(id, meter)| {
            let ticked = work().resets.contains(&id);
            (id, meter, ticked)
        })
        .collect();
    let fixed_date = requirement.as_ref().is_some_and(|view| {
        view.requirement
            .rules
            .iter()
            .any(|rule| matches!(rule, IntervalRule::FixedDate { .. }))
    });
    let one = TypedWork {
        tasks: vec![task.clone()],
        ..TypedWork::default()
    };
    let total = crate::ui::money(total_minor(&one, currency), currency);
    rsx! {
        div { class: "task-card",
            div { class: "task-card-head",
                // The triangle folds it (as ▸ on a folded task opens it).
                button {
                    class: "task-toggle",
                    r#type: "button",
                    "aria-label": tr(messages::WORK_FOLD_TASK),
                    "aria-expanded": "true",
                    onclick: move |_| on_collapse.call(()),
                    "▾"
                }
                input {
                    class: "field-input task-title",
                    placeholder: tr(messages::WORK_TASK_TITLE),
                    value: task.title.clone(),
                    maxlength: "40",
                    oninput: move |event| edit(&|task: &mut TypedTask| task.title = event.value()),
                }
                button {
                    class: "link-button rule-remove",
                    r#type: "button",
                    "aria-label": tr(messages::ACTION_REMOVE),
                    onclick: move |_| remove_task(&mut work.write(), index),
                    "✕"
                }
            }
            if let Some(view) = &requirement {
                p { class: "muted-note", {tr_with(messages::WORK_TASK_FOR, &[("name", MessageArg::from(view.requirement.name.as_str()))])} }
            }
            span { class: "field-label", {tr(messages::WORK_PARTS)} }
            div { class: "parts-table",
                span { class: "parts-head", {tr(messages::WORK_PART_WHAT)} }
                span { class: "parts-head", {tr(messages::WORK_PART_NUMBER)} }
                span { class: "parts-head", {tr(messages::WORK_PART_QUANTITY)} }
                span { class: "parts-head", {tr(messages::WORK_PART_PRICE)} }
                span { class: "parts-head" }
                for (row, part) in task.parts.iter().cloned().enumerate() {
                    PartRow { key: "{row}", work, index, row, part, currency }
                }
            }
            button {
                class: "link-button",
                r#type: "button",
                onclick: move |_| edit(&|task: &mut TypedTask| task.parts.push(TypedPart::default())),
                {tr(messages::WORK_ADD_PART)}
            }
            div { class: "field-row",
                label { class: "field",
                    span { class: "field-label", {tr(messages::WORK_LABOR_TIME)} }
                    input {
                        class: "field-input",
                        inputmode: "numeric",
                        placeholder: "0:00",
                        value: task.labor_time.clone(),
                        oninput: move |event| edit(&|task: &mut TypedTask| task.labor_time = duration_as_typed(&event.value())),
                    }
                }
                label { class: "field",
                    span { class: "field-label", {tr_with(messages::WORK_LABOR_COST, &[("currency", MessageArg::from(currency.as_str()))])} }
                    MoneyInput {
                        currency,
                        value: task.labor_cost.clone(),
                        on_input: move |text: String| edit(&|task: &mut TypedTask| task.labor_cost.clone_from(&text)),
                    }
                }
            }
            if requirement.is_some() {
                div { class: "field",
                    span { class: "field-label", {tr(messages::WORK_RESETS_FROM)} }
                    div { class: "choice-buttons", role: "group",
                        button {
                            r#type: "button",
                            class: if task.from_end { "choice" } else { "choice on" },
                            onclick: move |_| edit(&|task: &mut TypedTask| task.from_end = false),
                            {tr(messages::WORK_RESETS_START)}
                        }
                        button {
                            r#type: "button",
                            class: if task.from_end { "choice on" } else { "choice" },
                            onclick: move |_| {
                                edit(&|task: &mut TypedTask| task.from_end = true);
                                work.write().show_end = true;
                            },
                            {tr(messages::WORK_RESETS_END)}
                        }
                    }
                }
            }
            for (id, meter, ticked) in resets {
                label { key: "{id}", class: "switch task-reset",
                    input {
                        r#type: "checkbox",
                        checked: ticked,
                        onchange: move |event| {
                            let mut typed = work.write();
                            typed.resets.retain(|have| *have != id);
                            if event.checked() {
                                typed.resets.push(id);
                            }
                        },
                    }
                    span { {reset_label(&meter)} }
                }
            }
            for (number, line) in reminders.into_iter().enumerate() {
                p { key: "reminder-{number}", class: "muted-note task-reminder", {line} }
            }
            if fixed_date {
                label { class: "field",
                    span { class: "field-label", {tr(messages::WORK_NEXT_DUE)} }
                    input {
                        class: "field-input",
                        r#type: "date",
                        value: task.next_due.clone(),
                        oninput: move |event| edit(&|task: &mut TypedTask| task.next_due = event.value()),
                    }
                }
            }
            label { class: "field",
                span { class: "field-label", {tr(messages::LOG_NOTES)} }
                input {
                    class: "field-input",
                    value: task.notes,
                    oninput: move |event| edit(&|task: &mut TypedTask| task.notes = event.value()),
                }
            }
            p { class: "task-card-total", {tr_with(messages::WORK_TASK_TOTAL, &[("total", MessageArg::from(total))])} }
            if let Some(owner) = photos {
                AttachmentStrip { asset, owner, heading: messages::ATTACHMENT_TASK_PHOTOS, default_size: photo_size }
            }
        }
    }
}

/// Where a task card's photos are, if they can be shown now: only on a saved draft, for a task it
/// had when saved, and while no task has been removed since (their places would be off until
/// it's saved again).
fn task_photos(
    editing: Option<EventId>,
    typed: &TypedWork,
    saved_tasks: usize,
    index: usize,
) -> Option<AttachmentOwner> {
    let event = editing?;
    (typed.removed_tasks.is_empty() && index < saved_tasks).then(|| AttachmentOwner::Event {
        event,
        task: u16::try_from(index).ok(),
    })
}

/// One part's row.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PartRow(
    work: Signal<TypedWork>,
    index: usize,
    row: usize,
    part: TypedPart,
    currency: CurrencyCode,
) -> Element {
    let mut work = work;
    let mut edit = move |change: &dyn Fn(&mut TypedPart)| {
        if let Some(part) = work
            .write()
            .tasks
            .get_mut(index)
            .and_then(|task| task.parts.get_mut(row))
        {
            change(part);
        }
    };
    rsx! {
        // Each field's label shows on a phone, where the table's headings are hidden.
        label { class: "part-field",
            span { class: "part-label", {tr(messages::WORK_PART_WHAT)} }
            input { class: "field-input", "aria-label": tr(messages::WORK_PART_WHAT), value: part.description.clone(), maxlength: "100",
                oninput: move |event| edit(&|part: &mut TypedPart| part.description = event.value()) }
        }
        label { class: "part-field",
            span { class: "part-label", {tr(messages::WORK_PART_NUMBER)} }
            input { class: "field-input", "aria-label": tr(messages::WORK_PART_NUMBER), value: part.part_number.clone(), maxlength: "40",
                oninput: move |event| edit(&|part: &mut TypedPart| part.part_number = event.value()) }
        }
        label { class: "part-field",
            span { class: "part-label", {tr(messages::WORK_PART_QUANTITY)} }
            input { class: "field-input", "aria-label": tr(messages::WORK_PART_QUANTITY), inputmode: "decimal", placeholder: "1", value: part.quantity,
                oninput: move |event| edit(&|part: &mut TypedPart| part.quantity = event.value()) }
        }
        label { class: "part-field",
            span { class: "part-label", {tr(messages::WORK_PART_PRICE)} }
            MoneyInput {
                currency,
                value: part.unit_price,
                label: tr(messages::WORK_PART_PRICE),
                on_input: move |text: String| edit(&|part: &mut TypedPart| part.unit_price.clone_from(&text)),
            }
        }
        button {
            class: "link-button rule-remove",
            r#type: "button",
            "aria-label": tr(messages::ACTION_REMOVE),
            onclick: move |_| {
                if let Some(task) = work.write().tasks.get_mut(index)
                    && row < task.parts.len()
                {
                    task.parts.remove(row);
                }
            },
            "✕"
        }
    }
}

/// One squawk: sign it off (with the task that fixed it, and notes) or defer it (with a
/// reason), and a new severity.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn SquawkChoice(
    work: Signal<TypedWork>,
    disposition: TypedDisposition,
    view: SquawkView,
    high_word: String,
    tasks: Vec<String>,
) -> Element {
    let mut work = work;
    let squawk = view.id;
    let mut edit = move |change: &dyn Fn(&mut TypedDisposition)| {
        let mut typed = work.write();
        if !typed.dispositions.iter().any(|each| each.squawk == squawk) {
            typed.dispositions.push(TypedDisposition::undecided(squawk));
        }
        if let Some(each) = typed
            .dispositions
            .iter_mut()
            .find(|each| each.squawk == squawk)
        {
            change(each);
        }
    };
    let class = match disposition.action {
        Action::Undecided => "squawk-choice undecided",
        Action::SignOff => "squawk-choice signed-off",
        Action::Defer => "squawk-choice deferred",
    };
    let action = disposition.action;
    rsx! {
        div { class,
            div { class: "squawk-row-top",
                SeverityChip { severity: view.severity, high_word }
                span { class: "squawk-row-text", {view.squawk.description.as_str().to_owned()} }
            }
            div { class: "choice-buttons", role: "group",
                button {
                    r#type: "button",
                    class: if action == Action::SignOff { "choice on" } else { "choice" },
                    onclick: move |_| edit(&|each: &mut TypedDisposition| each.action = if each.action == Action::SignOff { Action::Undecided } else { Action::SignOff }),
                    {tr(messages::WORK_SIGN_OFF)}
                }
                button {
                    r#type: "button",
                    class: if action == Action::Defer { "choice on" } else { "choice" },
                    onclick: move |_| edit(&|each: &mut TypedDisposition| each.action = if each.action == Action::Defer { Action::Undecided } else { Action::Defer }),
                    {tr(messages::WORK_DEFER)}
                }
            }
            match action {
                Action::Undecided => rsx! {},
                Action::SignOff => rsx! {
                    div { class: "field-row",
                        label { class: "field",
                            span { class: "field-label", {tr(messages::WORK_FIXED_BY)} }
                            select {
                                class: "field-input",
                                onchange: move |event| {
                                    let task = event.value().parse::<usize>().ok();
                                    edit(&|each: &mut TypedDisposition| each.task = task);
                                },
                                option { value: "", selected: disposition.task.is_none(), {tr(messages::WORK_FIXED_BY_NONE)} }
                                for (index, title) in tasks.iter().cloned().enumerate() {
                                    option { key: "{index}", value: "{index}", selected: disposition.task == Some(index), {title} }
                                }
                            }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::LOG_NOTES)} }
                            input { class: "field-input", value: disposition.notes,
                                oninput: move |event| edit(&|each: &mut TypedDisposition| each.notes = event.value()) }
                        }
                    }
                },
                Action::Defer => rsx! {
                    div { class: "field-row",
                        label { class: "field",
                            span { class: "field-label", {tr(messages::WORK_DEFER_WHY)} }
                            input { class: "field-input", value: disposition.reason.clone(),
                                oninput: move |event| edit(&|each: &mut TypedDisposition| each.reason = event.value()) }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::SQUAWK_SEVERITY_LABEL)} }
                            select {
                                class: "field-input",
                                onchange: move |event| {
                                    let severity = Severity::ALL.into_iter().find(|each| format!("{each:?}") == event.value());
                                    edit(&|each: &mut TypedDisposition| each.severity = severity);
                                },
                                option { value: "", selected: disposition.severity.is_none(), {tr(messages::WORK_SEVERITY_SAME)} }
                                for each in Severity::ALL {
                                    option { key: "{each:?}", value: "{each:?}", selected: disposition.severity == Some(each), {tr(each.label())} }
                                }
                            }
                        }
                    }
                },
            }
        }
    }
}

/// The squawks the job deals with: those outstanding on the vehicle once `leave_out`'s
/// dispositions are left out (the job itself, and the one it corrects, as the server counts
/// them), High first, then the oldest.
pub(super) fn job_squawks(all: &[SquawkView], leave_out: &[EventId]) -> Vec<SquawkView> {
    let mut shown: Vec<SquawkView> = all
        .iter()
        .filter(|view| {
            let counted: Vec<CountedDisposition> = view
                .history
                .iter()
                .filter(|counted| !leave_out.contains(&counted.event))
                .cloned()
                .collect();
            status(&counted).is_outstanding()
        })
        .cloned()
        .collect();
    shown.sort_by(|a, b| {
        b.severity
            .cmp(&a.severity)
            .then(a.squawk.opened_at.cmp(&b.squawk.opened_at))
    });
    shown
}

/// The squawks Finish needs decided: every one the job deals with.
pub(super) fn job_squawk_ids(shown: &[SquawkView]) -> Vec<SquawkId> {
    shown.iter().map(|view| view.id).collect()
}

#[cfg(test)]
mod tests {
    use domain::label::Label;
    use domain::notes::Notes;
    use domain::requirement::Requirement;
    use domain::test_support::test_id;

    use super::*;

    fn requirement(n: u32, name: &str) -> RequirementView {
        RequirementView {
            id: test_id(n),
            requirement: Requirement {
                asset: test_id(10),
                name: Label::parse(name).unwrap(),
                notes: Notes::default(),
                enabled: true,
                rules: vec![],
                reference: None,
                mode: domain::requirement::ComplianceMode::Recurring,
                applicability: domain::requirement::Applicability::Applies,
                before_records: None,
                warnings: None,
                specs: vec![],
                usages: None,
            },
            last_done: None,
            status: domain::requirement::RequirementStatus::Active,
            due: None,
        }
    }

    #[test]
    fn a_bundle_adds_a_task_for_each_requirement_not_there_yet() {
        let requirements = [
            requirement(40, "Engine oil & filter"),
            requirement(41, "Rotate tires"),
        ];
        let bundle = Bundle {
            asset: test_id(10),
            name: Label::parse("5,000-mile service").unwrap(),
            requirements: vec![test_id(41), test_id(40)],
        };
        let already = [requirement_task(&requirements[0], &[])];
        let added = bundle_tasks(&bundle, &requirements, &already, &[]);
        assert_eq!(added.len(), 1);
        assert_eq!(
            (added[0].title.as_str(), added[0].requirement),
            ("Rotate tires", Some(test_id(41)))
        );
    }

    #[test]
    fn a_task_is_pre_filled_from_its_requirements_specs() {
        let spec =
            |name: &str, amount: Option<(&str, domain::spec::SpecUnit)>, number: Option<&str>| {
                Spec {
                    asset: test_id(10),
                    name: Label::parse(name).unwrap(),
                    amount: amount.map(|(text, unit)| SpecAmount::parse(text, unit).unwrap()),
                    detail: None,
                    part_number: number.map(str::to_owned),
                    notes: Notes::default(),
                }
            };
        let specs = vec![
            (
                test_id(30),
                spec(
                    "Engine oil",
                    Some(("5.5", domain::spec::SpecUnit::Quarts)),
                    None,
                ),
            ),
            (test_id(31), spec("Oil filter", None, Some("FL-500S"))),
            (
                test_id(32),
                spec(
                    "Drain plug torque",
                    Some(("30", domain::spec::SpecUnit::FootPounds)),
                    None,
                ),
            ),
        ];
        let mut oil = requirement(40, "Engine oil & filter");
        oil.requirement.specs = vec![test_id(30), test_id(31), test_id(32)];
        let task = requirement_task(&oil, &specs);
        let parts: Vec<(&str, &str, &str, Option<SpecId>)> = task
            .parts
            .iter()
            .map(|part| {
                (
                    part.description.as_str(),
                    part.part_number.as_str(),
                    part.quantity.as_str(),
                    part.spec,
                )
            })
            .collect();
        assert_eq!(
            parts,
            vec![
                ("Engine oil (qt)", "", "5.5", Some(test_id(30))),
                ("Oil filter", "FL-500S", "1", Some(test_id(31))),
            ]
        );
        assert_eq!(
            reminder_lines(&oil, &specs),
            ["Reminder: Drain plug torque 30 ft-lb (41 N·m)"]
        );
        // With no specs, one empty row to type in.
        let bare = requirement_task(&requirement(41, "Rotate tires"), &specs);
        assert_eq!(bare.parts, vec![TypedPart::default()]);
    }

    #[test]
    fn a_job_deals_with_the_squawks_outstanding_without_itself_and_its_original() {
        use domain::squawk::{Disposition, SquawkDisposition, SquawkStatus, SquawkText};
        use domain::time::UnixMillis;
        let view = |n: u32, history: Vec<(u32, bool)>| SquawkView {
            id: test_id(n),
            squawk: domain::squawk::Squawk {
                asset: test_id(10),
                description: SquawkText::parse("Rattle").unwrap(),
                notes: Notes::default(),
                severity: Severity::Low,
                opened_at: UnixMillis::from_millis(i64::from(n)),
                opened_by: test_id(1),
                found_on: None,
                received_at: UnixMillis::from_millis(0),
            },
            status: SquawkStatus::Open,
            severity: Severity::Low,
            history: history
                .into_iter()
                .map(|(event, signed)| CountedDisposition {
                    at: UnixMillis::from_millis(i64::from(event)),
                    event: test_id(event),
                    disposition: SquawkDisposition {
                        squawk: test_id(n),
                        action: if signed {
                            Disposition::SignedOff {
                                notes: Notes::default(),
                                task: None,
                            }
                        } else {
                            Disposition::Deferred {
                                reason: domain::event::Reason::parse("Later").unwrap(),
                            }
                        },
                        severity: None,
                    },
                })
                .collect(),
            opened_by_name: String::new(),
            may_change: false,
        };
        let all = [
            // Signed off by the job being corrected (100): outstanding for its correction.
            view(60, vec![(100, true)]),
            // Deferred by 100, then signed off by a later repair (200): not the correction's.
            view(61, vec![(100, false), (200, true)]),
            // Never dealt with.
            view(62, vec![]),
        ];
        let shown: Vec<SquawkId> = job_squawks(&all, &[test_id(100)])
            .iter()
            .map(|v| v.id)
            .collect();
        assert_eq!(shown, vec![test_id(60), test_id(62)]);
        assert_eq!(
            job_squawks(&all, &[]).len(),
            1,
            "a new job: only the one never dealt with"
        );
    }

    #[test]
    fn removing_a_task_keeps_the_sign_offs_right() {
        let mut work = TypedWork {
            tasks: vec![
                TypedTask::default(),
                TypedTask::default(),
                TypedTask::default(),
            ],
            dispositions: vec![
                TypedDisposition {
                    task: Some(0),
                    ..TypedDisposition::undecided(test_id(60))
                },
                TypedDisposition {
                    task: Some(1),
                    ..TypedDisposition::undecided(test_id(61))
                },
                TypedDisposition {
                    task: Some(2),
                    ..TypedDisposition::undecided(test_id(62))
                },
            ],
            ..TypedWork::default()
        };
        remove_task(&mut work, 1);
        let tasks: Vec<Option<usize>> = work.dispositions.iter().map(|each| each.task).collect();
        assert_eq!(tasks, vec![Some(0), None, Some(1)]);
    }
}
