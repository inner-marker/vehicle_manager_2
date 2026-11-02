//! Every screen's address. All screens render inside the [`Shell`] layout.

use dioxus::prelude::*;
use domain::i18n::{MessageId, messages};

use super::screens::{
    AllVehicles, Diagnostics, Due, GroupVehicles, Groups, LogEvent, Messages, NotFound, Places,
    Reports, Settings, Timeline, Users, VehicleDetail,
};
use super::shell::Shell;

/// The app's routes. Each variant renders the component of the same name.
#[derive(Debug, Clone, PartialEq, Routable)]
#[rustfmt::skip]
pub(crate) enum Route {
    #[layout(Shell)]
        // Every group's vehicles (#056); a group's own page is below.
        #[route("/")]
        AllVehicles {},
        #[route("/due")]
        Due {},
        #[route("/messages")]
        Messages {},
        #[route("/timeline")]
        Timeline {},
        #[route("/reports")]
        Reports {},
        // One group's vehicles (#056).
        #[route("/vehicles/group/:group_id")]
        GroupVehicles { group_id: String },
        // The UI says "vehicle"; the field is `asset_id` (AGENTS.md terminology rule).
        #[route("/vehicles/:asset_id")]
        VehicleDetail { asset_id: String },
        #[route("/log")]
        LogEvent {},
        #[route("/users")]
        Users {},
        // Managing groups (#058).
        #[route("/groups")]
        Groups {},
        // Each person's saved places (#060).
        #[route("/places")]
        Places {},
        #[route("/settings")]
        Settings {},
        #[route("/diagnostics")]
        Diagnostics {},
        // Anything else.
        #[route("/:..segments")]
        NotFound { segments: Vec<String> },
}

impl Route {
    /// The title shown in the top bar for this screen (a group's page shows the group's name
    /// instead; see the shell).
    pub(crate) const fn title(&self) -> MessageId {
        match self {
            Self::AllVehicles {} => messages::NAV_ALL_VEHICLES,
            Self::GroupVehicles { .. } => messages::NAV_VEHICLES,
            Self::Due {} => messages::NAV_DUE_AND_SQUAWKS,
            Self::Messages {} => messages::NAV_MESSAGES,
            Self::Timeline {} => messages::NAV_TIMELINE,
            Self::Reports {} => messages::NAV_REPORTS,
            Self::VehicleDetail { .. } => messages::SCREEN_VEHICLE_DETAIL,
            Self::LogEvent {} => messages::SCREEN_LOG_EVENT,
            Self::Users {} => messages::NAV_USERS,
            Self::Groups {} => messages::NAV_GROUPS,
            Self::Places {} => messages::NAV_PLACES,
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
            Route::AllVehicles {},
            Route::GroupVehicles {
                group_id: "0192f3a4".to_owned(),
            },
            Route::Due {},
            Route::Messages {},
            Route::Timeline {},
            Route::Reports {},
            Route::VehicleDetail {
                asset_id: "0192f3a4".to_owned(),
            },
            Route::LogEvent {},
            Route::Groups {},
            Route::Places {},
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
        assert_eq!(Route::AllVehicles {}.to_string(), "/");
        assert_eq!(
            Route::GroupVehicles {
                group_id: "abc".to_owned()
            }
            .to_string(),
            "/vehicles/group/abc"
        );
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
