//! Which screens appear in the sidebar and bottom tabs (plan §5 UI direction).

use domain::i18n::{MessageId, messages};

use super::icons::Icon;
use super::routes::Route;

/// Where a navigation item sits in the sidebar.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum NavSection {
    /// The vehicles: the shell shows All vehicles and each group here instead of this item
    /// (#056); the item itself is the phone's Vehicles tab.
    Vehicles,
    /// Everyday screens: Due & squawks, Messages, Timeline, Reports.
    Main,
    /// Below the divider: Users and Groups (Admins only), Places, Settings, Diagnostics.
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
    /// Shown only to the household's Admin (who can enter Admin mode).
    pub(crate) admin_only: bool,
}

/// Every navigation item, in sidebar order.
pub(crate) fn nav_items() -> Vec<NavItem> {
    let item = |route, label, icon, section, bottom_tab| NavItem {
        route,
        label,
        icon,
        section,
        bottom_tab,
        admin_only: false,
    };
    // Below the divider; `admin` ones only for the household's Admin.
    let manage = |route, label, icon| item(route, label, icon, NavSection::Manage, false);
    let admin = |route, label, icon| NavItem {
        admin_only: true,
        ..manage(route, label, icon)
    };
    vec![
        item(
            Route::AllVehicles {},
            messages::NAV_VEHICLES,
            Icon::Vehicles,
            NavSection::Vehicles,
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
        admin(Route::Users {}, messages::NAV_USERS, Icon::Users),
        admin(Route::Groups {}, messages::NAV_GROUPS, Icon::Groups),
        manage(Route::Places {}, messages::NAV_PLACES, Icon::Places),
        manage(Route::Settings {}, messages::NAV_SETTINGS, Icon::Settings),
        manage(
            Route::Diagnostics {},
            messages::NAV_DIAGNOSTICS,
            Icon::Diagnostics,
        ),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_nav_item_has_a_route_message_and_icon() {
        let items = nav_items();
        assert_eq!(items.len(), 10);
        // The nav label and the top-bar title come from the same message, except the phone's
        // Vehicles tab, whose screen is titled "All vehicles".
        for item in items
            .iter()
            .filter(|item| item.section != NavSection::Vehicles)
        {
            assert_eq!(item.route.title(), item.label, "{:?}", item.route);
        }
    }

    #[test]
    fn only_users_and_groups_are_admin_only() {
        let admin_only: Vec<Route> = nav_items()
            .into_iter()
            .filter(|item| item.admin_only)
            .map(|item| item.route)
            .collect();
        assert_eq!(admin_only, [Route::Users {}, Route::Groups {}]);
    }

    #[test]
    fn bottom_tabs_are_vehicles_due_and_messages() {
        let tabs: Vec<Route> = nav_items()
            .into_iter()
            .filter(|item| item.bottom_tab)
            .map(|item| item.route)
            .collect();
        assert_eq!(
            tabs,
            [Route::AllVehicles {}, Route::Due {}, Route::Messages {}]
        );
    }
}
