//! Recalls and bulletins in the app (milestone 9; #068; the human's picks in
//! `docs/ui-sketches/p2m9-references-options.svg`: **1B** and **4A**).
//!
//! - **+ Recall or bulletin** (Maintenance › Set up) opens [`ReferencePicker`]: **+ A new one**
//!   ([`ReferenceDialog`]: the document, then this vehicle's requirement for it), or one
//!   already recorded on another vehicle the person can see (a new requirement for it here).
//! - A recall's own view ([`ReferenceSheet`]): its details, its PDF and photos (the recall's,
//!   so every vehicle that links it sees them), the vehicles that link it, and **Edit** for
//!   whoever may (a Maintainer of every one of them, or the household's Admin in Admin mode).
//! - [`ReferenceLine`]: "Recall · NHTSA 24V-123 · Open it ›", on a requirement that links one.

use dioxus::prelude::*;
use domain::api::{AssetView, ReferenceInput, ReferenceLink, ReferenceView, ReferencesOutcome};
use domain::attachment::{AttachmentOwner, PhotoSize};
use domain::dates::CalendarDate;
use domain::i18n::{MessageId, Translator, messages};
use domain::label::Label;
use domain::log_area;
use domain::notes::Notes;
use domain::reference::{
    LISTED_TYPES, RECALL, ReferenceDocument, ReferenceDocumentId, ReferenceNumber,
    ReferenceProblem, ReferenceType,
};

use super::attachments::AttachmentStrip;
use super::compliance::{ComplianceChoices, TypedCompliance};
use super::documents::date_text;
use super::problem_from_error;
use crate::api;
use crate::ui::tr;
use crate::ui::units::use_units;

/// A requirement's name from a recall's title: at most 40 characters, cut at a word.
pub(super) fn short_name(title: &str) -> String {
    let title = title.trim();
    if title.chars().count() <= domain::label::LABEL_MAX_CHARS {
        return title.to_owned();
    }
    let cut: String = title.chars().take(domain::label::LABEL_MAX_CHARS).collect();
    match cut.rfind(' ') {
        Some(space) if space > 10 => cut[..space].trim_end().to_owned(),
        _ => cut.trim_end().to_owned(),
    }
}

/// A type's name: a listed one's message, or the household's own.
fn type_name(doc_type: &ReferenceType) -> String {
    match doc_type {
        ReferenceType::Listed { key } => Translator::english().tr_key(key),
        ReferenceType::Custom { name } => name.as_str().to_owned(),
    }
}

/// "Recall · NHTSA campaign 24V-123": the type and the first reference number.
pub(super) fn summary(document: &ReferenceDocument) -> String {
    let mut parts = vec![type_name(&document.doc_type)];
    if let Some(first) = document.numbers.first() {
        parts.push(match &first.scheme {
            Some(scheme) => format!("{} {}", scheme.as_str(), first.number),
            None => first.number.clone(),
        });
    } else if let Some(authority) = &document.authority {
        parts.push(authority.clone());
    }
    parts.join(" · ")
}

/// The reference documents the person can read, again whenever `reloads` changes.
pub(super) fn use_references(reloads: Signal<u32>) -> Signal<Vec<ReferenceView>> {
    let mut loaded = use_signal(Vec::new);
    use_effect(move || {
        let _ = reloads();
        spawn(async move {
            if let Ok(ReferencesOutcome::References { references }) = api::references().await {
                loaded.set(references);
            }
        });
    });
    loaded
}

