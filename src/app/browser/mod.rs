//! Portals: small browsers on the canvas. Each one is the system's own webview (wry: WebKit on
//! macOS, WebView2 on Windows) laid over the canvas and moved with it every frame; the app draws
//! the header and the address bar above the page. Agents drive them through the bridge.

pub(crate) mod console;
pub(crate) mod design;
pub(crate) mod extras;
pub(crate) mod shot;

use crate::prelude::*;
mod access;
mod chrome;
mod dialog;
mod paint;
mod sync;
#[cfg(test)]
mod tests;
mod types;
pub(crate) use types::*;

impl DrawingApp {}

/// The selection box's outline as four 2-point strips, for the page to let through.
#[cfg(not(target_os = "linux"))]
pub(crate) fn marquee_edges(m: Rect) -> [Rect; 4] {
    let (a, b) = (m.min - Vec2::splat(1.0), m.max + Vec2::splat(1.0));
    [
        Rect::from_min_max(a, Pos2::new(b.x, a.y + 2.0)),
        Rect::from_min_max(Pos2::new(a.x, b.y - 2.0), b),
        Rect::from_min_max(a, Pos2::new(a.x + 2.0, b.y)),
        Rect::from_min_max(Pos2::new(b.x - 2.0, a.y), b),
    ]
}
