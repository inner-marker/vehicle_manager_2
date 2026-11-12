//! Pulling the server's records to a desktop's copy (phase-3.md 4b, decision #075; plan §6).
//!
//! The server numbers every accepted change (`sequence`). A desktop keeps a **cursor** (the
//! highest number it has) and asks for the next **page**: the rows of every synced table
//! whose number is in the window after the cursor, as they are now (not the commands that
//! made them), with their field clocks and the tombstones in that window. Rows travel as
//! plain column values ([`SqlValue`]), because the desktop's copy has the same schema (the
//! same migrations) and only a server with the desktop's schema version is pulled from.

use serde::{Deserialize, Serialize};

use crate::id::DatabaseId;

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
    /// Not signed in.
    NotSignedIn,
    /// The server couldn't answer.
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
