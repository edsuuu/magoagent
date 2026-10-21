//! Hand-drawn (rough.js-like) strokes, hachures and arrowheads.

use crate::prelude::*;

/// Deterministic jitter seeded by element id, so the sketch doesn't wobble between frames.
pub(crate) struct Rng(pub(crate) u64);

impl Rng {
    pub(crate) fn new(seed: &str, pass: u64) -> Self {
        let hash = seed.bytes().fold(0xcbf29ce484222325_u64, |h, b| (h ^ b as u64).wrapping_mul(0x100000001b3));
        Self((hash ^ pass.wrapping_mul(0x9e3779b97f4a7c15)) | 1)
    }

    /// Uniform in [-1, 1].
    pub(crate) fn next(&mut self) -> f32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 40) as f32 / (1u64 << 23) as f32 - 1.0
    }
}

pub(crate) fn catmull_rom(points: &[[f32; 2]], steps: usize) -> Vec<[f32; 2]> {
    let mut out = vec![points[1]];
    for i in 1..points.len() - 2 {
        let (p0, p1, p2, p3) = (points[i - 1], points[i], points[i + 1], points[i + 2]);
        for step in 1..=steps {
            let t = step as f32 / steps as f32;
            let (t2, t3) = (t * t, t * t * t);
            let f = |a: f32, b: f32, c: f32, d: f32| {
                0.5 * (2.0 * b + (c - a) * t + (2.0 * a - 5.0 * b + 4.0 * c - d) * t2 + (3.0 * b - a - 3.0 * c + d) * t3)
            };
            out.push([f(p0[0], p1[0], p2[0], p3[0]), f(p0[1], p1[1], p2[1], p3[1])]);
        }
    }
    out
}

/// One rough.js-style stroke: jittered endpoints and a slight bow, scaled by roughness.
pub(crate) fn rough_line(a: [f32; 2], b: [f32; 2], rng: &mut Rng, roughness: f32) -> Vec<[f32; 2]> {
    let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
    let len = (dx * dx + dy * dy).sqrt().max(0.001);
    let off = (len / 10.0).min(1.5) * roughness;
    let bow = (len / 100.0).min(4.0) * roughness;
    let (nx, ny) = (-dy / len, dx / len);
    let mut at = |t: f32, bend: f32| {
        let k = bend * rng.next();
        [a[0] + dx * t + nx * k + off * rng.next(), a[1] + dy * t + ny * k + off * rng.next()]
    };
    let (p0, m1, m2, p3) = (at(0.0, 0.0), at(0.5, bow), at(0.75, bow), at(1.0, 0.0));
    catmull_rom(&[p0, p0, m1, m2, p3, p3], 6)
}

pub(crate) fn rough_outlines(element: &Element) -> Vec<Vec<[f32; 2]>> {
    let roughness = element.style.roughness as f32;
    let passes = if roughness == 0.0 || element.style.stroke_style != StrokeStyle::Solid { 1 } else { 2 };
    let mut out = Vec::new();
    if let Some((corners, r)) = box_corners(element) {
        let n = corners.len();
        for pass in 0..passes {
            let mut rng = Rng::new(&element.id, pass);
            let cuts: Vec<_> = (0..n)
                .map(|i| corner_cut(corners[(i + n - 1) % n], corners[i], corners[(i + 1) % n], r))
                .collect();
            if r <= 0.0 {
                // sharp corners: separate overshooting strokes, like rough.js
                for i in 0..n {
                    out.push(rough_line(corners[i], corners[(i + 1) % n], &mut rng, roughness));
                }
                continue;
            }
            // rounded: one continuous path per pass (arc, edge, arc, edge...)
            let mut path = Vec::new();
            for i in 0..n {
                let j = (i + 1) % n;
                let wobble = 0.6 * roughness;
                let c = [corners[i][0] + wobble * rng.next(), corners[i][1] + wobble * rng.next()];
                path.extend(quad(cuts[i].0, c, cuts[i].1, 6));
                let mut edge = rough_line(cuts[i].1, cuts[j].0, &mut rng, roughness);
                let last = edge.len() - 1;
                (edge[0], edge[last]) = (cuts[i].1, cuts[j].0);
                path.extend(&edge[1..last]);
            }
            path.push(path[0]);
            out.push(path);
        }
        return out;
    }
    match &element.kind {
        ElementKind::Ellipse { x, y, width: w, height: h } => {
            let (cx, cy) = (x + w / 2.0, y + h / 2.0);
            for pass in 0..passes {
                let mut rng = Rng::new(&element.id, pass);
                let n = 10;
                let start = rng.next() * std::f32::consts::PI;
                let ctl: Vec<[f32; 2]> = (0..n)
                    .map(|i| {
                        let t = start + i as f32 / n as f32 * std::f32::consts::TAU;
                        let k = 1.0 + 0.025 * roughness * rng.next();
                        [
                            cx + w / 2.0 * k * t.cos() + roughness * rng.next(),
                            cy + h / 2.0 * k * t.sin() + roughness * rng.next(),
                        ]
                    })
                    .collect();
                // ponytail: closes with a fixed overlap; rough.js also varies overlap per pass
                let mut ring = vec![ctl[n - 1]];
                ring.extend(&ctl);
                ring.extend([ctl[0], ctl[1], if roughness > 0.0 { ctl[2] } else { ctl[1] }]);
                out.push(catmull_rom(&ring, 8));
            }
        }
        ElementKind::Line { points, .. } => {
            let curved = (element.style.round && points.len() > 2) || element.rope;
            let path = line_path(element);
            for pass in 0..passes {
                let mut rng = Rng::new(&element.id, pass);
                if curved {
                    // one wobbly pass along the smooth curve instead of per-segment strokes
                    let wobble = roughness * 0.8;
                    let (dx, dy) = (wobble * rng.next(), wobble * rng.next());
                    out.push(path.iter().map(|p| [p[0] + dx, p[1] + dy]).collect());
                } else {
                    for pair in points.windows(2) {
                        out.push(rough_line(pair[0], pair[1], &mut rng, roughness));
                    }
                }
            }
        }
        ElementKind::FreeDraw { points } => out.push(points.clone()),
        _ => {}
    }
    out
}

