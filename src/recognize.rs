//! Desenhar para criar forma: the moment-based shape recognizer.

use crate::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Recognized {
    Rectangle,
    Diamond,
    Ellipse,
    Line,
    Arrow,
}

/// The web's moment-based shape recognizer (convertToShape.ts): closed strokes are matched
/// against rectangle/diamond/ellipse prototypes, open straight ones become lines, or arrows
/// when the arrowhead makes them lopsided. None means "not a shape".
pub(crate) fn classify_stroke(points: &[[f32; 2]], zoom: f32) -> Option<Recognized> {
    let (min, max) = points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    if points.len() < 3 || (max[0] - min[0]).max(max[1] - min[1]) * zoom < 25.0 {
        return None;
    }
    let pts = resample(points, 64);
    let path: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let gap_ratio = if path > 0.0 { dist(pts[0], pts[63]) / path } else { 0.0 };
    let moments = |values: &[f32], order: i32| {
        let n = values.len() as f32;
        let mean = values.iter().sum::<f32>() / n;
        let var = values.iter().map(|v| (v - mean).powi(2)).sum::<f32>() / n;
        let m = values.iter().map(|v| (v - mean).powi(order)).sum::<f32>() / n;
        if var.sqrt() > f32::EPSILON { m / var.sqrt().powi(order) } else { 0.0 }
    };
    if gap_ratio > 0.15 {
        // open: principal axes of the point cloud
        let n = pts.len() as f32;
        let c = [pts.iter().map(|p| p[0]).sum::<f32>() / n, pts.iter().map(|p| p[1]).sum::<f32>() / n];
        let (mut m20, mut m02, mut m11) = (0.0, 0.0, 0.0);
        for p in &pts {
            let (dx, dy) = (p[0] - c[0], p[1] - c[1]);
            (m20, m02, m11) = (m20 + dx * dx / n, m02 + dy * dy / n, m11 + dx * dy / n);
        }
        let diff = (m20 - m02).hypot(2.0 * m11);
        let (major_var, minor_var) = ((m20 + m02 + diff) / 2.0, (m20 + m02 - diff) / 2.0);
        let major = if m11.abs() > f32::EPSILON {
            Vec2::new(major_var - m02, m11).normalized()
        } else if m20 >= m02 {
            Vec2::X
        } else {
            Vec2::Y
        };
        let elongation = if major_var > 0.0 { minor_var / major_var } else { 1.0 };
        let u: Vec<f32> = pts.iter().map(|p| (p[0] - c[0]) * major.x + (p[1] - c[1]) * major.y).collect();
        // the shaft runs from the start to the farthest point; the arrowhead zone around it is exempt
        let tip = pts.iter().copied().max_by(|a, b| dist(pts[0], *a).total_cmp(&dist(pts[0], *b)))?;
        let tip_distance = dist(pts[0], tip);
        let shaft = pts.iter().filter(|p| dist(**p, tip) > 0.5 * tip_distance).map(|p| distance_to_segment(*p, pts[0], tip)).fold(0.0, f32::max);
        if elongation > 0.25 || (tip_distance > 0.0 && shaft / tip_distance > 0.15) {
            return None;
        }
        return Some(if moments(&u, 3).abs() >= 0.3 { Recognized::Arrow } else { Recognized::Line });
    }
    // closed: hull fill, corner sharpness and the kurtosis product against each prototype
    let box_area = (max[0] - min[0]) * (max[1] - min[1]);
    let hull = convex_hull(&pts);
    let hull_area = (0..hull.len())
        .map(|i| {
            let (a, b) = (hull[i], hull[(i + 1) % hull.len()]);
            a[0] * b[1] - b[0] * a[1]
        })
        .sum::<f32>()
        .abs()
        / 2.0;
    let hull_fill = if box_area > 0.0 { hull_area / box_area } else { 0.0 };
    let turns: Vec<f32> = (3..pts.len() - 3)
        .map(|i| {
            let (a, b, c) = (pts[i - 3], pts[i], pts[i + 3]);
            let (v1, v2) = ([b[0] - a[0], b[1] - a[1]], [c[0] - b[0], c[1] - b[1]]);
            (v1[0] * v2[1] - v1[1] * v2[0]).atan2(v1[0] * v2[0] + v1[1] * v2[1]).abs()
        })
        .collect();
    let total: f32 = turns.iter().sum();
    let mut taken = vec![false; turns.len()];
    let mut top4 = 0.0;
    for _ in 0..4 {
        let Some(peak) = (0..turns.len()).filter(|i| !taken[*i] && turns[*i] > 0.0).max_by(|a, b| turns[*a].total_cmp(&turns[*b])) else {
            break;
        };
        for i in peak.saturating_sub(3)..=(peak + 3).min(turns.len() - 1) {
            if !taken[i] {
                top4 += turns[i];
                taken[i] = true;
            }
        }
    }
    let corner_share = if total > 0.0 { top4 / total } else { 0.0 };
    let xs: Vec<f32> = pts.iter().map(|p| p[0]).collect();
    let ys: Vec<f32> = pts.iter().map(|p| p[1]).collect();
    let kurtosis = moments(&xs, 4) * moments(&ys, 4);
    [(Recognized::Rectangle, 1.0, 0.95, 1.83), (Recognized::Diamond, 0.5, 0.95, 3.24), (Recognized::Ellipse, std::f32::consts::FRAC_PI_4, 0.55, 2.25)]
        .into_iter()
        .map(|(shape, fill, corners, kurt)| {
            let d = ((hull_fill - fill) / 0.2).hypot((corner_share - corners) / 0.2).hypot((kurtosis - kurt) / 0.7);
            (shape, d)
        })
        .filter(|(_, d)| *d < 1.5)
        .min_by(|a, b| a.1.total_cmp(&b.1))
        .map(|(shape, _)| shape)
}

