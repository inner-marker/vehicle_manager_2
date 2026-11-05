//! Attachments: photos and PDFs on a vehicle, an event (or one of its tasks), a squawk, or an
//! official document
//! (milestone 8; plan §3 Attachments, Photos from the web UI; decisions #036, #037, #065).
//!
//! **Files are stored by their SHA-256** ([`FileHash`]) and never change once written, so the
//! same receipt attached twice is stored once (#065). An [`Attachment`] is the record about a
//! file: what it's on, its name, and what it is.
//!
//! - **Photos** are JPEGs at a chosen [`PhotoSize`] (Small, Medium, or Large), upright, with a
//!   400 px thumbnail; the app resizes before upload and the server converts anything that
//!   isn't a correctly sized JPEG (#036). HEIC is refused as a photo (the app sends a JPEG);
//!   it can only be kept as an **original**, stored as it is.
//! - **Documents** are PDFs of up to 50 MB, with their page count (#037).
//!
//! **On an event, attachments are part of the draft:** once it's finished or finalized, adding
//! or removing one is a correction, which starts with the original's attachments
//! ([`for_correction`]).
//!
//! # Examples
//!
//! ```
//! use domain::attachment::{FileType, PhotoSize};
//!
//! assert_eq!(PhotoSize::default(), PhotoSize::Medium);
//! assert_eq!(PhotoSize::Small.fit(4032, 3024), (1280, 960));
//! assert_eq!(FileType::sniff(b"%PDF-1.7\n"), Some(FileType::Pdf));
//! ```

use serde::{Deserialize, Serialize};

use crate::asset::AssetId;
use crate::command::{Record, record_patch};
use crate::document::AssetDocumentId;
use crate::event::EventId;
use crate::i18n::{MessageId, messages};
use crate::id::Id;
use crate::location::Coordinates;
use crate::meter::EventKind;
use crate::squawk::SquawkId;
use crate::time::UnixMillis;
use crate::user::UserId;

/// The ID of an attachment.
pub type AttachmentId = Id<Attachment>;

/// The most bytes a photo upload (or a kept original) may have: 25 MB (#036).
pub const PHOTO_MAX_BYTES: u64 = 25 * 1024 * 1024;

/// The most bytes a PDF may have: 50 MB (#037).
pub const PDF_MAX_BYTES: u64 = 50 * 1024 * 1024;

/// A thumbnail's long edge, in pixels (#036).
pub const THUMBNAIL_LONG_EDGE: u32 = 400;

/// The JPEG quality photos are saved at (#036).
pub const JPEG_QUALITY: u8 = 85;

/// The most characters a file's name may have.
pub const NAME_MAX_CHARS: usize = 255;

/// The most characters a camera's name may have.
pub const CAMERA_MAX_CHARS: usize = 100;

// --- Files ---------------------------------------------------------------------------------

/// A stored file's SHA-256, as 64 lowercase hex digits: the file's name in the store (#065).
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileHash(String);

impl FileHash {
    /// A hash from its hex digits (upper case is accepted and lowered).
    ///
    /// # Errors
    ///
    /// [`AttachmentProblem::BadHash`] unless it's 64 hex digits.
    pub fn parse(input: &str) -> Result<Self, AttachmentProblem> {
        if input.len() == 64 && input.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            Ok(Self(input.to_ascii_lowercase()))
        } else {
            Err(AttachmentProblem::BadHash)
        }
    }

    /// The hash from its 32 bytes.
    #[must_use]
    pub fn from_bytes(bytes: [u8; 32]) -> Self {
        const HEX: &[u8; 16] = b"0123456789abcdef";
        let mut text = String::with_capacity(64);
        for byte in bytes {
            text.push(char::from(HEX[usize::from(byte >> 4)]));
            text.push(char::from(HEX[usize::from(byte & 0x0F)]));
        }
        Self(text)
    }

    /// The 64 hex digits.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FileHash {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|problem| format!("{problem:?}"))
    }
}

impl From<FileHash> for String {
    fn from(hash: FileHash) -> Self {
        hash.0
    }
}

/// A file in the store and its size.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct StoredFile {
    /// Its SHA-256.
    pub hash: FileHash,
    /// Its size in bytes.
    pub bytes: u64,
}

