//! Photos on events, tasks, squawks, and vehicles (milestone 8, step 8e; the human's pick:
//! option A of `docs/ui-sketches/p2m8-photos-options.svg`).
//!
//! - **A strip of thumbnails** under each thing ([`AttachmentStrip`]), with **+ Add** at the
//!   end for whoever may add there. PDFs show as a page tile (they're added in 8f).
//! - **+ Add** opens a sheet: **Take photo** or **Choose photos**, the **size** (Small, Medium,
//!   or Large; the default follows the kind of event, #036), and **Keep the original** for
//!   Maintainers and Admins when the household allows it.
//! - **Tapping one opens it** full screen ([`Viewer`]): its details (when, where, camera, size,
//!   who added it), Download, Original, and Remove; ‹ › (or the arrow keys) for the next one.

use std::collections::HashMap;

use dioxus::prelude::*;
use domain::api::{
    AttachmentInput, AttachmentPart, AttachmentView, AttachmentsOutcome, PlacesOutcome,
    UploadOutcome, UploadPurpose, UploadedFile,
};
use domain::asset::AssetId;
use domain::attachment::{
    AttachmentId, AttachmentOwner, Content, FileName, FileType, Original, PDF_MAX_BYTES, PhotoSize,
};

/// [`ZOOMS`]'s "fit the width" step.
const FIT_WIDTH: usize = 2;
use domain::dates::{format_date, format_time};
use domain::i18n::{MessageArg, MessageId, Translator, messages};
use domain::log_area;

use super::problem_from_error;
use super::squawks::use_device_zone;
use crate::api;
use crate::data;
use crate::files;
use crate::ui::location::place_text;
use crate::ui::pdfs::{PdfPages, PdfState, ZOOMS, first_page_thumbnail};
use crate::ui::photos::{read_metadata, shrink};
use crate::ui::units::use_units;
use crate::ui::{tr, tr_with};

/// Thumbnails and full files already fetched this session, as `data:` URLs (they never change).
static FETCHED: GlobalSignal<HashMap<(AttachmentId, AttachmentPart), String>> =
    Signal::global(HashMap::new);

/// One part of an attachment as a `data:` URL, fetched once.
async fn fetched(id: AttachmentId, part: AttachmentPart, mime: &str) -> Option<String> {
    if let Some(url) = FETCHED.read().get(&(id, part)) {
        return Some(url.clone());
    }
    match files::fetch(id, part).await {
        Ok(bytes) => {
            let url = files::data_url(mime, &bytes);
            FETCHED.write().insert((id, part), url.clone());
            Some(url)
        }
        Err(error) => {
            tracing::debug!(target: log_area::UI, attachment = %id, %error, "couldn't fetch an attachment");
            None
        }
    }
}

/// What the strip loaded.
#[derive(Debug, Clone, PartialEq)]
struct Loaded {
    attachments: Vec<AttachmentView>,
    may_add: bool,
    may_keep_originals: bool,
}

async fn load(asset: AssetId, owner: AttachmentOwner) -> Result<Loaded, MessageId> {
    match data::attachments(asset, owner).await {
        Ok(AttachmentsOutcome::Attachments {
            attachments,
            may_add,
            may_keep_originals,
        }) => Ok(Loaded {
            // An event's list includes its tasks'; the strip shows only what's on `owner`.
            attachments: attachments
                .into_iter()
                .filter(|view| view.attachment.owner == owner)
                .collect(),
            may_add,
            may_keep_originals,
        }),
        Ok(other) => Err(refusal(&other)),
        Err(error) => Err(problem_from_error(&error).message()),
    }
}

/// The message for a refused attachments request.
fn refusal(outcome: &AttachmentsOutcome) -> MessageId {
    match outcome {
        AttachmentsOutcome::Invalid { problem } => problem.message(),
        AttachmentsOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        AttachmentsOutcome::NotFound
        | AttachmentsOutcome::Attachments { .. }
        | AttachmentsOutcome::Saved { .. }
        | AttachmentsOutcome::Done
        | AttachmentsOutcome::Unavailable => messages::SOMETHING_WENT_WRONG,
    }
}

