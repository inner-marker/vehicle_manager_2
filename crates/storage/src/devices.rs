//! Devices (phase-3.md 2a, decision #073; migration 0016): every session belongs to one.
//!
//! A device is listed (Settings → Devices) while it has a session that hasn't ended. Signing a
//! device out ([`Database::sign_out_device`]) ends its sessions and stamps it; its row stays,
//! because changes already applied carry its ID.

use domain::device::{DeviceKind, DeviceName};
use domain::id::DeviceId;
use domain::time::UnixMillis;
use domain::user::UserId;
use sqlx::Row;
use sqlx::sqlite::SqliteRow;

use crate::convert::{id_from_sql, id_to_sql};
use crate::users::{Session, insert_session_on};
use crate::{Database, StorageError};

/// A device as stored.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StoredDevice {
    /// Its ID (its changes are stamped with it).
    pub id: DeviceId,
    /// Whose it is.
    pub user: UserId,
    /// Desktop or browser.
    pub kind: DeviceKind,
    /// Its name, if it has one yet.
    pub name: Option<DeviceName>,
    /// A desktop's Ed25519 public key; a browser has none.
    pub public_key: Option<[u8; 32]>,
    /// When it was added (its first sign-in).
    pub added_at: UnixMillis,
    /// When it was last used.
    pub last_seen: UnixMillis,
    /// When it was signed out, if it was.
    pub signed_out_at: Option<UnixMillis>,
}

fn device_from_row(row: &SqliteRow) -> Result<StoredDevice, StorageError> {
    let id: &[u8] = row.try_get("id")?;
    let user: &[u8] = row.try_get("user_id")?;
    let kind: String = row.try_get("kind")?;
    let name: Option<String> = row.try_get("name")?;
    let public_key: Option<Vec<u8>> = row.try_get("public_key")?;
    let signed_out_at: Option<i64> = row.try_get("signed_out_at_ms")?;
    Ok(StoredDevice {
        id: id_from_sql(id, "id")?,
        user: id_from_sql(user, "user_id")?,
        kind: DeviceKind::parse(&kind).ok_or(StorageError::InvalidValue { column: "kind" })?,
        name: name
            .map(|name| DeviceName::parse(&name))
            .transpose()
            .map_err(|_| StorageError::InvalidValue { column: "name" })?,
        public_key: public_key
            .map(<[u8; 32]>::try_from)
            .transpose()
            .map_err(|_| StorageError::InvalidValue {
                column: "public_key",
            })?,
        added_at: UnixMillis::from_millis(row.try_get("added_at_ms")?),
        last_seen: UnixMillis::from_millis(row.try_get("last_seen_ms")?),
        signed_out_at: signed_out_at.map(UnixMillis::from_millis),
    })
}

impl Database {
    /// Stores a new device and its first session, together (a sign-in).
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails (nothing is stored then).
    pub async fn insert_device_with_session(
        &self,
        device: &StoredDevice,
        session: &Session,
    ) -> Result<(), StorageError> {
        let mut transaction = self.pool().begin().await?;
        sqlx::query(
            "INSERT INTO devices
                 (id, user_id, kind, name, public_key, added_at_ms, last_seen_ms, signed_out_at_ms)
             VALUES (?, ?, ?, ?, ?, ?, ?, ?)",
        )
        .bind(id_to_sql(device.id))
        .bind(id_to_sql(device.user))
        .bind(device.kind.as_str())
        .bind(device.name.as_ref().map(DeviceName::as_str))
        .bind(device.public_key.map(|key| key.to_vec()))
        .bind(device.added_at.as_millis())
        .bind(device.last_seen.as_millis())
        .bind(device.signed_out_at.map(UnixMillis::as_millis))
        .execute(&mut *transaction)
        .await?;
        insert_session_on(&mut transaction, session).await?;
        transaction.commit().await?;
        Ok(())
    }

