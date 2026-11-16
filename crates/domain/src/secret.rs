//! Secrets that must never reach a log (phase-3.md 1d; `docs/sub-plans/04-security.md`
//! §7.4): passwords, codes, and tokens.
//!
//! A [`Secret`] prints as `[redacted]` with `{:?}` and has no `Display`, so it can't end up
//! in a log line, an error message, or a `Debug` dump of a struct holding it by accident. Its
//! text is reached only through [`Secret::expose`], where the code means to use it (hash it,
//! compare it, send it). On the wire it's a plain string (`serde(transparent)`), so the API
//! is unchanged.
//!
//! Its memory isn't wiped when dropped (that needs a dependency or `unsafe`): the risk this
//! guards against is logs, not memory dumps.

use std::fmt;

use serde::{Deserialize, Serialize};

/// A password, code, or token. See the module docs.
#[derive(Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct Secret(String);

impl Secret {
    /// Wraps `value`.
    #[must_use]
    pub fn new(value: impl Into<String>) -> Self {
        Self(value.into())
    }

    /// The secret's text, for the code that uses it. Never log or display it.
    #[must_use]
    pub fn expose(&self) -> &str {
        &self.0
    }

    /// The secret's text, taken out of the wrapper (e.g. to move it into a hashing task).
    #[must_use]
    pub fn into_exposed(self) -> String {
        self.0
    }

    /// Whether it's empty.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("[redacted]")
    }
}

impl From<String> for Secret {
    fn from(value: String) -> Self {
        Self(value)
    }
}

impl From<&str> for Secret {
    fn from(value: &str) -> Self {
        Self(value.to_owned())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_secret_never_prints_its_text() {
        #[derive(Debug)]
        #[expect(dead_code, reason = "only printed")]
        struct Request {
            username: String,
            password: Secret,
        }
        let secret = Secret::new("correct horse battery");
        assert_eq!(format!("{secret:?}"), "[redacted]");
        let request = Request {
            username: "sam".into(),
            password: secret.clone(),
        };
        let printed = format!("{request:?} {request:#?}");
        assert!(!printed.contains("horse"), "{printed}");
        assert_eq!(secret.expose(), "correct horse battery");
    }

    #[test]
    fn on_the_wire_it_is_a_plain_string() {
        let secret = Secret::new("K7QM-4XRT");
        let json = serde_json::to_string(&secret).unwrap();
        assert_eq!(json, "\"K7QM-4XRT\"");
        let back: Secret = serde_json::from_str(&json).unwrap();
        assert_eq!(back.expose(), "K7QM-4XRT");
    }
}
