//! Events on the server: fuel-ups and readings (milestone 6), and maintenance and repairs
//! (milestone 7; plan §3 Events and Maintenance, #034).
//!
//! Who may do what (plan §6 roles; milestone 6 choice 10, milestone 7 choice 10):
//! - **See** a vehicle's events: any role that covers it.
//! - **Log** a fuel-up or reading: the Driver role on it (or higher), in Driver mode or above;
//!   a maintenance or repair (and its squawk dispositions): the Maintainer role.
//! - **Edit, delete** a draft, **Finalize now** a fuel-up or reading, **Finish** a job, and
//!   **correct** a finalized event: whoever logged it (while they may still log that kind),
//!   or a Maintainer or Admin on the vehicle.
//! - **Keep a reading that doesn't fit** its neighbours in time, with a reason: the Admin role
//!   on the vehicle, in Admin mode (choice 3). The event is flagged.
//!
//! Every event is checked (`domain::event_check`) against the vehicle's meters, fields, and
//! the readings that count, then tidied so it's flagged only for what it really needs.
//! Fuel-ups and readings finalize by themselves 24 hours after their last edit
//! ([`finalize_due`], run by the scheduler; the test minutes with test timers, #054).
//! Maintenance and repairs never do: **Finish** locks one, once every open or deferred squawk
//! on the vehicle is signed off or deferred in it (choice 8). One left untouched for 14 days
//! is a forgotten job, a reminder in Messages ([`forgotten_jobs_for`]).

use domain::access::{AssetAction, AssetScope, permits_on_asset};
use domain::api::{AssetsOutcome, EventView, EventsOutcome, LatestReading};
use domain::command::Change;
use domain::event::{
    Event, EventId, EventInput, EventPatch, EventProblem, EventStatus, Finalizer, Performer,
    Reason, event_changes,
};
use domain::event_check::{
    CheckContext, LocationRule, Warning, check, check_correction, check_finish, finalize, tidy,
};
use domain::id::DeviceId;
use domain::inbox::{Message, forgotten_job_message};
use domain::log_area;
use domain::meter::EventKind;
use domain::profile::event_kinds;
use domain::squawk::{SquawkId, status};
use domain::time::{Clock, UnixMillis};
use domain::timers::Timers;
use storage::events::EventWriter;
use storage::{Database, StorageError};

use crate::assets::{AssetsContext, unavailable};
use crate::commands::{ApplyError, issue};
use crate::health::Health;

/// A vehicle request's refusal as an events answer.
fn from_assets(outcome: &AssetsOutcome) -> EventsOutcome {
    match outcome {
        AssetsOutcome::NotFound => EventsOutcome::NotFound,
        AssetsOutcome::NotAllowed => EventsOutcome::NotAllowed,
        AssetsOutcome::Garage { .. }
        | AssetsOutcome::Asset { .. }
        | AssetsOutcome::Added { .. }
        | AssetsOutcome::MovePreview { .. }
        | AssetsOutcome::Done
        | AssetsOutcome::Invalid { .. }
        | AssetsOutcome::Unavailable => EventsOutcome::Unavailable,
    }
}

/// Logs a failure and answers "unavailable".
fn failed(what: &str, error: &dyn std::fmt::Display) -> EventsOutcome {
    from_assets(&unavailable(what, error))
}

fn invalid(problem: EventProblem) -> EventsOutcome {
    EventsOutcome::Invalid { problem }
}

/// Turns `Result<EventsOutcome, EventsOutcome>` into the outcome either way.
fn either(result: Result<EventsOutcome, EventsOutcome>) -> EventsOutcome {
    result.unwrap_or_else(|outcome| outcome)
}

/// What logging `kind` needs.
const fn log_action(kind: EventKind) -> AssetAction {
    match kind {
        EventKind::Fuel => AssetAction::LogFuel,
        EventKind::Reading => AssetAction::LogReading,
        EventKind::Maintenance | EventKind::Repair => AssetAction::LogMaintenance,
    }
}

