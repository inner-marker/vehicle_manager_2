//! Managing the household's users: Admin mode only (plan §6, decisions #045, #046).
//!
//! Every function checks that the signed-in user may manage users (the household's Admin, in
//! Admin mode), makes its changes through the command pipeline with that Admin recorded as
//! the actor, and refuses anything that would leave the household without an enabled Admin.
//!
//! **Temporary passwords** are made here, never typed by an Admin: 12 characters from the
//! setup code's alphabet, shown once, e.g. `QX7M-K2PD-9WRT` (typed exactly as shown). The user
//! must choose their own at their next sign-in.

use domain::access::{
    Actor, AdminAction, GrantTarget, Role, RoleGrant, RoleGrantId, RoleGrantPatch, leaves_an_admin,
    permits_admin,
};
use domain::api::{AccountStatus, UserAdminOutcome, UserSummary};
use domain::command::Change;
use domain::log_area;
use domain::time::Clock;
use domain::user::{User, UserId, UserPatch, Username, check_display_name};
use storage::Database;
use storage::users::{RoleGrantWriter, StoredUser, UserWriter};

use crate::codes::{group_by_four, random_code};
use crate::commands::{ApplyError, issue};
use crate::health::Health;
use crate::ids::new_id;
use crate::passwords::hash_password;
use crate::sessions::SignedIn;

/// Characters in a temporary password (before the dashes): 12, the minimum password length.
const TEMPORARY_PASSWORD_CHARS: usize = 12;

/// Who is acting, and the server state they act in.
pub struct AdminContext<'a> {
    /// The database.
    pub database: &'a Database,
    /// The signed-in Admin.
    pub admin: &'a SignedIn,
    /// The server's health.
    pub health: Health,
    /// The real clock.
    pub clock: &'a (dyn Clock + Sync),
}

impl AdminContext<'_> {
    fn allowed(&self) -> bool {
        permits_admin(
            &self.admin.grants,
            self.admin.mode,
            AdminAction::ManageUsers,
        )
    }

    fn actor(&self) -> Actor {
        Actor {
            user: self.admin.user,
            mode: self.admin.mode,
        }
    }

    async fn issue_user(&self, change: Change<User>) -> Result<(), ApplyError> {
        issue(
            self.database,
            &UserWriter,
            change,
            self.admin.device,
            Some(self.actor()),
            self.health,
            self.clock,
        )
        .await
        .map(|_| ())
    }

    async fn issue_grant(&self, change: Change<RoleGrant>) -> Result<(), ApplyError> {
        issue(
            self.database,
            &RoleGrantWriter,
            change,
            self.admin.device,
            Some(self.actor()),
            self.health,
            self.clock,
        )
        .await
        .map(|_| ())
    }
}

/// Logs a failure and turns it into [`UserAdminOutcome::Unavailable`].
fn unavailable(what: &str, error: &dyn std::fmt::Display) -> UserAdminOutcome {
    tracing::error!(target: log_area::AUTH, %error, what, "user management failed");
    UserAdminOutcome::Unavailable
}

/// Everyone, for the Users screen.
pub async fn list_users(context: &AdminContext<'_>) -> UserAdminOutcome {
    if !context.allowed() {
        return UserAdminOutcome::NotAllowed;
    }
    let (users, grants) = match (
        context.database.users().await,
        context.database.all_grants().await,
    ) {
        (Ok(users), Ok(grants)) => (users, grants),
        (Err(error), _) | (_, Err(error)) => return unavailable("list", &error),
    };
    let summaries = users
        .iter()
        .map(|stored| summary(stored, &users, &grants, context.admin.user))
        .collect();
    UserAdminOutcome::Users { users: summaries }
}

