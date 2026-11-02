//! Short labels people type for things they define: a meter's name, a custom field's label.
//!
//! # Examples
//!
//! ```
//! use domain::label::{Label, LabelError};
//!
//! assert_eq!(Label::parse(" Engine hours ").unwrap().as_str(), "Engine hours");
//! assert_eq!(Label::parse(""), Err(LabelError::Empty));
//! ```

use serde::{Deserialize, Serialize};

use crate::i18n::{MessageId, messages};

/// The most characters a label may have.
pub const LABEL_MAX_CHARS: usize = 40;

/// A label: trimmed, 1–40 characters.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(try_from = "String", into = "String")]
pub struct Label(String);

/// Why a label was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum LabelError {
    /// Nothing was entered.
    Empty,
    /// More than [`LABEL_MAX_CHARS`] characters.
    TooLong,
}

impl LabelError {
    /// The message explaining the problem.
    #[must_use]
    pub const fn message(self) -> MessageId {
        match self {
            Self::Empty => messages::LABEL_EMPTY,
            Self::TooLong => messages::LABEL_TOO_LONG,
        }
    }
}

impl Label {
    /// A label from what was typed.
    ///
    /// # Errors
    ///
    /// [`LabelError`] if it's empty or too long.
    pub fn parse(input: &str) -> Result<Self, LabelError> {
        let label = input.trim();
        match label.chars().count() {
            0 => Err(LabelError::Empty),
            length if length > LABEL_MAX_CHARS => Err(LabelError::TooLong),
            _ => Ok(Self(label.to_owned())),
        }
    }

    /// The label.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

impl TryFrom<String> for Label {
    type Error = String;

    fn try_from(value: String) -> Result<Self, Self::Error> {
        Self::parse(&value).map_err(|error| format!("{error:?}"))
    }
}

impl From<Label> for String {
    fn from(label: Label) -> Self {
        label.0
    }
}