impl AssetsContext<'_> {
    fn permits(&self, action: AssetAction, scope: AssetScope) -> bool {
        permits_on_asset(&self.user.grants, self.user.mode, action, scope)
    }

    /// Whether the person may change, finalize, or correct `event`.
    pub(crate) fn may_manage(&self, event: &Event, scope: AssetScope) -> bool {
        self.permits(AssetAction::ManageEvents, scope)
            || (event.logged_by == self.user.user && self.permits(log_action(event.kind), scope))
    }

    /// The event and its vehicle's scope, if the person can see it.
    async fn visible_event(&self, id: EventId) -> Result<(Event, AssetScope), EventsOutcome> {
        let event = self
            .database
            .event(id)
            .await
            .map_err(|error| failed("event", &error))?
            .ok_or(EventsOutcome::NotFound)?;
        let scope = self
            .visible(event.asset)
            .await
            .map_err(|outcome| from_assets(&outcome))?;
        Ok((event, scope))
    }

    /// The event, if the person may change, finalize, or correct it.
    async fn managed_event(&self, id: EventId) -> Result<(Event, AssetScope), EventsOutcome> {
        let (event, scope) = self.visible_event(id).await?;
        if !self.may_manage(&event, scope) {
            return Err(EventsOutcome::NotAllowed);
        }
        Ok((event, scope))
    }

    /// Saves an event change as the person; a finalized event's refusal is a problem.
    async fn save_event(&self, change: Change<Event>) -> Result<(), EventsOutcome> {
        let actor = domain::access::Actor {
            user: self.user.user,
            mode: self.user.mode,
        };
        issue(
            self.database,
            &EventWriter,
            change,
            self.user.device,
            Some(actor),
            self.health,
            self.clock,
        )
        .await
        .map(|_| ())
        .map_err(|error| match error {
            ApplyError::Storage(StorageError::Finalized) => invalid(EventProblem::Finalized),
            other @ (ApplyError::Rejected(_)
            | ApplyError::Storage(_)
            | ApplyError::Clock(_)
            | ApplyError::NewId(_)) => failed("save event", &other),
        })
    }

    /// The vehicle's outstanding squawks (oldest first), leaving `leave_out`'s dispositions
    /// out: the event being checked, and the one it corrects (its correction dispositions
    /// them again).
    async fn outstanding_squawks(
        &self,
        asset: domain::asset::AssetId,
        leave_out: &[EventId],
    ) -> Result<Vec<SquawkId>, EventsOutcome> {
        let mut outstanding: Vec<SquawkId> = self
            .database
            .squawk_states_of(asset)
            .await
            .map_err(|error| failed("squawks", &error))?
            .into_iter()
            .filter(|state| {
                let counted: Vec<_> = state
                    .history
                    .iter()
                    .filter(|counted| !leave_out.contains(&counted.event))
                    .cloned()
                    .collect();
                status(&counted).is_outstanding()
            })
            .map(|state| state.id)
            .collect();
        outstanding.reverse();
        Ok(outstanding)
    }

    /// Checks `event` (leaving `leave_out`'s readings and dispositions out of the history,
    /// and the event it corrects, if any), then tidies it. With `finishing`, the checks are
    /// **Finish**'s.
    async fn checked(
        &self,
        event: &mut Event,
        scope: AssetScope,
        leave_out: &[EventId],
        finishing: bool,
    ) -> Result<Vec<Warning>, EventsOutcome> {
        let mut leave_out = leave_out.to_vec();
        leave_out.extend(event.corrects);
        let asset = self
            .database
            .asset(event.asset)
            .await
            .map_err(|error| failed("asset", &error))?
            .ok_or(EventsOutcome::NotFound)?;
        let meters = self
            .database
            .meters_of(event.asset)
            .await
            .map_err(|error| failed("meters", &error))?;
        let fields = self
            .database
            .custom_fields_of(event.asset)
            .await
            .map_err(|error| failed("fields", &error))?;
        let history: Vec<_> = self
            .database
            .readings_of(event.asset)
            .await
            .map_err(|error| failed("readings", &error))?
            .into_iter()
            .filter(|reading| !leave_out.contains(&reading.event))
            .map(|reading| (reading.meter, reading.at, reading.value))
            .collect();
        let kinds = event_kinds(&asset.kind);
        // Only a job uses the schedule and squawks.
        let (requirements, outstanding_squawks) = if event.kind.finalizes_by_itself() {
            (Vec::new(), Vec::new())
        } else {
            let requirements = self
                .database
                .requirements_of(event.asset)
                .await
                .map_err(|error| failed("requirements", &error))?;
            let outstanding = self.outstanding_squawks(event.asset, &leave_out).await?;
            (requirements, outstanding)
        };
        let context = CheckContext {
            kinds: &kinds,
            meters: &meters,
            fields: &fields,
            history: &history,
            location_rule: LocationRule::Required,
            may_override_readings: self.permits(AssetAction::OverrideReadings, scope),
            requirements: &requirements,
            outstanding_squawks: &outstanding_squawks,
        };
        let warnings = if finishing {
            check_finish(event, &context)
        } else {
            check(event, &context)
        }
        .map_err(invalid)?;
        tidy(event, &context, &warnings);
        self.check_references(event).await?;
        Ok(warnings)
    }

    /// The people, vehicles, and saved place an event names exist (and the person can see
    /// the vehicles). A saved place that's gone, or isn't the person's, is dropped: it's only
    /// a hint, and its values were copied in.
    async fn check_references(&self, event: &mut Event) -> Result<(), EventsOutcome> {
        if let Some(Performer::User { user }) = &event.performed_by
            && self
                .database
                .user(*user)
                .await
                .map_err(|error| failed("user", &error))?
                .is_none()
        {
            return Err(invalid(EventProblem::BadPerformer));
        }
        if let Some(fuel) = &event.fuel {
            for towed in &fuel.towed {
                self.visible(*towed)
                    .await
                    .map_err(|outcome| from_assets(&outcome))?;
            }
        }
        if let Some(saved) = event.location.saved_place {
            let place = self
                .database
                .saved_place(saved)
                .await
                .map_err(|error| failed("saved place", &error))?;
            if place.is_none_or(|place| place.owner != self.user.user) {
                event.location.saved_place = None;
            }
        }
        Ok(())
    }

    /// A person's display name (empty if they're gone), looked up once per request: `names`
    /// keeps the ones already found.
    async fn name_of(
        &self,
        user: domain::user::UserId,
        names: &mut Vec<(domain::user::UserId, String)>,
    ) -> Result<String, EventsOutcome> {
        if let Some((_, name)) = names.iter().find(|(have, _)| *have == user) {
            return Ok(name.clone());
        }
        let name = self
            .database
            .user(user)
            .await
            .map_err(|error| failed("user", &error))?
            .map(|stored| stored.user.display_name)
            .unwrap_or_default();
        names.push((user, name.clone()));
        Ok(name)
    }

    /// The person's view of one event.
    async fn view_of(
        &self,
        id: EventId,
        event: Event,
        scope: AssetScope,
        corrected_by: Option<EventId>,
        names: &mut Vec<(domain::user::UserId, String)>,
    ) -> Result<EventView, EventsOutcome> {
        let manage = self.may_manage(&event, scope);
        let logged_by_name = self.name_of(event.logged_by, names).await?;
        let finalized_by_name = match event.finalized.map(|done| done.by) {
            Some(Finalizer::User(user)) => Some(self.name_of(user, names).await?),
            Some(Finalizer::Server) | None => None,
        };
        Ok(EventView {
            id,
            may_change: manage && event.status == EventStatus::Draft,
            may_correct: manage && event.status == EventStatus::Finalized && corrected_by.is_none(),
            corrected_by,
            logged_by_name,
            finalized_by_name,
            event,
        })
    }
}

