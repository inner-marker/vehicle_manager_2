//! Every screen's address. All screens render inside the [`Shell`] layout.

use dioxus::prelude::*;
use domain::i18n::{MessageId, messages};

use super::screens::{
    Diagnostics, Due, Garage, LogEvent, Messages, NotFound, Reports, Settings, Timeline, Users,
    VehicleDetail, Vehicles,
};
use super::shell::Shell;

/// The app's routes. Each variant renders the component of the same name.
#[derive(Debug, Clone, PartialEq, Routable)]
#[rustfmt::skip]
pub(crate) enum Route {
    #[layout(Shell)]
        #[route("/")]
        Garage {},
        #[route("/due")]
        Due {},
        #[route("/messages")]
        Messages {},
        #[route("/timeline")]
        Timeline {},
        #[route("/reports")]
        Reports {},
        #[route("/vehicles")]
        Vehicles {},
        // The UI says "vehicle"; the field is `asset_id` (AGENTS.md terminology rule).
        #[route("/vehicles/:asset_id")]
        VehicleDetail { asset_id: String },
        #[route("/log")]
        LogEvent {},
        #[route("/users")]
        Users {},
        #[route("/settings")]
        Settings {},
        #[route("/diagnostics")]
        Diagnostics {},
        // Anything else.
        #[route("/:..segments")]
        NotFound { segments: Vec<String> },
}

impl Route {
    /// The title shown in the top bar for this screen.
    pub(crate) const fn title(&self) -> MessageId {
        match self {
            Self::Garage {} => messages::NAV_GARAGE,
            Self::Due {} => messages::NAV_DUE_AND_SQUAWKS,
            Self::Messages {} => messages::NAV_MESSAGES,
            Self::Timeline {} => messages::NAV_TIMELINE,
            Self::Reports {} => messages::NAV_REPORTS,
            Self::Vehicles {} => messages::NAV_VEHICLES,
            Self::VehicleDetail { .. } => messages::SCREEN_VEHICLE_DETAIL,
            Self::LogEvent {} => messages::SCREEN_LOG_EVENT,
            Self::Users {} => messages::NAV_USERS,
            Self::Settings {} => messages::NAV_SETTINGS,
            Self::Diagnostics {} => messages::NAV_DIAGNOSTICS,
            Self::NotFound { .. } => messages::SCREEN_NOT_FOUND,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::str::FromStr;

    use super::*;

    fn every_route() -> Vec<Route> {
        vec![
            Route::Garage {},
            Route::Due {},
            Route::Messages {},
            Route::Timeline {},
            Route::Reports {},
            Route::Vehicles {},
            Route::VehicleDetail {
                asset_id: "0192f3a4".to_owned(),
            },
            Route::LogEvent {},
            Route::Settings {},
            Route::Diagnostics {},
        ]
    }

    #[test]
    fn every_route_round_trips_through_its_path() {
        for route in every_route() {
            let path = route.to_string();
            assert_eq!(
                Route::from_str(&path).ok(),
                Some(route.clone()),
                "path {path}"
            );
        }
    }

    #[test]
    fn routes_use_the_planned_paths() {
        assert_eq!(Route::Garage {}.to_string(), "/");
        assert_eq!(Route::Due {}.to_string(), "/due");
        assert_eq!(
            Route::VehicleDetail {
                asset_id: "abc".to_owned()
            }
            .to_string(),
            "/vehicles/abc"
        );
    }

    #[test]
    fn unknown_paths_route_to_not_found() {
        let route = Route::from_str("/no/such/screen").ok();
        assert!(matches!(route, Some(Route::NotFound { .. })), "{route:?}");
    }
}
