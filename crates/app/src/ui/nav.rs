//! Which screens appear in the sidebar and bottom tabs (plan §5 UI direction).

use domain::i18n::{MessageId, messages};

use super::icons::Icon;
use super::routes::Route;

/// Where a navigation item sits in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavSection {
    /// Everyday screens: Garage, Due & squawks, Messages, Timeline, Reports.
    Main,
    /// Below the divider: Vehicles, Settings, Diagnostics.
    Manage,
}

/// One entry in the navigation.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct NavItem {
    pub(crate) route: Route,
    pub(crate) label: MessageId,
    pub(crate) icon: Icon,
    pub(crate) section: NavSection,
    /// Also shown in the bottom tab bar on narrow screens.
    pub(crate) bottom_tab: bool,
}

/// Every navigation item, in sidebar order.
pub(crate) fn nav_items() -> Vec<NavItem> {
    let item = |route, label, icon, section, bottom_tab| NavItem {
        route,
        label,
        icon,
        section,
        bottom_tab,
    };
    vec![
        item(
            Route::Garage {},
            messages::NAV_GARAGE,
            Icon::Garage,
            NavSection::Main,
            true,
        ),
        item(
            Route::Due {},
            messages::NAV_DUE_AND_SQUAWKS,
            Icon::Due,
            NavSection::Main,
            true,
        ),
        item(
            Route::Messages {},
            messages::NAV_MESSAGES,
            Icon::Messages,
            NavSection::Main,
            true,
        ),
        item(
            Route::Timeline {},
            messages::NAV_TIMELINE,
            Icon::Timeline,
            NavSection::Main,
            false,
        ),
        item(
            Route::Reports {},
            messages::NAV_REPORTS,
            Icon::Reports,
            NavSection::Main,
            false,
        ),
        item(
            Route::Vehicles {},
            messages::NAV_VEHICLES,
            Icon::Vehicles,
            NavSection::Manage,
            false,
        ),
        item(
            Route::Settings {},
            messages::NAV_SETTINGS,
            Icon::Settings,
            NavSection::Manage,
            false,
        ),
        item(
            Route::Diagnostics {},
            messages::NAV_DIAGNOSTICS,
            Icon::Diagnostics,
            NavSection::Manage,
            false,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_nav_item_has_a_route_message_and_icon() {
        let items = nav_items();
        assert_eq!(items.len(), 8);
        for item in &items {
            // The nav label and the top-bar title come from the same message.
            assert_eq!(item.route.title(), item.label, "{:?}", item.route);
        }
    }

    #[test]
    fn bottom_tabs_are_garage_due_and_messages() {
        let tabs: Vec<Route> = nav_items()
            .into_iter()
            .filter(|item| item.bottom_tab)
            .map(|item| item.route)
            .collect();
        assert_eq!(tabs, [Route::Garage {}, Route::Due {}, Route::Messages {}]);
    }
}
