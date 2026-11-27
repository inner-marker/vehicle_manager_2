//! Household schedule templates on the server (Phase 5 milestone 4; decisions #031, #084).
//!
//! Who may do what:
//! - **See** the list: anyone signed in (names and what each holds).
//! - **Save** one from a vehicle: whoever may change that vehicle's schedule (a Maintainer on
//!   it, in Maintainer mode or above). The vehicle needs at least one requirement.
//! - **Apply** one to a vehicle: whoever may change its schedule, and only while it has no
//!   requirements, as Copy schedule. It's copied, never linked: meter rules move to the
//!   vehicle's meters of the same name and kind, and any without one are left out and listed.
//! - **Rename** or **remove** one: the household's Admin, in Admin mode.

use domain::access::{AdminAction, AssetScope, permits_admin};
use domain::api::{ScheduleOutcome, TemplateSummary, TemplatesOutcome, TemplatesView};
use domain::asset::AssetId;
use domain::command::Change;
use domain::label::Label;
use domain::log_area;
use domain::requirement::RequirementProblem;
use domain::template::{ScheduleTemplate, ScheduleTemplatePatch, TemplateId};
use storage::templates::TemplateWriter;

use crate::assets::{AssetsContext, unavailable};

/// A schedule answer as a templates answer.
fn from_schedule(outcome: ScheduleOutcome) -> TemplatesOutcome {
    match outcome {
        ScheduleOutcome::NotFound => TemplatesOutcome::NotFound,
        ScheduleOutcome::NotAllowed => TemplatesOutcome::NotAllowed,
        ScheduleOutcome::Invalid { problem } => TemplatesOutcome::Invalid { problem },
        ScheduleOutcome::Copied {
            requirements,
            bundles,
            specs,
            left_out,
        } => TemplatesOutcome::Applied {
            requirements,
            bundles,
            specs,
            left_out,
        },
        ScheduleOutcome::Schedule { .. }
        | ScheduleOutcome::SavedRequirement { .. }
        | ScheduleOutcome::SavedBundle { .. }
        | ScheduleOutcome::SavedSpec { .. }
        | ScheduleOutcome::SavedUsage { .. }
        | ScheduleOutcome::InvalidUsage { .. }
        | ScheduleOutcome::Done
        | ScheduleOutcome::InvalidSpec { .. }
        | ScheduleOutcome::Unavailable => TemplatesOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> TemplatesOutcome {
    let _ = unavailable(what, error);
    TemplatesOutcome::Unavailable
}

fn either(result: Result<TemplatesOutcome, TemplatesOutcome>) -> TemplatesOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

impl AssetsContext<'_> {
    fn may_manage_templates(&self) -> bool {
        permits_admin(
            &self.user.grants,
            self.user.mode,
            AdminAction::ManageServerSettings,
        )
    }

    async fn save_template(
        &self,
        change: Change<ScheduleTemplate>,
    ) -> Result<(), TemplatesOutcome> {
        self.save(&TemplateWriter, change)
            .await
            .map_err(|_| TemplatesOutcome::Unavailable)
    }

    async fn stored_template(&self, id: TemplateId) -> Result<ScheduleTemplate, TemplatesOutcome> {
        self.database
            .template(id)
            .await
            .map_err(|error| failed("template", &error))?
            .ok_or(TemplatesOutcome::NotFound)
    }
}

/// The Templates screen: the household's templates, the vehicles the person may apply one
/// to, and whether they may rename and remove them.
pub async fn list(context: &AssetsContext<'_>) -> TemplatesOutcome {
    either(list_templates(context).await)
}

async fn list_templates(context: &AssetsContext<'_>) -> Result<TemplatesOutcome, TemplatesOutcome> {
    let templates = context
        .database
        .templates()
        .await
        .map_err(|error| failed("templates", &error))?
        .into_iter()
        .map(|(id, template)| TemplateSummary {
            id,
            counts: template.counts(),
            name: template.name,
        })
        .collect();
    let mut apply_to = Vec::new();
    for (id, asset) in context
        .database
        .assets()
        .await
        .map_err(|error| failed("assets", &error))?
    {
        let scope = AssetScope {
            id,
            group: asset.group,
        };
        if !context.may_configure(scope) {
            continue;
        }
        let empty = context
            .database
            .requirements_of(id)
            .await
            .map_err(|error| failed("requirements", &error))?
            .is_empty();
        if empty {
            apply_to.push((id, asset.name.as_str().to_owned()));
        }
    }
    apply_to.sort_by_key(|(_, name)| name.to_lowercase());
    Ok(TemplatesOutcome::Templates {
        view: TemplatesView {
            templates,
            apply_to,
            may_manage: context.may_manage_templates(),
        },
    })
}

/// Saves vehicle `from`'s requirements, bundles, and specs as a template named `name`.
pub async fn save(context: &AssetsContext<'_>, from: AssetId, name: Label) -> TemplatesOutcome {
    either(save_from(context, from, name).await)
}

async fn save_from(
    context: &AssetsContext<'_>,
    from: AssetId,
    name: Label,
) -> Result<TemplatesOutcome, TemplatesOutcome> {
    context.configurable(from).await.map_err(from_schedule)?;
    let requirements = context
        .requirements_for(from)
        .await
        .map_err(from_schedule)?;
    if requirements.is_empty() {
        return Err(TemplatesOutcome::Invalid {
            problem: RequirementProblem::NothingToSave,
        });
    }
    let bundles: Vec<_> = context
        .bundles_for(from)
        .await
        .map_err(from_schedule)?
        .into_iter()
        .map(|(_, bundle)| bundle)
        .collect();
    let specs = context.specs_for(from).await.map_err(from_schedule)?;
    let meters = context
        .database
        .meters_of(from)
        .await
        .map_err(|error| failed("meters", &error))?;
    let template = ScheduleTemplate::from_vehicle(
        name,
        &meters,
        &requirements,
        &bundles,
        &specs,
        &context
            .database
            .usages_of(from)
            .await
            .map_err(|error| failed("usages", &error))?,
    );
    let id: TemplateId = context
        .new_id()
        .map_err(|_| TemplatesOutcome::Unavailable)?;
    context
        .save_template(Change::Create {
            id,
            values: ScheduleTemplatePatch::all(template),
        })
        .await?;
    tracing::debug!(target: log_area::APP, template = %id, %from, "template saved");
    Ok(TemplatesOutcome::Saved { id })
}

/// Applies template `id` to vehicle `to`, which must have no requirements yet: a copy.
pub async fn apply(context: &AssetsContext<'_>, id: TemplateId, to: AssetId) -> TemplatesOutcome {
    either(apply_to(context, id, to).await)
}

async fn apply_to(
    context: &AssetsContext<'_>,
    id: TemplateId,
    to: AssetId,
) -> Result<TemplatesOutcome, TemplatesOutcome> {
    context.configurable(to).await.map_err(from_schedule)?;
    let template = context.stored_template(id).await?;
    if !context
        .requirements_for(to)
        .await
        .map_err(from_schedule)?
        .is_empty()
    {
        return Err(TemplatesOutcome::Invalid {
            problem: RequirementProblem::ScheduleNotEmpty,
        });
    }
    let meters = context
        .database
        .meters_of(to)
        .await
        .map_err(|error| failed("meters", &error))?;
    let copied = template.apply(&meters, to);
    let outcome = crate::schedule::write_copied(context, copied, &template.usages)
        .await
        .map_err(from_schedule)?;
    tracing::debug!(target: log_area::APP, template = %id, %to, ?outcome, "template applied");
    Ok(from_schedule(outcome))
}

/// Copies template `id` onto `to`, a vehicle just added as a type that names it (#090): its
/// meter rules without a matching meter are left out, and answered. A template removed since
/// copies nothing.
///
/// # Errors
///
/// The vehicles answer to give if it can't be read or saved.
pub(crate) async fn copy_onto_new(
    context: &AssetsContext<'_>,
    id: TemplateId,
    to: AssetId,
) -> Result<Vec<(Label, Label)>, domain::api::AssetsOutcome> {
    let fail = |what: &str, error: &dyn std::fmt::Display| unavailable(what, error);
    let Some(template) = context
        .database
        .template(id)
        .await
        .map_err(|error| fail("template", &error))?
    else {
        return Ok(Vec::new());
    };
    let meters = context
        .database
        .meters_of(to)
        .await
        .map_err(|error| fail("meters", &error))?;
    let copied = template.apply(&meters, to);
    match crate::schedule::write_copied(context, copied, &template.usages).await {
        Ok(ScheduleOutcome::Copied { left_out, .. }) => {
            tracing::debug!(target: log_area::APP, template = %id, %to, "template copied onto a new vehicle");
            Ok(left_out)
        }
        _ => Err(domain::api::AssetsOutcome::Unavailable),
    }
}

/// Renames template `id` (the household's Admin).
pub async fn rename(context: &AssetsContext<'_>, id: TemplateId, name: Label) -> TemplatesOutcome {
    either(rename_template(context, id, name).await)
}

async fn rename_template(
    context: &AssetsContext<'_>,
    id: TemplateId,
    name: Label,
) -> Result<TemplatesOutcome, TemplatesOutcome> {
    if !context.may_manage_templates() {
        return Err(TemplatesOutcome::NotAllowed);
    }
    context.stored_template(id).await?;
    let patch = ScheduleTemplatePatch {
        name: Some(name),
        ..ScheduleTemplatePatch::default()
    };
    context.save_template(Change::Update { id, patch }).await?;
    tracing::debug!(target: log_area::APP, template = %id, "template renamed");
    Ok(TemplatesOutcome::Saved { id })
}

/// Removes template `id` (the household's Admin). Vehicles it was applied to keep their copy.
pub async fn delete(context: &AssetsContext<'_>, id: TemplateId) -> TemplatesOutcome {
    either(delete_template(context, id).await)
}

async fn delete_template(
    context: &AssetsContext<'_>,
    id: TemplateId,
) -> Result<TemplatesOutcome, TemplatesOutcome> {
    if !context.may_manage_templates() {
        return Err(TemplatesOutcome::NotAllowed);
    }
    context.stored_template(id).await?;
    context.save_template(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, template = %id, "template removed");
    Ok(TemplatesOutcome::Done)
}
