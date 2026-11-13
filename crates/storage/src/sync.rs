//! Pull pages (phase-3.md 4b, decision #075): read on the server, applied to a desktop's copy.
//!
//! **What's synced:** the record tables ([`RECORD_TABLES`], each with a `sequence`), the
//! children of the events in the page ([`EVENT_CHILD_TABLES`], rewritten whole with their
//! event), the pulled records' `field_clocks`, and the page's `tombstones`. Never the
//! server-only tables (passwords, sessions, devices, jobs, applied commands, stored files),
//! and never another person's private rows ([`PullScope`]).
//!
//! **Reading** happens in one transaction, so a page is one moment's state. **Applying**
//! writes the rows as they are (the copy has the same schema), replaces the children of each
//! event in the page, and moves the cursor, all in one transaction. Foreign keys aren't
//! checked while applying: a page can hold a row whose parent comes in a later page (a parent
//! changed after its child), and the server has already checked them.
//!
//! **SQL built at run time** (`AssertSqlSafe`) names only tables from the fixed lists here, and
//! columns checked against the copy's own schema; every value is bound.

use domain::id::DatabaseId;
use domain::log_area;
use domain::sync::{PullPage, SqlValue, TableRows};
use domain::time::UnixMillis;
use domain::user::UserId;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{AssertSqlSafe, Column, Row, SqliteConnection, TypeInfo, ValueRef};

use crate::StorageError;
use crate::convert::{id_from_sql, id_to_sql};
use crate::database::{Database, latest_schema_version};

/// The tables of records, each with its own `sequence` and `deleted`.
pub const RECORD_TABLES: [&str; 18] = [
    "users",
    "role_grants",
    "preferences",
    "message_marks",
    "groups",
    "assets",
    "meters",
    "custom_fields",
    "garage_orders",
    "saved_places",
    "events",
    "squawks",
    "requirements",
    "bundles",
    "license_plates",
    "asset_documents",
    "attachments",
    "reference_documents",
];

/// An event's parts, keyed by `event_id` and rewritten whole when the event changes.
pub const EVENT_CHILD_TABLES: [&str; 8] = [
    "event_readings",
    "event_field_values",
    "fuel_details",
    "work_details",
    "lifecycle_details",
    "event_tasks",
    "task_parts",
    "squawk_dispositions",
];

/// The per-record field clocks (#043), pulled with their records.
const FIELD_CLOCKS: &str = "field_clocks";

/// Deletes (#072), each with its own `sequence`.
const TOMBSTONES: &str = "tombstones";

/// The most IDs bound in one `IN (…)` list.
const IN_CHUNK: usize = 400;

/// This build's schema version (the newest migration it has).
#[must_use]
pub fn schema_version() -> i64 {
    latest_schema_version()
}

/// Whose copy a pull is for: their own private rows come, other people's don't.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PullScope {
    /// The person pulling.
    pub user: UserId,
    /// The household preferences record's ID (the database ID).
    pub household: DatabaseId,
}

/// The extra condition (and its values) that keeps a table's private rows to their owner.
fn private_filter(table: &str, scope: &PullScope) -> (&'static str, Vec<Vec<u8>>) {
    let user = id_to_sql(scope.user);
    match table {
        // The household's record (ID: the database ID) and the person's own (ID: theirs).
        "preferences" => (" AND id IN (?, ?)", vec![user, id_to_sql(scope.household)]),
        "garage_orders" => (" AND id = ?", vec![user]),
        "saved_places" => (" AND owner_id = ?", vec![user]),
        "message_marks" => (" AND user_id = ?", vec![user]),
        _ => ("", Vec::new()),
    }
}

/// A row's values, as SQLite holds them.
fn values_of(row: &SqliteRow) -> Result<Vec<SqlValue>, StorageError> {
    (0..row.columns().len())
        .map(|index| {
            let raw = row.try_get_raw(index)?;
            if raw.is_null() {
                return Ok(SqlValue::Null);
            }
            let kind = raw.type_info().name().to_owned();
            Ok(match kind.as_str() {
                "INTEGER" => SqlValue::Integer(row.try_get(index)?),
                "REAL" => SqlValue::Real(row.try_get(index)?),
                "TEXT" => SqlValue::Text(row.try_get(index)?),
                _ => SqlValue::blob(&row.try_get::<Vec<u8>, _>(index)?),
            })
        })
        .collect()
}