/// What a file is, from its first bytes (never from its name or what the sender says).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FileType {
    /// A JPEG.
    Jpeg,
    /// A PNG.
    Png,
    /// A HEIC or HEIF image (as iPhones take them).
    Heic,
    /// A PDF.
    Pdf,
}

impl FileType {
    /// What `bytes` start like, if it's a type the app knows.
    #[must_use]
    pub fn sniff(bytes: &[u8]) -> Option<Self> {
        if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
            Some(Self::Jpeg)
        } else if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
            Some(Self::Png)
        } else if bytes.starts_with(b"%PDF-") {
            Some(Self::Pdf)
        } else if is_heic(bytes) {
            Some(Self::Heic)
        } else {
            None
        }
    }

    /// Its MIME type, for downloads.
    #[must_use]
    pub const fn mime(self) -> &'static str {
        match self {
            Self::Jpeg => "image/jpeg",
            Self::Png => "image/png",
            Self::Heic => "image/heic",
            Self::Pdf => "application/pdf",
        }
    }

    /// Whether it's an image.
    #[must_use]
    pub const fn is_image(self) -> bool {
        matches!(self, Self::Jpeg | Self::Png | Self::Heic)
    }
}

/// Whether `bytes` start with an ISO media `ftyp` box naming a HEIC or HEIF brand.
fn is_heic(bytes: &[u8]) -> bool {
    const BRANDS: [&[u8; 4]; 6] = [b"heic", b"heix", b"hevc", b"heim", b"mif1", b"msf1"];
    bytes.get(4..8) == Some(b"ftyp".as_slice())
        && bytes
            .get(8..12)
            .is_some_and(|brand| BRANDS.iter().any(|known| brand == known.as_slice()))
}

/// A file's name as it was uploaded, e.g. "Invoice 2291.pdf": trimmed, 1–255 characters, with
/// no folders and no control characters.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct FileName(String);

impl FileName {
    /// A name from an upload: anything before the last `/` or `\` is dropped (some browsers
    /// send a path).
    ///
    /// # Errors
    ///
    /// [`AttachmentProblem::NameEmpty`] or [`AttachmentProblem::NameTooLong`].
    pub fn from_upload(input: &str) -> Result<Self, AttachmentProblem> {
        let last = input.rsplit(['/', '\\']).next().unwrap_or_default();
        let name: String = last.chars().filter(|c| !c.is_control()).collect();
        let name = name.trim();
        match name.chars().count() {
            0 => Err(AttachmentProblem::NameEmpty),
            length if length > NAME_MAX_CHARS => Err(AttachmentProblem::NameTooLong),
            _ => Ok(Self(name.to_owned())),
        }
    }

    /// The name.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for FileName {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        let name = Self::from_upload(&value).map_err(|problem| format!("{problem:?}"))?;
        if name.0 == value {
            Ok(name)
        } else {
            Err(format!("not a stored file name: {value:?}"))
        }
    }
}

impl From<FileName> for String {
    fn from(name: FileName) -> Self {
        name.0
    }
}

// --- Photos --------------------------------------------------------------------------------

/// How big a photo is kept (#036), chosen per photo.
#[derive(
    Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize,
)]
#[serde(rename_all = "kebab-case")]
pub enum PhotoSize {
    /// 1280 px: meter close-ups, receipts, labels (about 200 KB).
    Small,
    /// 2560 px: general photos (about 0.5–1 MB).
    #[default]
    Medium,
    /// 4096 px: damage, condition at the start of a job (about 2–3 MB).
    Large,
}

impl PhotoSize {
    /// Every size, smallest first.
    pub const ALL: [Self; 3] = [Self::Small, Self::Medium, Self::Large];

    /// The long edge it's kept at, in pixels.
    #[must_use]
    pub const fn long_edge(self) -> u32 {
        match self {
            Self::Small => 1280,
            Self::Medium => 2560,
            Self::Large => 4096,
        }
    }

    /// The size's name.
    #[must_use]
    pub const fn label(self) -> MessageId {
        match self {
            Self::Small => messages::PHOTO_SIZE_SMALL,
            Self::Medium => messages::PHOTO_SIZE_MEDIUM,
            Self::Large => messages::PHOTO_SIZE_LARGE,
        }
    }

    /// The size a `width` × `height` photo is kept at: its long edge brought down to this
    /// size's, keeping its shape. Never enlarged.
    #[must_use]
    pub fn fit(self, width: u32, height: u32) -> (u32, u32) {
        fit_within(width, height, self.long_edge())
    }