/// The strip of photos (and PDFs) on `owner` of vehicle `asset`, under `heading`, with + Add
/// for whoever may add; photos start at `default_size`. Shows nothing if there's nothing to
/// show and nothing to add.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn AttachmentStrip(
    asset: AssetId,
    owner: AttachmentOwner,
    heading: MessageId,
    default_size: PhotoSize,
) -> Element {
    let mut changes = use_signal(|| 0_u32);
    let mut loaded = use_signal(|| None::<Result<Loaded, MessageId>>);
    let mut adding = use_signal(|| false);
    let mut viewing = use_signal(|| None::<usize>);
    use_effect(use_reactive!(|asset, owner| {
        let _ = changes();
        spawn(async move {
            loaded.set(Some(load(asset, owner).await));
        });
    }));
    let Some(Ok(found)) = loaded() else {
        return rsx! {};
    };
    if found.attachments.is_empty() && !found.may_add {
        return rsx! {};
    }
    let Loaded {
        attachments,
        may_add,
        may_keep_originals,
    } = found;
    rsx! {
        section { class: "attachment-strip",
            h3 { class: "section-heading", {tr(heading).to_uppercase()} }
            div { class: "attachment-tiles",
                for (index, view) in attachments.iter().enumerate() {
                    Tile { key: "{view.id}", view: view.clone(), on_open: move |()| viewing.set(Some(index)) }
                }
                if may_add {
                    button {
                        class: "attachment-add",
                        r#type: "button",
                        onclick: move |_| adding.set(true),
                        {tr(messages::ATTACHMENT_ADD)}
                    }
                }
            }
        }
        if adding() {
            AddSheet {
                asset,
                owner,
                default_size,
                may_keep_originals,
                on_close: move |added: bool| {
                    adding.set(false);
                    if added {
                        changes += 1;
                    }
                },
            }
        }
        if let Some(index) = viewing() {
            Viewer {
                attachments,
                index,
                may_keep_originals,
                on_index: move |next: usize| viewing.set(Some(next)),
                on_close: move |removed: bool| {
                    viewing.set(None);
                    if removed {
                        changes += 1;
                    }
                },
            }
        }
    }
}

/// One thumbnail (a photo's, or a PDF's first page, or a page tile).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn Tile(view: AttachmentView, on_open: EventHandler<()>) -> Element {
    let id = view.id;
    let has_thumbnail = view.attachment.thumbnail.is_some();
    let mut url = use_signal(|| None::<String>);
    use_effect(use_reactive!(|id, has_thumbnail| {
        if has_thumbnail {
            spawn(async move {
                url.set(fetched(id, AttachmentPart::Thumbnail, FileType::Jpeg.mime()).await);
            });
        }
    }));
    // Added on this computer and not uploaded yet (2A): asked again when the outbox changes.
    let mut waiting = use_signal(|| false);
    let hash = view.attachment.file.hash.clone();
    use_effect(move || {
        let _changed = crate::ui::outbox::outbox_changes();
        let hash = hash.clone();
        spawn(async move {
            waiting.set(crate::ui::outbox::waiting_to_upload(&hash).await);
        });
    });
    let name = view.attachment.name.as_str().to_owned();
    let pages = match view.attachment.content {
        Content::Document { pages } => Some(pages),
        Content::Photo { .. } => None,
    };
    rsx! {
        button {
            class: if pages.is_some() { "attachment-tile attachment-tile-pdf" } else { "attachment-tile" },
            r#type: "button",
            title: "{name}",
            "aria-label": "{name}",
            onclick: move |_| on_open.call(()),
            if let Some(src) = url() {
                img { src: "{src}", alt: "" }
            }
            if let Some(pages) = pages {
                span { class: "attachment-tile-badge", {tr_with(messages::ATTACHMENT_PDF_PAGES, &[("count", MessageArg::from(i64::from(pages)))])} }
            }
            if waiting() {
                span { class: "attachment-tile-upload", title: tr(messages::UPLOAD_WAITING), "aria-label": tr(messages::UPLOAD_WAITING), "↑" }
            }
        }
    }
}

/// The size a photo on an event of `kind` starts at, for a vehicle of `asset_kind`: its
/// profile's choice, or Small on fuel-ups and readings and Medium on jobs (choice 8).
pub(crate) fn photo_size_for(
    asset_kind: &domain::asset::AssetKind,
    kind: domain::meter::EventKind,
) -> PhotoSize {
    domain::profile::profile(asset_kind).map_or_else(
        || PhotoSize::default_for(kind),
        |profile| profile.photo_size(kind),
    )
}