// --- Seeing events -----------------------------------------------------------------------

/// One vehicle's events, newest first.
pub async fn list(context: &AssetsContext<'_>, asset: domain::asset::AssetId) -> EventsOutcome {
    either(list_events(context, asset).await)
}

async fn list_events(
    context: &AssetsContext<'_>,
    asset: domain::asset::AssetId,
) -> Result<EventsOutcome, EventsOutcome> {
    let scope = context
        .visible(asset)
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    let events = context
        .database
        .events_of(asset)
        .await
        .map_err(|error| failed("events", &error))?;
    // Corrections are among the vehicle's own events.
    let corrected_by = |original: EventId| {
        events
            .iter()
            .find(|(_, event)| event.corrects == Some(original))
            .map(|(id, _)| *id)
    };
    let mut views = Vec::with_capacity(events.len());
    let mut names = Vec::new();
    for (id, event) in &events {
        views.push(
            context
                .view_of(*id, event.clone(), scope, corrected_by(*id), &mut names)
                .await?,
        );
    }
    Ok(EventsOutcome::Events { events: views })
}

/// One event.
pub async fn view(context: &AssetsContext<'_>, id: EventId) -> EventsOutcome {
    either(view_event(context, id).await)
}

async fn view_event(
    context: &AssetsContext<'_>,
    id: EventId,
) -> Result<EventsOutcome, EventsOutcome> {
    let (event, scope) = context.visible_event(id).await?;
    let corrected_by = context
        .database
        .correction_of(id)
        .await
        .map_err(|error| failed("correction", &error))?;
    Ok(EventsOutcome::Event {
        view: Box::new(
            context
                .view_of(id, event, scope, corrected_by, &mut Vec::new())
                .await?,
        ),
    })
}

