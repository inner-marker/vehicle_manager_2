//! The layout around every screen: sidebar, top bar (with the mode and account chips),
//! the Admin banner, bottom tabs, and the + Log buttons.
//!
//! All parts are always rendered; `main.css` media queries decide which show:
//! - **Wide** (≥ 1024 px): full sidebar, + Log in the top bar.
//! - **Medium** (640–1023 px): icon-only sidebar.
//! - **Narrow** (< 640 px): ☰ menu (sidebar as an overlay, with the theme toggle),
//!   bottom tabs, and a floating + Log button.

use dioxus::prelude::*;
use domain::i18n::{MessageArg, messages};
use domain::log_area;

use super::account::{AccountChip, AdminBanner, ModeChip};
use super::auth::{AuthState, refresh, use_auth};
use super::icons::{Icon, IconView};
use super::idle::{AdminEndedNotice, IdleWatcher};
use super::inbox::{UrgentBanner, provide_inbox_context, refresh_inbox, unread_count};
use super::nav::{NavItem, NavSection, nav_items};
use super::routes::Route;
use super::theme::ThemeToggle;
use super::vehicles::{
    SidebarVehicles, group_name, only_group_name, provide_vehicles_context, refresh_garage,
    vehicle_name,
};
use super::{tr, tr_with};

