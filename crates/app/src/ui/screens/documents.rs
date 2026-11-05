//! A vehicle's Documents tab (milestone 8, step 8h; the human's pick: option A of
//! `docs/ui-sketches/p2m8-documents-options.svg`; decision #066).
//!
//! One column of cards:
//! - **License plate:** the current one drawn like a plate, with who issued it, since when, and
//!   when it expires (from its registration); earlier plates folded below. A plate moved over
//!   from the old identifiers asks for who issued it.
//! - **Documents:** each current one (nothing renews it) as a card: its type, number, issuer,
//!   dates, a status chip ("Current", "Expires in 12 days", "Expired"), its scans, and
//!   **Renew…** and **Edit**; its earlier ones (what it renewed) folded below.
//!
//! Anyone who can see the vehicle sees them; a Maintainer (in that mode) changes them.

use dioxus::prelude::*;
use domain::api::{
    AssetView, DocumentInput, DocumentView, DocumentsOutcome, PlateInput, PlateView,
};
use domain::attachment::{AttachmentOwner, PhotoSize};
use domain::dates::{CalendarDate, format_date};
use domain::document::{
    AssetDocument, AssetDocumentId, DocumentStatus, DocumentType, REGISTRATION,
};
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::label::Label;
use domain::location::TimeZone;
use domain::notes::Notes;
use domain::plate::{Authority, LicensePlate, LicensePlateId, PlateNumber, REGIONS};
use domain::preferences::Preferences;
use domain::time::{Clock, UnixMillis};

use super::attachments::AttachmentStrip;
use super::places::use_countries;
use super::problem_from_error;
use super::squawks::use_device_zone;
use crate::api;
use crate::logging::SystemClock;
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

/// A current document's chip turns into "Expires in N days" this close to its expiry.
const SOON_DAYS: i64 = 30;

/// The person's date today, in the device's zone (`None` if the device's clock or zone is
/// unusable).
fn today(zone: &TimeZone) -> Option<CalendarDate> {
    domain::dates::today(SystemClock.now(), zone)
        .ok()
        .and_then(|text| CalendarDate::parse(&text).ok())
}

/// The days from `from` to `to` (negative if `to` is earlier).
fn days_between(from: &CalendarDate, to: &CalendarDate) -> Option<i64> {
    let from: jiff::civil::Date = from.as_str().parse().ok()?;
    let to: jiff::civil::Date = to.as_str().parse().ok()?;
    Some(i64::from((to - from).get_days()))
}

/// A date as the person writes dates: "31 Oct 2026".
fn date_text(date: &CalendarDate, units: &Preferences) -> String {
    let Ok(utc) = TimeZone::parse("UTC") else {
        return date.as_str().to_owned();
    };
    domain::dates::local_to_utc(date.as_str(), "12:00", &utc)
        .ok()
        .and_then(|at: UnixMillis| {
            format_date(at, &utc, units.date_format, Translator::english()).ok()
        })
        .unwrap_or_else(|| date.as_str().to_owned())
}

/// A document type's name: the profile's (translated) or the household's own.
fn type_name(doc_type: &DocumentType) -> String {
    match doc_type {
        DocumentType::Listed { key } => Translator::english().tr_key(key),
        DocumentType::Custom { name } => name.as_str().to_owned(),
    }
}

