//! Groups on the server (decision #052): the default group, and adding, changing, and removing
//! groups (the household's Admin, in Admin mode).
//!
//! Every vehicle is in exactly one group, so a group with vehicles can't be removed, and
//! neither can the last group.

use domain::access::AdminAction;
use domain::api::AssetsOutcome;
use domain::asset_form::{GroupForm, Problem};
use domain::command::Change;
use domain::group::{Group, GroupIcon, GroupId, GroupName, GroupPatch};
use domain::i18n::{Translator, messages};
use domain::id::DeviceId;
use domain::log_area;
use domain::time::Clock;
use storage::Database;
use storage::assets::GroupWriter;

use crate::assets::{AssetsContext, unavailable};
use crate::commands::issue;
use crate::health::Health;
use crate::ids::new_id;

/// Makes the default group ("Garage", in the household's language) if there are no groups
/// yet, so every vehicle can have one. Runs at startup; does nothing once a group exists.
///
/// # Errors
///
/// A description of what failed (the caller logs it and carries on; vehicles can't be added
/// until it succeeds).
pub async fn ensure_default_group(
    database: &Database,
    device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
) -> Result<(), String> {
    let groups = database.groups().await.map_err(|error| error.to_string())?;
    if !groups.is_empty() {
        return Ok(());
    }
    let name = GroupName::parse(&Translator::english().tr(messages::GROUP_DEFAULT_NAME))
        .map_err(|error| format!("{error:?}"))?;
    let group = Group {
        name,
        icon: GroupIcon::Garage,
    };
    let id: GroupId = new_id(clock.now()).map_err(|error| error.to_string())?;
    issue(
        database,
        &GroupWriter,
        Change::Create {
            id,
            values: GroupPatch::all(group),
        },
        device,
        None,
        health,
        clock,
    )
    .await
    .map_err(|error| error.to_string())?;
    tracing::info!(target: log_area::APP, group = %id, "default group made");
    Ok(())
}

fn invalid(problem: Problem) -> AssetsOutcome {
    AssetsOutcome::Invalid { problem }
}

/// Adds a group.
pub async fn add_group(context: &AssetsContext<'_>, form: &GroupForm) -> AssetsOutcome {
    if !context.may(AdminAction::ManageGroups) {
        return AssetsOutcome::NotAllowed;
    }
    let group = match form.parse() {
        Ok(group) => group,
        Err(problem) => return invalid(problem),
    };
    let id: GroupId = match context.new_id() {
        Ok(id) => id,
        Err(outcome) => return outcome,
    };
    let values = GroupPatch::all(group);
    match context
        .save(&GroupWriter, Change::Create { id, values })
        .await
    {
        Ok(()) => {
            tracing::info!(target: log_area::APP, user = %context.user.user, group = %id, "group added");
            AssetsOutcome::Done
        }
        Err(outcome) => outcome,
    }
}

/// Changes a group's name or icon.
pub async fn edit_group(
    context: &AssetsContext<'_>,
    id: GroupId,
    form: &GroupForm,
) -> AssetsOutcome {
    if !context.may(AdminAction::ManageGroups) {
        return AssetsOutcome::NotAllowed;
    }
    let wanted = match form.parse() {
        Ok(group) => group,
        Err(problem) => return invalid(problem),
    };
    let current = match find(context, id).await {
        Ok(group) => group,
        Err(outcome) => return outcome,
    };
    let patch = GroupPatch {
        name: (wanted.name != current.name).then_some(wanted.name),
        icon: (wanted.icon != current.icon).then_some(wanted.icon),
    };
    if patch.name.is_none() && patch.icon.is_none() {
        return AssetsOutcome::Done;
    }
    match context
        .save(&GroupWriter, Change::Update { id, patch })
        .await
    {
        Ok(()) => AssetsOutcome::Done,
        Err(outcome) => outcome,
    }
}

/// Removes an empty group (never the last one).
pub async fn delete_group(context: &AssetsContext<'_>, id: GroupId) -> AssetsOutcome {
    if !context.may(AdminAction::ManageGroups) {
        return AssetsOutcome::NotAllowed;
    }
    match removable(context, id).await {
        Ok(()) => {}
        Err(outcome) => return outcome,
    }
    match context
        .save(&GroupWriter, Change::<Group>::Delete { id })
        .await
    {
        Ok(()) => {
            tracing::info!(target: log_area::APP, user = %context.user.user, group = %id, "group removed");
            AssetsOutcome::Done
        }
        Err(outcome) => outcome,
    }
}

/// Checks a group exists, is empty, and isn't the last one.
async fn removable(context: &AssetsContext<'_>, id: GroupId) -> Result<(), AssetsOutcome> {
    let groups = context
        .database
        .groups()
        .await
        .map_err(|error| unavailable("groups", &error))?;
    if !groups.iter().any(|(group, _)| *group == id) {
        return Err(AssetsOutcome::NotFound);
    }
    if groups.len() == 1 {
        return Err(invalid(Problem::LastGroup));
    }
    let vehicles = context
        .database
        .assets_in_group(id)
        .await
        .map_err(|error| unavailable("assets in group", &error))?;
    if vehicles > 0 {
        return Err(invalid(Problem::GroupNotEmpty));
    }
    Ok(())
}

/// A group, if it exists.
async fn find(context: &AssetsContext<'_>, id: GroupId) -> Result<Group, AssetsOutcome> {
    context
        .database
        .groups()
        .await
        .map_err(|error| unavailable("groups", &error))?
        .into_iter()
        .find_map(|(group_id, group)| (group_id == id).then_some(group))
        .ok_or(AssetsOutcome::NotFound)
}
