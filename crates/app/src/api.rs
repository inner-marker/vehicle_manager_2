//! Server functions: the app's HTTP API, all under `/api/` (`domain::api::API_PATH_PREFIX`).
//!
//! Every function here is guarded by the auth layer from `server::auth`, which checks
//! the signed-in session and API version before the body runs; only the few public paths
//! (`server::auth::PUBLIC_API_PATHS`) work without a session. Tests prove no other
//! function escapes it. Keep these thin: authenticate (done by the layer), call into
//! `server`/`domain`, respond (AGENTS.md §3).
//!
//! **Never call a server function during server-side rendering.** On the server such
//! a call runs in-process and skips the HTTP layer (and so the auth check); call them
//! from event handlers or effects, which only run in the browser or app.

use dioxus::prelude::*;
use domain::api::{
    AssetsOutcome, AttachmentsOutcome, ChangePasswordOutcome, CheckSetupCodeOutcome,
    DevicesOutcome, DocumentsOutcome, EventsOutcome, Handshake, InboxOutcome, MarkMessage,
    PlacesOutcome, Pong, PreferencesOutcome, RecoveryOutcome, ReferencesOutcome, ScheduleOutcome,
    SessionState, SetupOutcome, SignInOutcome, SquawksOutcome, SwitchModeOutcome, UserAdminOutcome,
};
use domain::secret::Secret;
use domain::sync::PullOutcome;

// Imports used only inside function bodies are declared there: the `#[get]` macro
// keeps bodies in the server build only, so top-level imports would be unused on web.

/// Liveness check: answers with the server's clock.
///
/// # Errors
///
/// Refused by the auth layer (401 / 400 / 426 / 503 with an `ApiError` reason) without a
/// valid session or with a wrong API version; otherwise never fails.
#[get("/api/v1/ping")]
// Server functions must be async; these two don't await anything yet. The lint fires
// on the server-side wrapper `#[get]` generates, so it applies to the server build only.
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; no awaits yet"
    )
)]
pub(crate) async fn ping() -> ServerFnResult<Pong> {
    use crate::logging::SystemClock;
    use domain::time::Clock;

    Ok(Pong {
        server_time_ms: SystemClock.now().as_millis(),
    })
}

/// What the server is: app version, API versions it speaks and accepts, and its clock.
///
/// # Errors
///
/// Refused by the auth layer like [`ping`]; otherwise never fails.
#[get("/api/v1/handshake")]
// Server functions must be async; these two don't await anything yet. The lint fires
// on the server-side wrapper `#[get]` generates, so it applies to the server build only.
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; no awaits yet"
    )
)]
pub(crate) async fn handshake() -> ServerFnResult<Handshake> {
    use crate::logging::SystemClock;
    use domain::api::{API_VERSION, MIN_SUPPORTED_API_VERSION};
    use domain::time::Clock;

    Ok(Handshake {
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        api_version: API_VERSION,
        min_supported_api_version: MIN_SUPPORTED_API_VERSION,
        server_time_ms: SystemClock.now().as_millis(),
        healthy: ::server::health::current().is_healthy(),
        build: crate::build_info::this_build(),
        database_id: ::server::startup::database_id().map(|id| id.to_string()),
    })
}

/// Signs in. On success the web app gets an `HttpOnly` session cookie; `want_token` (the
/// desktop app) also returns the token, to send as `Authorization: Bearer`.
///
/// A public path ([`::server::auth::PUBLIC_API_PATHS`]): it works without a session.
///
/// # Errors
///
/// Only transport errors; refusals are [`SignInOutcome`] values.
#[post("/api/v1/sign-in")]
pub(crate) async fn sign_in(
    username: String,
    password: Secret,
    want_token: bool,
    pairing: Option<domain::api::Pairing>,
) -> ServerFnResult<SignInOutcome> {
    use ::server::sessions::{SignInError, SigningInDevice};

    let Some(database) = ::server::startup::database() else {
        return Ok(SignInOutcome::Unavailable);
    };
    // A desktop pairs with its key (#073); one too old to (API version 1) still gets a token.
    let device = match pairing {
        Some(pairing) => {
            let Some(public_key) = domain::device_signature::parse_public_key(&pairing.public_key)
            else {
                tracing::warn!(target: domain::log_area::AUTH, "sign-in refused: the pairing key isn't usable");
                return Ok(SignInOutcome::Unavailable);
            };
            SigningInDevice::Desktop {
                public_key,
                name: pairing
                    .name
                    .as_deref()
                    .and_then(domain::device::DeviceName::tidy),
            }
        }
        None if want_token => SigningInDevice::OlderDesktop,
        None => SigningInDevice::Browser {
            name: server_side::user_agent()
                .as_deref()
                .and_then(domain::device::browser_label),
        },
    };
    let now = server_side::now();
    let client = server_side::client_key();
    match ::server::sessions::sign_in_from(
        database,
        &server_side::LOGIN_LIMITER,
        &username,
        password,
        &client,
        now,
        device,
    )
    .await
    {
        Ok((token, signed_in)) => {
            server_side::set_session_cookie(Some(token.as_str()));
            Ok(SignInOutcome::SignedIn {
                session: server_side::session_info(&signed_in),
                token: want_token.then(|| Secret::new(token.as_str())),
            })
        }
        Err(SignInError::InvalidCredentials) => Ok(SignInOutcome::InvalidCredentials),
        Err(SignInError::RateLimited { retry_after_millis }) => {
            Ok(SignInOutcome::TooManyAttempts {
                // Rounded up, so "try again in 0 seconds" never appears.
                retry_after_seconds: (retry_after_millis + 999) / 1000,
            })
        }
        Err(SignInError::Unavailable) => Ok(SignInOutcome::Unavailable),
    }
}

/// Who is signed in on this session (if anyone), and whether the server still needs its first
/// Admin. A public path, so the app can ask before signing in.
///
/// # Errors
///
/// Only transport errors.
#[get("/api/v1/session")]
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; this one only reads the request"
    )
)]
pub(crate) async fn session() -> ServerFnResult<SessionState> {
    Ok(SessionState {
        signed_in: server_side::signed_in().map(|signed_in| server_side::session_info(&signed_in)),
        setup_needed: ::server::setup::is_open(),
        instance: ::server::startup::instance().map(|instance| instance.as_str().to_owned()),
    })
}

