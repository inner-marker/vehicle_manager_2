//! Photos and PDFs on the server (milestone 8; plan §3 Attachments; decisions #036, #037,
//! #065).
//!
//! **Uploading** stores a file and says what it is ([`upload`]): a photo is always decoded,
//! turned upright, brought down to its size, and saved again as a JPEG at quality 85 with a
//! 400 px thumbnail, so what's kept never carries the camera's metadata (#036); a PDF is
//! checked and its pages counted (#037). **Adding** then makes the attachment from files
//! already uploaded ([`add`]); the server works out a photo's pixels and a PDF's pages from
//! the stored files itself.
//!
//! Who may add, move, or remove one (choice 7): whoever may change what it's on.
//! - **A vehicle's own:** whoever may change the vehicle (an Admin on it).
//! - **An event's (or one of its tasks'):** whoever may change the event, **while it's a
//!   draft**; after that, a correction (choice 6), which starts with the original's
//!   attachments (`copy_to_correction`).
//! - **A squawk's:** whoever may change the squawk, at any time.
//! - **A document's:** a Maintainer (in 8d.2).
//! - **A reference document's** (on no vehicle, milestone 9): whoever may change the reference
//!   document; seeing one needs to be able to read it (`crate::references`).
//!
//! Once a vehicle's disposal is finalized (#067), nothing new is attached to it, except to an
//! event (a draft, so a correction).
//!
//! **Seeing** one needs only to see its vehicle, except a photo's original: only those who may
//! keep originals (Maintainers and Admins, #036).

use std::io::Cursor;

use domain::access::{AssetAction, AssetScope, highest_role, permits_on_asset};
use domain::api::{
    AssetsOutcome, AttachmentInput, AttachmentPart, AttachmentView, AttachmentsOutcome,
    UploadOutcome, UploadPurpose, UploadedFile,
};
use domain::asset::AssetId;
use domain::attachment::{
    Attachment, AttachmentId, AttachmentKind, AttachmentOwner, AttachmentPatch, AttachmentProblem,
    Content, FileName, FileType, JPEG_QUALITY, PhotoSize, StoredFile, check as check_attachment,
    check_original, check_upload, for_correction, thumbnail_size,
};
use domain::command::Change;
use domain::event::{EventId, EventStatus};
use domain::log_area;
use domain::preferences::{Originals, PreferencesId};
use domain::reference::ReferenceDocumentId;
use domain::time::UnixMillis;
use image::{DynamicImage, ImageDecoder, ImageReader, Limits, RgbImage};
use storage::Database;
use storage::attachments::AttachmentWriter;
use storage::files::FileStore;

use crate::assets::{AssetsContext, unavailable};
use crate::sessions::SignedIn;

/// The most bytes a photo's metadata (as read) may have.
pub const METADATA_MAX_BYTES: u64 = 256 * 1024;

/// The longest side the server will decode: well past any camera, short of a decompression
/// bomb.
const MAX_SIDE: u32 = 20_000;

/// The most memory decoding one image may take.
const MAX_DECODE_BYTES: u64 = 1024 * 1024 * 1024;

// --- Processing --------------------------------------------------------------------------

/// A photo as the server keeps it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProcessedPhoto {
    /// The JPEG.
    pub jpeg: Vec<u8>,
    /// Its width and height.
    pub pixels: (u32, u32),
    /// Its 400 px thumbnail, a JPEG.
    pub thumbnail: Vec<u8>,
}

/// `bytes` decoded (JPEG or PNG) and turned upright.
fn decode_upright(bytes: &[u8]) -> Result<DynamicImage, AttachmentProblem> {
    let mut reader = ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AttachmentProblem::Unreadable)?;
    let mut limits = Limits::default();
    limits.max_image_width = Some(MAX_SIDE);
    limits.max_image_height = Some(MAX_SIDE);
    limits.max_alloc = Some(MAX_DECODE_BYTES);
    reader.limits(limits);
    let mut decoder = reader
        .into_decoder()
        .map_err(|_| AttachmentProblem::Unreadable)?;
    let orientation = decoder
        .orientation()
        .map_err(|_| AttachmentProblem::Unreadable)?;
    let mut image =
        DynamicImage::from_decoder(decoder).map_err(|_| AttachmentProblem::Unreadable)?;
    image.apply_orientation(orientation);
    Ok(image)
}

