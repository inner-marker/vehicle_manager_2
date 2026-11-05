//! Getting a photo ready to upload (milestone 8; plan §3 "Photos from the web UI on a phone";
//! decision #036).
//!
//! 1. **Read its metadata first** (time taken, GPS, camera), because resizing drops it:
//!    [`read_metadata`], in Rust (`kamadak-exif`), on every platform.
//! 2. **Shrink it and turn it upright** in the webview's own image decoder ([`shrink`]), so
//!    a phone sends a small JPEG rather than a camera's several megabytes. The browser applies
//!    the camera's orientation when it draws the photo. On a Mac, the desktop app's webview
//!    also reads HEIC from Photos.
//! 3. The server checks it and saves it again either way (8d.1); if the webview can't read it
//!    (e.g. HEIC on Linux), the file is sent as it is, and the server explains.

use std::io::Cursor;

use dioxus::prelude::*;
use domain::attachment::{CAMERA_MAX_CHARS, PhotoMetadata};
use domain::location::Coordinates;
use domain::time::UnixMillis;
use exif::{In, Reader, Tag, Value};
use serde::{Deserialize, Serialize};

/// What a photo's EXIF said.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct ReadMetadata {
    /// The fields kept with the photo (its raw EXIF goes separately).
    pub(crate) metadata: PhotoMetadata,
    /// When it was taken, as the camera's local time with no zone (`2026:11:03 09:52:00`),
    /// when the camera didn't say its offset: the webview reads it in the device's zone.
    pub(crate) local_time: Option<String>,
    /// All of its EXIF, as read.
    pub(crate) raw: Option<Vec<u8>>,
}

/// An ASCII field's text, trimmed of padding and NULs.
fn ascii(exif: &exif::Exif, tag: Tag) -> Option<String> {
    let Value::Ascii(parts) = &exif.get_field(tag, In::PRIMARY)?.value else {
        return None;
    };
    let text: String = parts
        .iter()
        .flat_map(|part| part.iter())
        .map(|byte| char::from(*byte))
        .collect::<String>()
        .trim_matches(|c: char| c == '\0' || c.is_whitespace())
        .to_owned();
    (!text.is_empty()).then_some(text)
}

/// One GPS coordinate in degrees, from its degrees/minutes/seconds and its reference
/// (`N`/`S` or `E`/`W`; south and west are negative).
fn degrees(exif: &exif::Exif, value: Tag, reference: Tag) -> Option<f64> {
    let Value::Rational(parts) = &exif.get_field(value, In::PRIMARY)?.value else {
        return None;
    };
    let [d, m, s] = parts.as_slice() else {
        return None;
    };
    if [d, m, s].iter().any(|part| part.denom == 0) {
        return None;
    }
    let magnitude = d.to_f64() + m.to_f64() / 60.0 + s.to_f64() / 3600.0;
    let negative = matches!(ascii(exif, reference).as_deref(), Some("S" | "W"));
    Some(if negative { -magnitude } else { magnitude })
}

/// When it was taken, if the camera said its offset from UTC too.
fn taken_at(exif: &exif::Exif) -> (Option<UnixMillis>, Option<String>) {
    let Some(text) = ascii(exif, Tag::DateTimeOriginal).or_else(|| ascii(exif, Tag::DateTime))
    else {
        return (None, None);
    };
    let Ok(mut when) = exif::DateTime::from_ascii(text.as_bytes()) else {
        return (None, None);
    };
    let offset = ascii(exif, Tag::OffsetTimeOriginal).or_else(|| ascii(exif, Tag::OffsetTime));
    if let Some(offset) = offset
        && when.parse_offset(offset.as_bytes()).is_ok()
        && let Some(minutes) = when.offset
    {
        let civil = jiff::civil::date(
            i16::try_from(when.year).unwrap_or(0),
            i8::try_from(when.month).unwrap_or(0),
            i8::try_from(when.day).unwrap_or(0),
        )
        .to_datetime(jiff::civil::time(
            i8::try_from(when.hour).unwrap_or(0),
            i8::try_from(when.minute).unwrap_or(0),
            i8::try_from(when.second).unwrap_or(0),
            0,
        ));
        let zone =
            jiff::tz::Offset::from_seconds(i32::from(minutes) * 60).map(jiff::tz::TimeZone::fixed);
        if let Ok(zone) = zone
            && let Ok(moment) = civil.to_zoned(zone)
        {
            return (
                Some(UnixMillis::from_millis(moment.timestamp().as_millisecond())),
                None,
            );
        }
    }
    (None, Some(text))
}