/// The message for a refused documents request.
fn refusal(outcome: &DocumentsOutcome) -> MessageId {
    match outcome {
        DocumentsOutcome::InvalidDocument { problem } => problem.message(),
        DocumentsOutcome::InvalidPlate { problem } => problem.message(),
        DocumentsOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        DocumentsOutcome::Documents { .. }
        | DocumentsOutcome::SavedDocument { .. }
        | DocumentsOutcome::SavedPlate { .. }
        | DocumentsOutcome::Done
        | DocumentsOutcome::NotFound
        | DocumentsOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// What the tab loaded.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    documents: Vec<DocumentView>,
    plates: Vec<PlateView>,
    listed_types: Vec<String>,
    may_change: bool,
}

/// Which dialog is open.
#[derive(Debug, Clone, PartialEq)]
enum Dialog {
    NewDocument,
    Renew(AssetDocumentId, Box<AssetDocument>),
    EditDocument(AssetDocumentId, Box<AssetDocument>),
    NewPlate,
    EditPlate(LicensePlateId, Box<LicensePlate>),
}

/// The documents that nothing renews (the current ones, or expired and not yet renewed), and
/// for each, the ones it renewed (newest first).
fn chains(documents: &[DocumentView]) -> Vec<(DocumentView, Vec<DocumentView>)> {
    documents
        .iter()
        .filter(|view| view.renewed_by.is_none())
        .map(|head| {
            let mut earlier = Vec::new();
            let mut next = head.document.supersedes;
            while let Some(id) = next {
                let Some(view) = documents.iter().find(|view| view.id == id) else {
                    break;
                };
                earlier.push(view.clone());
                next = view.document.supersedes;
            }
            (head.clone(), earlier)
        })
        .collect()
}

/// The status chip's text and style.
fn status_chip(view: &DocumentView, today: &CalendarDate) -> (String, &'static str) {
    match view.status {
        DocumentStatus::Current => {
            let soon = view
                .document
                .expires_on
                .as_ref()
                .and_then(|expires| days_between(today, expires))
                .filter(|days| *days <= SOON_DAYS);
            soon.map_or_else(
                || (tr(DocumentStatus::Current.label()), "status-chip ok"),
                |days| {
                    (
                        tr_with(
                            messages::DOCUMENT_EXPIRES_IN,
                            &[("days", MessageArg::from(days))],
                        ),
                        "status-chip warn",
                    )
                },
            )
        }
        DocumentStatus::Expired => (tr(DocumentStatus::Expired.label()), "status-chip bad"),
        DocumentStatus::Superseded => (tr(DocumentStatus::Superseded.label()), "status-chip"),
    }
}

/// "Issued 1 Nov 2025 · expires 31 Oct 2026", "never expires".
fn dates_line(document: &AssetDocument, units: &Preferences) -> String {
    let mut parts = Vec::new();
    if let Some(issued) = &document.issued_on {
        parts.push(tr_with(
            messages::DOCUMENT_ISSUED_ON,
            &[("date", MessageArg::from(date_text(issued, units)))],
        ));
    }
    if let Some(effective) = &document.effective_on {
        parts.push(tr_with(
            messages::DOCUMENT_EFFECTIVE_ON,
            &[("date", MessageArg::from(date_text(effective, units)))],
        ));
    }
    parts.push(document.expires_on.as_ref().map_or_else(
        || tr(messages::DOCUMENT_NEVER_EXPIRES),
        |expires| {
            tr_with(
                messages::DOCUMENT_EXPIRES_ON,
                &[("date", MessageArg::from(date_text(expires, units)))],
            )
        },
    ));
    parts.join(" · ")
}

/// A plate's dates: "Since 1 Jun 2024 · expires 31 Oct 2027", "2019 – 2024".
fn plate_dates(plate: &LicensePlate, units: &Preferences) -> String {
    let mut parts = Vec::new();
    match (&plate.issued_on, &plate.retired_on) {
        (Some(issued), Some(retired)) => parts.push(format!(
            "{} – {}",
            date_text(issued, units),
            date_text(retired, units)
        )),
        (Some(issued), None) => parts.push(tr_with(
            messages::PLATE_SINCE,
            &[("date", MessageArg::from(date_text(issued, units)))],
        )),
        (None, Some(retired)) => parts.push(tr_with(
            messages::PLATE_RETIRED_ON,
            &[("date", MessageArg::from(date_text(retired, units)))],
        )),
        (None, None) => {}
    }
    if plate.retired_on.is_none()
        && let Some(expires) = &plate.expires_on
    {
        parts.push(tr_with(
            messages::DOCUMENT_EXPIRES_ON,
            &[("date", MessageArg::from(date_text(expires, units)))],
        ));
    }
    parts.join(" · ")
}

/// A plate's issuing authority, written out ("North Carolina", "DE", "Cherokee Nation").
fn authority_name(plate: &LicensePlate) -> Option<String> {
    plate
        .authority
        .as_ref()
        .map(|authority| authority.name().to_owned())
}

/// `asset`'s documents and plates as of `today`.
async fn load(asset: domain::asset::AssetId, today: CalendarDate) -> Result<Loaded, MessageId> {
    match api::documents(asset, today).await {
        Ok(DocumentsOutcome::Documents {
            documents,
            plates,
            listed_types,
            may_change,
        }) => Ok(Loaded {
            documents,
            plates,
            listed_types,
            may_change,
        }),
        Ok(other) => Err(refusal(&other)),
        Err(error) => Err(problem_from_error(&error).message()),
    }
}

/// The Documents tab.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn DocumentsTab(vehicle: AssetView) -> Element {
    let zone = use_device_zone();
    let units = use_units();
    let mut changes = use_signal(|| 0_u32);
    let mut loaded = use_signal(|| None::<Result<Loaded, MessageId>>);
    let mut dialog = use_signal(|| None::<Dialog>);
    let asset = vehicle.id;
    use_effect(move || {
        let _ = changes();
        let Some(today) = today(&zone()) else {
            loaded.set(Some(Err(messages::SOMETHING_WENT_WRONG)));
            return;
        };
        spawn(async move {
            loaded.set(Some(load(asset, today).await));
        });
    });
    let found = match loaded() {
        None => return rsx! { p { class: "auth-loading", {tr(messages::LOADING)} } },
        Some(Err(problem)) => {
            return rsx! { p { class: "placeholder", role: "alert", {tr(problem)} } };
        }
        Some(Ok(found)) => found,
    };
    let Some(today) = today(&zone()) else {
        return rsx! {};
    };
    let units = units();
    let current = found.plates.iter().find(|view| view.current).cloned();
    let earlier: Vec<PlateView> = found
        .plates
        .iter()
        .filter(|view| !view.current)
        .cloned()
        .collect();
    let may_change = found.may_change;
    let close = move |changed: bool| {
        dialog.set(None);
        if changed {
            changes += 1;
        }
    };
    rsx! {
        div { class: "documents-tab",
            div { class: "section-head",
                h3 { class: "section-heading", {tr(messages::DOCUMENTS_PLATE).to_uppercase()} }
                if may_change {
                    button { class: "secondary-button", r#type: "button", onclick: move |_| dialog.set(Some(Dialog::NewPlate)), {tr(messages::DOCUMENTS_ADD_PLATE)} }
                }
            }
            match current {
                Some(view) => rsx! {
                    PlateCard { view, units, may_change, on_edit: move |(id, plate): (LicensePlateId, LicensePlate)| dialog.set(Some(Dialog::EditPlate(id, Box::new(plate)))) }
                },
                None => rsx! { p { class: "muted-note", {tr(messages::DOCUMENTS_NO_PLATE)} } },
            }
            if !earlier.is_empty() {
                details { class: "documents-earlier",
                    summary { {tr_with(messages::DOCUMENTS_EARLIER_PLATES, &[("count", MessageArg::from(i64::try_from(earlier.len()).unwrap_or(i64::MAX)))])} }
                    for view in earlier {
                        div { key: "{view.id}", class: "documents-earlier-row",
                            span {
                                {format!("{} · {}", authority_name(&view.plate).unwrap_or_default(), view.plate.number.as_str())}
                                span { class: "muted-note", {format!("  {}", plate_dates(&view.plate, &units))} }
                            }
                            if may_change {
                                button {
                                    class: "link-button",
                                    r#type: "button",
                                    onclick: {
                                        let (id, plate) = (view.id, view.plate);
                                        move |_| dialog.set(Some(Dialog::EditPlate(id, Box::new(plate.clone()))))
                                    },
                                    {tr(messages::ACTION_EDIT)}
                                }
                            }
                        }
                    }
                }
            }
            div { class: "section-head",
                h3 { class: "section-heading", {tr(messages::DOCUMENTS_HEADING).to_uppercase()} }
                if may_change {
                    button { class: "secondary-button", r#type: "button", onclick: move |_| dialog.set(Some(Dialog::NewDocument)), {tr(messages::DOCUMENTS_ADD)} }
                }
            }
            if found.documents.is_empty() {
                p { class: "muted-note", {tr(messages::DOCUMENTS_NONE)} }
            }
            for (head, older) in chains(&found.documents) {
                DocumentCard {
                    key: "{head.id}",
                    asset,
                    head,
                    older,
                    today: today.clone(),
                    units,
                    may_change,
                    on_dialog: move |next: Dialog| dialog.set(Some(next)),
                }
            }
        }
        match dialog() {
            Some(Dialog::NewDocument) => rsx! {
                DocumentDialog { asset, listed_types: found.listed_types.clone(), plates: found.plates, mode: DocumentMode::New, on_close: close }
            },
            Some(Dialog::Renew(id, previous)) => rsx! {
                DocumentDialog { asset, listed_types: found.listed_types.clone(), plates: found.plates, mode: DocumentMode::Renew(id, previous), on_close: close }
            },
            Some(Dialog::EditDocument(id, document)) => rsx! {
                DocumentDialog { asset, listed_types: found.listed_types.clone(), plates: found.plates, mode: DocumentMode::Edit(id, document), on_close: close }
            },
            Some(Dialog::NewPlate) => rsx! { PlateDialog { asset, editing: None, today, on_close: close } },
            Some(Dialog::EditPlate(id, plate)) => rsx! { PlateDialog { asset, editing: Some((id, plate)), today, on_close: close } },
            None => rsx! {},
        }
    }
}

/// The current plate's card.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PlateCard(
    view: PlateView,
    units: Preferences,
    may_change: bool,
    on_edit: EventHandler<(LicensePlateId, LicensePlate)>,
) -> Element {
    let by = authority_name(&view.plate);
    let PlateView { id, plate, .. } = view;
    let number = plate.number.as_str().to_owned();
    let dates = plate_dates(&plate, &units);
    let follows = plate.expires_on.is_some();
    rsx! {
        div { class: "plate-card",
            div { class: "plate-picture",
                span { class: "plate-picture-by", {by.clone().unwrap_or_default().to_uppercase()} }
                span { class: "plate-picture-number", {number.clone()} }
            }
            div { class: "plate-card-text",
                strong { {by.as_ref().map_or_else(|| number.clone(), |by| format!("{by} · {number}"))} }
                if by.is_none() {
                    p { class: "form-error", {tr(messages::PLATE_CHOOSE_AUTHORITY)} }
                }
                span { class: "muted-note",
                    {dates}
                    if follows { {format!(" {}", tr(messages::PLATE_FROM_REGISTRATION))} }
                }
            }
            if may_change {
                button { class: "secondary-button", r#type: "button", onclick: move |_| on_edit.call((id, plate.clone())), {tr(messages::ACTION_EDIT)} }
            }
        }
    }
}

/// One document's card, with its scans and earlier ones.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DocumentCard(
    asset: domain::asset::AssetId,
    head: DocumentView,
    older: Vec<DocumentView>,
    today: CalendarDate,
    units: Preferences,
    may_change: bool,
    on_dialog: EventHandler<Dialog>,
) -> Element {
    let (chip, chip_class) = status_chip(&head, &today);
    let document = head.document.clone();
    let id = head.id;
    let details: Vec<String> = [document.number.clone(), document.issuer.clone()]
        .into_iter()
        .flatten()
        .collect();
    let renewing = document.clone();
    let editing = document.clone();
    rsx! {
        div { class: "document-card",
            div { class: "document-card-head",
                strong { class: "document-card-type", {type_name(&document.doc_type)} }
                span { class: chip_class, {chip} }
            }
            if !details.is_empty() {
                p { class: "document-card-line", {details.join(" · ")} }
            }
            p { class: "muted-note", {dates_line(&document, &units)} }
            AttachmentStrip { asset, owner: AttachmentOwner::Document { document: id }, heading: messages::DOCUMENTS_SCANS, default_size: PhotoSize::Medium }
            if may_change {
                div { class: "check-row document-card-buttons",
                    if document.expires_on.is_some() {
                        button { class: "secondary-button", r#type: "button", onclick: move |_| on_dialog.call(Dialog::Renew(id, Box::new(renewing.clone()))), {tr(messages::DOCUMENTS_RENEW)} }
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_dialog.call(Dialog::EditDocument(id, Box::new(editing.clone()))), {tr(messages::ACTION_EDIT)} }
                }
            }
        }
        if !older.is_empty() {
            details { class: "documents-earlier",
                summary { {tr_with(messages::DOCUMENTS_EARLIER, &[("count", MessageArg::from(i64::try_from(older.len()).unwrap_or(i64::MAX)))])} }
                for view in older {
                    div { key: "{view.id}", class: "documents-earlier-row",
                        span {
                            {view.document.number.clone().unwrap_or_else(|| type_name(&view.document.doc_type))}
                            span { class: "muted-note", {format!("  {}", dates_line(&view.document, &units))} }
                        }
                        if may_change {
                            button {
                                class: "link-button",
                                r#type: "button",
                                onclick: {
                                    let (id, document) = (view.id, view.document);
                                    move |_| on_dialog.call(Dialog::EditDocument(id, Box::new(document.clone())))
                                },
                                {tr(messages::ACTION_EDIT)}
                            }
                        }
                    }
                }
            }
        }
    }
}

// --- The document dialog -----------------------------------------------------------------

/// What the document dialog does.
#[derive(Debug, Clone, PartialEq)]
enum DocumentMode {
    New,
    Renew(AssetDocumentId, Box<AssetDocument>),
    Edit(AssetDocumentId, Box<AssetDocument>),
}

/// The value the type menu uses for "the household's own".
const CUSTOM: &str = "custom";

/// A date typed into a date field, if any; a bad one is a problem.
fn typed_date(text: &str) -> Result<Option<CalendarDate>, MessageId> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(None);
    }
    CalendarDate::parse(text)
        .map(Some)
        .map_err(domain::dates::DateError::message)
}

