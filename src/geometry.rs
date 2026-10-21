//! Bounds, outlines, distances and hit-test helpers.

use crate::prelude::*;

pub(crate) fn distance_to_segment(point: [f32; 2], start: [f32; 2], end: [f32; 2]) -> f32 {
    let v = [end[0] - start[0], end[1] - start[1]];
    let w = [point[0] - start[0], point[1] - start[1]];
    let len_sq = v[0] * v[0] + v[1] * v[1];
    if len_sq == 0.0 {
        return dist(point, start);
    }
    let t = ((w[0] * v[0] + w[1] * v[1]) / len_sq).clamp(0.0, 1.0);
    dist(point, [start[0] + t * v[0], start[1] + t * v[1]])
}

pub(crate) fn polygon_contains(poly: &[[f32; 2]], p: [f32; 2]) -> bool {
    let mut inside = false;
    let n = poly.len();
    for i in 0..n {
        let (a, b) = (poly[i], poly[(i + n - 1) % n]);
        if (a[1] > p[1]) != (b[1] > p[1]) && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0] {
            inside = !inside;
        }
    }
    inside
}

/// the reference app's adaptive corner radius (packages/common/src/constants.ts ROUNDNESS).
pub(crate) fn corner_radius(size: f32) -> f32 {
    if size <= 128.0 { size * 0.25 } else { 32.0 }
}

pub(crate) fn box_corners(element: &Element) -> Option<(Vec<[f32; 2]>, f32)> {
    match &element.kind {
        ElementKind::Rectangle { x, y, width: w, height: h } => Some((
            vec![[*x, *y], [x + w, *y], [x + w, y + h], [*x, y + h]],
            // frames always have the web's fixed 8px corner
            if element.frame.is_some() { 8.0 } else if element.style.round { corner_radius(w.min(*h)) } else { 0.0 },
        )),
        ElementKind::Diamond { x, y, width: w, height: h } => Some((
            vec![[x + w / 2.0, *y], [x + w, y + h / 2.0], [x + w / 2.0, y + h], [*x, y + h / 2.0]],
            if element.style.round { corner_radius(w.min(*h)) * 0.5 } else { 0.0 },
        )),
        _ => None,
    }
}

/// Points a corner with radius `r` is cut at: (toward previous, toward next).
pub(crate) fn corner_cut(prev: [f32; 2], c: [f32; 2], next: [f32; 2], r: f32) -> ([f32; 2], [f32; 2]) {
    let toward = |o: [f32; 2]| {
        let (dx, dy) = (o[0] - c[0], o[1] - c[1]);
        let len = (dx * dx + dy * dy).sqrt().max(0.001);
        let r = r.min(len / 2.0);
        [c[0] + dx / len * r, c[1] + dy / len * r]
    };
    (toward(prev), toward(next))
}

pub(crate) fn quad(a: [f32; 2], c: [f32; 2], b: [f32; 2], steps: usize) -> Vec<[f32; 2]> {
    (0..=steps)
        .map(|i| {
            let t = i as f32 / steps as f32;
            let u = 1.0 - t;
            [u * u * a[0] + 2.0 * u * t * c[0] + t * t * b[0], u * u * a[1] + 2.0 * u * t * c[1] + t * t * b[1]]
        })
        .collect()
}

/// Closed outline (rounded where needed) used for fills and hit testing.
pub(crate) fn shape_polygon(element: &Element) -> Option<Vec<[f32; 2]>> {
    if let ElementKind::Ellipse { x, y, width: w, height: h } = &element.kind {
        let (cx, cy) = (x + w / 2.0, y + h / 2.0);
        return Some(
            (0..48)
                .map(|i| {
                    let t = i as f32 / 48.0 * std::f32::consts::TAU;
                    [cx + w / 2.0 * t.cos(), cy + h / 2.0 * t.sin()]
                })
                .collect(),
        );
    }
    let (corners, r) = box_corners(element)?;
    if r <= 0.0 {
        return Some(corners);
    }
    let n = corners.len();
    let mut out = Vec::new();
    for i in 0..n {
        let (a, b) = corner_cut(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n], r);
        out.extend(quad(a, corners[i], b, 6));
    }
    Some(out)
}

