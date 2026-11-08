//! How big and how long a request may be (decision #071; `docs/sub-plans/04-security.md`
//! §5.1).
//!
//! One Axum middleware, outside the auth layer, for every request:
//! - **Size:** a body over its limit is refused with **413**: at once if its `Content-Length`
//!   says so, otherwise when reading reaches the limit. Everything but uploads gets
//!   [`BODY_LIMIT`], and its body is read here in full first: Dioxus's server functions
//!   panic on a body they can't read (dioxus-fullstack 0.7.10), so they're only given whole
//!   ones. An upload of a photo or PDF gets [`UPLOAD_BODY_LIMIT`] and is passed on as it
//!   arrives, cut off at the limit (its handler answers a body it can't read).
//! - **Time:** a request that hasn't been answered in time gets **408**. Uploads and
//!   downloads get [`FILE_TIME_LIMIT`] (a 50 MB PDF over a phone connection); everything else
//!   [`TIME_LIMIT`]. Receiving the request and waiting for the answer are timed, not sending
//!   the answer's body.

use std::time::Duration;

use axum::Router;
use axum::body::{Body, to_bytes};
use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::http::header::CONTENT_LENGTH;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use domain::api::{DOWNLOAD_PATH_PREFIX, UPLOAD_PATH};
use domain::log_area;

/// The most a request body may be, except uploads: 2 MB.
pub const BODY_LIMIT: usize = 2 * 1024 * 1024;

/// The most an upload's body may be: a PDF at its limit (50 MB), with room to spare.
pub const UPLOAD_BODY_LIMIT: usize = 51 * 1024 * 1024;

/// How long a request may take to be answered, except uploads and downloads.
pub const TIME_LIMIT: Duration = Duration::from_mins(1);

/// How long an upload or download may take to be answered.
pub const FILE_TIME_LIMIT: Duration = Duration::from_mins(10);

/// The limits the middleware applies.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Limits {
    /// Body size, except uploads.
    pub body: usize,
    /// Body size for uploads.
    pub upload_body: usize,
    /// Time, except uploads and downloads.
    pub time: Duration,
    /// Time for uploads and downloads.
    pub file_time: Duration,
}

impl Limits {
    /// The server's limits.
    pub const SERVER: Self = Self {
        body: BODY_LIMIT,
        upload_body: UPLOAD_BODY_LIMIT,
        time: TIME_LIMIT,
        file_time: FILE_TIME_LIMIT,
    };

    /// The body size and time limits for a request to `path`, and whether its body is passed
    /// on as it arrives (uploads) rather than read in full first.
    fn for_path(self, path: &str) -> (usize, Duration, bool) {
        if path == UPLOAD_PATH {
            (self.upload_body, self.file_time, true)
        } else if path.starts_with(DOWNLOAD_PATH_PREFIX) {
            (self.body, self.file_time, false)
        } else {
            (self.body, self.time, false)
        }
    }
}

/// Puts `router` behind the size and time limits.
pub fn apply<S>(router: Router<S>, limits: Limits) -> Router<S>
where
    S: Clone + Send + Sync + 'static,
{
    router.layer(middleware::from_fn_with_state(limits, limit))
}

/// The middleware.
async fn limit(State(limits): State<Limits>, request: Request, next: Next) -> Response {
    let path = request.uri().path().to_owned();
    let (body_limit, time_limit, streamed) = limits.for_path(&path);
    let declared = request
        .headers()
        .get(CONTENT_LENGTH)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.parse::<u64>().ok());
    if declared.is_some_and(|length| length > body_limit as u64) {
        tracing::info!(target: log_area::APP, path, length = declared, "request refused: body too large");
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    }
    let answer = async {
        let request = if streamed {
            request.map(|body| Body::new(http_body_util::Limited::new(body, body_limit)))
        } else {
            let (parts, body) = request.into_parts();
            let Ok(bytes) = to_bytes(body, body_limit).await else {
                tracing::info!(target: log_area::APP, path, "request refused: body too large or unreadable");
                return StatusCode::PAYLOAD_TOO_LARGE.into_response();
            };
            Request::from_parts(parts, Body::from(bytes))
        };
        next.run(request).await
    };
    if let Ok(response) = tokio::time::timeout(time_limit, answer).await {
        response
    } else {
        tracing::warn!(target: log_area::APP, path, seconds = time_limit.as_secs(), "request timed out");
        StatusCode::REQUEST_TIMEOUT.into_response()
    }
}

