//! The desktop window remembers where it was (phase-3.md 1b, GitHub #19).
//!
//! The window opens at its saved place (`desktop_config` in `main.rs`). [`WindowKeeper`]
//! then moves it back on screen if that place can't be seen (a monitor was unplugged), and
//! keeps the place up to date in `settings.json`: on every move or resize it notes the new
//! place, written at most once a second, and again when the window loses focus or closes and
//! when the app quits, so the last move is never lost. "Keep window on top" is
//! [`KeepOnTopRow`] in Settings → This device.

use std::cell::Cell;
use std::rc::Rc;
use std::time::{Duration, Instant};

use dioxus::desktop::tao::event::Event;
use dioxus::desktop::tao::monitor::MonitorHandle;
use dioxus::desktop::{LogicalPosition, WindowEvent, use_wry_event_handler, window};
use dioxus::prelude::*;
use domain::i18n::messages;
use domain::log_area;

use crate::settings;
use crate::ui::tr;
use crate::window_placement::{ScreenArea, WindowPlacement, is_on_screen};

/// The least time between two writes of the window's place while it's being moved.
const WRITE_INTERVAL: Duration = Duration::from_secs(1);

/// The window's place now. While maximized or full screen, the normal place is kept from
/// `previous` (so leaving those returns to it) and only the flags change.
fn current_placement(previous: Option<WindowPlacement>) -> Option<WindowPlacement> {
    let desktop = window();
    let window = &desktop.window;
    let maximized = window.is_maximized();
    let fullscreen = window.fullscreen().is_some();
    if (maximized || fullscreen)
        && let Some(previous) = previous
    {
        return Some(WindowPlacement {
            maximized,
            fullscreen,
            ..previous
        });
    }
    let scale = window.scale_factor();
    let position = window.outer_position().ok()?.to_logical::<f64>(scale);
    let size = window.inner_size().to_logical::<f64>(scale);
    Some(WindowPlacement {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
        maximized,
        fullscreen,
    })
}

/// A monitor's area in logical pixels.
fn screen_area(monitor: &MonitorHandle) -> ScreenArea {
    let scale = monitor.scale_factor();
    let position = monitor.position().to_logical::<f64>(scale);
    let size = monitor.size().to_logical::<f64>(scale);
    ScreenArea {
        x: position.x,
        y: position.y,
        width: size.width,
        height: size.height,
    }
}

/// Moves the window to the middle of the main monitor if it can't be seen where it is.
fn bring_on_screen() {
    let desktop = window();
    let window = &desktop.window;
    let Some(placement) = current_placement(None) else {
        return;
    };
    let screens: Vec<ScreenArea> = window
        .available_monitors()
        .map(|m| screen_area(&m))
        .collect();
    if is_on_screen(&placement, &screens) {
        return;
    }
    let Some(main) = window.primary_monitor().map(|m| screen_area(&m)) else {
        return;
    };
    let x = main.x + ((main.width - placement.width) / 2.0).max(0.0);
    let y = main.y + ((main.height - placement.height) / 2.0).max(0.0);
    tracing::info!(target: log_area::APP, "the window's saved place can't be seen; moving it onto the main screen");
    window.set_outer_position(LogicalPosition::new(x, y));
}

/// Writes `placement` to `settings.json` (a failure is only logged: the window still works).
fn write_placement(placement: WindowPlacement) {
    let Some(data_folder) = crate::data_folder::platform_data_folder() else {
        return;
    };
    if let Err(error) = settings::update(&data_folder, |saved| saved.window = Some(placement)) {
        tracing::warn!(target: log_area::APP, %error, "couldn't save the window's place");
    }
}

/// What the keeper remembers between events.
#[derive(Default)]
struct Keeper {
    /// The newest place, if it changed since the last write.
    unsaved: Cell<Option<WindowPlacement>>,
    /// The newest place known (saved or not), for keeping the normal place while maximized.
    latest: Cell<Option<WindowPlacement>>,
    /// When the place was last written.
    written: Cell<Option<Instant>>,
}

impl Keeper {
    /// Notes the window's new place, and writes it if the last write was long enough ago.
    fn moved(&self) {
        let Some(placement) = current_placement(self.latest.get()) else {
            return;
        };
        self.latest.set(Some(placement));
        self.unsaved.set(Some(placement));
        if self
            .written
            .get()
            .is_none_or(|written| written.elapsed() >= WRITE_INTERVAL)
        {
            self.flush();
        }
    }

    /// Writes the newest place if it hasn't been.
    fn flush(&self) {
        if let Some(placement) = self.unsaved.take() {
            write_placement(placement);
            self.written.set(Some(Instant::now()));
        }
    }
}

/// Keeps the desktop window on screen and its place saved. Renders nothing.
#[component]
pub(crate) fn WindowKeeper() -> Element {
    let keeper = use_hook(|| {
        bring_on_screen();
        let saved = crate::data_folder::platform_data_folder()
            .and_then(|folder| settings::load(&folder).window);
        let keeper = Keeper::default();
        keeper.latest.set(saved);
        Rc::new(keeper)
    });
    use_wry_event_handler(move |event, _| {
        let moved = matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::Moved(_) | WindowEvent::Resized(_),
                ..
            }
        );
        let leaving = matches!(
            event,
            Event::WindowEvent {
                event: WindowEvent::Focused(false) | WindowEvent::CloseRequested,
                ..
            } | Event::LoopDestroyed
        );
        if moved {
            keeper.moved();
        } else if leaving {
            keeper.flush();
        }
    });
    rsx! {}
}

/// Settings → This device: "Keep window on top", saved for this computer.
#[expect(
    unused_qualifications,
    reason = "false positive in rsx! event-handler expansion"
)]
#[component]
pub(crate) fn KeepOnTopRow() -> Element {
    let mut on_top = use_signal(|| {
        crate::data_folder::platform_data_folder()
            .is_some_and(|folder| settings::load(&folder).keep_on_top)
    });
    rsx! {
        div { class: "setting-row",
            span { class: "setting-label", {tr(messages::SETTINGS_KEEP_ON_TOP)} }
            label { class: "switch",
                input {
                    r#type: "checkbox",
                    checked: on_top(),
                    onchange: move |_| {
                        let wanted = !on_top();
                        window().window.set_always_on_top(wanted);
                        on_top.set(wanted);
                        let saved = crate::data_folder::platform_data_folder()
                            .map(|folder| settings::update(&folder, |saved| saved.keep_on_top = wanted));
                        if let Some(Err(error)) = saved {
                            tracing::warn!(target: log_area::APP, %error, "couldn't save 'keep window on top'");
                        }
                    },
                }
                span { {tr(messages::SETTINGS_KEEP_ON_TOP_NOTE)} }
            }
        }
    }
}
