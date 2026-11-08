//! PDFs in the app (milestone 8, step 8f; the human's pick: option A of
//! `docs/ui-sketches/p2m8-pdf-viewer-options.svg`; decision #037).
//!
//! Pages are drawn by **pdf.js's core library**, bundled with the app (`assets/pdfjs/`, version
//! 6.4.299; see its README) and loaded only when a PDF is opened or added, the same on the web
//! and in the desktop app. The app's own viewer is the frame around the pages ([`PdfPages`]):
//! one page under another, drawn as they come into view, with text that can be selected;
//! zoom; and the page in view. Search and a page-thumbnail sidebar are post-MVP.
//!
//! Adding a PDF draws its first page as the tile's thumbnail ([`first_page_thumbnail`]); the
//! server checks the file and counts its pages itself (8d.1).

use dioxus::prelude::*;
use serde::{Deserialize, Serialize};

/// The bundled pdf.js folder (served as it is, at a fixed path).
const PDFJS: Asset = asset!("/assets/pdfjs", AssetOptions::folder());

/// Loads pdf.js and opens the document `input.data` (Base64), with the bundled fonts,
/// character maps, and image decoders. `pdfjs`, `task` (which `destroy`s it), and `doc` are left
/// for the script that follows.
const OPEN: &str = r#"
const base = new URL(input.base.replace(/\/?$/, "/"), location.href).href;
const pdfjs = await import(base + "pdf.min.mjs");
pdfjs.GlobalWorkerOptions.workerSrc = base + "pdf.worker.min.mjs";
const bytes = Uint8Array.from(atob(input.data), (c) => c.charCodeAt(0));
const task = pdfjs.getDocument({
  data: bytes,
  cMapUrl: base + "cmaps/",
  cMapPacked: true,
  standardFontDataUrl: base + "standard_fonts/",
  wasmUrl: base + "wasm/",
  iccUrl: base + "iccs/",
  isEvalSupported: false,
  enableXfa: false,
});
const doc = await task.promise;
"#;

/// The viewer: pages in `input.container`, drawn as they come into view (and again at a new
/// zoom); tells the app how many pages there are and which one is in view. Zoom and closing
/// come through `window.__vmPdf[container]`.
const VIEWER: &str = r#"
const holder = document.getElementById(input.container);
let zoom = 1;
const first = await doc.getPage(1);
const unit = first.getViewport({ scale: 1 });
const scaleNow = () => Math.max(0.2, ((holder.clientWidth - 48) / unit.width) * zoom);
const pages = [];
for (let n = 1; n <= doc.numPages; n++) {
  const div = document.createElement("div");
  div.className = "pdf-page";
  div.dataset.page = String(n);
  holder.appendChild(div);
  pages.push(div);
}
const drawn = new Map();
const placeholders = () => {
  const s = scaleNow();
  for (const div of pages) {
    if (!drawn.has(+div.dataset.page)) {
      div.style.width = unit.width * s + "px";
      div.style.height = unit.height * s + "px";
    }
  }
};
const draw = async (div) => {
  const n = +div.dataset.page;
  const scale = scaleNow();
  if (drawn.get(n) === scale) return;
  drawn.set(n, scale);
  const page = await doc.getPage(n);
  const viewport = page.getViewport({ scale });
  const ratio = window.devicePixelRatio || 1;
  const canvas = document.createElement("canvas");
  canvas.width = Math.floor(viewport.width * ratio);
  canvas.height = Math.floor(viewport.height * ratio);
  canvas.style.width = viewport.width + "px";
  canvas.style.height = viewport.height + "px";
  const text = document.createElement("div");
  text.className = "textLayer";
  div.replaceChildren(canvas, text);
  div.style.width = viewport.width + "px";
  div.style.height = viewport.height + "px";
  div.style.setProperty("--scale-factor", String(scale));
  div.style.setProperty("--total-scale-factor", String(scale));
  await page.render({
    canvas,
    canvasContext: canvas.getContext("2d"),
    viewport,
    transform: ratio === 1 ? undefined : [ratio, 0, 0, ratio, 0, 0],
  }).promise;
  try {
    await new pdfjs.TextLayer({ textContentSource: page.streamTextContent(), container: text, viewport }).render();
  } catch (error) {}
};
placeholders();
const watcher = new IntersectionObserver(
  (entries) => { for (const entry of entries) if (entry.isIntersecting) draw(entry.target); },
  { root: holder, rootMargin: "600px 0px" });
pages.forEach((div) => watcher.observe(div));
let shown = 0;
const inView = () => {
  const middle = holder.scrollTop + holder.clientHeight / 3;
  let n = 1;
  for (const div of pages) { if (div.offsetTop <= middle) n = +div.dataset.page; }
  if (n !== shown) { shown = n; dioxus.send({ page: n, pages: doc.numPages }); }
};
holder.addEventListener("scroll", inView, { passive: true });
inView();
// A new width (a window resized, a phone turned) draws the pages again to fit it.
const redraw = () => {
  drawn.clear();
  placeholders();
  pages.forEach((div) => { watcher.unobserve(div); watcher.observe(div); });
};
let width = holder.clientWidth;
let pending = 0;
const resized = new ResizeObserver(() => {
  if (holder.clientWidth === width) return;
  width = holder.clientWidth;
  clearTimeout(pending);
  pending = setTimeout(redraw, 200);
});
resized.observe(holder);
window.__vmPdf = window.__vmPdf || {};
// Stays running (so it can keep telling the app the page) until the viewer closes.
await new Promise((resolve) => {
  window.__vmPdf[input.container] = {
    zoom: (z) => {
      zoom = z;
      redraw();
    },
    close: () => {
      watcher.disconnect();
      resized.disconnect();
      task.destroy();
      delete window.__vmPdf[input.container];
      resolve();
    },
  };
});
return {};
"#;