    /// The default for a photo on an event of `kind` (milestone 8, choice 8): Small on
    /// fuel-ups and readings (a pump or a gauge), Medium on everything else (work, paperwork). A profile can
    /// change it ([`crate::profile::Profile::photo_size`]).
    #[must_use]
    pub const fn default_for(kind: EventKind) -> Self {
        match kind {
            EventKind::Fuel | EventKind::Reading => Self::Small,
            EventKind::Maintenance
            | EventKind::Repair
            | EventKind::Acquired
            | EventKind::SentOut
            | EventKind::Returned
            | EventKind::Disposed => Self::Medium,
        }
    }
}

/// A thumbnail's size for a `width` × `height` image.
#[must_use]
pub fn thumbnail_size(width: u32, height: u32) -> (u32, u32) {
    fit_within(width, height, THUMBNAIL_LONG_EDGE)
}

/// `width` × `height` with the long edge at most `edge`, keeping the shape (rounded, and never
/// below 1 px).
fn fit_within(width: u32, height: u32, edge: u32) -> (u32, u32) {
    let long = width.max(height);
    if long <= edge {
        return (width, height);
    }
    let scale = |side: u32| {
        let scaled = (u64::from(side) * u64::from(edge) + u64::from(long) / 2) / u64::from(long);
        u32::try_from(scaled).unwrap_or(edge).max(1)
    };
    (scale(width), scale(height))
}

/// What a photo's metadata said, read before resizing (which drops it) and kept as fields
/// (#036). Exports leave out the location by default.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct PhotoMetadata {
    /// When it was taken.
    pub taken_at: Option<UnixMillis>,
    /// Where.
    pub coordinates: Option<Coordinates>,
    /// The camera, e.g. "Apple iPhone 15 Pro" (up to 100 characters).
    pub camera: Option<String>,
    /// All of it as read (EXIF), kept as a file for completeness.
    pub raw: Option<StoredFile>,
}

/// The file as taken, kept beside the photo (#036): only Maintainers and Admins may keep one.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Original {
    /// The file.
    pub file: StoredFile,
    /// What it is (a JPEG, PNG, or HEIC).
    pub file_type: FileType,
}

// --- Attachments ---------------------------------------------------------------------------

/// What an attachment is on. Every attachment is about one vehicle ([`Attachment::asset`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "on", rename_all = "kebab-case")]
pub enum AttachmentOwner {
    /// The vehicle itself.
    Vehicle,
    /// An event, or one of its tasks (by its place in the event's task list, like a
    /// squawk's sign-off).
    Event {
        /// Which event.
        event: EventId,
        /// The task, if it's on one.
        task: Option<u16>,
    },
    /// A squawk.
    Squawk {
        /// Which squawk.
        squawk: SquawkId,
    },
    /// An official document (its scans).
    Document {
        /// Which document.
        document: AssetDocumentId,
    },
}

impl AttachmentOwner {
    /// The event it's on, if any.
    #[must_use]
    pub const fn event(self) -> Option<EventId> {
        match self {
            Self::Event { event, .. } => Some(event),
            Self::Vehicle | Self::Squawk { .. } | Self::Document { .. } => None,
        }
    }

    /// Where it is after task `removed` leaves its event's list: a later task's place moves up
    /// one, and the removed task's attachments stay on the event.
    #[must_use]
    pub const fn after_task_removed(self, removed: u16) -> Self {
        match self {
            Self::Event {
                event,
                task: Some(task),
            } if task >= removed => Self::Event {
                event,
                task: if task == removed {
                    None
                } else {
                    Some(task - 1)
                },
            },
            Self::Vehicle | Self::Event { .. } | Self::Squawk { .. } | Self::Document { .. } => {
                self
            }
        }
    }
}

/// What kind of attachment: a photo or a document.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum AttachmentKind {
    /// A photo.
    Photo,
    /// A PDF.
    Document,
}

/// What's particular to a photo or a document.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum Content {
    /// A photo (a JPEG).
    Photo {
        /// The size it was kept at.
        size: PhotoSize,
        /// Its width, in pixels.
        width: u32,
        /// Its height, in pixels.
        height: u32,
        /// What its metadata said.
        metadata: PhotoMetadata,
        /// The file as taken, if kept.
        original: Option<Original>,
    },
    /// A PDF.
    Document {
        /// How many pages it has.
        pages: u32,
    },
}