/// Creates the first Admin with the setup code from the server log, and signs them in.
/// A public path; it works only while the server has no users.
///
/// # Errors
///
/// Only transport errors; refusals are [`SetupOutcome`] values.
#[post("/api/v1/setup")]
pub(crate) async fn setup(
    code: Secret,
    username: String,
    display_name: String,
    password: Secret,
    want_token: bool,
) -> ServerFnResult<SetupOutcome> {
    use ::server::setup::SetupError;

    let (Some(database), Some(device)) = (
        ::server::startup::database(),
        ::server::startup::server_device(),
    ) else {
        return Ok(SetupOutcome::Unavailable);
    };
    let now = server_side::now();
    // Wrong codes count against the same limit as wrong passwords, by client.
    let client_key = format!("setup:{}", server_side::client_key());
    if let Err(retry_after_millis) = server_side::LOGIN_LIMITER.check(&[&client_key], now) {
        return Ok(SetupOutcome::TooManyAttempts {
            retry_after_seconds: (retry_after_millis + 999) / 1000,
        });
    }
    let request = ::server::setup::SetupRequest {
        code,
        username: username.clone(),
        display_name,
        password: password.clone(),
    };
    let created = ::server::setup::create_first_admin(
        database,
        device,
        ::server::health::current(),
        &crate::logging::SystemClock,
        request,
    )
    .await;
    match created {
        Ok(_) => {}
        Err(SetupError::WrongCode) => {
            server_side::LOGIN_LIMITER.record_failure(&[&client_key], now);
            return Ok(SetupOutcome::WrongCode);
        }
        Err(SetupError::AlreadySetUp) => return Ok(SetupOutcome::AlreadySetUp),
        Err(SetupError::InvalidUsername(reason)) => {
            return Ok(SetupOutcome::InvalidUsername { reason });
        }
        Err(SetupError::InvalidDisplayName(reason)) => {
            return Ok(SetupOutcome::InvalidDisplayName { reason });
        }
        Err(SetupError::InvalidPassword(reason)) => {
            return Ok(SetupOutcome::InvalidPassword { reason });
        }
        Err(SetupError::Unavailable) => return Ok(SetupOutcome::Unavailable),
    }
    // Signed straight in, so the new Admin doesn't have to type it all again.
    Ok(match sign_in(username, password, want_token, None).await? {
        SignInOutcome::SignedIn { session, token } => SetupOutcome::SignedIn { session, token },
        SignInOutcome::InvalidCredentials
        | SignInOutcome::TooManyAttempts { .. }
        | SignInOutcome::Unavailable => SetupOutcome::Unavailable,
    })
}

/// Setup step 1: checks the setup code before the person types their account details.
/// A public path; wrong codes count toward the same limit as in [`setup`].
///
/// # Errors
///
/// Only transport errors.
#[post("/api/v1/setup/code")]
#[cfg_attr(
    feature = "server",
    expect(
        clippy::unused_async,
        reason = "server functions must be async; this one only checks memory"
    )
)]
pub(crate) async fn check_setup_code(code: Secret) -> ServerFnResult<CheckSetupCodeOutcome> {
    if !::server::setup::is_open() {
        return Ok(CheckSetupCodeOutcome::AlreadySetUp);
    }
    let now = server_side::now();
    let client_key = format!("setup:{}", server_side::client_key());
    if let Err(retry_after_millis) = server_side::LOGIN_LIMITER.check(&[&client_key], now) {
        return Ok(CheckSetupCodeOutcome::TooManyAttempts {
            retry_after_seconds: (retry_after_millis + 999) / 1000,
        });
    }
    if ::server::setup::code_is_valid(code.expose()) {
        Ok(CheckSetupCodeOutcome::Valid)
    } else {
        server_side::LOGIN_LIMITER.record_failure(&[&client_key], now);
        Ok(CheckSetupCodeOutcome::WrongCode)
    }
}

/// Changes the signed-in user's password; their other sessions are signed out.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ChangePasswordOutcome`] values.
#[post("/api/v1/password")]
pub(crate) async fn change_password(
    current: Secret,
    new: Secret,
) -> ServerFnResult<ChangePasswordOutcome> {
    use ::server::sessions::ChangePasswordError;

    let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
    else {
        return Ok(ChangePasswordOutcome::NotSignedIn);
    };
    Ok(
        match ::server::sessions::change_password(
            database,
            &signed_in,
            current,
            new,
            ::server::health::current(),
            &crate::logging::SystemClock,
        )
        .await
        {
            Ok(()) => ChangePasswordOutcome::Changed,
            Err(ChangePasswordError::WrongPassword) => ChangePasswordOutcome::WrongPassword,
            Err(ChangePasswordError::Invalid(reason)) => {
                ChangePasswordOutcome::InvalidNewPassword { reason }
            }
            Err(ChangePasswordError::Unavailable) => ChangePasswordOutcome::Unavailable,
        },
    )
}

/// Signs out: ends this session and clears the cookie.
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[post("/api/v1/sign-out")]
pub(crate) async fn sign_out() -> ServerFnResult<()> {
    if let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
        && let Err(error) = ::server::sessions::sign_out(database, signed_in.token_hash).await
    {
        tracing::error!(target: domain::log_area::AUTH, %error, "sign-out failed");
    }
    server_side::set_session_cookie(None);
    Ok(())
}

/// Switches this session's mode. Admin mode needs `password`.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SwitchModeOutcome`] values.
#[post("/api/v1/mode")]
pub(crate) async fn switch_mode(
    mode: domain::access::ActiveMode,
    password: Option<Secret>,
) -> ServerFnResult<SwitchModeOutcome> {
    use ::server::sessions::SwitchModeError;

    let (Some(signed_in), Some(database)) =
        (server_side::signed_in(), ::server::startup::database())
    else {
        return Ok(SwitchModeOutcome::NotSignedIn);
    };
    Ok(
        match ::server::sessions::switch_mode(
            database,
            &signed_in,
            mode,
            password,
            server_side::now(),
        )
        .await
        {
            Ok(mode) => SwitchModeOutcome::Switched { mode },
            Err(SwitchModeError::NotAllowed) => SwitchModeOutcome::NotAllowed,
            Err(SwitchModeError::WrongPassword) => SwitchModeOutcome::WrongPassword,
            Err(SwitchModeError::Unavailable) => SwitchModeOutcome::Unavailable,
        },
    )
}

/// Asks for a password-recovery code (decision #047). Always answers the same way; only for an
/// Admin does the server write a code to its log. A public path.
///
/// # Errors
///
/// Only transport errors.
#[post("/api/v1/recovery/request")]
pub(crate) async fn request_recovery(username: String) -> ServerFnResult<()> {
    if let Some(database) = ::server::startup::database() {
        // A rate-limited request gets the same answer: it must reveal nothing.
        let _ = ::server::recovery::request_code(
            database,
            &server_side::LOGIN_LIMITER,
            &username,
            &server_side::client_key(),
            server_side::now(),
            ::server::startup::timers(),
        )
        .await;
    }
    Ok(())
}

