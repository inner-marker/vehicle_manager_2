//! Commands: the **only** way data changes (decisions #005, #043; AGENTS.md §3).
//!
//! A [`Command`] says what changed, to which record, when (its [`Stamp`]), and in which
//! command format. The server applies it with **last-write-wins per field**: an update
//! carries only the fields that changed, and each field applies only if its stamp is
//! newer than the stamp of the last accepted change to that field.
//!
//! Each kind of record implements [`Record`], naming its storage entity and its
//! [`Patch`] type: one optional value per field, `Some` for "changed".
//!
//! # Examples
//!
//! ```
//! use domain::command::{Change, Patch, Record};
//! use domain::id::Id;
//! use domain::time::UnixMillis;
//!
//! #[derive(Debug)]
//! struct Note;
//!
//! #[derive(Debug, Clone, Default)]
//! struct NotePatch {
//!     title: Option<String>,
//!     body: Option<String>,
//! }
//!
//! impl Patch for NotePatch {
//!     fn changed_fields(&self) -> Vec<&'static str> {
//!         let mut fields = Vec::new();
//!         if self.title.is_some() { fields.push("title"); }
//!         if self.body.is_some() { fields.push("body"); }
//!         fields
//!     }
//!
//!     fn keep_only(&mut self, fields: &[&str]) {
//!         if !fields.contains(&"title") { self.title = None; }
//!         if !fields.contains(&"body") { self.body = None; }
//!     }
//! }
//!
//! impl Record for Note {
//!     const ENTITY: &'static str = "note";
//!     type Patch = NotePatch;
//! }
//!
//! // Changing only the title sends only the title.
//! let change: Change<Note> = Change::Update {
//!     id: Id::generate(UnixMillis::from_millis(1_700_000_000_000), [0; 10]).unwrap(),
//!     patch: NotePatch { title: Some("Oil change".into()), ..NotePatch::default() },
//! };
//! assert_eq!(change.fields(), vec!["title"]);
//! ```

use serde::{Deserialize, Serialize};

use crate::hlc::Stamp;
use crate::id::Id;

/// The command format this build writes. The server accepts formats from 1 up to this,
/// upcasting older ones (plan §6 "Multiple app versions").
pub const COMMAND_FORMAT_VERSION: u32 = 1;

/// Marker for command IDs.
#[derive(Debug)]
pub enum CommandKind {}

/// A command's own ID, so a retried command is recognized and never applied twice.
pub type CommandId = Id<CommandKind>;

/// A kind of record that commands change.
pub trait Record {
    /// The storage entity name (e.g. `"asset"`): stable, never derived from display text.
    const ENTITY: &'static str;
    /// The record's fields, each optional: `Some` means "set to this value".
    type Patch: Patch;
}

/// A set of field values, where an unset field means "not changed".
pub trait Patch: Clone + core::fmt::Debug {
    /// The names of the fields this patch sets, in a fixed order.
    fn changed_fields(&self) -> Vec<&'static str>;

    /// Unsets every field not named in `fields` (the server drops fields that lost to a
    /// newer change).
    fn keep_only(&mut self, fields: &[&str]);
}

/// What a command does to one record.
#[derive(Debug, Serialize, Deserialize)]
#[serde(
    tag = "change",
    rename_all = "kebab-case",
    bound(
        serialize = "R::Patch: Serialize",
        deserialize = "R::Patch: Deserialize<'de>"
    )
)]
pub enum Change<R: Record> {
    /// A new record, with every field it has.
    Create {
        /// The new record's ID, made by the device.
        id: Id<R>,
        /// Its fields.
        values: R::Patch,
    },
    /// Some fields of an existing record changed.
    Update {
        /// Which record.
        id: Id<R>,
        /// Only the changed fields.
        patch: R::Patch,
    },
    /// The record is deleted (a tombstone; it can't be changed afterwards).
    Delete {
        /// Which record.
        id: Id<R>,
    },
}

impl<R: Record> Change<R> {
    /// The record this change applies to.
    #[must_use]
    pub const fn record_id(&self) -> Id<R> {
        match self {
            Self::Create { id, .. } | Self::Update { id, .. } | Self::Delete { id } => *id,
        }
    }

