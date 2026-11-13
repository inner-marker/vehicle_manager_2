//! One data layer for screens' reads (phase-3.md 4d, decision #075).
//!
//! Each function here has the same name and answer as the server function in `crate::api`
//! that it stands in for. **On the web** it is that server function. **On a desktop** with a
//! complete copy of its server's records (`crate::local_copy`), it runs the server crate's own
//! read code against the copy, as the signed-in person in their current mode, so the same
//! access checks apply (#069); online, the copy catches up first, so a change just made on the
//! server shows. Without a complete copy (the first download hasn't finished, or the server's
//! schema version differs), the desktop asks the server as before.
//!
//! **Writes** (phase-3.md 5d, decision #076): the changes Driver mode allows (events, squawks,
//! the person's preferences, saved places, and Garage order) are made the same way. On a
//! desktop with a complete copy, each becomes an operation: added to the outbox, applied to
//! the copy with the server's own write code (so a mistake shows in the form at once, offline
//! too), then sent; online the form waits for the server's answer, offline it waits in the
//! outbox. Everything else, and the web, asks the server.
//!
//! Messages, Users, Devices, place lookup, and files stay with the server for now.

use dioxus::prelude::*;
use domain::api::{
    AssetsOutcome, AttachmentsOutcome, DocumentsOutcome, EventsOutcome, PlacesOutcome,
    PreferencesOutcome, ReferencesOutcome, ScheduleOutcome, SquawkInput, SquawksOutcome,
};
use domain::asset::AssetId;
use domain::attachment::AttachmentOwner;
use domain::dates::CalendarDate;
use domain::event::EventId;
use domain::event::EventInput;
use domain::location::{SavedPlaceForm, SavedPlaceId};
use domain::preferences::PreferencesPatch;
use domain::reference::ReferenceDocumentId;
use domain::squawk::SquawkId;

use crate::api;

