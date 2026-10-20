//! Painting elements on the egui canvas.

use crate::prelude::*;

pub(crate) mod images;
pub(crate) mod replay;
pub(crate) mod rough;
pub(crate) mod sticky;
pub(crate) mod svg;

pub(crate) struct View {
    pub(crate) origin: Vec2,
    pub(crate) zoom: f32,
    pub(crate) dark: bool,
    pub(crate) background: Color32,
}

pub(crate) fn rgba(c: [u8; 4]) -> Color32 {
    Color32::from_rgba_unmultiplied(c[0], c[1], c[2], c[3])
}

/// the reference app's dark mode: `invert(93%) hue-rotate(180deg)`, i.e. flip lightness, keep hue.
/// The web's dark-mode filter, `invert(93%) hue-rotate(180deg)` (applyDarkModeFilter).
pub(crate) fn themed(color: Color32, dark: bool) -> Color32 {
    if !dark {
        return color;
    }
    let [r, g, b, a] = color.to_srgba_unmultiplied();
    let [r, g, b] = [r, g, b].map(|v| (v as f32 * 0.07 + (255.0 - v as f32) * 0.93).round() / 255.0);
    let out = [
        -0.574 * r + 1.43 * g + 0.144 * b,
        0.426 * r + 0.43 * g + 0.144 * b,
        0.426 * r + 1.43 * g - 0.856 * b,
    ]
    .map(|v| (v.clamp(0.0, 1.0) * 255.0).round() as u8);
    Color32::from_rgba_unmultiplied(out[0], out[1], out[2], a)
}

pub(crate) fn with_opacity(color: [u8; 4], opacity: u8) -> Color32 {
    let a = (color[3] as f32 * opacity as f32 / 100.0).round() as u8;
    Color32::from_rgba_unmultiplied(color[0], color[1], color[2], a)
}

pub(crate) fn text_lines(value: &str) -> Vec<&str> {
    let lines: Vec<&str> = value.split('\n').collect();
    if lines.is_empty() { vec![""] } else { lines }
}

