//! A vehicle's usages on the server (Phase 5 milestone 6; decision #086).
//!
//! - **The default usage:** every vehicle has one, named for its kind and needing its
//!   profile's documents ([`domain::usage::default_usage`]). [`ensure_default_usages`] makes it
//!   at startup for any vehicle without one (every vehicle from before usages), and adding a
//!   vehicle makes its own (`make_default`).
//! - **Who:** everyone who sees the vehicle's schedule sees its usages; whoever may change the
//!   schedule (a Maintainer on it, in Maintainer mode or above) adds, changes, and removes them.
//! - **Removing** one unticks it on the vehicle's requirements. The default can't be removed
//!   (rename it instead).

use domain::api::ScheduleOutcome;
use domain::asset::{AssetId, AssetKind};
use domain::command::{Change, Clearable};
use domain::id::DeviceId;
use domain::log_area;
use domain::requirement::RequirementPatch;
use domain::time::Clock;
use domain::usage::{Usage, UsageId, UsagePatch, UsageProblem, check_usage, default_usage};
use storage::Database;
use storage::requirements::RequirementWriter;
use storage::usages::UsageWriter;

use crate::assets::AssetsContext;
use crate::commands::issue;
use crate::health::Health;
use crate::ids::new_id;

/// Makes the default usage for every vehicle without one (at startup).
///
/// # Errors
///
/// What went wrong, in words, if the vehicles can't be read or a usage can't be saved.
pub async fn ensure_default_usages(
    database: &Database,
    device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<usize, String> {
    let mut made = 0;
    for (asset, vehicle) in database.assets().await.map_err(|error| error.to_string())? {
        let usages = database
            .usages_of(asset)
            .await
            .map_err(|error| error.to_string())?;
        if usages.iter().any(|(_, usage)| usage.default) {
            continue;
        }
        let Some(usage) = default_usage(asset, &vehicle.kind) else {
            continue;
        };
        let id: UsageId = new_id(clock.now()).map_err(|error| error.to_string())?;
        issue(
            database,
            &UsageWriter,
            Change::Create {
                id,
                values: UsagePatch::all(usage),
            },
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
        tracing::info!(target: log_area::APP, made, "default usages made");
    }
    Ok(made)
}

/// Makes a new vehicle's default usage, as the person adding it.
///
/// # Errors
///
/// The vehicles answer to give if it can't be saved.
pub(crate) async fn make_default(
    context: &AssetsContext<'_>,
    asset: AssetId,
    kind: &AssetKind,
) -> Result<(), domain::api::AssetsOutcome> {
    save_default(context, default_usage(asset, kind)).await
}

/// Saves a new vehicle's default usage, if it has one (a vehicle type's, #090).
///
/// # Errors
///
/// The vehicles answer to give if it can't be saved.
pub(crate) async fn save_default(
    context: &AssetsContext<'_>,
    usage: Option<Usage>,
) -> Result<(), domain::api::AssetsOutcome> {
    let Some(usage) = usage else {
        return Ok(());
    };
    let id: UsageId = context.new_id()?;
    context
        .save(
            &UsageWriter,
            Change::Create {
                id,
                values: UsagePatch::all(usage),
            },
        )
        .await
}

fn either(result: Result<ScheduleOutcome, ScheduleOutcome>) -> ScheduleOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

const fn invalid(problem: UsageProblem) -> ScheduleOutcome {
    ScheduleOutcome::InvalidUsage { problem }
}

impl AssetsContext<'_> {
    async fn usages_for(&self, asset: AssetId) -> Result<Vec<(UsageId, Usage)>, ScheduleOutcome> {
        self.database.usages_of(asset).await.map_err(|error| {
            tracing::error!(target: log_area::APP, %error, %asset, "can't read usages");
            ScheduleOutcome::Unavailable
        })
    }

    async fn save_usage(&self, change: Change<Usage>) -> Result<(), ScheduleOutcome> {
        self.save(&UsageWriter, change)
            .await
            .map_err(|_| ScheduleOutcome::Unavailable)
    }

    /// The stored usage, if the person may change its vehicle's schedule.
    async fn configurable_usage(&self, id: UsageId) -> Result<Usage, ScheduleOutcome> {
        let usage = self
            .database
            .usage(id)
            .await
            .map_err(|_| ScheduleOutcome::Unavailable)?
            .ok_or(ScheduleOutcome::NotFound)?;
        self.configurable(usage.asset).await?;
        Ok(usage)
    }
}

/// Adds a usage to its vehicle (never a second default).
pub async fn add_usage(context: &AssetsContext<'_>, usage: Usage) -> ScheduleOutcome {
    either(add_usage_to(context, usage).await)
}

async fn add_usage_to(
    context: &AssetsContext<'_>,
    usage: Usage,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    context.configurable(usage.asset).await?;
    let usage = Usage {
        default: false,
        ..usage
    };
    check_usage(None, &usage, &context.usages_for(usage.asset).await?).map_err(invalid)?;
    let id: UsageId = context.new_id().map_err(|_| ScheduleOutcome::Unavailable)?;
    context
        .save_usage(Change::Create {
            id,
            values: UsagePatch::all(usage),
        })
        .await?;
    tracing::debug!(target: log_area::APP, usage = %id, "usage added");
    Ok(ScheduleOutcome::SavedUsage { id })
}

/// Changes a usage's name and the document types it needs (its vehicle, and whether it's the
/// default, stay).
pub async fn edit_usage(context: &AssetsContext<'_>, id: UsageId, usage: Usage) -> ScheduleOutcome {
    either(edit_usage_of(context, id, usage).await)
}

async fn edit_usage_of(
    context: &AssetsContext<'_>,
    id: UsageId,
    usage: Usage,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let current = context.configurable_usage(id).await?;
    let usage = Usage {
        asset: current.asset,
        default: current.default,
        ..usage
    };
    check_usage(Some(id), &usage, &context.usages_for(usage.asset).await?).map_err(invalid)?;
    let patch = UsagePatch {
        name: (current.name != usage.name).then_some(usage.name),
        documents: (current.documents != usage.documents).then_some(usage.documents),
        ..UsagePatch::default()
    };
    context.save_usage(Change::Update { id, patch }).await?;
    tracing::debug!(target: log_area::APP, usage = %id, "usage changed");
    Ok(ScheduleOutcome::SavedUsage { id })
}

/// Removes a usage (not the default), unticking it on its vehicle's requirements.
pub async fn delete_usage(context: &AssetsContext<'_>, id: UsageId) -> ScheduleOutcome {
    either(delete_usage_of(context, id).await)
}

async fn delete_usage_of(
    context: &AssetsContext<'_>,
    id: UsageId,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let usage = context.configurable_usage(id).await?;
    if usage.default {
        return Err(invalid(UsageProblem::DefaultStays));
    }
    for (requirement_id, requirement) in context.requirements_for(usage.asset).await? {
        // One that never chose counts for the default, which stays; one that chose loses
        // this usage, perhaps leaving it needed for none.
        let Some(ticked) = requirement.usages.filter(|ticked| ticked.contains(&id)) else {
            continue;
        };
        let patch = RequirementPatch {
            usages: Some(Clearable::Set(
                ticked.into_iter().filter(|each| *each != id).collect(),
            )),
            ..RequirementPatch::default()
        };
        context
            .save(
                &RequirementWriter,
                Change::Update {
                    id: requirement_id,
                    patch,
                },
            )
            .await
            .map_err(|_| ScheduleOutcome::Unavailable)?;
    }
    context.save_usage(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, usage = %id, "usage removed");
    Ok(ScheduleOutcome::Done)
}