/// What the document dialog's fields hold.
#[derive(Debug, Clone, Default, PartialEq)]
struct TypedDocument {
    type_key: String,
    custom_name: String,
    number: String,
    issuer: String,
    issued: String,
    effective: String,
    expires: String,
    plate: String,
    notes: String,
}

fn typed_document(document: &AssetDocument, dates: bool) -> TypedDocument {
    let (type_key, custom_name) = match &document.doc_type {
        DocumentType::Listed { key } => (key.clone(), String::new()),
        DocumentType::Custom { name } => (CUSTOM.to_owned(), name.as_str().to_owned()),
    };
    let date = |value: &Option<CalendarDate>| {
        if dates {
            value
                .as_ref()
                .map(|date| date.as_str().to_owned())
                .unwrap_or_default()
        } else {
            String::new()
        }
    };
    TypedDocument {
        type_key,
        custom_name,
        number: document.number.clone().unwrap_or_default(),
        issuer: document.issuer.clone().unwrap_or_default(),
        issued: date(&document.issued_on),
        effective: date(&document.effective_on),
        expires: date(&document.expires_on),
        plate: document.plate.map(|id| id.to_string()).unwrap_or_default(),
        notes: if dates {
            document.notes.as_str().to_owned()
        } else {
            String::new()
        },
    }
}

