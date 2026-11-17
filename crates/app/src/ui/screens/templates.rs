//! Templates (Phase 5 milestone 4; decisions #031, #084; sketch
//! `docs/ui-sketches/p5m4-specs-options.svg`, 4B): the household's named schedules.
//!
//! Each template shows what it holds and **Apply to…**, listing the vehicles the person may
//! give it to (they may change its schedule, and it has none yet). Applying copies it: later
//! changes to the template don't reach the vehicle, and meter rules without a matching meter
//! are left out and listed. New templates come from a vehicle's **Set up** (**Save as
//! template…**, [`SaveTemplateDialog`]). The household's Admin, in Admin mode, renames and
//! removes them.

use dioxus::prelude::*;
use domain::api::{TemplateSummary, TemplatesOutcome, TemplatesView};
use domain::asset::AssetId;
use domain::i18n::{MessageArg, MessageId, messages};
use domain::label::Label;
use domain::log_area;
use domain::template::{TemplateCounts, TemplateId};

use super::problem_from_error;
use crate::ui::{tr, tr_with};
use crate::{api, data};

/// A count as a Fluent number.
fn count(value: usize) -> MessageArg {
    MessageArg::from(i64::try_from(value).unwrap_or(i64::MAX))
}

/// "9 requirements · 2 bundles · 6 specs".
pub(super) fn counts_text(counts: TemplateCounts) -> String {
    tr_with(
        messages::TEMPLATE_COUNTS,
        &[
            ("requirements", count(counts.requirements)),
            ("bundles", count(counts.bundles)),
            ("specs", count(counts.specs)),
        ],
    )
}

/// What applying a template did: "Copied 1 requirement, 1 bundle, and 2 specs onto Van." and
/// any meter rules left out.
fn applied_text(outcome: &TemplatesOutcome, vehicle: &str) -> Option<String> {
    let TemplatesOutcome::Applied {
        requirements,
        bundles,
        specs,
        left_out,
    } = outcome
    else {
        return None;
    };
    let mut text = tr_with(
        messages::TEMPLATE_APPLIED,
        &[
            ("vehicle", MessageArg::from(vehicle)),
            ("requirements", count(*requirements)),
            ("bundles", count(*bundles)),
            ("specs", count(*specs)),
        ],
    );
    if !left_out.is_empty() {
        let names: Vec<String> = left_out
            .iter()
            .map(|(requirement, meter)| format!("{}: {}", requirement.as_str(), meter.as_str()))
            .collect();
        text.push(' ');
        text.push_str(&tr_with(
            messages::TEMPLATE_LEFT_OUT,
            &[("list", MessageArg::from(names.join(", ").as_str()))],
        ));
    }
    Some(text)
}

