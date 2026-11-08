//! Minimap, and the line between a menu's groups of rows.

use crate::prelude::*;

impl DrawingApp {
    /// Scaled-down drawing with the visible area; click or drag to move there.
    pub(crate) fn minimap(&mut self, ui: &mut egui::Ui) {
        let c = self.colors();
        let (rect, response) = ui.allocate_exact_size(Vec2::new(200.0, 130.0), Sense::click_and_drag());
        let world = self.minimap_world();
        let scale = (rect.width() / world.width()).min(rect.height() / world.height());
        let to_map = |p: Pos2| rect.center() + (p - world.center()) * scale;
        let p = ui.painter_at(rect);
        p.rect_filled(rect, 6, themed(rgba(self.scene.background), self.dark()));
        let ink = c.text.gamma_multiply(0.55);
        for e in &self.scene.elements {
            let pts: Vec<[f32; 2]> = match &e.kind {
                ElementKind::Line { .. } => line_path(e),
                ElementKind::FreeDraw { points } => points.iter().step_by(4).copied().collect(),
                _ => binding_outline(e, 0.0),
            };
            let mapped: Vec<Pos2> = pts.iter().map(|q| to_map(Pos2::new(q[0], q[1]))).collect();
            match &e.kind {
                ElementKind::Text { .. } => {
                    // text reads as a thin bar at this size
                    let b = Rect::from_two_pos(to_map(element_bounds(e).min), to_map(element_bounds(e).max));
                    p.rect_filled(b.shrink2(Vec2::new(0.0, b.height() * 0.3)), 1, ink);
                }
                ElementKind::Line { .. } | ElementKind::FreeDraw { .. } => {
                    p.add(Shape::line(mapped, Stroke::new(1.0, ink)));
                }
                _ => {
                    if let Some(fill) = e.style.fill {
                        p.add(Shape::convex_polygon(mapped.clone(), themed(rgba(fill), self.dark()).gamma_multiply(0.7), Stroke::NONE));
                    }
                    p.add(Shape::closed_line(mapped, Stroke::new(1.0, ink)));
                }
            }
        }
        let view = Rect::from_two_pos(to_map(self.to_world(self.canvas_rect.min)), to_map(self.to_world(self.canvas_rect.max)));
        p.rect(view, 2, PRIMARY.gamma_multiply(0.12), Stroke::new(1.5, PRIMARY), StrokeKind::Middle);
        if let Some(pos) = response.interact_pointer_pos().filter(|_| response.clicked() || response.dragged()) {
            self.pan_to(world.center() + (pos - rect.center()) / scale);
        }
        response.on_hover_cursor(CursorIcon::PointingHand).on_hover_text("Clique ou arraste para navegar");
    }
}

pub(crate) fn menu_line(ui: &mut egui::Ui, c: &UiColors) {
    let (line, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 9.0), Sense::hover());
    ui.painter().hline(line.x_range(), line.center().y, Stroke::new(1.0, c.surface_low));
}
