//! Minimap, and the line between a menu's groups of rows.

use crate::prelude::*;

impl DrawingApp {
    /// Scaled-down nodes with the visible area; click or drag to move there, arrows move a fifth of
    /// the view. True when it asks to be put away (the X).
    pub(crate) fn minimap(&mut self, ui: &mut egui::Ui) -> bool {
        let dark = self.dark();
        let accent = glass_accent(dark);
        let (panel, _) = ui.allocate_exact_size(crate::app::canvas::minimap::MAP_SIZE, Sense::hover());
        paint_glass(ui.painter(), panel, 13.0, dark, true);
        // the surface sits in the panel's 6 points of padding
        let surface = panel.shrink(4.875);
        let response = ui
            .interact(surface, egui::Id::unique("minimap_surface"), Sense::click_and_drag())
            .tip("Minimapa da tela. Clique ou arraste para mover a viewport. Use as setas para navegar.");
        let origin = surface.min;
        let live = self.minimap_world();
        let shown = self.minimap_area.area(self.now, live);
        let down = response.is_pointer_button_down_on();
        if down && !self.minimap_area.focused {
            response.request_focus();
        }
        self.minimap_area.press(down, self.now, shown);
        if let Some(pos) = response.interact_pointer_pos().filter(|_| down) {
            let local = pos.clamp(surface.min, surface.max);
            self.minimap_go(crate::app::canvas::minimap::to_world(shown, origin, local));
        }
        self.minimap_area.focused = response.has_focus();
        if self.minimap_area.focused {
            ui.memory_mut(|m| m.set_focus_lock_filter(response.id, egui::EventFilter { horizontal_arrows: true, vertical_arrows: true, ..Default::default() }));
            for (key, dx, dy) in [(Key::ArrowLeft, -1.0, 0.0), (Key::ArrowRight, 1.0, 0.0), (Key::ArrowUp, 0.0, -1.0), (Key::ArrowDown, 0.0, 1.0)] {
                if ui.input_mut(|i| i.consume_key(Modifiers::NONE, key)) {
                    self.minimap_nudge(dx, dy);
                }
            }
        }
        let p = ui.painter_at(surface);
        for (r, kind) in self.minimap_nodes() {
            let dot = crate::app::canvas::minimap::dot(
                crate::app::canvas::minimap::to_map(shown, origin, r.min),
                crate::app::canvas::minimap::to_map(shown, origin, r.max),
            );
            match kind {
                crate::app::canvas::minimap::MapDot::Window => _ = p.rect_filled(dot, 1, accent),
                crate::app::canvas::minimap::MapDot::Paper(c) => _ = p.rect_filled(dot, 1, rgba(c).gamma_multiply(0.8)),
                crate::app::canvas::minimap::MapDot::Shape => {
                    _ = p.rect_stroke(dot, 1, Stroke::new(1.0, glass_ink(dark).gamma_multiply(0.55)), StrokeKind::Inside)
                }
            }
        }
        let view = self.visible_world();
        let view =
            Rect::from_two_pos(crate::app::canvas::minimap::to_map(shown, origin, view.min), crate::app::canvas::minimap::to_map(shown, origin, view.max));
        p.rect_filled(view, 3, accent.gamma_multiply(0.08));
        p.rect_stroke(view, 3, Stroke::new(1.5, accent), StrokeKind::Inside);
        if response.has_focus() {
            p.rect_stroke(surface, 10, Stroke::new(2.0, accent), StrokeKind::Inside);
        }
        if self.minimap_area.settling() {
            ui.ctx().request_repaint();
        }
        // the X: 16 points, 8 from the corner
        let close = Rect::from_center_size(panel.right_top() + Vec2::new(-16.0, 16.0), Vec2::splat(16.0));
        let x = ui.interact(close, egui::Id::unique("minimap_close"), Sense::click());
        let ink = glass_ink(dark);
        let fill = if dark { Color32::from_rgb(0x3a, 0x3a, 0x3c) } else { Color32::from_rgb(0xf6, 0xf6, 0xf6) };
        ui.painter().circle_filled(close.center(), 8.0, fill);
        for s in [-1.0, 1.0] {
            let d = Vec2::new(3.0, 3.0 * s);
            ui.painter().line_segment([close.center() - d, close.center() + d], Stroke::new(1.2, ink.gamma_multiply(if x.hovered() { 1.0 } else { 0.6 })));
        }
        x.on_hover_cursor(CursorIcon::PointingHand).tip("Fechar minimapa").clicked()
    }
}

pub(crate) fn menu_line(ui: &mut egui::Ui, c: &UiColors) {
    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
}