/// `image` as RGB, any transparency laid over white (JPEG has none).
fn flattened(image: &DynamicImage) -> RgbImage {
    if !image.has_alpha() {
        return image.to_rgb8();
    }
    let rgba = image.to_rgba8();
    RgbImage::from_fn(rgba.width(), rgba.height(), |x, y| {
        let [red, green, blue, alpha] = rgba.get_pixel(x, y).0;
        let over_white = |channel: u8| {
            let mixed =
                (u16::from(channel) * u16::from(alpha) + 255 * (255 - u16::from(alpha)) + 127)
                    / 255;
            u8::try_from(mixed).unwrap_or(u8::MAX)
        };
        image::Rgb([over_white(red), over_white(green), over_white(blue)])
    })
}

/// `image` brought down to `width` × `height` and saved as a JPEG.
fn jpeg_at(image: &RgbImage, (width, height): (u32, u32)) -> Result<Vec<u8>, AttachmentProblem> {
    let resized = if (image.width(), image.height()) == (width, height) {
        image.clone()
    } else {
        image::imageops::resize(image, width, height, image::imageops::FilterType::Lanczos3)
    };
    let mut jpeg = Vec::new();
    image::codecs::jpeg::JpegEncoder::new_with_quality(&mut jpeg, JPEG_QUALITY)
        .encode_image(&resized)
        .map_err(|_| AttachmentProblem::Unreadable)?;
    Ok(jpeg)
}

/// A photo (JPEG or PNG) as the server keeps it at `size`: upright, its long edge at most the
/// size's, saved again as a JPEG (which drops its metadata), with a thumbnail.
///
/// # Errors
///
/// [`AttachmentProblem::Unreadable`] if it can't be decoded.
pub fn process_photo(bytes: &[u8], size: PhotoSize) -> Result<ProcessedPhoto, AttachmentProblem> {
    let image = flattened(&decode_upright(bytes)?);
    let pixels = size.fit(image.width(), image.height());
    let jpeg = jpeg_at(&image, pixels)?;
    let thumbnail = jpeg_at(&image, thumbnail_size(image.width(), image.height()))?;
    Ok(ProcessedPhoto {
        jpeg,
        pixels,
        thumbnail,
    })
}

/// A PDF's first page, as the app drew it (JPEG or PNG), as a 400 px JPEG thumbnail.
///
/// # Errors
///
/// [`AttachmentProblem::Unreadable`] if it can't be decoded.
pub fn process_thumbnail(bytes: &[u8]) -> Result<Vec<u8>, AttachmentProblem> {
    let image = flattened(&decode_upright(bytes)?);
    jpeg_at(&image, thumbnail_size(image.width(), image.height()))
}

/// How many pages a PDF has.
///
/// # Errors
///
/// [`AttachmentProblem::Unreadable`] if it can't be opened (damaged, or locked with a password),
/// or [`AttachmentProblem::NoPages`].
pub fn count_pages(bytes: &[u8]) -> Result<u32, AttachmentProblem> {
    let document = lopdf::Document::load_mem(bytes).map_err(|_| AttachmentProblem::Unreadable)?;
    match u32::try_from(document.get_pages().len()) {
        Ok(0) => Err(AttachmentProblem::NoPages),
        Ok(pages) => Ok(pages),
        Err(_) => Err(AttachmentProblem::Unreadable),
    }
}

/// A stored photo's width and height, from its header.
fn pixels_of(bytes: &[u8]) -> Result<(u32, u32), AttachmentProblem> {
    ImageReader::new(Cursor::new(bytes))
        .with_guessed_format()
        .map_err(|_| AttachmentProblem::Unreadable)?
        .into_dimensions()
        .map_err(|_| AttachmentProblem::Unreadable)
}

/// The size of `bytes`, as stored sizes are counted.
fn length(bytes: &[u8]) -> u64 {
    u64::try_from(bytes.len()).unwrap_or(u64::MAX)
}

