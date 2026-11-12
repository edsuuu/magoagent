//! The mini chat's panel in a window of its own: over every other window, out of the app's if
//! dragged there, its width and height pulled from any edge; where and how big it was left is kept.

use crate::app::chat_list::Look;
use crate::app::chat_mini::{HEIGHT, WIDTH};
use crate::app::ui::agents::AgentRow;
use crate::prelude::*;
use egui::viewport::ResizeDirection;

/// The smallest the window goes.
const MIN_SIZE: [f32; 2] = [300.0, 360.0];
/// How far in from its border a drag pulls an edge.
const EDGE: f32 = 6.0;

/// Where a window of `size` first opens beside the app's (`app`, on screen): its top right.
pub(crate) fn first_place(app: Rect, size: Vec2) -> [f32; 4] {
    [(app.right() - size.x - 24.0).round(), (app.top() + 90.0).round(), size.x, size.y]
}

/// The edge (or corner) at `at` of `rect`, if `at` is that near one.
pub(crate) fn edge_at(rect: Rect, at: Pos2) -> Option<ResizeDirection> {
    if !rect.contains(at) {
        return None;
    }
    let (left, right) = (at.x - rect.left() < EDGE, rect.right() - at.x < EDGE);
    let (top, bottom) = (at.y - rect.top() < EDGE, rect.bottom() - at.y < EDGE);
    Some(match (left, right, top, bottom) {
        (true, _, true, _) => ResizeDirection::NorthWest,
        (_, true, true, _) => ResizeDirection::NorthEast,
        (true, _, _, true) => ResizeDirection::SouthWest,
        (_, true, _, true) => ResizeDirection::SouthEast,
        (true, ..) => ResizeDirection::West,
        (_, true, ..) => ResizeDirection::East,
        (_, _, true, _) => ResizeDirection::North,
        (.., true) => ResizeDirection::South,
        _ => return None,
    })
}

/// Its edges, over everything in it: a press there hands the resize to the system.
pub(crate) fn resize_edges(ui: &mut egui::Ui, rect: Rect) {
    let Some(at) = ui.ctx().pointer_hover_pos() else { return };
    let Some(edge) = edge_at(rect, at) else { return };
    let cursor = match edge {
        ResizeDirection::North | ResizeDirection::South => CursorIcon::ResizeVertical,
        ResizeDirection::East | ResizeDirection::West => CursorIcon::ResizeHorizontal,
        ResizeDirection::NorthWest | ResizeDirection::SouthEast => CursorIcon::ResizeNwSe,
        ResizeDirection::NorthEast | ResizeDirection::SouthWest => CursorIcon::ResizeNeSw,
    };
    let band = Rect::from_center_size(at, Vec2::splat(2.0 * EDGE));
    let r = ui.interact(band, ui.scope_id().with("mini_edge"), Sense::drag()).on_hover_cursor(cursor);
    if r.hovered() && ui.input(|i| i.pointer.primary_pressed()) {
        ui.ctx().send_viewport_cmd(egui::ViewportCommand::BeginResize(edge));
    }
}

impl DrawingApp {
    /// Every frame the panel is open (and the backend has windows): its window.
    pub(crate) fn mini_window(&mut self, ctx: &Context, row: &AgentRow, look: &Look, c: &UiColors, time: f64, unread: bool) {
        let [x, y, w, h] = self.prefs.mini_chat_window.unwrap_or_else(|| {
            let app = ctx.input(|i| i.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
            first_place(app, Vec2::new(WIDTH, HEIGHT))
        });
        let builder = egui::ViewportBuilder::default()
            .with_title("Mini chat")
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            .with_resizable(true)
            .with_position([x, y])
            .with_inner_size([w, h])
            .with_min_inner_size(MIN_SIZE);
        let (placed, close) = ctx.show_viewport_immediate(egui::ViewportId::from_hash_of("mini_chat_window"), builder, |ui, _| {
            let inner = ui.ctx().clone();
            self.mini.window = true;
            self.mini_panel(&inner, row, ui.max_rect(), look, c, time, unread);
            self.mini.window = false;
            inner.input(|i| {
                let v = i.viewport();
                let placed = v.outer_rect.zip(v.inner_rect).map(|(o, n)| [o.left().round(), o.top().round(), n.width().round(), n.height().round()]);
                (placed, v.close_requested())
            })
        });
        // nothing of it lies over the canvas
        self.mini.rect = None;
        if close {
            self.mini.open = false;
        }
        if let Some(placed) = placed.filter(|p| p[2] >= MIN_SIZE[0] && p[3] >= MIN_SIZE[1]) {
            self.prefs.mini_chat_window = Some(placed);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_edge_or_corner_is_pulled_within_a_few_points_of_the_border() {
        let r = Rect::from_min_size(Pos2::ZERO, Vec2::new(340.0, 600.0));
        assert!(matches!(edge_at(r, Pos2::new(2.0, 300.0)), Some(ResizeDirection::West)));
        assert!(matches!(edge_at(r, Pos2::new(338.0, 598.0)), Some(ResizeDirection::SouthEast)));
        assert!(matches!(edge_at(r, Pos2::new(170.0, 1.0)), Some(ResizeDirection::North)));
        assert!(edge_at(r, Pos2::new(170.0, 300.0)).is_none(), "inside: the chat's");
        assert!(edge_at(r, Pos2::new(-3.0, 300.0)).is_none(), "outside: not its");
        assert_eq!(first_place(Rect::from_min_size(Pos2::new(100.0, 50.0), Vec2::new(1280.0, 800.0)), Vec2::new(340.0, 600.0)), [1016.0, 140.0, 340.0, 600.0]);
    }
}
