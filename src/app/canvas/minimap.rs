//! The minimap's numbers: what it maps, how it holds still under a drag and settles after, and
//! where a click or an arrow takes the view.

use crate::prelude::*;

pub(crate) const MAP_SIZE: Vec2 = Vec2::new(200.0, 150.0);
/// The map's room around its drawing, inside the panel's own padding.
const PADDING: f32 = 16.0;
const BOUNDS_PAD: f32 = 200.0;
const MIN_DOT: f32 = 2.0;
const SETTLE: f64 = 0.25;

/// The area the map covers, held while the pointer is down and eased to the live one after.
#[derive(Default)]
pub(crate) struct MinimapArea {
    held: Option<Rect>,
    settle: Option<(f64, Rect)>,
    /// The surface had the keys last frame: the canvas leaves the arrows to it.
    pub(crate) focused: bool,
}

impl MinimapArea {
    /// The area to draw now: the held one, or the one settling, or the live one.
    pub(crate) fn area(&mut self, now: f64, live: Rect) -> Rect {
        if let Some(held) = self.held {
            return held;
        }
        let Some((since, from)) = self.settle else { return live };
        let t = ((now - since) / SETTLE).min(1.0) as f32;
        if t >= 1.0 {
            self.settle = None;
            return live;
        }
        let t = t * t * (3.0 - 2.0 * t);
        Rect::from_min_max(from.min + (live.min - from.min) * t, from.max + (live.max - from.max) * t)
    }

    pub(crate) fn settling(&self) -> bool {
        self.settle.is_some()
    }

    /// The pointer went down (`down`) or came up: the area stays put while it is down, then settles.
    pub(crate) fn press(&mut self, down: bool, now: f64, shown: Rect) {
        match (down, self.held) {
            (true, None) => {
                self.settle = None;
                self.held = Some(shown);
            }
            (false, Some(held)) => {
                self.held = None;
                self.settle = Some((now, held));
            }
            _ => {}
        }
    }
}

/// Where a map of `area` puts a world point, and the scale it uses; `origin` is the map's top left.
pub(crate) fn map_scale(area: Rect) -> f32 {
    ((MAP_SIZE.x - PADDING * 2.0) / area.width().max(1.0)).min((MAP_SIZE.y - PADDING * 2.0) / area.height().max(1.0))
}

pub(crate) fn to_map(area: Rect, origin: Pos2, world: Pos2) -> Pos2 {
    origin + Vec2::splat(PADDING) + (world - area.min) * map_scale(area)
}

pub(crate) fn to_world(area: Rect, origin: Pos2, map: Pos2) -> Pos2 {
    area.min + (map - origin - Vec2::splat(PADDING)) / map_scale(area)
}

/// How the map draws an element.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum MapDot {
    Window,
    Paper([u8; 4]),
    Shape,
}

/// A node's dot: its box at least 2 points each way.
pub(crate) fn dot(a: Pos2, b: Pos2) -> Rect {
    Rect::from_min_size(a, (b - a).max(Vec2::splat(MIN_DOT)))
}

impl DrawingApp {
    /// What the map shows: windows (terminals, portals, IDEs) in the accent, notes in their paper,
    /// and, unlike the reference app (it maps only its nodes), the drawings as outlines.
    pub(crate) fn minimap_nodes(&self) -> Vec<(Rect, MapDot)> {
        let dot = |e: &Element| match () {
            _ if e.created.is_some() => MapDot::Paper(e.style.fill.unwrap_or(STICKY_PAPER)),
            _ if e.terminal.is_some() || e.portal.is_some() || e.ide.is_some() => MapDot::Window,
            _ => MapDot::Shape,
        };
        self.scene.elements.iter().map(|e| (element_bounds(e), dot(e))).collect()
    }

    /// The nodes' box with a margin; what a click may take the view to.
    pub(crate) fn minimap_reach(&self) -> Option<Rect> {
        self.minimap_nodes().into_iter().map(|(r, _)| r).reduce(|a, b| a.union(b)).map(|r| r.expand(BOUNDS_PAD))
    }

    /// The canvas on screen, in the world.
    pub(crate) fn visible_world(&self) -> Rect {
        Rect::from_two_pos(self.to_world(self.canvas_rect.min), self.to_world(self.canvas_rect.max))
    }

    /// World area the map covers at rest: the nodes' box with a margin, and what is on screen.
    pub(crate) fn minimap_world(&self) -> Rect {
        let view = self.visible_world();
        self.minimap_reach().map_or(view, |r| r.union(view))
    }

    /// Centers the view on a world point, kept inside the nodes' reach.
    pub(crate) fn minimap_go(&mut self, center: Pos2) {
        let center = self.minimap_reach().map_or(center, |r| center.clamp(r.min, r.max));
        self.pan_to(center);
    }

    /// An arrow key on the map: a fifth of the view.
    pub(crate) fn minimap_nudge(&mut self, dx: f32, dy: f32) {
        let view = self.visible_world();
        self.minimap_go(view.center() + view.size() * Vec2::new(dx, dy) * 0.2);
    }
}
