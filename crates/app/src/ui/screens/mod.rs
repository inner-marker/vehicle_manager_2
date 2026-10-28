//! One component per screen. Most are placeholders until their phase builds them.

mod diagnostics;
mod placeholders;

pub(crate) use diagnostics::Diagnostics;
pub(crate) use placeholders::{
    Due, Garage, LogEvent, Messages, NotFound, Reports, Settings, Timeline, VehicleDetail, Vehicles,
};
