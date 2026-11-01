//! Free-text notes: an asset's description first, later event, squawk, and task notes.
//!
//! Notes are a **Markdown subset** (bold, italics, lists, links; decision #055), stored as
//! text. Until an editor and rendering arrive they're entered and shown as plain text, so
//! nothing here parses Markdown: any text is accepted, and anything outside the subset simply
//! shows as typed.
//!
//! # Examples
//!
//! ```
//! use domain::notes::{Notes, NotesError};
//!
//! let notes = Notes::parse("  Synthetic oil only.\n\n- 5W-30\n").unwrap();
//! assert_eq!(notes.as_str(), "Synthetic oil only.\n\n- 5W-30");
//! assert!(Notes::parse("").unwrap().is_empty());
//! assert_eq!(Notes::parse(&"x".repeat(10_001)), Err(NotesError::TooLong));
//! ```

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, messages};

/// The most characters a note may have: room for a long description, short of a document
/// (documents are attachments, milestone 8).
pub const NOTES_MAX_CHARS: usize = 10_000;

/// A note: trimmed text in the Markdown subset, possibly empty.
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Notes(String);

/// Why a note was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum NotesError {
    /// More than [`NOTES_MAX_CHARS`] characters.
    TooLong,
}

impl NotesError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::TooLong => messages::NOTES_TOO_LONG,
        }
    }
}

impl Notes {
    /// A note from what was typed: surrounding blank space removed.
    ///
    /// # Errors
    ///
    /// [`NotesError::TooLong`] past [`NOTES_MAX_CHARS`].
    pub fn parse(input: &str) -> Result<Self, NotesError> {
        let text = input.trim();
        if text.chars().count() > NOTES_MAX_CHARS {
            return Err(NotesError::TooLong);
        }
        Ok(Self(text.to_owned()))
    }

    /// The note's text.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// Whether there's nothing in it.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

impl TryFrom<String> for Notes {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<Notes> for String {
    fn from(notes: Notes) -> Self {
        notes.0
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn markdown_characters_are_kept_as_typed() {
        let text = "**Bold**, _italics_, [a link](https://example.com)\n1. one";
        assert_eq!(Notes::parse(text).unwrap().as_str(), text);
    }

    #[test]
    fn the_limit_counts_characters_not_bytes() {
        assert!(Notes::parse(&"é".repeat(NOTES_MAX_CHARS)).is_ok());
    }

    #[test]
    fn a_stored_note_that_is_too_long_is_refused_when_read() {
        let json = serde_json::to_string(&"x".repeat(NOTES_MAX_CHARS + 1)).unwrap();
        assert!(serde_json::from_str::<Notes>(&json).is_err());
    }
}