/// A templates answer that isn't what was asked for, as a message.
fn refusal(outcome: &TemplatesOutcome) -> MessageId {
    match outcome {
        TemplatesOutcome::Invalid { problem } => problem.message(),
        TemplatesOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        TemplatesOutcome::NotFound => messages::TEMPLATE_NOT_FOUND,
        TemplatesOutcome::Templates { .. }
        | TemplatesOutcome::Saved { .. }
        | TemplatesOutcome::Applied { .. }
        | TemplatesOutcome::Done
        | TemplatesOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// What's open over the list.
#[derive(Debug, Clone, PartialEq)]
enum Open {
    Rename(TemplateSummary),
    Remove(TemplateSummary),
}

/// `/templates`.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Templates() -> Element {
    let mut reloads = use_signal(|| 0_u32);
    let mut done = use_signal(|| None::<String>);
    let mut problem = use_signal(|| None::<MessageId>);
    let mut open = use_signal(|| None::<Open>);
    let mut rename_problem = use_signal(|| None::<MessageId>);
    let loaded = use_resource(move || async move {
        let _ = reloads();
        data::list_templates().await
    });
    let view: TemplatesView = match &*loaded.read() {
        None => return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Ok(TemplatesOutcome::Templates { view })) => view.clone(),
        Some(Ok(other)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(refusal(other))} } };
        }
        Some(Err(error)) => {
            return rsx! { p { class: "form-error", role: "alert", {tr(problem_from_error(error).message())} } };
        }
    };
    let apply_to = view.apply_to.clone();
    let apply = move |(template, to, vehicle): (TemplateId, AssetId, String)| {
        spawn(async move {
            match api::apply_template(template, to).await {
                Ok(outcome @ TemplatesOutcome::Applied { .. }) => {
                    tracing::debug!(target: log_area::UI, %template, %to, "template applied");
                    problem.set(None);
                    done.set(applied_text(&outcome, &vehicle));
                    reloads += 1;
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    rsx! {
        div { class: "templates",
            p { class: "muted-note", {tr(messages::TEMPLATES_INTRO)} }
            if let Some(text) = done() {
                p { class: "form-note", role: "status", {text} }
            }
            if let Some(message) = problem() {
                p { class: "form-error", role: "alert", {tr(message)} }
            }
            if view.templates.is_empty() {
                p { class: "placeholder", {tr(messages::TEMPLATES_NONE)} }
            }
            for template in view.templates {
                div { key: "{template.id}", class: "setup-row template-row",
                    div { class: "setup-row-text",
                        span { class: "setup-row-name", {template.name.as_str().to_owned()} }
                        span { class: "setup-row-detail", {counts_text(template.counts)} }
                    }
                    div { class: "setup-row-buttons",
                        if !apply_to.is_empty() {
                            select {
                                class: "field-input template-apply",
                                "aria-label": tr(messages::TEMPLATE_APPLY_TO),
                                onchange: {
                                    let choices = apply_to.clone();
                                    let id = template.id;
                                    move |event: FormEvent| {
                                        let picked = choices
                                            .iter()
                                            .find(|(asset, _)| asset.to_string() == event.value())
                                            .cloned();
                                        if let Some((asset, name)) = picked {
                                            apply((id, asset, name));
                                        }
                                    }
                                },
                                option { value: "", selected: true, {tr(messages::TEMPLATE_APPLY_TO)} }
                                for (asset, name) in apply_to.clone() {
                                    option { key: "{asset}", value: "{asset}", {name} }
                                }
                            }
                        }
                        if view.may_manage {
                            button {
                                class: "secondary-button",
                                onclick: {
                                    let template = template.clone();
                                    move |_| open.set(Some(Open::Rename(template.clone())))
                                },
                                {tr(messages::TEMPLATE_RENAME)}
                            }
                            button {
                                class: "secondary-button",
                                onclick: move |_| open.set(Some(Open::Remove(template.clone()))),
                                {tr(messages::SPEC_REMOVE)}
                            }
                        }
                    }
                }
            }
        }
        match open() {
            None => rsx! {},
            Some(Open::Rename(template)) => rsx! {
                NameDialog {
                    title: tr(messages::TEMPLATE_RENAME),
                    start: template.name.as_str().to_owned(),
                    problem: rename_problem(),
                    on_save: move |name: Label| {
                        spawn(async move {
                            match api::rename_template(template.id, name).await {
                                Ok(TemplatesOutcome::Saved { .. }) => {
                                    reloads += 1;
                                    rename_problem.set(None);
                                    open.set(None);
                                }
                                Ok(other) => rename_problem.set(Some(refusal(&other))),
                                Err(error) => rename_problem.set(Some(problem_from_error(&error).message())),
                            }
                        });
                    },
                    on_close: move |()| {
                        rename_problem.set(None);
                        open.set(None);
                    },
                }
            },
            Some(Open::Remove(template)) => rsx! {
                RemoveTemplateDialog { template, reloads, on_close: move |()| open.set(None) }
            },
        }
    }
}

/// "Remove the template F-150 factory schedule? Vehicles it was applied to keep their copy."
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RemoveTemplateDialog(
    template: TemplateSummary,
    reloads: Signal<u32>,
    on_close: EventHandler<()>,
) -> Element {
    let mut problem = use_signal(|| None::<MessageId>);
    let id = template.id;
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title",
                    {tr_with(messages::TEMPLATE_REMOVE_TITLE, &[("name", MessageArg::from(template.name.as_str()))])}
                }
                p { {tr(messages::TEMPLATE_REMOVE_NOTE)} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button {
                        class: "primary-button danger",
                        onclick: move |_| {
                            spawn(async move {
                                match api::delete_template(id).await {
                                    Ok(TemplatesOutcome::Done) => {
                                        reloads += 1;
                                        on_close.call(());
                                    }
                                    Ok(other) => problem.set(Some(refusal(&other))),
                                    Err(error) => problem.set(Some(problem_from_error(&error).message())),
                                }
                            });
                        },
                        {tr(messages::SPEC_REMOVE)}
                    }
                    button { class: "secondary-button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

/// A name to type (a template's): `on_save` saves it, and `problem` says what went wrong.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn NameDialog(
    title: String,
    start: String,
    problem: Option<MessageId>,
    on_save: EventHandler<Label>,
    on_close: EventHandler<()>,
) -> Element {
    let mut name = use_signal(move || start);
    let mut typing = use_signal(|| None::<MessageId>);
    let save = move |event: FormEvent| {
        event.prevent_default();
        match Label::parse(&name()) {
            Ok(label) => {
                typing.set(None);
                on_save.call(label);
            }
            Err(error) => typing.set(Some(error.message())),
        }
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            form {
                class: "dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {title} }
                label { class: "field",
                    span { class: "field-label", {tr(messages::TEMPLATE_NAME)} }
                    input { class: "field-input", value: name(), maxlength: "40", oninput: move |event| name.set(event.value()) }
                }
                if let Some(message) = typing().or(problem) {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "primary-button", r#type: "submit", {tr(messages::ACTION_SAVE)} }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

/// **Save as template…** in a vehicle's Set up: its requirements, bundles, and specs, under a
/// name; then the Templates screen.
#[component]
pub(super) fn SaveTemplateDialog(asset: AssetId, on_close: EventHandler<()>) -> Element {
    let navigator = use_navigator();
    let mut problem = use_signal(|| None::<MessageId>);
    rsx! {
        NameDialog {
            title: tr(messages::TEMPLATE_SAVE_TITLE),
            start: String::new(),
            problem: problem(),
            on_save: move |name: Label| {
                spawn(async move {
                    match api::save_template(asset, name).await {
                        Ok(TemplatesOutcome::Saved { id }) => {
                            tracing::debug!(target: log_area::UI, template = %id, %asset, "template saved");
                            navigator.push(crate::ui::routes::Route::Templates {});
                        }
                        Ok(other) => problem.set(Some(refusal(&other))),
                        Err(error) => problem.set(Some(problem_from_error(&error).message())),
                    }
                });
            },
            on_close,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_template_says_what_it_holds_and_what_applying_it_did() {
        let counts = TemplateCounts {
            requirements: 9,
            bundles: 1,
            specs: 0,
        };
        assert_eq!(counts_text(counts), "9 requirements · 1 bundle · 0 specs");
        let applied = TemplatesOutcome::Applied {
            requirements: 1,
            bundles: 1,
            specs: 2,
            left_out: vec![(
                Label::parse("Engine oil & filter").unwrap(),
                Label::parse("Engine hours").unwrap(),
            )],
        };
        assert_eq!(
            applied_text(&applied, "Van").as_deref(),
            Some(
                "Copied 1 requirement, 1 bundle, and 2 specs onto Van. Left out (no matching meter): Engine oil & filter: Engine hours."
            )
        );
        assert_eq!(applied_text(&TemplatesOutcome::Done, "Van"), None);
    }
}
