use super::*;

/// Organizar's move for each box: into cells as big as the biggest box, GRID apart, in reading
/// order (top to bottom, then left to right) from the boxes' top left corner. Layout 0 is a grid
/// of ⌈√n⌉ columns, 1 a row, 2 a column. None under two boxes.
// ponytail: the reference doesn't say which layouts it cycles through; grid, row and column are ours
pub(crate) fn tidy_deltas(boxes: &[Rect], layout: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|a, b| a.union(b))?;
    let cell = boxes.iter().fold(Vec2::ZERO, |m, b| m.max(b.size())) + Vec2::splat(GRID);
    let columns = match layout {
        1 => boxes.len(),
        2 => 1,
        _ => (boxes.len() as f32).sqrt().ceil() as usize,
    };
    let mut order: Vec<usize> = (0..boxes.len()).collect();
    order.sort_by(|&a, &b| boxes[a].min.y.total_cmp(&boxes[b].min.y).then(boxes[a].min.x.total_cmp(&boxes[b].min.x)));
    let mut deltas = vec![Vec2::ZERO; boxes.len()];
    for (k, &i) in order.iter().enumerate() {
        let at = union.min + Vec2::new((k % columns) as f32 * cell.x, (k / columns) as f32 * cell.y);
        deltas[i] = at - boxes[i].min;
    }
    Some(deltas)
}

/// Alinhar's move for each box: to the boxes' common edge or center (0 left, 1 center, 2 right,
/// 3 top, 4 middle, 5 bottom) or spread with even gaps (6 across, 7 down). None under two boxes.
pub(crate) fn align_deltas(boxes: &[Rect], how: usize) -> Option<Vec<Vec2>> {
    if boxes.len() < 2 {
        return None;
    }
    let union = boxes.iter().copied().reduce(|a, b| a.union(b))?;
    match how {
        0..=5 => Some(
            boxes
                .iter()
                .map(|b| match how {
                    0 => Vec2::new(union.min.x - b.min.x, 0.0),
                    1 => Vec2::new(union.center().x - b.center().x, 0.0),
                    2 => Vec2::new(union.max.x - b.max.x, 0.0),
                    3 => Vec2::new(0.0, union.min.y - b.min.y),
                    4 => Vec2::new(0.0, union.center().y - b.center().y),
                    _ => Vec2::new(0.0, union.max.y - b.max.y),
                })
                .collect(),
        ),
        6 | 7 => {
            let horizontal = how == 6;
            let start = |b: &Rect| if horizontal { b.min.x } else { b.min.y };
            let span = |b: &Rect| if horizontal { b.width() } else { b.height() };
            let mut order: Vec<usize> = (0..boxes.len()).collect();
            order.sort_by(|a, b| start(&boxes[*a]).total_cmp(&start(&boxes[*b])));
            let total: f32 = boxes.iter().map(span).sum();
            let gap = (span(&union) - total) / (boxes.len() - 1) as f32;
            let mut deltas = vec![Vec2::ZERO; boxes.len()];
            let mut at = start(&union);
            for k in order {
                let d = at - start(&boxes[k]);
                deltas[k] = if horizontal { Vec2::new(d, 0.0) } else { Vec2::new(0.0, d) };
                at += span(&boxes[k]) + gap;
            }
            Some(deltas)
        }
        _ => None,
    }
}