pub(crate) fn dist(a: [f32; 2], b: [f32; 2]) -> f32 {
    (a[0] - b[0]).hypot(a[1] - b[1])
}

/// Axis-aligned bounds in world space (rotation included).
pub(crate) fn element_bounds(e: &Element) -> Rect {
    let local = local_bounds(e);
    if e.angle == 0.0 {
        return local;
    }
    let c = local.center();
    [local.left_top(), local.right_top(), local.right_bottom(), local.left_bottom()]
        .map(|p| c + rot(p - c, e.angle))
        .into_iter()
        .fold(Rect::NOTHING, |r, p| r.union(Rect::from_min_max(p, p)))
}

/// Smooth curve through a line's points when it is curved (arrow type "curved").
pub(crate) fn line_path(e: &Element) -> Vec<[f32; 2]> {
    match &e.kind {
        ElementKind::Line { points, .. } if e.rope && points.len() == 2 => rope_curve(points[0], points[1]),
        ElementKind::Line { points, .. } if e.style.round && points.len() > 2 => {
            let mut ctl = vec![points[0]];
            ctl.extend(points);
            ctl.push(points[points.len() - 1]);
            catmull_rom(&ctl, LINE_CURVE_STEPS)
        }
        ElementKind::Line { points, .. } => points.clone(),
        _ => Vec::new(),
    }
}

/// Where a rope between two points hangs at rest: the middle control point of its curve, below
/// the straight line by more the longer the rope.
pub(crate) fn rope_rest(a: [f32; 2], b: [f32; 2]) -> [f32; 2] {
    let sag = (dist(a, b) * 0.12).max(20.0);
    [(a[0] + b[0]) / 2.0, (a[1] + b[1]) / 2.0 + 2.0 * sag]
}

/// The curve through a rope's ends pulled toward `control` (a quadratic Bézier).
pub(crate) fn rope_through(a: [f32; 2], control: [f32; 2], b: [f32; 2]) -> Vec<[f32; 2]> {
    (0..=24)
        .map(|i| {
            let t = i as f32 / 24.0;
            let (u, w) = ((1.0 - t) * (1.0 - t), 2.0 * t * (1.0 - t));
            [u * a[0] + w * control[0] + t * t * b[0], u * a[1] + w * control[1] + t * t * b[1]]
        })
        .collect()
}

/// A rope hanging at rest between two points.
pub(crate) fn rope_curve(a: [f32; 2], b: [f32; 2]) -> Vec<[f32; 2]> {
    rope_through(a, rope_rest(a, b), b)
}

/// Unrotated bounds of the element in its own frame.
pub(crate) fn local_bounds(e: &Element) -> Rect {
    match &e.kind {
        ElementKind::Rectangle { x, y, width, height }
        | ElementKind::Ellipse { x, y, width, height }
        | ElementKind::Diamond { x, y, width, height } => Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*width, *height)),
        ElementKind::Text { x, y, value } => {
            let size = e.style.font_size;
            let lines = text_lines(value);
            let longest = lines.iter().map(|l| l.chars().count()).max().unwrap_or(0) as f32;
            Rect::from_min_size(
                Pos2::new(*x, *y),
                Vec2::new((longest * size * CHAR_WIDTH).max(size * 0.5), lines.len() as f32 * size * LINE_HEIGHT),
            )
        }
        ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
            // a rope's box holds its hanging curve (line_path has none for freehand)
            let curve = if e.rope { line_path(e) } else { Vec::new() };
            let path = if curve.is_empty() { points } else { &curve };
            path.iter().map(|&p| Rect::from_min_max(p.into(), p.into())).reduce(Rect::union).unwrap_or(Rect::ZERO)
        }
    }
}
