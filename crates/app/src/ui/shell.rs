//! The layout around every screen: sidebar, top bar (with the mode and account chips),
//! the Admin banner, bottom tabs, and the + Log buttons.
//!
//! All parts are always rendered; `main.css` media queries decide which show:
//! - **Wide** (≥ 1024 px): full sidebar, + Log in the top bar.
//! - **Medium** (640–1023 px): icon-only sidebar.
//! - **Narrow** (< 640 px): ☰ menu (sidebar as an overlay, with the theme toggle),
//!   bottom tabs, and a floating + Log button.

use dioxus::prelude::*;
use domain::i18n::messages;
use domain::log_area;

use super::account::{AccountChip, AdminBanner, ModeChip};
use super::auth::{AuthState, refresh, use_auth};
use super::icons::{Icon, IconView};
use super::idle::{AdminEndedNotice, IdleWatcher};
use super::inbox::{UrgentBanner, provide_inbox_context, refresh_inbox, unread_count};
use super::nav::{NavItem, NavSection, nav_items};
use super::routes::Route;
use super::theme::ThemeToggle;
use super::tr;

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
    }));
    let unread = unread_count(inbox);
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
                    h1 { class: "page-title", {tr(route.title())} }
                    span { class: "sync-status",
                        IconView { icon: Icon::SyncOff }
                        span { class: "sync-status-label", {tr(messages::SYNC_STATUS_UNKNOWN)} }
                    }
                    div { class: "topbar-theme", ThemeToggle {} }
                    Link { class: "log-button", to: Route::LogEvent {},
                        IconView { icon: Icon::Add }
                        span { {tr(messages::ACTION_LOG)} }
                    }
                    ModeChip {}
                    AccountChip {}
                }
                AdminBanner {}
                AdminEndedNotice {}
                UrgentBanner {}
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
            Link { class: "fab", to: Route::LogEvent {}, aria_label: tr(messages::ACTION_LOG),
                IconView { icon: Icon::Add }
            }
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