    /// The fields this change sets (none for a delete).
    #[must_use]
    pub fn fields(&self) -> Vec<&'static str> {
        match self {
            Self::Create { values: patch, .. } | Self::Update { patch, .. } => {
                patch.changed_fields()
            }
            Self::Delete { .. } => Vec::new(),
        }
    }
}

// `Clone` by hand: a derive would demand `R: Clone`, but `R` is only a marker type
// (often an empty enum); what actually gets cloned is `R::Patch`.
impl<R: Record> Clone for Change<R> {
    fn clone(&self) -> Self {
        match self {
            Self::Create { id, values } => Self::Create {
                id: *id,
                values: values.clone(),
            },
            Self::Update { id, patch } => Self::Update {
                id: *id,
                patch: patch.clone(),
            },
            Self::Delete { id } => Self::Delete { id: *id },
        }
    }
}

/// One change, with everything the server needs to apply it safely.
#[derive(Debug, Serialize, Deserialize)]
#[serde(bound(
    serialize = "R::Patch: Serialize",
    deserialize = "R::Patch: Deserialize<'de>"
))]
pub struct Command<R: Record> {
    /// Unique per command, so retries are recognized.
    pub id: CommandId,
    /// When and on which device it was made: the last-write-wins key.
    pub stamp: Stamp,
    /// The command format it was written in.
    pub format_version: u32,
    /// What it does.
    pub change: Change<R>,
}

impl<R: Record> Clone for Command<R> {
    fn clone(&self) -> Self {
        Self {
            id: self.id,
            stamp: self.stamp,
            format_version: self.format_version,
            change: self.change.clone(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hlc::Hlc;
    use crate::test_support::test_id;
    use crate::time::UnixMillis;

    #[derive(Debug)]
    struct Note;

    #[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
    struct NotePatch {
        #[serde(skip_serializing_if = "Option::is_none")]
        title: Option<String>,
        #[serde(skip_serializing_if = "Option::is_none")]
        body: Option<String>,
    }

    impl Patch for NotePatch {
        fn changed_fields(&self) -> Vec<&'static str> {
            [
                ("title", self.title.is_some()),
                ("body", self.body.is_some()),
            ]
            .into_iter()
            .filter_map(|(field, set)| set.then_some(field))
            .collect()
        }

        fn keep_only(&mut self, fields: &[&str]) {
            if !fields.contains(&"title") {
                self.title = None;
            }
            if !fields.contains(&"body") {
                self.body = None;
            }
        }
    }

    impl Record for Note {
        const ENTITY: &'static str = "note";
        type Patch = NotePatch;
    }

    #[test]
    fn a_change_knows_its_record_and_fields() {
        let id = test_id(1);
        let update: Change<Note> = Change::Update {
            id,
            patch: NotePatch {
                body: Some("x".into()),
                ..NotePatch::default()
            },
        };
        assert_eq!(update.record_id(), id);
        assert_eq!(update.fields(), vec!["body"]);
        assert!(Change::<Note>::Delete { id }.fields().is_empty());
    }

    #[test]
    fn keep_only_drops_fields_that_lost() {
        let mut patch = NotePatch {
            title: Some("t".into()),
            body: Some("b".into()),
        };
        patch.keep_only(&["body"]);
        assert_eq!(patch.changed_fields(), vec!["body"]);
    }

    #[test]
    fn commands_round_trip_through_json_with_only_the_changed_fields() {
        let command: Command<Note> = Command {
            id: test_id(1),
            stamp: Stamp {
                hlc: Hlc::new(UnixMillis::from_millis(1_700_000_000_000), 0).unwrap(),
                device: test_id(2),
            },
            format_version: COMMAND_FORMAT_VERSION,
            change: Change::Update {
                id: test_id(3),
                patch: NotePatch {
                    title: Some("Oil change".into()),
                    body: None,
                },
            },
        };

        let json = serde_json::to_string(&command).unwrap();
        assert!(json.contains(r#""change":"update""#), "{json}");
        assert!(
            !json.contains("body"),
            "unchanged fields aren't sent: {json}"
        );

        let back: Command<Note> = serde_json::from_str(&json).unwrap();
        assert_eq!(back.change.fields(), vec!["title"]);
        assert_eq!(back.stamp, command.stamp);
    }
}
