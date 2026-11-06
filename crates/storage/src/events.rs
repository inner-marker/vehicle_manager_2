//! The `events`, `event_readings`, `event_field_values`, and `fuel_details` tables (milestone
//! 6), a maintenance or repair event's work (milestone 7), and a lifecycle event's
//! `lifecycle_details` (milestone 9): the event writer for
//! the command pipeline, and the queries the server needs.
//!
//! **A finalized event never changes** (#034, choice 9): the writer refuses any change to one,
//! and any delete, with [`StorageError::Finalized`], so nothing edits one even by mistake. A
//! change to a draft is applied to the stored event and the whole row written back; a list that
//! changed (readings, field values) and the fuel details and work are rewritten whole.

mod work;

use std::collections::HashMap;

use domain::asset::AssetId;
use domain::dates::EventDates;
use domain::event::{
    Event, EventId, EventLocation, EventPatch, EventStatus, FieldValue, Finalized, Finalizer,
    FuelDetails, Performer, Reading, ReadingSet, Reason,
};
use domain::id::Id;
use domain::label::Label;
use domain::lifecycle::{LifecycleDetails, LifecycleRecord};
use domain::location::{Coordinates, Place, TimeZone};
use domain::meter::MeterId;
use domain::notes::Notes;
use domain::preferences::CurrencyCode;
use domain::time::UnixMillis;
use sqlx::query::Query;
use sqlx::sqlite::{SqliteArguments, SqliteRow};
use sqlx::{Row, Sqlite, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{
    id_from_sql, id_to_sql, invalid, json_from_sql, json_to_sql, name, required_name,
};
use crate::{Database, StorageError};

/// Writes the `events` table and its parts.
#[derive(Debug, Clone, Copy, Default)]
pub struct EventWriter;

impl RecordWriter<Event> for EventWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Event>,
        values: &EventPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let event = values
            .clone()
            .into_event()
            .map_err(|column| StorageError::InvalidValue { column })?;
        let query = sqlx::query(
            "INSERT INTO events (asset_id, kind, start_ms, end_ms, zone, city, region,
                 country_code, lat_e7, lon_e7, accuracy_m, fix_ms, location_source, place_name,
                 saved_place_id, geocoder_version, no_place_reason, skipped, notes, source,
                 logged_by, performer_user_id, performer_name, performer_contact,
                 reading_override, status, last_edited_ms, received_ms, finalized_ms,
                 finalized_by, finalized_by_user, corrects_id, correction_reason, sequence, id)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?,
                     ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
        );
        bind_event(query, &event, sequence, id)?
            .execute(&mut *connection)
            .await?;
        write_readings(connection, id, ReadingSet::Start, &event.readings).await?;
        write_fields(connection, id, &event.fields).await?;
        write_fuel(connection, id, event.fuel.as_ref()).await?;
        work::write_work(connection, id, event.work.as_ref()).await?;
        if let Some(details) = &event.lifecycle {
            write_lifecycle(connection, id, details).await?;
        }
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Event>,
        patch: &EventPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let mut event = load(connection, id)
            .await?
            .ok_or(StorageError::InvalidValue {
                column: "events.id",
            })?;
        if event.status == EventStatus::Finalized {
            return Err(StorageError::Finalized);
        }
        event.apply(patch.clone());
        let query = sqlx::query(
            "UPDATE events SET asset_id = ?, kind = ?, start_ms = ?, end_ms = ?, zone = ?,
                 city = ?, region = ?, country_code = ?, lat_e7 = ?, lon_e7 = ?,
                 accuracy_m = ?, fix_ms = ?, location_source = ?, place_name = ?,
                 saved_place_id = ?, geocoder_version = ?, no_place_reason = ?, skipped = ?,
                 notes = ?, source = ?, logged_by = ?, performer_user_id = ?,
                 performer_name = ?, performer_contact = ?, reading_override = ?, status = ?,
                 last_edited_ms = ?, received_ms = ?, finalized_ms = ?, finalized_by = ?,
                 finalized_by_user = ?, corrects_id = ?, correction_reason = ?, sequence = ?
             WHERE id = ?",
        );
        bind_event(query, &event, sequence, id)?
            .execute(&mut *connection)
            .await?;
        if patch.readings.is_some() {
            write_readings(connection, id, ReadingSet::Start, &event.readings).await?;
        }
        if patch.fields.is_some() {
            write_fields(connection, id, &event.fields).await?;
        }
        if patch.fuel.is_some() {
            write_fuel(connection, id, event.fuel.as_ref()).await?;
        }
        if patch.work.is_some() {
            work::write_work(connection, id, event.work.as_ref()).await?;
        }
        if patch.lifecycle.is_some() {
            sqlx::query("DELETE FROM lifecycle_details WHERE event_id = ?")
                .bind(id_to_sql(id))
                .execute(&mut *connection)
                .await?;
            if let Some(details) = &event.lifecycle {
                write_lifecycle(connection, id, details).await?;
            }
        }
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<Event>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // Only a draft can go; its parts stay with it, as a deleted event's do.
        let status: Option<String> =
            sqlx::query_scalar("SELECT status FROM events WHERE id = ? AND deleted = 0")
                .bind(id_to_sql(id))
                .fetch_optional(&mut *connection)
                .await?;
        if status.as_deref() == Some("finalized") {
            return Err(StorageError::Finalized);
        }
        sqlx::query("UPDATE events SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn reason_text(reason: Option<&Reason>) -> Option<String> {
    reason.map(|reason| reason.as_str().to_owned())
}