/// Completes a password recovery with the code from the server log and a new password, and
/// signs in. A public path.
///
/// # Errors
///
/// Only transport errors; refusals are [`RecoveryOutcome`] values.
#[post("/api/v1/recovery/complete")]
pub(crate) async fn complete_recovery(
    username: String,
    code: Secret,
    new_password: Secret,
    want_token: bool,
) -> ServerFnResult<RecoveryOutcome> {
    use ::server::recovery::RecoveryError;

    let Some(database) = ::server::startup::database() else {
        return Ok(RecoveryOutcome::Unavailable);
    };
    let result = ::server::recovery::complete(
        database,
        &server_side::LOGIN_LIMITER,
        &username,
        &code,
        new_password,
        &server_side::client_key(),
        &crate::logging::SystemClock,
    )
    .await;
    Ok(match result {
        Ok((token, signed_in)) => {
            server_side::set_session_cookie(Some(token.as_str()));
            RecoveryOutcome::SignedIn {
                session: server_side::session_info(&signed_in),
                token: want_token.then(|| Secret::new(token.as_str())),
            }
        }
        Err(RecoveryError::WrongCode) => RecoveryOutcome::WrongCode,
        Err(RecoveryError::RateLimited(millis)) => RecoveryOutcome::TooManyAttempts {
            retry_after_seconds: (millis + 999) / 1000,
        },
        Err(RecoveryError::InvalidPassword(reason)) => RecoveryOutcome::InvalidPassword { reason },
        Err(RecoveryError::Unavailable) => RecoveryOutcome::Unavailable,
    })
}

/// The signed-in user's display preferences: theirs, the household's, and the ones in effect.
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[get("/api/v1/preferences")]
pub(crate) async fn get_preferences() -> ServerFnResult<PreferencesOutcome> {
    Ok(
        server_side::with_preferences(async |context| ::server::preferences::view(&context).await)
            .await,
    )
}

/// Changes the signed-in user's own preferences.
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[post("/api/v1/preferences/mine")]
pub(crate) async fn set_my_preferences(
    patch: domain::preferences::PreferencesPatch,
) -> ServerFnResult<PreferencesOutcome> {
    Ok(server_side::with_preferences(async |context| {
        ::server::preferences::set_mine(&context, patch).await
    })
    .await)
}

/// Changes the household defaults (the household's Admin, in Admin mode).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PreferencesOutcome`] values.
#[post("/api/v1/preferences/household")]
pub(crate) async fn set_household_preferences(
    patch: domain::preferences::PreferencesPatch,
) -> ServerFnResult<PreferencesOutcome> {
    Ok(server_side::with_preferences(async |context| {
        ::server::preferences::set_household(&context, patch).await
    })
    .await)
}

/// The signed-in person's messages: what they can see in their mode, unread first (#050).
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[get("/api/v1/messages")]
pub(crate) async fn get_messages() -> ServerFnResult<InboxOutcome> {
    Ok(server_side::with_messages(async |context| ::server::messages::view(&context).await).await)
}

/// Marks one of the signed-in person's messages read, or dismisses it.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`InboxOutcome`] values.
#[post("/api/v1/messages/mark")]
pub(crate) async fn mark_message(key: String, action: MarkMessage) -> ServerFnResult<InboxOutcome> {
    let Some(key) = domain::inbox::MessageKey::parse(&key) else {
        return Ok(InboxOutcome::NotFound);
    };
    Ok(server_side::with_messages(async |context| {
        ::server::messages::mark(&context, key, action).await
    })
    .await)
}

/// The Garage: the groups and vehicles the signed-in person can see (#052).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[get("/api/v1/assets")]
pub(crate) async fn get_garage() -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| ::server::assets::garage(&context).await).await)
}

/// One vehicle's page: details, meters, custom fields, and what the person may do.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/page")]
pub(crate) async fn vehicle_page(id: domain::asset::AssetId) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| ::server::assets::view(&context, id).await).await)
}

/// Saves the signed-in person's custom Garage order (#056).
///
/// # Errors
///
/// Refused by the auth layer without a session; otherwise only transport errors.
#[post("/api/v1/assets/order")]
pub(crate) async fn set_garage_order(
    order: Vec<domain::asset::AssetId>,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| {
            ::server::assets::set_order(&context, order).await
        })
        .await,
    )
}

/// Adds a vehicle, with copies of its profile's meters and fields (the household's Admin).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/add")]
pub(crate) async fn add_vehicle(
    form: domain::asset_form::AssetForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| ::server::assets::add(&context, &form).await)
            .await,
    )
}

/// Changes a vehicle's details or lifecycle status (not its kind or group).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/edit")]
pub(crate) async fn edit_vehicle(
    id: domain::asset::AssetId,
    form: domain::asset_form::AssetForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| ::server::assets::edit(&context, id, &form).await)
            .await,
    )
}

/// Who gains, loses, or changes access if a vehicle moves to another group.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/move-preview")]
pub(crate) async fn preview_vehicle_move(
    id: domain::asset::AssetId,
    to: domain::group::GroupId,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::preview_move(&context, id, to).await
    })
    .await)
}

/// Moves a vehicle to another group (the household's Admin).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/move")]
pub(crate) async fn move_vehicle(
    id: domain::asset::AssetId,
    to: domain::group::GroupId,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| ::server::assets::move_to(&context, id, to).await)
            .await,
    )
}

/// Deletes a vehicle added by mistake, if `typed_name` is its name (the household's Admin).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/assets/delete")]
pub(crate) async fn delete_vehicle(
    id: domain::asset::AssetId,
    typed_name: String,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::delete(&context, id, &typed_name).await
    })
    .await)
}

/// Adds a meter to a vehicle (Maintainer work).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/meters/add")]
pub(crate) async fn add_vehicle_meter(
    asset: domain::asset::AssetId,
    form: domain::asset_form::MeterForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::add_meter(&context, asset, &form).await
    })
    .await)
}

/// Changes a meter's settings (not what it measures).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/meters/edit")]
pub(crate) async fn edit_vehicle_meter(
    id: domain::meter::MeterId,
    form: domain::asset_form::MeterForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::edit_meter(&context, id, &form).await
    })
    .await)
}

/// Removes a meter.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/meters/delete")]
pub(crate) async fn delete_vehicle_meter(
    id: domain::meter::MeterId,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| {
            ::server::assets::delete_meter(&context, id).await
        })
        .await,
    )
}

/// Adds a custom field to a vehicle (Maintainer work).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/fields/add")]
pub(crate) async fn add_vehicle_field(
    asset: domain::asset::AssetId,
    form: domain::asset_form::FieldForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::add_field(&context, asset, &form).await
    })
    .await)
}

/// Changes a custom field's label, on/off, or range.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/fields/edit")]
pub(crate) async fn edit_vehicle_field(
    id: domain::custom_field::CustomFieldId,
    form: domain::asset_form::FieldForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::assets::edit_field(&context, id, &form).await
    })
    .await)
}

/// Removes a custom field.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/fields/delete")]
pub(crate) async fn delete_vehicle_field(
    id: domain::custom_field::CustomFieldId,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| {
            ::server::assets::delete_field(&context, id).await
        })
        .await,
    )
}

/// Adds a group (the household's Admin).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/groups/add")]
pub(crate) async fn add_vehicle_group(
    form: domain::asset_form::GroupForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| {
            ::server::groups::add_group(&context, &form).await
        })
        .await,
    )
}

/// Changes a group's name or icon.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/groups/edit")]
pub(crate) async fn edit_vehicle_group(
    id: domain::group::GroupId,
    form: domain::asset_form::GroupForm,
) -> ServerFnResult<AssetsOutcome> {
    Ok(server_side::with_assets(async |context| {
        ::server::groups::edit_group(&context, id, &form).await
    })
    .await)
}