/// The first page, 400 px on its long side, as a JPEG (Base64), for the tile.
const THUMBNAIL: &str = r##"
const page = await doc.getPage(1);
const unit = page.getViewport({ scale: 1 });
const viewport = page.getViewport({ scale: 400 / Math.max(unit.width, unit.height) });
const canvas = document.createElement("canvas");
canvas.width = Math.round(viewport.width);
canvas.height = Math.round(viewport.height);
const context = canvas.getContext("2d");
context.fillStyle = "#fff";
context.fillRect(0, 0, canvas.width, canvas.height);
await page.render({ canvas, canvasContext: context, viewport }).promise;
const blob = await new Promise((resolve) => canvas.toBlob(resolve, "image/jpeg", 0.85));
const jpeg = new Uint8Array(await blob.arrayBuffer());
let text = "";
for (let i = 0; i < jpeg.length; i += 0x8000) text += String.fromCharCode.apply(null, jpeg.subarray(i, i + 0x8000));
await task.destroy();
return { jpeg: btoa(text) };
"##;

/// A script that receives its input, then opens the PDF, then runs `body`; any failure is
/// reported as `{ error }`.
fn script(body: &str) -> String {
    format!(
        "const input = await dioxus.recv();\ntry {{\n{OPEN}\n{body}\n}} catch (error) {{\n  dioxus.send({{ error: String(error) }});\n  return {{ error: String(error) }};\n}}"
    )
}

#[derive(Serialize)]
struct Input<'a> {
    base: String,
    data: &'a str,
    container: &'a str,
}

#[derive(Deserialize)]
struct Thumbnail {
    jpeg: Option<String>,
    error: Option<String>,
}

/// The first page of a PDF (`bytes`) as a 400 px JPEG, or `None` if pdf.js can't draw it (the
/// tile then shows a page icon).
pub(crate) async fn first_page_thumbnail(bytes: &[u8]) -> Option<Vec<u8>> {
    let eval = document::eval(&script(THUMBNAIL));
    let data = crate::files::to_base64(bytes);
    eval.send(Input {
        base: PDFJS.to_string(),
        data: &data,
        container: "",
    })
    .ok()?;
    match eval.join::<Thumbnail>().await {
        Ok(Thumbnail {
            jpeg: Some(jpeg), ..
        }) => crate::files::from_base64(&jpeg),
        Ok(Thumbnail { error, .. }) => {
            tracing::debug!(target: domain::log_area::UI, ?error, "couldn't draw a PDF's first page");
            None
        }
        Err(error) => {
            tracing::debug!(target: domain::log_area::UI, %error, "couldn't draw a PDF's first page");
            None
        }
    }
}

/// What the viewer script tells the app.
#[derive(Debug, Deserialize)]
struct Update {
    page: Option<u32>,
    pages: Option<u32>,
    error: Option<String>,
}

/// The zoom steps, as multiples of "fit the width".
pub(crate) const ZOOMS: [f64; 7] = [0.5, 0.75, 1.0, 1.25, 1.5, 2.0, 3.0];

/// Where the viewer is: the page in view, how many there are, or that it couldn't open.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PdfState {
    /// The page in view (1-based).
    pub(crate) page: u32,
    /// How many pages.
    pub(crate) pages: u32,
    /// It couldn't be opened.
    pub(crate) failed: bool,
}

/// A PDF's pages (`data`, Base64), one under another, in a scrolling container `container`.
/// `state` follows the page in view; `zoom` is an index into [`ZOOMS`].
#[component]
pub(crate) fn PdfPages(
    container: String,
    data: String,
    zoom: ReadSignal<usize>,
    state: Signal<PdfState>,
) -> Element {
    let mut state = state;
    let opened = container.clone();
    use_effect(move || {
        let container = opened.clone();
        let data = data.clone();
        spawn(async move {
            let mut eval = document::eval(&script(VIEWER));
            if eval
                .send(Input {
                    base: PDFJS.to_string(),
                    data: &data,
                    container: &container,
                })
                .is_err()
            {
                state.set(PdfState {
                    failed: true,
                    ..PdfState::default()
                });
                return;
            }
            while let Ok(update) = eval.recv::<Update>().await {
                if let Some(error) = update.error {
                    tracing::debug!(target: domain::log_area::UI, error, "couldn't open a PDF");
                    state.set(PdfState {
                        failed: true,
                        ..PdfState::default()
                    });
                    break;
                }
                state.set(PdfState {
                    page: update.page.unwrap_or(1),
                    pages: update.pages.unwrap_or(0),
                    failed: false,
                });
            }
        });
    });
    let zoomed = container.clone();
    use_effect(move || {
        let factor = ZOOMS.get(zoom()).copied().unwrap_or(1.0);
        let name = &zoomed;
        let _ = document::eval(&format!(
            "window.__vmPdf && window.__vmPdf[{name:?}] && window.__vmPdf[{name:?}].zoom({factor});"
        ));
    });
    let closing = container.clone();
    use_drop(move || {
        let name = &closing;
        let _ = document::eval(&format!(
            "window.__vmPdf && window.__vmPdf[{name:?}] && window.__vmPdf[{name:?}].close();"
        ));
    });
    rsx! {
        div { id: "{container}", class: "pdf-pages" }
    }
}