/// Binds every column of `event` (in the order the INSERT and UPDATE list them), then
/// `sequence` and `id`.
fn bind_event<'q>(
    query: Query<'q, Sqlite, SqliteArguments>,
    event: &Event,
    sequence: i64,
    id: EventId,
) -> Result<Query<'q, Sqlite, SqliteArguments>, StorageError> {
    let location = &event.location;
    let place = location.place.as_ref();
    let (performer_user, performer_name, performer_contact) = match &event.performed_by {
        None => (None, None, None),
        Some(Performer::User { user }) => (Some(id_to_sql(*user)), None, None),
        Some(Performer::Named { name, contact }) => (None, Some(name.clone()), contact.clone()),
    };
    let (finalized_ms, finalized_by, finalized_by_user) = match event.finalized {
        None => (None, None, None),
        Some(Finalized { at, by }) => match by {
            Finalizer::User(user) => (Some(at.as_millis()), Some("user"), Some(id_to_sql(user))),
            Finalizer::Server => (Some(at.as_millis()), Some("server"), None),
        },
    };
    Ok(query
        .bind(id_to_sql(event.asset))
        .bind(name(&event.kind, "events.kind")?)
        .bind(event.dates.start.as_millis())
        .bind(event.dates.end.map(UnixMillis::as_millis))
        .bind(event.dates.zone.as_str().to_owned())
        .bind(place.map(|place| place.city.as_str().to_owned()))
        .bind(place.map(|place| place.region.as_str().to_owned()))
        .bind(place.map(|place| place.country.as_str().to_owned()))
        .bind(location.coordinates.map(Coordinates::lat_e7))
        .bind(location.coordinates.map(Coordinates::lon_e7))
        .bind(location.accuracy_m)
        .bind(location.fix_time.map(UnixMillis::as_millis))
        .bind(name(&location.source, "events.location_source")?)
        .bind(
            location
                .place_name
                .as_ref()
                .map(|name| name.as_str().to_owned()),
        )
        .bind(location.saved_place.map(id_to_sql))
        .bind(location.geocoder_version.clone())
        .bind(reason_text(location.missing_reason.as_ref()))
        .bind(json_to_sql(&event.skipped, "events.skipped")?)
        .bind(event.notes.as_str().to_owned())
        .bind(name(&event.source, "events.source")?)
        .bind(id_to_sql(event.logged_by))
        .bind(performer_user)
        .bind(performer_name)
        .bind(performer_contact)
        .bind(reason_text(event.reading_override.as_ref()))
        .bind(name(&event.status, "events.status")?)
        .bind(event.last_edited_at.as_millis())
        .bind(event.received_at.as_millis())
        .bind(finalized_ms)
        .bind(finalized_by)
        .bind(finalized_by_user)
        .bind(event.corrects.map(id_to_sql))
        .bind(reason_text(event.correction_reason.as_ref()))
        .bind(sequence)
        .bind(id_to_sql(id)))
}

