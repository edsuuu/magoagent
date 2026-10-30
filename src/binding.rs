//! Arrows bound to shapes (orbit / inside), like the reference app.

use crate::prelude::*;

pub(crate) fn bindable(e: &Element) -> bool {
    !matches!(e.kind, ElementKind::Line { .. } | ElementKind::FreeDraw { .. })
}

/// getBindingGap: BASE_BINDING_GAP (5) + half the target's stroke.
pub(crate) fn binding_gap(e: &Element) -> f32 {
    5.0 + e.style.stroke_width / 2.0
}

/// World-space outline of a bindable element, grown by `grow`.
pub(crate) fn binding_outline(e: &Element, grow: f32) -> Vec<[f32; 2]> {
    let b = local_bounds(e).expand(grow);
    let (x, y, width, height) = (b.min.x, b.min.y, b.width(), b.height());
    let mut shape = e.clone();
    shape.kind = match e.kind {
        ElementKind::Ellipse { .. } => ElementKind::Ellipse { x, y, width, height },
        ElementKind::Diamond { .. } => ElementKind::Diamond { x, y, width, height },
        _ => ElementKind::Rectangle { x, y, width, height },
    };
    if matches!(e.kind, ElementKind::Text { .. }) {
        shape.style.round = false;
    }
    let c = b.center();
    shape_polygon(&shape)
        .unwrap_or_default()
        .into_iter()
        .map(|p| {
            let q = c + rot(Pos2::new(p[0], p[1]) - c, e.angle);
            [q.x, q.y]
        })
        .collect()
}

pub(crate) fn binding_for(target: &Element, world: Pos2, inside: bool) -> Binding {
    let b = local_bounds(target);
    let local = target.to_local(world) - b.min;
    let ratio = |v: f32, size: f32| if size > 0.0 { (v / size).clamp(0.0, 1.0) } else { 0.5 };
    Binding { element_id: target.id.clone(), fixed: [ratio(local.x, b.width()), ratio(local.y, b.height())], inside }
}

pub(crate) fn focus_point(target: &Element, binding: &Binding) -> Pos2 {
    let b = local_bounds(target);
    let p = b.min + Vec2::new(binding.fixed[0] * b.width(), binding.fixed[1] * b.height());
    b.center() + rot(p - b.center(), target.angle)
}

pub(crate) fn segment_intersection(a: Pos2, b: Pos2, c: [f32; 2], d: [f32; 2]) -> Option<Pos2> {
    let (c, d) = (Pos2::new(c[0], c[1]), Pos2::new(d[0], d[1]));
    let (r, s) = (b - a, d - c);
    let denom = r.x * s.y - r.y * s.x;
    if denom.abs() < 1e-6 {
        return None;
    }
    let t = ((c - a).x * s.y - (c - a).y * s.x) / denom;
    let u = ((c - a).x * r.y - (c - a).y * r.x) / denom;
    ((0.0..=1.0).contains(&t) && (0.0..=1.0).contains(&u)).then(|| a + r * t)
}

/// updateBoundPoint: "orbit" stops on the gap-grown outline along the line toward `other`.
pub(crate) fn bound_endpoint(target: &Element, binding: &Binding, other: Pos2) -> Pos2 {
    let focus = focus_point(target, binding);
    if binding.inside {
        return focus;
    }
    let poly = binding_outline(target, binding_gap(target));
    let n = poly.len();
    let hit = |to: Pos2| {
        (0..n).filter_map(|k| segment_intersection(focus, to, poly[k], poly[(k + 1) % n])).min_by(|a, b| a.distance(other).total_cmp(&b.distance(other)))
    };
    // `other` inside the outline (overlapping shapes): still leave through the side facing it,
    // never from the middle
    hit(other).or_else(|| hit(focus + (other - focus) * 1e4)).unwrap_or(focus)
}

/// Glues every bound arrow to its shapes (also used by the MCP server before saving).
pub(crate) fn resolve_bindings(elements: &mut [Element]) {
    for i in 0..elements.len() {
        if elements[i].ends.start.is_none() && elements[i].ends.end.is_none() {
            continue;
        }
        let mut arrow = elements[i].clone();
        if resolve_arrow(elements, &mut arrow) {
            elements[i] = arrow;
        }
    }
}

/// Recomputes a bound arrow's endpoints from its targets; drops bindings to missing shapes.
pub(crate) fn resolve_arrow(elements: &[Element], arrow: &mut Element) -> bool {
    let before = arrow.clone();
    let find = |b: &Option<Binding>| b.as_ref().and_then(|b| elements.iter().find(|e| e.id == b.element_id && e.id != arrow.id).map(|e| (e, b.clone())));
    let (start, end) = (find(&arrow.ends.start), find(&arrow.ends.end));
    if start.is_none() {
        arrow.ends.start = None;
    }
    if end.is_none() {
        arrow.ends.end = None;
    }
    if let ElementKind::Line { points, .. } = &mut arrow.kind {
        let n = points.len();
        if n >= 2 {
            let at = |k: usize| Pos2::new(points[k][0], points[k][1]);
            // a 2-point arrow aims at the other shape's anchor, longer ones at their neighbor point
            let start_aim = match (&end, n) {
                (Some((e, b)), 2) => focus_point(e, b),
                _ => at(1),
            };
            let end_aim = match (&start, n) {
                (Some((e, b)), 2) => focus_point(e, b),
                _ => at(n - 2),
            };
            if let Some((e, b)) = &start {
                let p = bound_endpoint(e, b, start_aim);
                points[0] = [p.x, p.y];
            }
            if let Some((e, b)) = &end {
                let p = bound_endpoint(e, b, end_aim);
                points[n - 1] = [p.x, p.y];
            }
        }
    }
    *arrow != before
}