/// The input for the server, or the problem with what was typed.
fn document_input(
    asset: domain::asset::AssetId,
    typed: &TypedDocument,
) -> Result<DocumentInput, MessageId> {
    let doc_type = if typed.type_key == CUSTOM {
        DocumentType::Custom {
            name: Label::parse(&typed.custom_name).map_err(|_| messages::DOCUMENTS_NAME_NEEDED)?,
        }
    } else {
        DocumentType::listed(&typed.type_key).map_err(domain::document::DocumentProblem::message)?
    };
    let registration = doc_type.is_registration();
    let optional = |text: &str| {
        let text = text.trim();
        (!text.is_empty()).then(|| text.to_owned())
    };
    Ok(DocumentInput {
        asset,
        doc_type,
        number: optional(&typed.number),
        issuer: optional(&typed.issuer),
        issued_on: typed_date(&typed.issued)?,
        effective_on: typed_date(&typed.effective)?,
        expires_on: typed_date(&typed.expires)?,
        plate: if registration {
            typed.plate.parse().ok()
        } else {
            None
        },
        notes: Notes::parse(&typed.notes).map_err(|_| messages::DOCUMENTS_NOTES_TOO_LONG)?,
    })
}

/// Add, renew, or change a document (and remove one, when changing it).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn DocumentDialog(
    asset: domain::asset::AssetId,
    listed_types: Vec<String>,
    plates: Vec<PlateView>,
    mode: DocumentMode,
    on_close: EventHandler<bool>,
) -> Element {
    let start = match &mode {
        DocumentMode::New => TypedDocument {
            type_key: listed_types
                .first()
                .cloned()
                .unwrap_or_else(|| CUSTOM.to_owned()),
            plate: plates
                .iter()
                .find(|view| view.current)
                .map(|view| view.id.to_string())
                .unwrap_or_default(),
            ..TypedDocument::default()
        },
        DocumentMode::Renew(_, previous) => typed_document(previous, false),
        DocumentMode::Edit(_, document) => typed_document(document, true),
    };
    let mut typed = use_signal(|| start);
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let mut removing = use_signal(|| false);
    let title = tr(match &mode {
        DocumentMode::New => messages::DOCUMENTS_ADD_TITLE,
        DocumentMode::Renew(..) => messages::DOCUMENTS_RENEW_TITLE,
        DocumentMode::Edit(..) => messages::DOCUMENTS_EDIT_TITLE,
    });
    let editing = match &mode {
        DocumentMode::Edit(id, _) => Some(*id),
        DocumentMode::New | DocumentMode::Renew(..) => None,
    };
    let save_mode = mode.clone();
    let save = move |_| {
        let input = match document_input(asset, &typed()) {
            Ok(input) => input,
            Err(message) => {
                problem.set(Some(message));
                return;
            }
        };
        let mode = save_mode.clone();
        busy.set(true);
        spawn(async move {
            let answer = match mode {
                DocumentMode::New => api::add_document(input).await,
                DocumentMode::Renew(previous, _) => api::renew_document(previous, input).await,
                DocumentMode::Edit(id, _) => api::edit_document(id, input).await,
            };
            busy.set(false);
            match answer {
                Ok(DocumentsOutcome::SavedDocument { .. }) => on_close.call(true),
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let remove = move |_| {
        let Some(id) = editing else { return };
        spawn(async move {
            match api::remove_document(id).await {
                Ok(DocumentsOutcome::Done) => on_close.call(true),
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let registration = typed().type_key == REGISTRATION;
    let field = |label: MessageId,
                 value: String,
                 kind: &'static str,
                 set: fn(&mut TypedDocument, String)| {
        rsx! {
            label { class: "field",
                span { class: "field-label", {tr(label)} }
                input { class: "field-input", r#type: kind, value, oninput: move |event| set(&mut typed.write(), event.value()) }
            }
        }
    };
    let now = typed();
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(false),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {title} }
                if matches!(mode, DocumentMode::Renew(..)) {
                    p { class: "dialog-text", {tr(messages::DOCUMENTS_RENEW_NOTE)} }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::DOCUMENTS_TYPE)} }
                    select {
                        class: "field-input",
                        disabled: matches!(mode, DocumentMode::Renew(..)),
                        onchange: move |event| typed.write().type_key = event.value(),
                        for key in listed_types {
                            option { key: "{key}", value: "{key}", selected: now.type_key == key, {Translator::english().tr_key(&key)} }
                        }
                        option { value: CUSTOM, selected: now.type_key == CUSTOM, {tr(messages::DOCUMENTS_TYPE_OTHER)} }
                    }
                }
                if now.type_key == CUSTOM {
                    {field(messages::DOCUMENTS_TYPE_NAME, now.custom_name.clone(), "text", |typed, value| typed.custom_name = value)}
                }
                div { class: "field-row",
                    {field(messages::DOCUMENTS_NUMBER, now.number.clone(), "text", |typed, value| typed.number = value)}
                    {field(messages::DOCUMENTS_ISSUER, now.issuer.clone(), "text", |typed, value| typed.issuer = value)}
                }
                div { class: "field-row",
                    {field(messages::DOCUMENTS_ISSUED, now.issued.clone(), "date", |typed, value| typed.issued = value)}
                    {field(messages::DOCUMENTS_EFFECTIVE, now.effective.clone(), "date", |typed, value| typed.effective = value)}
                    {field(messages::DOCUMENTS_EXPIRES, now.expires.clone(), "date", |typed, value| typed.expires = value)}
                }
                if registration {
                    label { class: "field",
                        span { class: "field-label", {tr(messages::DOCUMENTS_PLATE_FIELD)} }
                        select {
                            class: "field-input",
                            onchange: move |event| typed.write().plate = event.value(),
                            option { value: "", selected: now.plate.is_empty(), {tr(messages::DOCUMENTS_NO_PLATE_CHOICE)} }
                            for view in plates {
                                option { key: "{view.id}", value: "{view.id}", selected: now.plate == view.id.to_string(), {domain::plate::label(&view.plate)} }
                            }
                        }
                    }
                    p { class: "muted-note", {tr(messages::DOCUMENTS_PLATE_FOLLOWS)} }
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::DOCUMENTS_NOTES)} }
                    textarea { class: "field-input", value: now.notes, oninput: move |event| typed.write().notes = event.value() }
                }
                if editing.is_none() {
                    p { class: "muted-note", {tr(messages::DOCUMENTS_SCANS_LATER)} }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    if editing.is_some() {
                        if removing() {
                            button { class: "danger-button filled", r#type: "button", onclick: remove, {tr(messages::DOCUMENTS_REMOVE_CONFIRM)} }
                        } else {
                            button { class: "danger-button", r#type: "button", onclick: move |_| removing.set(true), {tr(messages::ACTION_REMOVE)} }
                        }
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(false), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "button", disabled: busy(), onclick: save,
                        {tr(if matches!(mode, DocumentMode::Renew(..)) { messages::DOCUMENTS_RENEW_SAVE } else { messages::ACTION_SAVE })}
                    }
                }
            }
        }
    }
}

// --- The plate dialog --------------------------------------------------------------------

/// Who issued a plate, as the dialog's first choice.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum By {
    UsState,
    Province,
    Country,
    Other,
}

