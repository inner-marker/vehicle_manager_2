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

use domain::api::{AssetsOutcome, DueEntry, DueOutcome, DueSubject};
use domain::asset::AssetId;
use domain::dates::{CalendarDate, is_known_zone, today, utc_to_local};
use domain::due::{DueContext, DueDefaults, DueStep, document_due, requirement_due, urgency_order};
use domain::event_check::PastReading;
use domain::location::TimeZone;
use domain::log_area;
use domain::preferences::PreferencesId;
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
    Ok(database
        .preference_choices(household)
        .await?
        .due_warnings
        .unwrap_or_default())
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
    Ok(entries)
}

/// The most urgent step on vehicle `asset`, if anything on it comes due.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn most_urgent(
    database: &Database,
    asset: AssetId,
    zone: &TimeZone,
    now: UnixMillis,
    defaults: &DueDefaults,
) -> Result<Option<DueStep>, StorageError> {
    Ok(vehicle_due(database, asset, zone, now, defaults)
        .await?
        .iter()
        .map(|entry| entry.due.step)
        .max())
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
