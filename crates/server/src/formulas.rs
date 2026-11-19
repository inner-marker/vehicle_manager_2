//! Calculated fields and derived meters on the server (Phase 5 milestone 7; decision #087).
//!
//! - **Older meters' keys** ([`ensure_meter_keys`]): at startup, every meter from before keys
//!   gets one, made from its name ([`domain::calculated::new_key`]) and saved like any change,
//!   so desktops get it by syncing.
//! - **Readings with derived meters' added** (`readings_with_derived`), for what's due.
//! - **A vehicle's formulas, loaded once** (`Formulas`): its calculated fields' values on
//!   each event (`values_for`) and, for the log form, each kind's earlier values
//!   (`history`). Worked out when asked, never stored.
//!
//! On the desktop, the same, from its copy.

use domain::api::{CalculatedValue, FormulaHistory};
use domain::asset::AssetId;
use domain::calculated::{Vehicle, calculate, new_key, scopes, with_derived};
use domain::command::Change;
use domain::custom_field::{CustomField, CustomFieldId};
use domain::event::{Event, EventId};
use domain::event_check::PastReading;
use domain::formula::Scope;
use domain::id::DeviceId;
use domain::log_area;
use domain::meter::{EventKind, Meter, MeterId, MeterPatch};
use domain::preferences::Preferences;
use domain::time::Clock;
use storage::assets::MeterWriter;
use storage::{Database, StorageError};

use crate::commands::issue;
use crate::health::Health;

