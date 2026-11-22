//! Where the desktop window was, so it reopens there (phase-3.md 1b, GitHub #19).
//!
//! Saved in `settings.json` (`crate::settings`) in **logical** pixels, so a window keeps its
//! size on a screen with a different scale. Pure: the desktop app reads the window and the
//! monitors, and asks [`is_on_screen`] whether a saved place can still be seen (a monitor
//! may have been unplugged since).

use serde::{Deserialize, Serialize};

/// The window's last place: its normal position and size (kept while maximized or full
/// screen, so leaving those returns to it), and whether it was maximized or full screen.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct WindowPlacement {
    /// The window's left edge, in logical pixels: where the window is placed when it opens
    /// (on macOS, its content's; on Windows and Linux, its outer frame's).
    pub x: f64,
    /// The window's top edge, in logical pixels: where the window is placed when it opens
    /// (on macOS, its content's, under the title bar; on Windows and Linux, its outer frame's).
    pub y: f64,
    /// The width of the window's content, in logical pixels.
    pub width: f64,
    /// The height of the window's content, in logical pixels.
    pub height: f64,
    /// Whether it was maximized.
    #[serde(default)]
    pub maximized: bool,
    /// Whether it was full screen.
    #[serde(default)]
    pub fullscreen: bool,
}

/// A monitor's area, in logical pixels.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScreenArea {
    /// Its left edge.
    pub x: f64,
    /// Its top edge.
    pub y: f64,
    /// Its width.
    pub width: f64,
    /// Its height.
    pub height: f64,
}

/// How much of the window's top strip (where its title bar is) must be on a monitor for the
/// window to count as visible: enough to see it and drag it.
const VISIBLE_WIDTH: f64 = 120.0;

/// The height of that top strip.
const VISIBLE_HEIGHT: f64 = 40.0;

impl WindowPlacement {
    /// This placement with its size at least `min` (width, height): a hand-edited or very
    /// old file can't open a window smaller than the app lays out for.
    #[must_use]
    pub const fn at_least(self, min: (f64, f64)) -> Self {
        Self {
            width: self.width.max(min.0),
            height: self.height.max(min.1),
            ..self
        }
    }

    /// Whether every number is a real, finite one with a positive size.
    #[must_use]
    pub fn is_valid(&self) -> bool {
        [self.x, self.y, self.width, self.height]
            .iter()
            .all(|value| value.is_finite())
            && self.width > 0.0
            && self.height > 0.0
    }
}

/// The size of a window's content, from its outer frame: the frame's size, less how far the
/// content's top-left corner sits inside the frame's (the title bar). For macOS, where tao's
/// `inner_size` keeps reporting the size the window opened at (tao #889; phase-6.md 1e) and a
/// macOS window has no border on its other sides. In logical pixels, as (width, height).
#[must_use]
pub fn content_size(
    frame_size: (f64, f64),
    frame_top_left: (f64, f64),
    content_top_left: (f64, f64),
) -> (f64, f64) {
    let inset_x = (content_top_left.0 - frame_top_left.0).max(0.0);
    let inset_y = (content_top_left.1 - frame_top_left.1).max(0.0);
    (
        (frame_size.0 - inset_x).max(0.0),
        (frame_size.1 - inset_y).max(0.0),
    )
}

/// Whether the window at `placement` can be seen and dragged: enough of its top strip lies
/// on one of `screens`.
#[must_use]
pub fn is_on_screen(placement: &WindowPlacement, screens: &[ScreenArea]) -> bool {
    if !placement.is_valid() {
        return false;
    }
    screens.iter().any(|screen| {
        let left = placement.x.max(screen.x);
        let right = (placement.x + placement.width).min(screen.x + screen.width);
        let top = placement.y.max(screen.y);
        let bottom = (placement.y + VISIBLE_HEIGHT).min(screen.y + screen.height);
        right - left >= VISIBLE_WIDTH && bottom - top >= VISIBLE_HEIGHT
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const LAPTOP: ScreenArea = ScreenArea {
        x: 0.0,
        y: 0.0,
        width: 1512.0,
        height: 982.0,
    };

    /// A second monitor to the laptop's right.
    const EXTERNAL: ScreenArea = ScreenArea {
        x: 1512.0,
        y: 0.0,
        width: 2560.0,
        height: 1440.0,
    };

    fn placed(x: f64, y: f64) -> WindowPlacement {
        WindowPlacement {
            x,
            y,
            width: 1200.0,
            height: 820.0,
            maximized: false,
            fullscreen: false,
        }
    }

    #[test]
    fn a_window_on_a_monitor_is_on_screen() {
        assert!(is_on_screen(&placed(100.0, 80.0), &[LAPTOP]));
        assert!(is_on_screen(&placed(2000.0, 200.0), &[LAPTOP, EXTERNAL]));
    }

    #[test]
    fn a_window_on_an_unplugged_monitor_is_off_screen() {
        assert!(!is_on_screen(&placed(2000.0, 200.0), &[LAPTOP]));
        assert!(!is_on_screen(&placed(100.0, 80.0), &[]));
    }

    #[test]
    fn a_title_bar_barely_showing_or_above_the_screen_is_off_screen() {
        // Only 50 px of its width shows at the right edge.
        assert!(!is_on_screen(&placed(1462.0, 80.0), &[LAPTOP]));
        // Its top strip is above the screen.
        assert!(!is_on_screen(&placed(100.0, -500.0), &[LAPTOP]));
        // Partly off to the left, but plenty of the title bar shows.
        assert!(is_on_screen(&placed(-600.0, 80.0), &[LAPTOP]));
    }

    #[test]
    fn nonsense_numbers_are_off_screen_and_small_sizes_grow() {
        let broken = WindowPlacement {
            width: f64::NAN,
            ..placed(0.0, 0.0)
        };
        assert!(!is_on_screen(&broken, &[LAPTOP]));
        let tiny = WindowPlacement {
            width: 50.0,
            height: 900.0,
            ..placed(0.0, 0.0)
        };
        let grown = tiny.at_least((400.0, 600.0));
        assert!((grown.width - 400.0).abs() < f64::EPSILON);
        assert!((grown.height - 900.0).abs() < f64::EPSILON);
    }

    #[test]
    fn content_size_is_the_frame_less_its_title_bar() {
        // A 900 × 650 window as macOS reports it: a 682-high frame at y 168, content at y 200.
        let size = content_size((900.0, 682.0), (300.0, 168.0), (300.0, 200.0));
        assert_eq!(size, (900.0, 650.0));
        // Without a title bar, the content is the whole frame.
        assert_eq!(
            content_size((800.0, 600.0), (10.0, 20.0), (10.0, 20.0)),
            (800.0, 600.0)
        );
        // Odd readings never make the content bigger than the frame, or negative.
        assert_eq!(
            content_size((800.0, 600.0), (10.0, 40.0), (10.0, 20.0)),
            (800.0, 600.0)
        );
        assert_eq!(
            content_size((800.0, 20.0), (0.0, 0.0), (0.0, 32.0)),
            (800.0, 0.0)
        );
    }

    #[test]
    fn a_placement_round_trips_and_older_files_default_the_flags() {
        let json = r#"{"x":10.0,"y":20.0,"width":1000.0,"height":700.0}"#;
        let read: WindowPlacement = serde_json::from_str(json).unwrap();
        assert!(!read.maximized && !read.fullscreen);
        let back: WindowPlacement =
            serde_json::from_str(&serde_json::to_string(&read).unwrap()).unwrap();
        assert_eq!(back, read);
    }
}
