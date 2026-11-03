//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod connection;
mod diagnostics;
mod event_page;
mod groups;
mod log_chooser;
mod log_form;
mod log_input;
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
pub(crate) use event_page::EventDetail;
pub(crate) use groups::Groups;
pub(crate) use log_chooser::{LogChooserDialog, LogEvent, provide_log_chooser};
pub(crate) use log_form::{CorrectEvent, EditEvent, LogForm};
pub(crate) use messages::Messages;
pub(crate) use placeholders::{Due, NotFound, Reports, Timeline};
pub(crate) use places::Places;
pub(crate) use settings::Settings;
pub(crate) use users::Users;
pub(crate) use vehicle::VehicleDetail;
pub(crate) use vehicles::{AllVehicles, GroupVehicles};
