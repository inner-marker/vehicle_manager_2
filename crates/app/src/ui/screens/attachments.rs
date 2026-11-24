//! Photos on events, tasks, squawks, and vehicles (milestone 8, step 8e; the human's pick:
//! option A of `docs/ui-sketches/p2m8-photos-options.svg`).
//!
//! - **A strip of thumbnails** under each thing ([`AttachmentStrip`]), with **+ Add** at the
//!   end for whoever may add there. PDFs show as a page tile (they're added in 8f).
//! - **+ Add** opens a sheet: **Take photo** or **Choose photos**, the **size** (Small, Medium,
//!   or Large; the default follows the kind of event, #036), and **Keep the original** for
//!   Maintainers and Admins when the household allows it.
//! - **Scan pages** ([`ScanPages`], phase-6.md 2c): photos of a document's pages, joined into
//!   one PDF by the server (or the desktop's copy).
//! - **On a record not saved yet** ([`HeldStrip`], phase-6.md 2b), files upload as they're
//!   added and are tied to the record when it's saved.
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
    SCAN_PAGES_MAX, pack_pages,
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
use crate::ui::icons::IconView;
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
    /// The vehicle's main photo, for its own photos' strip (#24).
    #[props(default)]
    main: Option<MainPhoto>,
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
                main,
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