/// Removes an empty group (never the last).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AssetsOutcome`] values.
#[post("/api/v1/groups/delete")]
pub(crate) async fn delete_vehicle_group(
    id: domain::group::GroupId,
) -> ServerFnResult<AssetsOutcome> {
    Ok(
        server_side::with_assets(async |context| {
            ::server::groups::delete_group(&context, id).await
        })
        .await,
    )
}

/// The signed-in person's saved places (#059).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[get("/api/v1/places")]
pub(crate) async fn get_places() -> ServerFnResult<PlacesOutcome> {
    Ok(server_side::with_places(async |context| ::server::places::list(&context).await).await)
}

/// Adds a saved place for the signed-in person.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[post("/api/v1/places/add")]
pub(crate) async fn add_place(
    form: domain::location::SavedPlaceForm,
) -> ServerFnResult<PlacesOutcome> {
    Ok(
        server_side::with_places(async |context| ::server::places::add(&context, &form).await)
            .await,
    )
}

/// Changes one of the signed-in person's saved places.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[post("/api/v1/places/edit")]
pub(crate) async fn edit_place(
    id: domain::location::SavedPlaceId,
    form: domain::location::SavedPlaceForm,
) -> ServerFnResult<PlacesOutcome> {
    Ok(
        server_side::with_places(async |context| ::server::places::edit(&context, id, &form).await)
            .await,
    )
}

/// Removes one of the signed-in person's saved places.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[post("/api/v1/places/delete")]
pub(crate) async fn delete_place(
    id: domain::location::SavedPlaceId,
) -> ServerFnResult<PlacesOutcome> {
    Ok(
        server_side::with_places(async |context| ::server::places::delete(&context, id).await)
            .await,
    )
}

/// What's known about a point: the person's saved place it's in, and the suggested city,
/// state, country, and time zone (#038, #059).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[post("/api/v1/places/locate")]
pub(crate) async fn locate(
    coordinates: domain::location::Coordinates,
) -> ServerFnResult<PlacesOutcome> {
    Ok(server_side::with_places(async |context| {
        ::server::places::locate(&context, coordinates).await
    })
    .await)
}

/// Every country, by name, for typing a place by hand.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[get("/api/v1/places/countries")]
pub(crate) async fn get_countries() -> ServerFnResult<PlacesOutcome> {
    Ok(server_side::with_places(async |context| ::server::places::countries(&context).await).await)
}

/// About the place data: its version and credit (for About on Diagnostics).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PlacesOutcome`] values.
#[get("/api/v1/places/data")]
pub(crate) async fn get_place_data() -> ServerFnResult<PlacesOutcome> {
    Ok(
        server_side::with_places(async |context| ::server::places::data_info(context.geocoder))
            .await,
    )
}

/// One vehicle's fuel-ups and readings, newest first (milestone 6).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events")]
pub(crate) async fn vehicle_events(asset: domain::asset::AssetId) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| ::server::events::list(&context, asset).await)
            .await,
    )
}

/// One event, and what the person may do with it.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/view")]
pub(crate) async fn event_view(id: domain::event::EventId) -> ServerFnResult<EventsOutcome> {
    Ok(server_side::with_events(async |context| ::server::events::view(&context, id).await).await)
}

/// What the log form needs: each meter's latest reading, and whether the person may keep a
/// reading that doesn't fit.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/log-context")]
pub(crate) async fn event_log_context(
    asset: domain::asset::AssetId,
) -> ServerFnResult<EventsOutcome> {
    Ok(server_side::with_events(async |context| {
        ::server::events::log_context(&context, asset).await
    })
    .await)
}

/// Logs a fuel-up or reading, as a draft.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/add")]
pub(crate) async fn add_event(input: domain::event::EventInput) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| ::server::events::add(&context, input).await)
            .await,
    )
}

/// Changes a draft.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/edit")]
pub(crate) async fn edit_event(
    id: domain::event::EventId,
    input: domain::event::EventInput,
) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| ::server::events::edit(&context, id, input).await)
            .await,
    )
}

/// **Finalize now**: locks a draft.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/finalize")]
pub(crate) async fn finalize_event(id: domain::event::EventId) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| {
            ::server::events::finalize_now(&context, id).await
        })
        .await,
    )
}

/// Deletes a draft.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/delete")]
pub(crate) async fn delete_event(id: domain::event::EventId) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| ::server::events::delete(&context, id).await)
            .await,
    )
}

/// Corrects a finalized event with a new one, and a reason.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/correct")]
pub(crate) async fn correct_event(
    original: domain::event::EventId,
    input: domain::event::EventInput,
    reason: String,
) -> ServerFnResult<EventsOutcome> {
    Ok(server_side::with_events(async |context| {
        ::server::events::correct(&context, original, input, &reason).await
    })
    .await)
}

/// **Finish**: locks a maintenance or repair draft once it passes Finish's checks (every
/// outstanding squawk signed off or deferred in it).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`EventsOutcome`] values.
#[post("/api/v1/events/finish")]
pub(crate) async fn finish_event(id: domain::event::EventId) -> ServerFnResult<EventsOutcome> {
    Ok(
        server_side::with_events(async |context| ::server::events::finish(&context, id).await)
            .await,
    )
}

/// One vehicle's squawks, newest first (milestone 7).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SquawksOutcome`] values.
#[post("/api/v1/squawks")]
pub(crate) async fn vehicle_squawks(
    asset: domain::asset::AssetId,
) -> ServerFnResult<SquawksOutcome> {
    Ok(
        server_side::with_squawks(async |context| ::server::squawks::list(&context, asset).await)
            .await,
    )
}

/// One squawk, with its history.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SquawksOutcome`] values.
#[post("/api/v1/squawks/view")]
pub(crate) async fn squawk_view(id: domain::squawk::SquawkId) -> ServerFnResult<SquawksOutcome> {
    Ok(
        server_side::with_squawks(async |context| ::server::squawks::view(&context, id).await)
            .await,
    )
}

/// Opens a squawk.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SquawksOutcome`] values.
#[post("/api/v1/squawks/open")]
pub(crate) async fn open_squawk(input: domain::api::SquawkInput) -> ServerFnResult<SquawksOutcome> {
    Ok(
        server_side::with_squawks(async |context| ::server::squawks::open(&context, input).await)
            .await,
    )
}

/// Changes a squawk no job has dealt with yet.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SquawksOutcome`] values.
#[post("/api/v1/squawks/edit")]
pub(crate) async fn edit_squawk(
    id: domain::squawk::SquawkId,
    input: domain::api::SquawkInput,
) -> ServerFnResult<SquawksOutcome> {
    Ok(server_side::with_squawks(async |context| {
        ::server::squawks::edit(&context, id, input).await
    })
    .await)
}