/// One user's summary: their role on all assets, and whether they're the last enabled Admin.
fn summary(
    stored: &StoredUser,
    users: &[StoredUser],
    grants: &[(RoleGrantId, RoleGrant)],
    you: UserId,
) -> UserSummary {
    let role_on_all = grants
        .iter()
        .filter(|(_, grant)| grant.user == stored.id && grant.target == GrantTarget::AllAssets)
        .map(|(_, grant)| grant.role)
        .max();
    // The last Admin: without this user, no enabled household Admin would remain.
    let others: Vec<RoleGrant> = grants
        .iter()
        .map(|(_, grant)| *grant)
        .filter(|grant| grant.user != stored.id)
        .collect();
    let is_admin = role_on_all == Some(Role::Admin) && !stored.user.disabled;
    let is_last_admin = is_admin && !leaves_an_admin(&others, |user| is_enabled(users, user));
    UserSummary {
        id: stored.id,
        username: stored.user.username.clone(),
        display_name: stored.user.display_name.clone(),
        status: if stored.user.disabled {
            AccountStatus::Disabled
        } else if stored.user.must_change_password {
            AccountStatus::TemporaryPassword
        } else {
            AccountStatus::Active
        },
        role_on_all,
        is_you: stored.id == you,
        is_last_admin,
    }
}

fn is_enabled(users: &[StoredUser], user: UserId) -> bool {
    users
        .iter()
        .any(|stored| stored.id == user && !stored.user.disabled)
}

/// A new temporary password, and its hash.
async fn temporary_password() -> Result<(String, String), String> {
    let password =
        group_by_four(&random_code(TEMPORARY_PASSWORD_CHARS).map_err(|error| error.to_string())?);
    let hash = hash_password(password.clone())
        .await
        .map_err(|error| error.to_string())?;
    Ok((password, hash))
}

/// Adds a user with `role` on all assets and a temporary password, which is returned.
pub async fn add_user(
    context: &AdminContext<'_>,
    username: &str,
    display_name: &str,
    role: Role,
) -> UserAdminOutcome {
    if !context.allowed() {
        return UserAdminOutcome::NotAllowed;
    }
    let username = match Username::parse(username) {
        Ok(username) => username,
        Err(reason) => return UserAdminOutcome::InvalidUsername { reason },
    };
    let display_name = match check_display_name(display_name) {
        Ok(name) => name,
        Err(reason) => return UserAdminOutcome::InvalidDisplayName { reason },
    };
    match context.database.user_by_username(&username).await {
        Ok(Some(_)) => return UserAdminOutcome::UsernameTaken,
        Ok(None) => {}
        Err(error) => return unavailable("add", &error),
    }
    let (password, hash) = match temporary_password().await {
        Ok(pair) => pair,
        Err(error) => return unavailable("add", &error),
    };
    let (Ok(user_id), Ok(grant_id)) = (new_id(context.clock.now()), new_id(context.clock.now()))
    else {
        return unavailable("add", &"no random bytes for IDs");
    };
    let user = User {
        username,
        display_name,
        disabled: false,
        must_change_password: true,
    };
    let grant = RoleGrant {
        user: user_id,
        target: GrantTarget::AllAssets,
        role,
    };
    if let Err(error) = context
        .issue_user(Change::Create {
            id: user_id,
            values: UserPatch::all(user),
        })
        .await
    {
        return unavailable("add", &error);
    }
    if let Err(error) = context
        .issue_grant(Change::Create {
            id: grant_id,
            values: RoleGrantPatch::all(grant),
        })
        .await
    {
        return unavailable("add", &error);
    }
    if let Err(error) = context
        .database
        .set_password_hash(user_id, &hash, context.clock.now())
        .await
    {
        return unavailable("add", &error);
    }
    tracing::info!(target: log_area::AUTH, user = %user_id, ?role, by = %context.admin.user, "user added");
    UserAdminOutcome::TemporaryPassword { password }
}