/// Replaces one set of event `id`'s readings (the start ones, or a job's end ones).
async fn write_readings(
    connection: &mut SqliteConnection,
    id: EventId,
    set: ReadingSet,
    readings: &[Reading],
) -> Result<(), StorageError> {
    let set = name(&set, "event_readings.reading_set")?;
    sqlx::query("DELETE FROM event_readings WHERE event_id = ? AND reading_set = ?")
        .bind(id_to_sql(id))
        .bind(set.clone())
        .execute(&mut *connection)
        .await?;
    for reading in readings {
        sqlx::query(
            "INSERT INTO event_readings (event_id, reading_set, meter_id, value)
             VALUES (?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(set.clone())
        .bind(id_to_sql(reading.meter))
        .bind(reading.value)
        .execute(&mut *connection)
        .await?;
    }
    Ok(())
}

async fn write_fields(
    connection: &mut SqliteConnection,
    id: EventId,
    fields: &[FieldValue],
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM event_field_values WHERE event_id = ?")
        .bind(id_to_sql(id))
        .execute(&mut *connection)
        .await?;
    for value in fields {
        sqlx::query("INSERT INTO event_field_values (event_id, field_id, data) VALUES (?, ?, ?)")
            .bind(id_to_sql(id))
            .bind(id_to_sql(value.field))
            .bind(json_to_sql(&value.data, "event_field_values.data")?)
            .execute(&mut *connection)
            .await?;
    }
    Ok(())
}

/// Writes a lifecycle event's details (milestone 9): as JSON, with the send-out a return ends.
async fn write_lifecycle(
    connection: &mut SqliteConnection,
    id: EventId,
    details: &LifecycleDetails,
) -> Result<(), StorageError> {
    let sent_out = match details {
        LifecycleDetails::Returned(back) => Some(id_to_sql(back.sent_out)),
        LifecycleDetails::Acquired(_)
        | LifecycleDetails::SentOut(_)
        | LifecycleDetails::Disposed(_) => None,
    };
    sqlx::query("INSERT INTO lifecycle_details (event_id, details, sent_out_id) VALUES (?, ?, ?)")
        .bind(id_to_sql(id))
        .bind(json_to_sql(details, "lifecycle_details.details")?)
        .bind(sent_out)
        .execute(connection)
        .await?;
    Ok(())
}

async fn write_fuel(
    connection: &mut SqliteConnection,
    id: EventId,
    fuel: Option<&FuelDetails>,
) -> Result<(), StorageError> {
    sqlx::query("DELETE FROM fuel_details WHERE event_id = ?")
        .bind(id_to_sql(id))
        .execute(&mut *connection)
        .await?;
    let Some(fuel) = fuel else {
        return Ok(());
    };
    sqlx::query(
        "INSERT INTO fuel_details (event_id, volume_ml, cost_minor, currency, filled_full,
             missed_previous, highway_pct, towing_pct, hauling_pct, towed, reported_economy)
         VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)",
    )
    .bind(id_to_sql(id))
    .bind(fuel.volume_ml)
    .bind(fuel.cost_minor)
    .bind(fuel.currency.as_str().to_owned())
    .bind(fuel.filled_full)
    .bind(fuel.missed_previous)
    .bind(fuel.highway_pct)
    .bind(fuel.towing_pct)
    .bind(fuel.hauling_pct)
    .bind(json_to_sql(&fuel.towed, "fuel_details.towed")?)
    .bind(fuel.reported_economy)
    .execute(connection)
    .await?;
    Ok(())
}

// --- Reading events back -----------------------------------------------------------------

/// An optional reason column.
fn reason_from(text: Option<String>, column: &'static str) -> Result<Option<Reason>, StorageError> {
    text.map(|text| Reason::parse(&text).map_err(invalid(column)))
        .transpose()
}

/// An optional ID column.
fn optional_id<T>(row: &SqliteRow, column: &'static str) -> Result<Option<Id<T>>, StorageError> {
    row.try_get::<Option<&[u8]>, _>(column)?
        .map(|bytes| id_from_sql(bytes, column))
        .transpose()
}

