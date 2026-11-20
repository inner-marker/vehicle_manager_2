//! "Ready to use?" on screen (Phase 5 milestone 6; decision #086; sketch
//! `docs/ui-sketches/p5m6-readiness-options.svg`, 1A, 2A, 3A, 4A).
//!
//! - **1A, the Overview's card:** a chip per usage (the default first), the chosen usage's
//!   headline ("✗ Not drivable: 2 things"), then every item with ✓, !, or ✗, its reason, and
//!   a link; and that it's advisory.
//! - **2A, the Garage chip:** the default usage's answer in words ("✗ Not drivable",
//!   "! Drivable", "✓ Towable").
//! - **3A, the Usages section** in Maintenance → Set up, with its dialog (a name and the
//!   document types it needs).
//! - **4A, Needed for** in the requirement dialog.
//!
//! Colour is never the only sign: every mark has its symbol and its words.

use dioxus::prelude::*;
use domain::api::{AssetView, ReadinessOutcome, ScheduleOutcome};
use domain::document::DocumentType;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;
use domain::log_area;
use domain::readiness::{ItemReason, Mark, Readiness, ReadinessItem};
use domain::usage::{Usage, UsageId};

use super::documents::type_name;
use super::problem_from_error;
use super::vehicle_setup::SectionHeading;
use crate::ui::routes::Route;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// The class that colours a mark.
pub(super) const fn mark_class(mark: Mark) -> &'static str {
    match mark {
        Mark::Ready => "ready-ok",
        Mark::Warning => "ready-warn",
        Mark::NotReady => "ready-no",
    }
}

/// A mark's symbol.
pub(super) const fn mark_symbol(mark: Mark) -> &'static str {
    match mark {
        Mark::Ready => "✓",
        Mark::Warning => "!",
        Mark::NotReady => "✗",
    }
}

/// A usage's answer in words: "Drivable", or "Not drivable".
pub(super) fn answer(name: &Label, mark: Mark) -> String {
    match mark {
        Mark::Ready | Mark::Warning => name.as_str().to_owned(),
        Mark::NotReady => tr_with(
            messages::READY_NOT,
            &[(
                "usage",
                MessageArg::from(name.as_str().to_lowercase().as_str()),
            )],
        ),
    }
}

/// 2A: the Garage card's chip for its default usage.
#[component]
pub(super) fn ReadyChip(name: Label, mark: Mark) -> Element {
    rsx! {
        span { class: "status-chip ready-chip {mark_class(mark)}",
            {format!("{} {}", mark_symbol(mark), answer(&name, mark))}
        }
    }
}

/// The headline: "✗ Not drivable: 2 things", "! Drivable: 1 thing due soon", "✓ Drivable".
pub(super) fn headline(readiness: &Readiness) -> String {
    let count = readiness
        .items
        .iter()
        .filter(|item| item.mark == readiness.mark)
        .count();
    let count = MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX));
    let name = answer(&readiness.name, readiness.mark);
    let text = match readiness.mark {
        Mark::Ready => name,
        Mark::Warning => tr_with(
            messages::READY_WARNINGS,
            &[("usage", MessageArg::from(name.as_str())), ("count", count)],
        ),
        Mark::NotReady => tr_with(
            messages::READY_BLOCKERS,
            &[("usage", MessageArg::from(name.as_str())), ("count", count)],
        ),
    };
    format!("{} {text}", mark_symbol(readiness.mark))
}