// --- Adding ------------------------------------------------------------------------------

/// The message for an upload the server refused (or couldn't take).
fn upload_problem(outcome: &UploadOutcome) -> MessageId {
    match outcome {
        UploadOutcome::Invalid { problem } => problem.message(),
        UploadOutcome::NotAllowed => messages::VEHICLES_NOT_ALLOWED,
        UploadOutcome::Uploaded { .. } | UploadOutcome::Unavailable => {
            messages::SOMETHING_WENT_WRONG
        }
    }
}

/// Uploads `bytes` for `purpose`.
async fn send(purpose: UploadPurpose, bytes: Vec<u8>) -> Result<UploadedFile, MessageId> {
    match data::upload(purpose, bytes).await {
        Ok(UploadOutcome::Uploaded { uploaded }) => Ok(uploaded),
        Ok(other) => Err(upload_problem(&other)),
        Err(error) => {
            tracing::debug!(target: log_area::UI, %error, "an upload didn't reach the server");
            Err(messages::SOMETHING_WENT_WRONG)
        }
    }
}

/// Everything about adding one photo: read its metadata, shrink it, upload it (and its original
/// and metadata if kept), then add it to `owner`.
async fn add_photo(
    asset: AssetId,
    owner: AttachmentOwner,
    file: dioxus::html::FileData,
    size: PhotoSize,
    keep_original: bool,
) -> Result<(), MessageId> {
    let name = FileName::from_upload(&file.name())
        .map_err(domain::attachment::AttachmentProblem::message)?;
    let bytes = file
        .read_bytes()
        .await
        .map_err(|_| messages::ATTACHMENT_UNREADABLE)?
        .to_vec();
    let read = read_metadata(&bytes);
    let mime = file
        .content_type()
        .unwrap_or_else(|| "image/jpeg".to_owned());
    let shrunk = shrink(&bytes, &mime, size.long_edge(), read.local_time.as_deref()).await;
    let mut metadata = read.metadata;
    metadata.taken_at = metadata.taken_at.or(shrunk.taken_at);
    let photo = send(
        UploadPurpose::Photo { size },
        shrunk.jpeg.unwrap_or_else(|| bytes.clone()),
    )
    .await?;
    let original = if keep_original {
        let kept = send(UploadPurpose::Original, bytes).await?;
        kept.file_type.map(|file_type| Original {
            file: kept.file,
            file_type,
        })
    } else {
        None
    };
    if let Some(raw) = read.raw {
        // Metadata that doesn't upload isn't worth losing the photo over.
        metadata.raw = send(UploadPurpose::PhotoMetadata, raw)
            .await
            .ok()
            .map(|uploaded| uploaded.file);
    }
    let (width, height) = photo.pixels.unwrap_or((0, 0));
    let input = AttachmentInput {
        asset,
        owner,
        name,
        file: photo.file,
        thumbnail: photo.thumbnail,
        content: Content::Photo {
            size,
            width,
            height,
            metadata,
            original,
        },
    };
    match data::add_attachment(input).await {
        Ok(AttachmentsOutcome::Saved { id }) => {
            tracing::debug!(target: log_area::UI, attachment = %id, "photo added");
            Ok(())
        }
        Ok(other) => Err(refusal(&other)),
        Err(error) => Err(problem_from_error(&error).message()),
    }
}

/// Everything about adding one PDF: upload it (the server checks it and counts its pages),
/// draw and upload its first page as the thumbnail, then add it to `owner`.
async fn add_pdf(
    asset: AssetId,
    owner: AttachmentOwner,
    file: dioxus::html::FileData,
) -> Result<(), MessageId> {
    let name = FileName::from_upload(&file.name())
        .map_err(domain::attachment::AttachmentProblem::message)?;
    // Refused here before sending 60 MB for the server to refuse.
    if file.size() > PDF_MAX_BYTES {
        return Err(messages::ATTACHMENT_PDF_TOO_LARGE);
    }
    let bytes = file
        .read_bytes()
        .await
        .map_err(|_| messages::ATTACHMENT_UNREADABLE)?
        .to_vec();
    let thumbnail = first_page_thumbnail(&bytes).await;
    let uploaded = send(UploadPurpose::Document, bytes).await?;
    let thumbnail = match thumbnail {
        // A thumbnail that doesn't upload isn't worth losing the PDF over: it shows a page tile.
        Some(jpeg) => send(UploadPurpose::DocumentThumbnail, jpeg)
            .await
            .ok()
            .map(|uploaded| uploaded.file),
        None => None,
    };
    let input = AttachmentInput {
        asset,
        owner,
        name,
        file: uploaded.file,
        thumbnail,
        content: Content::Document {
            pages: uploaded.pages.unwrap_or(1),
        },
    };
    match data::add_attachment(input).await {
        Ok(AttachmentsOutcome::Saved { id }) => {
            tracing::debug!(target: log_area::UI, attachment = %id, "PDF added");
            Ok(())
        }
        Ok(other) => Err(refusal(&other)),
        Err(error) => Err(problem_from_error(&error).message()),
    }
}