fn location_from_row(row: &SqliteRow) -> Result<EventLocation, StorageError> {
    let city: Option<String> = row.try_get("city")?;
    let place = match city {
        None => None,
        Some(city) => {
            let region: String = row.try_get("region")?;
            let country: String = row.try_get("country_code")?;
            Some(Place::parse(&city, &region, &country).map_err(invalid("events.city"))?)
        }
    };
    let lat: Option<i32> = row.try_get("lat_e7")?;
    let lon: Option<i32> = row.try_get("lon_e7")?;
    let coordinates = match (lat, lon) {
        (Some(lat), Some(lon)) => {
            Some(Coordinates::from_e7(lat, lon).map_err(invalid("events.lat_e7"))?)
        }
        _ => None,
    };
    let place_name: Option<String> = row.try_get("place_name")?;
    Ok(EventLocation {
        place,
        coordinates,
        accuracy_m: row.try_get("accuracy_m")?,
        fix_time: row
            .try_get::<Option<i64>, _>("fix_ms")?
            .map(UnixMillis::from_millis),
        source: required_name(row.try_get("location_source")?, "events.location_source")?,
        place_name: place_name
            .map(|name| Label::parse(&name).map_err(invalid("events.place_name")))
            .transpose()?,
        saved_place: optional_id(row, "saved_place_id")?,
        geocoder_version: row.try_get("geocoder_version")?,
        missing_reason: reason_from(row.try_get("no_place_reason")?, "events.no_place_reason")?,
    })
}

fn performer_from_row(row: &SqliteRow) -> Result<Option<Performer>, StorageError> {
    if let Some(user) = optional_id(row, "performer_user_id")? {
        return Ok(Some(Performer::User { user }));
    }
    let name: Option<String> = row.try_get("performer_name")?;
    let contact: Option<String> = row.try_get("performer_contact")?;
    Ok(name.map(|name| Performer::Named { name, contact }))
}

fn finalized_from_row(row: &SqliteRow) -> Result<Option<Finalized>, StorageError> {
    let Some(at) = row.try_get::<Option<i64>, _>("finalized_ms")? else {
        return Ok(None);
    };
    let by = match optional_id(row, "finalized_by_user")? {
        Some(user) => Finalizer::User(user),
        None => Finalizer::Server,
    };
    Ok(Some(Finalized {
        at: UnixMillis::from_millis(at),
        by,
    }))
}

/// An event from its `events` row; its readings, field values, fuel details, and work are
/// added by [`with_parts`].
fn event_from_row(row: &SqliteRow) -> Result<(EventId, Event), StorageError> {
    let zone: String = row.try_get("zone")?;
    let skipped: String = row.try_get("skipped")?;
    let notes: String = row.try_get("notes")?;
    let event = Event {
        asset: id_from_sql(row.try_get::<&[u8], _>("asset_id")?, "events.asset_id")?,
        kind: required_name(row.try_get("kind")?, "events.kind")?,
        dates: EventDates {
            start: UnixMillis::from_millis(row.try_get("start_ms")?),
            end: row
                .try_get::<Option<i64>, _>("end_ms")?
                .map(UnixMillis::from_millis),
            zone: TimeZone::parse(&zone).map_err(invalid("events.zone"))?,
        },
        location: location_from_row(row)?,
        readings: Vec::new(),
        skipped: json_from_sql(&skipped, "events.skipped")?,
        fields: Vec::new(),
        notes: Notes::parse(&notes).map_err(invalid("events.notes"))?,
        source: required_name(row.try_get("source")?, "events.source")?,
        logged_by: id_from_sql(row.try_get::<&[u8], _>("logged_by")?, "events.logged_by")?,
        performed_by: performer_from_row(row)?,
        fuel: None,
        work: None,
        lifecycle: None,
        reading_override: reason_from(row.try_get("reading_override")?, "events.reading_override")?,
        status: required_name(row.try_get("status")?, "events.status")?,
        last_edited_at: UnixMillis::from_millis(row.try_get("last_edited_ms")?),
        received_at: UnixMillis::from_millis(row.try_get("received_ms")?),
        finalized: finalized_from_row(row)?,
        corrects: optional_id(row, "corrects_id")?,
        correction_reason: reason_from(
            row.try_get("correction_reason")?,
            "events.correction_reason",
        )?,
    };
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "events.id")?,
        event,
    ))
}

fn fuel_from_row(row: &SqliteRow) -> Result<FuelDetails, StorageError> {
    let currency: String = row.try_get("currency")?;
    let towed: String = row.try_get("towed")?;
    let percent = |column: &'static str| -> Result<u8, StorageError> {
        u8::try_from(row.try_get::<i64, _>(column)?).map_err(invalid(column))
    };
    Ok(FuelDetails {
        volume_ml: row.try_get("volume_ml")?,
        cost_minor: row.try_get("cost_minor")?,
        currency: CurrencyCode::parse(&currency).map_err(invalid("fuel_details.currency"))?,
        filled_full: row.try_get("filled_full")?,
        missed_previous: row.try_get("missed_previous")?,
        highway_pct: percent("highway_pct")?,
        towing_pct: percent("towing_pct")?,
        hauling_pct: percent("hauling_pct")?,
        towed: json_from_sql(&towed, "fuel_details.towed")?,
        reported_economy: row.try_get("reported_economy")?,
    })
}