/// Reading the copy on a desktop.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod local {
    use ::server::assets::AssetsContext;
    use ::server::health::Health;
    use ::server::origin::Origin;
    use ::server::places::PlacesContext;
    use ::server::preferences::PreferencesContext;
    use ::server::sessions::SignedIn;
    use domain::api::{
        AssetsOutcome, EventsOutcome, PlacesOutcome, PreferencesOutcome, SquawksOutcome,
    };
    use domain::event::{EventId, EventInput, EventPatch};
    use domain::hlc::Hlc;
    use domain::id::DatabaseId;
    use domain::location::{SavedPlaceForm, SavedPlaceId, SavedPlacePatch};
    use domain::log_area;
    use domain::operation::{OperationChange, Refusal};
    use domain::squawk::{SquawkId, SquawkPatch};
    use domain::time::Clock;
    use storage::Database;

    use crate::local_copy::Sent;
    use crate::logging::SystemClock;

    /// A server function's answer, as a save on the copy needs to read it.
    pub(super) trait Answer: Sized {
        /// Whether it saved.
        fn saved(&self) -> bool;
        /// The answer the server refused with, if it's this kind.
        fn refused(refusal: Refusal) -> Option<Self>;
        /// "Unavailable".
        fn unavailable() -> Self;
    }

    impl Answer for EventsOutcome {
        fn saved(&self) -> bool {
            matches!(self, Self::Saved { .. } | Self::Done)
        }
        fn refused(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Events(answer) => Some(answer),
                Refusal::Squawks(_)
                | Refusal::Places(_)
                | Refusal::Preferences(_)
                | Refusal::Assets(_)
                | Refusal::Attachments(_) => None,
            }
        }
        fn unavailable() -> Self {
            Self::Unavailable
        }
    }

    impl Answer for SquawksOutcome {
        fn saved(&self) -> bool {
            matches!(self, Self::Saved { .. } | Self::Done)
        }
        fn refused(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Squawks(answer) => Some(answer),
                Refusal::Events(_)
                | Refusal::Places(_)
                | Refusal::Preferences(_)
                | Refusal::Assets(_)
                | Refusal::Attachments(_) => None,
            }
        }
        fn unavailable() -> Self {
            Self::Unavailable
        }
    }

    impl Answer for PlacesOutcome {
        fn saved(&self) -> bool {
            matches!(self, Self::Added { .. } | Self::Done)
        }
        fn refused(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Places(answer) => Some(answer),
                Refusal::Events(_)
                | Refusal::Squawks(_)
                | Refusal::Preferences(_)
                | Refusal::Assets(_)
                | Refusal::Attachments(_) => None,
            }
        }
        fn unavailable() -> Self {
            Self::Unavailable
        }
    }

    impl Answer for PreferencesOutcome {
        fn saved(&self) -> bool {
            matches!(self, Self::Preferences { .. })
        }
        fn refused(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Preferences(answer) => Some(answer),
                Refusal::Events(_)
                | Refusal::Squawks(_)
                | Refusal::Places(_)
                | Refusal::Assets(_)
                | Refusal::Attachments(_) => None,
            }
        }
        fn unavailable() -> Self {
            Self::Unavailable
        }
    }

    impl Answer for AssetsOutcome {
        fn saved(&self) -> bool {
            matches!(self, Self::Done | Self::Added { .. })
        }
        fn refused(refusal: Refusal) -> Option<Self> {
            match refusal {
                Refusal::Assets(answer) => Some(answer),
                Refusal::Events(_)
                | Refusal::Squawks(_)
                | Refusal::Places(_)
                | Refusal::Preferences(_)
                | Refusal::Attachments(_) => None,
            }
        }
        fn unavailable() -> Self {
            Self::Unavailable
        }
    }

    /// The copy and who is reading it.
    pub(super) struct Reader {
        database: Database,
        database_id: DatabaseId,
        user: SignedIn,
    }

    impl Reader {
        /// The copy, caught up, and the signed-in person as the copy knows them (their grants
        /// from it, their mode from the session); `None` to ask the server instead.
        pub(super) async fn new() -> Option<Self> {
            let session = crate::local_copy::session()?;
            let (database, state) = crate::local_copy::for_reading().await?;
            let user = match database.user_by_username(&session.username).await {
                Ok(Some(user)) => user,
                Ok(None) => return None,
                Err(error) => {
                    tracing::warn!(target: log_area::SYNC, %error, "reading the copy: can't find the person");
                    return None;
                }
            };
            let grants = database
                .grants_for(user.id)
                .await
                .ok()?
                .into_iter()
                .map(|(_, grant)| grant)
                .collect();
            Some(Self {
                database,
                database_id: state.database_id,
                user: SignedIn {
                    // Not a session on the server: reads never use these two.
                    token_hash: [0; 32],
                    device: state.database_id.cast(),
                    user: user.id,
                    username: session.username,
                    display_name: session.display_name,
                    must_change_password: session.must_change_password,
                    grants,
                    mode: session.mode,
                },
            })
        }

        pub(super) fn assets(&self) -> AssetsContext<'_> {
            self.assets_from(Origin::Online)
        }

        pub(super) fn preferences(&self) -> PreferencesContext<'_> {
            self.preferences_from(Origin::Online)
        }

        pub(super) fn places(&self) -> PlacesContext<'_> {
            self.places_from(Origin::Online)
        }

        /// For a change from `origin` (an operation, applied to the copy).
        pub(super) fn assets_from<'a>(&'a self, origin: Origin<'a>) -> AssetsContext<'a> {
            AssetsContext {
                database: &self.database,
                database_id: self.database_id,
                user: &self.user,
                // The copy is always "healthy": the server checks its own health when the
                // change reaches it.
                health: Health::Healthy,
                clock: &SystemClock,
                origin,
            }
        }

        pub(super) fn preferences_from<'a>(&'a self, origin: Origin<'a>) -> PreferencesContext<'a> {
            PreferencesContext {
                database: &self.database,
                database_id: self.database_id,
                user: &self.user,
                health: Health::Healthy,
                clock: &SystemClock,
                origin,
            }
        }

        pub(super) fn places_from<'a>(&'a self, origin: Origin<'a>) -> PlacesContext<'a> {
            PlacesContext {
                database: &self.database,
                user: &self.user,
                health: Health::Healthy,
                clock: &SystemClock,
                origin,
                // A saved place's list and changes need no place data.
                geocoder: None,
            }
        }

        /// Saves `change` on the desktop (5d): into the outbox, then onto the copy with the
        /// server's own code (`apply`, which also says the time of the last command it wrote);
        /// out of the outbox again if the copy refuses it. Then it's sent: online the answer is
        /// the server's (a refusal shows in the form), offline it waits.
        pub(super) async fn save<T: Answer>(
            &self,
            change: OperationChange,
            apply: impl AsyncFnOnce(Origin<'_>) -> (T, Option<Hlc>),
        ) -> T {
            let Some(operation) = self.queue(change).await else {
                return T::unavailable();
            };
            // Boxed: a server function's future is large.
            let (answer, last) = Box::pin(apply(Origin::pushed(&operation))).await;
            if !answer.saved() {
                self.unqueue(operation.id).await;
                return answer;
            }
            self.move_clock(last).await;
            // A refused change opened again (Messages, 2A) and now saved anew: the old one goes.
            if let Some(reopened) = crate::ui::outbox::settle_reopened(&operation.change)
                && let Err(error) = self.database.remove_from_outbox(reopened).await
            {
                tracing::warn!(target: log_area::SYNC, %error, "couldn't take a reopened change out of the outbox");
            }
            tracing::debug!(target: log_area::SYNC, operation = %operation.id, "saved on this computer");
            match crate::local_copy::send_now(&self.database, operation.id).await {
                Sent::Applied | Sent::Waiting => answer,
                Sent::Refused(refusal) => T::refused(refusal).unwrap_or_else(T::unavailable),
            }
        }

        /// A new operation for `change`, added to the outbox; `None` (logged) if it can't be.
        async fn queue(&self, change: OperationChange) -> Option<domain::operation::Operation> {
            let operation = crate::local_copy::new_operation(&self.database, change)
                .await
                .inspect_err(|error| {
                    tracing::error!(target: log_area::SYNC, %error, "couldn't make a change");
                })
                .ok()?;
            self.database
                .add_to_outbox(&operation, SystemClock.now())
                .await
                .inspect_err(|error| {
                    tracing::error!(target: log_area::SYNC, %error, "couldn't add a change to the outbox");
                })
                .ok()?;
            Some(operation)
        }

        /// Takes a change the copy refused out of the outbox.
        async fn unqueue(&self, id: domain::operation::OperationId) {
            if let Err(error) = self.database.remove_from_outbox(id).await {
                tracing::error!(target: log_area::SYNC, %error, "couldn't take a refused change out of the outbox");
            }
        }

        /// Moves the desktop's clock past the last command a change wrote.
        async fn move_clock(&self, last: Option<Hlc>) {
            if let Some(last) = last
                && let Err(error) = self.database.advance_device_hlc(last).await
            {
                tracing::warn!(target: log_area::SYNC, %error, "couldn't move the desktop's clock");
            }
        }

        /// What an event's edit changes, against the copy's event (nothing if it's not there:
        /// the edit then says "not found").
        pub(super) async fn event_changes(&self, id: EventId, input: EventInput) -> EventPatch {
            match self.database.event(id).await {
                Ok(Some(current)) => input.changes_from(&current),
                Ok(None) | Err(_) => EventPatch::default(),
            }
        }

        /// What a squawk's edit changes, against the copy's squawk.
        pub(super) async fn squawk_changes(
            &self,
            id: SquawkId,
            input: domain::api::SquawkInput,
        ) -> SquawkPatch {
            match self.database.squawk(id).await {
                Ok(Some(current)) => input.changes_from(&current),
                Ok(None) | Err(_) => SquawkPatch::default(),
            }
        }

        /// What a saved place's edit changes, against the copy's place; the form's problem if
        /// it doesn't read.
        pub(super) async fn place_changes(
            &self,
            id: SavedPlaceId,
            form: &SavedPlaceForm,
        ) -> Result<SavedPlacePatch, PlacesOutcome> {
            let wanted = form
                .parse(self.user.user)
                .map_err(|problem| PlacesOutcome::Invalid { problem })?;
            Ok(match self.database.saved_place(id).await {
                Ok(Some(current)) => current.changes_to(wanted),
                Ok(None) | Err(_) => SavedPlacePatch::default(),
            })
        }
    }
}

