//! Household vehicle types on the server (phase-6.md 3e; decision #090).
//!
//! Who may do what:
//! - **See** them: anyone signed in (adding a vehicle offers them, and a vehicle of a type shows
//!   its name and icon).
//! - **Add, change, or remove** one: the household's Admin, in Admin mode, as templates are.
//!
//! Changes go to the server only: on the desktop they're read from its copy, and changing them
//! needs the server. A type's change never touches the vehicles already added as it; a removed
//! type's vehicles keep its ID and show their kind.

use domain::access::{AdminAction, permits_admin};
use domain::api::{TypesOutcome, TypesView};
use domain::command::Change;
use domain::log_area;
use domain::vehicle_type::{TypeProblem, VehicleType, VehicleTypeId, VehicleTypePatch, check_type};
use storage::vehicle_types::VehicleTypeWriter;

use crate::assets::{AssetsContext, unavailable};

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> TypesOutcome {
    let _ = unavailable(what, error);
    TypesOutcome::Unavailable
}

fn either(result: Result<TypesOutcome, TypesOutcome>) -> TypesOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

const fn invalid(problem: TypeProblem) -> TypesOutcome {
    TypesOutcome::Invalid { problem }
}

impl AssetsContext<'_> {
    /// Whether the person may change the household's types: its Admin, in Admin mode.
    fn may_manage_types(&self) -> bool {
        permits_admin(
            &self.user.grants,
            self.user.mode,
            AdminAction::ManageServerSettings,
        )
    }

    async fn kept_types(&self) -> Result<Vec<(VehicleTypeId, VehicleType)>, TypesOutcome> {
        self.database
            .vehicle_types()
            .await
            .map_err(|error| failed("vehicle types", &error))
    }

    /// Checks `kept` (as `id`, if it's kept already) against the others and the templates.
    async fn checked_type(
        &self,
        kept: &VehicleType,
        id: Option<VehicleTypeId>,
    ) -> Result<(), TypesOutcome> {
        if !self.may_manage_types() {
            return Err(TypesOutcome::NotAllowed);
        }
        let others = self.kept_types().await?;
        check_type(kept, id, others.iter().map(|(id, each)| (*id, each))).map_err(invalid)?;
        if let Some(template) = kept.template
            && self
                .database
                .template(template)
                .await
                .map_err(|error| failed("template", &error))?
                .is_none()
        {
            return Err(invalid(TypeProblem::TemplateGone));
        }
        Ok(())
    }
}

/// Manage → Vehicle types: the household's types and templates, and whether the person may
/// change them.
pub async fn list(context: &AssetsContext<'_>) -> TypesOutcome {
    either(list_types(context).await)
}

async fn list_types(context: &AssetsContext<'_>) -> Result<TypesOutcome, TypesOutcome> {
    let templates = context
        .database
        .templates()
        .await
        .map_err(|error| failed("templates", &error))?
        .into_iter()
        .map(|(id, template)| (id, template.name))
        .collect();
    Ok(TypesOutcome::Types {
        view: TypesView {
            types: context.kept_types().await?,
            templates,
            may_manage: context.may_manage_types(),
        },
    })
}

/// Adds `kept`.
pub async fn add(context: &AssetsContext<'_>, kept: VehicleType) -> TypesOutcome {
    either(add_type(context, kept).await)
}

async fn add_type(
    context: &AssetsContext<'_>,
    kept: VehicleType,
) -> Result<TypesOutcome, TypesOutcome> {
    context.checked_type(&kept, None).await?;
    let id: VehicleTypeId = context.new_id().map_err(|_| TypesOutcome::Unavailable)?;
    context
        .save(
            &VehicleTypeWriter,
            Change::Create {
                id,
                values: VehicleTypePatch::all(kept),
            },
        )
        .await
        .map_err(|_| TypesOutcome::Unavailable)?;
    tracing::debug!(target: log_area::APP, vehicle_type = %id, "vehicle type added");
    Ok(TypesOutcome::Saved { id })
}

/// Changes type `id` to `kept` (only what differs is written). Vehicles already added as it
/// stay as they are.
pub async fn edit(
    context: &AssetsContext<'_>,
    id: VehicleTypeId,
    kept: VehicleType,
) -> TypesOutcome {
    either(edit_type(context, id, kept).await)
}

async fn edit_type(
    context: &AssetsContext<'_>,
    id: VehicleTypeId,
    kept: VehicleType,
) -> Result<TypesOutcome, TypesOutcome> {
    if !context.may_manage_types() {
        return Err(TypesOutcome::NotAllowed);
    }
    let before = context
        .database
        .vehicle_type(id)
        .await
        .map_err(|error| failed("vehicle type", &error))?
        .ok_or(TypesOutcome::NotFound)?;
    context.checked_type(&kept, Some(id)).await?;
    let patch = VehicleTypePatch::changes(&before, kept);
    if !domain::command::Patch::changed_fields(&patch).is_empty() {
        context
            .save(&VehicleTypeWriter, Change::Update { id, patch })
            .await
            .map_err(|_| TypesOutcome::Unavailable)?;
    }
    tracing::debug!(target: log_area::APP, vehicle_type = %id, "vehicle type changed");
    Ok(TypesOutcome::Saved { id })
}

/// Removes type `id`. Its vehicles keep their kind, and show it.
pub async fn delete(context: &AssetsContext<'_>, id: VehicleTypeId) -> TypesOutcome {
    either(delete_type(context, id).await)
}

async fn delete_type(
    context: &AssetsContext<'_>,
    id: VehicleTypeId,
) -> Result<TypesOutcome, TypesOutcome> {
    if !context.may_manage_types() {
        return Err(TypesOutcome::NotAllowed);
    }
    context
        .database
        .vehicle_type(id)
        .await
        .map_err(|error| failed("vehicle type", &error))?
        .ok_or(TypesOutcome::NotFound)?;
    context
        .save(&VehicleTypeWriter, Change::<VehicleType>::Delete { id })
        .await
        .map_err(|_| TypesOutcome::Unavailable)?;
    tracing::debug!(target: log_area::APP, vehicle_type = %id, "vehicle type removed");
    Ok(TypesOutcome::Done)
}
