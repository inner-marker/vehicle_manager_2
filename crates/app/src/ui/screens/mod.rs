//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod connection;
mod diagnostics;
mod messages;
mod placeholders;
mod settings;
mod users;
mod vehicles;

pub(crate) use account::{
    ChangePasswordForm, Field, ForcedPasswordChange, FormError, Setup, SignIn,
};
pub(crate) use connection::{ConnectionPanel, problem_from_error};
pub(crate) use diagnostics::Diagnostics;
pub(crate) use messages::Messages;
pub(crate) use placeholders::{Due, LogEvent, NotFound, Reports, Timeline, VehicleDetail};
pub(crate) use settings::Settings;
pub(crate) use users::Users;
pub(crate) use vehicles::{AllVehicles, GroupVehicles};
