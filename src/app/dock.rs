//! The enlarged window held at a side of the screen: its title dragged to the left or right edge
//! docks it as a full-height column that stays put while the canvas pans and zooms under it.
//! Values from the reference app: the width per kind, the zone it docks in, the snap to that width.

use crate::prelude::*;

/// How far in from the edge a drag docks a panel already docked there.
const DOCKED_ZONE: f32 = 110.0;
/// Around the docked column, and how close a resized width falls back to the default one.
const DOCK_MARGIN: f32 = 8.0;
const DOCK_SNAP: f32 = 28.0;
const DOCK_MIN: f32 = 240.0;

impl DrawingApp {
    /// The column's width for a kind: terminal and portal 520, note 400, an IDE 340 for its tree
    /// or 720 with files open.
    pub(crate) fn dock_width(&self, element: &Element) -> f32 {
        if element.created.is_some() {
            400.0
        } else if element.ide.is_some() {
            if self.ides.get(&element.id).is_some_and(|ide| ide.has_text_tabs()) { 720.0 } else { 340.0 }
        } else {
            520.0
        }
    }

    /// The docked column: under the top row, the height of the window, at the side (`right`).
    pub(crate) fn dock_rect(&self, right: bool, width: f32) -> Rect {
        let c = self.canvas_rect;
        let top = TITLE_BAR.max(TOP_ROW + 48.0) + DOCK_MARGIN;
        let width = width.min(c.width() - 2.0 * DOCK_MARGIN).max(1.0);
        let left = if right { c.right() - DOCK_MARGIN - width } else { c.left() + DOCK_MARGIN };
        Rect::from_min_max(Pos2::new(left, top), Pos2::new(left + width, c.bottom() - DOCK_MARGIN))
    }

    /// How far the corner buttons step in, (left, right): past a docked column on that side.
    pub(crate) fn corner_inset(&self) -> (f32, f32) {
        match self.dock.filter(|_| self.terminal_elevated.is_some()) {
            Some((right, _)) => {
                let w = self.elevated_rect().width() + DOCK_MARGIN;
                if right { (0.0, w) } else { (w, 0.0) }
            }
            None => (0.0, 0.0),
        }
    }

    /// Where a title let go at `x` puts the panel: docked at a side (keeping its width if it stays
    /// there), or back in the middle. The zone is as wide as the column would be, 110 to a third
    /// of the screen; 110 for the side it is already on.
    pub(crate) fn dock_side(&self, x: f32, element: &Element) -> Option<(bool, f32)> {
        let c = self.canvas_rect;
        let width = self.dock_width(element);
        let zone = |right: bool| match self.dock {
            Some((side, _)) if side == right => DOCKED_ZONE,
            _ => (width + 12.0).max(DOCKED_ZONE).min(c.width() / 3.0),
        };
        let right = if x < c.left() + zone(false) {
            false
        } else if x > c.right() - zone(true) {
            true
        } else {
            return None;
        };
        let kept = self.dock.filter(|(side, _)| *side == right).map(|(_, w)| w);
        Some((right, kept.unwrap_or(width)))
    }

    /// A docked width dragged to `width`: on the canvas grid, back to the kind's own within 28.
    pub(crate) fn dock_resized(&self, width: f32, element: &Element) -> f32 {
        let default = self.dock_width(element);
        let width = if (width - default).abs() < DOCK_SNAP { default } else { (width / GRID).round() * GRID };
        width.clamp(DOCK_MIN, (self.canvas_rect.width() * 0.7).max(DOCK_MIN))
    }

    /// The enlarged window's title as a handle (short of its buttons): dragged, the panel follows
    /// and docks where it is let go; a double-click puts it back. Docked, its inner edge resizes it.
    pub(crate) fn dock_grip(&mut self, ctx: &Context) {
        let Some(i) = self.elevated_index() else {
            self.dock = None;
            self.dock_drag = None;
            return;
        };
        if self.elevate_animating() {
            return;
        }
        let element = self.scene.elements[i].clone();
        let (frame, zoom) = self.terminal_frame(&element);
        // (height, left, right) of the strip: an IDE's own bar leaves its name and its status out
        let (head, left, right) = if element.ide.is_some() {
            (40.0, 200.0, 260.0)
        } else if let Some(portal) = &element.portal {
            (if portal.chrome_hidden { 0.0 } else { PORTAL_HEADER * zoom }, 0.0, 48.0)
        } else if element.created.is_some() {
            (NOTE_HEADER * zoom, 0.0, 0.0)
        } else {
            (TERMINAL_HEADER * zoom, 0.0, 48.0)
        };
        let grip = Rect::from_min_max(frame.min + Vec2::new(left, 0.0), Pos2::new(frame.right() - right, frame.top() + head));
        let at = ctx.input(|i| i.pointer.interact_pos());
        if grip.is_positive() {
            let r = egui::Area::new(egui::Id::unique("dock_grip"))
                .order(egui::Order::Foreground)
                .fixed_pos(grip.min)
                .show(ctx, |ui| ui.allocate_exact_size(grip.size(), Sense::click_and_drag()).1)
                .inner;
            let r = r.on_hover_cursor(if self.dock_drag.is_some() { CursorIcon::Grabbing } else { CursorIcon::Grab });
            if r.double_clicked() {
                self.set_elevated(None);
                return;
            }
            if let (true, Some(p)) = (r.drag_started(), at) {
                self.dock_drag = Some((p - frame.min, p));
            } else if let (Some((grab, _)), Some(p)) = (self.dock_drag, at.filter(|_| r.dragged())) {
                self.dock_drag = Some((grab, p));
            }
            if r.drag_stopped() {
                if let Some((_, p)) = self.dock_drag.take() {
                    self.dock = self.dock_side(p.x, &element);
                }
            }
        }
        // where it would dock, while it is dragged
        if let Some((_, p)) = self.dock_drag {
            if let Some((side, width)) = self.dock_side(p.x, &element) {
                let painter = ctx.layer_painter(egui::LayerId::new(egui::Order::Middle, egui::Id::unique("dock_zone")));
                painter.rect(self.dock_rect(side, width), 12, PRIMARY.gamma_multiply(0.12), Stroke::new(1.5, PRIMARY), egui::StrokeKind::Inside);
            }
            ctx.request_repaint();
        }
        // docked: the inner edge sets the width
        let Some((side, _)) = self.dock.filter(|_| self.dock_drag.is_none()) else { return };
        let edge = if side { frame.left() } else { frame.right() };
        let handle = Rect::from_center_size(Pos2::new(edge, frame.center().y), Vec2::new(8.0, frame.height()));
        let r = egui::Area::new(egui::Id::unique("dock_edge"))
            .order(egui::Order::Foreground)
            .fixed_pos(handle.min)
            .show(ctx, |ui| ui.allocate_exact_size(handle.size(), Sense::drag()).1)
            .inner
            .on_hover_cursor(CursorIcon::ResizeHorizontal);
        if let (true, Some(p)) = (r.dragged(), at) {
            let c = self.canvas_rect;
            let width = if side { c.right() - DOCK_MARGIN - p.x } else { p.x - c.left() - DOCK_MARGIN };
            self.dock = Some((side, self.dock_resized(width, &element)));
        }
    }
}
