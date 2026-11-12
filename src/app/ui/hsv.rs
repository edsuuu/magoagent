//! The custom color picker of the note bar: a saturation/value square and a hue bar, drawn here
//! (egui's own has English labels and opens in a window of its own).

use crate::prelude::*;
use egui::ecolor::Hsva;
use egui::epaint::Mesh;

/// A square (saturation across, value up) over a hue bar, `width` wide. True when the color changed.
pub(crate) fn hsv_picker(ui: &mut egui::Ui, hsva: &mut Hsva, width: f32) -> bool {
    let mut changed = false;
    let (square, response) = ui.allocate_exact_size(Vec2::new(width, width * 0.7), Sense::click_and_drag());
    let hue = Color32::from(Hsva::new(hsva.h, 1.0, 1.0, 1.0));
    // white to the hue along the top, black along the bottom: bilinear, so four corners paint it
    let mut mesh = Mesh::default();
    for (pos, color) in
        [(square.left_top(), Color32::WHITE), (square.right_top(), hue), (square.right_bottom(), Color32::BLACK), (square.left_bottom(), Color32::BLACK)]
    {
        mesh.colored_vertex(pos, color);
    }
    mesh.add_triangle(0, 1, 2);
    mesh.add_triangle(0, 2, 3);
    ui.painter().add(Shape::mesh(mesh));
    if let (true, Some(p)) = (response.is_pointer_button_down_on(), response.interact_pointer_pos()) {
        hsva.s = ((p.x - square.left()) / square.width()).clamp(0.0, 1.0);
        hsva.v = 1.0 - ((p.y - square.top()) / square.height()).clamp(0.0, 1.0);
        changed = true;
    }
    let marker = Pos2::new(square.left() + hsva.s * square.width(), square.bottom() - hsva.v * square.height());
    ui.painter().circle_stroke(marker, 5.0, Stroke::new(2.5, Color32::BLACK));
    ui.painter().circle_stroke(marker, 5.0, Stroke::new(1.5, Color32::WHITE));

    ui.add_space(2.0);
    let (bar, response) = ui.allocate_exact_size(Vec2::new(width, 14.0), Sense::click_and_drag());
    let mut mesh = Mesh::default();
    for i in 0..=6 {
        let x = bar.left() + bar.width() * i as f32 / 6.0;
        let color = Color32::from(Hsva::new(i as f32 / 6.0, 1.0, 1.0, 1.0));
        mesh.colored_vertex(Pos2::new(x, bar.top()), color);
        mesh.colored_vertex(Pos2::new(x, bar.bottom()), color);
        if i > 0 {
            let k = 2 * i as u32;
            mesh.add_triangle(k - 2, k - 1, k);
            mesh.add_triangle(k - 1, k, k + 1);
        }
    }
    ui.painter().add(Shape::mesh(mesh));
    if let (true, Some(p)) = (response.is_pointer_button_down_on(), response.interact_pointer_pos()) {
        hsva.h = ((p.x - bar.left()) / bar.width()).clamp(0.0, 1.0);
        changed = true;
    }
    let x = bar.left() + hsva.h * bar.width();
    ui.painter().rect_stroke(
        Rect::from_center_size(Pos2::new(x, bar.center().y), Vec2::new(6.0, 18.0)),
        3,
        Stroke::new(2.0, Color32::WHITE),
        StrokeKind::Outside,
    );
    changed
}
