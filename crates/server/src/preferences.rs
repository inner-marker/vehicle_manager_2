//! Display preferences on the server (decisions #029, #048).
//!
//! - **Anyone signed in** changes their own (any mode: they're the user's, not the household's).
//! - **The household's Admin, in Admin mode,** changes the household defaults.
//!
//! The household's record ID is the database's ID; each user's is their user ID. The first
//! change creates a record, later ones update it; each setting merges on its own (#043).

use domain::access::{Actor, AdminAction, permits_admin};
use domain::api::{PreferencesOutcome, PreferencesView};
use domain::command::Change;
use domain::id::DatabaseId;
use domain::log_area;
use domain::preferences::{Preferences, PreferencesId, PreferencesPatch};
use domain::time::Clock;
use storage::Database;
use storage::preferences::PreferencesWriter;

use crate::commands::issue;
use crate::health::Health;
use crate::sessions::SignedIn;

/// What preference changes need.
pub struct PreferencesContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// Its ID (the household record's ID).
    pub database_id: DatabaseId,
    /// Who is asking.
    pub user: &'a SignedIn,
    /// The server's health.
    pub health: Health,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
}

impl PreferencesContext<'_> {
    fn household_id(&self) -> PreferencesId {
        self.database_id.cast()
    }

    fn mine_id(&self) -> PreferencesId {
        self.user.user.cast()
    }
}

fn unavailable(error: &dyn std::fmt::Display) -> PreferencesOutcome {
    tracing::error!(target: log_area::APP, %error, "preferences failed");
    PreferencesOutcome::Unavailable
}

/// The signed-in user's preferences: theirs, the household's, and the ones in effect.
pub async fn view(context: &PreferencesContext<'_>) -> PreferencesOutcome {
    let mine = context.database.preference_choices(context.mine_id()).await;
    let household = context
        .database
        .preference_choices(context.household_id())
        .await;
    match (mine, household) {
        (Ok(mine), Ok(household)) => PreferencesOutcome::Preferences {
            view: PreferencesView {
                mine,
                household,
                effective: Preferences::resolve(&mine, &household),
            },
        },
        (Err(error), _) | (_, Err(error)) => unavailable(&error),
    }
}

/// Changes the signed-in user's own preferences.
pub async fn set_mine(
    context: &PreferencesContext<'_>,
    patch: PreferencesPatch,
) -> PreferencesOutcome {
    change(context, context.mine_id(), patch).await
}

/// Changes the household defaults (the household's Admin, in Admin mode).
pub async fn set_household(
    context: &PreferencesContext<'_>,
    patch: PreferencesPatch,
) -> PreferencesOutcome {
    if !permits_admin(
        &context.user.grants,
        context.user.mode,
        AdminAction::ManageServerSettings,
    ) {
        return PreferencesOutcome::NotAllowed;
    }
    change(context, context.household_id(), patch).await
}

async fn change(
    context: &PreferencesContext<'_>,
    id: PreferencesId,
    patch: PreferencesPatch,
) -> PreferencesOutcome {
    let exists = match context.database.preferences_exist(id).await {
        Ok(exists) => exists,
        Err(error) => return unavailable(&error),
    };
    let change = if exists {
        Change::Update { id, patch }
    } else {
        Change::Create { id, values: patch }
    };
    let actor = Actor {
        user: context.user.user,
        mode: context.user.mode,
    };
    if let Err(error) = issue(
        context.database,
        &PreferencesWriter,
        change,
        context.user.device,
        Some(actor),
        context.health,
        context.clock,
    )
    .await
    {
        return unavailable(&error);
    }
    tracing::debug!(target: log_area::APP, user = %context.user.user, household = (id == context.household_id()), "preferences changed");
    view(context).await
}
