//! A maintenance or repair event's work (milestone 7): the `work_details`, `event_tasks`,
//! `task_parts`, and `squawk_dispositions` tables, and its end readings (the `end` set in
//! `event_readings`). Written whole whenever the work changes, like the fuel details.

use std::collections::HashMap;

use domain::event::{Event, EventId, Part, Reading, ReadingSet, Reason, Task, WorkDetails};
use domain::label::Label;
use domain::notes::Notes;
use domain::preferences::CurrencyCode;
use domain::squawk::{Disposition, SquawkDisposition};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use super::{Scope, reason_text, write_readings};
use crate::StorageError;
use crate::convert::{id_from_sql, id_to_sql, invalid, name, required_name};

/// Replaces event `id`'s work (and its end readings) with `work`, or removes it.
pub(super) async fn write_work(
    connection: &mut SqliteConnection,
    id: EventId,
    work: Option<&WorkDetails>,
) -> Result<(), StorageError> {
    // Children before parents, for the foreign keys.
    for sql in [
        "DELETE FROM task_parts WHERE event_id = ?",
        "DELETE FROM event_tasks WHERE event_id = ?",
        "DELETE FROM squawk_dispositions WHERE event_id = ?",
        "DELETE FROM work_details WHERE event_id = ?",
    ] {
        sqlx::query(sql)
            .bind(id_to_sql(id))
            .execute(&mut *connection)
            .await?;
    }
    let end_readings = work.map_or(&[][..], |work| &work.end_readings[..]);
    write_readings(connection, id, ReadingSet::End, end_readings).await?;
    let Some(work) = work else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO work_details (event_id, currency, complaint, cause) VALUES (?, ?, ?, ?)",
    )
    .bind(id_to_sql(id))
    .bind(work.currency.as_str().to_owned())
    .bind(
        work.complaint
            .as_ref()
            .map(|notes| notes.as_str().to_owned()),
    )
    .bind(work.cause.as_ref().map(|notes| notes.as_str().to_owned()))
    .execute(&mut *connection)
    .await?;
    for (position, task) in (0_i64..).zip(&work.tasks) {
        write_task(connection, id, position, task).await?;
    }
    for (position, disposition) in (0_i64..).zip(&work.dispositions) {
        write_disposition(connection, id, position, disposition).await?;
    }
    Ok(())
}

async fn write_task(
    connection: &mut SqliteConnection,
    id: EventId,
    position: i64,
    task: &Task,
) -> Result<(), StorageError> {
    sqlx::query(
        "INSERT INTO event_tasks (event_id, position, title, requirement_id, notes,
             labor_seconds, labor_cost_minor, resets_from, next_due)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id_to_sql(id))
    .bind(position)
    .bind(task.title.as_str().to_owned())
    .bind(task.requirement.map(id_to_sql))
    .bind(task.notes.as_str().to_owned())
    .bind(task.labor_seconds)
    .bind(task.labor_cost_minor)
    .bind(name(&task.resets_from, "event_tasks.resets_from")?)
    .bind(task.next_due.clone())
    .execute(&mut *connection)
    .await?;
    for (part_position, part) in (0_i64..).zip(&task.parts) {
        sqlx::query(
            "INSERT INTO task_parts (event_id, task_position, position, description,
                 part_number, quantity_milli, unit_price_minor)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(position)
        .bind(part_position)
        .bind(part.description.clone())
        .bind(part.part_number.clone())
        .bind(part.quantity_milli)
        .bind(part.unit_price_minor)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn write_disposition(
    connection: &mut SqliteConnection,
    id: EventId,
    position: i64,
    disposition: &SquawkDisposition,
) -> Result<(), StorageError> {
    let (notes, task, reason) = match &disposition.action {
        Disposition::SignedOff { notes, task } => {
            (notes.as_str().to_owned(), task.map(i64::from), None)
        }
        Disposition::Deferred { reason } => (String::new(), None, reason_text(Some(reason))),
    };
    sqlx::query(
        "INSERT INTO squawk_dispositions (event_id, squawk_id, position, action, notes,
             task_position, reason, severity)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id_to_sql(id))
    .bind(id_to_sql(disposition.squawk))
    .bind(position)
    .bind(match disposition.action {
        Disposition::SignedOff { .. } => "signed-off",
        Disposition::Deferred { .. } => "deferred",
    })
    .bind(notes)
    .bind(task)
    .bind(reason)
    .bind(
        disposition
            .severity
            .map(|severity| name(&severity, "squawk_dispositions.severity"))
            .transpose()?,
    )
    .execute(connection)
    .await?;
    Ok(())
}

