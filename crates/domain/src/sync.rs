//! Pulling the server's records to a desktop's copy (phase-3.md 4b, decision #075; plan §6).
//!
//! The server numbers every accepted change (`sequence`). A desktop keeps a **cursor** (the
//! highest number it has) and asks for the next **page**: the rows of every synced table
//! whose number is in the window after the cursor, as they are now (not the commands that
//! made them), with their field clocks and the tombstones in that window. Rows travel as
//! plain column values ([`SqlValue`]), because the desktop's copy has the same schema (the
//! same migrations) and only a server with the desktop's schema version is pulled from.
//!
//! **Pushing** (phase-3.md 5c, decision #076) goes the other way: a desktop sends the
//! operations in its outbox, oldest first ([`PUSH_LIMIT`] at a time), and the server answers
//! each ([`OperationAnswer`]) until it has to stop ([`PushStop`]); the rest wait for the next
//! push.

use serde::{Deserialize, Serialize};

use crate::id::DatabaseId;
use crate::operation::{OperationId, Refusal};

/// How many change numbers one page covers. A page never splits a change number.
pub const PULL_WINDOW: u64 = 500;

/// One column's value, as SQLite stores it. A blob travels as lowercase hex.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum SqlValue {
    /// `NULL`.
    #[serde(rename = "n")]
    Null,
    /// An integer.
    #[serde(rename = "i")]
    Integer(i64),
    /// A floating-point number.
    #[serde(rename = "r")]
    Real(f64),
    /// Text.
    #[serde(rename = "t")]
    Text(String),
    /// A blob, as lowercase hex.
    #[serde(rename = "b")]
    Blob(String),
}

impl SqlValue {
    /// A blob value from its bytes.
    #[must_use]
    pub fn blob(bytes: &[u8]) -> Self {
        use std::fmt::Write;
        Self::Blob(bytes.iter().fold(String::new(), |mut text, byte| {
            let _ = write!(text, "{byte:02x}");
            text
        }))
    }

    /// A blob's bytes, or `None` if this isn't a blob (or its hex is malformed).
    #[must_use]
    pub fn blob_bytes(&self) -> Option<Vec<u8>> {
        let Self::Blob(hex) = self else {
            return None;
        };
        if !hex.len().is_multiple_of(2) || !hex.is_ascii() {
            return None;
        }
        (0..hex.len())
            .step_by(2)
            .map(|i| u8::from_str_radix(&hex[i..i + 2], 16).ok())
            .collect()
    }
}

/// Some rows of one table: its column names, and each row's values in that order.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct TableRows {
    /// The table's name.
    pub table: String,
    /// The columns, in the order of each row's values.
    pub columns: Vec<String>,
    /// The rows.
    pub rows: Vec<Vec<SqlValue>>,
}

/// One page of changes, from `cursor` (exclusive) to `next_cursor` (inclusive).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct PullPage {
    /// The server's database ID (the copy must be of this server).
    pub database_id: DatabaseId,
    /// The server's schema version (the same as the desktop's, or there's no page).
    pub schema: i64,
    /// The cursor this page starts after.
    pub cursor: u64,
    /// The cursor to ask from next.
    pub next_cursor: u64,
    /// The server's newest change number now; the copy is up to date once its cursor
    /// reaches it.
    pub server_sequence: u64,
    /// The changed rows, table by table (tables with none are left out).
    pub tables: Vec<TableRows>,
}

impl PullPage {
    /// Whether this page brings the copy up to date.
    #[must_use]
    pub const fn up_to_date(&self) -> bool {
        self.next_cursor >= self.server_sequence
    }

    /// How many rows the page carries.
    #[must_use]
    pub fn row_count(&self) -> usize {
        self.tables.iter().map(|table| table.rows.len()).sum()
    }
}

/// The most operations one push carries.
pub const PUSH_LIMIT: usize = 100;

/// The server's answer to one pushed operation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "answer", rename_all = "kebab-case")]
pub enum OperationAnswer {
    /// Applied (now, or when it was first sent): it leaves the outbox.
    Applied {
        /// Which operation.
        id: OperationId,
    },
    /// Refused, and why: it stays on the desktop for Messages (sketch 2A).
    Refused {
        /// Which operation.
        id: OperationId,
        /// The server function's answer.
        refusal: Refusal,
    },
}

impl OperationAnswer {
    /// The operation it answers.
    #[must_use]
    pub const fn id(&self) -> OperationId {
        match self {
            Self::Applied { id } | Self::Refused { id, .. } => *id,
        }
    }
}