/// The shell layout; the current screen renders in its [`Outlet`].
// rsx! expands event handlers to fully qualified `dioxus_elements::events::on*` calls,
// and the prelude glob also imports those functions, so rustc flags the macro's own
// paths. Not fixable in our code (explicit imports would depend on Dioxus internals).
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn Shell() -> Element {
    let route = use_route::<Route>();
    let auth = use_auth();
    let inbox = provide_inbox_context();
    let vehicles = provide_vehicles_context();
    let chooser = super::screens::provide_log_chooser();
    // The household's word for squawks (#064) is in the navigation's labels.
    let _word = super::SQUAWK_WORD();
    // Ask the server again on every navigation, so an Admin mode it ended after 15 idle
    // minutes (or a session it ended) stops showing.
    use_effect(use_reactive!(|route| {
        let _ = route;
        spawn(refresh(auth));
    }));
    // Messages: again on every navigation and whenever the mode changes (Admin messages show
    // only in Admin mode).
    let mode = use_memo(move || match auth() {
        AuthState::SignedIn(session) => Some(session.mode),
        AuthState::Loading
        | AuthState::NeedsSetup
        | AuthState::SignedOut
        | AuthState::Unreachable(_) => None,
    });
    use_effect(use_reactive!(|route| {
        let _ = (route, mode());
        spawn(refresh_inbox(inbox));
        // The groups and vehicles too: the sidebar lists the groups (#056).
        spawn(async move {
            let _ = refresh_garage(vehicles).await;
        });
    }));
    let unread = unread_count(inbox);
    // A group's page is titled with the group's name (#056).
    let title = match &route {
        Route::GroupVehicles { group_id } => {
            group_name(vehicles, group_id).unwrap_or_else(|| tr(route.title()))
        }
        // With one group, All vehicles is that group, so it's titled with its name.
        Route::AllVehicles {} => only_group_name(vehicles).unwrap_or_else(|| tr(route.title())),
        // A vehicle's page is titled with its name.
        Route::VehicleDetail { asset_id } | Route::VehicleTab { asset_id, .. } => {
            vehicle_name(vehicles, asset_id).unwrap_or_else(|| tr(route.title()))
        }
        // A log form: "Fuel-up · Truck".
        Route::LogForm { asset_id, kind } => {
            let kind = if kind == "fuel" {
                messages::EVENT_KIND_FUEL
            } else {
                messages::EVENT_KIND_READING
            };
            vehicle_name(vehicles, asset_id)
                .map_or_else(|| tr(kind), |name| format!("{} · {name}", tr(kind)))
        }
        Route::Due {}
        | Route::Messages {}
        | Route::Timeline {}
        | Route::Reports {}
        | Route::LogEvent {}
        | Route::EventDetail { .. }
        | Route::EditEvent { .. }
        | Route::CorrectEvent { .. }
        | Route::Users {}
        | Route::Groups {}
        | Route::Places {}
        | Route::Settings {}
        | Route::Diagnostics {}
        | Route::NotFound { .. } => tr(route.title()),
    };
    // + Log starts on the vehicle whose page (or form) this is.
    let here = match &route {
        Route::VehicleDetail { asset_id }
        | Route::VehicleTab { asset_id, .. }
        | Route::LogForm { asset_id, .. } => asset_id.parse().ok(),
        Route::AllVehicles {}
        | Route::Due {}
        | Route::Messages {}
        | Route::Timeline {}
        | Route::Reports {}
        | Route::GroupVehicles { .. }
        | Route::LogEvent {}
        | Route::EventDetail { .. }
        | Route::EditEvent { .. }
        | Route::CorrectEvent { .. }
        | Route::Users {}
        | Route::Groups {}
        | Route::Places {}
        | Route::Settings {}
        | Route::Diagnostics {}
        | Route::NotFound { .. } => None,
    };
    let mut menu_open = use_signal(|| false);
    let close_menu = move |_| menu_open.set(false);

    // Users appears only for the household's Admin (who can enter Admin mode).
    let is_household_admin = matches!(
        auth(),
        super::auth::AuthState::SignedIn(session)
            if session.available_modes.contains(&domain::access::ActiveMode::Admin)
    );
    let items: Vec<NavItem> = nav_items()
        .into_iter()
        .filter(|item| is_household_admin || !item.admin_only)
        .collect();
    let main_items: Vec<NavItem> = items
        .iter()
        .filter(|i| i.section == NavSection::Main)
        .cloned()
        .collect();
    let manage_items: Vec<NavItem> = items
        .iter()
        .filter(|i| i.section == NavSection::Manage)
        .cloned()
        .collect();
    let tab_items: Vec<NavItem> = items.iter().filter(|i| i.bottom_tab).cloned().collect();

    rsx! {
        IdleWatcher {}
        div { class: "shell",
            aside { class: if menu_open() { "sidebar open" } else { "sidebar" },
                button {
                    class: "menu-close",
                    "aria-label": tr(messages::MENU_CLOSE),
                    onclick: close_menu,
                    IconView { icon: Icon::Close }
                }
                nav { class: "sidebar-nav",
                    SidebarVehicles { on_navigate: close_menu }
                    hr {}
                    for item in main_items {
                        SidebarLink {
                            key: "{item.route}",
                            badge: if item.route == (Route::Messages {}) { unread } else { 0 },
                            item,
                            on_navigate: close_menu,
                        }
                    }
                    hr {}
                    for item in manage_items {
                        SidebarLink { key: "{item.route}", badge: 0, item, on_navigate: close_menu }
                    }
                }
                div { class: "sidebar-theme", ThemeToggle {} }
            }
            if menu_open() {
                div { class: "scrim", onclick: close_menu }
            }

            div { class: "main",
                // The top bar and its banners stay put while the page scrolls.
                div { class: "topbar-stack",
                header { class: "topbar",
                    button {
                        class: "menu-open",
                        "aria-label": tr(messages::MENU_OPEN),
                        onclick: move |_| {
                            menu_open.set(true);
                            tracing::debug!(target: log_area::UI, "navigation menu opened");
                        },
                        IconView { icon: Icon::Menu }
                    }
                    h1 { class: "page-title", {title} }
                    span { class: "sync-status",
                        IconView { icon: Icon::SyncOff }
                        span { class: "sync-status-label", {tr(messages::SYNC_STATUS_UNKNOWN)} }
                    }
                    div { class: "topbar-theme", ThemeToggle {} }
                    button { class: "log-button", r#type: "button", onclick: move |_| chooser.open(here),
                        IconView { icon: Icon::Add }
                        span { {tr(messages::ACTION_LOG)} }
                    }
                    super::auth::InstanceTag {}
                    if let AuthState::SignedIn(session) = auth() {
                        if let Some(minutes) = session.timers.test_minutes() {
                            span { class: "test-timers-tag", title: tr_with(messages::TEST_TIMERS_TAG, &[("minutes", MessageArg::from(i64::from(minutes)))]),
                                {tr_with(messages::TEST_TIMERS_TAG, &[("minutes", MessageArg::from(i64::from(minutes)))])}
                            }
                        }
                    }
                    ModeChip {}
                    AccountChip {}
                }
                AdminBanner {}
                AdminEndedNotice {}
                UrgentBanner {}
                }
                main { class: "content", Outlet::<Route> {} }
            }

            nav { class: "bottom-tabs",
                for item in tab_items {
                    Link {
                        key: "{item.route}",
                        class: "bottom-tab",
                        active_class: "active",
                        to: item.route.clone(),
                        IconView { icon: item.icon }
                        span { {tr(item.label)} }
                        if item.route == (Route::Messages {}) && unread > 0 {
                            span { class: "nav-badge", "{unread}" }
                        }
                    }
                }
            }
            button { class: "fab", r#type: "button", "aria-label": tr(messages::ACTION_LOG), onclick: move |_| chooser.open(here),
                IconView { icon: Icon::Add }
            }
            super::screens::LogChooserDialog {}
        }
    }
}

/// One sidebar entry: icon plus label (the label hides on medium screens), and a count badge
/// when `badge` isn't 0 (unread messages).
#[component]
fn SidebarLink(item: NavItem, badge: usize, on_navigate: EventHandler<MouseEvent>) -> Element {
    let label = tr(item.label);
    rsx! {
        Link {
            class: "sidebar-link",
            active_class: "active",
            to: item.route.clone(),
            title: label.clone(),
            onclick: move |event| on_navigate.call(event),
            IconView { icon: item.icon }
            span { class: "sidebar-label", {label} }
            if badge > 0 {
                span { class: "nav-badge", "{badge}" }
            }
        }
    }
}
