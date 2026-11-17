//! What's due on the server (Phase 5 milestone 2; decision #082).
//!
//! `vehicle_due` gathers one vehicle's meters, readings, finished tasks, requirements (and
//! when each was added), documents, and the household's due warnings, and asks
//! [`domain::due`] where each requirement and document stands. The Maintenance tab
//! ([`crate::schedule::view`]), the Overview ([`list`]), and the Garage
//! ([`crate::assets::garage`]) use it, on the server and, from its copy, on the desktop.
//!
//! **Today** is the person's: each call takes their device's time zone (#082), as the
//! documents' status already does. An unknown zone falls back to UTC.
//!
//! **Who sees it:** anyone whose role covers the vehicle, as with its schedule and documents.
//!
//! **Across vehicles** (#083): due messages in each person's inbox, the
//! Garage's fleet strip, and Due & squawks ([`fleet`]) cover the vehicles the Garage shows
//! them: their role covers it, and it isn't retired or disposed.

use domain::access::{AssetScope, role_on};
use domain::api::{
    AssetsOutcome, DueEntry, DueOutcome, DueSubject, FleetDue, FleetOutcome, FleetSquawk,
    FleetView, SquawksOutcome,
};
use domain::asset::{Asset, AssetId};
use domain::dates::{CalendarDate, is_known_zone, today, utc_to_local};
use domain::due::{
    DueContext, DueDefaults, DueStep, FleetCounts, document_due, requirement_due, snooze_meter,
    urgency_order,
};
use domain::event_check::PastReading;
use domain::inbox::{Message, due_message};
use domain::location::TimeZone;
use domain::log_area;
use domain::meter::MeterId;
use domain::preferences::PreferencesId;
use domain::requirement::{DoneTask, last_done};
use domain::time::UnixMillis;
use storage::{Database, StorageError};

use crate::assets::AssetsContext;

/// `zone` if it's known, else UTC.
fn usable(zone: &TimeZone) -> TimeZone {
    if is_known_zone(zone) {
        zone.clone()
    } else {
        TimeZone::parse("UTC").unwrap_or_else(|_| zone.clone())
    }
}

/// The household's due warnings (its record's, else the built-in defaults).
///
/// # Errors
///
/// [`StorageError`] if the preferences can't be read.
pub(crate) async fn household_warnings(
    database: &Database,
    household: PreferencesId,
) -> Result<DueDefaults, StorageError> {
    Ok(household_due(database, household).await?.0)
}

/// The household's due warnings and the step its due messages start at (#083).
///
/// # Errors
///
/// [`StorageError`] if the preferences can't be read.
async fn household_due(
    database: &Database,
    household: PreferencesId,
) -> Result<(DueDefaults, DueStep), StorageError> {
    let choices = database.preference_choices(household).await?;
    Ok((
        choices.due_warnings.unwrap_or_default(),
        choices.due_messages_from.unwrap_or(DueStep::DueSoon),
    ))
}

/// One vehicle's due entries, with what its messages and snoozes need.
pub(crate) struct VehicleDue {
    /// What comes due, most urgent first.
    pub(crate) entries: Vec<DueEntry>,
    /// Its finished tasks (each requirement's cycle runs from its latest).
    pub(crate) done: Vec<DoneTask>,
    /// Each meter's latest value.
    pub(crate) latest: Vec<(MeterId, i64)>,
}

impl VehicleDue {
    /// Its most urgent step, if anything on it comes due.
    pub(crate) fn most_urgent(&self) -> Option<DueStep> {
        self.entries.iter().map(|entry| entry.due.step).max()
    }
}

/// Everything on vehicle `asset` that comes due, most urgent first, on `now`'s date in
/// `zone`, with the household's `defaults`.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn vehicle_due(
    database: &Database,
    asset: AssetId,
    zone: &TimeZone,
    now: UnixMillis,
    defaults: &DueDefaults,
) -> Result<Vec<DueEntry>, StorageError> {
    Ok(vehicle_due_in_full(database, asset, zone, now, defaults)
        .await?
        .entries)
}