/// Why the server stopped before the end of a push. The operation it stopped at, and the rest,
/// stay in the outbox.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "stopped", rename_all = "kebab-case")]
pub enum PushStop {
    /// Written by a newer app than the server knows: the server needs updating.
    ServerTooOld {
        /// The operation's format.
        format: u32,
    },
    /// Written by an app older than the server accepts (#072): the app needs updating. Its
    /// outbox is kept.
    AppTooOld {
        /// The operation's format.
        format: u32,
        /// The oldest the server accepts.
        oldest: u32,
    },
    /// The desktop's clock is far ahead of the server's: set it right (the Admin is told).
    ClockAhead {
        /// How far, in milliseconds.
        ahead_millis: i64,
    },
    /// The server can't apply changes now (unhealthy, or a failure): try again later.
    Unavailable,
}

/// The answer to a push.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum PushOutcome {
    /// An answer for each operation, in order, until the server stopped (if it did).
    Answers {
        /// One per operation answered.
        answers: Vec<OperationAnswer>,
        /// Why it stopped early, if it did.
        stopped: Option<PushStop>,
    },
    /// The server's schema version differs from the desktop's (as for a pull).
    OtherSchema {
        /// The server's schema version.
        server: i64,
    },
    /// Only a paired desktop pushes.
    NotAllowed,
    /// The server isn't well (its database failed a check): it takes no changes until it is
    /// (7b); the desktop waits.
    Unhealthy,
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't answer.
    Unavailable,
}

/// The answer to a pull.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum PullOutcome {
    /// The next page.
    Page {
        /// It.
        page: Box<PullPage>,
    },
    /// The server's schema version differs from the desktop's: no copy until one is updated
    /// (#075).
    OtherSchema {
        /// The server's schema version.
        server: i64,
    },
    /// Only a paired desktop pulls (a browser never keeps a copy, #071).
    NotAllowed,
    /// The server isn't well (its database failed a check): it serves no pages until it is
    /// (7b); the desktop waits.
    Unhealthy,
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't answer.
    Unavailable,
}

// --- Rebuild (phase-3.md 7b, decision #078) -------------------------------------------------

/// A person's new temporary password after a rebuild, for the Admin to give them (shown once).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TemporaryPassword {
    /// Their username.
    pub username: String,
    /// Their name.
    pub display_name: String,
    /// The password, typed exactly as shown.
    pub password: crate::secret::Secret,
}

/// The answer to a step of rebuilding a fresh server from a desktop's copy.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "outcome", rename_all = "kebab-case")]
pub enum RebuildOutcome {
    /// Started: send the pages with this token.
    Started {
        /// The rebuild's token (until it finishes, or the server restarts).
        token: crate::secret::Secret,
    },
    /// The page is in.
    PageTaken,
    /// The records are in: everyone's new temporary password (the Admin keeps their own).
    Finished {
        /// One for each other person.
        passwords: Vec<TemporaryPassword>,
    },
    /// The files are settled: the attachments whose files never came, by name, left out.
    Settled {
        /// Their names.
        left_out: Vec<String>,
    },
    /// The server isn't fresh (it has people or vehicles): only a fresh one is rebuilt.
    NotFresh,
    /// The setup code is wrong.
    WrongCode,
    /// Too many wrong codes; try again in this many seconds.
    TooManyAttempts {
        /// Seconds to wait.
        retry_after_seconds: i64,
    },
    /// No rebuild is going on with this token, the page is from another server than the
    /// first, or the person isn't the household's Admin.
    NotAllowed,
    /// A page doesn't fit this server's schema, or the Admin's password breaks a rule.
    Invalid,
    /// The server couldn't (unhealthy, or a failure).
    Unavailable,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn blobs_travel_as_hex_and_values_are_short_on_the_wire() {
        let blob = SqlValue::blob(&[0x01, 0xab, 0xff]);
        assert_eq!(blob, SqlValue::Blob("01abff".into()));
        assert_eq!(blob.blob_bytes(), Some(vec![0x01, 0xab, 0xff]));
        assert_eq!(SqlValue::Blob("0g".into()).blob_bytes(), None);
        assert_eq!(SqlValue::Integer(1).blob_bytes(), None);
        let json = serde_json::to_string(&vec![
            SqlValue::Null,
            SqlValue::Integer(7),
            SqlValue::Text("x".into()),
            blob,
        ])
        .unwrap();
        assert_eq!(json, r#"["n",{"i":7},{"t":"x"},{"b":"01abff"}]"#);
    }

    #[test]
    fn a_page_is_up_to_date_once_it_reaches_the_servers_newest_change() {
        let page = PullPage {
            database_id: crate::test_support::test_id(1),
            schema: 18,
            cursor: 0,
            next_cursor: 500,
            server_sequence: 900,
            tables: vec![],
        };
        assert!(!page.up_to_date());
        assert!(
            PullPage {
                next_cursor: 900,
                ..page
            }
            .up_to_date()
        );
    }
}
