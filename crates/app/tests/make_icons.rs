//! Makes the app icon's PNGs from its master SVG (phase-3.md 1a), and checks they're current.
//!
//! The PNGs are committed, so builds need no SVG renderer. After changing
//! `assets/app-icon/icon.svg`, write them again with:
//!
//! ```text
//! cargo test -p app --test make_icons -- --ignored
//! ```

use std::path::{Path, PathBuf};

/// The sizes made: the browser tab (16, 32), the iPhone home screen (180), Android and the
/// web app manifest (192, 512), and the desktop window and bundle (64, 128, 256, 512).
const SIZES: [u32; 8] = [16, 32, 64, 128, 180, 192, 256, 512];

fn icon_folder() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/app-icon")
}

/// The master SVG drawn at `size` × `size`, as a PNG (`None` if it can't be read or drawn).
fn render(size: u32) -> Option<Vec<u8>> {
    let svg = std::fs::read(icon_folder().join("icon.svg")).ok()?;
    let tree = resvg::usvg::Tree::from_data(&svg, &resvg::usvg::Options::default()).ok()?;
    let mut pixmap = resvg::tiny_skia::Pixmap::new(size, size)?;
    #[expect(
        clippy::cast_precision_loss,
        reason = "icon sizes are at most 512, exact in f32"
    )]
    let scale = size as f32 / tree.size().width();
    resvg::render(
        &tree,
        resvg::tiny_skia::Transform::from_scale(scale, scale),
        &mut pixmap.as_mut(),
    );
    pixmap.encode_png().ok()
}

fn png_path(size: u32) -> PathBuf {
    icon_folder().join(format!("icon-{size}.png"))
}

#[test]
#[ignore = "writes the icon PNGs; run after changing icon.svg"]
fn write_icon_pngs() {
    for size in SIZES {
        let png = render(size).expect("the master SVG draws");
        std::fs::write(png_path(size), png).expect("writing a PNG");
    }
}

/// The most one color channel may differ: drawing differs by a shade between CPUs (SIMD).
const TOLERANCE: u8 = 3;

#[test]
fn the_committed_pngs_match_the_svg() {
    for size in SIZES {
        let committed = std::fs::read(png_path(size))
            .ok()
            .and_then(|bytes| resvg::tiny_skia::Pixmap::decode_png(&bytes).ok());
        let fresh = render(size).expect("the master SVG draws");
        let fresh = resvg::tiny_skia::Pixmap::decode_png(&fresh).expect("decoding");
        let matches = committed.is_some_and(|committed| {
            committed.width() == size
                && committed.height() == size
                && committed
                    .data()
                    .iter()
                    .zip(fresh.data())
                    .all(|(old, new)| old.abs_diff(*new) <= TOLERANCE)
        });
        assert!(
            matches,
            "assets/app-icon/icon-{size}.png is missing or out of date with icon.svg; \
             run `cargo test -p app --test make_icons -- --ignored`"
        );
    }
}

/// `Dioxus.toml`, which names the bundle's identifier and icons.
const DIOXUS_TOML: &str = include_str!("../Dioxus.toml");

#[test]
fn the_bundle_uses_the_app_id_and_existing_icons() {
    assert!(
        DIOXUS_TOML.contains(&format!("identifier = \"{}\"", domain::identity::BUNDLE_ID)),
        "Dioxus.toml's bundle identifier must be domain::identity::BUNDLE_ID"
    );
    let icons: Vec<&str> = DIOXUS_TOML
        .lines()
        .map(str::trim)
        .filter_map(|line| line.strip_prefix('"')?.strip_suffix("\","))
        .collect();
    assert!(!icons.is_empty(), "Dioxus.toml lists the bundle's icons");
    for icon in icons {
        let path = Path::new(env!("CARGO_MANIFEST_DIR")).join(icon);
        assert!(
            path.is_file(),
            "Dioxus.toml names {icon}, which doesn't exist"
        );
    }
}

#[test]
fn the_web_page_starts_with_the_app_name_as_its_title() {
    assert!(
        DIOXUS_TOML.contains(&format!("title = \"{}\"", domain::identity::DISPLAY_NAME)),
        "Dioxus.toml's [web.app] title must be domain::identity::DISPLAY_NAME"
    );
}
