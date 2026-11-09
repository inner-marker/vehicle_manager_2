//! Devices (phase-3.md 2e, decision #073): your own list, renaming, signing out (one, or all
//! but this one), and the household Admin's view of a person's devices.

#![cfg(test)]

mod common;

use common::NOW;
use domain::access::ActiveMode;
use domain::api::DevicesOutcome;
use domain::device::{DeviceName, DeviceNameError, browser_label};
use domain::test_support::ManualClock;
use domain::timers::Timers;
use server::devices::{admin_sign_out, devices_of, my_devices, rename, sign_out, sign_out_others};
use server::health::Health;
use server::login_limiter::LoginLimiter;
use server::sessions::{SignedIn, SigningInDevice, resolve, sign_in_from, switch_mode};
use server::user_admin::AdminContext;
use storage::Database;

/// Signs `username` in from a browser named `name`.
async fn browser(database: &Database, username: &str, name: &str) -> SignedIn {
    let (_, signed_in) = sign_in_from(
        database,
        &LoginLimiter::new(),
        username,
        "a long password".into(),
        name,
        NOW,
        SigningInDevice::Browser {
            name: Some(DeviceName::parse(name).unwrap()),
        },
    )
    .await
    .unwrap();
    signed_in
}

fn ids(outcome: &DevicesOutcome) -> Vec<(String, bool)> {
    let DevicesOutcome::Devices { devices } = outcome else {
        panic!("expected devices, got {outcome:?}");
    };
    devices
        .iter()
        .map(|device| {
            (
                device
                    .name
                    .as_ref()
                    .map_or("-", DeviceName::as_str)
                    .to_owned(),
                device.this_device,
            )
        })
        .collect()
}

#[tokio::test]
async fn your_list_marks_this_device_and_a_browser_is_named_from_its_user_agent() {
    let (_temp, database, _, _) = common::setup().await;
    let phone = browser(&database, "sam", "Safari on iPhone").await;
    let _laptop = browser(&database, "sam", "Chrome on Mac").await;
    let listed = ids(&my_devices(&database, &phone, NOW).await);
    assert!(
        listed.contains(&("Safari on iPhone".into(), true)),
        "{listed:?}"
    );
    assert!(
        listed.contains(&("Chrome on Mac".into(), false)),
        "{listed:?}"
    );
    // The common setup's own sign-in (unnamed) is there too.
    assert_eq!(listed.len(), 3, "{listed:?}");
    let agent = "Mozilla/5.0 (iPhone; CPU iPhone OS 18_1 like Mac OS X) AppleWebKit/605.1.15 (KHTML, like Gecko) Version/18.1 Mobile/15E148 Safari/604.1";
    assert_eq!(browser_label(agent).unwrap().as_str(), "Safari on iPhone");
}

#[tokio::test]
async fn you_rename_your_own_devices_only() {
    let (_temp, database, stuart, sam) = common::setup().await;
    assert_eq!(
        rename(&database, &sam, sam.device, "Sam's phone", NOW).await,
        DevicesOutcome::Done
    );
    assert_eq!(
        database
            .device(sam.device)
            .await
            .unwrap()
            .unwrap()
            .name
            .unwrap()
            .as_str(),
        "Sam's phone"
    );
    assert_eq!(
        rename(&database, &sam, stuart.device, "mine now", NOW).await,
        DevicesOutcome::NotFound,
        "someone else's"
    );
    assert_eq!(
        rename(&database, &sam, sam.device, "  ", NOW).await,
        DevicesOutcome::InvalidName {
            reason: DeviceNameError::Empty
        }
    );
}

#[tokio::test]
async fn you_sign_out_another_device_or_all_others_but_not_this_one_here() {
    let (_temp, database, _, sam) = common::setup().await;
    let phone = browser(&database, "sam", "Safari on iPhone").await;
    let laptop = browser(&database, "sam", "Chrome on Mac").await;
    assert_eq!(
        sign_out(&database, &sam, sam.device, NOW).await,
        DevicesOutcome::NotAllowed,
        "this device: use the account menu"
    );
    assert_eq!(
        sign_out(&database, &sam, phone.device, NOW).await,
        DevicesOutcome::Done
    );
    assert!(
        resolve(&database, phone.token_hash, NOW, Timers::NORMAL)
            .await
            .unwrap()
            .is_none()
    );
    assert_eq!(
        database
            .device(phone.device)
            .await
            .unwrap()
            .unwrap()
            .signed_out_at,
        Some(NOW)
    );
    assert_eq!(
        sign_out_others(&database, &sam, NOW).await,
        DevicesOutcome::Done
    );
    assert!(
        resolve(&database, laptop.token_hash, NOW, Timers::NORMAL)
            .await
            .unwrap()
            .is_none()
    );
    assert!(
        resolve(&database, sam.token_hash, NOW, Timers::NORMAL)
            .await
            .unwrap()
            .is_some(),
        "this one stays"
    );
}

