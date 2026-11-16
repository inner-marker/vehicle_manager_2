//! Uploading and downloading photos and PDFs (milestone 8; decisions #036, #037, #065).
//!
//! Plain routes, not server functions: an upload's body is the file itself (up to a 50 MB PDF,
//! so only this route's limit is raised), and a download's answer is the file. Both are under
//! `/api/`, so the auth layer guards them like every server function; the work is in
//! `server::attachments`.

use dioxus::prelude::dioxus_server::axum::Router;
use dioxus::prelude::dioxus_server::axum::body::{Body, to_bytes};
use dioxus::prelude::dioxus_server::axum::extract::{Path, Request};
use dioxus::prelude::dioxus_server::axum::http::{HeaderValue, StatusCode, header};
use dioxus::prelude::dioxus_server::axum::response::{IntoResponse, Response};
use dioxus::prelude::dioxus_server::axum::routing::{get, post};
use domain::api::{AttachmentPart, AttachmentsOutcome, UPLOAD_PATH, UploadOutcome, UploadPurpose};
use domain::attachment::{AttachmentId, PDF_MAX_BYTES};
use domain::time::Clock;
use server::sessions::SignedIn;

use crate::logging::SystemClock;

/// The most an upload's body may be (the limits layer also enforces it, decision #071).
const UPLOAD_LIMIT: usize = server::limits::UPLOAD_BODY_LIMIT;

/// The upload and download routes, to merge into the server's router (inside the auth layer).
pub fn routes<S>() -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    // The limit is checked when the body is read, against the largest file (a PDF).
    debug_assert!(u64::try_from(UPLOAD_LIMIT).is_ok_and(|limit| limit > PDF_MAX_BYTES));
    Router::new()
        .route(UPLOAD_PATH, post(upload))
        .route("/api/v1/attachments/{id}/{part}", get(download))
}

/// An answer as JSON.
fn json(value: &impl serde::Serialize) -> Response {
    match serde_json::to_vec(value) {
        Ok(body) => (
            [(
                header::CONTENT_TYPE,
                HeaderValue::from_static("application/json"),
            )],
            body,
        )
            .into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

/// `POST /api/v1/files/upload?for=…`: stores the body for what the query says. Answers an
/// [`UploadOutcome`] as JSON.
async fn upload(request: Request) -> Response {
    let purpose = request.uri().query().and_then(UploadPurpose::from_query);
    let user = request.extensions().get::<SignedIn>().cloned();
    let (Some(database), Some(store), Some(user)) = (
        server::startup::database(),
        server::startup::file_store(),
        user,
    ) else {
        return json(&UploadOutcome::Unavailable);
    };
    let Some(purpose) = purpose else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let Ok(bytes) = to_bytes(request.into_body(), UPLOAD_LIMIT).await else {
        return json(&UploadOutcome::Invalid {
            problem: domain::attachment::AttachmentProblem::PdfTooLarge,
        });
    };
    let files = storage::files::Files::Folder(store.clone());
    let context = server::attachments::UploadContext {
        database,
        files: &files,
        user: &user,
        health: server::health::current(),
        now: SystemClock.now(),
    };
    json(&server::attachments::upload(&context, purpose, bytes.to_vec()).await)
}

/// A file name for `Content-Disposition`, percent-encoded (RFC 5987).
fn encoded_name(name: &str) -> String {
    name.bytes()
        .map(|byte| {
            if byte.is_ascii_alphanumeric() || b".-_~".contains(&byte) {
                char::from(byte).to_string()
            } else {
                format!("%{byte:02X}")
            }
        })
        .collect()
}

/// `GET /api/v1/attachments/<id>/<part>`: the file, if the person may see it.
async fn download(Path((id, part)): Path<(String, String)>, request: Request) -> Response {
    let (Ok(id), Some(part)) = (id.parse::<AttachmentId>(), AttachmentPart::parse(&part)) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let user = request.extensions().get::<SignedIn>().cloned();
    let (Some(database), Some(database_id), Some(store), Some(user)) = (
        server::startup::database(),
        server::startup::database_id(),
        server::startup::file_store(),
        user,
    ) else {
        return StatusCode::SERVICE_UNAVAILABLE.into_response();
    };
    let context = server::assets::AssetsContext {
        database,
        database_id,
        user: &user,
        health: server::health::current(),
        clock: &SystemClock,
        origin: ::server::origin::Origin::Online,
    };
    match server::attachments::download(&context, store, id, part).await {
        Ok(file) => {
            let disposition = format!(
                "inline; filename*=UTF-8''{}",
                encoded_name(file.name.as_str())
            );
            let mut response = Response::new(Body::from(file.bytes));
            let headers = response.headers_mut();
            headers.insert(header::CONTENT_TYPE, HeaderValue::from_static(file.mime));
            if let Ok(value) = HeaderValue::from_str(&disposition) {
                headers.insert(header::CONTENT_DISPOSITION, value);
            }
            // Never changes (files are named by their contents), but only for this person.
            headers.insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("private, max-age=86400"),
            );
            headers.insert(
                header::X_CONTENT_TYPE_OPTIONS,
                HeaderValue::from_static("nosniff"),
            );
            response
        }
        Err(AttachmentsOutcome::NotAllowed) => StatusCode::FORBIDDEN.into_response(),
        Err(
            AttachmentsOutcome::NotFound
            | AttachmentsOutcome::Invalid { .. }
            | AttachmentsOutcome::Attachments { .. }
            | AttachmentsOutcome::Saved { .. }
            | AttachmentsOutcome::Done,
        ) => StatusCode::NOT_FOUND.into_response(),
        Err(AttachmentsOutcome::Unavailable) => StatusCode::SERVICE_UNAVAILABLE.into_response(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn file_names_are_percent_encoded_for_downloads() {
        assert_eq!(encoded_name("Invoice 2291.pdf"), "Invoice%202291.pdf");
        assert_eq!(encoded_name("café\";.jpg"), "caf%C3%A9%22%3B.jpg");
    }
}
