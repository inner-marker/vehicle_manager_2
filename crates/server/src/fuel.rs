//! Fuel economy on the server (Phase 5 milestone 5; decisions #080, #085).
//!
//! [`summary`] gathers a vehicle's fuel-ups and its fuel odometer, and asks [`domain::fuel`]
//! for its stretches, averages, and breakdowns. The Fuel tab and the Overview use it, on the
//! server and, from its copy, on the desktop (offline too).
//!
//! **Which fuel-ups:** every one that isn't deleted, drafts included (a fuel-up is a draft for
//! its first day, and today's should count), except that a finalized correction stands in for
//! what it corrects, and a correction still in draft waits (its original counts until then).
//!
//! **Who sees it:** anyone whose role covers the vehicle.

use domain::api::{AssetsOutcome, FuelOutcome, FuelView};
use domain::asset::AssetId;
use domain::event::{Event, EventId, EventStatus};
use domain::fuel::{Fill, fuel_meter, summarize};
use domain::log_area;
use domain::meter::EventKind;
use storage::StorageError;

use crate::assets::AssetsContext;

/// The fuel-ups that count for economy among a vehicle's `events` (see the module's notes).
fn counted_fuel_ups(events: Vec<(EventId, Event)>) -> Vec<(EventId, Event)> {
    let corrected: Vec<EventId> = events
        .iter()
        .filter(|(_, event)| event.status == EventStatus::Finalized)
        .filter_map(|(_, event)| event.corrects)
        .collect();
    events
        .into_iter()
        .filter(|(_, event)| event.kind == EventKind::Fuel)
        .filter(|(id, event)| match event.status {
            EventStatus::Finalized => !corrected.contains(id),
            EventStatus::Draft => event.corrects.is_none(),
        })
        .collect()
}

/// Vehicle `asset`'s fuel economy, for anyone whose role covers it.
pub async fn summary(context: &AssetsContext<'_>, asset: AssetId) -> FuelOutcome {
    match context.visible(asset).await {
        Ok(_) => {}
        Err(AssetsOutcome::Unavailable) => return FuelOutcome::Unavailable,
        Err(_) => return FuelOutcome::NotFound,
    }
    match fuel_view(context, asset).await {
        Ok(view) => {
            tracing::debug!(target: log_area::APP, %asset, stretches = view.summary.stretches.len(), "fuel economy");
            FuelOutcome::Fuel {
                view: Box::new(view),
            }
        }
        Err(error) => {
            tracing::error!(target: log_area::APP, %error, %asset, "fuel economy: can't work it out");
            FuelOutcome::Unavailable
        }
    }
}

async fn fuel_view(context: &AssetsContext<'_>, asset: AssetId) -> Result<FuelView, StorageError> {
    let database = context.database;
    let meter = fuel_meter(&database.meters_of(asset).await?);
    let fills: Vec<Fill> = counted_fuel_ups(database.events_of(asset).await?)
        .iter()
        .filter_map(|(id, event)| Fill::of(*id, event, meter))
        .collect();
    let last_fill = fills.iter().map(|fill| fill.at).max();
    let summary = summarize(&fills, context.clock.now());
    let mut towed_names = Vec::new();
    for (vehicle, _) in &summary.breakdowns.towed {
        let Some(vehicle) = vehicle else {
            continue;
        };
        // A towed vehicle the person can't see isn't named (the screen says "another vehicle").
        let name = match context.visible(*vehicle).await {
            Ok(_) => database
                .asset(*vehicle)
                .await?
                .map(|towed| towed.name.as_str().to_owned()),
            Err(_) => None,
        };
        if let Some(name) = name {
            towed_names.push((*vehicle, name));
        }
    }
    Ok(FuelView {
        summary,
        towed_names,
        last_fill,
    })
}