impl Content {
    /// Photo or document.
    #[must_use]
    pub const fn kind(&self) -> AttachmentKind {
        match self {
            Self::Photo { .. } => AttachmentKind::Photo,
            Self::Document { .. } => AttachmentKind::Document,
        }
    }
}

/// One attachment.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Attachment {
    /// Which vehicle it's about.
    pub asset: AssetId,
    /// What it's on.
    pub owner: AttachmentOwner,
    /// The file shown: the JPEG, or the PDF.
    pub file: StoredFile,
    /// Its thumbnail (a JPEG; for a PDF, its first page, which the app draws), if any.
    pub thumbnail: Option<StoredFile>,
    /// The name it was uploaded with.
    pub name: FileName,
    /// A photo's or a document's details.
    pub content: Content,
    /// When it was added.
    pub added_at: UnixMillis,
    /// Who added it.
    pub added_by: UserId,
    /// When the server first received it.
    pub received_at: UnixMillis,
}

impl Attachment {
    /// Every file it uses: the file, its thumbnail, and a photo's original and raw metadata.
    /// A stored file no attachment uses may be erased (#065).
    #[must_use]
    pub fn files(&self) -> Vec<&StoredFile> {
        let mut files = vec![&self.file];
        files.extend(&self.thumbnail);
        if let Content::Photo {
            metadata, original, ..
        } = &self.content
        {
            files.extend(original.as_ref().map(|original| &original.file));
            files.extend(&metadata.raw);
        }
        files
    }
}

record_patch! {
    /// The changeable fields of an [`Attachment`]; `Some` means "set to this". Only its owner
    /// changes after it's added (a task's place moving); the rest is set once.
    AttachmentPatch
    {
        /// See [`Attachment::asset`] (set when added).
        asset: AssetId,
        /// See [`Attachment::owner`].
        owner: AttachmentOwner,
        /// See [`Attachment::file`] (set when added).
        file: StoredFile,
        /// See [`Attachment::thumbnail`] (set when added).
        thumbnail: Option<StoredFile>,
        /// See [`Attachment::name`] (set when added).
        name: FileName,
        /// See [`Attachment::content`] (set when added).
        content: Content,
        /// See [`Attachment::added_at`] (set when added).
        added_at: UnixMillis,
        /// See [`Attachment::added_by`] (set when added).
        added_by: UserId,
        /// See [`Attachment::received_at`] (set when added).
        received_at: UnixMillis,
    }
}

impl AttachmentPatch {
    /// Every field of `attachment`, for adding it.
    #[must_use]
    pub fn all(attachment: Attachment) -> Self {
        Self {
            asset: Some(attachment.asset),
            owner: Some(attachment.owner),
            file: Some(attachment.file),
            thumbnail: Some(attachment.thumbnail),
            name: Some(attachment.name),
            content: Some(attachment.content),
            added_at: Some(attachment.added_at),
            added_by: Some(attachment.added_by),
            received_at: Some(attachment.received_at),
        }
    }

    /// Whether it changes only the owner (the one field that may change after adding).
    #[must_use]
    pub const fn only_moves(&self) -> bool {
        self.asset.is_none()
            && self.file.is_none()
            && self.thumbnail.is_none()
            && self.name.is_none()
            && self.content.is_none()
            && self.added_at.is_none()
            && self.added_by.is_none()
            && self.received_at.is_none()
    }
}

impl Record for Attachment {
    const ENTITY: &'static str = "attachment";
    type Patch = AttachmentPatch;
}

/// The attachments a correction of an event starts with: each of `original`'s, now on
/// `correction` (the same files, so nothing is stored twice). The caller gives each its new ID
/// and who added it and when.
#[must_use]
pub fn for_correction(original: &[Attachment], correction: EventId) -> Vec<Attachment> {
    original
        .iter()
        .filter_map(|attachment| match attachment.owner {
            AttachmentOwner::Event { task, .. } => Some(Attachment {
                owner: AttachmentOwner::Event {
                    event: correction,
                    task,
                },
                ..attachment.clone()
            }),
            AttachmentOwner::Vehicle
            | AttachmentOwner::Squawk { .. }
            | AttachmentOwner::Document { .. } => None,
        })
        .collect()
}

// --- Checks --------------------------------------------------------------------------------