#[cfg(test)]
mod tests {
    use axum::routing::{get, post};
    use http_body_util::BodyExt;
    use tower::ServiceExt;

    use super::*;

    /// Small limits, so tests are quick.
    const TEST: Limits = Limits {
        body: 10,
        upload_body: 100,
        time: Duration::from_millis(50),
        file_time: Duration::from_millis(500),
    };

    /// Echoes the body's length, or 400 if it couldn't be read whole.
    async fn length(request: Request) -> Response {
        match to_bytes(request.into_body(), usize::MAX).await {
            Ok(bytes) => bytes.len().to_string().into_response(),
            Err(_) => StatusCode::BAD_REQUEST.into_response(),
        }
    }

    /// Answers after `millis` milliseconds.
    async fn slow(millis: u64) -> &'static str {
        tokio::time::sleep(Duration::from_millis(millis)).await;
        "done"
    }

    fn router() -> Router {
        let download = format!("{DOWNLOAD_PATH_PREFIX}{{id}}");
        let routes = Router::new()
            .route("/api/v1/echo", post(length))
            .route(UPLOAD_PATH, post(length))
            .route("/api/v1/slow", get(|| slow(200)))
            .route(&download, get(|| slow(200)));
        apply(routes, TEST)
    }

    async fn send(request: Request) -> (StatusCode, String) {
        let response = router().oneshot(request).await.unwrap();
        let status = response.status();
        let body = response.into_body().collect().await.unwrap().to_bytes();
        (status, String::from_utf8(body.to_vec()).unwrap())
    }

    fn post_body(path: &str, bytes: usize, with_length: bool) -> Request {
        let builder = Request::post(path);
        let builder = if with_length {
            builder.header(CONTENT_LENGTH, bytes)
        } else {
            builder
        };
        builder.body(Body::from(vec![b'x'; bytes])).unwrap()
    }

    #[tokio::test]
    async fn a_body_within_its_limit_passes() {
        assert_eq!(
            send(post_body("/api/v1/echo", 10, true)).await,
            (StatusCode::OK, "10".into())
        );
        assert_eq!(
            send(post_body(UPLOAD_PATH, 100, true)).await,
            (StatusCode::OK, "100".into()),
            "uploads get the larger limit"
        );
    }

    #[tokio::test]
    async fn a_body_declared_too_large_is_refused_before_it_is_read() {
        assert_eq!(
            send(post_body("/api/v1/echo", 11, true)).await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
        assert_eq!(
            send(post_body(UPLOAD_PATH, 101, true)).await.0,
            StatusCode::PAYLOAD_TOO_LARGE
        );
    }

    #[tokio::test]
    async fn a_body_without_a_length_is_refused_or_cut_off_at_the_limit() {
        assert_eq!(
            send(post_body("/api/v1/echo", 11, false)).await.0,
            StatusCode::PAYLOAD_TOO_LARGE,
            "read in full here, so the handler never sees it"
        );
        assert_eq!(
            send(post_body("/api/v1/echo", 10, false)).await,
            (StatusCode::OK, "10".into())
        );
        assert_eq!(
            send(post_body(UPLOAD_PATH, 101, false)).await.0,
            StatusCode::BAD_REQUEST,
            "an upload's handler can't read past the limit"
        );
    }

    #[tokio::test]
    async fn a_slow_answer_times_out_but_a_download_gets_longer() {
        let slow = Request::get("/api/v1/slow").body(Body::empty()).unwrap();
        assert_eq!(send(slow).await.0, StatusCode::REQUEST_TIMEOUT);
        let download = Request::get(format!("{DOWNLOAD_PATH_PREFIX}abc"))
            .body(Body::empty())
            .unwrap();
        assert_eq!(send(download).await, (StatusCode::OK, "done".into()));
    }
}
