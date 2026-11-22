//! Small screen choices remembered on this device: which Settings section was open, and
//! similar. Kept in the web view's local storage, as the + Log chooser keeps its last vehicle.
//! Only names of screen choices go here, never records or anything secret (#071).

use dioxus::prelude::*;

/// Every key starts with this, so the app's entries are easy to find.
const PREFIX: &str = "vehiclemanager.";

/// Remembers `value` under `key` on this device.
pub(crate) fn remember(key: &str, value: &str) {
    let script = format!(
        "localStorage.setItem({}, {});",
        quoted(&format!("{PREFIX}{key}")),
        quoted(value)
    );
    let _ = document::eval(&script);
}

/// What was remembered under `key` on this device, if anything. Call after mounting only
/// (never during server rendering).
pub(crate) async fn recall(key: &str) -> Option<String> {
    let script = format!(
        "return localStorage.getItem({});",
        quoted(&format!("{PREFIX}{key}"))
    );
    document::eval(&script)
        .join::<Option<String>>()
        .await
        .ok()
        .flatten()
}

/// `text` as a JavaScript string literal.
fn quoted(text: &str) -> String {
    serde_json::to_string(text).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn text_is_quoted_safely_for_the_script() {
        assert_eq!(quoted("units"), "\"units\"");
        assert_eq!(quoted("a\"b');x"), "\"a\\\"b');x\"");
    }
}