/// How big each size is, for the picker.
const fn size_hint(size: PhotoSize) -> MessageId {
    match size {
        PhotoSize::Small => messages::PHOTO_SIZE_SMALL_HINT,
        PhotoSize::Medium => messages::PHOTO_SIZE_MEDIUM_HINT,
        PhotoSize::Large => messages::PHOTO_SIZE_LARGE_HINT,
    }
}

/// The + Add sheet: Take photo or Choose photos, the size, and Keep the original.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn AddSheet(
    asset: AssetId,
    owner: AttachmentOwner,
    default_size: PhotoSize,
    may_keep_originals: bool,
    on_close: EventHandler<bool>,
) -> Element {
    let mut size = use_signal(|| default_size);
    let mut keep_original = use_signal(|| false);
    let mut progress = use_signal(|| None::<(usize, usize)>);
    let mut problems = use_signal(Vec::<String>::new);
    // What was taken or chosen, waiting for Add (the human's M8 note: the sheet stays open after
    // the camera, so the size can still change, or it can be cancelled). `true` for a PDF.
    let mut picked = use_signal(Vec::<(dioxus::html::FileData, bool)>::new);
    let mut added_any = use_signal(|| false);
    let busy = progress().is_some();
    let mut chosen = move |files: Vec<dioxus::html::FileData>, pdf: bool| {
        problems.set(Vec::new());
        picked
            .write()
            .extend(files.into_iter().map(|file| (file, pdf)));
    };
    let add = move |_| {
        let files = picked();
        if files.is_empty() {
            return;
        }
        problems.set(Vec::new());
        spawn(async move {
            let count = files.len();
            let mut failed = Vec::new();
            let mut left = Vec::new();
            for (index, (file, pdf)) in files.into_iter().enumerate() {
                progress.set(Some((index + 1, count)));
                let name = file.name();
                let added = if pdf {
                    add_pdf(asset, owner, file.clone()).await
                } else {
                    add_photo(
                        asset,
                        owner,
                        file.clone(),
                        size(),
                        keep_original() && !crate::ui::offline::is_offline_now(),
                    )
                    .await
                };
                match added {
                    Ok(()) => added_any.set(true),
                    Err(problem) => {
                        failed.push(tr_with(
                            messages::ATTACHMENT_FAILED,
                            &[
                                ("name", MessageArg::from(name.as_str())),
                                ("problem", MessageArg::from(tr(problem).as_str())),
                            ],
                        ));
                        left.push((file, pdf));
                    }
                }
            }
            progress.set(None);
            if failed.is_empty() {
                on_close.call(true);
            } else {
                // The ones that didn't go stay, to try again or remove.
                picked.set(left);
                problems.set(failed);
            }
        });
    };
    let waiting: Vec<(usize, String, bool)> = picked()
        .iter()
        .enumerate()
        .map(|(index, (file, pdf))| (index, file.name(), *pdf))
        .collect();
    let count = waiting.len();
    rsx! {
        div { class: "dialog-scrim", onclick: move |_| if !busy { on_close.call(added_any()) },
            div {
                class: "dialog attachment-sheet",
                role: "dialog",
                "aria-modal": "true",
                onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::ATTACHMENT_ADD_PHOTO)} }
                label { class: "sheet-choice",
                    input {
                        r#type: "file",
                        accept: "image/*",
                        capture: "environment",
                        disabled: busy,
                        onchange: move |event| chosen(event.files(), false),
                    }
                    span { class: "sheet-choice-label", {tr(messages::ATTACHMENT_TAKE_PHOTO)} }
                    span { class: "sheet-choice-hint", {tr(messages::ATTACHMENT_TAKE_PHOTO_HINT)} }
                }
                label { class: "sheet-choice",
                    input {
                        r#type: "file",
                        accept: "image/*",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| chosen(event.files(), false),
                    }
                    span { class: "sheet-choice-label", {tr(messages::ATTACHMENT_CHOOSE_PHOTOS)} }
                    span { class: "sheet-choice-hint", {tr(messages::ATTACHMENT_CHOOSE_PHOTOS_HINT)} }
                }
                label { class: "sheet-choice",
                    input {
                        r#type: "file",
                        accept: "application/pdf,.pdf",
                        multiple: true,
                        disabled: busy,
                        onchange: move |event| chosen(event.files(), true),
                    }
                    span { class: "sheet-choice-label", {tr(messages::ATTACHMENT_CHOOSE_PDF)} }
                    span { class: "sheet-choice-hint", {tr(messages::ATTACHMENT_CHOOSE_PDF_HINT)} }
                }
                h3 { class: "section-heading", {tr(messages::ATTACHMENT_SIZE).to_uppercase()} }
                div { class: "photo-sizes", role: "radiogroup",
                    for option in PhotoSize::ALL {
                        button {
                            key: "{option:?}",
                            class: if size() == option { "photo-size photo-size-on" } else { "photo-size" },
                            r#type: "button",
                            role: "radio",
                            "aria-checked": if size() == option { "true" } else { "false" },
                            disabled: busy,
                            onclick: move |_| size.set(option),
                            span { class: "photo-size-name", {tr(option.label())} }
                            span { class: "photo-size-hint", {tr(size_hint(option))} }
                        }
                    }
                }
                if may_keep_originals {
                    // An original needs the server (they're large; #077): off while offline.
                    label { class: "check", title: crate::ui::offline::needs_server_note(),
                        input {
                            r#type: "checkbox",
                            checked: keep_original() && !crate::ui::offline::is_offline_now(),
                            disabled: busy || crate::ui::offline::is_offline_now(),
                            onchange: move |event| keep_original.set(event.checked()),
                        }
                        span { {tr(messages::ATTACHMENT_KEEP_ORIGINAL)} }
                    }
                }
                if !waiting.is_empty() {
                    ul { class: "picked-files",
                        for (index, name, pdf) in waiting {
                            li { key: "{index}-{name}", class: "picked-file",
                                span { class: "picked-file-kind", {tr(if pdf { messages::ATTACHMENT_KIND_PDF } else { messages::ATTACHMENT_KIND_PHOTO })} }
                                span { class: "picked-file-name", {name.clone()} }
                                button {
                                    class: "link-button rule-remove",
                                    r#type: "button",
                                    disabled: busy,
                                    "aria-label": tr_with(messages::ATTACHMENT_UNPICK, &[("name", MessageArg::from(name.as_str()))]),
                                    onclick: move |_| {
                                        let mut files = picked.write();
                                        if index < files.len() {
                                            files.remove(index);
                                        }
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }
                if let Some((at, count)) = progress() {
                    p { class: "muted-note", role: "status",
                        {tr_with(messages::ATTACHMENT_ADDING, &[
                            ("at", MessageArg::from(i64::try_from(at).unwrap_or(i64::MAX))),
                            ("count", MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX))),
                        ])}
                    }
                }
                for (index, problem) in problems().into_iter().enumerate() {
                    p { key: "{index}", class: "form-error", role: "alert", {problem} }
                }
                div { class: "form-buttons",
                    button {
                        class: "secondary-button",
                        r#type: "button",
                        disabled: busy,
                        onclick: move |_| on_close.call(added_any()),
                        {tr(if added_any() { messages::ATTACHMENT_DONE } else { messages::ACTION_CANCEL })}
                    }
                    button {
                        class: "primary-button",
                        r#type: "button",
                        disabled: busy || count == 0,
                        onclick: add,
                        {tr_with(messages::ATTACHMENT_ADD_COUNT, &[("count", MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX)))])}
                    }
                }
            }
        }
    }
}