/// What `bytes` (a photo) say about themselves; nothing if they carry no EXIF.
#[must_use]
pub(crate) fn read_metadata(bytes: &[u8]) -> ReadMetadata {
    let Ok(exif) = Reader::new().read_from_container(&mut Cursor::new(bytes)) else {
        return ReadMetadata::default();
    };
    let coordinates = degrees(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef)
        .zip(degrees(&exif, Tag::GPSLongitude, Tag::GPSLongitudeRef))
        .and_then(|(latitude, longitude)| Coordinates::from_degrees(latitude, longitude).ok());
    let make = ascii(&exif, Tag::Make);
    let model = ascii(&exif, Tag::Model);
    let camera = match (make, model) {
        // Many cameras repeat the make in the model ("Apple" + "iPhone 15 Pro" vs.
        // "Canon" + "Canon EOS R6").
        (Some(make), Some(model)) if model.starts_with(&make) => Some(model),
        (Some(make), Some(model)) => Some(format!("{make} {model}")),
        (make, model) => make.or(model),
    }
    .map(|camera| camera.chars().take(CAMERA_MAX_CHARS).collect());
    let (taken_at, local_time) = taken_at(&exif);
    ReadMetadata {
        metadata: PhotoMetadata {
            taken_at,
            coordinates,
            camera,
            raw: None,
        },
        local_time,
        raw: Some(exif.buf().to_vec()),
    }
}

/// The webview's side of [`shrink`]: draw the photo (upright) at most `edge` pixels on its long
/// side, as a JPEG at quality 0.85; and read a camera's local time in the device's zone.
const SHRINK_SCRIPT: &str = r##"
const input = await dioxus.recv();
const out = {};
try {
  const bytes = Uint8Array.from(atob(input.data), (c) => c.charCodeAt(0));
  const url = URL.createObjectURL(new Blob([bytes], { type: input.mime || "image/jpeg" }));
  const image = await new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => resolve(img);
    img.onerror = () => reject(new Error("can't read it"));
    img.src = url;
  });
  URL.revokeObjectURL(url);
  const long = Math.max(image.naturalWidth, image.naturalHeight);
  const scale = Math.min(1, input.edge / long);
  const width = Math.max(1, Math.round(image.naturalWidth * scale));
  const height = Math.max(1, Math.round(image.naturalHeight * scale));
  const canvas = document.createElement("canvas");
  canvas.width = width; canvas.height = height;
  const context = canvas.getContext("2d");
  context.fillStyle = "#fff"; context.fillRect(0, 0, width, height);
  context.drawImage(image, 0, 0, width, height);
  const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.85));
  const jpeg = new Uint8Array(await blob.arrayBuffer());
  let text = "";
  for (let i = 0; i < jpeg.length; i += 0x8000) {
    text += String.fromCharCode.apply(null, jpeg.subarray(i, i + 0x8000));
  }
  out.jpeg = btoa(text);
} catch (error) {
  out.error = String(error);
}
const local = /^(\d{4}):(\d{2}):(\d{2}) (\d{2}):(\d{2}):(\d{2})/.exec(input.local_time || "");
if (local) {
  out.taken = new Date(+local[1], +local[2] - 1, +local[3], +local[4], +local[5], +local[6]).getTime();
}
return out;
"##;

/// A JavaScript time (milliseconds, a float) as whole milliseconds, if it's a sensible one.
fn whole_millis(ms: f64) -> Option<i64> {
    // Within ±10,000 years of 1970: far inside both types' ranges.
    const LIMIT: f64 = 315_576_000_000_000.0;
    if !ms.is_finite() || ms.abs() > LIMIT {
        return None;
    }
    #[expect(
        clippy::cast_possible_truncation,
        reason = "rounded and range-checked just above"
    )]
    Some(ms.round() as i64)
}

#[derive(Serialize)]
struct ShrinkInput<'a> {
    data: String,
    mime: &'a str,
    edge: u32,
    local_time: Option<&'a str>,
}

#[derive(Deserialize)]
struct ShrinkOutput {
    jpeg: Option<String>,
    error: Option<String>,
    taken: Option<f64>,
}

/// A photo shrunk for upload, and when it was taken if only the camera's local time was known.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Shrunk {
    /// The JPEG, or `None` if the webview couldn't read the photo (send it as it is).
    pub(crate) jpeg: Option<Vec<u8>>,
    /// When it was taken (from the camera's local time, read in the device's zone).
    pub(crate) taken_at: Option<UnixMillis>,
}