/// Answers from the copy on a desktop that has one (`$read` with a `local::Reader` named
/// `$reader`), otherwise from the server (`$server`).
macro_rules! read {
    ($reader:ident => $read:expr, $server:expr) => {{
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        if let Some($reader) = local::Reader::new().await {
            return Ok($read);
        }
        $server.await
    }};
}

/// Saves on the desktop's copy and outbox when it has a complete copy (`$change` applied with
/// `$apply`, a server function given the context `$context` from a `local::Reader` named
/// `$reader`), otherwise asks the server (`$server`).
macro_rules! write {
    ($reader:ident, $change:expr, $context:ident => $make:ident, $apply:expr, $server:expr) => {{
        #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
        // Boxed: catching the copy up first makes a large future.
        if let Some($reader) = Box::pin(local::Reader::new()).await {
            let change = $change;
            return Ok($reader
                .save(change, async |origin| {
                    let $context = $reader.$make(origin);
                    let answer = $apply.await;
                    (answer, $context.origin.last_command_hlc().ok().flatten())
                })
                .await);
        }
        $server.await
    }};
}

/// Logs an event (`api::add_event`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn add_event(input: EventInput) -> ServerFnResult<EventsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::AddEvent { input: input.clone() },
        context => assets_from,
        ::server::events::add(&context, input.clone()),
        api::add_event(input)
    )
}

