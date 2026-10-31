//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod connection;
mod diagnostics;
mod placeholders;
mod settings;
mod users;

pub(crate) use account::{ChangePasswordForm, ForcedPasswordChange, Setup, SignIn};
pub(crate) use connection::{ConnectionPanel, problem_from_error};
pub(crate) use diagnostics::Diagnostics;
pub(crate) use placeholders::{
    Due, Garage, LogEvent, Messages, NotFound, Reports, Timeline, VehicleDetail, Vehicles,
};
pub(crate) use settings::Settings;
pub(crate) use users::Users;