// --- Reading work back -------------------------------------------------------------------

fn notes_from(text: Option<String>, column: &'static str) -> Result<Option<Notes>, StorageError> {
    text.map(|text| Notes::parse(&text).map_err(invalid(column)))
        .transpose()
}

fn task_from_row(row: &SqliteRow) -> Result<Task, StorageError> {
    let title: String = row.try_get("title")?;
    let notes: String = row.try_get("notes")?;
    Ok(Task {
        title: Label::parse(&title).map_err(invalid("event_tasks.title"))?,
        requirement: super::optional_id(row, "requirement_id")?,
        notes: Notes::parse(&notes).map_err(invalid("event_tasks.notes"))?,
        parts: Vec::new(),
        labor_seconds: row.try_get("labor_seconds")?,
        labor_cost_minor: row.try_get("labor_cost_minor")?,
        resets_from: required_name(row.try_get("resets_from")?, "event_tasks.resets_from")?,
        next_due: row.try_get("next_due")?,
    })
}

fn part_from_row(row: &SqliteRow) -> Result<Part, StorageError> {
    Ok(Part {
        description: row.try_get("description")?,
        part_number: row.try_get("part_number")?,
        quantity_milli: row.try_get("quantity_milli")?,
        unit_price_minor: row.try_get("unit_price_minor")?,
    })
}

fn disposition_from_row(row: &SqliteRow) -> Result<SquawkDisposition, StorageError> {
    let action: String = row.try_get("action")?;
    let action = match action.as_str() {
        "signed-off" => {
            let notes: String = row.try_get("notes")?;
            let task = row
                .try_get::<Option<i64>, _>("task_position")?
                .map(|task| {
                    u16::try_from(task).map_err(invalid("squawk_dispositions.task_position"))
                })
                .transpose()?;
            Disposition::SignedOff {
                notes: Notes::parse(&notes).map_err(invalid("squawk_dispositions.notes"))?,
                task,
            }
        }
        "deferred" => {
            let reason: String = row.try_get("reason")?;
            Disposition::Deferred {
                reason: Reason::parse(&reason).map_err(invalid("squawk_dispositions.reason"))?,
            }
        }
        _ => {
            return Err(StorageError::InvalidValue {
                column: "squawk_dispositions.action",
            });
        }
    };
    let severity: Option<String> = row.try_get("severity")?;
    Ok(SquawkDisposition {
        squawk: id_from_sql(
            row.try_get::<&[u8], _>("squawk_id")?,
            "squawk_dispositions.squawk_id",
        )?,
        action,
        severity: severity
            .map(|severity| required_name(severity, "squawk_dispositions.severity"))
            .transpose()?,
    })
}

/// The work of `events[position]`, which must already have its work details.
fn work_mut<'e>(
    events: &'e mut [(EventId, Event)],
    position: usize,
    column: &'static str,
) -> Result<&'e mut WorkDetails, StorageError> {
    events[position]
        .1
        .work
        .as_mut()
        .ok_or(StorageError::InvalidValue { column })
}

