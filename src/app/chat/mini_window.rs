//! The mini chat's panel in a window of its own: over every other window, out of the app's if
//! dragged there, its width and height pulled from any edge; where and how big it was left is kept.

use crate::app::chat::list::Look;
use crate::app::chat::mini::{HEIGHT, WIDTH};
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
        let saved = self.prefs.mini_chat_window;
        let [x, y, w, h] = *self.mini.opened_at.get_or_insert_with(|| {
            saved.unwrap_or_else(|| {
                let app = ctx.input(|i| i.viewport().outer_rect).unwrap_or(Rect::from_min_size(Pos2::ZERO, Vec2::new(1280.0, 800.0)));
                first_place(app, Vec2::new(WIDTH, HEIGHT))
            })
        });
        let builder = egui::ViewportBuilder::default()
            .with_title("Mini chat")
            .with_transparent(true)
            .with_decorations(false)
            .with_always_on_top()
            .with_taskbar(false)
            // shown once the system's frame is off it (`plain::strip`): no glass square for a frame
            .with_visible(!cfg!(windows) || self.mini.plain)
            .with_resizable(true)
            .with_position([x, y])
            .with_inner_size([w, h])
            .with_min_inner_size(MIN_SIZE);
        let id = egui::ViewportId::from_hash_of("mini_chat_window");
        // made again (opened after the pill), the window has the system's glass frame again
        if !ctx.input(|i| i.raw.viewports.contains_key(&id)) {
            self.mini.plain = false;
        }
        let (placed, close) = ctx.show_viewport_immediate(id, builder, |ui, _| {
            let inner = ui.ctx().clone();
            self.mini.window = true;
            // its top row is cut off (plain::strip): the panel starts under it
            let area = ui.max_rect();
            let area = if cfg!(windows) { area.with_min_y(area.top() + 1.0) } else { area };
            self.mini_panel(&inner, row, area, look, c, time, unread);
            self.mini.window = false;
            inner.input(|i| {
                let v = i.viewport();
                let placed = v.outer_rect.zip(v.inner_rect).map(|(o, n)| [o.left().round(), o.top().round(), n.width().round(), n.height().round()]);
                (placed, v.close_requested())
            })
        });
        // its corners are the panel's own, round (mini.rs): none of the system's square border
        #[cfg(windows)]
        if !self.mini.plain {
            self.mini.plain = crate::app::voice::popup::plain::strip("Mini chat");
        }
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

    #[test]
    fn the_panel_opens_from_the_pill_the_way_it_fits_on_the_screen() {
        use crate::app::chat::mini::panel_place;
        // a second monitor to the right, less its taskbar
        let work = Rect::from_min_max(Pos2::new(1920.0, 0.0), Pos2::new(3840.0, 1040.0));
        let size = Vec2::new(340.0, 600.0);
        let pill = |x: f32, y: f32| Rect::from_min_size(Pos2::new(x, y), Vec2::new(220.0, 44.0));
        // room everywhere: its top right on the pill's, going down
        assert_eq!(panel_place(pill(3000.0, 100.0), size, work), Rect::from_min_size(Pos2::new(2880.0, 100.0), size));
        // near the bottom: upward, its bottom on the pill's
        assert_eq!(panel_place(pill(3000.0, 900.0), size, work).bottom(), 944.0);
        // near the left edge: to the right of it, from the pill's left
        assert_eq!(panel_place(pill(1930.0, 100.0), size, work).left(), 1930.0);
        // half off the screen: pulled in whole
        let r = panel_place(pill(3700.0, -30.0), size, work);
        assert!(work.contains_rect(r), "{r:?}");
        // a screen smaller than the panel: as big as the screen
        let small = Rect::from_min_size(Pos2::ZERO, Vec2::new(800.0, 500.0));
        assert_eq!(panel_place(pill(100.0, 100.0), size, small).size(), Vec2::new(340.0, 500.0));
    }
}