    /// The device with this ID, if there is one (signed out or not).
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn device(&self, id: DeviceId) -> Result<Option<StoredDevice>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, kind, name, public_key, added_at_ms, last_seen_ms, signed_out_at_ms
             FROM devices WHERE id = ?",
        )
            .bind(id_to_sql(id))
            .fetch_optional(self.pool())
            .await?
            .as_ref()
            .map(device_from_row)
            .transpose()
    }

    /// `user`'s devices that are signed in at `now` (not signed out, with a session that
    /// hasn't ended), most recently used first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn signed_in_devices_of(
        &self,
        user: UserId,
        now: UnixMillis,
    ) -> Result<Vec<StoredDevice>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, kind, name, public_key, added_at_ms, last_seen_ms, signed_out_at_ms
             FROM devices
             WHERE user_id = ? AND signed_out_at_ms IS NULL
               AND EXISTS (SELECT 1 FROM sessions
                           WHERE sessions.device_id = devices.id AND sessions.expires_at_ms > ?)
             ORDER BY last_seen_ms DESC, id",
        )
        .bind(id_to_sql(user))
        .bind(now.as_millis())
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(device_from_row)
        .collect()
    }

    /// Renames a device.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn rename_device(&self, id: DeviceId, name: &DeviceName) -> Result<(), StorageError> {
        sqlx::query("UPDATE devices SET name = ? WHERE id = ?")
            .bind(name.as_str())
            .bind(id_to_sql(id))
            .execute(self.pool())
            .await?;
        Ok(())
    }

    /// Signs a device out: ends its sessions and stamps it (once). Returns how many sessions
    /// ended.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if a query fails (nothing changes then).
    pub async fn sign_out_device(
        &self,
        id: DeviceId,
        now: UnixMillis,
    ) -> Result<u64, StorageError> {
        let mut transaction = self.pool().begin().await?;
        let ended = sqlx::query("DELETE FROM sessions WHERE device_id = ?")
            .bind(id_to_sql(id))
            .execute(&mut *transaction)
            .await?
            .rows_affected();
        sqlx::query(
            "UPDATE devices SET signed_out_at_ms = ? WHERE id = ? AND signed_out_at_ms IS NULL",
        )
        .bind(now.as_millis())
        .bind(id_to_sql(id))
        .execute(&mut *transaction)
        .await?;
        transaction.commit().await?;
        Ok(ended)
    }

    /// Records a desktop's report of `count` wrong unlock tries, the last at `at` (migration
    /// 0017, phase-3.md 3d): replaces any earlier report.
    ///
    /// # Errors
    ///
    /// [`StorageError::Sqlx`] if the query fails.
    pub async fn record_failed_unlocks(
        &self,
        id: DeviceId,
        count: u32,
        at: UnixMillis,
    ) -> Result<(), StorageError> {
        sqlx::query(
            "UPDATE devices SET failed_unlocks = ?, last_failed_unlock_ms = ? WHERE id = ?",
        )
        .bind(i64::from(count))
        .bind(at.as_millis())
        .bind(id_to_sql(id))
        .execute(self.pool())
        .await?;
        Ok(())
    }

    /// Every device's latest failed-unlock report, oldest first.
    ///
    /// # Errors
    ///
    /// [`StorageError`] if the query fails or a stored value is invalid.
    pub async fn failed_unlock_reports(&self) -> Result<Vec<FailedUnlockReport>, StorageError> {
        sqlx::query(
            "SELECT id, user_id, kind, name, public_key, added_at_ms, last_seen_ms, signed_out_at_ms,
                    failed_unlocks, last_failed_unlock_ms
             FROM devices
             WHERE failed_unlocks > 0 AND last_failed_unlock_ms IS NOT NULL
             ORDER BY last_failed_unlock_ms",
        )
        .fetch_all(self.pool())
        .await?
        .iter()
        .map(|row| {
            let count: i64 = row.try_get("failed_unlocks")?;
            let at: i64 = row.try_get("last_failed_unlock_ms")?;
            Ok(FailedUnlockReport {
                device: device_from_row(row)?,
                count: u32::try_from(count).map_err(|_| StorageError::InvalidValue {
                    column: "failed_unlocks",
                })?,
                at: UnixMillis::from_millis(at),
            })
        })
        .collect()
    }
}

/// A device's latest failed-unlock report (phase-3.md 3d).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FailedUnlockReport {
    /// The device.
    pub device: StoredDevice,
    /// How many wrong tries came before the unlock.
    pub count: u32,
    /// When the last wrong try was.
    pub at: UnixMillis,
}