/// [`vehicle_due`], with the vehicle's finished tasks and each meter's latest value.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn vehicle_due_in_full(
    database: &Database,
    asset: AssetId,
    zone: &TimeZone,
    now: UnixMillis,
    defaults: &DueDefaults,
) -> Result<VehicleDue, StorageError> {
    let zone = usable(zone);
    let meters = database.meters_of(asset).await?;
    let readings: Vec<PastReading> = database
        .readings_of(asset)
        .await?
        .into_iter()
        .map(|reading| PastReading {
            meter: reading.meter,
            at: reading.at,
            value: reading.value,
            reset: reading.reset,
        })
        .collect();
    let done = database.done_tasks_of(asset).await?;
    let added = database.requirements_added(asset).await?;
    let context = DueContext {
        now,
        zone: &zone,
        meters: &meters,
        readings: &readings,
        done: &done,
        defaults,
    };
    let mut entries = Vec::new();
    for (id, requirement) in database.requirements_of(asset).await? {
        let added_on = added
            .iter()
            .find(|(added, _)| *added == id)
            .and_then(|(_, at)| utc_to_local(*at, &zone).ok())
            .map(|(date, _)| date);
        if let Some(due) = requirement_due(id, &requirement, &context, added_on.as_deref()) {
            entries.push(DueEntry {
                subject: DueSubject::Requirement {
                    id,
                    name: requirement.name.clone(),
                },
                due,
            });
        }
    }
    if let Some(today) = today(now, &zone)
        .ok()
        .and_then(|text| CalendarDate::parse(&text).ok())
    {
        let documents = database.documents_of(asset).await?;
        for (id, document) in &documents {
            if let Some(due) = document_due(*id, document, &documents, &today, &zone, defaults) {
                entries.push(DueEntry {
                    subject: DueSubject::Document {
                        id: *id,
                        doc_type: document.doc_type.clone(),
                    },
                    due,
                });
            }
        }
    }
    entries.sort_by(|a, b| urgency_order(&a.due, &b.due));
    let latest = meters
        .iter()
        .filter_map(|(meter, _)| {
            readings
                .iter()
                .filter(|reading| reading.meter == *meter)
                .max_by_key(|reading| (reading.at, reading.reset))
                .map(|reading| (*meter, reading.value))
        })
        .collect();
    Ok(VehicleDue {
        entries,
        done,
        latest,
    })
}

/// The vehicles the Garage shows the signed-in person: their role covers it, and it isn't
/// retired or disposed (by its lifecycle events, or its setup).
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn shown_vehicles(
    context: &AssetsContext<'_>,
) -> Result<Vec<(AssetId, Asset)>, StorageError> {
    let mut shown = Vec::new();
    for (id, asset) in context.database.assets().await? {
        let scope = AssetScope {
            id,
            group: asset.group,
        };
        if role_on(&context.user.grants, scope).is_none() {
            continue;
        }
        let standing = crate::lifecycle::standing(context.database, id, asset.status).await?;
        if standing.status.shows_in_garage() {
            shown.push((id, asset));
        }
    }
    Ok(shown)
}

/// The signed-in person's due messages, for the vehicles the Garage shows them, on `now`'s
/// date in `zone`, with each of those vehicles' meters' latest values (for snoozes, #083).
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn due_messages(
    context: &AssetsContext<'_>,
    zone: &TimeZone,
) -> Result<(Vec<Message>, Vec<(MeterId, i64)>), StorageError> {
    let now = context.clock.now();
    let (defaults, from) = household_due(context.database, context.database_id.cast()).await?;
    let mut messages = Vec::new();
    let mut latest = Vec::new();
    for (asset, vehicle) in shown_vehicles(context).await? {
        let due = vehicle_due_in_full(context.database, asset, zone, now, &defaults).await?;
        for entry in &due.entries {
            let cycle = match &entry.subject {
                DueSubject::Requirement { id, .. } => {
                    last_done(*id, &due.done).map(|task| task.event)
                }
                DueSubject::Document { .. } => None,
            };
            messages.extend(due_message(
                asset,
                vehicle.name.as_str(),
                &entry.subject,
                &entry.due,
                cycle,
                from,
                now,
            ));
        }
        latest.extend(due.latest);
    }
    Ok((messages, latest))
}

