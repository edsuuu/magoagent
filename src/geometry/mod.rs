//! Bounds, outlines, distances and hit-test helpers.

pub(crate) mod binding;
pub(crate) mod labels;
pub(crate) mod recognize;
pub(crate) mod rope;

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

/// the reference app's adaptive corner radius.
pub(crate) fn corner_radius(size: f32) -> f32 {
    if size <= 128.0 { size * 0.25 } else { 32.0 }
}

pub(crate) fn box_corners(element: &Element) -> Option<(Vec<[f32; 2]>, f32)> {
    match &element.kind {
        ElementKind::Rectangle { x, y, width: w, height: h } => Some((
            vec![[*x, *y], [x + w, *y], [x + w, y + h], [*x, y + h]],
            // frames always have the web's fixed 8px corner
            if element.frame.is_some() {
                8.0
            } else if element.style.round {
                corner_radius(w.min(*h))
            } else {
                0.0
            },
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

/// Whether what `paint_element` draws of `e` can reach `view` (world) at `zoom`: its box grown by what is painted past
/// it (the stroke and its hand jitter, arrowheads, an ellipse's wobble, a curve's swing between its points, a frame's
/// name above it, a text's halo).
pub(crate) fn may_show(e: &Element, view: Rect, zoom: f32) -> bool {
    let mut area = local_bounds(e);
    let curved = matches!(&e.kind, ElementKind::Line { points, .. } if e.style.round && points.len() > 2);
    // ponytail: a curve swings out of its points' box by less than a third of its size; its own path would be exact but costs its samples
    let swing = if curved { 0.3 * area.size().max_elem() } else { 0.05 * area.size().max_elem() };
    area = area.expand(40.0 + 2.0 * e.style.stroke_width + swing);
    if let (Some(name), ElementKind::Rectangle { x, y, .. }) = (&e.frame, &e.kind)
        && !name.is_empty()
    {
        // 13 points at the screen's scale (never below half), each letter at most one em
        let size = 13.0 * zoom.max(0.5) / zoom.max(f32::EPSILON);
        area = area.union(Rect::from_min_size(Pos2::new(*x, y - 4.0 - size * 1.5), Vec2::new(name.chars().count() as f32 * size, size * 1.5)));
    }
    if e.angle != 0.0 {
        area = Rect::from_center_size(area.center(), Vec2::splat(area.size().length()));
    }
    // the halo and the antialiasing: a few screen pixels
    area.expand(8.0 / zoom.max(f32::EPSILON)).intersects(view)
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

/// Size of a text in world units: its widest line in the hand font, one line height per line.
pub(crate) fn text_size(value: &str, size: f32) -> Vec2 {
    let lines = text_lines(value);
    let width = lines.iter().map(|l| text_advance(l, size)).fold(0.0, f32::max);
    Vec2::new(width.max(size * 0.5), lines.len() as f32 * size * LINE_HEIGHT)
}

/// Excalidraw's hand-drawn font, Excalifont (OFL 1.1, assets/Excalifont-license.txt), whole in one file.
pub(crate) const HAND_FACE: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Excalifont-Regular.ttf"));
pub(crate) const ASSISTANT_FONT: &[u8] = include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/assets/Assistant.ttf"));

/// Advance width of one line in the hand font, from the bundled faces in egui's fallback order,
/// so the box of a text is the width it is painted. Characters no face has (emoji, system
/// symbols) count one em. ponytail: a rough stand-in for egui's layout, measured without a ui.
pub(crate) fn text_advance(line: &str, size: f32) -> f32 {
    static FACES: std::sync::OnceLock<Vec<ttf_parser::Face<'static>>> = std::sync::OnceLock::new();
    let faces = FACES.get_or_init(|| [HAND_FACE, ASSISTANT_FONT].into_iter().filter_map(|data| ttf_parser::Face::parse(data, 0).ok()).collect());
    line.chars()
        .map(|ch| {
            faces
                .iter()
                .find_map(|face| {
                    let glyph = face.glyph_index(ch)?;
                    Some(face.glyph_hor_advance(glyph)? as f32 / face.units_per_em() as f32 * size)
                })
                .unwrap_or(size)
        })
        .sum()
}

/// Unrotated bounds of the element in its own frame.
pub(crate) fn local_bounds(e: &Element) -> Rect {
    match &e.kind {
        ElementKind::Rectangle { x, y, width, height } | ElementKind::Ellipse { x, y, width, height } | ElementKind::Diamond { x, y, width, height } => {
            Rect::from_min_size(Pos2::new(*x, *y), Vec2::new(*width, *height))
        }
        ElementKind::Text { x, y, value } => Rect::from_min_size(Pos2::new(*x, *y), text_size(value, e.style.font_size)),
        ElementKind::Line { points, .. } | ElementKind::FreeDraw { points } => {
            // a rope's box holds its hanging curve (line_path has none for freehand)
            let curve = if e.rope { line_path(e) } else { Vec::new() };
            let path = if curve.is_empty() { points } else { &curve };
            path.iter().map(|&p| Rect::from_min_max(p.into(), p.into())).reduce(Rect::union).unwrap_or(Rect::ZERO)
        }
    }
}

/// Where a window the size of `want` goes among `elements`: at `want` when that is free, else in the rings around
/// `around` (the window that opens it, or `want` itself), lined up with its edges 60 away: first below, right, left and
/// above it, then its corners, then the next ring out, overlapping no window or note. What an agent opens stays around
/// it, not piled up on one side (asked: "a posição dos terminais, às vezes ele junta tudo na direita").
pub(crate) fn free_spot_among<'a>(elements: impl Iterator<Item = &'a Element>, want: Rect, around: Rect) -> Rect {
    const GAP: f32 = 60.0;
    let taken: Vec<Rect> = elements.filter(|e| e.is_node() || e.created.is_some()).map(|e| element_bounds(e).expand(20.0)).collect();
    let free = |spot: &Rect| !taken.iter().any(|t| t.intersects(*spot));
    if free(&want) {
        return want;
    }
    let size = want.size();
    // column or row `k` of the rings: 0 lines up with `around`, past it one window (and the gap) at a time
    let place = |k: i32, low: f32, high: f32, length: f32| match k {
        0 => low,
        k if k > 0 => high + GAP + (k - 1) as f32 * (length + GAP),
        k => low - GAP - length - (-k - 1) as f32 * (length + GAP),
    };
    // ponytail: six rings; a canvas packed past that overlaps at `want`
    let mut spots: Vec<(i32, i32)> = (-6..=6).flat_map(|column| (-6..=6).map(move |row| (column, row))).filter(|spot| *spot != (0, 0)).collect();
    spots.sort_by_key(|&(column, row)| (column.abs().max(row.abs()), column.abs() + row.abs(), row < 0, column < 0, row == 0));
    spots
        .into_iter()
        .map(|(column, row)| {
            let x = place(column, around.min.x, around.max.x, size.x);
            let y = place(row, around.min.y, around.max.y, size.y);
            Rect::from_min_size(Pos2::new(x, y), size)
        })
        .find(free)
        .unwrap_or(want)
}

/// Where `spot` sits from `of`, in the words an agent reads ("aberto à sua direita").
pub(crate) fn side_of(spot: Rect, of: Rect) -> &'static str {
    let away = spot.center() - of.center();
    // by how far past the other it is on each axis, in its own size
    let (across, down) = (away.x / (spot.width() + of.width()), away.y / (spot.height() + of.height()));
    match (across.abs() >= down.abs(), across >= 0.0, down >= 0.0) {
        (true, true, _) => "à sua direita",
        (true, false, _) => "à sua esquerda",
        (false, _, true) => "abaixo de você",
        (false, _, false) => "acima de você",
    }
}