/// What the log form needs: each meter's latest reading, and whether the person may keep a
/// reading that doesn't fit.
pub async fn log_context(
    context: &AssetsContext<'_>,
    asset: domain::asset::AssetId,
) -> EventsOutcome {
    either(log_context_of(context, asset).await)
}

async fn log_context_of(
    context: &AssetsContext<'_>,
    asset: domain::asset::AssetId,
) -> Result<EventsOutcome, EventsOutcome> {
    let scope = context
        .visible(asset)
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    let latest = context
        .database
        .latest_readings(asset)
        .await
        .map_err(|error| failed("latest readings", &error))?
        .into_iter()
        .map(|reading| LatestReading {
            meter: reading.meter,
            at: reading.at,
            value: reading.value,
        })
        .collect();
    Ok(EventsOutcome::LogContext {
        latest,
        may_override_readings: context.permits(AssetAction::OverrideReadings, scope),
    })
}

// --- Logging and changing ----------------------------------------------------------------

/// Logs an event, as a draft.
pub async fn add(context: &AssetsContext<'_>, input: EventInput) -> EventsOutcome {
    either(add_event(context, input).await)
}

async fn add_event(
    context: &AssetsContext<'_>,
    input: EventInput,
) -> Result<EventsOutcome, EventsOutcome> {
    let scope = context
        .visible(input.asset)
        .await
        .map_err(|outcome| from_assets(&outcome))?;
    if !context.permits(log_action(input.kind), scope) {
        return Err(EventsOutcome::NotAllowed);
    }
    let kind = input.kind;
    let mut event = input.into_draft(context.user.user, context.clock.now());
    let warnings = context.checked(&mut event, scope, &[], false).await?;
    let id: EventId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_event(Change::Create {
            id,
            values: EventPatch::all(event),
        })
        .await?;
    tracing::debug!(target: log_area::APP, event = %id, ?kind, "event logged");
    Ok(EventsOutcome::Saved { id, warnings })
}

/// Changes a draft.
pub async fn edit(context: &AssetsContext<'_>, id: EventId, input: EventInput) -> EventsOutcome {
    either(edit_event(context, id, input).await)
}

