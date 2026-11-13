//! This computer's photos and PDFs (phase-3.md 6c, decision #077).
//!
//! The files database (`storage::local_files`, `files.sqlite`) sits beside the copy, opens and
//! closes with it under the same key, and goes with it at sign-out.
//!
//! - **Showing a file** ([`crate::files::fetch`]) looks here first ([`local`]); a file
//!   downloaded from the server is kept ([`keep`]) if it's what the copy says it is.
//! - **Thumbnails and PDFs download by themselves** ([`fetch_missing`], after each pull); a
//!   photo only when it's opened. Originals are never kept (they're only on request).
//! - **The limit** ([`photo_limit`], a setting per computer): past it, the photos opened
//!   longest ago are dropped; thumbnails, PDFs, and uploads waiting never are.

use std::path::Path;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};

use domain::api::AttachmentPart;
use domain::attachment::{Attachment, AttachmentId, Content, FileHash, PhotoLimit};
use domain::log_area;
use domain::time::Clock;
use storage::LocalKey;
use storage::files::hash_of;
use storage::local_files::{LocalFiles, LocalKind};

use crate::logging::SystemClock;

/// The open files database, if any.
static FILES: Mutex<Option<LocalFiles>> = Mutex::new(None);

/// A download of missing files is running (one at a time).
static FETCHING: AtomicBool = AtomicBool::new(false);

fn files_slot() -> std::sync::MutexGuard<'static, Option<LocalFiles>> {
    FILES
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// The open files database, if there is one.
#[must_use]
pub fn current() -> Option<LocalFiles> {
    files_slot().clone()
}

/// Opens the files database in the copy's `folder` with its `key` (with the copy). A failure
/// is logged: files then come from the server, as before.
pub(crate) async fn open(folder: &Path, key: &LocalKey) {
    match LocalFiles::open(folder, key).await {
        Ok(files) => *files_slot() = Some(files),
        Err(error) => {
            tracing::error!(target: log_area::SYNC, %error, "couldn't open this computer's files");
        }
    }
}

/// Closes it (with the copy).
pub(crate) async fn close() {
    let open = files_slot().take();
    if let Some(files) = open {
        files.close().await;
    }
}

/// Which file of `attachment` `part` is, and how it's kept here; `None` for an original
/// (never kept) or a part it doesn't have.
fn kept_as(attachment: &Attachment, part: AttachmentPart) -> Option<(FileHash, LocalKind)> {
    match part {
        AttachmentPart::Thumbnail => attachment
            .thumbnail
            .as_ref()
            .map(|thumbnail| (thumbnail.hash.clone(), LocalKind::Thumbnail)),
        AttachmentPart::File => Some((
            attachment.file.hash.clone(),
            match attachment.content {
                Content::Document { .. } => LocalKind::Pdf,
                Content::Photo { .. } => LocalKind::Photo,
            },
        )),
        AttachmentPart::Original => None,
    }
}

/// The attachment, from the copy.
async fn attachment(id: AttachmentId) -> Option<Attachment> {
    let copy = crate::local_copy::current()?;
    copy.attachment(id).await.ok().flatten()
}

/// `part` of attachment `id`, if this computer has it (noted as opened now).
pub async fn local(id: AttachmentId, part: AttachmentPart) -> Option<Vec<u8>> {
    let files = current()?;
    let (hash, _) = kept_as(&attachment(id).await?, part)?;
    let bytes = files.get(&hash).await.ok()??;
    if let Err(error) = files.mark_opened(&hash, SystemClock.now()).await {
        tracing::debug!(target: log_area::SYNC, %error, "couldn't note a file as opened");
    }
    Some(bytes)
}

/// Keeps `bytes`, downloaded for `part` of attachment `id`, if they're the file the copy names
/// (then drops photos past the limit).
pub async fn keep(id: AttachmentId, part: AttachmentPart, bytes: &[u8]) {
    let Some(files) = current() else {
        return;
    };
    let Some(attachment) = attachment(id).await else {
        return;
    };
    let Some((hash, kind)) = kept_as(&attachment, part) else {
        return;
    };
    if hash_of(bytes) != hash {
        tracing::warn!(target: log_area::SYNC, attachment = %id, "a downloaded file isn't the one the copy names; not kept");
        return;
    }
    if let Err(error) = files.put(bytes, kind, SystemClock.now()).await {
        tracing::warn!(target: log_area::SYNC, %error, "couldn't keep a downloaded file");
        return;
    }
    if kind == LocalKind::Photo {
        drop_over_limit(&files).await;
    }
}