/// Sets `user`'s role on all assets (`None` removes it). Refused if it would leave no Admin.
pub async fn set_role_on_all(
    context: &AdminContext<'_>,
    user: UserId,
    role: Option<Role>,
) -> UserAdminOutcome {
    if !context.allowed() {
        return UserAdminOutcome::NotAllowed;
    }
    let (users, grants) = match (
        context.database.users().await,
        context.database.all_grants().await,
    ) {
        (Ok(users), Ok(grants)) => (users, grants),
        (Err(error), _) | (_, Err(error)) => return unavailable("role", &error),
    };
    if !users.iter().any(|stored| stored.id == user) {
        return UserAdminOutcome::NotFound;
    }
    let (theirs, others): (Vec<_>, Vec<_>) = grants
        .into_iter()
        .partition(|(_, grant)| grant.user == user && grant.target == GrantTarget::AllAssets);
    let mut after: Vec<RoleGrant> = others.iter().map(|(_, grant)| *grant).collect();
    if let Some(role) = role {
        after.push(RoleGrant {
            user,
            target: GrantTarget::AllAssets,
            role,
        });
    }
    if !leaves_an_admin(&after, |id| is_enabled(&users, id)) {
        return UserAdminOutcome::LastAdmin;
    }
    // A role change is a delete plus a create: one clear entry each in the history (#045).
    for (id, _) in theirs {
        if let Err(error) = context.issue_grant(Change::Delete { id }).await {
            return unavailable("role", &error);
        }
    }
    if let Some(role) = role {
        let Ok(id) = new_id(context.clock.now()) else {
            return unavailable("role", &"no random bytes for an ID");
        };
        let grant = RoleGrant {
            user,
            target: GrantTarget::AllAssets,
            role,
        };
        if let Err(error) = context
            .issue_grant(Change::Create {
                id,
                values: RoleGrantPatch::all(grant),
            })
            .await
        {
            return unavailable("role", &error);
        }
    }
    tracing::info!(target: log_area::AUTH, %user, ?role, by = %context.admin.user, "role on all vehicles changed");
    UserAdminOutcome::Done
}

/// Disables (or enables again) `user`. Disabling signs them out everywhere, and is refused if
/// it would leave no Admin.
pub async fn set_disabled(
    context: &AdminContext<'_>,
    user: UserId,
    disabled: bool,
) -> UserAdminOutcome {
    if !context.allowed() {
        return UserAdminOutcome::NotAllowed;
    }
    let (users, grants) = match (
        context.database.users().await,
        context.database.all_grants().await,
    ) {
        (Ok(users), Ok(grants)) => (users, grants),
        (Err(error), _) | (_, Err(error)) => return unavailable("disable", &error),
    };
    if !users.iter().any(|stored| stored.id == user) {
        return UserAdminOutcome::NotFound;
    }
    let grants: Vec<RoleGrant> = grants.into_iter().map(|(_, grant)| grant).collect();
    if disabled && !leaves_an_admin(&grants, |id| id != user && is_enabled(&users, id)) {
        return UserAdminOutcome::LastAdmin;
    }
    let patch = UserPatch {
        disabled: Some(disabled),
        ..UserPatch::default()
    };
    if let Err(error) = context.issue_user(Change::Update { id: user, patch }).await {
        return unavailable("disable", &error);
    }
    if disabled && let Err(error) = context.database.delete_sessions_of(user).await {
        return unavailable("disable", &error);
    }
    tracing::info!(target: log_area::AUTH, %user, disabled, by = %context.admin.user, "user enabled or disabled");
    UserAdminOutcome::Done
}

/// Gives `user` a new temporary password (returned), which they must change at their next
/// sign-in, and signs them out everywhere.
pub async fn reset_password(context: &AdminContext<'_>, user: UserId) -> UserAdminOutcome {
    if !context.allowed() {
        return UserAdminOutcome::NotAllowed;
    }
    match context.database.user(user).await {
        Ok(Some(_)) => {}
        Ok(None) => return UserAdminOutcome::NotFound,
        Err(error) => return unavailable("reset", &error),
    }
    let (password, hash) = match temporary_password().await {
        Ok(pair) => pair,
        Err(error) => return unavailable("reset", &error),
    };
    if let Err(error) = context
        .database
        .set_password_hash(user, &hash, context.clock.now())
        .await
    {
        return unavailable("reset", &error);
    }
    let patch = UserPatch {
        must_change_password: Some(true),
        ..UserPatch::default()
    };
    if let Err(error) = context.issue_user(Change::Update { id: user, patch }).await {
        return unavailable("reset", &error);
    }
    if let Err(error) = context.database.delete_sessions_of(user).await {
        return unavailable("reset", &error);
    }
    tracing::info!(target: log_area::AUTH, %user, by = %context.admin.user, "password reset to a temporary one");
    UserAdminOutcome::TemporaryPassword { password }
}