/// Checks an upload for an attachment of `kind`: what its bytes say it is, and its size.
/// Photos may be JPEGs or PNGs (the server converts a PNG); documents must be PDFs.
///
/// # Errors
///
/// [`AttachmentProblem::Empty`], [`AttachmentProblem::HeicPhoto`],
/// [`AttachmentProblem::NotAPhoto`], [`AttachmentProblem::NotAPdf`],
/// [`AttachmentProblem::PhotoTooLarge`], or [`AttachmentProblem::PdfTooLarge`].
pub fn check_upload(
    kind: AttachmentKind,
    file_type: Option<FileType>,
    bytes: u64,
) -> Result<(), AttachmentProblem> {
    if bytes == 0 {
        return Err(AttachmentProblem::Empty);
    }
    match (kind, file_type) {
        (AttachmentKind::Photo, Some(FileType::Jpeg | FileType::Png)) => {
            if bytes > PHOTO_MAX_BYTES {
                Err(AttachmentProblem::PhotoTooLarge)
            } else {
                Ok(())
            }
        }
        (AttachmentKind::Photo, Some(FileType::Heic)) => Err(AttachmentProblem::HeicPhoto),
        (AttachmentKind::Photo, Some(FileType::Pdf) | None) => Err(AttachmentProblem::NotAPhoto),
        (AttachmentKind::Document, Some(FileType::Pdf)) => {
            if bytes > PDF_MAX_BYTES {
                Err(AttachmentProblem::PdfTooLarge)
            } else {
                Ok(())
            }
        }
        (
            AttachmentKind::Document,
            Some(FileType::Jpeg | FileType::Png | FileType::Heic) | None,
        ) => Err(AttachmentProblem::NotAPdf),
    }
}

/// Checks a kept original: an image (as taken, HEIC included) of up to 25 MB.
///
/// # Errors
///
/// [`AttachmentProblem::Empty`], [`AttachmentProblem::NotAPhoto`], or
/// [`AttachmentProblem::PhotoTooLarge`].
pub fn check_original(file_type: Option<FileType>, bytes: u64) -> Result<(), AttachmentProblem> {
    if bytes == 0 {
        Err(AttachmentProblem::Empty)
    } else if !file_type.is_some_and(FileType::is_image) {
        Err(AttachmentProblem::NotAPhoto)
    } else if bytes > PHOTO_MAX_BYTES {
        Err(AttachmentProblem::PhotoTooLarge)
    } else {
        Ok(())
    }
}

/// Checks an attachment's details: a photo has a size and a camera name within limits, and a
/// document has at least one page; on an event's task, the task exists (`tasks` is how many
/// the event has).
///
/// # Errors
///
/// [`AttachmentProblem::NoSize`], [`AttachmentProblem::CameraTooLong`],
/// [`AttachmentProblem::NoPages`], or [`AttachmentProblem::NoSuchTask`].
pub fn check(attachment: &Attachment, tasks: usize) -> Result<(), AttachmentProblem> {
    match &attachment.content {
        Content::Photo {
            width,
            height,
            metadata,
            ..
        } => {
            if *width == 0 || *height == 0 {
                return Err(AttachmentProblem::NoSize);
            }
            if metadata
                .camera
                .as_ref()
                .is_some_and(|camera| camera.chars().count() > CAMERA_MAX_CHARS)
            {
                return Err(AttachmentProblem::CameraTooLong);
            }
        }
        Content::Document { pages } => {
            if *pages == 0 {
                return Err(AttachmentProblem::NoPages);
            }
        }
    }
    match attachment.owner {
        AttachmentOwner::Event {
            task: Some(task), ..
        } if usize::from(task) >= tasks => Err(AttachmentProblem::NoSuchTask),
        AttachmentOwner::Vehicle
        | AttachmentOwner::Event { .. }
        | AttachmentOwner::Squawk { .. }
        | AttachmentOwner::Document { .. } => Ok(()),
    }
}