async fn edit_event(
    context: &AssetsContext<'_>,
    id: EventId,
    input: EventInput,
) -> Result<EventsOutcome, EventsOutcome> {
    let (current, scope) = context.managed_event(id).await?;
    if current.status == EventStatus::Finalized {
        return Err(invalid(EventProblem::Finalized));
    }
    let mut event = input.edit(&current, context.clock.now());
    let warnings = context.checked(&mut event, scope, &[id], false).await?;
    let tasks = event.work.as_ref().map_or(0, |work| work.tasks.len());
    context
        .save_event(Change::Update {
            id,
            patch: event_changes(&current, &event),
        })
        .await?;
    crate::attachments::settle_tasks(context, current.asset, id, tasks)
        .await
        .map_err(|_| EventsOutcome::Unavailable)?;
    Ok(EventsOutcome::Saved { id, warnings })
}

/// **Finalize now**: locks a fuel-up or reading draft (#034). A job is finished instead
/// ([`finish`]).
pub async fn finalize_now(context: &AssetsContext<'_>, id: EventId) -> EventsOutcome {
    either(finalize_event(context, id).await)
}

async fn finalize_event(
    context: &AssetsContext<'_>,
    id: EventId,
) -> Result<EventsOutcome, EventsOutcome> {
    let (current, _) = context.managed_event(id).await?;
    if current.status == EventStatus::Finalized {
        return Err(invalid(EventProblem::Finalized));
    }
    if !current.kind.finalizes_by_itself() {
        return Err(invalid(EventProblem::NotAJob));
    }
    context
        .save_event(Change::Update {
            id,
            patch: finalize(Finalizer::User(context.user.user), context.clock.now()),
        })
        .await?;
    tracing::debug!(target: log_area::APP, event = %id, "event finalized");
    Ok(EventsOutcome::Done)
}

/// **Finish**: locks a maintenance or repair draft once it passes Finish's checks (choice 8):
/// everything a save checks, and every outstanding squawk signed off or deferred in it.
pub async fn finish(context: &AssetsContext<'_>, id: EventId) -> EventsOutcome {
    either(finish_event(context, id).await)
}

async fn finish_event(
    context: &AssetsContext<'_>,
    id: EventId,
) -> Result<EventsOutcome, EventsOutcome> {
    let (current, scope) = context.managed_event(id).await?;
    if current.status == EventStatus::Finalized {
        return Err(invalid(EventProblem::Finalized));
    }
    if current.kind.finalizes_by_itself() {
        return Err(invalid(EventProblem::NotAJob));
    }
    // Checked as stored; a save already tidied it.
    let mut event = current;
    context.checked(&mut event, scope, &[id], true).await?;
    context
        .save_event(Change::Update {
            id,
            patch: finalize(Finalizer::User(context.user.user), context.clock.now()),
        })
        .await?;
    tracing::debug!(target: log_area::APP, event = %id, "job finished");
    Ok(EventsOutcome::Done)
}

/// Deletes a draft (a mistake); a finalized event can only be corrected.
pub async fn delete(context: &AssetsContext<'_>, id: EventId) -> EventsOutcome {
    either(delete_event(context, id).await)
}

async fn delete_event(
    context: &AssetsContext<'_>,
    id: EventId,
) -> Result<EventsOutcome, EventsOutcome> {
    let (current, _) = context.managed_event(id).await?;
    if current.status == EventStatus::Finalized {
        return Err(invalid(EventProblem::Finalized));
    }
    context.save_event(Change::Delete { id }).await?;
    Ok(EventsOutcome::Done)
}

/// Corrects a finalized event: a new draft, for the same vehicle and kind, naming the original
/// with a reason (choice 2). The correction's values count instead of the original's.
pub async fn correct(
    context: &AssetsContext<'_>,
    original: EventId,
    input: EventInput,
    reason: &str,
) -> EventsOutcome {
    either(correct_event(context, original, input, reason).await)
}