/// Removes a squawk opened by mistake (no job has dealt with it yet).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`SquawksOutcome`] values.
#[post("/api/v1/squawks/delete")]
pub(crate) async fn delete_squawk(id: domain::squawk::SquawkId) -> ServerFnResult<SquawksOutcome> {
    Ok(
        server_side::with_squawks(async |context| ::server::squawks::delete(&context, id).await)
            .await,
    )
}

/// What's attached to something on a vehicle (an event's includes its tasks'), oldest first.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AttachmentsOutcome`] values.
#[post("/api/v1/attachments")]
pub(crate) async fn attachments(
    asset: domain::asset::AssetId,
    owner: domain::attachment::AttachmentOwner,
) -> ServerFnResult<AttachmentsOutcome> {
    Ok(server_side::with_attachments(async |context, _| {
        ::server::attachments::list(&context, asset, owner).await
    })
    .await)
}

/// Adds an attachment from files already uploaded (`domain::api::UPLOAD_PATH`).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AttachmentsOutcome`] values.
#[post("/api/v1/attachments/add")]
pub(crate) async fn add_attachment(
    input: domain::api::AttachmentInput,
) -> ServerFnResult<AttachmentsOutcome> {
    Ok(server_side::with_attachments(async |context, store| {
        ::server::attachments::add(&context, store, input).await
    })
    .await)
}

/// Moves an attachment to another task of its event (or to the event itself).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AttachmentsOutcome`] values.
#[post("/api/v1/attachments/move")]
pub(crate) async fn move_attachment(
    id: domain::attachment::AttachmentId,
    task: Option<u16>,
) -> ServerFnResult<AttachmentsOutcome> {
    Ok(server_side::with_attachments(async |context, _| {
        ::server::attachments::move_to(&context, id, task).await
    })
    .await)
}

/// Removes an attachment.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`AttachmentsOutcome`] values.
#[post("/api/v1/attachments/delete")]
pub(crate) async fn remove_attachment(
    id: domain::attachment::AttachmentId,
) -> ServerFnResult<AttachmentsOutcome> {
    Ok(server_side::with_attachments(async |context, _| {
        ::server::attachments::remove(&context, id).await
    })
    .await)
}

/// One vehicle's official documents and license plates, each document's status as of `today`
/// (the person's date).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/documents")]
pub(crate) async fn documents(
    asset: domain::asset::AssetId,
    today: domain::dates::CalendarDate,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::list(&context, asset, &today).await
    })
    .await)
}

/// Adds an official document.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/documents/add")]
pub(crate) async fn add_document(
    input: domain::api::DocumentInput,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::add_document(&context, input).await
    })
    .await)
}

/// Renews an official document: a new one that supersedes it.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/documents/renew")]
pub(crate) async fn renew_document(
    previous: domain::document::AssetDocumentId,
    input: domain::api::DocumentInput,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::renew(&context, previous, input).await
    })
    .await)
}

/// Changes an official document.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/documents/edit")]
pub(crate) async fn edit_document(
    id: domain::document::AssetDocumentId,
    input: domain::api::DocumentInput,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::edit_document(&context, id, input).await
    })
    .await)
}

/// Removes an official document (and its scans) that nothing renews.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/documents/delete")]
pub(crate) async fn remove_document(
    id: domain::document::AssetDocumentId,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::remove_document(&context, id).await
    })
    .await)
}

/// Adds a license plate, retiring the current one.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/plates/add")]
pub(crate) async fn add_plate(
    input: domain::api::PlateInput,
    today: domain::dates::CalendarDate,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::add_plate(&context, input, &today).await
    })
    .await)
}

/// Changes a license plate.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/plates/edit")]
pub(crate) async fn edit_plate(
    id: domain::plate::LicensePlateId,
    input: domain::api::PlateInput,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::edit_plate(&context, id, input).await
    })
    .await)
}

/// Removes a license plate no registration names.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DocumentsOutcome`] values.
#[post("/api/v1/plates/delete")]
pub(crate) async fn remove_plate(
    id: domain::plate::LicensePlateId,
) -> ServerFnResult<DocumentsOutcome> {
    Ok(server_side::with_documents(async |context| {
        ::server::documents::remove_plate(&context, id).await
    })
    .await)
}

/// The reference documents (recalls and bulletins) the person can read, to link one to another
/// vehicle (#068).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ReferencesOutcome`] values.
#[post("/api/v1/references")]
pub(crate) async fn references() -> ServerFnResult<ReferencesOutcome> {
    Ok(
        server_side::with_references(async |context| ::server::references::list(&context).await)
            .await,
    )
}

/// One reference document.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ReferencesOutcome`] values.
#[post("/api/v1/references/one")]
pub(crate) async fn reference(
    id: domain::reference::ReferenceDocumentId,
) -> ServerFnResult<ReferencesOutcome> {
    Ok(
        server_side::with_references(async |context| {
            ::server::references::view(&context, id).await
        })
        .await,
    )
}

/// Adds a reference document, with its first vehicle's requirement for it.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ReferencesOutcome`] values.
#[post("/api/v1/references/add")]
pub(crate) async fn add_reference(
    input: domain::api::ReferenceInput,
    link: domain::api::ReferenceLink,
) -> ServerFnResult<ReferencesOutcome> {
    Ok(server_side::with_references(async |context| {
        ::server::references::add(&context, input, link).await
    })
    .await)
}

/// Changes a reference document.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ReferencesOutcome`] values.
#[post("/api/v1/references/edit")]
pub(crate) async fn edit_reference(
    id: domain::reference::ReferenceDocumentId,
    input: domain::api::ReferenceInput,
) -> ServerFnResult<ReferencesOutcome> {
    Ok(server_side::with_references(async |context| {
        ::server::references::edit(&context, id, input).await
    })
    .await)
}

/// Removes a reference document no requirement links (and its attachments).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ReferencesOutcome`] values.
#[post("/api/v1/references/delete")]
pub(crate) async fn remove_reference(
    id: domain::reference::ReferenceDocumentId,
) -> ServerFnResult<ReferencesOutcome> {
    Ok(server_side::with_references(async |context| {
        ::server::references::remove(&context, id).await
    })
    .await)
}

/// One vehicle's maintenance schedule: its requirements (each with when it was last done) and
/// bundles.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements")]
pub(crate) async fn vehicle_schedule(
    asset: domain::asset::AssetId,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(
        server_side::with_schedule(async |context| ::server::schedule::view(&context, asset).await)
            .await,
    )
}

/// Adds a requirement.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/add")]
pub(crate) async fn add_requirement(
    requirement: domain::requirement::Requirement,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::add_requirement(&context, requirement).await
    })
    .await)
}

/// Changes a requirement.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/edit")]
pub(crate) async fn edit_requirement(
    id: domain::requirement::RequirementId,
    requirement: domain::requirement::Requirement,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::edit_requirement(&context, id, requirement).await
    })
    .await)
}

/// Removes a requirement no task has done.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/delete")]
pub(crate) async fn delete_requirement(
    id: domain::requirement::RequirementId,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::delete_requirement(&context, id).await
    })
    .await)
}

/// Adds a service bundle.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/bundles/add")]
pub(crate) async fn add_bundle(
    bundle: domain::requirement::Bundle,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::add_bundle(&context, bundle).await
    })
    .await)
}