/// `rows` as one table's [`TableRows`], or `None` if there are none.
fn table_rows(table: &str, rows: &[SqliteRow]) -> Result<Option<TableRows>, StorageError> {
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    Ok(Some(TableRows {
        table: table.to_owned(),
        columns: first
            .columns()
            .iter()
            .map(|column| column.name().to_owned())
            .collect(),
        rows: rows.iter().map(values_of).collect::<Result<_, _>>()?,
    }))
}

/// `?, ?, …` for `count` values.
fn placeholders(count: usize) -> String {
    vec!["?"; count].join(", ")
}

/// Where a pull stands on a desktop's copy.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PullState {
    /// The server's database ID (the copy's records are keyed by it, e.g. the household
    /// preferences).
    pub database_id: DatabaseId,
    /// The highest change number the copy has.
    pub cursor: u64,
    /// The server's newest change number at the last pull.
    pub server_sequence: u64,
    /// When the last pull finished, if one has.
    pub last_pull: Option<UnixMillis>,
}

impl PullState {
    /// Whether the copy was complete at its last pull (the first download has finished, and
    /// no page was left): only then do screens read it (phase-3.md 4d).
    #[must_use]
    pub const fn complete(&self) -> bool {
        self.last_pull.is_some() && self.cursor >= self.server_sequence
    }
}

/// A page read on the server, before the server adds its identity.
#[derive(Debug, Clone, PartialEq)]
pub struct ReadPage {
    /// The cursor to ask from next.
    pub next_cursor: u64,
    /// The server's newest change number.
    pub server_sequence: u64,
    /// The rows.
    pub tables: Vec<TableRows>,
}

fn to_u64(value: i64, column: &'static str) -> Result<u64, StorageError> {
    u64::try_from(value).map_err(|_| StorageError::InvalidValue { column })
}

fn to_i64(value: u64, column: &'static str) -> Result<i64, StorageError> {
    i64::try_from(value).map_err(|_| StorageError::InvalidValue { column })
}

impl Database {
    /// The page after `cursor` for `scope`: rows changed in the next `window` change numbers
    /// (fewer if the server has fewer), read in one transaction.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails.
    pub async fn read_pull_page(
        &self,
        scope: &PullScope,
        cursor: u64,
        window: u64,
    ) -> Result<ReadPage, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let newest: i64 = sqlx::query_scalar("SELECT last_sequence FROM database_meta")
            .fetch_one(&mut *transaction)
            .await?;
        let server_sequence = to_u64(newest, "last_sequence")?;
        let end = cursor.saturating_add(window).min(server_sequence);
        let mut tables = Vec::new();
        if end <= cursor {
            return Ok(ReadPage {
                next_cursor: cursor,
                server_sequence,
                tables,
            });
        }
        let (from, to) = (to_i64(cursor, "cursor")?, to_i64(end, "cursor")?);
        let mut pulled_ids: Vec<Vec<u8>> = Vec::new();
        for table in RECORD_TABLES {
            let (filter, values) = private_filter(table, scope);
            let sql = format!(
                "SELECT * FROM {table} WHERE sequence > ? AND sequence <= ?{filter} ORDER BY sequence"
            );
            let mut query = sqlx::query(AssertSqlSafe(sql.as_str())).bind(from).bind(to);
            for value in values {
                query = query.bind(value);
            }
            let rows = query.fetch_all(&mut *transaction).await?;
            for row in &rows {
                pulled_ids.push(row.try_get::<Vec<u8>, _>("id")?);
            }
            tables.extend(table_rows(table, &rows)?);
        }
        for table in EVENT_CHILD_TABLES {
            let sql = format!(
                "SELECT * FROM {table} WHERE event_id IN
                     (SELECT id FROM events WHERE sequence > ? AND sequence <= ?)"
            );
            let rows = sqlx::query(AssertSqlSafe(sql.as_str()))
                .bind(from)
                .bind(to)
                .fetch_all(&mut *transaction)
                .await?;
            tables.extend(table_rows(table, &rows)?);
        }
        let mut clocks = Vec::new();
        for chunk in pulled_ids.chunks(IN_CHUNK) {
            let sql = format!(
                "SELECT * FROM {FIELD_CLOCKS} WHERE record_id IN ({})",
                placeholders(chunk.len())
            );
            let mut query = sqlx::query(AssertSqlSafe(sql.as_str()));
            for id in chunk {
                query = query.bind(id.as_slice());
            }
            clocks.extend(query.fetch_all(&mut *transaction).await?);
        }
        tables.extend(table_rows(FIELD_CLOCKS, &clocks)?);
        let tombstones = sqlx::query(
            "SELECT * FROM tombstones WHERE sequence > ? AND sequence <= ? ORDER BY sequence",
        )
        .bind(from)
        .bind(to)
        .fetch_all(&mut *transaction)
        .await?;
        tables.extend(table_rows(TOMBSTONES, &tombstones)?);
        transaction.commit().await?;
        Ok(ReadPage {
            next_cursor: end,
            server_sequence,
            tables,
        })
    }

    /// How many records the synced tables hold (live and deleted), for Diagnostics.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails.
    pub async fn record_count(&self) -> Result<u64, StorageError> {
        let mut total: u64 = 0;
        for table in RECORD_TABLES {
            let count: i64 =
                sqlx::query_scalar(AssertSqlSafe(format!("SELECT COUNT(*) FROM {table}")))
                    .fetch_one(self.pool())
                    .await?;
            total = total.saturating_add(to_u64(count, "count")?);
        }
        Ok(total)
    }

    /// Where the copy's pull stands, or `None` if this isn't a copy.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn pull_state(&self) -> Result<Option<PullState>, StorageError> {
        let Some(row) = sqlx::query(
            "SELECT server_database_id, cursor, server_sequence, last_pull_ms
             FROM pull_state WHERE id = 1",
        )
        .fetch_optional(self.pool())
        .await?
        else {
            return Ok(None);
        };
        Ok(Some(PullState {
            database_id: id_from_sql(
                row.try_get::<&[u8], _>("server_database_id")?,
                "server_database_id",
            )?,
            cursor: to_u64(row.try_get("cursor")?, "cursor")?,
            server_sequence: to_u64(row.try_get("server_sequence")?, "server_sequence")?,
            last_pull: row
                .try_get::<Option<i64>, _>("last_pull_ms")?
                .map(UnixMillis::from_millis),
        }))
    }

    /// Applies `page` to this copy and moves its cursor, in one transaction.
    ///
    /// # Errors
    ///
    /// - [`StorageError::OtherServer`] if the page is from another server, or this isn't a copy.
    /// - [`StorageError::OutOfStep`] if the page doesn't start at the copy's cursor.
    /// - [`StorageError::InvalidValue`] if it names a table that isn't synced or a column the
    ///   copy doesn't have, or a row's values don't match its columns.
    /// - [`StorageError::Sqlx`] if a write fails (nothing is applied).
    pub async fn apply_pull_page(
        &self,
        page: &PullPage,
        now: UnixMillis,
    ) -> Result<(), StorageError> {
        let mut connection = self.pool().acquire().await?;
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await?;
        let applied = apply_on(&mut connection, page, now).await;
        if foreign_keys != 0 {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *connection)
                .await?;
        }
        applied?;
        tracing::debug!(
            target: log_area::SYNC,
            cursor = page.next_cursor,
            server_sequence = page.server_sequence,
            rows = page.row_count(),
            "pulled a page"
        );
        Ok(())
    }
}