/// A refusal's message.
fn refusal(outcome: &ReferencesOutcome) -> MessageId {
    match outcome {
        ReferencesOutcome::Invalid { problem } => problem.message(),
        ReferencesOutcome::InvalidRequirement { problem } => problem.message(),
        ReferencesOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        ReferencesOutcome::NotFound => messages::REFERENCE_NOT_FOUND,
        ReferencesOutcome::InUse => messages::REFERENCE_IN_USE,
        ReferencesOutcome::References { .. }
        | ReferencesOutcome::Reference { .. }
        | ReferencesOutcome::Saved { .. }
        | ReferencesOutcome::Done
        | ReferencesOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

// --- The picker (1B) ---------------------------------------------------------------------

/// **+ Recall or bulletin**: a new one, or one already recorded (on the person's other
/// vehicles, not yet on this one).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ReferencePicker(
    known: Vec<ReferenceView>,
    linked: Vec<ReferenceDocumentId>,
    on_new: EventHandler<()>,
    on_pick: EventHandler<ReferenceView>,
    on_close: EventHandler<()>,
) -> Element {
    let others: Vec<ReferenceView> = known
        .into_iter()
        .filter(|view| !linked.contains(&view.id))
        .collect();
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog reference-picker", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::REFERENCE_PICKER_TITLE)} }
                button { class: "log-tile reference-new", r#type: "button", onclick: move |_| on_new.call(()),
                    span { class: "log-tile-name", {tr(messages::REFERENCE_NEW_ONE)} }
                }
                if !others.is_empty() {
                    h3 { class: "section-heading", {tr(messages::REFERENCE_ALREADY_RECORDED).to_uppercase()} }
                    for view in others {
                        button {
                            key: "{view.id}",
                            class: "log-tile",
                            r#type: "button",
                            onclick: {
                                let picked = view;
                                move |_| on_pick.call(picked.clone())
                            },
                            span { class: "log-tile-text",
                                span { class: "log-tile-name", {view.document.title.clone()} }
                                span { class: "log-tile-detail",
                                    {
                                        let on: Vec<String> = view.vehicles.iter().map(|(_, name)| name.clone()).collect();
                                        format!("{} · {}", summary(&view.document), on.join(", "))
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CANCEL)} }
                }
            }
        }
    }
}

// --- The form ----------------------------------------------------------------------------

/// The document's fields as typed.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Typed {
    type_key: String,
    authority: String,
    numbers: Vec<(String, String)>,
    title: String,
    description: String,
    remedy: String,
    issued_on: String,
    effective_on: String,
    revision: String,
    applicability: String,
    url: String,
}

impl Typed {
    fn from_document(document: &ReferenceDocument) -> Self {
        let date = |date: &Option<CalendarDate>| {
            date.as_ref()
                .map(|date| date.as_str().to_owned())
                .unwrap_or_default()
        };
        Self {
            type_key: match &document.doc_type {
                ReferenceType::Listed { key } => key.clone(),
                ReferenceType::Custom { .. } => String::new(),
            },
            authority: document.authority.clone().unwrap_or_default(),
            numbers: document
                .numbers
                .iter()
                .map(|number| {
                    (
                        number
                            .scheme
                            .as_ref()
                            .map(|scheme| scheme.as_str().to_owned())
                            .unwrap_or_default(),
                        number.number.clone(),
                    )
                })
                .collect(),
            title: document.title.clone(),
            description: document.description.as_str().to_owned(),
            remedy: document.remedy.as_str().to_owned(),
            issued_on: date(&document.issued_on),
            effective_on: date(&document.effective_on),
            revision: document.revision.clone().unwrap_or_default(),
            applicability: document.applicability.as_str().to_owned(),
            url: document.url.clone().unwrap_or_default(),
        }
    }

    /// What to send, or the first problem's message. A custom type stays as it was
    /// (`kept_type`).
    fn input(&self, kept_type: Option<&ReferenceType>) -> Result<ReferenceInput, MessageId> {
        let optional = |text: &str| Some(text.trim().to_owned()).filter(|text| !text.is_empty());
        let date = |text: &str| {
            optional(text)
                .map(|text| CalendarDate::parse(&text).map_err(domain::dates::DateError::message))
                .transpose()
        };
        let notes = |text: &str| Notes::parse(text).map_err(domain::notes::NotesError::message);
        let doc_type = match (self.type_key.is_empty(), kept_type) {
            (true, Some(kept)) => kept.clone(),
            (true, None) => ReferenceType::listed(RECALL).map_err(ReferenceProblem::message)?,
            (false, _) => {
                ReferenceType::listed(&self.type_key).map_err(ReferenceProblem::message)?
            }
        };
        let numbers = self
            .numbers
            .iter()
            .filter(|(_, number)| !number.trim().is_empty())
            .map(|(scheme, number)| {
                Ok(ReferenceNumber {
                    scheme: optional(scheme)
                        .map(|scheme| Label::parse(&scheme))
                        .transpose()
                        .map_err(domain::label::LabelError::message)?,
                    number: number.trim().to_owned(),
                })
            })
            .collect::<Result<Vec<_>, MessageId>>()?;
        Ok(ReferenceInput {
            doc_type,
            authority: optional(&self.authority),
            numbers,
            title: self.title.trim().to_owned(),
            description: notes(&self.description)?,
            remedy: notes(&self.remedy)?,
            issued_on: date(&self.issued_on)?,
            effective_on: date(&self.effective_on)?,
            revision: optional(&self.revision),
            supersedes: None,
            applicability: notes(&self.applicability)?,
            url: optional(&self.url),
        })
    }
}