/// Which events' parts to read: one event, or every event of one vehicle.
#[derive(Debug, Clone, Copy)]
enum Scope {
    Event(EventId),
    Asset(AssetId),
}

impl Scope {
    fn key(self) -> Vec<u8> {
        match self {
            Self::Event(id) => id_to_sql(id),
            Self::Asset(id) => id_to_sql(id),
        }
    }
}

/// Adds each event's readings, field values, fuel details, lifecycle details, and work (the work first, so end
/// readings have somewhere to go).
async fn with_parts(
    connection: &mut SqliteConnection,
    scope: Scope,
    mut events: Vec<(EventId, Event)>,
) -> Result<Vec<(EventId, Event)>, StorageError> {
    let (readings_sql, fields_sql, fuel_sql, lifecycle_sql) = match scope {
        Scope::Event(_) => (
            "SELECT event_id, reading_set, meter_id, value FROM event_readings
             WHERE event_id = ? ORDER BY meter_id",
            "SELECT event_id, field_id, data FROM event_field_values WHERE event_id = ?
             ORDER BY field_id",
            "SELECT * FROM fuel_details WHERE event_id = ?",
            "SELECT event_id, details FROM lifecycle_details WHERE event_id = ?",
        ),
        Scope::Asset(_) => (
            "SELECT r.event_id, r.reading_set, r.meter_id, r.value FROM event_readings r
             JOIN events e ON e.id = r.event_id WHERE e.asset_id = ? AND e.deleted = 0
             ORDER BY r.meter_id",
            "SELECT v.event_id, v.field_id, v.data FROM event_field_values v
             JOIN events e ON e.id = v.event_id WHERE e.asset_id = ? AND e.deleted = 0
             ORDER BY v.field_id",
            "SELECT f.* FROM fuel_details f
             JOIN events e ON e.id = f.event_id WHERE e.asset_id = ? AND e.deleted = 0",
            "SELECT l.event_id, l.details FROM lifecycle_details l
             JOIN events e ON e.id = l.event_id WHERE e.asset_id = ? AND e.deleted = 0",
        ),
    };
    let index: HashMap<EventId, usize> = events
        .iter()
        .enumerate()
        .map(|(position, (id, _))| (*id, position))
        .collect();
    let event_of = |row: &SqliteRow| -> Result<Option<usize>, StorageError> {
        let id: EventId = id_from_sql(row.try_get::<&[u8], _>("event_id")?, "event_id")?;
        Ok(index.get(&id).copied())
    };
    work::add_work(connection, scope, &mut events, &index).await?;
    for row in sqlx::query(readings_sql)
        .bind(scope.key())
        .fetch_all(&mut *connection)
        .await?
    {
        if let Some(position) = event_of(&row)? {
            let reading = Reading {
                meter: id_from_sql(
                    row.try_get::<&[u8], _>("meter_id")?,
                    "event_readings.meter_id",
                )?,
                value: row.try_get("value")?,
            };
            let set = required_name(row.try_get("reading_set")?, "event_readings.reading_set")?;
            work::add_reading(&mut events, position, set, reading)?;
        }
    }
    for row in sqlx::query(fields_sql)
        .bind(scope.key())
        .fetch_all(&mut *connection)
        .await?
    {
        if let Some(position) = event_of(&row)? {
            let data: String = row.try_get("data")?;
            events[position].1.fields.push(FieldValue {
                field: id_from_sql(
                    row.try_get::<&[u8], _>("field_id")?,
                    "event_field_values.field_id",
                )?,
                data: json_from_sql(&data, "event_field_values.data")?,
            });
        }
    }
    for row in sqlx::query(fuel_sql)
        .bind(scope.key())
        .fetch_all(&mut *connection)
        .await?
    {
        if let Some(position) = event_of(&row)? {
            events[position].1.fuel = Some(fuel_from_row(&row)?);
        }
    }
    add_lifecycle(connection, lifecycle_sql, scope, &mut events, &index).await?;
    Ok(events)
}