// --- Looking at one ----------------------------------------------------------------------

/// "1.2 MB", "350 KB".
fn bytes_text(bytes: u64) -> String {
    const KB: u64 = 1024;
    const MB: u64 = 1024 * 1024;
    if bytes >= MB {
        // One decimal place, rounded.
        let tenths = (bytes * 10 + MB / 2) / MB;
        format!("{}.{} MB", tenths / 10, tenths % 10)
    } else {
        format!("{} KB", bytes.div_ceil(KB))
    }
}

/// Saves `bytes` as `name`: on desktop into the Downloads folder (or home); on the web, as the
/// browser's download.
#[cfg_attr(
    all(feature = "desktop", not(target_arch = "wasm32")),
    expect(
        clippy::unused_async,
        reason = "the web path awaits the browser; the desktop path writes the file directly"
    )
)]
async fn save_download(name: &str, mime: &str, bytes: &[u8]) -> Option<String> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    {
        let _ = mime;
        let folder = dirs::download_dir().or_else(dirs::home_dir)?;
        let path = folder.join(name);
        std::fs::write(&path, bytes).ok()?;
        Some(path.display().to_string())
    }
    #[cfg(not(all(feature = "desktop", not(target_arch = "wasm32"))))]
    {
        let eval = document::eval(
            r"const input = await dioxus.recv();
              const a = document.createElement('a');
              a.href = input.url; a.download = input.name;
              document.body.appendChild(a); a.click(); a.remove();
              return true;",
        );
        eval.send(serde_json::json!({ "url": files::data_url(mime, bytes), "name": name }))
            .ok()?;
        eval.join::<bool>().await.ok()?;
        None
    }
}