/// Changes a draft (`api::edit_event`): on the desktop, only the fields the person changed.
///
/// # Errors
///
/// As the server function.
pub(crate) async fn edit_event(id: EventId, input: EventInput) -> ServerFnResult<EventsOutcome> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if let Some(reader) = Box::pin(local::Reader::new()).await {
        let changes = reader.event_changes(id, input.clone()).await;
        let change = domain::operation::OperationChange::EditEvent {
            id,
            changes: changes.clone(),
        };
        return Ok(reader
            .save(change, async |origin| {
                let context = reader.assets_from(origin);
                let answer = ::server::events::edit_changes(&context, id, changes).await;
                (answer, context.origin.last_command_hlc().ok().flatten())
            })
            .await);
    }
    api::edit_event(id, input).await
}

/// **Finalize now** (`api::finalize_event`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn finalize_event(id: EventId) -> ServerFnResult<EventsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::FinalizeEvent { id },
        context => assets_from,
        ::server::events::finalize_now(&context, id),
        api::finalize_event(id)
    )
}

/// **Finish** (`api::finish_event`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn finish_event(id: EventId) -> ServerFnResult<EventsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::FinishEvent { id },
        context => assets_from,
        ::server::events::finish(&context, id),
        api::finish_event(id)
    )
}

/// Deletes a draft (`api::delete_event`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn delete_event(id: EventId) -> ServerFnResult<EventsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::DeleteEvent { id },
        context => assets_from,
        ::server::events::delete(&context, id),
        api::delete_event(id)
    )
}

/// Corrects a finalized event (`api::correct_event`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn correct_event(
    original: EventId,
    input: EventInput,
    reason: String,
) -> ServerFnResult<EventsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::CorrectEvent {
            original,
            input: input.clone(),
            reason: reason.clone(),
        },
        context => assets_from,
        ::server::events::correct(&context, original, input.clone(), &reason),
        api::correct_event(original, input, reason)
    )
}

/// Opens a squawk (`api::open_squawk`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn open_squawk(input: SquawkInput) -> ServerFnResult<SquawksOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::OpenSquawk { input: input.clone() },
        context => assets_from,
        ::server::squawks::open(&context, input.clone()),
        api::open_squawk(input)
    )
}

/// Changes a squawk (`api::edit_squawk`): on the desktop, only the fields the person changed.
///
/// # Errors
///
/// As the server function.
pub(crate) async fn edit_squawk(
    id: SquawkId,
    input: SquawkInput,
) -> ServerFnResult<SquawksOutcome> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if let Some(reader) = Box::pin(local::Reader::new()).await {
        let changes = reader.squawk_changes(id, input.clone()).await;
        let change = domain::operation::OperationChange::EditSquawk {
            id,
            changes: changes.clone(),
        };
        return Ok(reader
            .save(change, async |origin| {
                let context = reader.assets_from(origin);
                let answer = ::server::squawks::edit_changes(&context, id, changes).await;
                (answer, context.origin.last_command_hlc().ok().flatten())
            })
            .await);
    }
    api::edit_squawk(id, input).await
}

/// Removes a squawk (`api::delete_squawk`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn delete_squawk(id: SquawkId) -> ServerFnResult<SquawksOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::DeleteSquawk { id },
        context => assets_from,
        ::server::squawks::delete(&context, id),
        api::delete_squawk(id)
    )
}

/// Changes the person's own preferences (`api::set_my_preferences`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn set_my_preferences(
    patch: PreferencesPatch,
) -> ServerFnResult<PreferencesOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::SetMyPreferences { patch },
        context => preferences_from,
        ::server::preferences::set_mine(&context, patch),
        api::set_my_preferences(patch)
    )
}

/// Adds a saved place (`api::add_place`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn add_place(form: SavedPlaceForm) -> ServerFnResult<PlacesOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::AddPlace { form: form.clone() },
        context => places_from,
        ::server::places::add(&context, &form),
        api::add_place(form)
    )
}

/// Changes a saved place (`api::edit_place`): on the desktop, only the fields the person
/// changed.
///
/// # Errors
///
/// As the server function.
pub(crate) async fn edit_place(
    id: SavedPlaceId,
    form: SavedPlaceForm,
) -> ServerFnResult<PlacesOutcome> {
    #[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
    if let Some(reader) = Box::pin(local::Reader::new()).await {
        let changes = match reader.place_changes(id, &form).await {
            Ok(changes) => changes,
            Err(problem) => return Ok(problem),
        };
        let change = domain::operation::OperationChange::EditPlace {
            id,
            changes: changes.clone(),
        };
        return Ok(reader
            .save(change, async |origin| {
                let context = reader.places_from(origin);
                let answer = ::server::places::edit_changes(&context, id, changes).await;
                (answer, context.origin.last_command_hlc().ok().flatten())
            })
            .await);
    }
    api::edit_place(id, form).await
}