/// Changes a service bundle.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/bundles/edit")]
pub(crate) async fn edit_bundle(
    id: domain::requirement::BundleId,
    bundle: domain::requirement::Bundle,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::edit_bundle(&context, id, bundle).await
    })
    .await)
}

/// Removes a service bundle.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/bundles/delete")]
pub(crate) async fn delete_bundle(
    id: domain::requirement::BundleId,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::delete_bundle(&context, id).await
    })
    .await)
}

/// Copies another vehicle's schedule onto one that has none yet (#031).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`ScheduleOutcome`] values.
#[post("/api/v1/requirements/copy")]
pub(crate) async fn copy_schedule(
    from: domain::asset::AssetId,
    to: domain::asset::AssetId,
) -> ServerFnResult<ScheduleOutcome> {
    Ok(server_side::with_schedule(async |context| {
        ::server::schedule::copy(&context, from, to).await
    })
    .await)
}

/// Everyone, for the Users screen (Admin mode only).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`UserAdminOutcome`] values.
#[get("/api/v1/users")]
pub(crate) async fn list_users() -> ServerFnResult<UserAdminOutcome> {
    Ok(
        server_side::with_admin(async |context| ::server::user_admin::list_users(&context).await)
            .await,
    )
}

/// Adds a user with `role` on all vehicles; answers with their temporary password.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`UserAdminOutcome`] values.
#[post("/api/v1/users/add")]
pub(crate) async fn add_user(
    username: String,
    display_name: String,
    role: domain::access::Role,
) -> ServerFnResult<UserAdminOutcome> {
    Ok(server_side::with_admin(async |context| {
        ::server::user_admin::add_user(&context, &username, &display_name, role).await
    })
    .await)
}

/// Sets a user's role on all vehicles (`None` removes it).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`UserAdminOutcome`] values.
#[post("/api/v1/users/role")]
pub(crate) async fn set_user_role(
    user: domain::user::UserId,
    target: domain::access::GrantTarget,
    role: Option<domain::access::Role>,
) -> ServerFnResult<UserAdminOutcome> {
    Ok(server_side::with_admin(async |context| {
        ::server::user_admin::set_role(&context, user, target, role).await
    })
    .await)
}

/// Disables a user (signing them out everywhere) or enables them again.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`UserAdminOutcome`] values.
#[post("/api/v1/users/disabled")]
pub(crate) async fn set_user_disabled(
    user: domain::user::UserId,
    disabled: bool,
) -> ServerFnResult<UserAdminOutcome> {
    Ok(server_side::with_admin(async |context| {
        ::server::user_admin::set_disabled(&context, user, disabled).await
    })
    .await)
}

/// Gives a user a new temporary password and signs them out everywhere.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`UserAdminOutcome`] values.
#[post("/api/v1/users/reset-password")]
pub(crate) async fn reset_user_password(
    user: domain::user::UserId,
) -> ServerFnResult<UserAdminOutcome> {
    Ok(server_side::with_admin(async |context| {
        ::server::user_admin::reset_password(&context, user).await
    })
    .await)
}

// --- Devices (phase-3.md 2e, #073) ------------------------------------------------------

/// Your signed-in devices and browsers, this one marked.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[get("/api/v1/devices")]
pub(crate) async fn devices() -> ServerFnResult<DevicesOutcome> {
    Ok(
        server_side::with_signed_in_devices(async |database, signed_in, now| {
            ::server::devices::my_devices(database, signed_in, now).await
        })
        .await,
    )
}

/// Renames one of your devices.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/devices/rename")]
pub(crate) async fn rename_device(
    device: domain::id::DeviceId,
    name: String,
) -> ServerFnResult<DevicesOutcome> {
    Ok(
        server_side::with_signed_in_devices(async |database, signed_in, now| {
            ::server::devices::rename(database, signed_in, device, &name, now).await
        })
        .await,
    )
}

/// Signs out one of your devices (not this one).
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/devices/sign-out")]
pub(crate) async fn sign_out_device(
    device: domain::id::DeviceId,
) -> ServerFnResult<DevicesOutcome> {
    Ok(
        server_side::with_signed_in_devices(async |database, signed_in, now| {
            ::server::devices::sign_out(database, signed_in, device, now).await
        })
        .await,
    )
}

/// Signs out all your devices except this one.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/devices/sign-out-others")]
pub(crate) async fn sign_out_other_devices() -> ServerFnResult<DevicesOutcome> {
    Ok(
        server_side::with_signed_in_devices(async |database, signed_in, now| {
            ::server::devices::sign_out_others(database, signed_in, now).await
        })
        .await,
    )
}

/// This desktop was just unlocked after `count` wrong PINs or passwords, the last at `last_at`
/// (phase-3.md 3d): the household Admin sees it in Messages.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/devices/failed-unlocks")]
pub(crate) async fn report_failed_unlocks(
    count: u32,
    last_at: domain::time::UnixMillis,
) -> ServerFnResult<DevicesOutcome> {
    Ok(
        server_side::with_signed_in_devices(async |database, signed_in, now| {
            ::server::devices::report_failed_unlocks(database, signed_in, count, last_at, now).await
        })
        .await,
    )
}

/// The next page of changes after `cursor` for this desktop's copy (phase-3.md 4b, decision
/// #075): paired desktops only, and only if `schema` is the server's schema version.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`PullOutcome`] values.
#[post("/api/v1/sync/pull")]
pub(crate) async fn pull(cursor: u64, schema: i64) -> ServerFnResult<PullOutcome> {
    Ok(
        server_side::with_pull(async |database, database_id, signed_in| {
            ::server::sync::pull(database, database_id, signed_in, cursor, schema).await
        })
        .await,
    )
}

/// A person's signed-in devices: the household Admin, in Admin mode.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/users/devices")]
pub(crate) async fn user_devices(user: domain::user::UserId) -> ServerFnResult<DevicesOutcome> {
    Ok(server_side::with_admin_devices(async |context| {
        ::server::devices::devices_of(&context, user).await
    })
    .await)
}

/// Signs out one of a person's devices, or all of them (`device` `None`): the household
/// Admin, in Admin mode.
///
/// # Errors
///
/// Refused by the auth layer without a session; refusals are [`DevicesOutcome`] values.
#[post("/api/v1/users/devices/sign-out")]
pub(crate) async fn sign_out_user_devices(
    user: domain::user::UserId,
    device: Option<domain::id::DeviceId>,
) -> ServerFnResult<DevicesOutcome> {
    Ok(server_side::with_admin_devices(async |context| {
        ::server::devices::admin_sign_out(&context, user, device).await
    })
    .await)
}

/// Server-side helpers for the server functions above: the current request's signed-in
/// user and client, the session cookie, and the login limiter.
#[cfg(feature = "server")]
mod server_side {
    use std::sync::LazyLock;

