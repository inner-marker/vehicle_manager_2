//! Devices someone is signed in on (phase-3.md 2e, decision #073): Settings → Devices for your
//! own, and the household Admin's view of anyone's (Users, in Admin mode).
//!
//! Signing a device out ends its sessions and stamps it (`storage::devices`); a desktop's key
//! then stops working, so it must sign in (pair) again. Your own list never offers to sign out
//! the device you're using (that's the account menu's Sign out).

use domain::api::{DeviceView, DevicesOutcome};
use domain::device::DeviceName;
use domain::id::DeviceId;
use domain::log_area;
use domain::time::UnixMillis;
use domain::user::UserId;
use storage::Database;
use storage::devices::StoredDevice;

use crate::sessions::SignedIn;
use crate::user_admin::AdminContext;

/// Logs a failure and turns it into [`DevicesOutcome::Unavailable`].
fn unavailable(what: &str, error: &dyn std::fmt::Display) -> DevicesOutcome {
    tracing::error!(target: log_area::AUTH, %error, what, "devices: failed");
    DevicesOutcome::Unavailable
}

fn view(device: &StoredDevice, this_device: DeviceId) -> DeviceView {
    DeviceView {
        id: device.id,
        kind: device.kind,
        name: device.name.clone(),
        paired: device.public_key.is_some(),
        added_at: device.added_at,
        last_seen: device.last_seen,
        this_device: device.id == this_device,
    }
}

/// `user`'s signed-in devices, as seen from `this_device`.
async fn list(
    database: &Database,
    user: UserId,
    this_device: DeviceId,
    now: UnixMillis,
) -> DevicesOutcome {
    match database.signed_in_devices_of(user, now).await {
        Ok(devices) => DevicesOutcome::Devices {
            devices: devices
                .iter()
                .map(|device| view(device, this_device))
                .collect(),
        },
        Err(error) => unavailable("list", &error),
    }
}

/// The device `id`, if it's `user`'s and signed in at `now`.
async fn signed_in_device_of(
    database: &Database,
    user: UserId,
    id: DeviceId,
    now: UnixMillis,
) -> Result<Option<StoredDevice>, storage::StorageError> {
    Ok(database
        .signed_in_devices_of(user, now)
        .await?
        .into_iter()
        .find(|device| device.id == id))
}

/// Your own signed-in devices, this one marked.
pub async fn my_devices(
    database: &Database,
    signed_in: &SignedIn,
    now: UnixMillis,
) -> DevicesOutcome {
    list(database, signed_in.user, signed_in.device, now).await
}

/// Renames one of your own devices.
pub async fn rename(
    database: &Database,
    signed_in: &SignedIn,
    id: DeviceId,
    name: &str,
    now: UnixMillis,
) -> DevicesOutcome {
    let name = match DeviceName::parse(name) {
        Ok(name) => name,
        Err(reason) => return DevicesOutcome::InvalidName { reason },
    };
    match signed_in_device_of(database, signed_in.user, id, now).await {
        Ok(Some(_)) => {}
        Ok(None) => return DevicesOutcome::NotFound,
        Err(error) => return unavailable("rename", &error),
    }
    match database.rename_device(id, &name).await {
        Ok(()) => {
            tracing::info!(target: log_area::AUTH, user = %signed_in.user, device = %id, "device renamed");
            DevicesOutcome::Done
        }
        Err(error) => unavailable("rename", &error),
    }
}

/// Signs out one of your own devices (not the one you're using).
pub async fn sign_out(
    database: &Database,
    signed_in: &SignedIn,
    id: DeviceId,
    now: UnixMillis,
) -> DevicesOutcome {
    if id == signed_in.device {
        return DevicesOutcome::NotAllowed;
    }
    match signed_in_device_of(database, signed_in.user, id, now).await {
        Ok(Some(_)) => {}
        Ok(None) => return DevicesOutcome::NotFound,
        Err(error) => return unavailable("sign out", &error),
    }
    match database.sign_out_device(id, now).await {
        Ok(ended) => {
            tracing::info!(target: log_area::AUTH, user = %signed_in.user, device = %id, sessions_ended = ended, "device signed out");
            DevicesOutcome::Done
        }
        Err(error) => unavailable("sign out", &error),
    }
}

/// Signs out every one of your devices except the one you're using.
pub async fn sign_out_others(
    database: &Database,
    signed_in: &SignedIn,
    now: UnixMillis,
) -> DevicesOutcome {
    let devices = match database.signed_in_devices_of(signed_in.user, now).await {
        Ok(devices) => devices,
        Err(error) => return unavailable("sign out others", &error),
    };
    let mut count = 0_u32;
    for device in devices
        .iter()
        .filter(|device| device.id != signed_in.device)
    {
        if let Err(error) = database.sign_out_device(device.id, now).await {
            return unavailable("sign out others", &error);
        }
        count += 1;
    }
    tracing::info!(target: log_area::AUTH, user = %signed_in.user, devices = count, "signed out everywhere else");
    DevicesOutcome::Done
}

/// Another person's signed-in devices: the household Admin, in Admin mode.
pub async fn devices_of(context: &AdminContext<'_>, user: UserId) -> DevicesOutcome {
    if !context.allowed() {
        return DevicesOutcome::NotAllowed;
    }
    list(
        context.database,
        user,
        context.admin.device,
        context.clock.now(),
    )
    .await
}

/// Signs out one of a person's devices, or (`id` `None`) all of them: the household Admin, in
/// Admin mode. The device the Admin is using is never among them (their account menu's Sign
/// out does that), so "all" of their own leaves it signed in.
pub async fn admin_sign_out(
    context: &AdminContext<'_>,
    user: UserId,
    id: Option<DeviceId>,
) -> DevicesOutcome {
    if !context.allowed() {
        return DevicesOutcome::NotAllowed;
    }
    let now = context.clock.now();
    let devices = match context.database.signed_in_devices_of(user, now).await {
        Ok(devices) => devices,
        Err(error) => return unavailable("admin sign out", &error),
    };
    let chosen: Vec<&StoredDevice> = devices
        .iter()
        .filter(|device| device.id != context.admin.device)
        .filter(|device| id.is_none_or(|id| device.id == id))
        .collect();
    if id.is_some() && chosen.is_empty() {
        return DevicesOutcome::NotFound;
    }
    for device in &chosen {
        if let Err(error) = context.database.sign_out_device(device.id, now).await {
            return unavailable("admin sign out", &error);
        }
    }
    tracing::info!(target: log_area::AUTH, admin = %context.admin.user, %user, devices = chosen.len(), "an Admin signed out devices");
    DevicesOutcome::Done
}