/// Adds each event's lifecycle details, from `sql` (by event or by vehicle).
async fn add_lifecycle(
    connection: &mut SqliteConnection,
    sql: &'static str,
    scope: Scope,
    events: &mut [(EventId, Event)],
    index: &HashMap<EventId, usize>,
) -> Result<(), StorageError> {
    for row in sqlx::query(sql)
        .bind(scope.key())
        .fetch_all(&mut *connection)
        .await?
    {
        let id: EventId = id_from_sql(row.try_get::<&[u8], _>("event_id")?, "event_id")?;
        if let Some(position) = index.get(&id) {
            let details: String = row.try_get("details")?;
            events[*position].1.lifecycle =
                Some(json_from_sql(&details, "lifecycle_details.details")?);
        }
    }
    Ok(())
}

/// One event, if it exists and isn't deleted, read on `connection` (inside a command's
/// transaction, so it sees the command's own changes).
async fn load(
    connection: &mut SqliteConnection,
    id: EventId,
) -> Result<Option<Event>, StorageError> {
    let Some(row) = sqlx::query("SELECT * FROM events WHERE id = ? AND deleted = 0")
        .bind(id_to_sql(id))
        .fetch_optional(&mut *connection)
        .await?
    else {
        return Ok(None);
    };
    let event = event_from_row(&row)?;
    Ok(with_parts(connection, Scope::Event(id), vec![event])
        .await?
        .pop()
        .map(|(_, event)| event))
}

/// A reading that counts: its meter, its event, when, and its value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CountedReading {
    /// Which meter.
    pub meter: MeterId,
    /// Which event it's on.
    pub event: EventId,
    /// The event's start.
    pub at: UnixMillis,
    /// The value, in the meter's stored unit.
    pub value: i64,
}

impl Database {
    /// One vehicle's lifecycle events that count (milestone 9): not deleted, and not ones a
    /// correction replaces. The rules in `domain::lifecycle` work on these. Each comes with
    /// what it corrects, if anything, so a caller can also leave out the one it's checking
    /// against.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn lifecycle_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(LifecycleRecord, Option<EventId>)>, StorageError> {
        let rows = sqlx::query(
            "SELECT e.id, e.start_ms, e.status, e.corrects_id, l.details
             FROM lifecycle_details l JOIN events e ON e.id = l.event_id
             WHERE e.asset_id = ? AND e.deleted = 0
               AND NOT EXISTS (SELECT 1 FROM events c
                               WHERE c.corrects_id = e.id AND c.deleted = 0)
             ORDER BY e.start_ms, e.id",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?;
        rows.iter()
            .map(|row| {
                let details: String = row.try_get("details")?;
                let corrects = row
                    .try_get::<Option<Vec<u8>>, _>("corrects_id")?
                    .map(|bytes| id_from_sql(&bytes, "events.corrects_id"))
                    .transpose()?;
                Ok((
                    LifecycleRecord {
                        id: id_from_sql(row.try_get::<&[u8], _>("id")?, "events.id")?,
                        at: UnixMillis::from_millis(row.try_get("start_ms")?),
                        status: required_name(row.try_get("status")?, "events.status")?,
                        details: json_from_sql(&details, "lifecycle_details.details")?,
                    },
                    corrects,
                ))
            })
            .collect()
    }

