//! Replaying what an AI drew through the MCP server, with its cursor.

use crate::prelude::*;

/// Hand-drawn outline in world coordinates, shared by the canvas and the SVG/PNG export.
/// How long the agent's cursor stays after it finishes drawing (seconds).
pub(crate) const AGENT_LINGER: f64 = 1.5;
pub(crate) const AGENT_COLOR: Color32 = Color32::from_rgb(0xe8, 0x59, 0x0c);

/// Seconds to replay one AI-drawn element: strokes by their length, text by its letters.
pub(crate) fn reveal_duration(e: &Element) -> f64 {
    match &e.kind {
        ElementKind::Text { value, .. } => (value.chars().count() as f64 * 0.035).clamp(0.25, 1.0),
        _ if e.image.is_some() || e.created.is_some() || e.frame.is_some() => 0.35,
        _ => {
            let length: f32 = rough_outlines(e).iter().flat_map(|o| o.windows(2).map(|w| dist(w[0], w[1]))).sum();
            (length as f64 / 1400.0).clamp(0.3, 1.0)
        }
    }
}

/// Where the cursor rests once `e` is drawn (world).
pub(crate) fn reveal_pen(e: &Element) -> Pos2 {
    match &e.kind {
        ElementKind::Text { .. } => local_bounds(e).right_center(),
        ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } if !points.is_empty() => {
            let last = points[points.len() - 1];
            Pos2::new(last[0], last[1])
        }
        _ => element_bounds(e).right_bottom(),
    }
}

/// Draws `e` partway, the way a hand would: strokes along their outline, then fill and
/// arrowheads fade in; text is typed; notes, images and frames fade in. Returns the pen
/// position on screen.
pub(crate) fn paint_partial(painter: &Painter, e: &Element, view: &View, progress: f32) -> Pos2 {
    let (center, angle) = (e.center(), e.angle);
    let to = |p: [f32; 2]| {
        let p = if angle == 0.0 { Pos2::new(p[0], p[1]) } else { center + rot(Pos2::new(p[0], p[1]) - center, angle) };
        Pos2::new(view.origin.x + p.x * view.zoom, view.origin.y + p.y * view.zoom)
    };
    let mut part = e.clone();
    match &mut part.kind {
        ElementKind::Text { value, .. } => {
            let n = (value.chars().count() as f32 * progress).ceil() as usize;
            *value = value.chars().take(n).collect();
            paint_element(painter, &part, view, false, false, false);
            let b = local_bounds(&part);
            return to([b.max.x, b.center().y]);
        }
        _ if e.image.is_some() || e.created.is_some() || e.frame.is_some() => {
            part.style.opacity = (e.style.opacity as f32 * progress) as u8;
            paint_element(painter, &part, view, false, false, false);
            let b = local_bounds(e);
            return to([b.max.x, b.max.y]);
        }
        _ => {}
    }
    let outlines = rough_outlines(e);
    let total: f32 = outlines.iter().flat_map(|o| o.windows(2).map(|w| dist(w[0], w[1]))).sum();
    let mut budget = total * (progress / 0.8).min(1.0);
    let width = if matches!(e.kind, ElementKind::FreeDraw { .. }) { e.style.stroke_width * 1.5 } else { e.style.stroke_width };
    let stroke = Stroke::new(width * view.zoom, themed(with_opacity(e.style.stroke, e.style.opacity), view.dark));
    let mut pen = to(outlines.first().and_then(|o| o.first()).copied().unwrap_or([center.x, center.y]));
    'outer: for outline in &outlines {
        for w in outline.windows(2) {
            if budget <= 0.0 {
                break 'outer;
            }
            let seg = dist(w[0], w[1]);
            let t = if seg > 0.0 { (budget / seg).min(1.0) } else { 1.0 };
            let end = [w[0][0] + (w[1][0] - w[0][0]) * t, w[0][1] + (w[1][1] - w[0][1]) * t];
            painter.line_segment([to(w[0]), to(end)], stroke);
            pen = to(end);
            budget -= seg;
        }
    }
    if progress > 0.8 {
        part.style.opacity = (e.style.opacity as f32 * (progress - 0.8) / 0.2) as u8;
        paint_element(painter, &part, view, false, false, false);
    }
    pen
}

/// A collaborator-style pointer with the agent's name in a pill beside it.
pub(crate) fn paint_agent_cursor(painter: &Painter, at: Pos2, name: &str, alpha: f32) {
    let color = AGENT_COLOR.gamma_multiply(alpha);
    let arrow = [at, at + Vec2::new(0.0, 16.0), at + Vec2::new(4.5, 12.0), at + Vec2::new(11.0, 11.5)];
    painter.add(Shape::convex_polygon(arrow.to_vec(), color, Stroke::new(1.5, Color32::WHITE.gamma_multiply(alpha))));
    let galley = painter.layout_no_wrap(name.to_string(), egui::FontId::proportional(12.0), Color32::WHITE.gamma_multiply(alpha));
    let pill = Rect::from_min_size(at + Vec2::new(12.0, 16.0), galley.size() + Vec2::new(12.0, 6.0));
    painter.rect_filled(pill, 6, color);
    painter.galley(pill.min + Vec2::new(6.0, 3.0), galley, Color32::WHITE);
}
