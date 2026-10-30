//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod connection;
mod diagnostics;
mod placeholders;

pub(crate) use account::{ChangePasswordForm, ForcedPasswordChange, Setup, SignIn};
pub(crate) use connection::{ConnectionPanel, problem_from_error};
pub(crate) use diagnostics::Diagnostics;
pub(crate) use placeholders::{
    Due, Garage, LogEvent, Messages, NotFound, Reports, Settings, Timeline, VehicleDetail, Vehicles,
};
