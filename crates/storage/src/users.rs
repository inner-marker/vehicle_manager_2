//! Users, role grants, passwords, and sessions in the database.
//!
//! - **Users and role grants** are records: they change only through commands, written by
//!   [`UserWriter`] and [`RoleGrantWriter`] inside the command's transaction.
//! - **Password hashes and sessions** are server-only and never records or synced. Sessions
//!   are stored by the SHA-256 of their token (the caller hashes it), never the token.

use domain::access::{ActiveMode, GrantTarget, Role, RoleGrant, RoleGrantId, RoleGrantPatch};
use domain::id::{DeviceId, Id};
use domain::time::UnixMillis;
use domain::user::{User, UserId, UserPatch, Username};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqliteConnection};

use crate::commands::RecordWriter;
use crate::convert::{id_from_sql, id_to_sql};
use crate::{Database, StorageError};

// --- Column forms of domain enums --------------------------------------------------------

fn role_to_sql(role: Role) -> &'static str {
    match role {
        Role::Viewer => "viewer",
        Role::Driver => "driver",
        Role::Maintainer => "maintainer",
        Role::Admin => "admin",
    }
}

fn role_from_sql(value: &str) -> Result<Role, StorageError> {
    match value {
        "viewer" => Ok(Role::Viewer),
        "driver" => Ok(Role::Driver),
        "maintainer" => Ok(Role::Maintainer),
        "admin" => Ok(Role::Admin),
        _ => Err(StorageError::InvalidValue { column: "role" }),
    }
}

fn mode_to_sql(mode: ActiveMode) -> &'static str {
    match mode {
        ActiveMode::Driver => "driver",
        ActiveMode::Maintainer => "maintainer",
        ActiveMode::Admin => "admin",
    }
}

fn mode_from_sql(value: &str) -> Result<ActiveMode, StorageError> {
    match value {
        "driver" => Ok(ActiveMode::Driver),
        "maintainer" => Ok(ActiveMode::Maintainer),
        "admin" => Ok(ActiveMode::Admin),
        _ => Err(StorageError::InvalidValue {
            column: "active_mode",
        }),
    }
}

/// `(target_kind, target_asset)` for a grant target.
fn target_to_sql(target: GrantTarget) -> (&'static str, Option<Vec<u8>>) {
    match target {
        GrantTarget::AllAssets => ("all-assets", None),
        GrantTarget::Asset(asset) => ("asset", Some(id_to_sql(asset))),
    }
}

fn target_from_row(row: &SqliteRow) -> Result<GrantTarget, StorageError> {
    let kind: String = row.try_get("target_kind")?;
    match kind.as_str() {
        "all-assets" => Ok(GrantTarget::AllAssets),
        "asset" => {
            let asset: &[u8] = row.try_get("target_asset")?;
            Ok(GrantTarget::Asset(id_from_sql(asset, "target_asset")?))
        }
        _ => Err(StorageError::InvalidValue {
            column: "target_kind",
        }),
    }
}

/// A patch field that must be set when creating a record.
fn required<T>(value: Option<T>, column: &'static str) -> Result<T, StorageError> {
    value.ok_or(StorageError::InvalidValue { column })
}

// --- Users -------------------------------------------------------------------------------

/// Writes the `users` table for the command pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct UserWriter;

impl RecordWriter<User> for UserWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: UserId,
        values: &UserPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO users (id, username, display_name, disabled, must_change_password, sequence)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(required(values.username.as_ref(), "username")?.as_str())
        .bind(required(values.display_name.as_deref(), "display_name")?)
        .bind(values.disabled.unwrap_or(false))
        .bind(values.must_change_password.unwrap_or(false))
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: UserId,
        patch: &UserPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        // `coalesce` keeps a column as it is when its patch field is unset.
        sqlx::query(
            "UPDATE users SET
                 username = coalesce(?, username),
                 display_name = coalesce(?, display_name),
                 disabled = coalesce(?, disabled),
                 must_change_password = coalesce(?, must_change_password),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.username.as_ref().map(Username::as_str))
        .bind(patch.display_name.as_deref())
        .bind(patch.disabled)
        .bind(patch.must_change_password)
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: UserId,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE users SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

/// A user as stored, with its ID.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredUser {
    /// The user's ID.
    pub id: UserId,
    /// The user.
    pub user: User,
}

fn user_from_row(row: &SqliteRow) -> Result<StoredUser, StorageError> {
    let username: String = row.try_get("username")?;
    Ok(StoredUser {
        id: id_from_sql(row.try_get::<&[u8], _>("id")?, "users.id")?,
        user: User {
            username: Username::parse(&username)
                .map_err(|_| StorageError::InvalidValue { column: "username" })?,
            display_name: row.try_get("display_name")?,
            disabled: row.try_get("disabled")?,
            must_change_password: row.try_get("must_change_password")?,
        },
    })
}