/// A new recall or bulletin for `vehicle` (with its requirement there), or a change to one
/// (`existing`). `on_saved` gets its ID.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ReferenceDialog(
    vehicle: AssetView,
    existing: Option<ReferenceView>,
    on_saved: EventHandler<ReferenceDocumentId>,
    on_close: EventHandler<()>,
) -> Element {
    let start = existing.clone();
    let mut typed = use_signal(|| match &start {
        Some(view) => Typed::from_document(&view.document),
        None => Typed {
            type_key: RECALL.to_owned(),
            numbers: vec![(String::new(), String::new())],
            ..Typed::default()
        },
    });
    let mut name = use_signal(String::new);
    let mut named = use_signal(|| false);
    let compliance = use_signal(|| TypedCompliance {
        mode: domain::requirement::ComplianceMode::OneTime,
        ..TypedCompliance::default()
    });
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let editing = existing.as_ref().map(|view| view.id);
    let kept_type = existing.as_ref().map(|view| view.document.doc_type.clone());
    let asset = vehicle.id;
    let save = move |event: FormEvent| {
        event.prevent_default();
        if busy() {
            return;
        }
        let input = match typed().input(kept_type.as_ref()) {
            Ok(input) => input,
            Err(message) => {
                problem.set(Some(message));
                return;
            }
        };
        let link = if editing.is_none() {
            let wanted = if named() {
                name()
            } else {
                short_name(&input.title)
            };
            let name = match Label::parse(&wanted) {
                Ok(name) => name,
                Err(error) => {
                    problem.set(Some(error.message()));
                    return;
                }
            };
            let applicability = match compliance().applicability() {
                Ok(applicability) => applicability,
                Err(message) => {
                    problem.set(Some(message));
                    return;
                }
            };
            Some(ReferenceLink {
                asset,
                name,
                mode: compliance().mode,
                applicability,
            })
        } else {
            None
        };
        busy.set(true);
        spawn(async move {
            let result = match (editing, link) {
                (Some(id), _) => api::edit_reference(id, input).await,
                (None, Some(link)) => api::add_reference(input, link).await,
                (None, None) => return,
            };
            busy.set(false);
            match result {
                Ok(ReferencesOutcome::Saved { id, .. }) => {
                    tracing::debug!(target: log_area::UI, reference = %id, "reference document saved");
                    on_saved.call(id);
                }
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let now = typed();
    let shown_name = if named() {
        name()
    } else {
        short_name(&now.title)
    };
    let numbers = now.numbers.clone();
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            form {
                class: "dialog wide-dialog reference-dialog",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                onsubmit: save,
                h2 { class: "dialog-title", {tr(if editing.is_some() { messages::REFERENCE_EDIT_TITLE } else { messages::REFERENCE_NEW_TITLE })} }
                div { class: "reference-columns",
                    div { class: "reference-document",
                        div { class: "field-row",
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_TYPE)} }
                                select { class: "field-input", onchange: move |event| typed.write().type_key = event.value(),
                                    for key in LISTED_TYPES {
                                        option { key: "{key}", value: key, selected: now.type_key == key, {Translator::english().tr_key(key)} }
                                    }
                                }
                            }
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_AUTHORITY)} }
                                input { class: "field-input", autocomplete: "off", value: now.authority.clone(), oninput: move |event| typed.write().authority = event.value() }
                            }
                        }
                        h3 { class: "section-heading", {tr(messages::REFERENCE_NUMBERS).to_uppercase()} }
                        for (index, (scheme, number)) in numbers.into_iter().enumerate() {
                            div { key: "{index}", class: "field-row reference-number",
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::REFERENCE_SCHEME)} }
                                    input { class: "field-input", autocomplete: "off", value: scheme, oninput: move |event| typed.write().numbers[index].0 = event.value() }
                                }
                                label { class: "field",
                                    span { class: "field-label", {tr(messages::REFERENCE_NUMBER)} }
                                    input { class: "field-input", autocomplete: "off", value: number, oninput: move |event| typed.write().numbers[index].1 = event.value() }
                                }
                                button { class: "link-button rule-remove", r#type: "button", "aria-label": tr(messages::ACTION_REMOVE),
                                    onclick: move |_| { typed.write().numbers.remove(index); },
                                    "✕"
                                }
                            }
                        }
                        if now.numbers.len() < domain::reference::MAX_NUMBERS {
                            button { class: "link-button", r#type: "button", onclick: move |_| typed.write().numbers.push((String::new(), String::new())), {tr(messages::REFERENCE_ANOTHER_NUMBER)} }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::REFERENCE_TITLE)} }
                            input { class: "field-input", autocomplete: "off", value: now.title.clone(), oninput: move |event| typed.write().title = event.value() }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::REFERENCE_DESCRIPTION)} }
                            textarea { class: "field-input", rows: "3", value: now.description.clone(), oninput: move |event| typed.write().description = event.value() }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::REFERENCE_REMEDY)} }
                            textarea { class: "field-input", rows: "2", value: now.remedy.clone(), oninput: move |event| typed.write().remedy = event.value() }
                        }
                        div { class: "field-row",
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_ISSUED)} }
                                input { class: "field-input", r#type: "date", value: now.issued_on.clone(), oninput: move |event| typed.write().issued_on = event.value() }
                            }
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_EFFECTIVE)} }
                                input { class: "field-input", r#type: "date", value: now.effective_on.clone(), oninput: move |event| typed.write().effective_on = event.value() }
                            }
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_REVISION)} }
                                input { class: "field-input", autocomplete: "off", value: now.revision.clone(), oninput: move |event| typed.write().revision = event.value() }
                            }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::REFERENCE_APPLIES_TO)} }
                            input { class: "field-input", autocomplete: "off", value: now.applicability.clone(), oninput: move |event| typed.write().applicability = event.value() }
                        }
                        label { class: "field",
                            span { class: "field-label", {tr(messages::REFERENCE_URL)} }
                            input { class: "field-input", r#type: "url", autocomplete: "off", value: now.url, oninput: move |event| typed.write().url = event.value() }
                        }
                    }
                    if editing.is_none() {
                        div { class: "reference-link",
                            h3 { class: "section-heading",
                                {crate::ui::tr_with(messages::REFERENCE_ON_VEHICLE, &[("vehicle", domain::i18n::MessageArg::from(vehicle.asset.name.as_str()))]).to_uppercase()}
                            }
                            label { class: "field",
                                span { class: "field-label", {tr(messages::REFERENCE_REQUIREMENT_NAME)} }
                                input { class: "field-input", autocomplete: "off", maxlength: "40", value: shown_name,
                                    oninput: move |event| {
                                        named.set(true);
                                        name.set(event.value());
                                    },
                                }
                            }
                            ComplianceChoices { compliance }
                        }
                    }
                }
                p { class: "muted-note", {tr(if editing.is_some() { messages::REFERENCE_EDIT_NOTE } else { messages::REFERENCE_ATTACH_NOTE })} }
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