    /// One event, if it exists and isn't deleted.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn event(&self, id: EventId) -> Result<Option<Event>, StorageError> {
        let mut connection = self.pool().acquire().await?;
        load(&mut connection, id).await
    }

    /// One vehicle's events (not deleted), newest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn events_of(&self, asset: AssetId) -> Result<Vec<(EventId, Event)>, StorageError> {
        let mut connection = self.pool().acquire().await?;
        let events = sqlx::query(
            "SELECT * FROM events WHERE asset_id = ? AND deleted = 0
             ORDER BY start_ms DESC, id DESC",
        )
        .bind(id_to_sql(asset))
        .fetch_all(&mut *connection)
        .await?
        .iter()
        .map(event_from_row)
        .collect::<Result<Vec<_>, _>>()?;
        with_parts(&mut connection, Scope::Asset(asset), events).await
    }

    /// Every reading on one vehicle's events that counts, in time order: not on a deleted
    /// event, nor on one that has been corrected (its correction's readings count instead). A
    /// job's end readings are at its end (or its start, for a one-day job).
    /// The checks compare a new reading with its neighbours here (leave the event itself out,
    /// and for a correction, the event it corrects).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn readings_of(&self, asset: AssetId) -> Result<Vec<CountedReading>, StorageError> {
        sqlx::query(
            "SELECT r.meter_id, r.event_id, r.value,
                    CASE r.reading_set WHEN 'end' THEN coalesce(e.end_ms, e.start_ms)
                                       ELSE e.start_ms END AS at_ms
             FROM event_readings r JOIN events e ON e.id = r.event_id
             WHERE e.asset_id = ? AND e.deleted = 0
               AND NOT EXISTS (SELECT 1 FROM events c WHERE c.corrects_id = e.id AND c.deleted = 0)
             ORDER BY at_ms, r.value",
        )
        .bind(id_to_sql(asset))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(|row| {
            Ok(CountedReading {
                meter: id_from_sql(
                    row.try_get::<&[u8], _>("meter_id")?,
                    "event_readings.meter_id",
                )?,
                event: id_from_sql(
                    row.try_get::<&[u8], _>("event_id")?,
                    "event_readings.event_id",
                )?,
                at: UnixMillis::from_millis(row.try_get("at_ms")?),
                value: row.try_get("value")?,
            })
        })
        .collect()
    }

    /// Each of one vehicle's meters' latest reading that counts (by the event's date).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn latest_readings(
        &self,
        asset: AssetId,
    ) -> Result<Vec<CountedReading>, StorageError> {
        let mut latest: Vec<CountedReading> = Vec::new();
        // In time order, so each meter's last one wins.
        for reading in self.readings_of(asset).await? {
            match latest.iter_mut().find(|have| have.meter == reading.meter) {
                Some(have) => *have = reading,
                None => latest.push(reading),
            }
        }
        Ok(latest)
    }

    /// The drafts last edited at or before `cutoff` (due to be finalized).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored ID is invalid.
    pub async fn drafts_edited_before(
        &self,
        cutoff: UnixMillis,
    ) -> Result<Vec<EventId>, StorageError> {
        let ids: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT id FROM events
             WHERE status = 'draft' AND deleted = 0 AND last_edited_ms <= ?
               AND kind IN ('fuel', 'reading')
             ORDER BY last_edited_ms",
        )
        .bind(cutoff.as_millis())
        .fetch_all(self.pool())
        .await?;
        ids.iter().map(|id| id_from_sql(id, "events.id")).collect()
    }

    /// The maintenance and repair drafts last edited at or before `cutoff` (forgotten jobs,
    /// #034: only a reminder, they're never finalized by themselves).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored ID is invalid.
    pub async fn forgotten_jobs(&self, cutoff: UnixMillis) -> Result<Vec<EventId>, StorageError> {
        let ids: Vec<Vec<u8>> = sqlx::query_scalar(
            "SELECT id FROM events
             WHERE status = 'draft' AND deleted = 0 AND last_edited_ms <= ?
               AND kind IN ('maintenance', 'repair')
             ORDER BY last_edited_ms",
        )
        .bind(cutoff.as_millis())
        .fetch_all(self.pool())
        .await?;
        ids.iter().map(|id| id_from_sql(id, "events.id")).collect()
    }

    /// One vehicle's finalized events that count, newest first: not deleted, and not corrected
    /// by a finalized correction (that counts instead). Squawk dispositions and done tasks come
    /// from these.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails or a stored value is invalid.
    pub async fn counted_events_of(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(EventId, Event)>, StorageError> {
        let events = self.events_of(asset).await?;
        let corrected: Vec<EventId> = events
            .iter()
            .filter(|(_, event)| event.status == EventStatus::Finalized)
            .filter_map(|(_, event)| event.corrects)
            .collect();
        Ok(events
            .into_iter()
            .filter(|(id, event)| event.status == EventStatus::Finalized && !corrected.contains(id))
            .collect())
    }

    /// The event that corrects `original`, if any (not deleted).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored ID is invalid.
    pub async fn correction_of(&self, original: EventId) -> Result<Option<EventId>, StorageError> {
        let id: Option<Vec<u8>> = sqlx::query_scalar(
            "SELECT id FROM events WHERE corrects_id = ? AND deleted = 0 ORDER BY id DESC LIMIT 1",
        )
        .bind(id_to_sql(original))
        .fetch_optional(self.pool())
        .await?;
        id.map(|id| id_from_sql(&id, "events.id")).transpose()
    }
}
