//! Pinch to zoom a photo in the viewer (phase-6.md 2d; sketch part 6 of
//! `docs/ui-sketches/p6m2-faster-logging-options.svg`).
//!
//! The viewer handles the gestures itself, so only the photo zooms, not the page (iOS Safari
//! ignores the page's own "don't zoom" setting; the photo's box has `touch-action: none`):
//! - **Pinch** zooms (up to 5×) around the fingers; with one finger, a zoomed photo **pans**.
//! - **Double-tap** (or double-click) zooms to 2× there, and back.
//! - On a computer: the **trackpad's pinch** (Safari's gesture events, which the macOS desktop
//!   app's web view sends, or Ctrl + scroll elsewhere) and **Ctrl + scroll**.
//! - **Swiping** left or right changes photos, only when it isn't zoomed (it presses the
//!   viewer's own ‹ › buttons).
//!
//! The zoom stays within the photo's own frame, and a zoomed photo can't be panned past its
//! edges. A new photo is a new frame, so its zoom starts at 1×. The gestures run in the page:
//! on the desktop, every pointer event crossing into Rust would make pinching lag.

use dioxus::prelude::*;

/// The script: given the photo's frame's ID, it handles that frame's gestures until it's gone.
const ZOOM_SCRIPT: &str = r"
const id = await dioxus.recv();
const frame = document.getElementById(id);
const img = frame && frame.querySelector('img');
if (!frame || !img || frame.dataset.zoomReady) { return; }
frame.dataset.zoomReady = '1';
const MAX = 5, DOUBLE = 2, TAP_MS = 300, TAP_MOVE = 10, SWIPE = 60;
let scale = 1, x = 0, y = 0;
const pointers = new Map();
let pinch = null, pan = null, lastTap = null, swipe = null, gesture = null;

function bounds() {
  return { x: img.offsetWidth * (scale - 1) / 2, y: img.offsetHeight * (scale - 1) / 2 };
}
function apply() {
  scale = Math.min(MAX, Math.max(1, scale));
  const limit = bounds();
  x = Math.min(limit.x, Math.max(-limit.x, x));
  y = Math.min(limit.y, Math.max(-limit.y, y));
  if (scale <= 1.001) { scale = 1; x = 0; y = 0; }
  img.style.transform = `translate(${x}px, ${y}px) scale(${scale})`;
  frame.classList.toggle('zoomed', scale > 1);
}
// Zooms to `next` keeping the point at (cx, cy) (page coordinates) where it is.
function zoomAt(next, cx, cy) {
  const box = frame.getBoundingClientRect();
  const px = cx - (box.left + box.width / 2), py = cy - (box.top + box.height / 2);
  const ratio = Math.min(MAX, Math.max(1, next)) / scale;
  x = px - (px - x) * ratio;
  y = py - (py - y) * ratio;
  scale *= ratio;
  apply();
}
function middle() {
  const [a, b] = [...pointers.values()];
  return { x: (a.x + b.x) / 2, y: (a.y + b.y) / 2, d: Math.hypot(a.x - b.x, a.y - b.y) || 1 };
}

frame.addEventListener('pointerdown', (e) => {
  frame.setPointerCapture(e.pointerId);
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 2) {
    const m = middle();
    pinch = { d: m.d, scale, x, y, mx: m.x, my: m.y };
    pan = null; swipe = null;
  } else if (pointers.size === 1) {
    pan = { x: e.clientX, y: e.clientY };
    swipe = { x: e.clientX, y: e.clientY, at: Date.now() };
  }
});
frame.addEventListener('pointermove', (e) => {
  if (!pointers.has(e.pointerId)) { return; }
  pointers.set(e.pointerId, { x: e.clientX, y: e.clientY });
  if (pointers.size === 2 && pinch) {
    // From the pinch's start: the point first under the fingers stays under them.
    const m = middle();
    const box = frame.getBoundingClientRect();
    const cx = box.left + box.width / 2, cy = box.top + box.height / 2;
    const ux = (pinch.mx - cx - pinch.x) / pinch.scale, uy = (pinch.my - cy - pinch.y) / pinch.scale;
    scale = Math.min(MAX, Math.max(1, pinch.scale * m.d / pinch.d));
    x = m.x - cx - ux * scale;
    y = m.y - cy - uy * scale;
    apply();
  } else if (pointers.size === 1 && pan && scale > 1) {
    x += e.clientX - pan.x; y += e.clientY - pan.y;
    pan = { x: e.clientX, y: e.clientY };
    apply();
  }
});
function lift(e) {
  if (!pointers.has(e.pointerId)) { return; }
  pointers.delete(e.pointerId);
  if (pointers.size < 2) { pinch = null; }
  if (pointers.size === 1) {
    const [rest] = [...pointers.values()];
    pan = { x: rest.x, y: rest.y };
    return;
  }
  if (e.type !== 'pointerup' || !swipe) { return; }
  const dx = e.clientX - swipe.x, dy = e.clientY - swipe.y;
  const moved = Math.hypot(dx, dy);
  if (moved < TAP_MOVE) {
    const now = Date.now();
    if (lastTap && now - lastTap.at < TAP_MS && Math.hypot(e.clientX - lastTap.x, e.clientY - lastTap.y) < 3 * TAP_MOVE) {
      if (scale > 1) { scale = 1; x = 0; y = 0; apply(); } else { zoomAt(DOUBLE, e.clientX, e.clientY); }
      lastTap = null;
    } else {
      lastTap = { at: now, x: e.clientX, y: e.clientY };
    }
  } else if (scale === 1 && Math.abs(dx) > SWIPE && Math.abs(dx) > 2 * Math.abs(dy)) {
    const step = frame.parentElement.querySelector(dx < 0 ? '.photo-viewer-next' : '.photo-viewer-previous');
    if (step) { step.click(); }
  }
  swipe = null; pan = null;
}
frame.addEventListener('pointerup', lift);
frame.addEventListener('pointercancel', lift);
frame.addEventListener('wheel', (e) => {
  if (!e.ctrlKey) { return; }
  e.preventDefault();
  zoomAt(scale * Math.exp(-e.deltaY * 0.002), e.clientX, e.clientY);
}, { passive: false });
// Safari's own pinch (a Mac's trackpad; on iOS the pointers above do it).
frame.addEventListener('gesturestart', (e) => {
  e.preventDefault();
  gesture = pointers.size < 2 ? { scale } : null;
});
frame.addEventListener('gesturechange', (e) => {
  e.preventDefault();
  if (gesture) { zoomAt(gesture.scale * e.scale, e.clientX, e.clientY); }
});
frame.addEventListener('gestureend', (e) => { e.preventDefault(); gesture = null; });
frame.addEventListener('dragstart', (e) => e.preventDefault());
";

/// Starts the gestures on the photo frame with ID `id` (once; again is harmless).
pub(crate) fn start(id: &str) {
    let eval = document::eval(ZOOM_SCRIPT);
    if let Err(error) = eval.send(id) {
        tracing::debug!(target: domain::log_area::UI, %error, "the photo's zoom couldn't start");
    }
}

/// The ID of the frame around the photo with attachment ID `id`.
pub(crate) fn frame_id(id: impl std::fmt::Display) -> String {
    format!("photo-zoom-{id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_photo_has_its_own_frame() {
        assert_eq!(frame_id("abc"), "photo-zoom-abc");
        assert_ne!(frame_id(1), frame_id(2));
    }
}