/// Arrowhead outlines at both ends (getArrowheadPoints: size 25/15/12, angle 20°/25°/90°).
pub(crate) fn arrowheads(element: &Element) -> Vec<(Vec<[f32; 2]>, Head)> {
    let ElementKind::Line { points, arrow: true } = &element.kind else {
        return Vec::new();
    };
    if points.len() < 2 {
        return Vec::new();
    }
    let path = line_path(element);
    let step = if path.len() != points.len() { 3 } else { 1 }; // a little back along a curve
    let n = points.len();
    let v = |p: [f32; 2]| Pos2::new(p[0], p[1]);
    let mut out = Vec::new();
    for (head, at_end) in [(element.ends.start_head, false), (element.ends.end_head, true)] {
        let Some(head) = head else {
            continue;
        };
        let (tip, from, segment) = if at_end {
            (v(path[path.len() - 1]), v(path[path.len() - 1 - step]), v(points[n - 1]) - v(points[n - 2]))
        } else {
            (v(path[0]), v(path[step]), v(points[0]) - v(points[1]))
        };
        let dir = (tip - from).normalized();
        let diamond = matches!(head, Head::Diamond | Head::DiamondOutline);
        let size = match head {
            Head::Arrow => 25.0,
            _ if diamond => 12.0,
            _ => 15.0,
        };
        let size = f32::min(size, segment.length() * if diamond { 0.25 } else { 0.5 });
        let angle = match head {
            Head::Arrow => 20.0_f32,
            Head::Bar => 90.0,
            _ => 25.0,
        }
        .to_radians();
        let back = tip - dir * size;
        let side = |a: f32| tip + rot(back - tip, a);
        let pts: Vec<Pos2> = match head {
            Head::Arrow => vec![side(-angle), tip, side(angle)],
            Head::Bar => vec![side(-angle), side(angle)],
            Head::Triangle | Head::TriangleOutline => vec![tip, side(-angle), side(angle)],
            Head::Diamond | Head::DiamondOutline => vec![tip, side(-angle), tip - dir * size * 2.0, side(angle)],
            Head::Circle | Head::CircleOutline => {
                let r = (size + element.style.stroke_width - 2.0).max(2.0) / 2.0;
                let c = tip - dir * r;
                (0..20).map(|k| c + Vec2::angled(k as f32 / 20.0 * std::f32::consts::TAU) * r).collect()
            }
        };
        out.push((pts.iter().map(|p| [p.x, p.y]).collect(), head));
    }
    out
}

/// Hachure / cross-hatch lines clipped to the shape (rough.js: gap = 4 × stroke, angle −41°).
pub(crate) fn fill_strokes(element: &Element) -> Vec<Vec<[f32; 2]>> {
    let (Some(poly), true) = (shape_polygon(element), element.style.fill_style != FillStyle::Solid) else {
        return Vec::new();
    };
    let gap = (element.style.stroke_width * 4.0).max(4.0);
    let mut angles = vec![-41.0_f32];
    if element.style.fill_style == FillStyle::CrossHatch {
        angles.push(49.0);
    }
    let mut rng = Rng::new(&element.id, 7);
    let mut out = Vec::new();
    for angle in angles {
        let (sin, cos) = angle.to_radians().sin_cos();
        let rot = |p: [f32; 2], s: f32| [p[0] * cos + p[1] * s * sin, -p[0] * s * sin + p[1] * cos];
        let rotated: Vec<[f32; 2]> = poly.iter().map(|p| rot(*p, 1.0)).collect();
        let (min_y, max_y) = rotated.iter().fold((f32::MAX, f32::MIN), |(lo, hi), p| (lo.min(p[1]), hi.max(p[1])));
        let mut y = min_y + gap / 2.0;
        while y < max_y {
            let n = rotated.len();
            let mut xs: Vec<f32> = (0..n)
                .filter_map(|i| {
                    let (a, b) = (rotated[i], rotated[(i + 1) % n]);
                    ((a[1] <= y) != (b[1] <= y)).then(|| a[0] + (y - a[1]) / (b[1] - a[1]) * (b[0] - a[0]))
                })
                .collect();
            xs.sort_by(f32::total_cmp);
            for pair in xs.chunks_exact(2) {
                let a = rot([pair[0], y], -1.0);
                let b = rot([pair[1], y], -1.0);
                out.push(rough_line(a, b, &mut rng, element.style.roughness as f32 * 0.5));
            }
            y += gap;
        }
    }
    out
}