impl Database {
    /// The user with `username` (not deleted), if any.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn user_by_username(
        &self,
        username: &Username,
    ) -> Result<Option<StoredUser>, StorageError> {
        sqlx::query(
            "SELECT id, username, display_name, disabled, must_change_password
             FROM users WHERE username = ? AND deleted = 0",
        )
        .bind(username.as_str())
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(user_from_row)
        .transpose()
    }

    /// The user with `id` (not deleted), if any.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn user(&self, id: UserId) -> Result<Option<StoredUser>, StorageError> {
        sqlx::query(
            "SELECT id, username, display_name, disabled, must_change_password
             FROM users WHERE id = ? AND deleted = 0",
        )
        .bind(id_to_sql(id))
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(user_from_row)
        .transpose()
    }

    /// Every user (not deleted), by username.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn users(&self) -> Result<Vec<StoredUser>, StorageError> {
        sqlx::query(
            "SELECT id, username, display_name, disabled, must_change_password
             FROM users WHERE deleted = 0 ORDER BY username",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(user_from_row)
        .collect()
    }

    /// How many users exist (not deleted). Zero means the first Admin hasn't been set up.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn user_count(&self) -> Result<i64, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT count(*) FROM users WHERE deleted = 0")
                .fetch_one(self.pool())
                .await?,
        )
    }

    /// Stores (or replaces) a user's password hash.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails (e.g. the user doesn't exist).
    pub async fn set_password_hash(
        &self,
        user: UserId,
        password_hash: &str,
        now: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO user_passwords (user_id, password_hash, changed_at_ms) VALUES (?, ?, ?)
             ON CONFLICT (user_id) DO UPDATE SET
                 password_hash = excluded.password_hash, changed_at_ms = excluded.changed_at_ms",
        )
        .bind(id_to_sql(user))
        .bind(password_hash)
        .bind(now.as_millis())
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// A user's password hash, if they have one.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn password_hash(&self, user: UserId) -> Result<Option<String>, StorageError> {
        Ok(
            sqlx::query_scalar("SELECT password_hash FROM user_passwords WHERE user_id = ?")
                .bind(id_to_sql(user))
                .fetch_optional(self.pool())
                .await?,
        )
    }
}

// --- Role grants -------------------------------------------------------------------------

/// Writes the `role_grants` table for the command pipeline.
#[derive(Debug, Clone, Copy, Default)]
pub struct RoleGrantWriter;

impl RecordWriter<RoleGrant> for RoleGrantWriter {
    async fn create(
        &self,
        connection: &mut SqliteConnection,
        id: Id<RoleGrant>,
        values: &RoleGrantPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let (target_kind, target_asset) = target_to_sql(required(values.target, "target")?);
        sqlx::query(
            "INSERT INTO role_grants (id, user_id, target_kind, target_asset, role, sequence)
             VALUES (?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(id))
        .bind(id_to_sql(required(values.user, "user")?))
        .bind(target_kind)
        .bind(target_asset)
        .bind(role_to_sql(required(values.role, "role")?))
        .bind(sequence)
        .execute(connection)
        .await?;
        Ok(())
    }

    /// Grants aren't edited (a role change is a delete plus a create), but a create that
    /// arrives for an existing grant is applied field by field, like any record.
    async fn update(
        &self,
        connection: &mut SqliteConnection,
        id: Id<RoleGrant>,
        patch: &RoleGrantPatch,
        sequence: i64,
    ) -> Result<(), StorageError> {
        let target = patch.target.map(target_to_sql);
        sqlx::query(
            "UPDATE role_grants SET
                 user_id = coalesce(?, user_id),
                 target_kind = coalesce(?, target_kind),
                 target_asset = CASE WHEN ? THEN ? ELSE target_asset END,
                 role = coalesce(?, role),
                 sequence = ?
             WHERE id = ?",
        )
        .bind(patch.user.map(id_to_sql))
        .bind(target.as_ref().map(|(kind, _)| *kind))
        .bind(target.is_some())
        .bind(target.and_then(|(_, asset)| asset))
        .bind(patch.role.map(role_to_sql))
        .bind(sequence)
        .bind(id_to_sql(id))
        .execute(connection)
        .await?;
        Ok(())
    }

    async fn delete(
        &self,
        connection: &mut SqliteConnection,
        id: Id<RoleGrant>,
        sequence: i64,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE role_grants SET deleted = 1, sequence = ? WHERE id = ?")
            .bind(sequence)
            .bind(id_to_sql(id))
            .execute(connection)
            .await?;
        Ok(())
    }
}

fn grant_from_row(row: &SqliteRow) -> Result<(RoleGrantId, RoleGrant), StorageError> {
    let role: String = row.try_get("role")?;
    Ok((
        id_from_sql(row.try_get::<&[u8], _>("id")?, "role_grants.id")?,
        RoleGrant {
            user: id_from_sql(row.try_get::<&[u8], _>("user_id")?, "role_grants.user_id")?,
            target: target_from_row(row)?,
            role: role_from_sql(&role)?,
        },
    ))
}

impl Database {
    /// One user's grants (not deleted).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn grants_for(
        &self,
        user: UserId,
    ) -> Result<Vec<(RoleGrantId, RoleGrant)>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, target_kind, target_asset, role
             FROM role_grants WHERE user_id = ? AND deleted = 0",
        )
        .bind(id_to_sql(user))
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(grant_from_row)
        .collect()
    }

    /// Everyone's grants (not deleted), e.g. for the always-one-Admin check.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn all_grants(&self) -> Result<Vec<(RoleGrantId, RoleGrant)>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, target_kind, target_asset, role
             FROM role_grants WHERE deleted = 0",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(grant_from_row)
        .collect()
    }
}