/// Why an attachment (or an upload) was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "problem", rename_all = "kebab-case")]
pub enum AttachmentProblem {
    /// A file with nothing in it.
    Empty,
    /// A photo upload that isn't a JPEG or PNG.
    NotAPhoto,
    /// A HEIC photo the app should have sent as a JPEG.
    HeicPhoto,
    /// A document upload that isn't a PDF.
    NotAPdf,
    /// A photo (or original) over 25 MB.
    PhotoTooLarge,
    /// A PDF over 50 MB.
    PdfTooLarge,
    /// A file name with nothing in it.
    NameEmpty,
    /// A file name over 255 characters.
    NameTooLong,
    /// A photo with no width or height.
    NoSize,
    /// A camera name over 100 characters.
    CameraTooLong,
    /// A PDF with no pages.
    NoPages,
    /// An attachment on a task the event doesn't have.
    NoSuchTask,
    /// A file hash that isn't 64 hex digits.
    BadHash,
    /// A change to an attachment other than moving it (the rest is set when it's added).
    Unchangeable,
    /// A photo or PDF the server couldn't read (damaged, or a PDF it can't open).
    Unreadable,
    /// A file that wasn't uploaded first (or not with that size).
    NotUploaded,
    /// Keeping an original while the household has them off (#036).
    OriginalsOff,
    /// An attachment on a finished event (correct the event instead, choice 6).
    EventFinished,
}

impl AttachmentProblem {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Empty => messages::ATTACHMENT_EMPTY,
            Self::NotAPhoto => messages::ATTACHMENT_NOT_A_PHOTO,
            Self::HeicPhoto => messages::ATTACHMENT_HEIC_PHOTO,
            Self::NotAPdf => messages::ATTACHMENT_NOT_A_PDF,
            Self::PhotoTooLarge => messages::ATTACHMENT_PHOTO_TOO_LARGE,
            Self::PdfTooLarge => messages::ATTACHMENT_PDF_TOO_LARGE,
            Self::NameEmpty => messages::ATTACHMENT_NAME_EMPTY,
            Self::NameTooLong => messages::ATTACHMENT_NAME_TOO_LONG,
            Self::NoSize => messages::ATTACHMENT_NO_SIZE,
            Self::CameraTooLong => messages::ATTACHMENT_CAMERA_TOO_LONG,
            Self::NoPages => messages::ATTACHMENT_NO_PAGES,
            Self::NoSuchTask => messages::ATTACHMENT_NO_SUCH_TASK,
            Self::BadHash => messages::ATTACHMENT_BAD_HASH,
            Self::Unchangeable => messages::ATTACHMENT_UNCHANGEABLE,
            Self::Unreadable => messages::ATTACHMENT_UNREADABLE,
            Self::NotUploaded => messages::ATTACHMENT_NOT_UPLOADED,
            Self::OriginalsOff => messages::ATTACHMENT_ORIGINALS_OFF,
            Self::EventFinished => messages::ATTACHMENT_EVENT_FINISHED,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::command::Patch;
    use crate::test_support::test_id;

    fn stored(byte: u8, bytes: u64) -> StoredFile {
        StoredFile {
            hash: FileHash::from_bytes([byte; 32]),
            bytes,
        }
    }

    fn photo(owner: AttachmentOwner) -> Attachment {
        Attachment {
            asset: test_id(1),
            owner,
            file: stored(1, 600_000),
            thumbnail: Some(stored(2, 30_000)),
            name: FileName::from_upload("IMG_0412.HEIC").unwrap(),
            content: Content::Photo {
                size: PhotoSize::Medium,
                width: 2560,
                height: 1920,
                metadata: PhotoMetadata::default(),
                original: None,
            },
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(5),
            received_at: UnixMillis::from_millis(0),
        }
    }

    fn on_task(task: Option<u16>) -> AttachmentOwner {
        AttachmentOwner::Event {
            event: test_id(7),
            task,
        }
    }

    #[test]
    fn file_types_come_from_the_first_bytes() {
        assert_eq!(
            FileType::sniff(&[0xFF, 0xD8, 0xFF, 0xE1]),
            Some(FileType::Jpeg)
        );
        assert_eq!(
            FileType::sniff(b"\x89PNG\r\n\x1a\n...."),
            Some(FileType::Png)
        );
        assert_eq!(FileType::sniff(b"%PDF-1.4"), Some(FileType::Pdf));
        assert_eq!(
            FileType::sniff(b"\0\0\0\x18ftypheic\0\0\0\0"),
            Some(FileType::Heic)
        );
        assert_eq!(
            FileType::sniff(b"\0\0\0\x18ftypmif1\0\0\0\0"),
            Some(FileType::Heic)
        );
        // An MP4 has an ftyp box too, but not a HEIC brand.
        assert_eq!(FileType::sniff(b"\0\0\0\x18ftypisom\0\0\0\0"), None);
        assert_eq!(FileType::sniff(b"GIF89a"), None);
        assert_eq!(FileType::sniff(b""), None);
        assert!(FileType::Heic.is_image());
        assert!(!FileType::Pdf.is_image());
    }