// --- Its view ----------------------------------------------------------------------------

/// A recall's or bulletin's own view, from `vehicle`'s page: its details, its PDF and photos,
/// the vehicles that link it, and **Edit**.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ReferenceSheet(
    id: ReferenceDocumentId,
    vehicle: AssetView,
    on_close: EventHandler<()>,
) -> Element {
    let units = use_units();
    let mut loaded = use_signal(|| None::<Result<ReferenceView, MessageId>>);
    let mut changes = use_signal(|| 0_u32);
    let mut editing = use_signal(|| false);
    use_effect(use_reactive!(|id| {
        let _ = changes();
        spawn(async move {
            let answer = match api::reference(id).await {
                Ok(ReferencesOutcome::Reference { view }) => Ok(*view),
                Ok(other) => Err(refusal(&other)),
                Err(error) => Err(problem_from_error(&error).message()),
            };
            loaded.set(Some(answer));
        });
    }));
    let body = match loaded() {
        None => rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => rsx! { p { class: "form-error", role: "alert", {tr(problem)} } },
        Some(Ok(view)) => {
            let document = view.document.clone();
            let mut rows: Vec<(MessageId, String)> = Vec::new();
            let mut push = |label: MessageId, value: Option<String>| {
                if let Some(value) = value.filter(|value| !value.is_empty()) {
                    rows.push((label, value));
                }
            };
            push(
                messages::REFERENCE_TYPE,
                Some(type_name(&document.doc_type)),
            );
            push(messages::REFERENCE_AUTHORITY, document.authority.clone());
            push(
                messages::REFERENCE_NUMBERS,
                Some(
                    document
                        .numbers
                        .iter()
                        .map(|number| match &number.scheme {
                            Some(scheme) => format!("{} {}", scheme.as_str(), number.number),
                            None => number.number.clone(),
                        })
                        .collect::<Vec<_>>()
                        .join(" · "),
                ),
            );
            push(
                messages::REFERENCE_DESCRIPTION,
                Some(document.description.as_str().to_owned()),
            );
            push(
                messages::REFERENCE_REMEDY,
                Some(document.remedy.as_str().to_owned()),
            );
            push(
                messages::REFERENCE_ISSUED,
                document
                    .issued_on
                    .as_ref()
                    .map(|date| date_text(date, &units())),
            );
            push(
                messages::REFERENCE_EFFECTIVE,
                document
                    .effective_on
                    .as_ref()
                    .map(|date| date_text(date, &units())),
            );
            push(messages::REFERENCE_REVISION, document.revision.clone());
            push(
                messages::REFERENCE_APPLIES_TO,
                Some(document.applicability.as_str().to_owned()),
            );
            push(
                messages::REFERENCE_LINKED,
                Some(
                    view.vehicles
                        .iter()
                        .map(|(_, name)| name.clone())
                        .collect::<Vec<_>>()
                        .join(", "),
                ),
            );
            let url = document.url.clone();
            let may_change = view.may_change;
            rsx! {
                h2 { class: "dialog-title", {document.title} }
                dl { class: "event-rows",
                    for (label, value) in rows {
                        div { key: "{label.as_str()}", class: "event-row",
                            dt { {tr(label)} }
                            dd { {value} }
                        }
                    }
                }
                if let Some(url) = url {
                    a { class: "link-button", href: "{url}", target: "_blank", rel: "noopener noreferrer", {tr(messages::REFERENCE_OPEN_SOURCE)} }
                }
                AttachmentStrip {
                    asset: vehicle.id,
                    owner: AttachmentOwner::Reference { reference: id },
                    heading: messages::REFERENCE_FILES,
                    default_size: PhotoSize::Medium,
                }
                div { class: "form-buttons",
                    if may_change {
                        button { class: "secondary-button", r#type: "button", onclick: move |_| editing.set(true), {tr(messages::ACTION_EDIT)} }
                    }
                    button { class: "primary-button", r#type: "button", onclick: move |_| on_close.call(()), {tr(messages::ACTION_CLOSE)} }
                }
                if editing() {
                    ReferenceDialog {
                        vehicle,
                        existing: Some(view),
                        on_saved: move |_| {
                            editing.set(false);
                            changes += 1;
                        },
                        on_close: move |()| editing.set(false),
                    }
                }
            }
        }
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(()),
            div { class: "dialog wide-dialog reference-sheet", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                {body}
            }
        }
    }
}

/// "Recall · NHTSA campaign 24V-123 · Open it ›" on a requirement that links one.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(super) fn ReferenceLine(reference: ReferenceDocumentId, vehicle: AssetView) -> Element {
    let mut text = use_signal(|| None::<String>);
    let mut open = use_signal(|| false);
    use_effect(use_reactive!(|reference| {
        spawn(async move {
            if let Ok(ReferencesOutcome::Reference { view }) = api::reference(reference).await {
                text.set(Some(summary(&view.document)));
            }
        });
    }));
    rsx! {
        if let Some(text) = text() {
            button { class: "link-button reference-line", r#type: "button", onclick: move |_| open.set(true),
                {format!("{text} · {} ›", tr(messages::REFERENCE_OPEN))}
            }
        }
        if open() {
            ReferenceSheet { id: reference, vehicle, on_close: move |()| open.set(false) }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_long_title_makes_a_name_cut_at_a_word() {
        assert_eq!(
            short_name(" Rear axle nut may loosen "),
            "Rear axle nut may loosen"
        );
        let name = short_name("Front passenger air bag inflator may rupture during deployment");
        assert!(name.chars().count() <= 40, "{name}");
        assert_eq!(name, "Front passenger air bag inflator may");
    }

    #[test]
    fn typed_numbers_skip_blank_rows_and_dates_must_be_dates() {
        let typed = Typed {
            type_key: RECALL.to_owned(),
            numbers: vec![
                ("NHTSA campaign".into(), "24V-123".into()),
                (String::new(), "  ".into()),
            ],
            title: "Rear axle nut may loosen".into(),
            issued_on: "2024-03-01".into(),
            ..Typed::default()
        };
        let input = typed.input(None).unwrap();
        assert_eq!(input.numbers.len(), 1);
        assert_eq!(input.issued_on.unwrap().as_str(), "2024-03-01");
        assert_eq!(input.authority, None);
        let bad = Typed {
            issued_on: "soon".into(),
            ..typed
        };
        assert!(bad.input(None).is_err());
    }
}