// --- Sessions ----------------------------------------------------------------------------

/// A signed-in session, as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Session {
    /// SHA-256 of the session token.
    pub token_hash: [u8; 32],
    /// Who.
    pub user: UserId,
    /// The device it acts as.
    pub device: DeviceId,
    /// When it started.
    pub created_at: UnixMillis,
    /// When it was last used.
    pub last_used: UnixMillis,
    /// When it ends unless used again.
    pub expires_at: UnixMillis,
    /// The mode it's in.
    pub active_mode: ActiveMode,
}

fn session_from_row(row: &SqliteRow) -> Result<Session, StorageError> {
    let token_hash: &[u8] = row.try_get("token_hash")?;
    let mode: String = row.try_get("active_mode")?;
    Ok(Session {
        token_hash: token_hash
            .try_into()
            .map_err(|_| StorageError::InvalidValue {
                column: "token_hash",
            })?,
        user: id_from_sql(row.try_get::<&[u8], _>("user_id")?, "sessions.user_id")?,
        device: id_from_sql(row.try_get::<&[u8], _>("device_id")?, "sessions.device_id")?,
        created_at: UnixMillis::from_millis(row.try_get("created_at_ms")?),
        last_used: UnixMillis::from_millis(row.try_get("last_used_ms")?),
        expires_at: UnixMillis::from_millis(row.try_get("expires_at_ms")?),
        active_mode: mode_from_sql(&mode)?,
    })
}

impl Database {
    /// Stores a new session.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn insert_session(&self, session: &Session) -> Result<(), StorageError> {
        sqlx::query(
            "INSERT INTO sessions
                 (token_hash, user_id, device_id, created_at_ms, last_used_ms, expires_at_ms, active_mode)
             VALUES (?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(session.token_hash.to_vec())
        .bind(id_to_sql(session.user))
        .bind(id_to_sql(session.device))
        .bind(session.created_at.as_millis())
        .bind(session.last_used.as_millis())
        .bind(session.expires_at.as_millis())
        .bind(mode_to_sql(session.active_mode))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// The session with this token hash, if it exists (expired or not; the caller decides).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn session(&self, token_hash: [u8; 32]) -> Result<Option<Session>, StorageError> {
        sqlx::query(
            "SELECT token_hash, user_id, device_id, created_at_ms, last_used_ms, expires_at_ms, active_mode
             FROM sessions WHERE token_hash = ?",
        )
        .bind(token_hash.to_vec())
        .fetch_optional(self.pool())
        .await?
        .as_ref()
        .map(session_from_row)
        .transpose()
    }

    /// Records a use of the session: when, and its new end.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn touch_session(
        &self,
        token_hash: [u8; 32],
        now: UnixMillis,
        expires_at: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET last_used_ms = ?, expires_at_ms = ? WHERE token_hash = ?")
            .bind(now.as_millis())
            .bind(expires_at.as_millis())
            .bind(token_hash.to_vec())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Changes the session's active mode.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn set_session_mode(
        &self,
        token_hash: [u8; 32],
        mode: ActiveMode,
    ) -> Result<(), StorageError> {
        sqlx::query("UPDATE sessions SET active_mode = ? WHERE token_hash = ?")
            .bind(mode_to_sql(mode))
            .bind(token_hash.to_vec())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Ends one session (sign out).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn delete_session(&self, token_hash: [u8; 32]) -> Result<(), StorageError> {
        sqlx::query("DELETE FROM sessions WHERE token_hash = ?")
            .bind(token_hash.to_vec())
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Ends every session of a user (disabled, deleted, or password reset), and returns how
    /// many ended.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn delete_sessions_of(&self, user: UserId) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM sessions WHERE user_id = ?")
            .bind(id_to_sql(user))
            .execute(self.pool())
            .await?
            .rows_affected())
    }

    /// Removes sessions that ended before `now`, and returns how many.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn delete_expired_sessions(&self, now: UnixMillis) -> Result<u64, StorageError> {
        Ok(sqlx::query("DELETE FROM sessions WHERE expires_at_ms <= ?")
            .bind(now.as_millis())
            .execute(self.pool())
            .await?
            .rows_affected())
    }
}