/// Stuart (the household Admin) switched to Admin mode.
async fn admin_mode(database: &Database, stuart: &SignedIn) -> SignedIn {
    switch_mode(
        database,
        stuart,
        ActiveMode::Admin,
        Some("a long password".into()),
        NOW,
    )
    .await
    .unwrap();
    resolve(database, stuart.token_hash, NOW, Timers::NORMAL)
        .await
        .unwrap()
        .unwrap()
}

#[tokio::test]
async fn only_the_household_admin_in_admin_mode_sees_and_signs_out_anyones_devices() {
    let (_temp, database, stuart, sam) = common::setup().await;
    let clock = ManualClock::new(NOW);
    let phone = browser(&database, "sam", "Safari on iPhone").await;

    // Stuart is the Admin, but in Driver mode: refused.
    let driver = AdminContext {
        database: &database,
        admin: &stuart,
        health: Health::Healthy,
        clock: &clock,
    };
    assert_eq!(
        devices_of(&driver, sam.user).await,
        DevicesOutcome::NotAllowed
    );
    assert_eq!(
        admin_sign_out(&driver, sam.user, None).await,
        DevicesOutcome::NotAllowed
    );
    // Sam isn't an Admin at all.
    let not_admin = AdminContext {
        admin: &sam,
        ..driver
    };
    assert_eq!(
        devices_of(&not_admin, stuart.user).await,
        DevicesOutcome::NotAllowed
    );

    // In Admin mode: Sam's two devices, then one, then none.
    switch_mode(
        &database,
        &stuart,
        ActiveMode::Admin,
        Some("a long password".into()),
        NOW,
    )
    .await
    .unwrap();
    let admin = resolve(&database, stuart.token_hash, NOW, Timers::NORMAL)
        .await
        .unwrap()
        .unwrap();
    let context = AdminContext {
        database: &database,
        admin: &admin,
        health: Health::Healthy,
        clock: &clock,
    };
    assert_eq!(ids(&devices_of(&context, sam.user).await).len(), 2);
    assert_eq!(
        admin_sign_out(&context, sam.user, Some(phone.device)).await,
        DevicesOutcome::Done
    );
    assert_eq!(ids(&devices_of(&context, sam.user).await).len(), 1);
    assert_eq!(
        admin_sign_out(&context, sam.user, Some(phone.device)).await,
        DevicesOutcome::NotFound,
        "already signed out"
    );
    assert_eq!(
        admin_sign_out(&context, sam.user, None).await,
        DevicesOutcome::Done
    );
    assert!(ids(&devices_of(&context, sam.user).await).is_empty());
    assert!(
        resolve(&database, sam.token_hash, NOW, Timers::NORMAL)
            .await
            .unwrap()
            .is_none()
    );
    // The Users screen counts devices.
    let domain::api::UserAdminOutcome::Users { users } =
        server::user_admin::list_users(&context).await
    else {
        panic!("users");
    };
    let sams = users.iter().find(|user| user.id == sam.user).unwrap();
    assert_eq!(sams.devices, 0);
    let stuarts = users.iter().find(|user| user.id == stuart.user).unwrap();
    assert_eq!(stuarts.devices, 1);
}

#[tokio::test]
async fn all_of_the_admins_own_devices_leaves_the_one_in_use() {
    let (_temp, database, stuart, _) = common::setup().await;
    let clock = ManualClock::new(NOW);
    let admin = admin_mode(&database, &stuart).await;
    let context = AdminContext {
        database: &database,
        admin: &admin,
        health: Health::Healthy,
        clock: &clock,
    };
    let _ = browser(&database, "stuart", "Safari on iPhone").await;
    assert_eq!(ids(&devices_of(&context, stuart.user).await).len(), 2);
    assert_eq!(
        admin_sign_out(&context, stuart.user, None).await,
        DevicesOutcome::Done
    );
    let left = ids(&devices_of(&context, stuart.user).await);
    assert_eq!(left.len(), 1);
    assert!(left[0].1, "the Admin's own device stays: {left:?}");
    assert_eq!(
        admin_sign_out(&context, stuart.user, Some(stuart.device)).await,
        DevicesOutcome::NotFound,
        "not the device in use"
    );
}