    #[test]
    fn photo_sizes_shrink_the_long_edge_and_never_enlarge() {
        assert_eq!(PhotoSize::default(), PhotoSize::Medium);
        assert_eq!(PhotoSize::Small.fit(4032, 3024), (1280, 960));
        assert_eq!(PhotoSize::Medium.fit(3024, 4032), (1920, 2560));
        assert_eq!(PhotoSize::Large.fit(4032, 3024), (4032, 3024));
        assert_eq!(PhotoSize::Small.fit(800, 600), (800, 600));
        // A very thin panorama keeps at least a pixel.
        assert_eq!(PhotoSize::Small.fit(20_000, 2), (1280, 1));
        assert_eq!(thumbnail_size(2560, 1920), (400, 300));
    }

    #[test]
    fn small_is_the_default_on_fuel_ups_and_readings() {
        assert_eq!(PhotoSize::default_for(EventKind::Fuel), PhotoSize::Small);
        assert_eq!(PhotoSize::default_for(EventKind::Reading), PhotoSize::Small);
        assert_eq!(
            PhotoSize::default_for(EventKind::Maintenance),
            PhotoSize::Medium
        );
        assert_eq!(PhotoSize::default_for(EventKind::Repair), PhotoSize::Medium);
    }

    #[test]
    fn hashes_are_64_hex_digits() {
        let hash = FileHash::from_bytes([0xAB; 32]);
        assert_eq!(hash.as_str().len(), 64);
        assert!(hash.as_str().starts_with("abab"));
        assert_eq!(FileHash::parse(&"AB".repeat(32)), Ok(hash));
        assert_eq!(FileHash::parse("abc"), Err(AttachmentProblem::BadHash));
        assert_eq!(
            FileHash::parse(&"zz".repeat(32)),
            Err(AttachmentProblem::BadHash)
        );
    }

    #[test]
    fn file_names_drop_folders_and_control_characters() {
        assert_eq!(
            FileName::from_upload("C:\\Users\\sam\\Invoice 2291.pdf")
                .unwrap()
                .as_str(),
            "Invoice 2291.pdf"
        );
        assert_eq!(
            FileName::from_upload(" photos/IMG\u{0}_1.jpg ")
                .unwrap()
                .as_str(),
            "IMG_1.jpg"
        );
        assert_eq!(
            FileName::from_upload("folder/"),
            Err(AttachmentProblem::NameEmpty)
        );
        assert_eq!(
            FileName::from_upload(&"a".repeat(256)),
            Err(AttachmentProblem::NameTooLong)
        );
        // A stored name must already be clean.
        assert!(serde_json::from_str::<FileName>("\"a/b.pdf\"").is_err());
    }

    #[test]
    fn uploads_are_checked_by_what_they_are_and_their_size() {
        use AttachmentKind::{Document, Photo};
        assert_eq!(check_upload(Photo, Some(FileType::Jpeg), 1), Ok(()));
        assert_eq!(
            check_upload(Photo, Some(FileType::Png), PHOTO_MAX_BYTES),
            Ok(())
        );
        assert_eq!(
            check_upload(Photo, Some(FileType::Jpeg), PHOTO_MAX_BYTES + 1),
            Err(AttachmentProblem::PhotoTooLarge)
        );
        assert_eq!(
            check_upload(Photo, Some(FileType::Heic), 1),
            Err(AttachmentProblem::HeicPhoto)
        );
        assert_eq!(
            check_upload(Photo, Some(FileType::Pdf), 1),
            Err(AttachmentProblem::NotAPhoto)
        );
        assert_eq!(
            check_upload(Document, Some(FileType::Pdf), PDF_MAX_BYTES),
            Ok(())
        );
        assert_eq!(
            check_upload(Document, Some(FileType::Pdf), PDF_MAX_BYTES + 1),
            Err(AttachmentProblem::PdfTooLarge)
        );
        assert_eq!(
            check_upload(Document, Some(FileType::Jpeg), 1),
            Err(AttachmentProblem::NotAPdf)
        );
        assert_eq!(
            check_upload(Document, None, 1),
            Err(AttachmentProblem::NotAPdf)
        );
        assert_eq!(
            check_upload(Photo, Some(FileType::Jpeg), 0),
            Err(AttachmentProblem::Empty)
        );
    }

