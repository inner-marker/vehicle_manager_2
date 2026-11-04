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
use domain::event::EventId;
use domain::i18n::{MessageArg, messages};
use domain::meter::{EventKind, Meter, MeterId, MeterUnit};
use domain::preferences::CurrencyCode;
use domain::profile::high_severity_name;
use domain::quantity::{duration_as_typed, show_money};
use domain::requirement::{Bundle, BundleId, IntervalRule};
use domain::squawk::{CountedDisposition, Severity, SquawkId, status};

use super::FormError;
use super::squawks::{SeverityChip, use_squawk_sheet};
use super::work_input::{Action, TypedDisposition, TypedPart, TypedTask, TypedWork, total_minor};
use crate::ui::{tr, tr_with};

/// A meter's unit, short.
fn unit_abbr(unit: MeterUnit) -> String {
    tr(match unit {
        MeterUnit::Miles => messages::UNIT_ABBR_MI,
        MeterUnit::Kilometers => messages::UNIT_ABBR_KM,
        MeterUnit::Hours => messages::UNIT_ABBR_H,
    })
}

/// The tasks for `bundle`'s requirements, in its order (those already in the list are left
/// out).
pub(super) fn bundle_tasks(
    bundle: &Bundle,
    requirements: &[RequirementView],
    already: &[TypedTask],
) -> Vec<TypedTask> {
    bundle
        .requirements
        .iter()
        .filter(|id| !already.iter().any(|task| task.requirement == Some(**id)))
        .filter_map(|id| requirements.iter().find(|view| view.id == *id))
        .map(requirement_task)
        .collect()
}

/// A new task doing `view`'s requirement, titled with its name.
pub(super) fn requirement_task(view: &RequirementView) -> TypedTask {
    TypedTask {
        title: view.requirement.name.as_str().to_owned(),
        requirement: Some(view.id),
        parts: vec![TypedPart::default()],
        ..TypedTask::default()
    }
}

/// Removes task `index`, and keeps the sign-offs naming the tasks after it right.
pub(super) fn remove_task(work: &mut TypedWork, index: usize) {
    if index >= work.tasks.len() {
        return;
    }
    work.tasks.remove(index);
    for disposition in &mut work.dispositions {
        disposition.task = match disposition.task {
            Some(task) if task == index => None,
            Some(task) if task > index => Some(task - 1),
            other => other,
        };
    }
}

/// A folded task's title: "▸ Engine oil & filter" (or that it has none yet).
fn folded_title(task: &TypedTask) -> String {
    let title = if task.title.trim().is_empty() {
        tr(messages::WORK_UNTITLED_TASK)
    } else {
        task.title.clone()
    };
    format!("▸ {title}")
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
    bits.push(show_money(total_minor(&one, currency), currency));
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
    squawks: Vec<SquawkView>,
    currency: CurrencyCode,
    work_problem: Option<String>,
    squawk_problem: Option<String>,
    end_problem: Option<(MeterId, String)>,
) -> Element {
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
    let high_word = high_severity_name(&vehicle.asset.kind, domain::i18n::Translator::english());
    let titles: Vec<String> = typed.tasks.iter().map(|task| task.title.clone()).collect();
    let asset = vehicle.asset.clone();
    let asset_id = vehicle.id;
    let meters = vehicle.meters;
    // Each menu's closure keeps its own copy.
    let for_menu = requirements.clone();
    let for_bundles = requirements.clone();
    let menu_bundles = bundles.clone();
    rsx! {
        // Readings at the end.
        div { class: "log-item",
            if typed.show_end {
                h3 { class: "section-heading", {tr(messages::WORK_END_READINGS).to_uppercase()} }
                for (id, meter) in meters {
                    EndReading { key: "{id}", work, id, meter, problem: end_problem.clone().filter(|(at, _)| *at == id).map(|(_, text)| text) }
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
            div { class: "check-row",
                if !free.is_empty() {
                    select {
                        class: "field-input work-add",
                        onchange: move |event| {
                            let picked = for_menu.iter().find(|view| view.id.to_string() == event.value()).cloned();
                            if let Some(view) = picked {
                                let mut typed = work.write();
                                typed.tasks.push(requirement_task(&view));
                                open.set(Some(typed.tasks.len() - 1));
                            }
                        },
                        option { value: "", selected: true, {tr(messages::WORK_FROM_REQUIREMENT)} }
                        for view in free {
                            option { key: "{view.id}", value: "{view.id}", {view.requirement.name.as_str().to_owned()} }
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
                                let added = bundle_tasks(&bundle, &for_bundles, &typed.tasks);
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
            if typed.tasks.is_empty() {
                p { class: "muted-note", {tr(messages::WORK_NO_TASKS)} }
            }
            for (index, task) in typed.tasks.iter().cloned().enumerate() {
                if open() == Some(index) {
                    TaskCard { key: "{index}", work, index, task, requirements: requirements.clone(), currency }
                } else {
                    button {
                        key: "{index}",
                        class: "task-folded",
                        r#type: "button",
                        onclick: move |_| open.set(Some(index)),
                        span { class: "task-folded-title", {folded_title(&task)} }
                        span { class: "task-folded-summary", {task_summary(&task, currency)} }
                    }
                }
            }
            FormError { message: work_problem }
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
            button { class: "link-button", r#type: "button", onclick: move |_| sheet.open_new(asset_id, &asset, None),
                {tr(messages::WORK_OPEN_SQUAWK)}
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
                oninput: move |event| {
                    let mut typed = work.write();
                    match typed.end_readings.iter_mut().find(|(have, _)| *have == id) {
                        Some(entry) => entry.1 = event.value(),
                        None => typed.end_readings.push((id, event.value())),
                    }
                },
            }
        }
        FormError { message: problem }
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
    currency: CurrencyCode,
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
    let total = show_money(total_minor(&one, currency), currency);
    rsx! {
        div { class: "task-card",
            div { class: "task-card-head",
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
                    PartRow { key: "{row}", work, index, row, part }
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
                    input {
                        class: "field-input",
                        inputmode: "decimal",
                        value: task.labor_cost.clone(),
                        oninput: move |event| edit(&|task: &mut TypedTask| task.labor_cost = event.value()),
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
        }
    }
}

/// One part's row.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PartRow(work: Signal<TypedWork>, index: usize, row: usize, part: TypedPart) -> Element {
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
        input { class: "field-input", "aria-label": tr(messages::WORK_PART_WHAT), value: part.description.clone(), maxlength: "100",
            oninput: move |event| edit(&|part: &mut TypedPart| part.description = event.value()) }
        input { class: "field-input", "aria-label": tr(messages::WORK_PART_NUMBER), value: part.part_number.clone(), maxlength: "40",
            oninput: move |event| edit(&|part: &mut TypedPart| part.part_number = event.value()) }
        input { class: "field-input", "aria-label": tr(messages::WORK_PART_QUANTITY), inputmode: "decimal", placeholder: "1", value: part.quantity.clone(),
            oninput: move |event| edit(&|part: &mut TypedPart| part.quantity = event.value()) }
        input { class: "field-input", "aria-label": tr(messages::WORK_PART_PRICE), inputmode: "decimal", value: part.unit_price,
            oninput: move |event| edit(&|part: &mut TypedPart| part.unit_price = event.value()) }
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
            },
            last_done: None,
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
        let already = [requirement_task(&requirements[0])];
        let added = bundle_tasks(&bundle, &requirements, &already);
        assert_eq!(added.len(), 1);
        assert_eq!(
            (added[0].title.as_str(), added[0].requirement),
            ("Rotate tires", Some(test_id(41)))
        );
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
