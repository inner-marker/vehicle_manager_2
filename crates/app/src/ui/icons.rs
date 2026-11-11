//! Icons: Lucide SVGs (ISC license, see `assets/icons/LICENSE-lucide`), embedded at
//! compile time and inlined so they take the surrounding text color (`currentColor`).

use dioxus::prelude::*;

/// One of the app's icons. Named by meaning, not by the Lucide file it uses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Icon {
    Garage,
    Due,
    Messages,
    Timeline,
    Reports,
    Vehicles,
    Settings,
    Diagnostics,
    Users,
    Add,
    Menu,
    Close,
    Light,
    Dark,
    System,
    Refresh,
    SyncOff,
    /// A browser, in Settings → Devices (phase-3.md 2e).
    Browser,
    /// All vehicles (#056).
    AllVehicles,
    /// Group icons (#052), besides the Garage's.
    Hangar,
    Boathouse,
    Storage,
    Warehouse,
    Shop,
    Farm,
    House,
    /// ▲▼ while arranging.
    Up,
    Down,
    /// The Groups screen (#058).
    Groups,
    /// The Places screen (#060).
    Places,
}

impl Icon {
    /// The icon's SVG markup.
    fn svg(self) -> &'static str {
        match self {
            Self::Garage => include_str!("../../assets/icons/warehouse.svg"),
            Self::Due => include_str!("../../assets/icons/calendar-clock.svg"),
            Self::Messages => include_str!("../../assets/icons/inbox.svg"),
            Self::Timeline => include_str!("../../assets/icons/list.svg"),
            Self::Reports => include_str!("../../assets/icons/file-text.svg"),
            Self::Vehicles => include_str!("../../assets/icons/car.svg"),
            Self::Settings => include_str!("../../assets/icons/settings.svg"),
            Self::Diagnostics => include_str!("../../assets/icons/activity.svg"),
            Self::Users => include_str!("../../assets/icons/users.svg"),
            Self::Add => include_str!("../../assets/icons/plus.svg"),
            Self::Menu => include_str!("../../assets/icons/menu.svg"),
            Self::Close => include_str!("../../assets/icons/x.svg"),
            Self::Light => include_str!("../../assets/icons/sun.svg"),
            Self::Dark => include_str!("../../assets/icons/moon.svg"),
            Self::System => include_str!("../../assets/icons/monitor.svg"),
            Self::Refresh => include_str!("../../assets/icons/refresh-cw.svg"),
            Self::SyncOff => include_str!("../../assets/icons/cloud-off.svg"),
            Self::Browser => include_str!("../../assets/icons/globe.svg"),
            Self::AllVehicles => include_str!("../../assets/icons/layout-grid.svg"),
            Self::Hangar => include_str!("../../assets/icons/plane.svg"),
            Self::Boathouse => include_str!("../../assets/icons/sailboat.svg"),
            Self::Storage => include_str!("../../assets/icons/package.svg"),
            Self::Warehouse => include_str!("../../assets/icons/container.svg"),
            Self::Shop => include_str!("../../assets/icons/wrench.svg"),
            Self::Farm => include_str!("../../assets/icons/tractor.svg"),
            Self::House => include_str!("../../assets/icons/house.svg"),
            Self::Up => include_str!("../../assets/icons/chevron-up.svg"),
            Self::Down => include_str!("../../assets/icons/chevron-down.svg"),
            Self::Groups => include_str!("../../assets/icons/layers.svg"),
            Self::Places => include_str!("../../assets/icons/map-pin.svg"),
        }
    }
}

/// Renders an icon. Decorative only: the button or link around it carries the label.
#[component]
pub(crate) fn IconView(icon: Icon) -> Element {
    rsx! {
        // Inner HTML is safe here: it's our own SVG files, embedded at compile time
        // and checked for scripts and links when they were added.
        span { class: "icon", "aria-hidden": "true", dangerous_inner_html: icon.svg() }
    }
}