pub(crate) fn paint_element(painter: &Painter, element: &Element, view: &View, faded: bool, hide_text: bool, halo: bool) {
    let opacity = if faded { element.style.opacity / 3 } else { element.style.opacity };
    let stroke_color = themed(with_opacity(element.style.stroke, opacity), view.dark);
    let (center, angle) = (element.center(), element.angle);
    let to = |p: &[f32; 2]| {
        let p = if angle == 0.0 { Pos2::new(p[0], p[1]) } else { center + rot(Pos2::new(p[0], p[1]) - center, angle) };
        Pos2::new(view.origin.x + p.x * view.zoom, view.origin.y + p.y * view.zoom)
    };
    if let Some(url) = &element.image {
        let b = local_bounds(element);
        let corners = [b.left_top(), b.right_top(), b.right_bottom(), b.left_bottom()].map(|c| to(&[c.x, c.y]));
        match image_texture(painter.ctx(), url) {
            Some(texture) => {
                let mut mesh = egui::Mesh::with_texture(texture.id());
                let tint = Color32::WHITE.gamma_multiply(opacity as f32 / 100.0);
                for (pos, uv) in corners.into_iter().zip([Pos2::new(0.0, 0.0), Pos2::new(1.0, 0.0), Pos2::new(1.0, 1.0), Pos2::new(0.0, 1.0)]) {
                    mesh.vertices.push(egui::epaint::Vertex { pos, uv, color: tint });
                }
                mesh.indices.extend([0, 1, 2, 0, 2, 3]);
                painter.add(mesh);
            }
            None => {
                painter.add(Shape::closed_line(corners.to_vec(), Stroke::new(1.0, stroke_color)));
            }
        }
        return;
    }
    if let Some(created) = element.created {
        // sticky note: soft drop shadow, paper, faint inner edge and the creation date
        let b = local_bounds(element);
        let paper: Vec<Pos2> = sticky_outline(b, element.style.round, 0.0).iter().map(to).collect();
        let shadow = sticky_outline(b, element.style.round, 3.0).iter().map(to).collect();
        let alpha = opacity as f32 / 100.0;
        painter.add(Shape::convex_polygon(shadow, Color32::from_black_alpha((0.16 * alpha * 255.0) as u8), Stroke::NONE));
        let paper_color = themed(with_opacity(element.style.fill.unwrap_or([0xff, 0xdf, 0x6b, 255]), opacity), view.dark);
        painter.add(Shape::convex_polygon(paper.clone(), paper_color, Stroke::new(0.5 * view.zoom, Color32::from_black_alpha((0.08 * alpha * 255.0) as u8))));
        if b.width() >= 75.0 && b.height() >= 75.0 {
            if let Some(date) = sticky_date(created, b.width() - 32.0 < 80.0) {
                let at = to(&[b.max.x - 16.0, b.max.y - 11.0]);
                painter.text(at, egui::Align2::RIGHT_BOTTOM, date, egui::FontId::proportional(12.0 * view.zoom), stroke_color);
            }
        }
        return;
    }
    if let (Some(fill), Some(poly)) = (element.style.fill, shape_polygon(element)) {
        let fill = themed(with_opacity(fill, opacity), view.dark);
        if element.style.fill_style == FillStyle::Solid {
            painter.add(Shape::convex_polygon(poly.iter().map(to).collect(), fill, Stroke::NONE));
        } else {
            let s = Stroke::new((element.style.stroke_width / 2.0).max(0.5) * view.zoom, fill);
            for line in fill_strokes(element) {
                painter.add(Shape::line(line.iter().map(to).collect(), s));
            }
        }
    }
    let width = if matches!(element.kind, ElementKind::FreeDraw { .. }) {
        element.style.stroke_width * 1.5
    } else {
        element.style.stroke_width
    };
    let stroke = Stroke::new(width * view.zoom, stroke_color);
    for outline in rough_outlines(element) {
        if outline.len() < 2 {
            continue;
        }
        let pts: Vec<Pos2> = outline.iter().map(to).collect();
        let sw = element.style.stroke_width;
        match element.style.stroke_style {
            StrokeStyle::Solid => {
                painter.add(Shape::line(pts, stroke));
            }
            StrokeStyle::Dashed => painter.extend(Shape::dashed_line(&pts, stroke, 8.0 * view.zoom, (8.0 + sw) * view.zoom)),
            StrokeStyle::Dotted => painter.extend(Shape::dashed_line(&pts, stroke, 1.5 * view.zoom, (6.0 + sw) * view.zoom)),
        }
    }
    for (shape, head) in arrowheads(element) {
        let pts: Vec<Pos2> = shape.iter().map(to).collect();
        match head.fill() {
            Some(solid) => {
                let fill = if solid { stroke_color } else { view.background };
                painter.add(Shape::convex_polygon(pts, fill, stroke));
            }
            None => {
                painter.add(Shape::line(pts, stroke));
            }
        }
    }
    if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&element.frame, &element.kind) {
        if !name.is_empty() {
            let color = themed(Color32::from_rgb(0x99, 0x99, 0x99), view.dark);
            painter.text(to(&[*x, y - 4.0]), egui::Align2::LEFT_BOTTOM, name, FontId::proportional(13.0 * view.zoom.max(0.5)), color);
        }
    }
    if let (ElementKind::Text { x, y, value }, false) = (&element.kind, hide_text) {
        let size = element.style.font_size;
        let font = FontId::new(size * view.zoom, egui::FontFamily::Name(HAND_FONT.into()));
        let width = local_bounds(element).width() * view.zoom;
        for (i, line) in text_lines(value).into_iter().enumerate() {
            let galley = painter.layout_no_wrap(line.to_string(), font.clone(), stroke_color);
            // labels are centered like the reference app's container text
            let indent = if is_label(element) { (width - galley.size().x).max(0.0) / 2.0 / view.zoom } else { 0.0 };
            let pos = to(&[*x + indent, y + i as f32 * size * LINE_HEIGHT]);
            if halo {
                // background-colored outline keeps text readable over lines
                let ring = (size * view.zoom * 0.08).clamp(1.0, 3.0);
                for k in 0..12 {
                    let offset = Vec2::angled(k as f32 / 12.0 * std::f32::consts::TAU) * ring;
                    let shadow = painter.layout_no_wrap(line.to_string(), font.clone(), view.background);
                    painter.add(egui::epaint::TextShape::new(pos + offset, shadow, view.background).with_angle(angle));
                }
            }
            painter.add(egui::epaint::TextShape::new(pos, galley, stroke_color).with_angle(angle));
        }
    }
}