/// What the plate dialog's fields hold.
#[derive(Debug, Clone, PartialEq)]
struct TypedPlate {
    by: Option<By>,
    region: String,
    country: String,
    other: String,
    number: String,
    issued: String,
    expires: String,
    retired: String,
    notes: String,
}

fn typed_plate(plate: Option<&LicensePlate>) -> TypedPlate {
    let date = |value: Option<&CalendarDate>| {
        value
            .map(|date| date.as_str().to_owned())
            .unwrap_or_default()
    };
    let (by, region, country, other) = match plate.and_then(|plate| plate.authority.as_ref()) {
        Some(Authority::Region { country, region }) => (
            Some(if country == "CA" {
                By::Province
            } else {
                By::UsState
            }),
            region.clone(),
            String::new(),
            String::new(),
        ),
        Some(Authority::Country { country }) => (
            Some(By::Country),
            String::new(),
            country.clone(),
            String::new(),
        ),
        Some(Authority::Other { name }) => {
            (Some(By::Other), String::new(), String::new(), name.clone())
        }
        // A new plate starts as a US state's; one moved over has none chosen yet.
        None => (
            plate.is_none().then_some(By::UsState),
            String::new(),
            String::new(),
            String::new(),
        ),
    };
    TypedPlate {
        by,
        region,
        country,
        other,
        number: plate
            .map(|plate| plate.number.as_str().to_owned())
            .unwrap_or_default(),
        issued: date(plate.and_then(|plate| plate.issued_on.as_ref())),
        expires: date(plate.and_then(|plate| plate.expires_on.as_ref())),
        retired: date(plate.and_then(|plate| plate.retired_on.as_ref())),
        notes: plate
            .map(|plate| plate.notes.as_str().to_owned())
            .unwrap_or_default(),
    }
}