/// Removes a saved place (`api::delete_place`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn delete_place(id: SavedPlaceId) -> ServerFnResult<PlacesOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::DeletePlace { id },
        context => places_from,
        ::server::places::delete(&context, id),
        api::delete_place(id)
    )
}

/// Saves the person's Garage order (`api::set_garage_order`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn set_garage_order(order: Vec<AssetId>) -> ServerFnResult<AssetsOutcome> {
    write!(
        reader,
        domain::operation::OperationChange::SetGarageOrder { order: order.clone() },
        context => assets_from,
        ::server::assets::set_order(&context, order.clone()),
        api::set_garage_order(order)
    )
}

/// The person's preferences (`api::get_preferences`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn get_preferences() -> ServerFnResult<PreferencesOutcome> {
    read!(reader => ::server::preferences::view(&reader.preferences()).await, api::get_preferences())
}

/// The person's saved places (`api::get_places`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn get_places() -> ServerFnResult<PlacesOutcome> {
    read!(reader => ::server::places::list(&reader.places()).await, api::get_places())
}

/// The Garage (`api::get_garage`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn get_garage() -> ServerFnResult<AssetsOutcome> {
    read!(reader => ::server::assets::garage(&reader.assets()).await, api::get_garage())
}

/// One vehicle's page (`api::vehicle_page`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn vehicle_page(id: AssetId) -> ServerFnResult<AssetsOutcome> {
    read!(reader => ::server::assets::view(&reader.assets(), id).await, api::vehicle_page(id))
}

/// A vehicle's events (`api::vehicle_events`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn vehicle_events(asset: AssetId) -> ServerFnResult<EventsOutcome> {
    read!(reader => ::server::events::list(&reader.assets(), asset).await, api::vehicle_events(asset))
}

/// One event (`api::event_view`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn event_view(id: EventId) -> ServerFnResult<EventsOutcome> {
    read!(reader => ::server::events::view(&reader.assets(), id).await, api::event_view(id))
}

/// What the log form needs (`api::event_log_context`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn event_log_context(asset: AssetId) -> ServerFnResult<EventsOutcome> {
    read!(reader => ::server::events::log_context(&reader.assets(), asset).await, api::event_log_context(asset))
}

/// A vehicle's squawks (`api::vehicle_squawks`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn vehicle_squawks(asset: AssetId) -> ServerFnResult<SquawksOutcome> {
    read!(reader => ::server::squawks::list(&reader.assets(), asset).await, api::vehicle_squawks(asset))
}

/// One squawk (`api::squawk_view`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn squawk_view(id: SquawkId) -> ServerFnResult<SquawksOutcome> {
    read!(reader => ::server::squawks::view(&reader.assets(), id).await, api::squawk_view(id))
}

/// A record's photos and documents, as a list (`api::attachments`; the files themselves come
/// from the server until milestone 6).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn attachments(
    asset: AssetId,
    owner: AttachmentOwner,
) -> ServerFnResult<AttachmentsOutcome> {
    read!(reader => ::server::attachments::list(&reader.assets(), asset, owner).await, api::attachments(asset, owner))
}

/// A vehicle's documents and plates (`api::documents`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn documents(
    asset: AssetId,
    today: CalendarDate,
) -> ServerFnResult<DocumentsOutcome> {
    read!(reader => ::server::documents::list(&reader.assets(), asset, &today).await, api::documents(asset, today))
}

/// The reference documents (`api::references`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn references() -> ServerFnResult<ReferencesOutcome> {
    read!(reader => ::server::references::list(&reader.assets()).await, api::references())
}

/// One reference document (`api::reference`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn reference(id: ReferenceDocumentId) -> ServerFnResult<ReferencesOutcome> {
    read!(reader => ::server::references::view(&reader.assets(), id).await, api::reference(id))
}

/// A vehicle's maintenance schedule (`api::vehicle_schedule`).
///
/// # Errors
///
/// As the server function.
pub(crate) async fn vehicle_schedule(asset: AssetId) -> ServerFnResult<ScheduleOutcome> {
    read!(reader => ::server::schedule::view(&reader.assets(), asset).await, api::vehicle_schedule(asset))
}