async fn correct_event(
    context: &AssetsContext<'_>,
    original_id: EventId,
    input: EventInput,
    reason: &str,
) -> Result<EventsOutcome, EventsOutcome> {
    let (original, scope) = context.managed_event(original_id).await?;
    let reason = Reason::parse(reason).map_err(invalid)?;
    let mut event = input.into_draft(context.user.user, context.clock.now());
    event.corrects = Some(original_id);
    event.correction_reason = Some(reason);
    check_correction(&original, original_id, &event).map_err(invalid)?;
    if context
        .database
        .correction_of(original_id)
        .await
        .map_err(|error| failed("correction", &error))?
        .is_some()
    {
        return Err(invalid(EventProblem::AlreadyCorrected));
    }
    let warnings = context
        .checked(&mut event, scope, &[original_id], false)
        .await?;
    let id: EventId = context.new_id().map_err(|outcome| from_assets(&outcome))?;
    context
        .save_event(Change::Create {
            id,
            values: EventPatch::all(event),
        })
        .await?;
    // The correction starts with the original's photos and documents (choice 6).
    crate::attachments::copy_to_correction(context, original.asset, original_id, id)
        .await
        .map_err(|_| EventsOutcome::Unavailable)?;
    tracing::debug!(target: log_area::APP, event = %id, original = %original_id, "event corrected");
    Ok(EventsOutcome::Saved { id, warnings })
}

// --- Forgotten jobs ----------------------------------------------------------------------

/// The forgotten-job reminders (#034) for the signed-in person: maintenance and repair drafts
/// untouched for the delay (14 days, or the test minutes), on vehicles they can see, that they
/// logged or may manage.
///
/// # Errors
///
/// Unavailable if the drafts or their vehicles can't be read.
pub async fn forgotten_jobs_for(
    context: &AssetsContext<'_>,
    timers: Timers,
) -> Result<Vec<Message>, EventsOutcome> {
    let delay = timers.forgotten_after_millis();
    let cutoff = UnixMillis::from_millis(context.clock.now().as_millis() - delay);
    let forgotten = context
        .database
        .forgotten_jobs(cutoff)
        .await
        .map_err(|error| failed("forgotten jobs", &error))?;
    let mut messages = Vec::new();
    for id in forgotten {
        let Some(event) = context
            .database
            .event(id)
            .await
            .map_err(|error| failed("event", &error))?
        else {
            continue;
        };
        // Someone who can't see the vehicle (any more) gets nothing.
        let Ok(scope) = context.visible(event.asset).await else {
            continue;
        };
        if !context.may_manage(&event, scope) {
            continue;
        }
        let vehicle = context
            .database
            .asset(event.asset)
            .await
            .map_err(|error| failed("asset", &error))?
            .map(|asset| asset.name.as_str().to_owned())
            .unwrap_or_default();
        messages.push(forgotten_job_message(id, &event, &vehicle, delay));
    }
    Ok(messages)
}

// --- The automatic finalize --------------------------------------------------------------

/// Finalizes every fuel-up and reading draft last edited at least the finalize delay before
/// `now` (24 hours, or the test minutes), as the server. Returns how many were finalized.
///
/// # Errors
///
/// A message for the job's record if the drafts can't be read or one can't be finalized (the
/// others are still tried).
pub async fn finalize_due(
    database: &Database,
    device: DeviceId,
    health: Health,
    clock: &(dyn Clock + Sync),
    timers: Timers,
) -> Result<usize, String> {
    let now = clock.now();
    let cutoff = UnixMillis::from_millis(now.as_millis() - timers.finalize_after_millis());
    let due = database
        .drafts_edited_before(cutoff)
        .await
        .map_err(|error| format!("can't read the drafts: {error}"))?;
    let mut failures = Vec::new();
    for id in &due {
        let change = Change::Update {
            id: *id,
            patch: finalize(Finalizer::Server, now),
        };
        if let Err(error) = issue(database, &EventWriter, change, device, None, health, clock).await
        {
            failures.push(format!("{id}: {error}"));
        }
    }
    if failures.is_empty() {
        Ok(due.len())
    } else {
        Err(format!("couldn't finalize {}", failures.join("; ")))
    }
}