/// An item's name, its reason, and where its link goes.
fn item_lines(
    item: &ReadinessItem,
    vehicle: &AssetView,
    units: &domain::preferences::Preferences,
) -> (String, String, Route) {
    let tab = |tab: &str| Route::VehicleTab {
        asset_id: vehicle.id.to_string(),
        tab: tab.to_owned(),
    };
    match &item.reason {
        ItemReason::Requirement { name, due, .. } => (
            name.as_str().to_owned(),
            super::due_view::left_text(due, &vehicle.meters, units),
            tab("maintenance"),
        ),
        ItemReason::Document { doc_type, due, .. } => (
            type_name(doc_type),
            due.as_ref().map_or_else(
                || tr(messages::READY_DOCUMENT_CURRENT),
                |due| super::due_view::left_text(due, &[], units),
            ),
            tab("documents"),
        ),
        ItemReason::MissingDocument { doc_type } => (
            type_name(doc_type),
            tr(messages::READY_DOCUMENT_MISSING),
            tab("documents"),
        ),
        ItemReason::HighIssue {
            id,
            description,
            deferred,
        } => (
            description.clone(),
            tr(if *deferred {
                messages::READY_ISSUE_DEFERRED
            } else {
                messages::READY_ISSUE_OPEN
            }),
            Route::SquawkDetail {
                squawk_id: id.to_string(),
            },
        ),
    }
}

/// 1A: the Overview's **Ready to use?**
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ReadyCard(vehicle: AssetView) -> Element {
    let units = use_units();
    let asset = vehicle.id;
    let loaded = use_resource(move || async move { data::vehicle_readiness(asset).await });
    let mut chosen = use_signal(|| None::<UsageId>);
    let usages = match &*loaded.read() {
        Some(Ok(ReadinessOutcome::Readiness { usages })) => usages.clone(),
        _ => return rsx! {},
    };
    let Some(shown) = usages
        .iter()
        .find(|usage| Some(usage.usage) == chosen())
        .or_else(|| usages.first())
        .cloned()
    else {
        return rsx! {};
    };
    rsx! {
        section { class: "vehicle-panel wide ready-card",
            h3 { class: "section-heading", {tr(messages::PANEL_READY).to_uppercase()} }
            if usages.len() > 1 {
                div { class: "filter-chips", role: "group",
                    for usage in usages.iter() {
                        button {
                            key: "{usage.usage}",
                            class: if usage.usage == shown.usage { "chip-filter on" } else { "chip-filter" },
                            "aria-pressed": usage.usage == shown.usage,
                            onclick: { let id = usage.usage; move |_| chosen.set(Some(id)) },
                            {format!("{} {}", mark_symbol(usage.mark), usage.name.as_str())}
                        }
                    }
                }
            }
            p { class: "ready-headline {mark_class(shown.mark)}", {headline(&shown)} }
            ul { class: "ready-items",
                for (index, item) in shown.items.iter().enumerate() {
                    {
                        let (name, reason, to) = item_lines(item, &vehicle, &units());
                        rsx! {
                            li { key: "{index}",
                                span { class: "ready-mark {mark_class(item.mark)}", "aria-hidden": "true", {mark_symbol(item.mark)} }
                                Link { class: "ready-item", to,
                                    span { class: "ready-item-name", {name} }
                                    span { class: "ready-item-reason", {reason} }
                                }
                            }
                        }
                    }
                }
            }
            if shown.items.is_empty() {
                p { class: "muted-note", {tr(messages::READY_NOTHING)} }
            }
            p { class: "muted-note", {tr(messages::READY_ADVISORY)} }
        }
    }
}

/// 4A: **Needed for**, in the requirement dialog. `chosen` is `None` until something is
/// chosen (the default usage counts it), then the ticked usages, perhaps none.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn NeededFor(
    usages: Vec<(UsageId, Usage)>,
    chosen: Signal<Option<Vec<UsageId>>>,
) -> Element {
    let mut chosen = chosen;
    let ticked = |id: UsageId, default: bool| chosen().map_or(default, |list| list.contains(&id));
    let all = usages.clone();
    rsx! {
        fieldset { class: "spec-checklist",
            legend { class: "field-label", {tr(messages::USAGE_NEEDED_FOR)} }
            for (id, usage) in usages {
                label { key: "{id}", class: "switch",
                    input {
                        r#type: "checkbox",
                        checked: ticked(id, usage.default),
                        onchange: {
                            let all = all.clone();
                            move |event: FormEvent| {
                                // The first change turns "the default" into a list of its own.
                                let mut list = chosen().unwrap_or_else(|| {
                                    all.iter().filter(|(_, usage)| usage.default).map(|(id, _)| *id).collect()
                                });
                                list.retain(|have| *have != id);
                                if event.checked() {
                                    list.push(id);
                                }
                                chosen.set(Some(list));
                            }
                        },
                    }
                    span { {usage.name.as_str().to_owned()} }
                }
            }
            p { class: "muted-note", {tr(messages::USAGE_NEEDED_FOR_NOTE)} }
        }
    }
}

