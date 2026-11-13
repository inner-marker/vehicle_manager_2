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
//! Writes still go to the server (offline writes are milestone 5). Messages, Users, Devices,
//! place lookup, and files stay with the server for now.

use dioxus::prelude::*;
use domain::api::{
    AssetsOutcome, AttachmentsOutcome, DocumentsOutcome, EventsOutcome, PlacesOutcome,
    PreferencesOutcome, ReferencesOutcome, ScheduleOutcome, SquawksOutcome,
};
use domain::asset::AssetId;
use domain::attachment::AttachmentOwner;
use domain::dates::CalendarDate;
use domain::event::EventId;
use domain::reference::ReferenceDocumentId;
use domain::squawk::SquawkId;

use crate::api;

/// Reading the copy on a desktop.
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
mod local {
    use ::server::assets::AssetsContext;
    use ::server::health::Health;
    use ::server::places::PlacesContext;
    use ::server::preferences::PreferencesContext;
    use ::server::sessions::SignedIn;
    use domain::id::DatabaseId;
    use domain::log_area;
    use storage::Database;

    use crate::logging::SystemClock;

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
            AssetsContext {
                database: &self.database,
                database_id: self.database_id,
                user: &self.user,
                // Reads don't depend on the server's health (only changes do).
                health: Health::Healthy,
                clock: &SystemClock,
                origin: ::server::origin::Origin::Online,
            }
        }

        pub(super) fn preferences(&self) -> PreferencesContext<'_> {
            PreferencesContext {
                database: &self.database,
                database_id: self.database_id,
                user: &self.user,
                health: Health::Healthy,
                clock: &SystemClock,
                origin: ::server::origin::Origin::Online,
            }
        }

        pub(super) fn places(&self) -> PlacesContext<'_> {
            PlacesContext {
                database: &self.database,
                user: &self.user,
                health: Health::Healthy,
                clock: &SystemClock,
                origin: ::server::origin::Origin::Online,
                // A saved place's list needs no place data.
                geocoder: None,
            }
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
