//! Sending photos and PDFs to the server and fetching them back (milestone 8; decisions #036,
//! #037).
//!
//! Uploads and downloads use the server's plain file routes (`domain::api::UPLOAD_PATH` and
//! `domain::api::download_path`), not server functions, so they're sent here with the same
//! headers every server-function call carries ([`crate::client::current_headers`]): the API
//! version, and on desktop the session token. On the web the session cookie goes with them.
//!
//! A fetched file is shown from a `data:` URL, because an `<img src>` pointing at the server
//! couldn't carry those headers.

use dioxus::prelude::dioxus_fullstack::{get_server_url, reqwest};
use dioxus::prelude::*;
use domain::api::{AttachmentPart, UPLOAD_PATH, UploadOutcome, UploadPurpose, download_path};
use domain::attachment::AttachmentId;

/// The server's address: the desktop app's chosen server, or (on the web) the page's own.
async fn base_url() -> Result<String, String> {
    let set = get_server_url();
    if !set.is_empty() {
        return Ok(set.trim_end_matches('/').to_owned());
    }
    document::eval("return location.origin")
        .join::<String>()
        .await
        .map_err(|error| error.to_string())
}

/// Sends `bytes` for `purpose`; the server's answer.
///
/// # Errors
///
/// What went wrong reaching the server (not a refusal, which is an [`UploadOutcome`]).
pub async fn upload(purpose: UploadPurpose, bytes: Vec<u8>) -> Result<UploadOutcome, String> {
    let url = format!("{}{UPLOAD_PATH}?{}", base_url().await?, purpose.to_query());
    let response = reqwest::Client::new()
        .post(url)
        .headers(crate::client::current_headers())
        .body(bytes)
        .send()
        .await
        .map_err(|error| error.to_string())?;
    // The server's size limit (`server::limits`) refuses an oversized body before the upload
    // route sees it; it's the same refusal as the check before sending.
    if response.status() == StatusCode::PAYLOAD_TOO_LARGE {
        return Ok(UploadOutcome::Invalid {
            problem: domain::attachment::AttachmentProblem::PdfTooLarge,
        });
    }
    response
        .json::<UploadOutcome>()
        .await
        .map_err(|error| error.to_string())
}

/// One part of an attachment's file.
///
/// # Errors
///
/// What went wrong, including a refusal (e.g. "403 Forbidden").
pub async fn download(id: AttachmentId, part: AttachmentPart) -> Result<Vec<u8>, String> {
    let url = format!("{}{}", base_url().await?, download_path(id, part));
    let response = reqwest::Client::new()
        .get(url)
        .headers(crate::client::current_headers())
        .send()
        .await
        .map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(response.status().to_string());
    }
    response
        .bytes()
        .await
        .map(|bytes| bytes.to_vec())
        .map_err(|error| error.to_string())
}

const ALPHABET: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

/// `bytes` in standard Base64 (with padding).
#[must_use]
pub fn to_base64(bytes: &[u8]) -> String {
    let mut text = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for chunk in bytes.chunks(3) {
        let [a, b, c] = [
            chunk[0],
            chunk.get(1).copied().unwrap_or(0),
            chunk.get(2).copied().unwrap_or(0),
        ];
        let sextets = [
            a >> 2,
            ((a & 0b11) << 4) | (b >> 4),
            ((b & 0x0F) << 2) | (c >> 6),
            c & 0x3F,
        ];
        for (index, sextet) in sextets.iter().enumerate() {
            if index <= chunk.len() {
                text.push(char::from(ALPHABET[usize::from(*sextet)]));
            } else {
                text.push('=');
            }
        }
    }
    text
}

/// The bytes of standard Base64 text (padding optional), if it is that.
#[must_use]
pub fn from_base64(text: &str) -> Option<Vec<u8>> {
    let mut bytes = Vec::with_capacity(text.len() / 4 * 3);
    let mut buffer = 0_u32;
    let mut bits = 0;
    for character in text.bytes().filter(|byte| *byte != b'=') {
        let value = ALPHABET.iter().position(|letter| *letter == character)?;
        buffer = (buffer << 6) | u32::try_from(value).ok()?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            bytes.push(u8::try_from((buffer >> bits) & 0xFF).ok()?);
        }
    }
    Some(bytes)
}

/// A `data:` URL for showing `bytes` of type `mime`.
#[must_use]
pub fn data_url(mime: &str, bytes: &[u8]) -> String {
    format!("data:{mime};base64,{}", to_base64(bytes))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips_and_matches_the_standard() {
        assert_eq!(to_base64(b""), "");
        assert_eq!(to_base64(b"f"), "Zg==");
        assert_eq!(to_base64(b"fo"), "Zm8=");
        assert_eq!(to_base64(b"foo"), "Zm9v");
        assert_eq!(to_base64(&[0xFF, 0xD8, 0xFF, 0xE0]), "/9j/4A==");
        for length in 0..40_u8 {
            let bytes: Vec<u8> = (0..length).map(|n| n.wrapping_mul(37)).collect();
            assert_eq!(from_base64(&to_base64(&bytes)), Some(bytes));
        }
        assert_eq!(from_base64("not base64!"), None);
        assert_eq!(
            data_url("image/jpeg", b"foo"),
            "data:image/jpeg;base64,Zm9v"
        );
    }
}
