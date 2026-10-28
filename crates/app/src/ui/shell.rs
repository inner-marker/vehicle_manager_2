//! The layout around every screen: sidebar, top bar, bottom tabs, and the + Log buttons.
//!
//! All parts are always rendered; `main.css` media queries decide which show:
//! - **Wide** (≥ 1024 px): full sidebar, + Log in the top bar.
//! - **Medium** (640–1023 px): icon-only sidebar.
//! - **Narrow** (< 640 px): ☰ menu (sidebar as an overlay, with the theme toggle),
//!   bottom tabs, and a floating + Log button.

use dioxus::prelude::*;
use domain::i18n::messages;
use domain::log_area;

use super::icons::{Icon, IconView};
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
    let mut menu_open = use_signal(|| false);
    let close_menu = move |_| menu_open.set(false);

    let items = nav_items();
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
                        SidebarLink { key: "{item.route}", item, on_navigate: close_menu }
                    }
                    hr {}
                    for item in manage_items {
                        SidebarLink { key: "{item.route}", item, on_navigate: close_menu }
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
                    }
                }
            }
            Link { class: "fab", to: Route::LogEvent {}, aria_label: tr(messages::ACTION_LOG),
                IconView { icon: Icon::Add }
            }
        }
    }
}

/// One sidebar entry: icon plus label (the label hides on medium screens).
#[component]
fn SidebarLink(item: NavItem, on_navigate: EventHandler<MouseEvent>) -> Element {
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
        }
    }
}
