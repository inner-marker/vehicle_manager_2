//! An Admin's way into Admin mode (sub-plan 04 §4.2; decision #071; phase-6.md 4e): their
//! password (the default) or a PIN of 6 to 12 digits, stored hashed beside the password and
//! never synced. Each Admin chooses their own, confirming with their password; no "none".
//! Entering with it is [`crate::sessions::switch_mode_limited`].

use domain::access::ActiveMode;
use domain::log_area;
use domain::secret::Secret;
use domain::user::{PinError, check_admin_pin};
use storage::Database;

use crate::passwords::{Busy, hash_password, verify_password};
use crate::sessions::SignedIn;

/// Why choosing a PIN (or the password) was refused.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AdminPinError {
    /// They can't enter Admin mode, so there's nothing to choose.
    NotAllowed,
    /// Their password, asked to confirm, was wrong.
    WrongPassword,
    /// The PIN isn't 6 to 12 digits.
    Invalid(PinError),
    /// The database or hashing failed.
    Unavailable,
}

/// Whether the person enters Admin mode with a PIN.
///
/// # Errors
///
/// [`AdminPinError::Unavailable`] if the database fails.
pub async fn has_admin_pin(
    database: &Database,
    signed_in: &SignedIn,
) -> Result<bool, AdminPinError> {
    database
        .admin_pin_hash(signed_in.user)
        .await
        .map(|hash| hash.is_some())
        .map_err(|_| AdminPinError::Unavailable)
}

/// Sets the person's PIN for Admin mode, or with `None` goes back to their password, after
/// checking `password`. Only for someone who may enter Admin mode.
///
/// # Errors
///
/// [`AdminPinError`].
pub async fn set_admin_pin(
    database: &Database,
    signed_in: &SignedIn,
    password: Secret,
    pin: Option<Secret>,
) -> Result<(), AdminPinError> {
    if !signed_in.available_modes().contains(&ActiveMode::Admin) {
        return Err(AdminPinError::NotAllowed);
    }
    if let Some(pin) = &pin {
        check_admin_pin(pin.expose()).map_err(AdminPinError::Invalid)?;
    }
    let stored = database
        .password_hash(signed_in.user)
        .await
        .map_err(|_| AdminPinError::Unavailable)?;
    let matches = match stored {
        Some(hash) => verify_password(password, hash)
            .await
            .map_err(|Busy| AdminPinError::Unavailable)?,
        None => false,
    };
    if !matches {
        return Err(AdminPinError::WrongPassword);
    }
    let hash = match pin {
        Some(pin) => Some(
            hash_password(pin)
                .await
                .map_err(|_| AdminPinError::Unavailable)?,
        ),
        None => None,
    };
    database
        .set_admin_pin_hash(signed_in.user, hash.as_deref())
        .await
        .map_err(|_| AdminPinError::Unavailable)?;
    tracing::info!(target: log_area::AUTH, user = %signed_in.user, pin = hash.is_some(), "Admin mode's way in changed");
    Ok(())
}
