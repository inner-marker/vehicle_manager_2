//! "Ready to use?" on the server (Phase 5 milestone 6; decisions #051, #063, #086).
//!
//! `vehicle_readiness` gathers a vehicle's usages, requirements, what's due (milestone 2),
//! documents, and open or deferred High issues, and asks [`domain::readiness`] about each
//! usage. The Overview shows every usage ([`view`]); each Garage card shows its default
//! usage's answer (`default_badge`). On the desktop, the same, from its copy.
//!
//! **Who sees it:** anyone whose role covers the vehicle.

use domain::api::{AssetsOutcome, DueEntry, ReadinessOutcome};
use domain::asset::AssetId;
use domain::dates::{CalendarDate, today};
use domain::label::Label;
use domain::location::TimeZone;
use domain::log_area;
use domain::readiness::{HighIssue, Mark, Readiness, VehicleRecords, readiness};
use domain::squawk::{Severity, SquawkStatus};
use domain::time::UnixMillis;
use storage::{Database, StorageError};

use crate::assets::AssetsContext;

/// A vehicle's open and deferred High issues.
async fn high_issues(database: &Database, asset: AssetId) -> Result<Vec<HighIssue>, StorageError> {
    Ok(database
        .squawk_states_of(asset)
        .await?
        .into_iter()
        .filter(|state| state.severity() == Severity::High)
        .filter_map(|state| {
            let status = state.status();
            status.is_outstanding().then(|| HighIssue {
                id: state.id,
                description: state.squawk.description.as_str().to_owned(),
                deferred: status == SquawkStatus::Deferred,
            })
        })
        .collect())
}

/// Readiness for each of vehicle `asset`'s usages (the default first), given what's due on it
/// (`due`, from [`crate::due::vehicle_due`]) on `now`'s date in `zone`.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn vehicle_readiness(
    database: &Database,
    asset: AssetId,
    due: &[DueEntry],
    zone: &TimeZone,
    now: UnixMillis,
) -> Result<Vec<Readiness>, StorageError> {
    let usages = database.usages_of(asset).await?;
    if usages.is_empty() {
        return Ok(Vec::new());
    }
    let requirements = database.requirements_of(asset).await?;
    let documents = database.documents_of(asset).await?;
    let high = high_issues(database, asset).await?;
    let Some(today) = today(now, zone)
        .ok()
        .and_then(|text| CalendarDate::parse(&text).ok())
    else {
        return Ok(Vec::new());
    };
    let records = VehicleRecords {
        requirements: &requirements,
        due,
        documents: &documents,
        high: &high,
        today: &today,
    };
    Ok(usages
        .iter()
        .map(|(id, usage)| readiness(*id, usage, &records))
        .collect())
}

/// The default usage's name and answer, for a Garage card.
///
/// # Errors
///
/// [`StorageError`] if a query fails.
pub(crate) async fn default_badge(
    database: &Database,
    asset: AssetId,
    due: &[DueEntry],
    zone: &TimeZone,
    now: UnixMillis,
) -> Result<Option<(Label, Mark)>, StorageError> {
    Ok(vehicle_readiness(database, asset, due, zone, now)
        .await?
        .into_iter()
        .find(|readiness| readiness.default)
        .map(|readiness| (readiness.name, readiness.mark)))
}

/// Vehicle `asset`'s readiness for each of its usages, in the person's `zone`.
pub async fn view(
    context: &AssetsContext<'_>,
    asset: AssetId,
    zone: &TimeZone,
) -> ReadinessOutcome {
    match context.visible(asset).await {
        Ok(_) => {}
        Err(AssetsOutcome::Unavailable) => return ReadinessOutcome::Unavailable,
        Err(_) => return ReadinessOutcome::NotFound,
    }
    let now = context.clock.now();
    let answer = async {
        let defaults =
            crate::due::household_warnings(context.database, context.database_id.cast()).await?;
        let due = crate::due::vehicle_due(context.database, asset, zone, now, &defaults).await?;
        vehicle_readiness(context.database, asset, &due, zone, now).await
    };
    match answer.await {
        Ok(usages) => {
            tracing::debug!(target: log_area::APP, %asset, usages = usages.len(), "readiness");
            ReadinessOutcome::Readiness { usages }
        }
        Err(error) => {
            tracing::error!(target: log_area::APP, %error, %asset, "readiness: can't work it out");
            ReadinessOutcome::Unavailable
        }
    }
}
