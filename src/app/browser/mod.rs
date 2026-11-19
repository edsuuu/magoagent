//! Portals: small browsers on the canvas. Each one is the system's own webview (wry: WebKit on
//! macOS, WebView2 on Windows, WebKitGTK on Linux) laid over the canvas and moved with it every
//! frame; the app draws the header and the address bar above the page. Agents drive them through
//! the bridge.

pub(crate) mod console;
pub(crate) mod design;
pub(crate) mod extras;
pub(crate) mod shot;
pub(crate) mod vault;

use crate::prelude::*;
mod access;
mod chrome;
mod dialog;
#[cfg(target_os = "macos")]
mod native;
mod paint;
mod sync;
#[cfg(test)]
mod tests;
mod types;
#[cfg(target_os = "linux")]
mod webkit;
#[cfg(target_os = "macos")]
pub(crate) use native::page_host;
pub(crate) use types::*;
#[cfg(target_os = "linux")]
pub(crate) use webkit::{PAGES_UNAVAILABLE, pages_open};
#[cfg(target_os = "linux")]
use webkit::{cut_page, new_tab_view, release_keys, veil};

impl DrawingApp {}

/// macOS with SwiftUI drawing the canvas: the pages lie in its view, under what SwiftUI draws over them (native.rs).
pub(crate) fn pages_in_swift() -> bool {
    #[cfg(target_os = "macos")]
    return crate::app::native::enabled("canvas");
    #[cfg(not(target_os = "macos"))]
    false
}

/// The selection box's outline as four 2-point strips, for the page to let through.
pub(crate) fn marquee_edges(m: Rect) -> [Rect; 4] {
    let (a, b) = (m.min - Vec2::splat(1.0), m.max + Vec2::splat(1.0));
    [
        Rect::from_min_max(a, Pos2::new(b.x, a.y + 2.0)),
        Rect::from_min_max(Pos2::new(a.x, b.y - 2.0), b),
        Rect::from_min_max(a, Pos2::new(a.x + 2.0, b.y)),
        Rect::from_min_max(Pos2::new(b.x - 2.0, a.y), b),
    ]
}