    #[test]
    fn an_original_may_be_heic_but_must_be_an_image() {
        assert_eq!(check_original(Some(FileType::Heic), 3_000_000), Ok(()));
        assert_eq!(
            check_original(Some(FileType::Pdf), 1),
            Err(AttachmentProblem::NotAPhoto)
        );
        assert_eq!(
            check_original(Some(FileType::Heic), PHOTO_MAX_BYTES + 1),
            Err(AttachmentProblem::PhotoTooLarge)
        );
    }

    #[test]
    fn details_are_checked_and_a_task_must_exist() {
        assert_eq!(check(&photo(on_task(Some(1))), 2), Ok(()));
        assert_eq!(
            check(&photo(on_task(Some(2))), 2),
            Err(AttachmentProblem::NoSuchTask)
        );
        assert_eq!(check(&photo(on_task(None)), 0), Ok(()));
        assert_eq!(check(&photo(AttachmentOwner::Vehicle), 0), Ok(()));

        let mut flat = photo(AttachmentOwner::Vehicle);
        if let Content::Photo { width, .. } = &mut flat.content {
            *width = 0;
        }
        assert_eq!(check(&flat, 0), Err(AttachmentProblem::NoSize));

        let mut chatty = photo(AttachmentOwner::Vehicle);
        if let Content::Photo { metadata, .. } = &mut chatty.content {
            metadata.camera = Some("x".repeat(101));
        }
        assert_eq!(check(&chatty, 0), Err(AttachmentProblem::CameraTooLong));

        let empty_pdf = Attachment {
            content: Content::Document { pages: 0 },
            ..photo(AttachmentOwner::Vehicle)
        };
        assert_eq!(check(&empty_pdf, 0), Err(AttachmentProblem::NoPages));
    }

    #[test]
    fn removing_a_task_moves_its_attachments_to_the_event() {
        let removed = 1;
        assert_eq!(
            on_task(Some(0)).after_task_removed(removed),
            on_task(Some(0))
        );
        assert_eq!(on_task(Some(1)).after_task_removed(removed), on_task(None));
        assert_eq!(
            on_task(Some(2)).after_task_removed(removed),
            on_task(Some(1))
        );
        assert_eq!(on_task(None).after_task_removed(removed), on_task(None));
        assert_eq!(
            AttachmentOwner::Vehicle.after_task_removed(removed),
            AttachmentOwner::Vehicle
        );
    }

    #[test]
    fn a_correction_starts_with_the_originals_attachments() {
        let correction: EventId = test_id(8);
        let original = [photo(on_task(Some(1))), photo(on_task(None))];
        let copied = for_correction(&original, correction);
        assert_eq!(copied.len(), 2);
        assert_eq!(
            copied[0].owner,
            AttachmentOwner::Event {
                event: correction,
                task: Some(1)
            }
        );
        assert_eq!(copied[0].file, original[0].file);
        assert_eq!(copied[1].owner.event(), Some(correction));
    }

    #[test]
    fn files_lists_everything_an_attachment_uses() {
        let mut kept = photo(AttachmentOwner::Vehicle);
        if let Content::Photo {
            metadata, original, ..
        } = &mut kept.content
        {
            metadata.raw = Some(stored(3, 4_000));
            *original = Some(Original {
                file: stored(4, 3_000_000),
                file_type: FileType::Heic,
            });
        }
        let hashes: Vec<u8> = kept
            .files()
            .iter()
            .map(|file| u8::from_str_radix(&file.hash.as_str()[..2], 16).unwrap())
            .collect();
        assert_eq!(hashes, [1, 2, 4, 3]);
    }

    #[test]
    fn an_attachment_round_trips_and_only_its_owner_changes() {
        let attachment = photo(on_task(Some(0)));
        let json = serde_json::to_string(&attachment).unwrap();
        assert_eq!(
            serde_json::from_str::<Attachment>(&json).unwrap(),
            attachment
        );

        let added = AttachmentPatch::all(attachment);
        assert_eq!(added.changed_fields().len(), 9);
        assert!(!added.only_moves());
        let moved = AttachmentPatch {
            owner: Some(on_task(None)),
            ..AttachmentPatch::default()
        };
        assert!(moved.only_moves());
    }
}