/// Adds each maintenance or repair event's work (all but its end readings, which come with the
/// other readings), read in four queries. `index` finds an event's place in `events`.
pub(super) async fn add_work(
    connection: &mut SqliteConnection,
    scope: Scope,
    events: &mut [(EventId, Event)],
    index: &HashMap<EventId, usize>,
) -> Result<(), StorageError> {
    let (details_sql, tasks_sql, parts_sql, dispositions_sql) = match scope {
        Scope::Event(_) => (
            "SELECT * FROM work_details WHERE event_id = ?",
            "SELECT * FROM event_tasks WHERE event_id = ? ORDER BY position",
            "SELECT * FROM task_parts WHERE event_id = ? ORDER BY task_position, position",
            "SELECT * FROM squawk_dispositions WHERE event_id = ? ORDER BY position",
        ),
        Scope::Asset(_) => (
            "SELECT w.* FROM work_details w
             JOIN events e ON e.id = w.event_id WHERE e.asset_id = ? AND e.deleted = 0",
            "SELECT t.* FROM event_tasks t
             JOIN events e ON e.id = t.event_id WHERE e.asset_id = ? AND e.deleted = 0
             ORDER BY t.event_id, t.position",
            "SELECT p.* FROM task_parts p
             JOIN events e ON e.id = p.event_id WHERE e.asset_id = ? AND e.deleted = 0
             ORDER BY p.event_id, p.task_position, p.position",
            "SELECT d.* FROM squawk_dispositions d
             JOIN events e ON e.id = d.event_id WHERE e.asset_id = ? AND e.deleted = 0
             ORDER BY d.event_id, d.position",
        ),
    };
    let event_of = |row: &SqliteRow| -> Result<Option<usize>, StorageError> {
        let id: EventId = id_from_sql(row.try_get::<&[u8], _>("event_id")?, "event_id")?;
        Ok(index.get(&id).copied())
    };
    let rows = |sql| sqlx::query(sql).bind(scope.key());
    for row in rows(details_sql).fetch_all(&mut *connection).await? {
        if let Some(position) = event_of(&row)? {
            let currency: String = row.try_get("currency")?;
            events[position].1.work = Some(WorkDetails {
                tasks: Vec::new(),
                end_readings: Vec::new(),
                dispositions: Vec::new(),
                currency: CurrencyCode::parse(&currency)
                    .map_err(invalid("work_details.currency"))?,
                complaint: notes_from(row.try_get("complaint")?, "work_details.complaint")?,
                cause: notes_from(row.try_get("cause")?, "work_details.cause")?,
            });
        }
    }
    for row in rows(tasks_sql).fetch_all(&mut *connection).await? {
        if let Some(position) = event_of(&row)? {
            let task = task_from_row(&row)?;
            work_mut(events, position, "event_tasks.event_id")?
                .tasks
                .push(task);
        }
    }
    for row in rows(parts_sql).fetch_all(&mut *connection).await? {
        if let Some(position) = event_of(&row)? {
            let task = usize::try_from(row.try_get::<i64, _>("task_position")?)
                .map_err(invalid("task_parts.task_position"))?;
            let part = part_from_row(&row)?;
            work_mut(events, position, "task_parts.event_id")?
                .tasks
                .get_mut(task)
                .ok_or(StorageError::InvalidValue {
                    column: "task_parts.task_position",
                })?
                .parts
                .push(part);
        }
    }
    for row in rows(dispositions_sql).fetch_all(&mut *connection).await? {
        if let Some(position) = event_of(&row)? {
            let disposition = disposition_from_row(&row)?;
            work_mut(events, position, "squawk_dispositions.event_id")?
                .dispositions
                .push(disposition);
        }
    }
    Ok(())
}

/// Puts one row of `event_readings` on its event: a start reading on the event itself, an end
/// reading on its work.
pub(super) fn add_reading(
    events: &mut [(EventId, Event)],
    position: usize,
    set: ReadingSet,
    reading: Reading,
) -> Result<(), StorageError> {
    match set {
        ReadingSet::Start => events[position].1.readings.push(reading),
        ReadingSet::End => work_mut(events, position, "event_readings.reading_set")?
            .end_readings
            .push(reading),
    }
    Ok(())
}