/// The meter a due message about `subject` on vehicle `asset` snoozes on, with its latest
/// value: the requirement's first counting-up meter ([`snooze_meter`]); none for a document.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn snooze_meter_of(
    database: &Database,
    asset: AssetId,
    subject: &DueSubject,
) -> Result<Option<(MeterId, i64)>, StorageError> {
    let DueSubject::Requirement { id, .. } = subject else {
        return Ok(None);
    };
    let Some((_, requirement)) = database
        .requirements_of(asset)
        .await?
        .into_iter()
        .find(|(found, _)| found == id)
    else {
        return Ok(None);
    };
    let meters = database.meters_of(asset).await?;
    let Some(meter) = snooze_meter(&requirement, &meters) else {
        return Ok(None);
    };
    Ok(database
        .readings_of(asset)
        .await?
        .into_iter()
        .filter(|reading| reading.meter == meter)
        .max_by_key(|reading| (reading.at, reading.reset))
        .map(|reading| (meter, reading.value)))
}

/// Due & squawks (#083): what comes due (Coming up or more urgent) and the open or deferred
/// squawks across the vehicles the Garage shows the person, in their `zone`, with the fleet
/// strip's counts.
pub async fn fleet(context: &AssetsContext<'_>, zone: &TimeZone) -> FleetOutcome {
    match fleet_view(context, zone).await {
        Ok(view) => {
            tracing::debug!(target: log_area::APP, due = view.due.len(), squawks = view.squawks.len(), "due and squawks");
            FleetOutcome::Fleet { view }
        }
        Err(error) => {
            tracing::error!(target: log_area::APP, %error, "due and squawks: can't work it out");
            FleetOutcome::Unavailable
        }
    }
}

async fn fleet_view(
    context: &AssetsContext<'_>,
    zone: &TimeZone,
) -> Result<FleetView, StorageError> {
    let defaults = household_warnings(context.database, context.database_id.cast()).await?;
    let now = context.clock.now();
    let mut due = Vec::new();
    let mut squawks = Vec::new();
    for (asset, vehicle) in shown_vehicles(context).await? {
        let name = vehicle.name.as_str().to_owned();
        for entry in vehicle_due(context.database, asset, zone, now, &defaults).await? {
            if entry.due.step > DueStep::Ok {
                due.push(FleetDue {
                    asset,
                    vehicle: name.clone(),
                    entry,
                });
            }
        }
        // Its squawks as the vehicle's page shows them; only the outstanding ones.
        if let SquawksOutcome::Squawks { squawks: all, .. } =
            crate::squawks::list(context, asset).await
        {
            squawks.extend(
                all.into_iter()
                    .filter(|view| view.status.is_outstanding())
                    .map(|view| FleetSquawk {
                        asset,
                        vehicle: name.clone(),
                        view,
                    }),
            );
        }
    }
    due.sort_by(|a, b| {
        urgency_order(&a.entry.due, &b.entry.due).then_with(|| a.vehicle.cmp(&b.vehicle))
    });
    squawks.sort_by(|a, b| {
        b.view
            .severity
            .cmp(&a.view.severity)
            .then(a.view.squawk.opened_at.cmp(&b.view.squawk.opened_at))
    });
    let counts = FleetCounts::of(due.iter().map(|item| item.entry.due.step), squawks.len());
    Ok(FleetView {
        due,
        squawks,
        counts,
    })
}

/// Everything on vehicle `asset` that comes due, most urgent first, for the Overview's
/// "Coming up" and the Maintenance tab, in the person's `zone`.
pub async fn list(context: &AssetsContext<'_>, asset: AssetId, zone: &TimeZone) -> DueOutcome {
    match context.visible(asset).await {
        Ok(_) => {}
        Err(AssetsOutcome::Unavailable) => return DueOutcome::Unavailable,
        Err(_) => return DueOutcome::NotFound,
    }
    let defaults = match household_warnings(context.database, context.database_id.cast()).await {
        Ok(defaults) => defaults,
        Err(error) => {
            tracing::error!(target: log_area::APP, %error, "due: can't read the household's warnings");
            return DueOutcome::Unavailable;
        }
    };
    match vehicle_due(
        context.database,
        asset,
        zone,
        context.clock.now(),
        &defaults,
    )
    .await
    {
        Ok(entries) => {
            tracing::debug!(target: log_area::APP, %asset, entries = entries.len(), "what's due");
            DueOutcome::Due { entries }
        }
        Err(error) => {
            tracing::error!(target: log_area::APP, %error, %asset, "due: can't work it out");
            DueOutcome::Unavailable
        }
    }
}