/// Runs `work` (decoding, counting pages, reading or writing files) off the async threads.
async fn blocking<T: Send + 'static>(
    work: impl FnOnce() -> T + Send + 'static,
) -> Result<T, tokio::task::JoinError> {
    tokio::task::spawn_blocking(work).await
}

// --- Uploading ---------------------------------------------------------------------------

/// What an upload needs.
pub struct UploadContext<'a> {
    /// The database (to list the stored files).
    pub database: &'a Database,
    /// The file store.
    pub store: &'a FileStore,
    /// Who is uploading.
    pub user: &'a SignedIn,
    /// The server's health.
    pub health: crate::health::Health,
    /// Now.
    pub now: UnixMillis,
}

/// Stores `bytes` (and lists them), returning what was stored.
async fn store_bytes(
    context: &UploadContext<'_>,
    bytes: Vec<u8>,
) -> Result<StoredFile, UploadOutcome> {
    let store = context.store.clone();
    let stored = blocking(move || store.put(&bytes))
        .await
        .map_err(|error| upload_failed("store", &error))?
        .map_err(|error| upload_failed("store", &error))?;
    context
        .database
        .record_stored_file(&stored, context.now)
        .await
        .map_err(|error| upload_failed("list", &error))?;
    Ok(stored)
}

/// Logs a failure and answers "unavailable".
fn upload_failed(what: &str, error: &dyn std::fmt::Display) -> UploadOutcome {
    tracing::error!(target: log_area::APP, what, %error, "upload failed");
    UploadOutcome::Unavailable
}

/// Stores an upload for `purpose`. Anyone who may add attachments somewhere (a Driver or
/// higher, in that mode) may upload; a file no attachment comes to use is erased later.
pub async fn upload(
    context: &UploadContext<'_>,
    purpose: UploadPurpose,
    bytes: Vec<u8>,
) -> UploadOutcome {
    if !context.health.is_healthy() {
        return UploadOutcome::Unavailable;
    }
    let role = highest_role(&context.user.grants).map(|role| role.min(context.user.mode.cap()));
    if role.is_none_or(|role| role < domain::access::Role::Driver) {
        return UploadOutcome::NotAllowed;
    }
    match upload_checked(context, purpose, bytes).await {
        Ok(uploaded) => UploadOutcome::Uploaded { uploaded },
        Err(outcome) => outcome,
    }
}

async fn upload_checked(
    context: &UploadContext<'_>,
    purpose: UploadPurpose,
    bytes: Vec<u8>,
) -> Result<UploadedFile, UploadOutcome> {
    let invalid = |problem| UploadOutcome::Invalid { problem };
    let file_type = FileType::sniff(&bytes);
    let size = length(&bytes);
    match purpose {
        UploadPurpose::Photo { size: photo_size } => {
            check_upload(AttachmentKind::Photo, file_type, size).map_err(invalid)?;
            let processed = blocking(move || process_photo(&bytes, photo_size))
                .await
                .map_err(|error| upload_failed("photo", &error))?
                .map_err(invalid)?;
            let file = store_bytes(context, processed.jpeg).await?;
            let thumbnail = store_bytes(context, processed.thumbnail).await?;
            Ok(UploadedFile {
                file,
                file_type: Some(FileType::Jpeg),
                thumbnail: Some(thumbnail),
                pixels: Some(processed.pixels),
                pages: None,
            })
        }
        UploadPurpose::Original => {
            check_original(file_type, size).map_err(invalid)?;
            Ok(UploadedFile {
                file: store_bytes(context, bytes).await?,
                file_type,
                thumbnail: None,
                pixels: None,
                pages: None,
            })
        }
        UploadPurpose::PhotoMetadata => {
            if size == 0 {
                return Err(invalid(AttachmentProblem::Empty));
            }
            if size > METADATA_MAX_BYTES {
                return Err(invalid(AttachmentProblem::PhotoTooLarge));
            }
            Ok(UploadedFile {
                file: store_bytes(context, bytes).await?,
                file_type: None,
                thumbnail: None,
                pixels: None,
                pages: None,
            })
        }
        UploadPurpose::Document => {
            check_upload(AttachmentKind::Document, file_type, size).map_err(invalid)?;
            let (pages, bytes) = blocking(move || (count_pages(&bytes), bytes))
                .await
                .map_err(|error| upload_failed("pdf", &error))?;
            let pages = pages.map_err(invalid)?;
            Ok(UploadedFile {
                file: store_bytes(context, bytes).await?,
                file_type: Some(FileType::Pdf),
                thumbnail: None,
                pixels: None,
                pages: Some(pages),
            })
        }
        UploadPurpose::DocumentThumbnail => {
            check_upload(AttachmentKind::Photo, file_type, size).map_err(invalid)?;
            let thumbnail = blocking(move || process_thumbnail(&bytes))
                .await
                .map_err(|error| upload_failed("thumbnail", &error))?
                .map_err(invalid)?;
            Ok(UploadedFile {
                file: store_bytes(context, thumbnail).await?,
                file_type: Some(FileType::Jpeg),
                thumbnail: None,
                pixels: None,
                pages: None,
            })
        }
    }
}