    use dioxus::prelude::dioxus_fullstack::FullstackContext;
    use dioxus::prelude::dioxus_server::http::HeaderValue;
    use dioxus::prelude::dioxus_server::http::header::SET_COOKIE;
    use domain::api::SessionInfo;
    use domain::time::{Clock, UnixMillis};
    use server::login_limiter::LoginLimiter;
    use server::sessions::{IDLE_LIFETIME_MILLIS, SignedIn};

    use crate::logging::SystemClock;

    /// Failed sign-ins, shared by every request.
    pub(super) static LOGIN_LIMITER: LazyLock<LoginLimiter> = LazyLock::new(LoginLimiter::new);

    pub(super) fn now() -> UnixMillis {
        SystemClock.now()
    }

    /// The signed-in user the auth layer attached to this request.
    pub(super) fn signed_in() -> Option<SignedIn> {
        FullstackContext::current()?.extension::<SignedIn>()
    }

    /// Where the request came from, for the login limit: the address the trusted proxies
    /// (`tailscale serve`) put in `X-Forwarded-For`, never one the client wrote itself
    /// (`server::client_address`, decision #071).
    pub(super) fn client_key() -> String {
        let forwarded_for = FullstackContext::current().and_then(|context| {
            context
                .parts_mut()
                .headers
                .get_all("x-forwarded-for")
                .iter()
                .filter_map(|value| value.to_str().ok())
                .map(str::to_owned)
                .reduce(|all, next| format!("{all},{next}"))
        });
        server::client_address::client_from_forwarded(
            forwarded_for.as_deref(),
            server::client_address::proxy_hops(),
        )
    }

    /// The request's `User-Agent`, if it sent one (to name a browser; phase-3.md 2e).
    pub(super) fn user_agent() -> Option<String> {
        FullstackContext::current().and_then(|context| {
            context
                .parts_mut()
                .headers
                .get("user-agent")
                .and_then(|value| value.to_str().ok())
                .map(str::to_owned)
        })
    }

    /// Sets the session cookie to `token`, or clears it with `None`.
    pub(super) fn set_session_cookie(token: Option<&str>) {
        let name = server::auth::session_cookie_name();
        // HttpOnly: scripts can't read it. Secure: HTTPS only (tailscale serve; browsers also
        // allow it on localhost). SameSite=Strict: never sent from other sites' pages.
        let cookie = match token {
            Some(token) => format!(
                "{name}={token}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={}",
                IDLE_LIFETIME_MILLIS / 1000
            ),
            None => format!("{name}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0"),
        };
        if let (Some(context), Ok(value)) =
            (FullstackContext::current(), HeaderValue::from_str(&cookie))
        {
            context.add_response_header(SET_COOKIE, value);
        }
    }

    /// Runs `action` as the signed-in user in an Admin context, or answers that it's
    /// unavailable (no database) or not allowed (not signed in).
    pub(super) async fn with_admin(
        action: impl AsyncFnOnce(server::user_admin::AdminContext<'_>) -> domain::api::UserAdminOutcome,
    ) -> domain::api::UserAdminOutcome {
        let Some(database) = server::startup::database() else {
            return domain::api::UserAdminOutcome::Unavailable;
        };
        let Some(admin) = signed_in() else {
            return domain::api::UserAdminOutcome::NotAllowed;
        };
        let context = server::user_admin::AdminContext {
            database,
            admin: &admin,
            health: server::health::current(),
            clock: &SystemClock,
        };
        action(context).await
    }

    /// Runs `action` with the database, the signed-in user, and now, or answers that it's
    /// unavailable or not signed in (phase-3.md 2e).
    pub(super) async fn with_signed_in_devices(
        action: impl AsyncFnOnce(
            &storage::Database,
            &SignedIn,
            UnixMillis,
        ) -> domain::api::DevicesOutcome,
    ) -> domain::api::DevicesOutcome {
        let Some(database) = server::startup::database() else {
            return domain::api::DevicesOutcome::Unavailable;
        };
        let Some(signed_in) = signed_in() else {
            return domain::api::DevicesOutcome::NotSignedIn;
        };
        action(database, &signed_in, now()).await
    }

    /// For a pull: the database, its ID, and who's asking (refusals as [`PullOutcome`]).
    pub(super) async fn with_pull(
        action: impl AsyncFnOnce(
            &storage::Database,
            domain::id::DatabaseId,
            &SignedIn,
        ) -> domain::sync::PullOutcome,
    ) -> domain::sync::PullOutcome {
        let (Some(database), Some(database_id)) =
            (server::startup::database(), server::startup::database_id())
        else {
            return domain::sync::PullOutcome::Unavailable;
        };
        let Some(signed_in) = signed_in() else {
            return domain::sync::PullOutcome::NotSignedIn;
        };
        action(database, database_id, &signed_in).await
    }

    /// As [`with_admin`], for the Admin's view of a person's devices.
    pub(super) async fn with_admin_devices(
        action: impl AsyncFnOnce(server::user_admin::AdminContext<'_>) -> domain::api::DevicesOutcome,
    ) -> domain::api::DevicesOutcome {
        let Some(database) = server::startup::database() else {
            return domain::api::DevicesOutcome::Unavailable;
        };
        let Some(admin) = signed_in() else {
            return domain::api::DevicesOutcome::NotAllowed;
        };
        let context = server::user_admin::AdminContext {
            database,
            admin: &admin,
            health: server::health::current(),
            clock: &SystemClock,
        };
        action(context).await
    }

    /// Runs `action` with the signed-in user's preferences context, or answers that it's
    /// unavailable (no database, or not signed in).
    pub(super) async fn with_preferences(
        action: impl AsyncFnOnce(
            server::preferences::PreferencesContext<'_>,
        ) -> domain::api::PreferencesOutcome,
    ) -> domain::api::PreferencesOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::PreferencesOutcome::Unavailable;
        };
        let context = server::preferences::PreferencesContext {
            database,
            database_id,
            user: &user,
            health: server::health::current(),
            clock: &SystemClock,
        };
        action(context).await
    }

