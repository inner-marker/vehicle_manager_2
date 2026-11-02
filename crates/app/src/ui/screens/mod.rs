//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod connection;
mod diagnostics;
mod groups;
mod messages;
mod placeholders;
mod places;
mod roles;
mod settings;
mod users;
mod vehicle;
mod vehicle_setup;
mod vehicles;

pub(crate) use account::{
    ChangePasswordForm, Field, ForcedPasswordChange, FormError, Setup, SignIn,
};
pub(crate) use connection::{ConnectionPanel, problem_from_error};
pub(crate) use diagnostics::Diagnostics;
pub(crate) use groups::Groups;
pub(crate) use messages::Messages;
pub(crate) use placeholders::{Due, LogEvent, NotFound, Reports, Timeline};
pub(crate) use places::Places;
pub(crate) use settings::Settings;
pub(crate) use users::Users;
pub(crate) use vehicle::VehicleDetail;
pub(crate) use vehicles::{AllVehicles, GroupVehicles};