/// The full-screen view of one attachment, with ‹ › to the others.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn Viewer(
    attachments: Vec<AttachmentView>,
    index: usize,
    may_keep_originals: bool,
    on_index: EventHandler<usize>,
    on_close: EventHandler<bool>,
) -> Element {
    let units = use_units();
    let zone = use_device_zone();
    let mut confirm = use_signal(|| false);
    let mut note = use_signal(|| None::<String>);
    let mut url = use_signal(|| None::<String>);
    // Offline, a photo whose full size isn't here: its thumbnail, enlarged (1A).
    let mut preview = use_signal(|| None::<String>);
    let mut missing = use_signal(|| false);
    let mut place = use_signal(|| None::<String>);
    let mut zoom = use_signal(|| FIT_WIDTH);
    let mut pdf_state = use_signal(PdfState::default);
    let count = attachments.len();
    let Some(view) = attachments.get(index).cloned() else {
        return rsx! {};
    };
    let id = view.id;
    let (mime, metadata, pixels, size, has_original) = match &view.attachment.content {
        Content::Photo {
            size,
            width,
            height,
            metadata,
            original,
        } => (
            FileType::Jpeg.mime(),
            Some(metadata.clone()),
            Some((*width, *height)),
            Some(*size),
            original.is_some(),
        ),
        Content::Document { .. } => (FileType::Pdf.mime(), None, None, None, false),
    };
    let is_pdf = mime == FileType::Pdf.mime();
    let coordinates = metadata.as_ref().and_then(|metadata| metadata.coordinates);
    use_effect(use_reactive!(|id, coordinates| {
        url.set(None);
        preview.set(None);
        missing.set(false);
        place.set(None);
        note.set(None);
        zoom.set(FIT_WIDTH);
        pdf_state.set(PdfState::default());
        spawn(async move {
            let full = fetched(id, AttachmentPart::File, mime).await;
            if full.is_none() && crate::ui::offline::is_offline_now() {
                missing.set(true);
                if mime == FileType::Jpeg.mime() {
                    preview
                        .set(fetched(id, AttachmentPart::Thumbnail, FileType::Jpeg.mime()).await);
                }
            }
            url.set(full);
        });
        if let Some(point) = coordinates {
            spawn(async move {
                // The place data's first choice, looked up each time; only coordinates are kept.
                if let Ok(PlacesOutcome::Located { located }) = api::locate(point).await
                    && let Some(suggestion) = located.suggestion
                {
                    place.set(Some(place_text(&suggestion.place)));
                }
            });
        }
    }));
    let english = Translator::english();
    let when = |at| {
        format!(
            "{}, {}",
            format_date(at, &zone(), units().date_format, english).unwrap_or_default(),
            format_time(at, &zone(), english).unwrap_or_default()
        )
    };
    let taken = metadata
        .as_ref()
        .and_then(|metadata| metadata.taken_at)
        .map(when);
    let camera = metadata
        .as_ref()
        .and_then(|metadata| metadata.camera.clone());
    let size_line = match (size, pixels) {
        (Some(size), Some((width, height))) => format!(
            "{} · {width} × {height} · {}",
            tr(size.label()),
            bytes_text(view.attachment.file.bytes)
        ),
        _not_a_photo => bytes_text(view.attachment.file.bytes),
    };
    let added = tr_with(
        messages::ATTACHMENT_ADDED_BY,
        &[
            ("name", MessageArg::from(view.added_by_name.as_str())),
            (
                "date",
                MessageArg::from(when(view.attachment.added_at).as_str()),
            ),
        ],
    );
    let name = view.attachment.name.as_str().to_owned();
    let download_name = name.clone();
    let original_name = name.clone();
    let download = move |part: AttachmentPart, file_name: String| {
        spawn(async move {
            let part_mime = if part == AttachmentPart::Original {
                "application/octet-stream"
            } else {
                mime
            };
            match files::fetch(id, part).await {
                Ok(bytes) => {
                    if let Some(path) = save_download(&file_name, part_mime, &bytes).await {
                        note.set(Some(tr_with(
                            messages::ATTACHMENT_SAVED_TO,
                            &[("path", MessageArg::from(path.as_str()))],
                        )));
                    }
                }
                Err(_) => note.set(Some(tr(messages::SOMETHING_WENT_WRONG))),
            }
        });
    };
    let previous = (index > 0).then(|| index - 1);
    let next = (index + 1 < count).then_some(index + 1);
    rsx! {
        div {
            class: "photo-viewer",
            role: "dialog",
            "aria-modal": "true",
            tabindex: "0",
            onmounted: move |event| async move { let _ = event.set_focus(true).await; },
            onkeydown: move |event| {
                let key = event.key();
                if key == Key::ArrowLeft && let Some(previous) = previous {
                    on_index.call(previous);
                } else if key == Key::ArrowRight && let Some(next) = next {
                    on_index.call(next);
                } else if key == Key::Escape {
                    on_close.call(false);
                }
            },
            div { class: "photo-viewer-bar",
                button { class: "link-button", r#type: "button", onclick: move |_| on_close.call(false), {tr(messages::ATTACHMENT_BACK)} }
                if is_pdf {
                    div { class: "pdf-tools",
                        button { class: "secondary-button", r#type: "button", disabled: zoom() == 0, "aria-label": tr(messages::PDF_ZOOM_OUT), onclick: move |_| zoom.set(zoom().saturating_sub(1)), "−" }
                        button { class: "secondary-button", r#type: "button", onclick: move |_| zoom.set(FIT_WIDTH), {tr(messages::PDF_FIT_WIDTH)} }
                        button { class: "secondary-button", r#type: "button", disabled: zoom() + 1 >= ZOOMS.len(), "aria-label": tr(messages::PDF_ZOOM_IN), onclick: move |_| zoom.set((zoom() + 1).min(ZOOMS.len() - 1)), "+" }
                        if pdf_state().pages > 0 {
                            span { class: "muted-note",
                                {tr_with(messages::PDF_PAGE_OF, &[
                                    ("page", MessageArg::from(i64::from(pdf_state().page))),
                                    ("pages", MessageArg::from(i64::from(pdf_state().pages))),
                                ])}
                            }
                        }
                    }
                }
                if count > 1 {
                    span { class: "muted-note",
                        {tr_with(messages::ATTACHMENT_POSITION, &[
                            ("at", MessageArg::from(i64::try_from(index + 1).unwrap_or(i64::MAX))),
                            ("count", MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX))),
                        ])}
                    }
                }
            }
            div { class: "photo-viewer-body",
                div { class: "photo-viewer-image",
                    if let Some(previous) = previous {
                        button { class: "photo-viewer-step photo-viewer-previous", r#type: "button", "aria-label": tr(messages::ATTACHMENT_PREVIOUS), onclick: move |_| on_index.call(previous), "‹" }
                    }
                    match url() {
                        Some(src) if mime == FileType::Jpeg.mime() => rsx! { img { src: "{src}", alt: "{name}" } },
                        Some(_) if pdf_state().failed => rsx! { p { class: "muted-note", {tr(messages::ATTACHMENT_PDF_CANT_SHOW)} } },
                        Some(src) => rsx! {
                            PdfPages {
                                key: "{id}",
                                container: format!("pdf-{id}"),
                                data: src.split_once(',').map(|(_, data)| data.to_owned()).unwrap_or_default(),
                                zoom,
                                state: pdf_state,
                            }
                        },
                        None if missing() => match preview() {
                            Some(src) => rsx! {
                                div { class: "photo-viewer-preview",
                                    img { src: "{src}", alt: "{name}" }
                                    p { class: "muted-note", {tr(messages::PHOTO_PREVIEW_NOTE)} }
                                }
                            },
                            None => rsx! { p { class: "muted-note", {tr(messages::FILE_NOT_HERE)} } },
                        },
                        None => rsx! { p { class: "muted-note", {tr(messages::LOADING)} } },
                    }
                    if let Some(next) = next {
                        button { class: "photo-viewer-step photo-viewer-next", r#type: "button", "aria-label": tr(messages::ATTACHMENT_NEXT), onclick: move |_| on_index.call(next), "›" }
                    }
                }
                div { class: "photo-viewer-details",
                    h2 { class: "photo-viewer-name", {name} }
                    dl {
                        if let Some(taken) = taken {
                            dt { {tr(messages::ATTACHMENT_TAKEN)} }
                            dd { {taken} }
                        }
                        if let Some(place) = place() {
                            dt { {tr(messages::ATTACHMENT_WHERE)} }
                            dd { {place} }
                        }
                        if let Some(camera) = camera {
                            dt { {tr(messages::ATTACHMENT_CAMERA)} }
                            dd { {camera} }
                        }
                        dt { {tr(messages::ATTACHMENT_SIZE)} }
                        dd { {size_line} }
                        dt { {tr(messages::ATTACHMENT_ADDED)} }
                        dd { {added} }
                    }
                    div { class: "check-row",
                        button { class: "secondary-button", r#type: "button", onclick: move |_| download(AttachmentPart::File, download_name.clone()), {tr(messages::ATTACHMENT_DOWNLOAD)} }
                        if has_original && may_keep_originals {
                            button { class: "secondary-button", r#type: "button", onclick: move |_| download(AttachmentPart::Original, original_name.clone()), {tr(messages::ATTACHMENT_ORIGINAL)} }
                        }
                        if view.may_change {
                            button {
                                class: "danger-button",
                                r#type: "button",
                                onclick: move |_| confirm.set(true),
                                {tr(messages::ACTION_REMOVE)}
                            }
                        }
                    }
                    if let Some(note) = note() {
                        p { class: "muted-note", role: "status", {note} }
                    }
                }
            }
        }
        if confirm() {
            RemoveDialog { id, on_close: move |removed: bool| {
                confirm.set(false);
                if removed {
                    on_close.call(true);
                }
            } }
        }
    }
}

/// "Remove this photo?"
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn RemoveDialog(id: AttachmentId, on_close: EventHandler<bool>) -> Element {
    let mut problem = use_signal(|| None::<MessageId>);
    let remove = move |_| {
        spawn(async move {
            match data::remove_attachment(id).await {
                Ok(AttachmentsOutcome::Done) => on_close.call(true),
                Ok(other) => problem.set(Some(refusal(&other))),
                Err(error) => problem.set(Some(problem_from_error(&error).message())),
            }
        });
    };
    rsx! {
        div { class: "dialog-scrim viewer-dialog", onclick: move |_| on_close.call(false),
            div { class: "dialog", role: "dialog", "aria-modal": "true", onclick: move |event| event.stop_propagation(),
                h2 { class: "dialog-title", {tr(messages::ATTACHMENT_REMOVE_TITLE)} }
                p { class: "dialog-text", {tr(messages::ATTACHMENT_REMOVE_TEXT)} }
                if let Some(message) = problem() {
                    p { class: "form-error", role: "alert", {tr(message)} }
                }
                div { class: "form-buttons",
                    button { class: "secondary-button", r#type: "button", onclick: move |_| on_close.call(false), {tr(messages::ACTION_CANCEL)} }
                    button { class: "danger-button", r#type: "button", onclick: remove, {tr(messages::ACTION_REMOVE)} }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sizes_read_as_kilobytes_or_megabytes() {
        assert_eq!(bytes_text(1), "1 KB");
        assert_eq!(bytes_text(350 * 1024), "350 KB");
        assert_eq!(bytes_text(1024 * 1024), "1.0 MB");
        assert_eq!(bytes_text(2_621_440), "2.5 MB");
    }
}