/// The document types a usage can need: the kind's profile's, and any it already lists.
fn document_choices(vehicle: &AssetView, listed: &[DocumentType]) -> Vec<DocumentType> {
    let mut choices: Vec<DocumentType> = domain::profile::profile(&vehicle.asset.kind)
        .map(|profile| {
            profile
                .documents
                .iter()
                .filter_map(|key| DocumentType::listed(key).ok())
                .collect()
        })
        .unwrap_or_default();
    for document in listed {
        if !choices.contains(document) {
            choices.push(document.clone());
        }
    }
    choices
}

/// "Needs: Registration, Insurance", or that it needs no documents.
fn needs_text(usage: &Usage) -> String {
    if usage.documents.is_empty() {
        return tr(messages::USAGE_NEEDS_NOTHING);
    }
    let names: Vec<String> = usage.documents.iter().map(type_name).collect();
    tr_with(
        messages::USAGE_NEEDS,
        &[("documents", MessageArg::from(names.join(", ").as_str()))],
    )
}

/// 3A: the Usages section in Set up.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn UsagesSection(
    vehicle: AssetView,
    usages: Vec<(UsageId, Usage)>,
    reloads: Signal<u32>,
) -> Element {
    let mut editing = use_signal(|| None::<Option<(UsageId, Usage)>>);
    rsx! {
        section { class: "setup-section",
            SectionHeading { title: messages::USAGE_SECTION,
                button { class: "secondary-button", onclick: move |_| editing.set(Some(None)), {tr(messages::USAGE_ADD)} }
            }
            for (id, usage) in usages {
                div { key: "{id}", class: "setup-row",
                    div { class: "setup-row-text",
                        span { class: "setup-row-name",
                            if usage.default {
                                {tr_with(messages::USAGE_DEFAULT_NAME, &[("name", MessageArg::from(usage.name.as_str()))])}
                            } else {
                                {usage.name.as_str().to_owned()}
                            }
                        }
                        span { class: "setup-row-detail", {needs_text(&usage)} }
                    }
                    div { class: "setup-row-buttons",
                        button {
                            class: "secondary-button",
                            onclick: move |_| editing.set(Some(Some((id, usage.clone())))),
                            {tr(messages::ACTION_EDIT)}
                        }
                    }
                }
            }
            p { class: "muted-note", {tr(messages::USAGE_SECTION_NOTE)} }
        }
        if let Some(existing) = editing() {
            UsageDialog { vehicle, existing, reloads, on_close: move |()| editing.set(None) }
        }
    }
}

