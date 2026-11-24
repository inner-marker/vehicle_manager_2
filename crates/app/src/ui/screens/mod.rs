//! One component per screen. Most are placeholders until their phase builds them.

mod account;
mod attachments;
mod compliance;
mod connection;
mod devices;
mod diagnostics;
mod documents;
mod due_list;
mod due_view;
mod event_page;
mod formula_view;
mod fuel_view;
mod groups;
mod job_page;
mod labor_rates;
mod life_input;
mod life_part;
mod log_chooser;
mod log_form;
mod log_input;
mod maintenance;
mod messages;
mod not_sent;
mod placeholders;
mod places;
mod readiness_view;
mod references;
mod roles;
mod settings;
mod snooze;
mod specs_view;
mod squawks;
mod templates;
mod timeline;
mod users;
mod vehicle;
mod vehicle_setup;
mod vehicles;
mod work_input;
mod work_log;

#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) use account::too_many_attempts;
pub(crate) use account::{
    ChangePasswordForm, Field, ForcedPasswordChange, FormError, Setup, SignIn, TypeToConfirm,
};
pub(crate) use connection::{ConnectionPanel, problem_from_error};
pub(crate) use diagnostics::Diagnostics;
pub(crate) use due_list::{Due, DueList};
pub(crate) use event_page::EventDetail;
pub(crate) use groups::Groups;
pub(crate) use log_chooser::{LogChooserDialog, LogEvent, may_log_for, provide_log_chooser};
pub(crate) use log_form::{CorrectEvent, EditEvent, LogForm, LogService, RepairSquawk};
pub(crate) use messages::Messages;
pub(crate) use placeholders::{NotFound, Reports, Timeline};
pub(crate) use places::Places;
pub(crate) use settings::Settings;
pub(crate) use squawks::device_zone;
#[cfg(all(feature = "desktop", not(target_arch = "wasm32")))]
pub(crate) use squawks::use_device_zone;
pub(crate) use squawks::{SquawkDetail, SquawkSheetDialog, provide_squawk_sheet};
pub(crate) use templates::Templates;
pub(crate) use users::Users;
pub(crate) use vehicle::{VehicleDetail, VehicleTab};
pub(crate) use vehicle_setup::event_from_value;
pub(crate) use vehicles::{AllVehicles, GroupVehicles};