// --- Attachments -------------------------------------------------------------------------

/// A vehicle request's refusal as an attachments answer.
fn from_assets(outcome: &AssetsOutcome) -> AttachmentsOutcome {
    match outcome {
        AssetsOutcome::NotFound => AttachmentsOutcome::NotFound,
        AssetsOutcome::NotAllowed => AttachmentsOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => AttachmentsOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> AttachmentsOutcome {
    from_assets(&unavailable(what, error))
}

/// Turns `Result<AttachmentsOutcome, AttachmentsOutcome>` into the outcome either way.
fn either(result: Result<AttachmentsOutcome, AttachmentsOutcome>) -> AttachmentsOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

const fn invalid(problem: AttachmentProblem) -> AttachmentsOutcome {
    AttachmentsOutcome::Invalid { problem }
}

/// Whether the person may add to, move, or remove from `owner`, and how many tasks it has
/// (an event's; otherwise 0).
struct Permission {
    allowed: bool,
    tasks: usize,
}

impl AssetsContext<'_> {
    fn permits_attachment(&self, action: AssetAction, scope: AssetScope) -> bool {
        permits_on_asset(&self.user.grants, self.user.mode, action, scope)
    }

    /// Whether the person may change what's attached to `owner` on `asset` (whose scope is
    /// `scope`), checking `owner` is on that vehicle. An event must still be a draft; anything
    /// else takes nothing new once the vehicle's disposal is finalized (#067).
    async fn attachment_permission(
        &self,
        asset: AssetId,
        owner: AttachmentOwner,
        scope: AssetScope,
    ) -> Result<Permission, AttachmentsOutcome> {
        let mut permission = self.owner_permission(asset, owner, scope).await?;
        if permission.allowed
            && owner.event().is_none()
            && !crate::lifecycle::takes_new_records(self.database, asset)
                .await
                .map_err(|error| failed("lifecycle", &error))?
        {
            permission.allowed = false;
        }
        Ok(permission)
    }

    /// Whether the person may change a reference document's attachments (#068): it must be
    /// one they can read, and they must be allowed to change it.
    async fn reference_permission(
        &self,
        reference: ReferenceDocumentId,
    ) -> Result<Permission, AttachmentsOutcome> {
        if !crate::references::readable(self, reference)
            .await
            .map_err(|error| failed("reference", &error))?
        {
            return Err(AttachmentsOutcome::NotFound);
        }
        Ok(Permission {
            allowed: crate::references::changeable(self, reference)
                .await
                .map_err(|error| failed("reference", &error))?,
            tasks: 0,
        })
    }

    /// Where an attachment's owner is and what the person may do there: on a vehicle (its ID
    /// and scope), or a reference document's (on none).
    async fn place_of(
        &self,
        asset: Option<AssetId>,
        owner: AttachmentOwner,
    ) -> Result<(Option<(AssetId, AssetScope)>, Permission), AttachmentsOutcome> {
        if let AttachmentOwner::Reference { reference } = owner {
            return Ok((None, self.reference_permission(reference).await?));
        }
        let asset = asset.ok_or(AttachmentsOutcome::NotFound)?;
        let scope = self.attachment_scope(asset).await?;
        let permission = self.attachment_permission(asset, owner, scope).await?;
        Ok((Some((asset, scope)), permission))
    }

    async fn owner_permission(
        &self,
        asset: AssetId,
        owner: AttachmentOwner,
        scope: AssetScope,
    ) -> Result<Permission, AttachmentsOutcome> {
        match owner {
            AttachmentOwner::Vehicle => Ok(Permission {
                allowed: self.permits_attachment(AssetAction::ManageAsset, scope),
                tasks: 0,
            }),
            AttachmentOwner::Event { event, .. } => {
                let found = self
                    .database
                    .event(event)
                    .await
                    .map_err(|error| failed("event", &error))?
                    .filter(|found| found.asset == asset)
                    .ok_or(AttachmentsOutcome::NotFound)?;
                if found.status == EventStatus::Finalized {
                    return Err(invalid(AttachmentProblem::EventFinished));
                }
                Ok(Permission {
                    allowed: self.may_manage(&found, scope),
                    tasks: found.work.map_or(0, |work| work.tasks.len()),
                })
            }
            AttachmentOwner::Squawk { squawk } => {
                let found = self
                    .database
                    .squawk(squawk)
                    .await
                    .map_err(|error| failed("squawk", &error))?
                    .filter(|found| found.asset == asset)
                    .ok_or(AttachmentsOutcome::NotFound)?;
                Ok(Permission {
                    allowed: self.may_change_squawk(&found, scope),
                    tasks: 0,
                })
            }
            AttachmentOwner::Document { document } => {
                self.database
                    .document(document)
                    .await
                    .map_err(|error| failed("document", &error))?
                    .filter(|found| found.asset == asset)
                    .ok_or(AttachmentsOutcome::NotFound)?;
                Ok(Permission {
                    allowed: self.permits_attachment(AssetAction::ManageDocuments, scope),
                    tasks: 0,
                })
            }
            AttachmentOwner::Reference { reference } => self.reference_permission(reference).await,
        }
    }

    /// Whether the household lets originals be kept (#036).
    async fn originals_allowed(&self) -> Result<bool, AttachmentsOutcome> {
        let household: PreferencesId = self.database_id.cast();
        let choices = self
            .database
            .preference_choices(household)
            .await
            .map_err(|error| failed("preferences", &error))?;
        Ok(choices.originals.unwrap_or_default() == Originals::Allowed)
    }

    /// The vehicle's scope, if the person can see it.
    async fn attachment_scope(&self, asset: AssetId) -> Result<AssetScope, AttachmentsOutcome> {
        self.visible(asset)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    /// The attachment and where it is (its vehicle and scope; none for a reference
    /// document's), if the person can see it.
    async fn visible_attachment(
        &self,
        id: AttachmentId,
    ) -> Result<(Attachment, Option<(AssetId, AssetScope)>), AttachmentsOutcome> {
        let attachment = self
            .database
            .attachment(id)
            .await
            .map_err(|error| failed("attachment", &error))?
            .ok_or(AttachmentsOutcome::NotFound)?;
        if let AttachmentOwner::Reference { reference } = attachment.owner {
            self.reference_permission(reference).await?;
            return Ok((attachment, None));
        }
        let asset = attachment.asset.ok_or(AttachmentsOutcome::NotFound)?;
        let scope = self.attachment_scope(asset).await?;
        Ok((attachment, Some((asset, scope))))
    }

    /// The attachment, if the person can see it and may change what it's on.
    async fn changeable_attachment(
        &self,
        id: AttachmentId,
    ) -> Result<Attachment, AttachmentsOutcome> {
        let (attachment, _) = self.visible_attachment(id).await?;
        let (_, permission) = self.place_of(attachment.asset, attachment.owner).await?;
        if !permission.allowed {
            return Err(AttachmentsOutcome::NotAllowed);
        }
        Ok(attachment)
    }

    /// `file` as the store lists it, if it was uploaded with that size.
    async fn uploaded(&self, file: &StoredFile) -> Result<(), AttachmentsOutcome> {
        let listed = self
            .database
            .stored_file(&file.hash)
            .await
            .map_err(|error| failed("stored file", &error))?;
        if listed.as_ref() == Some(file) {
            Ok(())
        } else {
            Err(invalid(AttachmentProblem::NotUploaded))
        }
    }

    /// Saves an attachment change as the person.
    async fn save_attachment(&self, change: Change<Attachment>) -> Result<(), AttachmentsOutcome> {
        self.save(&AttachmentWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }
}

/// What's attached to `owner` on `asset` (an event's includes its tasks'), oldest first.
pub async fn list(
    context: &AssetsContext<'_>,
    asset: AssetId,
    owner: AttachmentOwner,
) -> AttachmentsOutcome {
    either(list_attachments(context, asset, owner).await)
}

async fn list_attachments(
    context: &AssetsContext<'_>,
    asset: AssetId,
    owner: AttachmentOwner,
) -> Result<AttachmentsOutcome, AttachmentsOutcome> {
    // A finished event's attachments are shown, but nobody may change them.
    let (place, allowed) = match context.place_of(Some(asset), owner).await {
        Ok((place, permission)) => (place, permission.allowed),
        Err(AttachmentsOutcome::Invalid {
            problem: AttachmentProblem::EventFinished,
        }) => (Some((asset, context.attachment_scope(asset).await?)), false),
        Err(outcome) => return Err(outcome),
    };
    let found = match owner {
        AttachmentOwner::Reference { reference } => {
            context.database.attachments_of_reference(reference).await
        }
        AttachmentOwner::Vehicle
        | AttachmentOwner::Event { .. }
        | AttachmentOwner::Squawk { .. }
        | AttachmentOwner::Document { .. } => context.database.attachments_on(asset, owner).await,
    }
    .map_err(|error| failed("attachments", &error))?;
    let mut attachments = Vec::with_capacity(found.len());
    for (id, attachment) in found {
        let added_by_name = context
            .database
            .user(attachment.added_by)
            .await
            .map_err(|error| failed("user", &error))?
            .map(|stored| stored.user.display_name)
            .unwrap_or_default();
        attachments.push(AttachmentView {
            id,
            attachment,
            added_by_name,
            may_change: allowed,
        });
    }
    // Only on a vehicle: a reference document's photos are kept as shown.
    let may_keep_originals = allowed
        && place.is_some_and(|(_, scope)| {
            context.permits_attachment(AssetAction::KeepOriginals, scope)
        })
        && context.originals_allowed().await?;
    Ok(AttachmentsOutcome::Attachments {
        attachments,
        may_add: allowed,
        may_keep_originals,
    })
}

/// Adds an attachment from files already uploaded.
pub async fn add(
    context: &AssetsContext<'_>,
    store: &FileStore,
    input: AttachmentInput,
) -> AttachmentsOutcome {
    either(add_attachment(context, store, input).await)
}

/// The stored file's contents.
async fn read_stored(store: &FileStore, file: &StoredFile) -> Result<Vec<u8>, AttachmentsOutcome> {
    let store = store.clone();
    let hash = file.hash.clone();
    blocking(move || store.get(&hash))
        .await
        .map_err(|error| failed("file", &error))?
        .map_err(|error| failed("file", &error))
}

/// A photo's or PDF's details as the stored `file` says (its pixels or pages), whatever the app
/// sent in `claimed`; a kept original or metadata must have been uploaded, and an original
/// needs the person and the household to allow it.
async fn verified_content(
    context: &AssetsContext<'_>,
    store: &FileStore,
    scope: Option<AssetScope>,
    file: &StoredFile,
    claimed: Content,
) -> Result<Content, AttachmentsOutcome> {
    let bytes = read_stored(store, file).await?;
    Ok(match claimed {
        Content::Photo {
            size,
            metadata,
            original,
            ..
        } => {
            if FileType::sniff(&bytes) != Some(FileType::Jpeg) {
                return Err(invalid(AttachmentProblem::NotAPhoto));
            }
            if let Some(original) = &original {
                if !scope.is_some_and(|scope| {
                    context.permits_attachment(AssetAction::KeepOriginals, scope)
                }) {
                    return Err(AttachmentsOutcome::NotAllowed);
                }
                if !context.originals_allowed().await? {
                    return Err(invalid(AttachmentProblem::OriginalsOff));
                }
                context.uploaded(&original.file).await?;
            }
            if let Some(raw) = &metadata.raw {
                context.uploaded(raw).await?;
            }
            let (width, height) = blocking(move || pixels_of(&bytes))
                .await
                .map_err(|error| failed("photo", &error))?
                .map_err(invalid)?;
            Content::Photo {
                size,
                width,
                height,
                metadata,
                original,
            }
        }
        Content::Document { .. } => {
            if FileType::sniff(&bytes) != Some(FileType::Pdf) {
                return Err(invalid(AttachmentProblem::NotAPdf));
            }
            let pages = blocking(move || count_pages(&bytes))
                .await
                .map_err(|error| failed("pdf", &error))?
                .map_err(invalid)?;
            Content::Document { pages }
        }
    })
}

async fn add_attachment(
    context: &AssetsContext<'_>,
    store: &FileStore,
    input: AttachmentInput,
) -> Result<AttachmentsOutcome, AttachmentsOutcome> {
    // A reference document's attachment goes on no vehicle, whatever `input.asset` says.
    let (place, permission) = context.place_of(Some(input.asset), input.owner).await?;
    if !permission.allowed {
        return Err(AttachmentsOutcome::NotAllowed);
    }
    let scope = place.map(|(_, scope)| scope);
    context.uploaded(&input.file).await?;
    if let Some(thumbnail) = &input.thumbnail {
        context.uploaded(thumbnail).await?;
    }
    let found = verified_content(context, store, scope, &input.file, input.content).await?;
    let now = context.clock.now();
    let attachment = Attachment {
        asset: place.map(|(asset, _)| asset),
        owner: input.owner,
        file: input.file,
        thumbnail: input.thumbnail,
        name: input.name,
        content: found,
        added_at: now,
        added_by: context.user.user,
        received_at: now,
    };
    check_attachment(&attachment, permission.tasks).map_err(invalid)?;
    let id: AttachmentId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_attachment(Change::Create {
            id,
            values: AttachmentPatch::all(attachment),
        })
        .await?;
    tracing::debug!(target: log_area::APP, attachment = %id, owner = ?input.owner, "attachment added");
    Ok(AttachmentsOutcome::Saved { id })
}

/// Moves an attachment to another task of its event (or to the event itself), e.g. when a task
/// is removed. Nothing else about it changes.
pub async fn move_to(
    context: &AssetsContext<'_>,
    id: AttachmentId,
    task: Option<u16>,
) -> AttachmentsOutcome {
    either(move_attachment(context, id, task).await)
}

async fn move_attachment(
    context: &AssetsContext<'_>,
    id: AttachmentId,
    task: Option<u16>,
) -> Result<AttachmentsOutcome, AttachmentsOutcome> {
    let (attachment, _) = context.visible_attachment(id).await?;
    let AttachmentOwner::Event { event, .. } = attachment.owner else {
        return Err(invalid(AttachmentProblem::Unchangeable));
    };
    let (_, permission) = context.place_of(attachment.asset, attachment.owner).await?;
    if !permission.allowed {
        return Err(AttachmentsOutcome::NotAllowed);
    }
    let owner = AttachmentOwner::Event { event, task };
    let moved = Attachment {
        owner,
        ..attachment
    };
    check_attachment(&moved, permission.tasks).map_err(invalid)?;
    context
        .save_attachment(Change::Update {
            id,
            patch: AttachmentPatch {
                owner: Some(owner),
                ..AttachmentPatch::default()
            },
        })
        .await?;
    Ok(AttachmentsOutcome::Saved { id })
}

/// Removes an attachment (a soft delete; its files are erased once nothing needs them, #065).
pub async fn remove(context: &AssetsContext<'_>, id: AttachmentId) -> AttachmentsOutcome {
    either(remove_attachment(context, id).await)
}

async fn remove_attachment(
    context: &AssetsContext<'_>,
    id: AttachmentId,
) -> Result<AttachmentsOutcome, AttachmentsOutcome> {
    context.changeable_attachment(id).await?;
    context.save_attachment(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, attachment = %id, "attachment removed");
    Ok(AttachmentsOutcome::Done)
}

/// A file to send: its contents, MIME type, and name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Download {
    /// The contents.
    pub bytes: Vec<u8>,
    /// Its MIME type.
    pub mime: &'static str,
    /// The name it was uploaded with (a thumbnail's too).
    pub name: FileName,
}

/// One part of an attachment, if the person may see it.
///
/// # Errors
///
/// The refusal: not found (including a part it doesn't have), not allowed (an original, for
/// someone who may not keep them), or unavailable.
pub async fn download(
    context: &AssetsContext<'_>,
    store: &FileStore,
    id: AttachmentId,
    part: AttachmentPart,
) -> Result<Download, AttachmentsOutcome> {
    let (attachment, place) = context.visible_attachment(id).await?;
    let (file, mime) = match part {
        AttachmentPart::File => (
            attachment.file.clone(),
            match attachment.content {
                Content::Photo { .. } => FileType::Jpeg.mime(),
                Content::Document { .. } => FileType::Pdf.mime(),
            },
        ),
        AttachmentPart::Thumbnail => (
            attachment
                .thumbnail
                .clone()
                .ok_or(AttachmentsOutcome::NotFound)?,
            FileType::Jpeg.mime(),
        ),
        AttachmentPart::Original => {
            if !place.is_some_and(|(_, scope)| {
                context.permits_attachment(AssetAction::KeepOriginals, scope)
            }) {
                return Err(AttachmentsOutcome::NotAllowed);
            }
            let Content::Photo {
                original: Some(original),
                ..
            } = &attachment.content
            else {
                return Err(AttachmentsOutcome::NotFound);
            };
            (original.file.clone(), original.file_type.mime())
        }
    };
    let bytes = read_stored(store, &file).await?;
    Ok(Download {
        bytes,
        mime,
        name: attachment.name,
    })
}

// --- Events ------------------------------------------------------------------------------

/// Gives a new correction (`correction`) the attachments of the event it corrects
/// (`original`), as the person making it (choice 6).
///
/// # Errors
///
/// The refusal if one can't be saved.
pub(crate) async fn copy_to_correction(
    context: &AssetsContext<'_>,
    asset: AssetId,
    original: EventId,
    correction: EventId,
) -> Result<(), AttachmentsOutcome> {
    let originals: Vec<Attachment> = context
        .database
        .attachments_on(
            asset,
            AttachmentOwner::Event {
                event: original,
                task: None,
            },
        )
        .await
        .map_err(|error| failed("attachments", &error))?
        .into_iter()
        .map(|(_, attachment)| attachment)
        .collect();
    let now = context.clock.now();
    for copy in for_correction(&originals, correction) {
        let id: AttachmentId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
        context
            .save_attachment(Change::Create {
                id,
                values: AttachmentPatch::all(Attachment {
                    received_at: now,
                    ..copy
                }),
            })
            .await?;
    }
    Ok(())
}

/// Moves attachments on tasks an edited draft no longer has (`tasks` is how many it has now)
/// to the event itself, so none points at a missing task.
///
/// # Errors
///
/// The refusal if one can't be saved.
pub(crate) async fn settle_tasks(
    context: &AssetsContext<'_>,
    asset: AssetId,
    event: EventId,
    tasks: usize,
) -> Result<(), AttachmentsOutcome> {
    let on_event = context
        .database
        .attachments_on(asset, AttachmentOwner::Event { event, task: None })
        .await
        .map_err(|error| failed("attachments", &error))?;
    for (id, attachment) in on_event {
        if let AttachmentOwner::Event {
            task: Some(task), ..
        } = attachment.owner
            && usize::from(task) >= tasks
        {
            context
                .save_attachment(Change::Update {
                    id,
                    patch: AttachmentPatch {
                        owner: Some(AttachmentOwner::Event { event, task: None }),
                        ..AttachmentPatch::default()
                    },
                })
                .await?;
        }
    }
    Ok(())
}