/// Shrinks `bytes` (of type `mime`) to `edge` pixels on the long side, upright, as a JPEG.
pub(crate) async fn shrink(
    bytes: &[u8],
    mime: &str,
    edge: u32,
    local_time: Option<&str>,
) -> Shrunk {
    let eval = document::eval(SHRINK_SCRIPT);
    let input = ShrinkInput {
        data: crate::files::to_base64(bytes),
        mime,
        edge,
        local_time,
    };
    if eval.send(input).is_err() {
        return Shrunk {
            jpeg: None,
            taken_at: None,
        };
    }
    match eval.join::<ShrinkOutput>().await {
        Ok(output) => {
            if let Some(error) = &output.error {
                tracing::debug!(target: domain::log_area::UI, error, "the webview couldn't shrink a photo; sending it as it is");
            }
            Shrunk {
                jpeg: output.jpeg.as_deref().and_then(crate::files::from_base64),
                taken_at: output
                    .taken
                    .and_then(whole_millis)
                    .map(UnixMillis::from_millis),
            }
        }
        Err(error) => {
            tracing::debug!(target: domain::log_area::UI, %error, "couldn't shrink a photo");
            Shrunk {
                jpeg: None,
                taken_at: None,
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use exif::experimental::Writer;
    use exif::{Field, Rational};

    use super::*;

    fn field(tag: Tag, value: Value) -> Field {
        Field {
            tag,
            ifd_num: In::PRIMARY,
            value,
        }
    }

    fn ascii_value(text: &str) -> Value {
        Value::Ascii(vec![text.as_bytes().to_vec()])
    }

    fn dms(degrees: u32, minutes: u32, seconds_hundredths: u32) -> Value {
        Value::Rational(vec![
            Rational::from((degrees, 1)),
            Rational::from((minutes, 1)),
            Rational::from((seconds_hundredths, 100)),
        ])
    }

    /// EXIF as a camera writes it (a bare TIFF block, which the reader accepts like a JPEG's).
    fn exif(fields: &[Field]) -> Vec<u8> {
        let mut writer = Writer::new();
        for field in fields {
            writer.push_field(field);
        }
        let mut bytes = Cursor::new(Vec::new());
        writer.write(&mut bytes, false).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn a_phones_photo_gives_its_time_place_and_camera() {
        let bytes = exif(&[
            field(Tag::Make, ascii_value("Apple")),
            field(Tag::Model, ascii_value("iPhone 15 Pro")),
            field(Tag::DateTimeOriginal, ascii_value("2026:11:03 09:52:00")),
            field(Tag::OffsetTimeOriginal, ascii_value("-05:00")),
            field(Tag::GPSLatitude, dms(35, 35, 4236)),
            field(Tag::GPSLatitudeRef, ascii_value("N")),
            field(Tag::GPSLongitude, dms(82, 33, 540)),
            field(Tag::GPSLongitudeRef, ascii_value("W")),
        ]);
        let read = read_metadata(&bytes);
        assert_eq!(read.metadata.camera.as_deref(), Some("Apple iPhone 15 Pro"));
        // 09:52 at UTC−5 is 14:52 UTC.
        assert_eq!(
            read.metadata
                .taken_at
                .map(domain::time_utils::format_utc_iso8601),
            Some("2026-11-03T14:52:00.000Z".to_owned())
        );
        assert_eq!(read.local_time, None);
        let point = read.metadata.coordinates.unwrap();
        assert!(
            (point.latitude() - 35.5951).abs() < 0.0001,
            "{}",
            point.latitude()
        );
        assert!(
            (point.longitude() + 82.5515).abs() < 0.0001,
            "{}",
            point.longitude()
        );
        assert!(read.raw.is_some());
    }

    #[test]
    fn without_an_offset_the_time_is_left_to_the_device_and_the_make_isnt_repeated() {
        let bytes = exif(&[
            field(Tag::Make, ascii_value("Canon")),
            field(Tag::Model, ascii_value("Canon EOS R6")),
            field(Tag::DateTimeOriginal, ascii_value("2026:11:03 09:52:00")),
        ]);
        let read = read_metadata(&bytes);
        assert_eq!(read.metadata.camera.as_deref(), Some("Canon EOS R6"));
        assert_eq!(read.metadata.taken_at, None);
        assert_eq!(read.local_time.as_deref(), Some("2026:11:03 09:52:00"));
        assert_eq!(read.metadata.coordinates, None);
    }

    #[test]
    fn a_photo_without_exif_has_no_metadata() {
        assert_eq!(
            read_metadata(b"\x89PNG\r\n\x1a\n not much"),
            ReadMetadata::default()
        );
        assert_eq!(whole_millis(f64::NAN), None);
        assert_eq!(whole_millis(1_793_651_400_000.4), Some(1_793_651_400_000));
    }
}