impl Database {
    /// Empties this copy of everything pulled (every synced table, field clocks, tombstones)
    /// and starts its pull again from the beginning, keeping its outbox and clock (phase-3.md
    /// 5d, decision #076): after the server refuses a change the copy already shows, so it
    /// downloads the server's records again. Until that finishes the copy isn't complete
    /// ([`PullState::complete`]), so screens read from the server.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails (nothing is changed then).
    pub async fn forget_pulled(&self) -> Result<(), StorageError> {
        let mut connection = self.pool().acquire().await?;
        let foreign_keys: i64 = sqlx::query_scalar("PRAGMA foreign_keys")
            .fetch_one(&mut *connection)
            .await?;
        sqlx::query("PRAGMA foreign_keys = OFF")
            .execute(&mut *connection)
            .await?;
        let forgotten = forget_on(&mut connection).await;
        if foreign_keys != 0 {
            sqlx::query("PRAGMA foreign_keys = ON")
                .execute(&mut *connection)
                .await?;
        }
        forgotten?;
        tracing::info!(target: log_area::SYNC, "the copy's records were cleared to download again");
        Ok(())
    }
}

/// The work of [`Database::forget_pulled`], on one connection with foreign keys off.
async fn forget_on(connection: &mut SqliteConnection) -> Result<(), StorageError> {
    let mut transaction = sqlx::Connection::begin(connection).await?;
    for table in EVENT_CHILD_TABLES
        .iter()
        .chain(RECORD_TABLES.iter())
        .chain([FIELD_CLOCKS, TOMBSTONES].iter())
    {
        // The names are this module's own constants.
        sqlx::query(AssertSqlSafe(format!("DELETE FROM {table}")))
            .execute(&mut *transaction)
            .await?;
    }
    sqlx::query(
        "UPDATE pull_state SET cursor = 0, server_sequence = 0, last_pull_ms = NULL WHERE id = 1",
    )
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// The work of [`Database::apply_pull_page`], on one connection with foreign keys off.
async fn apply_on(
    connection: &mut SqliteConnection,
    page: &PullPage,
    now: UnixMillis,
) -> Result<(), StorageError> {
    let mut transaction = sqlx::Connection::begin(connection).await?;
    let Some(row) = sqlx::query("SELECT server_database_id, cursor FROM pull_state WHERE id = 1")
        .fetch_optional(&mut *transaction)
        .await?
    else {
        return Err(StorageError::OtherServer);
    };
    let server: DatabaseId = id_from_sql(
        row.try_get::<&[u8], _>("server_database_id")?,
        "server_database_id",
    )?;
    if server != page.database_id {
        return Err(StorageError::OtherServer);
    }
    if to_u64(row.try_get("cursor")?, "cursor")? != page.cursor {
        return Err(StorageError::OutOfStep);
    }

    // An event's children are rewritten whole: first remove the ones the copy has.
    if let Some(events) = page.tables.iter().find(|table| table.table == "events") {
        let id_index = column_index(events, "id")?;
        for row in &events.rows {
            let id = row
                .get(id_index)
                .and_then(SqlValue::blob_bytes)
                .ok_or(StorageError::InvalidValue { column: "id" })?;
            for child in EVENT_CHILD_TABLES {
                sqlx::query(AssertSqlSafe(format!(
                    "DELETE FROM {child} WHERE event_id = ?"
                )))
                .bind(id.as_slice())
                .execute(&mut *transaction)
                .await?;
            }
        }
    }
    for table in &page.tables {
        let name = table.table.as_str();
        if !(RECORD_TABLES.contains(&name)
            || EVENT_CHILD_TABLES.contains(&name)
            || name == FIELD_CLOCKS
            || name == TOMBSTONES)
        {
            return Err(StorageError::InvalidValue { column: "table" });
        }
        let known = local_columns(&mut transaction, name).await?;
        if table.columns.is_empty() || table.columns.iter().any(|column| !known.contains(column)) {
            return Err(StorageError::InvalidValue { column: "column" });
        }
        // The names are checked against the copy's own schema above, so quoting is enough.
        let sql = format!(
            "INSERT OR REPLACE INTO {name} ({}) VALUES ({})",
            table
                .columns
                .iter()
                .map(|column| format!("\"{column}\""))
                .collect::<Vec<_>>()
                .join(", "),
            placeholders(table.columns.len())
        );
        for row in &table.rows {
            if row.len() != table.columns.len() {
                return Err(StorageError::InvalidValue { column: "row" });
            }
            bound(sqlx::query(AssertSqlSafe(sql.as_str())), row)?
                .execute(&mut *transaction)
                .await?;
        }
    }
    sqlx::query(
        "UPDATE pull_state SET cursor = ?, server_sequence = ?, last_pull_ms = ? WHERE id = 1",
    )
    .bind(to_i64(page.next_cursor, "cursor")?)
    .bind(to_i64(page.server_sequence, "server_sequence")?)
    .bind(now.as_millis())
    .execute(&mut *transaction)
    .await?;
    transaction.commit().await?;
    Ok(())
}

/// The position of `column` in `table`'s columns.
fn column_index(table: &TableRows, column: &'static str) -> Result<usize, StorageError> {
    table
        .columns
        .iter()
        .position(|name| name == column)
        .ok_or(StorageError::InvalidValue { column })
}

/// The copy's own column names for `table`.
async fn local_columns(
    connection: &mut SqliteConnection,
    table: &str,
) -> Result<Vec<String>, StorageError> {
    Ok(sqlx::query_scalar("SELECT name FROM pragma_table_info(?)")
        .bind(table)
        .fetch_all(connection)
        .await?)
}

/// `query` with `row`'s values bound in order.
fn bound<'q>(
    mut query: sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments>,
    row: &'q [SqlValue],
) -> Result<sqlx::query::Query<'q, sqlx::Sqlite, SqliteArguments>, StorageError> {
    for value in row {
        query = match value {
            SqlValue::Null => query.bind(None::<i64>),
            SqlValue::Integer(number) => query.bind(*number),
            SqlValue::Real(number) => query.bind(*number),
            SqlValue::Text(text) => query.bind(text.as_str()),
            SqlValue::Blob(_) => query.bind(
                value
                    .blob_bytes()
                    .ok_or(StorageError::InvalidValue { column: "blob" })?,
            ),
        };
    }
    Ok(query)
}
