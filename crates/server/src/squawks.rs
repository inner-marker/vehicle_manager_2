//! Squawks on the server (milestone 7; plan §3 Squawks; decisions #051, #063).
//!
//! Who may do what (choice 10):
//! - **See** a vehicle's squawks: any role that covers it.
//! - **Open** one: the Driver role on it (or higher), in Driver mode or above. One found on an
//!   event names it (an event of the same vehicle).
//! - **Change or remove** one: whoever opened it (while they may still open squawks), or a
//!   Maintainer or Admin on the vehicle, **only until a job dispositions it** (in a draft or
//!   a finished one). After that its history stays, and the next job can change its severity.
//!
//! Dispositions are made inside maintenance and repair events ([`crate::events`]).

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::api::{AssetsOutcome, SquawkInput, SquawkView, SquawksOutcome};
use domain::asset::AssetId;
use domain::command::Change;
use domain::event::{Event, EventId};
use domain::log_area;
use domain::squawk::{Squawk, SquawkId, SquawkPatch, SquawkProblem, SquawkStatus};
use storage::squawks::{SquawkState, SquawkWriter};

use crate::assets::{AssetsContext, unavailable};

/// A vehicle request's refusal as a squawks answer.
fn from_assets(outcome: &AssetsOutcome) -> SquawksOutcome {
    match outcome {
        AssetsOutcome::NotFound => SquawksOutcome::NotFound,
        AssetsOutcome::NotAllowed => SquawksOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => SquawksOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> SquawksOutcome {
    from_assets(&unavailable(what, error))
}

/// Turns `Result<SquawksOutcome, SquawksOutcome>` into the outcome either way.
fn either(result: Result<SquawksOutcome, SquawksOutcome>) -> SquawksOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

/// Whether any of `events` (drafts too) dispositions `squawk`.
fn dispositioned_in(events: &[(EventId, Event)], squawk: SquawkId) -> bool {
    events.iter().any(|(_, event)| {
        event.work.as_ref().is_some_and(|work| {
            work.dispositions
                .iter()
                .any(|disposition| disposition.squawk == squawk)
        })
    })
}

impl AssetsContext<'_> {
    fn permits_squawk(&self, action: AssetAction, scope: AssetScope) -> bool {
        permits_on_asset(&self.user.grants, self.user.mode, action, scope)
    }

    /// Whether the person may change or remove `squawk` (ignoring its history).
    pub(crate) fn may_change_squawk(&self, squawk: &Squawk, scope: AssetScope) -> bool {
        self.permits_squawk(AssetAction::ManageEvents, scope)
            || (squawk.opened_by == self.user.user
                && self.permits_squawk(AssetAction::OpenSquawk, scope))
    }

    /// The vehicle's scope, if the person can see it.
    async fn squawk_scope(&self, asset: AssetId) -> Result<AssetScope, SquawksOutcome> {
        self.visible(asset)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }

    /// One vehicle's events (for whether a job has dealt with a squawk).
    async fn events_for_squawks(
        &self,
        asset: AssetId,
    ) -> Result<Vec<(EventId, Event)>, SquawksOutcome> {
        self.database
            .events_of(asset)
            .await
            .map_err(|error| failed("events", &error))
    }

    /// The person's view of one squawk.
    async fn squawk_view(
        &self,
        state: SquawkState,
        scope: AssetScope,
        events: &[(EventId, Event)],
    ) -> Result<SquawkView, SquawksOutcome> {
        let opened_by_name = self
            .database
            .user(state.squawk.opened_by)
            .await
            .map_err(|error| failed("user", &error))?
            .map(|stored| stored.user.display_name)
            .unwrap_or_default();
        let untouched = state.status() == SquawkStatus::Open && !dispositioned_in(events, state.id);
        Ok(SquawkView {
            id: state.id,
            status: state.status(),
            severity: state.severity(),
            may_change: untouched && self.may_change_squawk(&state.squawk, scope),
            opened_by_name,
            squawk: state.squawk,
            history: state.history,
        })
    }

    /// The squawk and its vehicle's scope, if the person may change it now.
    async fn changeable_squawk(
        &self,
        id: SquawkId,
    ) -> Result<(Squawk, AssetScope), SquawksOutcome> {
        let squawk = self
            .database
            .squawk(id)
            .await
            .map_err(|error| failed("squawk", &error))?
            .ok_or(SquawksOutcome::NotFound)?;
        let scope = self.squawk_scope(squawk.asset).await?;
        if !self.may_change_squawk(&squawk, scope) {
            return Err(SquawksOutcome::NotAllowed);
        }
        let events = self.events_for_squawks(squawk.asset).await?;
        if dispositioned_in(&events, id) {
            return Err(SquawksOutcome::Invalid {
                problem: SquawkProblem::Dispositioned,
            });
        }
        Ok((squawk, scope))
    }

    /// Saves a squawk change as the person.
    async fn save_squawk(&self, change: Change<Squawk>) -> Result<(), SquawksOutcome> {
        self.save(&SquawkWriter, change)
            .await
            .map_err(|outcome| from_assets(&outcome))
    }
}

// --- Seeing squawks ----------------------------------------------------------------------

/// One vehicle's squawks, newest first.
pub async fn list(context: &AssetsContext<'_>, asset: AssetId) -> SquawksOutcome {
    either(list_squawks(context, asset).await)
}

async fn list_squawks(
    context: &AssetsContext<'_>,
    asset: AssetId,
) -> Result<SquawksOutcome, SquawksOutcome> {
    let scope = context.squawk_scope(asset).await?;
    let states = context
        .database
        .squawk_states_of(asset)
        .await
        .map_err(|error| failed("squawks", &error))?;
    let events = context.events_for_squawks(asset).await?;
    let mut squawks = Vec::with_capacity(states.len());
    for state in states {
        squawks.push(context.squawk_view(state, scope, &events).await?);
    }
    Ok(SquawksOutcome::Squawks {
        squawks,
        // Not once the vehicle's disposal is finalized (#067).
        may_open: context.permits_squawk(AssetAction::OpenSquawk, scope)
            && crate::lifecycle::takes_new_records(context.database, asset)
                .await
                .map_err(|error| failed("lifecycle", &error))?,
    })
}

/// One squawk, with its history.
pub async fn view(context: &AssetsContext<'_>, id: SquawkId) -> SquawksOutcome {
    either(view_squawk(context, id).await)
}

async fn view_squawk(
    context: &AssetsContext<'_>,
    id: SquawkId,
) -> Result<SquawksOutcome, SquawksOutcome> {
    let squawk = context
        .database
        .squawk(id)
        .await
        .map_err(|error| failed("squawk", &error))?
        .ok_or(SquawksOutcome::NotFound)?;
    let scope = context.squawk_scope(squawk.asset).await?;
    let state = context
        .database
        .squawk_states_of(squawk.asset)
        .await
        .map_err(|error| failed("squawks", &error))?
        .into_iter()
        .find(|state| state.id == id)
        .ok_or(SquawksOutcome::NotFound)?;
    let events = context.events_for_squawks(squawk.asset).await?;
    Ok(SquawksOutcome::Squawk {
        view: Box::new(context.squawk_view(state, scope, &events).await?),
    })
}

// --- Opening and changing ----------------------------------------------------------------

/// Opens a squawk.
pub async fn open(context: &AssetsContext<'_>, input: SquawkInput) -> SquawksOutcome {
    either(open_squawk(context, input).await)
}

async fn open_squawk(
    context: &AssetsContext<'_>,
    input: SquawkInput,
) -> Result<SquawksOutcome, SquawksOutcome> {
    let scope = context.squawk_scope(input.asset).await?;
    if !context.permits_squawk(AssetAction::OpenSquawk, scope) {
        return Err(SquawksOutcome::NotAllowed);
    }
    // Nothing new once the vehicle's disposal is finalized (#067).
    if !crate::lifecycle::takes_new_records(context.database, input.asset)
        .await
        .map_err(|error| failed("lifecycle", &error))?
    {
        return Err(SquawksOutcome::NotAllowed);
    }
    if let Some(event) = input.found_on {
        let found_on = context
            .database
            .event(event)
            .await
            .map_err(|error| failed("event", &error))?;
        if found_on.is_none_or(|found_on| found_on.asset != input.asset) {
            return Err(SquawksOutcome::NotFound);
        }
    }
    let now = context.clock.now();
    let squawk = Squawk {
        asset: input.asset,
        description: input.description,
        notes: input.notes,
        severity: input.severity,
        opened_at: input.opened_at,
        opened_by: context.user.user,
        found_on: input.found_on,
        received_at: now,
    };
    let id: SquawkId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_squawk(Change::Create {
            id,
            values: SquawkPatch::all(squawk),
        })
        .await?;
    tracing::debug!(target: log_area::APP, squawk = %id, asset = %input.asset, severity = ?input.severity, "squawk opened");
    Ok(SquawksOutcome::Saved { id })
}

/// Changes a squawk's description, notes, severity, or when it was noticed (until a job
/// dispositions it). Its vehicle and the event it was found on stay as they were.
pub async fn edit(context: &AssetsContext<'_>, id: SquawkId, input: SquawkInput) -> SquawksOutcome {
    either(edit_squawk(context, id, input).await)
}

async fn edit_squawk(
    context: &AssetsContext<'_>,
    id: SquawkId,
    input: SquawkInput,
) -> Result<SquawksOutcome, SquawksOutcome> {
    let (current, _) = context.changeable_squawk(id).await?;
    // Only what differs, so the fields' clocks show what really changed.
    let patch = input.changes_from(&current);
    context.save_squawk(Change::Update { id, patch }).await?;
    tracing::debug!(target: log_area::APP, squawk = %id, "squawk changed");
    Ok(SquawksOutcome::Saved { id })
}

/// Removes a squawk opened by mistake (until a job dispositions it).
pub async fn delete(context: &AssetsContext<'_>, id: SquawkId) -> SquawksOutcome {
    either(delete_squawk(context, id).await)
}

async fn delete_squawk(
    context: &AssetsContext<'_>,
    id: SquawkId,
) -> Result<SquawksOutcome, SquawksOutcome> {
    context.changeable_squawk(id).await?;
    context.save_squawk(Change::Delete { id }).await?;
    tracing::debug!(target: log_area::APP, squawk = %id, "squawk removed");
    Ok(SquawksOutcome::Done)
}
