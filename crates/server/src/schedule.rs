//! A vehicle's maintenance schedule on the server: its requirements (with their interval
//! rules and when each was last done) and service bundles (milestone 7, choice 4).
//!
//! Who may do what (plan §6 roles):
//! - **See** a vehicle's schedule: any role that covers it.
//! - **Change** it (add, change, or remove requirements and bundles, and copy a schedule onto
//!   it): the Maintainer role on it (or higher), in Maintainer mode or above. Copying also
//!   needs to see the vehicle it copies from.
//!
//! A requirement a task has done (in a draft or a finished job) stays in the history: it can
//! be turned off but not removed. Removing one also takes it out of the bundles, and a bundle
//! left empty goes too.

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::api::{AssetsOutcome, RequirementView, ScheduleOutcome};
use domain::asset::AssetId;
use domain::command::Change;
use domain::log_area;
use domain::requirement::{
    Bundle, BundleId, BundlePatch, Requirement, RequirementId, RequirementPatch,
    RequirementProblem, check_bundle, check_rules, copy_schedule, last_done,
};
use storage::requirements::{BundleWriter, RequirementWriter};

use crate::assets::{AssetsContext, unavailable};

/// A vehicle request's refusal as a schedule answer.
fn from_assets(outcome: &AssetsOutcome) -> ScheduleOutcome {
    match outcome {
        AssetsOutcome::NotFound => ScheduleOutcome::NotFound,
        AssetsOutcome::NotAllowed => ScheduleOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => ScheduleOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> ScheduleOutcome {
    from_assets(&unavailable(what, error))
}

const fn invalid(problem: RequirementProblem) -> ScheduleOutcome {
    ScheduleOutcome::Invalid { problem }
}

/// Turns `Result<ScheduleOutcome, ScheduleOutcome>` into the outcome either way.
fn either(result: Result<ScheduleOutcome, ScheduleOutcome>) -> ScheduleOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

impl AssetsContext<'_> {
    /// The vehicle's scope, if the person can see it.
    async fn schedule_scope(&self, asset: AssetId) -> Result<AssetScope, ScheduleOutcome> {
        self.visible(asset)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    fn may_configure(&self, scope: AssetScope) -> bool {
        permits_on_asset(
            &self.user.grants,
            self.user.mode,
            AssetAction::ConfigureMaintenance,
            scope,
        )
    }

    /// Checks the person may change `asset`'s schedule.
    async fn configurable(&self, asset: AssetId) -> Result<(), ScheduleOutcome> {
        let scope = self.schedule_scope(asset).await?;
        if self.may_configure(scope) {
            Ok(())
        } else {
            Err(ScheduleOutcome::NotAllowed)
        }
    }

    async fn requirements_for(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(RequirementId, Requirement)>, ScheduleOutcome> {
        self.database
            .requirements_of(asset)
            .await
            .map_err(|error| failed("requirements", &error))
    }

    async fn bundles_for(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(BundleId, Bundle)>, ScheduleOutcome> {
        self.database
            .bundles_of(asset)
            .await
            .map_err(|error| failed("bundles", &error))
    }

    /// Checks a requirement's rules against its vehicle's meters.
    async fn check_requirement(&self, requirement: &Requirement) -> Result<(), ScheduleOutcome> {
        let meters = self
            .database
            .meters_of(requirement.asset)
            .await
            .map_err(|error| failed("meters", &error))?;
        check_rules(&requirement.rules, &meters).map_err(invalid)
    }

    /// Checks a bundle's requirements against its vehicle's.
    async fn check_bundle_items(&self, bundle: &Bundle) -> Result<(), ScheduleOutcome> {
        let known: Vec<RequirementId> = self
            .requirements_for(bundle.asset)
            .await?
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        check_bundle(&bundle.requirements, &known).map_err(invalid)
    }

    async fn save_requirement(&self, change: Change<Requirement>) -> Result<(), ScheduleOutcome> {
        self.save(&RequirementWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    async fn save_bundle(&self, change: Change<Bundle>) -> Result<(), ScheduleOutcome> {
        self.save(&BundleWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    /// The stored requirement, if the person may change its vehicle's schedule.
    async fn configurable_requirement(
        &self,
        id: RequirementId,
    ) -> Result<Requirement, ScheduleOutcome> {
        let requirement = self
            .database
            .requirement(id)
            .await
            .map_err(|error| failed("requirement", &error))?
            .ok_or(ScheduleOutcome::NotFound)?;
        self.configurable(requirement.asset).await?;
        Ok(requirement)
    }

    /// The stored bundle, if the person may change its vehicle's schedule.
    async fn configurable_bundle(&self, id: BundleId) -> Result<Bundle, ScheduleOutcome> {
        let bundle = self
            .database
            .bundle(id)
            .await
            .map_err(|error| failed("bundle", &error))?
            .ok_or(ScheduleOutcome::NotFound)?;
        self.configurable(bundle.asset).await?;
        Ok(bundle)
    }
}

// --- Seeing a schedule -------------------------------------------------------------------

/// One vehicle's requirements (each with when it was last done) and bundles.
pub async fn view(context: &AssetsContext<'_>, asset: AssetId) -> ScheduleOutcome {
    either(view_schedule(context, asset).await)
}

async fn view_schedule(
    context: &AssetsContext<'_>,
    asset: AssetId,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let scope = context.schedule_scope(asset).await?;
    let done = context
        .database
        .done_tasks_of(asset)
        .await
        .map_err(|error| failed("done tasks", &error))?;
    let requirements = context
        .requirements_for(asset)
        .await?
        .into_iter()
        .map(|(id, requirement)| RequirementView {
            id,
            last_done: last_done(id, &done).cloned(),
            requirement,
        })
        .collect();
    Ok(ScheduleOutcome::Schedule {
        requirements,
        bundles: context.bundles_for(asset).await?,
        may_configure: context.may_configure(scope),
    })
}

// --- Requirements ------------------------------------------------------------------------

/// Adds a requirement to its vehicle.
pub async fn add_requirement(
    context: &AssetsContext<'_>,
    requirement: Requirement,
) -> ScheduleOutcome {
    either(add_requirement_to(context, requirement).await)
}

async fn add_requirement_to(
    context: &AssetsContext<'_>,
    requirement: Requirement,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    context.configurable(requirement.asset).await?;
    context.check_requirement(&requirement).await?;
    let id: RequirementId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    let asset = requirement.asset;
    context
        .save_requirement(Change::Create {
            id,
            values: RequirementPatch::all(requirement),
        })
        .await?;
    tracing::debug!(target: log_area::APP, requirement = %id, %asset, "requirement added");
    Ok(ScheduleOutcome::SavedRequirement { id })
}

/// Changes a requirement's name, notes, whether it's on, and its rules (its vehicle stays).
pub async fn edit_requirement(
    context: &AssetsContext<'_>,
    id: RequirementId,
    requirement: Requirement,
) -> ScheduleOutcome {
    either(edit_requirement_of(context, id, requirement).await)
}

async fn edit_requirement_of(
    context: &AssetsContext<'_>,
    id: RequirementId,
    requirement: Requirement,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let current = context.configurable_requirement(id).await?;
    let requirement = Requirement {
        asset: current.asset,
        ..requirement
    };
    context.check_requirement(&requirement).await?;
    // Only what differs, so the fields' clocks show what really changed.
    let patch = RequirementPatch {
        name: (current.name != requirement.name).then_some(requirement.name),
        notes: (current.notes != requirement.notes).then_some(requirement.notes),
        enabled: (current.enabled != requirement.enabled).then_some(requirement.enabled),
        rules: (current.rules != requirement.rules).then_some(requirement.rules),
        ..RequirementPatch::default()
    };
    context
        .save_requirement(Change::Update { id, patch })
        .await?;
    tracing::debug!(target: log_area::APP, requirement = %id, "requirement changed");
    Ok(ScheduleOutcome::SavedRequirement { id })
}

/// Removes a requirement no task has done, and takes it out of the bundles (removing any
/// left empty).
pub async fn delete_requirement(context: &AssetsContext<'_>, id: RequirementId) -> ScheduleOutcome {
    either(delete_requirement_of(context, id).await)
}

async fn delete_requirement_of(
    context: &AssetsContext<'_>,
    id: RequirementId,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let requirement = context.configurable_requirement(id).await?;
    let used = context
        .database
        .events_of(requirement.asset)
        .await
        .map_err(|error| failed("events", &error))?
        .iter()
        .filter_map(|(_, event)| event.work.as_ref())
        .flat_map(|work| &work.tasks)
        .any(|task| task.requirement == Some(id));
    if used {
        return Err(invalid(RequirementProblem::InUse));
    }
    for (bundle_id, bundle) in context.bundles_for(requirement.asset).await? {
        if !bundle.requirements.contains(&id) {
            continue;
        }
        let rest: Vec<RequirementId> = bundle
            .requirements
            .into_iter()
            .filter(|item| *item != id)
            .collect();
        let change = if rest.is_empty() {
            Change::Delete { id: bundle_id }
        } else {
            Change::Update {
                id: bundle_id,
                patch: BundlePatch {
                    requirements: Some(rest),
                    ..BundlePatch::default()
                },
            }
        };
        context.save_bundle(change).await?;
    }
    context.save_requirement(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, requirement = %id, "requirement removed");
    Ok(ScheduleOutcome::Done)
}

// --- Bundles -----------------------------------------------------------------------------

/// Adds a bundle to its vehicle.
pub async fn add_bundle(context: &AssetsContext<'_>, bundle: Bundle) -> ScheduleOutcome {
    either(add_bundle_to(context, bundle).await)
}

async fn add_bundle_to(
    context: &AssetsContext<'_>,
    bundle: Bundle,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    context.configurable(bundle.asset).await?;
    context.check_bundle_items(&bundle).await?;
    let id: BundleId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_bundle(Change::Create {
            id,
            values: BundlePatch::all(bundle),
        })
        .await?;
    tracing::debug!(target: log_area::APP, bundle = %id, "bundle added");
    Ok(ScheduleOutcome::SavedBundle { id })
}

/// Changes a bundle's name and requirements (its vehicle stays).
pub async fn edit_bundle(
    context: &AssetsContext<'_>,
    id: BundleId,
    bundle: Bundle,
) -> ScheduleOutcome {
    either(edit_bundle_of(context, id, bundle).await)
}

async fn edit_bundle_of(
    context: &AssetsContext<'_>,
    id: BundleId,
    bundle: Bundle,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    let current = context.configurable_bundle(id).await?;
    let bundle = Bundle {
        asset: current.asset,
        ..bundle
    };
    context.check_bundle_items(&bundle).await?;
    let patch = BundlePatch {
        name: (current.name != bundle.name).then_some(bundle.name),
        requirements: (current.requirements != bundle.requirements).then_some(bundle.requirements),
        ..BundlePatch::default()
    };
    context.save_bundle(Change::Update { id, patch }).await?;
    tracing::debug!(target: log_area::APP, bundle = %id, "bundle changed");
    Ok(ScheduleOutcome::SavedBundle { id })
}

/// Removes a bundle (the jobs it filled in keep their tasks).
pub async fn delete_bundle(context: &AssetsContext<'_>, id: BundleId) -> ScheduleOutcome {
    either(delete_bundle_of(context, id).await)
}

async fn delete_bundle_of(
    context: &AssetsContext<'_>,
    id: BundleId,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    context.configurable_bundle(id).await?;
    context.save_bundle(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, bundle = %id, "bundle removed");
    Ok(ScheduleOutcome::Done)
}

// --- Copying a schedule ------------------------------------------------------------------

/// Copies `from`'s requirements and bundles onto `to` (#031), which must have none yet. Meter
/// rules move to `to`'s meter with the same name and kind; ones with no match are left out
/// and listed.
pub async fn copy(context: &AssetsContext<'_>, from: AssetId, to: AssetId) -> ScheduleOutcome {
    either(copy_onto(context, from, to).await)
}

async fn copy_onto(
    context: &AssetsContext<'_>,
    from: AssetId,
    to: AssetId,
) -> Result<ScheduleOutcome, ScheduleOutcome> {
    context.schedule_scope(from).await?;
    context.configurable(to).await?;
    if from == to || !context.requirements_for(to).await?.is_empty() {
        return Err(invalid(RequirementProblem::ScheduleNotEmpty));
    }
    let meters = |asset| async move {
        context
            .database
            .meters_of(asset)
            .await
            .map_err(|error| failed("meters", &error))
    };
    let bundles: Vec<Bundle> = context
        .bundles_for(from)
        .await?
        .into_iter()
        .map(|(_, bundle)| bundle)
        .collect();
    let copied = copy_schedule(
        &context.requirements_for(from).await?,
        &bundles,
        &meters(from).await?,
        &meters(to).await?,
        to,
    );
    let mut new_ids: Vec<(RequirementId, RequirementId)> = Vec::new();
    for (old, requirement) in copied.requirements {
        let id: RequirementId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
        context
            .save_requirement(Change::Create {
                id,
                values: RequirementPatch::all(requirement),
            })
            .await?;
        new_ids.push((old, id));
    }
    let copied_bundles = copied.bundles.len();
    for bundle in copied.bundles {
        let requirements = bundle
            .requirements
            .iter()
            .filter_map(|old| {
                new_ids
                    .iter()
                    .find(|(was, _)| was == old)
                    .map(|(_, id)| *id)
            })
            .collect();
        let id: BundleId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
        context
            .save_bundle(Change::Create {
                id,
                values: BundlePatch::all(Bundle {
                    requirements,
                    ..bundle
                }),
            })
            .await?;
    }
    tracing::debug!(target: log_area::APP, %from, %to, requirements = new_ids.len(), bundles = copied_bundles, left_out = copied.left_out.len(), "schedule copied");
    Ok(ScheduleOutcome::Copied {
        requirements: new_ids.len(),
        bundles: copied_bundles,
        left_out: copied.left_out,
    })
}