    /// Runs `action` with the signed-in person's message center context, or answers that it's
    /// unavailable (no database, or nobody signed in).
    pub(super) async fn with_messages(
        action: impl AsyncFnOnce(server::messages::MessagesContext<'_>) -> domain::api::InboxOutcome,
    ) -> domain::api::InboxOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::InboxOutcome::Unavailable;
        };
        let context = server::messages::MessagesContext {
            database,
            database_id,
            user: &user,
            health: server::health::current(),
            problem: server::health::unhealthy_since(),
            clock: &SystemClock,
            timers: server::startup::timers(),
        };
        action(context).await
    }

    /// Runs `action` with the signed-in person's vehicles context, or answers that it's
    /// unavailable (no database, or nobody signed in).
    pub(super) async fn with_assets(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::AssetsOutcome,
    ) -> domain::api::AssetsOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::AssetsOutcome::Unavailable;
        };
        let context = server::assets::AssetsContext {
            database,
            database_id,
            user: &user,
            health: server::health::current(),
            clock: &SystemClock,
        };
        action(context).await
    }

    /// Runs `action` with the signed-in person's vehicles context for an events request, or
    /// answers that it's unavailable (no database, or nobody signed in).
    pub(super) async fn with_events(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::EventsOutcome,
    ) -> domain::api::EventsOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::EventsOutcome::Unavailable;
        };
        let context = server::assets::AssetsContext {
            database,
            database_id,
            user: &user,
            health: server::health::current(),
            clock: &SystemClock,
        };
        action(context).await
    }

    /// The signed-in person's vehicles context.
    fn assets_context<'a>(
        database: &'a storage::Database,
        database_id: domain::id::DatabaseId,
        user: &'a SignedIn,
    ) -> server::assets::AssetsContext<'a> {
        server::assets::AssetsContext {
            database,
            database_id,
            user,
            health: server::health::current(),
            clock: &SystemClock,
        }
    }

    /// Runs `action` with the signed-in person's vehicles context for a squawks request, or
    /// answers that it's unavailable (no database, or nobody signed in).
    pub(super) async fn with_squawks(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::SquawksOutcome,
    ) -> domain::api::SquawksOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::SquawksOutcome::Unavailable;
        };
        action(assets_context(database, database_id, &user)).await
    }

    /// Runs `action` with the signed-in person's vehicles context and the file store for an
    /// attachments request, or answers that it's unavailable (no database or files folder, or
    /// nobody signed in).
    pub(super) async fn with_attachments(
        action: impl AsyncFnOnce(
            server::assets::AssetsContext<'_>,
            &'static storage::files::FileStore,
        ) -> domain::api::AttachmentsOutcome,
    ) -> domain::api::AttachmentsOutcome {
        let (Some(database), Some(database_id), Some(user), Some(store)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
            server::startup::file_store(),
        ) else {
            return domain::api::AttachmentsOutcome::Unavailable;
        };
        action(assets_context(database, database_id, &user), store).await
    }

    /// Runs `action` with the signed-in person's vehicles context for a documents request, or
    /// answers that it's unavailable (no database, or nobody signed in).
    pub(super) async fn with_documents(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::DocumentsOutcome,
    ) -> domain::api::DocumentsOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::DocumentsOutcome::Unavailable;
        };
        action(assets_context(database, database_id, &user)).await
    }

    /// Runs `action` with the signed-in person's vehicles context for a reference documents
    /// request, or answers that it's unavailable (no database, or nobody signed in).
    pub(super) async fn with_references(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::ReferencesOutcome,
    ) -> domain::api::ReferencesOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::ReferencesOutcome::Unavailable;
        };
        action(assets_context(database, database_id, &user)).await
    }

    /// Runs `action` with the signed-in person's vehicles context for a schedule request, or
    /// answers that it's unavailable (no database, or nobody signed in).
    pub(super) async fn with_schedule(
        action: impl AsyncFnOnce(server::assets::AssetsContext<'_>) -> domain::api::ScheduleOutcome,
    ) -> domain::api::ScheduleOutcome {
        let (Some(database), Some(database_id), Some(user)) = (
            server::startup::database(),
            server::startup::database_id(),
            signed_in(),
        ) else {
            return domain::api::ScheduleOutcome::Unavailable;
        };
        action(assets_context(database, database_id, &user)).await
    }

    /// Runs `action` with the signed-in person's places context, or answers that it's
    /// unavailable (no database, or nobody signed in).
    pub(super) async fn with_places(
        action: impl AsyncFnOnce(server::places::PlacesContext<'_>) -> domain::api::PlacesOutcome,
    ) -> domain::api::PlacesOutcome {
        let (Some(database), Some(user)) = (server::startup::database(), signed_in()) else {
            return domain::api::PlacesOutcome::Unavailable;
        };
        let context = server::places::PlacesContext {
            database,
            user: &user,
            health: server::health::current(),
            clock: &SystemClock,
            geocoder: server::places::geocoder(),
        };
        action(context).await
    }

    pub(super) fn session_info(signed_in: &SignedIn) -> SessionInfo {
        SessionInfo {
            username: signed_in.username.clone(),
            display_name: signed_in.display_name.clone(),
            must_change_password: signed_in.must_change_password,
            mode: signed_in.mode,
            available_modes: signed_in.available_modes(),
            timers: server::startup::timers(),
        }
    }
}

/// Tests of the auth coverage guarantee: every registered server function is under
/// `/api/` and refuses unauthenticated requests. Server-only (the registry and router
/// exist only with the `server` feature); CI runs them with `--features server`.
#[cfg(all(test, feature = "server"))]
mod tests {
    use dioxus::prelude::dioxus_server::FullstackState;
    use dioxus::prelude::dioxus_server::axum::Router;
    use dioxus::prelude::dioxus_server::axum::body::Body;
    use dioxus::prelude::dioxus_server::http::{Request, StatusCode};
    use dioxus::prelude::{DioxusRouterExt, ServeConfig, ServerFunction};
    use domain::api::API_PATH_PREFIX;
    use tower::ServiceExt;

    use crate::ui::App;

    #[test]
    fn every_server_function_is_under_api_prefix() {
        let functions = ServerFunction::collect();
        assert!(!functions.is_empty(), "no server functions registered");
        for function in functions {
            assert!(
                function.path().starts_with(API_PATH_PREFIX),
                "{} {} is outside {API_PATH_PREFIX}, so the auth layer wouldn't guard it",
                function.method(),
                function.path()
            );
        }
    }

    #[test]
    fn every_public_path_is_a_real_server_function() {
        // A typo in the public list would silently leave a function protected, or worse,
        // name a path that a future function could take over unprotected.
        let registered: Vec<&str> = ServerFunction::collect()
            .into_iter()
            .map(ServerFunction::path)
            .collect();
        for public in ::server::auth::PUBLIC_API_PATHS {
            assert!(
                registered.contains(public),
                "{public} isn't a registered server function"
            );
        }
    }

    #[tokio::test]
    async fn every_server_function_rejects_unauthenticated_requests() {
        // The real server-function routes, behind the real auth layer.
        let server_functions =
            Router::<FullstackState>::new().serve_api_application(ServeConfig::new(), App);
        let guarded = ::server::auth::protect(
            server_functions,
            // A session source with no database: nothing can be signed in.
            ::server::auth::AuthConfig::new(::server::auth::SessionSource::new(
                || None,
                std::sync::Arc::new(crate::logging::SystemClock),
            )),
        );

        let protected = ServerFunction::collect()
            .into_iter()
            .filter(|function| !::server::auth::PUBLIC_API_PATHS.contains(&function.path()));
        for function in protected {
            let unauthenticated = Request::builder()
                .method(function.method())
                .uri(function.path())
                .body(Body::empty())
                .unwrap();
            let response = guarded.clone().oneshot(unauthenticated).await.unwrap();
            // 503 here (no database, so no session can be valid); 401 with one.
            assert!(
                matches!(
                    response.status(),
                    StatusCode::UNAUTHORIZED | StatusCode::SERVICE_UNAVAILABLE
                ),
                "{} {} answered without a session",
                function.method(),
                function.path()
            );
        }
    }
}
