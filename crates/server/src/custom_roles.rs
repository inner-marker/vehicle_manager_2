//! A household's custom roles on the server (phase-6.md 4c; sub-plan 05; decision #091).
//!
//! Who may do what:
//! - **See** them: anyone signed in (a person sees the roles they hold and what they allow).
//! - **Add, change, or remove** one: the household's Admin, in Admin mode, as users and their
//!   roles are. Server only: the desktop reads them from its copy.
//!
//! A change to a role's permissions reaches everyone who holds it at once (grants are read
//! with their role's permissions). Removing a role removes its grants.

use domain::api::{RoleSummary, RolesOutcome, RolesView};
use domain::command::Change;
use domain::custom_role::{CustomRole, CustomRoleId, CustomRolePatch, check_custom_role};
use domain::log_area;
use storage::custom_roles::CustomRoleWriter;
use storage::users::RoleGrantWriter;

use crate::commands::issue;
use crate::ids::new_id;
use crate::user_admin::AdminContext;

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> RolesOutcome {
    tracing::error!(target: log_area::AUTH, %error, what, "custom roles failed");
    RolesOutcome::Unavailable
}

fn either(result: Result<RolesOutcome, RolesOutcome>) -> RolesOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

impl AdminContext<'_> {
    async fn kept_roles(&self) -> Result<Vec<(CustomRoleId, CustomRole)>, RolesOutcome> {
        self.database
            .custom_roles()
            .await
            .map_err(|error| failed("custom roles", &error))
    }

    fn managing(&self) -> Result<(), RolesOutcome> {
        if self.allowed() {
            Ok(())
        } else {
            Err(RolesOutcome::NotAllowed)
        }
    }

    async fn save_role(&self, change: Change<CustomRole>) -> Result<(), RolesOutcome> {
        issue(
            self.database,
            &CustomRoleWriter,
            change,
            self.admin.device,
            Some(domain::access::Actor {
                user: self.admin.user,
                mode: self.admin.mode,
            }),
            self.health,
            self.clock,
        )
        .await
        .map(|_| ())
        .map_err(|error| failed("save", &error))
    }
}

/// Manage → Roles: the household's custom roles, who holds each, and whether the person may
/// change them.
pub async fn list(context: &AdminContext<'_>) -> RolesOutcome {
    either(list_roles(context).await)
}

async fn list_roles(context: &AdminContext<'_>) -> Result<RolesOutcome, RolesOutcome> {
    let grants = context
        .database
        .all_grants()
        .await
        .map_err(|error| failed("grants", &error))?;
    let roles = context
        .kept_roles()
        .await?
        .into_iter()
        .map(|(id, role)| {
            let mut holders: Vec<_> = grants
                .iter()
                .filter(|(_, grant)| grant.custom.is_some_and(|custom| custom.role == id))
                .map(|(_, grant)| grant.user)
                .collect();
            holders.sort_unstable();
            holders.dedup();
            RoleSummary {
                id,
                role,
                holders: holders.len(),
            }
        })
        .collect();
    Ok(RolesOutcome::Roles {
        view: RolesView {
            roles,
            may_manage: context.allowed(),
        },
    })
}

/// Adds `role`.
pub async fn add(context: &AdminContext<'_>, role: CustomRole) -> RolesOutcome {
    either(add_role(context, role).await)
}

async fn add_role(
    context: &AdminContext<'_>,
    role: CustomRole,
) -> Result<RolesOutcome, RolesOutcome> {
    context.managing()?;
    let kept = context.kept_roles().await?;
    check_custom_role(&role, None, kept.iter().map(|(id, role)| (*id, role)))
        .map_err(|problem| RolesOutcome::Invalid { problem })?;
    let id: CustomRoleId = new_id(context.clock.now()).map_err(|_| RolesOutcome::Unavailable)?;
    context
        .save_role(Change::Create {
            id,
            values: CustomRolePatch::all(role),
        })
        .await?;
    tracing::info!(target: log_area::AUTH, role = %id, by = %context.admin.user, "custom role added");
    Ok(RolesOutcome::Saved { id })
}

/// Changes role `id` to `role` (only what differs is written); everyone holding it gets the
/// new permissions.
pub async fn edit(context: &AdminContext<'_>, id: CustomRoleId, role: CustomRole) -> RolesOutcome {
    either(edit_role(context, id, role).await)
}

async fn edit_role(
    context: &AdminContext<'_>,
    id: CustomRoleId,
    role: CustomRole,
) -> Result<RolesOutcome, RolesOutcome> {
    context.managing()?;
    let kept = context.kept_roles().await?;
    let Some((_, before)) = kept.iter().find(|(kept_id, _)| *kept_id == id) else {
        return Err(RolesOutcome::NotFound);
    };
    check_custom_role(&role, Some(id), kept.iter().map(|(id, role)| (*id, role)))
        .map_err(|problem| RolesOutcome::Invalid { problem })?;
    let patch = CustomRolePatch::changes(before, role);
    if !domain::command::Patch::changed_fields(&patch).is_empty() {
        context.save_role(Change::Update { id, patch }).await?;
    }
    tracing::info!(target: log_area::AUTH, role = %id, by = %context.admin.user, "custom role changed");
    Ok(RolesOutcome::Saved { id })
}

/// Removes role `id` and every grant of it.
pub async fn delete(context: &AdminContext<'_>, id: CustomRoleId) -> RolesOutcome {
    either(delete_role(context, id).await)
}

async fn delete_role(
    context: &AdminContext<'_>,
    id: CustomRoleId,
) -> Result<RolesOutcome, RolesOutcome> {
    context.managing()?;
    if !context
        .kept_roles()
        .await?
        .iter()
        .any(|(kept_id, _)| *kept_id == id)
    {
        return Err(RolesOutcome::NotFound);
    }
    let grants = context
        .database
        .all_grants()
        .await
        .map_err(|error| failed("grants", &error))?;
    for (grant_id, _) in grants
        .iter()
        .filter(|(_, grant)| grant.custom.is_some_and(|custom| custom.role == id))
    {
        issue(
            context.database,
            &RoleGrantWriter,
            Change::<domain::access::RoleGrant>::Delete { id: *grant_id },
            context.admin.device,
            Some(domain::access::Actor {
                user: context.admin.user,
                mode: context.admin.mode,
            }),
            context.health,
            context.clock,
        )
        .await
        .map_err(|error| failed("grant", &error))?;
    }
    context
        .save_role(Change::<CustomRole>::Delete { id })
        .await?;
    tracing::info!(target: log_area::AUTH, role = %id, by = %context.admin.user, "custom role removed");
    Ok(RolesOutcome::Done)
}