/// The input for the server, or the problem with what was typed.
fn plate_input(asset: domain::asset::AssetId, typed: &TypedPlate) -> Result<PlateInput, MessageId> {
    let authority = match typed.by {
        Some(By::UsState) => Some(Authority::region("US", &typed.region)),
        Some(By::Province) => Some(Authority::region("CA", &typed.region)),
        Some(By::Country) => Some(Authority::country(&typed.country)),
        Some(By::Other) => Some(Authority::other(&typed.other)),
        None => None,
    }
    .transpose()
    .map_err(domain::plate::PlateProblem::message)?;
    Ok(PlateInput {
        asset,
        authority,
        number: PlateNumber::parse(&typed.number).map_err(domain::plate::PlateProblem::message)?,
        issued_on: typed_date(&typed.issued)?,
        expires_on: typed_date(&typed.expires)?,
        retired_on: typed_date(&typed.retired)?,
        notes: Notes::parse(&typed.notes).map_err(|_| messages::DOCUMENTS_NOTES_TOO_LONG)?,
    })
}

/// Add a plate, or change one (and remove one, when changing it).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn PlateDialog(
    asset: domain::asset::AssetId,
    editing: Option<(LicensePlateId, Box<LicensePlate>)>,
    today: CalendarDate,
    on_close: EventHandler<bool>,
) -> Element {
    let countries = use_countries();
    let mut typed = use_signal(|| typed_plate(editing.as_ref().map(|(_, plate)| plate.as_ref())));
    let mut problem = use_signal(|| None::<MessageId>);
    let mut busy = use_signal(|| false);
    let mut removing = use_signal(|| false);
    let id = editing.as_ref().map(|(id, _)| *id);
    let save = move |_| {
        let input = match plate_input(asset, &typed()) {
            Ok(input) => input,
            Err(message) => {
                problem.set(Some(message));
                return;
            }
        };
        let today = today.clone();
        busy.set(true);
        spawn(async move {
            let answer = match id {
                Some(id) => api::edit_plate(id, input).await,
                None => api::add_plate(input, today).await,
            };
            busy.set(false);
            match answer {
                Ok(DocumentsOutcome::SavedPlate { .. }) => on_close.call(true),
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let remove = move |_| {
        let Some(id) = id else { return };
        spawn(async move {
            match api::remove_plate(id).await {
                Ok(DocumentsOutcome::Done) => on_close.call(true),
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    let now = typed();
    let regions: Vec<(&str, &str)> = REGIONS
        .iter()
        .filter(|(country, _, _)| match now.by {
            Some(By::UsState) => *country == "US",
            Some(By::Province) => *country == "CA",
            Some(By::Country | By::Other) | None => false,
        })
        .map(|(_, code, name)| (*code, *name))
        .collect();
    let date_field = |label: MessageId, value: String, set: fn(&mut TypedPlate, String)| {
        rsx! {
            label { class: "field",
                span { class: "field-label", {tr(label)} }
                input { class: "field-input", r#type: "date", value, oninput: move |event| set(&mut typed.write(), event.value()) }
            }
        }
    };
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| on_close.call(false),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(if id.is_some() { messages::PLATE_EDIT_TITLE } else { messages::PLATE_ADD_TITLE })} }
                if id.is_none() {
                    p { class: "dialog-text", {tr(messages::PLATE_ADD_NOTE)} }
                }
                span { class: "field-label", {tr(messages::PLATE_ISSUED_BY)} }
                div { class: "choice-buttons",
                    for (choice, label) in [(By::UsState, messages::PLATE_BY_STATE), (By::Province, messages::PLATE_BY_PROVINCE), (By::Country, messages::PLATE_BY_COUNTRY), (By::Other, messages::PLATE_BY_OTHER)] {
                        button {
                            key: "{choice:?}",
                            class: if now.by == Some(choice) { "choice on" } else { "choice" },
                            r#type: "button",
                            "aria-pressed": now.by == Some(choice),
                            onclick: move |_| {
                                let mut typed = typed.write();
                                typed.by = Some(choice);
                                typed.region.clear();
                            },
                            {tr(label)}
                        }
                    }
                }
                match now.by {
                    Some(By::UsState | By::Province) => rsx! {
                        label { class: "field",
                            span { class: "field-label", {tr(if now.by == Some(By::Province) { messages::PLATE_PROVINCE } else { messages::PLATE_STATE })} }
                            select {
                                class: "field-input",
                                onchange: move |event| typed.write().region = event.value(),
                                option { value: "", selected: now.region.is_empty(), "—" }
                                for (code, name) in regions {
                                    option { key: "{code}", value: "{code}", selected: now.region == code, {name} }
                                }
                            }
                        }
                    },
                    Some(By::Country) => rsx! {
                        label { class: "field",
                            span { class: "field-label", {tr(messages::PLATE_COUNTRY)} }
                            select {
                                class: "field-input",
                                onchange: move |event| typed.write().country = event.value(),
                                option { value: "", selected: now.country.is_empty(), "—" }
                                for (code, name) in countries() {
                                    option { key: "{code.as_str()}", value: "{code.as_str()}", selected: now.country == code.as_str(), {name} }
                                }
                            }
                        }
                    },
                    Some(By::Other) => rsx! {
                        label { class: "field",
                            span { class: "field-label", {tr(messages::PLATE_OTHER)} }
                            input { class: "field-input", value: now.other.clone(), oninput: move |event| typed.write().other = event.value() }
                        }
                    },
                    None => rsx! { p { class: "form-error", {tr(messages::PLATE_CHOOSE_AUTHORITY)} } },
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::PLATE_NUMBER)} }
                    input { class: "field-input", autocomplete: "off", value: now.number.clone(), oninput: move |event| typed.write().number = event.value() }
                }
                div { class: "field-row",
                    {date_field(messages::DOCUMENTS_ISSUED, now.issued.clone(), |typed, value| typed.issued = value)}
                    {date_field(messages::DOCUMENTS_EXPIRES, now.expires.clone(), |typed, value| typed.expires = value)}
                    {date_field(messages::PLATE_RETIRED, now.retired.clone(), |typed, value| typed.retired = value)}
                }
                label { class: "field",
                    span { class: "field-label", {tr(messages::DOCUMENTS_NOTES)} }
                    textarea { class: "field-input", value: now.notes.clone(), oninput: move |event| typed.write().notes = event.value() }
                }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    if id.is_some() {
                        if removing() {
                            button { class: "danger-button filled", r#type: "button", onclick: remove, {tr(messages::PLATE_REMOVE_CONFIRM)} }
                        } else {
                            button { class: "danger-button", r#type: "button", onclick: move |_| removing.set(true), {tr(messages::ACTION_REMOVE)} }
                        }
                    }
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(false), {tr(messages::ACTION_CANCEL)} }
                    button { class: "primary-button", r#type: "button", disabled: busy(), onclick: save,
                        {tr(if id.is_some() { messages::ACTION_SAVE } else { messages::PLATE_ADD_SAVE })}
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use domain::test_support::test_id;

    use super::*;

    fn date(text: &str) -> CalendarDate {
        CalendarDate::parse(text).unwrap()
    }

    fn view(
        n: u32,
        supersedes: Option<u32>,
        renewed_by: Option<u32>,
        expires: &str,
    ) -> DocumentView {
        DocumentView {
            id: test_id(n),
            document: AssetDocument {
                asset: test_id(1),
                doc_type: DocumentType::listed(REGISTRATION).unwrap(),
                number: None,
                issuer: None,
                issued_on: None,
                effective_on: None,
                expires_on: Some(date(expires)),
                supersedes: supersedes.map(test_id),
                plate: None,
                notes: Notes::default(),
                added_at: UnixMillis::from_millis(0),
                added_by: test_id(5),
                received_at: UnixMillis::from_millis(0),
            },
            status: if renewed_by.is_some() {
                DocumentStatus::Superseded
            } else {
                DocumentStatus::Current
            },
            renewed_by: renewed_by.map(test_id),
        }
    }

    #[test]
    fn each_current_document_carries_what_it_renewed_newest_first() {
        let documents = [
            view(10, None, Some(11), "2024-10-31"),
            view(11, Some(10), Some(12), "2025-10-31"),
            view(12, Some(11), None, "2026-10-31"),
        ];
        let found = chains(&documents);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].0.id, test_id(12));
        let earlier: Vec<_> = found[0].1.iter().map(|view| view.id).collect();
        assert_eq!(earlier, [test_id(11), test_id(10)]);
    }

    #[test]
    fn a_document_close_to_its_expiry_says_how_close() {
        let current = view(12, None, None, "2026-11-20");
        assert_eq!(
            status_chip(&current, &date("2026-11-04")),
            ("Expires in 16 days".to_owned(), "status-chip warn")
        );
        assert_eq!(
            status_chip(&current, &date("2026-09-01")).1,
            "status-chip ok"
        );
        assert_eq!(
            days_between(&date("2026-11-04"), &date("2026-11-03")),
            Some(-1)
        );
    }

    #[test]
    fn a_plates_authority_is_typed_from_the_dialog() {
        let mut typed = typed_plate(None);
        assert_eq!(
            typed.by,
            Some(By::UsState),
            "a new plate starts as a US state's"
        );
        typed.region = "NC".into();
        typed.number = "ncx 4471".into();
        let input = plate_input(test_id(1), &typed).unwrap();
        assert_eq!(
            input.authority,
            Some(Authority::region("US", "NC").unwrap())
        );
        assert_eq!(input.number.as_str(), "NCX 4471");
        typed.by = Some(By::Province);
        assert_eq!(
            plate_input(test_id(1), &typed),
            Err(messages::PLATE_UNKNOWN_REGION)
        );
        let moved = LicensePlate {
            asset: test_id(1),
            authority: None,
            number: PlateNumber::parse("NCX 4471").unwrap(),
            issued_on: None,
            expires_on: None,
            retired_on: None,
            notes: Notes::default(),
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(5),
            received_at: UnixMillis::from_millis(0),
        };
        assert_eq!(
            typed_plate(Some(&moved)).by,
            None,
            "a moved plate has none chosen"
        );
    }

    #[test]
    fn a_renewal_keeps_the_type_number_and_plate_but_not_the_dates() {
        let mut previous = view(12, None, None, "2026-10-31").document;
        previous.number = Some("R-55120".into());
        previous.plate = Some(test_id(70));
        let renewal = typed_document(&previous, false);
        assert_eq!(renewal.type_key, REGISTRATION);
        assert_eq!(renewal.number, "R-55120");
        assert_eq!(renewal.plate, test_id::<LicensePlate>(70).to_string());
        assert!(renewal.expires.is_empty());
        assert_eq!(
            document_input(
                test_id(1),
                &TypedDocument {
                    expires: "31/10/2027".into(),
                    ..renewal
                }
            ),
            Err(messages::DATE_BAD_DATE)
        );
    }
}