/// The limit on downloaded photos, from this computer's settings.
#[must_use]
pub fn photo_limit() -> PhotoLimit {
    crate::data_folder::platform_data_folder()
        .map(|folder| crate::settings::load(&folder).photo_limit)
        .unwrap_or_default()
}

/// Sets the limit (Diagnostics, 3B), and drops photos past it now.
pub async fn set_photo_limit(limit: PhotoLimit) {
    let saved = crate::data_folder::platform_data_folder()
        .map(|folder| crate::settings::update(&folder, |settings| settings.photo_limit = limit));
    if let Some(Err(error)) = saved {
        tracing::warn!(target: log_area::APP, %error, "couldn't save the photos limit");
    }
    if let Some(files) = current() {
        drop_over_limit(&files).await;
    }
}

/// Drops the photos opened longest ago until they're within the limit.
async fn drop_over_limit(files: &LocalFiles) {
    let Some(limit) = photo_limit().bytes() else {
        return;
    };
    if let Err(error) = files.drop_photos_over(limit).await {
        tracing::warn!(target: log_area::SYNC, %error, "couldn't drop photos past the limit");
    }
}

/// Downloads the thumbnails and PDFs of the copy's attachments that aren't here yet, one at a
/// time, until none are missing or one fails (offline, or the server's busy: tried again after
/// the next pull). How many were downloaded.
pub async fn fetch_missing() -> usize {
    if FETCHING.swap(true, Ordering::Relaxed) {
        return 0;
    }
    let fetched = fetch_all_missing().await;
    FETCHING.store(false, Ordering::Relaxed);
    if fetched > 0 {
        tracing::info!(target: log_area::SYNC, fetched, "downloaded thumbnails and PDFs");
    }
    fetched
}

async fn fetch_all_missing() -> usize {
    let (Some(copy), Some(files)) = (crate::local_copy::current(), current()) else {
        return 0;
    };
    let Ok(attachments) = copy.all_attachments().await else {
        return 0;
    };
    let mut fetched = 0;
    for (id, attachment) in attachments {
        let mut wanted = vec![AttachmentPart::Thumbnail];
        if matches!(attachment.content, Content::Document { .. }) {
            wanted.push(AttachmentPart::File);
        }
        for part in wanted {
            if crate::local_copy::is_offline() {
                return fetched;
            }
            let Some((hash, _)) = kept_as(&attachment, part) else {
                continue;
            };
            if !files
                .missing(std::slice::from_ref(&hash))
                .await
                .is_ok_and(|missing| !missing.is_empty())
            {
                continue;
            }
            match crate::files::download(id, part).await {
                Ok(bytes) => {
                    keep(id, part, &bytes).await;
                    fetched += 1;
                }
                Err(error) => {
                    tracing::debug!(target: log_area::SYNC, attachment = %id, %error, "couldn't download a file; trying again later");
                    return fetched;
                }
            }
        }
    }
    fetched
}

#[cfg(test)]
mod tests {
    use domain::attachment::{AttachmentOwner, FileName, PhotoMetadata, PhotoSize, StoredFile};
    use domain::test_support::test_id;
    use domain::time::UnixMillis;

    use super::*;

    fn file(byte: u8) -> StoredFile {
        StoredFile {
            hash: hash_of(&[byte]),
            bytes: 1,
        }
    }

    fn attachment(content: Content) -> Attachment {
        Attachment {
            asset: Some(test_id(1)),
            owner: AttachmentOwner::Vehicle,
            file: file(1),
            thumbnail: Some(file(2)),
            name: FileName::from_upload("a.jpg").unwrap(),
            content,
            added_at: UnixMillis::from_millis(0),
            added_by: test_id(2),
            received_at: UnixMillis::from_millis(0),
        }
    }

    #[test]
    fn a_photo_is_kept_as_a_photo_a_pdf_as_a_pdf_and_an_original_never() {
        let photo = attachment(Content::Photo {
            size: PhotoSize::Small,
            width: 1,
            height: 1,
            metadata: PhotoMetadata::default(),
            original: None,
        });
        let pdf = attachment(Content::Document { pages: 1 });
        assert_eq!(
            kept_as(&photo, AttachmentPart::File),
            Some((file(1).hash, LocalKind::Photo))
        );
        assert_eq!(
            kept_as(&pdf, AttachmentPart::File),
            Some((file(1).hash, LocalKind::Pdf))
        );
        assert_eq!(
            kept_as(&photo, AttachmentPart::Thumbnail),
            Some((file(2).hash, LocalKind::Thumbnail))
        );
        assert_eq!(kept_as(&photo, AttachmentPart::Original), None);
    }
}