/// The strip on a record that isn't saved yet (phase-6.md 2b; sketch part 1): what was added
/// so far, each with ✕, and + Add. The files are uploaded as they're added and held in `held`
/// until the record is saved ([`attach_held`]). Keep the original is offered to whoever may
/// keep originals on this vehicle's own photos.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn HeldStrip(
    asset: AssetId,
    held: Signal<Vec<Held>>,
    heading: MessageId,
    default_size: PhotoSize,
) -> Element {
    let mut adding = use_signal(|| false);
    let mut may_keep_originals = use_signal(|| false);
    use_effect(use_reactive!(|asset| {
        spawn(async move {
            let found = load(asset, AttachmentOwner::Vehicle).await;
            may_keep_originals.set(found.is_ok_and(|found| found.may_keep_originals));
        });
    }));
    let tiles: Vec<(usize, String, Option<String>, Option<u32>)> = held
        .read()
        .iter()
        .enumerate()
        .map(|(index, file)| {
            let pages = match file.content {
                Content::Document { pages } => Some(pages),
                Content::Photo { .. } => None,
            };
            (
                index,
                file.name.as_str().to_owned(),
                file.preview.clone(),
                pages,
            )
        })
        .collect();
    rsx! {
        section { class: "attachment-strip",
            h3 { class: "section-heading", {tr(heading).to_uppercase()} }
            div { class: "attachment-tiles",
                for (index, name, preview, pages) in tiles {
                    div { key: "{index}-{name}", class: "attachment-held",
                        div {
                            class: if pages.is_some() { "attachment-tile attachment-tile-pdf" } else { "attachment-tile" },
                            title: "{name}",
                            if let Some(src) = preview {
                                img { src: "{src}", alt: "" }
                            }
                            if let Some(pages) = pages {
                                span { class: "attachment-tile-badge", {tr_with(messages::ATTACHMENT_PDF_PAGES, &[("count", MessageArg::from(i64::from(pages)))])} }
                            }
                        }
                        button {
                            class: "attachment-held-remove",
                            r#type: "button",
                            "aria-label": tr_with(messages::ATTACHMENT_UNPICK, &[("name", MessageArg::from(name.as_str()))]),
                            onclick: move |_| {
                                let mut files = held.write();
                                if index < files.len() {
                                    files.remove(index);
                                }
                            },
                            "✕"
                        }
                    }
                }
                button {
                    class: "attachment-add",
                    r#type: "button",
                    onclick: move |_| adding.set(true),
                    {tr(messages::ATTACHMENT_ADD)}
                }
            }
            p { class: "muted-note", {tr(messages::ATTACHMENT_ADDED_ON_SAVE)} }
        }
        if adding() {
            AddSheet {
                asset,
                owner: AttachmentOwner::Vehicle,
                default_size,
                may_keep_originals: may_keep_originals(),
                on_close: move |_| adding.set(false),
                hold: Some(held),
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

/// A file uploaded for a record that isn't saved yet (phase-6.md 2b): tied to the record when
/// it's saved ([`attach_held`]), or dropped. Uploaded files nothing points to are left for the
/// server's file check, as after any upload that isn't added.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Held {
    name: FileName,
    file: domain::attachment::StoredFile,
    thumbnail: Option<domain::attachment::StoredFile>,
    content: Content,
    /// A `data:` URL to show it by before it's saved (the shrunk photo, or a PDF's first page).
    preview: Option<String>,
}

impl Held {
    /// What adds it to `owner` of vehicle `asset`.
    fn input(self, asset: AssetId, owner: AttachmentOwner) -> AttachmentInput {
        AttachmentInput {
            asset,
            owner,
            name: self.name,
            file: self.file,
            thumbnail: self.thumbnail,
            content: self.content,
        }
    }
}

/// What ties `held` to `owner` once it's saved, in the order they were added.
pub(crate) fn held_inputs(
    asset: AssetId,
    owner: AttachmentOwner,
    held: Vec<Held>,
) -> Vec<AttachmentInput> {
    held.into_iter()
        .map(|file| file.input(asset, owner))
        .collect()
}

/// Ties `held` to `owner` (a record just saved). Offline on the desktop, each goes into the
/// outbox after the record. Returns how many couldn't be added (logged; they can be added again
/// on the record's page).
pub(crate) async fn attach_held(asset: AssetId, owner: AttachmentOwner, held: Vec<Held>) -> usize {
    let mut failed = 0;
    for input in held_inputs(asset, owner, held) {
        if let Err(problem) = attach(input).await {
            tracing::warn!(target: log_area::UI, ?problem, "a file added before saving couldn't be tied to the record");
            failed += 1;
        }
    }
    failed
}

/// Adds an attachment from files already uploaded.
async fn attach(input: AttachmentInput) -> Result<(), MessageId> {
    match data::add_attachment(input).await {
        Ok(AttachmentsOutcome::Saved { id }) => {
            tracing::debug!(target: log_area::UI, attachment = %id, "attachment added");
            Ok(())
        }
        Ok(other) => Err(refusal(&other)),
        Err(error) => Err(problem_from_error(&error).message()),
    }
}

/// Everything about preparing one photo: read its metadata, shrink it, upload it (and its
/// original and metadata if kept).
async fn prepare_photo(
    file: dioxus::html::FileData,
    size: PhotoSize,
    keep_original: bool,
) -> Result<Held, MessageId> {
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
    let (sent, sent_mime) = match shrunk.jpeg {
        Some(jpeg) => (jpeg, "image/jpeg".to_owned()),
        None => (bytes.clone(), mime),
    };
    let preview = Some(files::data_url(&sent_mime, &sent));
    let photo = send(UploadPurpose::Photo { size }, sent).await?;
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
    Ok(Held {
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
        preview,
    })
}

/// Everything about preparing one PDF: upload it (the server checks it and counts its pages),
/// and draw and upload its first page as the thumbnail.
async fn prepare_pdf(file: dioxus::html::FileData) -> Result<Held, MessageId> {
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
    let first_page = first_page_thumbnail(&bytes).await;
    let preview = first_page
        .as_ref()
        .map(|jpeg| files::data_url(FileType::Jpeg.mime(), jpeg));
    let uploaded = send(UploadPurpose::Document, bytes).await?;
    let thumbnail = match first_page {
        // A thumbnail that doesn't upload isn't worth losing the PDF over: it shows a page tile.
        Some(jpeg) => send(UploadPurpose::DocumentThumbnail, jpeg)
            .await
            .ok()
            .map(|uploaded| uploaded.file),
        None => None,
    };
    Ok(Held {
        name,
        file: uploaded.file,
        thumbnail,
        content: Content::Document {
            pages: uploaded.pages.unwrap_or(1),
        },
        preview,
    })
}

/// One page of a scan: the photo as it will be sent (shrunk to the size chosen), and a
/// `data:` URL to show it by.
#[derive(Debug, Clone, PartialEq)]
struct ScanPage {
    photo: Vec<u8>,
    preview: String,
}

/// Moves the page at `index` one place earlier (or `later`), if there's a place to move to.
fn move_page<T>(pages: &mut [T], index: usize, later: bool) {
    let other = if later {
        index.checked_add(1)
    } else {
        index.checked_sub(1)
    };
    if let Some(other) = other
        && index < pages.len()
        && other < pages.len()
    {
        pages.swap(index, other);
    }
}

/// A scan's name: "Scan 2026-11-22.pdf", today's date on this device.
async fn scan_name() -> Result<FileName, MessageId> {
    let today = document::eval("return new Date().toLocaleDateString('sv');")
        .join::<String>()
        .await
        .unwrap_or_default();
    let name = if today.is_empty() {
        "Scan.pdf".to_owned()
    } else {
        format!("Scan {today}.pdf")
    };
    FileName::from_upload(&name).map_err(domain::attachment::AttachmentProblem::message)
}

/// Everything about preparing a scan (phase-6.md 2c): its pages, packed, go up as one upload,
/// which the server (or the desktop's copy) joins into a PDF with a thumbnail of page 1.
async fn prepare_scan(pages: Vec<ScanPage>, size: PhotoSize) -> Result<Held, MessageId> {
    let preview = pages.first().map(|page| page.preview.clone());
    let photos: Vec<Vec<u8>> = pages.into_iter().map(|page| page.photo).collect();
    let uploaded = send(UploadPurpose::ScannedPages { size }, pack_pages(&photos)).await?;
    Ok(Held {
        name: scan_name().await?,
        file: uploaded.file,
        thumbnail: uploaded.thumbnail,
        content: Content::Document {
            pages: uploaded.pages.unwrap_or(1),
        },
        preview,
    })
}

/// Reads and shrinks a page just taken or chosen; one the webview can't read goes as it is
/// (the server reads it, or refuses it).
async fn scan_page(file: dioxus::html::FileData, size: PhotoSize) -> Result<ScanPage, MessageId> {
    let bytes = file
        .read_bytes()
        .await
        .map_err(|_| messages::ATTACHMENT_UNREADABLE)?
        .to_vec();
    let mime = file
        .content_type()
        .unwrap_or_else(|| "image/jpeg".to_owned());
    let (photo, mime) = match shrink(&bytes, &mime, size.long_edge(), None).await.jpeg {
        Some(jpeg) => (jpeg, "image/jpeg".to_owned()),
        None => (bytes, mime),
    };
    Ok(ScanPage {
        preview: files::data_url(&mime, &photo),
        photo,
    })
}

/// **Scan pages** (sketch part 2): each shot is a page; ‹ › move one, ✕ drops it; **Make PDF**
/// joins them. `on_done` gets the PDF, uploaded.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn ScanPages(size: PhotoSize, on_done: EventHandler<Held>, on_cancel: EventHandler<()>) -> Element {
    let mut pages = use_signal(Vec::<ScanPage>::new);
    let mut busy = use_signal(|| false);
    let mut problem = use_signal(|| None::<MessageId>);
    let mut add = move |files: Vec<dioxus::html::FileData>| {
        problem.set(None);
        busy.set(true);
        spawn(async move {
            for file in files {
                if pages.read().len() >= SCAN_PAGES_MAX {
                    problem.set(Some(messages::ATTACHMENT_TOO_MANY_PAGES));
                    break;
                }
                match scan_page(file, size).await {
                    Ok(page) => pages.write().push(page),
                    Err(refused) => problem.set(Some(refused)),
                }
            }
            busy.set(false);
        });
    };
    let make = move |_| {
        let taken = pages();
        if taken.is_empty() {
            return;
        }
        problem.set(None);
        busy.set(true);
        spawn(async move {
            match prepare_scan(taken, size).await {
                Ok(held) => on_done.call(held),
                Err(refused) => problem.set(Some(refused)),
            }
            busy.set(false);
        });
    };
    let count = pages.read().len();
    let tiles: Vec<(usize, String)> = pages
        .read()
        .iter()
        .enumerate()
        .map(|(index, page)| (index, page.preview.clone()))
        .collect();
    let count_arg = || MessageArg::from(i64::try_from(count).unwrap_or(i64::MAX));
    rsx! {
        h2 { class: "dialog-title", {tr(messages::SCAN_TITLE)} }
        p { class: "muted-note", {tr_with(messages::SCAN_COUNT, &[("count", count_arg())])} }
        div { class: "scan-pages",
            for (index, preview) in tiles {
                div { key: "{index}-{preview.len()}", class: "scan-page",
                    img { src: "{preview}", alt: tr_with(messages::SCAN_PAGE, &[("number", MessageArg::from(i64::try_from(index + 1).unwrap_or(i64::MAX)))]) }
                    button {
                        class: "attachment-held-remove",
                        r#type: "button",
                        disabled: busy(),
                        "aria-label": tr(messages::SCAN_REMOVE_PAGE),
                        onclick: move |_| {
                            let mut list = pages.write();
                            if index < list.len() {
                                list.remove(index);
                            }
                        },
                        "✕"
                    }
                    div { class: "scan-page-order",
                        button {
                            class: "link-button",
                            r#type: "button",
                            disabled: busy() || index == 0,
                            "aria-label": tr(messages::SCAN_EARLIER),
                            onclick: move |_| move_page(&mut pages.write(), index, false),
                            "‹"
                        }
                        span { "{index + 1}" }
                        button {
                            class: "link-button",
                            r#type: "button",
                            disabled: busy() || index + 1 == count,
                            "aria-label": tr(messages::SCAN_LATER),
                            onclick: move |_| move_page(&mut pages.write(), index, true),
                            "›"
                        }
                    }
                }
            }
        }
        label { class: "sheet-choice",
            input {
                r#type: "file",
                accept: "image/*",
                capture: "environment",
                multiple: true,
                disabled: busy(),
                onchange: move |event| add(event.files()),
            }
            span { class: "sheet-choice-label", {tr(if count == 0 { messages::SCAN_FIRST_PAGE } else { messages::SCAN_NEXT_PAGE })} }
            span { class: "sheet-choice-hint", {tr(messages::SCAN_PAGE_HINT)} }
        }
        if let Some(refused) = problem() {
            p { class: "form-error", role: "alert", {tr(refused)} }
        }
        if busy() {
            p { class: "muted-note", role: "status", {tr(messages::SCAN_WORKING)} }
        }
        div { class: "form-buttons",
            button {
                class: "secondary-button",
                r#type: "button",
                disabled: busy(),
                onclick: move |_| on_cancel.call(()),
                {tr(messages::ACTION_CANCEL)}
            }
            button {
                class: "primary-button",
                r#type: "button",
                disabled: busy() || count == 0,
                onclick: make,
                {tr_with(messages::SCAN_MAKE_PDF, &[("count", count_arg())])}
            }
        }
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
    /// A record not saved yet: files are uploaded and held here, not added to `owner`.
    #[props(default)]
    hold: Option<Signal<Vec<Held>>>,
) -> Element {
    let mut size = use_signal(|| default_size);
    let mut keep_original = use_signal(|| false);
    let mut progress = use_signal(|| None::<(usize, usize)>);
    let mut problems = use_signal(Vec::<String>::new);
    // What was taken or chosen, waiting for Add (the human's M8 note: the sheet stays open after
    // the camera, so the size can still change, or it can be cancelled). `true` for a PDF.
    let mut picked = use_signal(Vec::<(dioxus::html::FileData, bool)>::new);
    let mut added_any = use_signal(|| false);
    // Scan pages (phase-6.md 2c): the sheet shows the scan's pages instead.
    let mut scanning = use_signal(|| false);
    let busy = progress().is_some();
    let scanned = move |held: Held| {
        scanning.set(false);
        match hold {
            Some(mut hold) => {
                hold.write().push(held);
                on_close.call(true);
            }
            None => {
                spawn(async move {
                    match attach(held.input(asset, owner)).await {
                        Ok(()) => on_close.call(true),
                        Err(problem) => problems.set(vec![tr(problem)]),
                    }
                });
            }
        }
    };
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
                let prepared = if pdf {
                    prepare_pdf(file.clone()).await
                } else {
                    prepare_photo(
                        file.clone(),
                        size(),
                        keep_original() && !crate::ui::offline::is_offline_now(),
                    )
                    .await
                };
                let added = match (prepared, hold) {
                    (Ok(prepared), Some(mut hold)) => {
                        hold.write().push(prepared);
                        Ok(())
                    }
                    (Ok(prepared), None) => attach(prepared.input(asset, owner)).await,
                    (Err(problem), _) => Err(problem),
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
                if scanning() {
                    ScanPages { size: size(), on_done: scanned, on_cancel: move |()| scanning.set(false) }
                } else {
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
                button {
                    class: "sheet-choice",
                    r#type: "button",
                    disabled: busy,
                    onclick: move |_| {
                        problems.set(Vec::new());
                        scanning.set(true);
                    },
                    span { class: "sheet-choice-label", {tr(messages::SCAN_CHOICE)} }
                    span { class: "sheet-choice-hint", {tr(messages::SCAN_CHOICE_HINT)} }
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

/// A vehicle's main photo, for the viewer of its own photos (#24, phase-6.md 2h).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct MainPhoto {
    /// The photo that's its main one, if any.
    pub current: Option<AttachmentId>,
    /// They may choose it (whoever may change the vehicle's own photos).
    pub may_choose: bool,
    /// Called with the new main photo (or none).
    pub on_choose: EventHandler<Option<AttachmentId>>,
}

/// The viewer's star (sketch part 5): gold ★ on the main photo, hollow ☆ on the others
/// ("Make Main Photo"); clicking toggles. Needs the server (2h). Without the right to choose,
/// only the main photo's gold star shows.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
fn MainStar(main: MainPhoto, photo: AttachmentId) -> Element {
    let is_main = main.current == Some(photo);
    if !main.may_choose {
        return if is_main {
            rsx! { span { class: "main-star on", title: tr(messages::MAIN_PHOTO_IS), "aria-label": tr(messages::MAIN_PHOTO_IS), "★" } }
        } else {
            rsx! {}
        };
    }
    let offline = crate::ui::offline::needs_server_note();
    let label = tr(if is_main {
        messages::MAIN_PHOTO_CLEAR
    } else {
        messages::MAIN_PHOTO_MAKE
    });
    rsx! {
        button {
            class: if is_main { "main-star on" } else { "main-star" },
            r#type: "button",
            title: offline.clone().unwrap_or_else(|| label.clone()),
            "aria-label": label,
            "aria-pressed": if is_main { "true" } else { "false" },
            disabled: offline.is_some(),
            onclick: move |_| main.on_choose.call((!is_main).then_some(photo)),
            if is_main { "★" } else { "☆" }
        }
    }
}

/// A vehicle's main photo as a square thumbnail (#24, phase-6.md 2h): on its Garage card and the
/// Overview's header. Its kind's icon while it has none (or the photo can't be fetched).
#[component]
pub(crate) fn VehiclePhoto(
    photo: Option<AttachmentId>,
    kind: domain::asset::AssetKind,
    class: &'static str,
) -> Element {
    let mut url = use_signal(|| None::<String>);
    use_effect(use_reactive!(|photo| {
        url.set(None);
        if let Some(id) = photo {
            spawn(async move {
                url.set(fetched(id, AttachmentPart::Thumbnail, FileType::Jpeg.mime()).await);
            });
        }
    }));
    rsx! {
        span { class: "vehicle-photo {class}", "aria-hidden": "true",
            match url() {
                Some(src) => rsx! { img { src: "{src}", alt: "" } },
                None => rsx! { IconView { icon: crate::ui::vehicles::kind_icon(&kind) } },
            }
        }
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
    main: Option<MainPhoto>,
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
                if let Some(main) = main.filter(|_| !is_pdf) {
                    MainStar { main, photo: id }
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
                        Some(src) if mime == FileType::Jpeg.mime() => {
                            // Pinch to zoom (phase-6.md 2d): a frame per photo, so each starts at 1×.
                            let frame = crate::ui::photo_zoom::frame_id(id);
                            let mounted = frame.clone();
                            rsx! {
                                div {
                                    key: "{frame}",
                                    id: "{frame}",
                                    class: "photo-zoom",
                                    onmounted: move |_| crate::ui::photo_zoom::start(&mounted),
                                    img { src: "{src}", alt: "{name}", draggable: "false" }
                                }
                            }
                        }
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

    #[test]
    fn held_files_are_tied_to_the_saved_record_in_the_order_added() {
        use domain::attachment::{FileHash, StoredFile};
        use domain::test_support::test_id;
        let held = |byte: u8, name: &str, pages: u32| Held {
            name: FileName::from_upload(name).unwrap(),
            file: StoredFile {
                hash: FileHash::from_bytes([byte; 32]),
                bytes: 1000,
            },
            thumbnail: None,
            content: Content::Document { pages },
            preview: None,
        };
        let asset: AssetId = test_id(1);
        let owner = AttachmentOwner::Event {
            event: test_id(2),
            task: None,
        };
        let inputs = held_inputs(
            asset,
            owner,
            vec![held(1, "receipt.pdf", 1), held(2, "invoice.pdf", 3)],
        );
        let names: Vec<&str> = inputs.iter().map(|input| input.name.as_str()).collect();
        assert_eq!(names, vec!["receipt.pdf", "invoice.pdf"]);
        assert!(
            inputs
                .iter()
                .all(|input| input.asset == asset && input.owner == owner)
        );
        assert_eq!(inputs[1].content, Content::Document { pages: 3 });
        assert!(held_inputs(asset, owner, Vec::new()).is_empty());
    }

    #[test]
    fn a_scans_pages_move_one_place_and_stay_in_range() {
        let mut pages = vec!['a', 'b', 'c'];
        move_page(&mut pages, 2, false);
        assert_eq!(pages, vec!['a', 'c', 'b']);
        move_page(&mut pages, 0, true);
        assert_eq!(pages, vec!['c', 'a', 'b']);
        // The first can't go earlier, the last can't go later, and past the end is nothing.
        move_page(&mut pages, 0, false);
        move_page(&mut pages, 2, true);
        move_page(&mut pages, 7, true);
        assert_eq!(pages, vec!['c', 'a', 'b']);
    }
}