/// `n` points evenly spaced along the stroke.
pub(crate) fn resample(pts: &[[f32; 2]], n: usize) -> Vec<[f32; 2]> {
    let total: f32 = pts.windows(2).map(|w| dist(w[0], w[1])).sum();
    let step = total / (n - 1) as f32;
    let mut out = vec![pts[0]];
    let (mut prev, mut carried) = (pts[0], 0.0);
    for &p in &pts[1..] {
        let mut seg = dist(prev, p);
        while step > 0.0 && carried + seg >= step && out.len() < n {
            let t = (step - carried) / seg;
            prev = [prev[0] + t * (p[0] - prev[0]), prev[1] + t * (p[1] - prev[1])];
            out.push(prev);
            seg = dist(prev, p);
            carried = 0.0;
        }
        carried += seg;
        prev = p;
    }
    out.resize(n, pts[pts.len() - 1]);
    out
}

/// Andrew's monotone chain.
pub(crate) fn convex_hull(points: &[[f32; 2]]) -> Vec<[f32; 2]> {
    let mut sorted = points.to_vec();
    sorted.sort_by(|a, b| a[0].total_cmp(&b[0]).then(a[1].total_cmp(&b[1])));
    let cross = |o: [f32; 2], a: [f32; 2], b: [f32; 2]| (a[0] - o[0]) * (b[1] - o[1]) - (a[1] - o[1]) * (b[0] - o[0]);
    let half = |pts: &mut dyn Iterator<Item = [f32; 2]>| {
        let mut chain: Vec<[f32; 2]> = Vec::new();
        for p in pts {
            while chain.len() >= 2 && cross(chain[chain.len() - 2], chain[chain.len() - 1], p) <= 0.0 {
                chain.pop();
            }
            chain.push(p);
        }
        chain.pop();
        chain
    };
    let mut hull = half(&mut sorted.iter().copied());
    hull.extend(half(&mut sorted.iter().rev().copied()));
    if hull.len() >= 3 { hull } else { points.to_vec() }
}

/// Desenhar para criar forma: the element a stroke becomes, or None (the stroke just fades).
/// A line from or to a shape is a connector, so it becomes a bound arrow.
pub(crate) fn recognize_shape(app: &DrawingApp, points: &[[f32; 2]]) -> Option<Element> {
    let shape = classify_stroke(points, app.zoom)?;
    let (min, max) = points.iter().fold(([f32::MAX; 2], [f32::MIN; 2]), |(lo, hi), p| ([lo[0].min(p[0]), lo[1].min(p[1])], [hi[0].max(p[0]), hi[1].max(p[1])]));
    let (x, y, width, height) = (min[0], min[1], max[0] - min[0], max[1] - min[1]);
    let start = points[0];
    let kind = match shape {
        Recognized::Rectangle => ElementKind::Rectangle { x, y, width, height },
        Recognized::Diamond => ElementKind::Diamond { x, y, width, height },
        Recognized::Ellipse => ElementKind::Ellipse { x, y, width, height },
        Recognized::Line => ElementKind::Line { points: vec![start, points[points.len() - 1]], arrow: false },
        Recognized::Arrow => {
            // the tip is the input point nearest the bounding-box spot farthest from the start
            let spots = [
                [x, y],
                [x + width / 2.0, y],
                [x + width, y],
                [x + width, y + height / 2.0],
                [x + width, y + height],
                [x + width / 2.0, y + height],
                [x, y + height],
                [x, y + height / 2.0],
            ];
            let ideal = spots.into_iter().max_by(|a, b| dist(*a, start).total_cmp(&dist(*b, start)))?;
            let tip = points.iter().copied().min_by(|a, b| dist(*a, ideal).total_cmp(&dist(*b, ideal)))?;
            ElementKind::Line { points: vec![start, tip], arrow: dist(start, tip) >= 60.0 }
        }
    };
    let mut element = app.new_element(kind);
    if let ElementKind::Line { points, arrow } = &mut element.kind {
        let ends = [Pos2::new(points[0][0], points[0][1]), Pos2::new(points[1][0], points[1][1])];
        let targets = ends.map(|p| app.binding_target(p, None));
        if !*arrow && targets.iter().any(Option::is_some) && targets[0] != targets[1] {
            *arrow = true;
            element.ends.start_head = None;
        }
        if *arrow {
            return Some(app.bind_new_arrow(element));
        }
    }
    Some(element)
}