/// Add or change a usage, and remove one (not the default).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn UsageDialog(
    vehicle: AssetView,
    existing: Option<(UsageId, Usage)>,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let start = existing.clone();
    let mut name = use_signal(|| {
        start
            .as_ref()
            .map(|(_, usage)| usage.name.as_str().to_owned())
            .unwrap_or_default()
    });
    let start = existing.clone();
    let mut documents = use_signal(|| start.map(|(_, usage)| usage.documents).unwrap_or_default());
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let editing_id = existing.as_ref().map(|(id, _)| *id);
    let is_default = existing.as_ref().is_some_and(|(_, usage)| usage.default);
    let choices = document_choices(&vehicle, &documents());
    let asset = vehicle.id;
    let mut finish = move |result: Result<ScheduleOutcome, ServerFnError>| match result {
        Ok(ScheduleOutcome::SavedUsage { .. } | ScheduleOutcome::Done) => {
            reloads += 1;
            on_close.call(());
        }
        Ok(ScheduleOutcome::InvalidUsage { problem: refused }) => {
            problem.set(Some(refused.message()));
        }
        Ok(ScheduleOutcome::NotAllowed) => problem.set(Some(messages::VEHICLES_NOT_ALLOWED)),
        Ok(other) => {
            tracing::debug!(target: log_area::UI, ?other, "usage: unexpected answer");
            problem.set(Some(messages::SOMETHING_WENT_WRONG));
        }
        Err(error) => problem.set(Some(problem_from_error(&error).message())),
    };
    let save = move |event: FormEvent| {
        event.prevent_default();
        let label = match Label::parse(&name()) {
            Ok(label) => label,
            Err(error) => {
                problem.set(Some(error.message()));
                return;
            }
        };
        let usage = Usage {
            asset,
            name: label,
            default: is_default,
            documents: documents(),
        };
        busy.set(true);
        spawn(async move {
            let result = match editing_id {
                Some(id) => api::edit_usage(id, usage).await,
                None => api::add_usage(usage).await,
            };
            busy.set(false);
            finish(result);
        });
    };
    let remove = move |_| {
        let Some(id) = editing_id else { return };
        busy.set(true);
        spawn(async move {
            let result = api::delete_usage(id).await;
            busy.set(false);
            finish(result);
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
                h2 { class: "dialog-title", {tr(if editing_id.is_some() { messages::USAGE_EDIT_TITLE } else { messages::USAGE_ADD_TITLE })} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::USAGE_NAME)} }
                    input { class: "field-input", value: name(), maxlength: "40", oninput: move |event| name.set(event.value()) }
                }
                fieldset { class: "spec-checklist",
                    legend { class: "field-label", {tr(messages::USAGE_DOCUMENTS)} }
                    for document in choices {
                        label { key: "{type_name(&document)}", class: "switch",
                            input {
                                r#type: "checkbox",
                                checked: documents().contains(&document),
                                onchange: {
                                    let document = document.clone();
                                    move |event: FormEvent| {
                                        let mut list = documents.write();
                                        list.retain(|have| *have != document);
                                        if event.checked() {
                                            list.push(document.clone());
                                        }
                                    }
                                },
                            }
                            span { {type_name(&document)} }
                        }
                    }
                    p { class: "muted-note", {tr(messages::USAGE_DOCUMENTS_NOTE)} }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "primary-button", r#type: "submit", disabled: busy(), {tr(messages::ACTION_SAVE)} }
                    if editing_id.is_some() && !is_default {
                        button { class: "secondary-button", r#type: "button", disabled: busy(), onclick: remove, {tr(messages::ACTION_REMOVE)} }
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_usages_answer_reads_as_words() {
        let drivable = Label::parse("Drivable").unwrap();
        assert_eq!(answer(&drivable, Mark::Ready), "Drivable");
        assert_eq!(answer(&drivable, Mark::Warning), "Drivable");
        assert_eq!(answer(&drivable, Mark::NotReady), "Not drivable");
        for mark in [Mark::Ready, Mark::Warning, Mark::NotReady] {
            assert!(!mark_symbol(mark).is_empty());
            assert!(mark_class(mark).starts_with("ready-"));
        }
    }

    #[test]
    fn the_headline_counts_what_sets_the_answer() {
        use domain::test_support::test_id;
        let item = |mark| ReadinessItem {
            mark,
            reason: ItemReason::MissingDocument {
                doc_type: DocumentType::listed("document-insurance").unwrap(),
            },
        };
        let readiness = |mark, items| Readiness {
            usage: test_id(1),
            name: Label::parse("Drivable").unwrap(),
            default: true,
            mark,
            items,
        };
        assert_eq!(
            headline(&readiness(
                Mark::NotReady,
                vec![
                    item(Mark::NotReady),
                    item(Mark::NotReady),
                    item(Mark::Warning)
                ]
            )),
            "✗ Not drivable: 2 things"
        );
        assert_eq!(
            headline(&readiness(Mark::Warning, vec![item(Mark::Warning)])),
            "! Drivable: 1 thing due soon"
        );
        assert_eq!(headline(&readiness(Mark::Ready, vec![])), "✓ Drivable");
    }
}
