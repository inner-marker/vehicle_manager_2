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
    /// Below the divider: Users and Groups (Admins only), Settings, Diagnostics.
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
        NavItem {
            admin_only: true,
            ..item(
                Route::Users {},
                messages::NAV_USERS,
                Icon::Users,
                NavSection::Manage,
                false,
            )
        },
        NavItem {
            admin_only: true,
            ..item(
                Route::Groups {},
                messages::NAV_GROUPS,
                Icon::Groups,
                NavSection::Manage,
                false,
            )
        },
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
        assert_eq!(items.len(), 9);
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