/// Gives every meter without a stored key one (at startup), made from its name and different
/// from its vehicle's other keys.
///
/// # Errors
///
/// What went wrong, in words, if the meters can't be read or a key can't be saved.
pub async fn ensure_meter_keys(
    database: &Database,
    device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<usize, String> {
    let text = |error: StorageError| error.to_string();
    let mut made = 0;
    for (id, asset) in database.unkeyed_meters().await.map_err(text)? {
        // Read afresh each time: a key given just now counts as taken.
        let meters = database.meters_of(asset).await.map_err(text)?;
        let fields = database.custom_fields_of(asset).await.map_err(text)?;
        let Some((_, meter)) = meters.iter().find(|(each, _)| *each == id) else {
            continue;
        };
        let others: Vec<(MeterId, Meter)> = meters
            .iter()
            .filter(|(each, _)| *each != id)
            .cloned()
            .collect();
        let patch = MeterPatch {
            key: Some(new_key(&meter.name, &fields, &others)),
            ..MeterPatch::default()
        };
        issue(
            database,
            &MeterWriter,
            Change::Update { id, patch },
            device,
            None,
            health,
            clock,
        )
        .await
        .map_err(|error| error.to_string())?;
        made += 1;
    }
    if made > 0 {
        tracing::info!(target: log_area::APP, made, "meter keys given");
    }
    Ok(made)
}

/// Vehicle `asset`'s typed readings that count, in time order.
///
/// # Errors
///
/// [`StorageError`] if the query fails.
pub(crate) async fn typed_readings(
    database: &Database,
    asset: AssetId,
) -> Result<Vec<PastReading>, StorageError> {
    Ok(database
        .readings_of(asset)
        .await?
        .into_iter()
        .map(|reading| PastReading {
            meter: reading.meter,
            at: reading.at,
            value: reading.value,
            reset: reading.reset,
        })
        .collect())
}

/// Vehicle `asset`'s readings that count, with its derived meters' (`meters`) added, in time
/// order: what what's due counts from.
///
/// # Errors
///
/// [`StorageError`] if the query fails.
pub(crate) async fn readings_with_derived(
    database: &Database,
    asset: AssetId,
    meters: &[(MeterId, Meter)],
) -> Result<Vec<PastReading>, StorageError> {
    let readings = typed_readings(database, asset).await?;
    Ok(if meters.iter().any(|(_, meter)| meter.derived()) {
        with_derived(meters, &readings)
    } else {
        readings
    })
}

/// A vehicle's calculated fields and what they're worked out from.
pub(crate) struct Formulas {
    fields: Vec<(CustomFieldId, CustomField)>,
    meters: Vec<(MeterId, Meter)>,
    readings: Vec<PastReading>,
    /// The events that count (not those a correction replaces), oldest first.
    events: Vec<(EventId, Event)>,
    units: Preferences,
}

impl Formulas {
    /// Vehicle `asset`'s, for a person who sees values in `units`; `None` if it has no
    /// calculated field.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if a query fails.
    pub(crate) async fn load(
        database: &Database,
        asset: AssetId,
        units: Preferences,
    ) -> Result<Option<Self>, StorageError> {
        let fields = database.custom_fields_of(asset).await?;
        if !fields.iter().any(|(_, field)| field.calculated()) {
            return Ok(None);
        }
        let meters = database.meters_of(asset).await?;
        let readings = typed_readings(database, asset).await?;
        let all = database.events_of(asset).await?;
        let mut events: Vec<(EventId, Event)> = all
            .iter()
            .filter(|(id, _)| !all.iter().any(|(_, other)| other.corrects == Some(*id)))
            .cloned()
            .collect();
        events.sort_by_key(|(id, event)| (event.dates.start, *id));
        Ok(Some(Self {
            fields,
            meters,
            readings,
            events,
            units,
        }))
    }

    fn vehicle(&self) -> Vehicle<'_> {
        Vehicle {
            fields: &self.fields,
            meters: &self.meters,
            readings: &self.readings,
        }
    }

    /// The events of `kind` that count, oldest first, and their values.
    fn of_kind(&self, kind: EventKind) -> (Vec<EventId>, Vec<Scope>) {
        let events: Vec<(EventId, &Event)> = self
            .events
            .iter()
            .filter(|(_, event)| event.kind == kind)
            .map(|(id, event)| (*id, event))
            .collect();
        let found = scopes(
            &events.iter().map(|(_, event)| *event).collect::<Vec<_>>(),
            &self.vehicle(),
        );
        (events.into_iter().map(|(id, _)| id).collect(), found)
    }

    /// Event `id`'s calculated fields' values (none for an event a correction replaces).
    pub(crate) fn values_for(&self, id: EventId, event: &Event) -> Vec<CalculatedValue> {
        let (ids, found) = self.of_kind(event.kind);
        let Some(index) = ids.iter().position(|each| *each == id) else {
            return Vec::new();
        };
        self.fields
            .iter()
            .filter(|(_, field)| field.calculated() && field.shown(event.kind))
            .filter_map(|(field_id, field)| {
                calculate(
                    field,
                    &found,
                    index,
                    &self.meters,
                    &event.dates.zone,
                    &self.units,
                )
                .map(|value| CalculatedValue {
                    field: *field_id,
                    value,
                })
            })
            .collect()
    }

    /// Every event's calculated fields' values, by event (an event without any is left out).
    pub(crate) fn all_values(&self) -> Vec<(EventId, Vec<CalculatedValue>)> {
        let mut kinds: Vec<EventKind> = self.events.iter().map(|(_, event)| event.kind).collect();
        kinds.sort_unstable();
        kinds.dedup();
        let mut found = Vec::new();
        for kind in kinds {
            let fields: Vec<&(CustomFieldId, CustomField)> = self
                .fields
                .iter()
                .filter(|(_, field)| field.calculated() && field.shown(kind))
                .collect();
            if fields.is_empty() {
                continue;
            }
            let (ids, values) = self.of_kind(kind);
            for (index, id) in ids.iter().enumerate() {
                let Some((_, event)) = self.events.iter().find(|(each, _)| each == id) else {
                    continue;
                };
                let calculated = fields
                    .iter()
                    .filter_map(|(field_id, field)| {
                        calculate(
                            field,
                            &values,
                            index,
                            &self.meters,
                            &event.dates.zone,
                            &self.units,
                        )
                        .map(|value| CalculatedValue {
                            field: *field_id,
                            value,
                        })
                    })
                    .collect();
                found.push((*id, calculated));
            }
        }
        found
    }

    /// Each kind with a calculated field, and its events' values, oldest first (for the log
    /// form).
    pub(crate) fn history(&self) -> Vec<FormulaHistory> {
        let mut kinds: Vec<EventKind> = self
            .fields
            .iter()
            .filter(|(_, field)| field.calculated() && field.enabled)
            .flat_map(|(_, field)| field.shown_on.iter().copied())
            .collect();
        kinds.sort_unstable();
        kinds.dedup();
        kinds
            .into_iter()
            .map(|kind| FormulaHistory {
                kind,
                earlier: self.of_kind(kind).1,
            })
            .collect()
    }
}
